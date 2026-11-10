//! Bound resource expansion before usvg recursively resolves patterns, uses and other links.

use std::collections::{HashMap, VecDeque};

use roxmltree::{Document, Node, NodeId, ParsingOptions};

use crate::IoError;

const MAX_NODES: u32 = 1_000_000;
const MAX_WORK: usize = 10_000_000;
// Resolving a resource uses several parser frames in addition to its XML children.
const LINK_DEPTH: usize = 4;

type ResourceIds<'a> = HashMap<&'a str, (NodeId, NodeId)>;
type Graph = Vec<Vec<(usize, usize)>>;

fn too_costly() -> IoError {
    IoError::Svg("the drawing's references are too costly to parse (deep or cyclic SVG resources)".into())
}

/// XML and resource expansion share the same budget, including inside embedded drawings.
pub(super) fn checked(xml: &[u8], limit: usize) -> Result<usize, IoError> {
    let depth = super::nesting_depth(xml, limit)?;
    if depth > limit {
        return Ok(depth);
    }
    if !xml.contains(&b'#') {
        return Ok(depth);
    }
    let text = std::str::from_utf8(xml).map_err(|_| IoError::Svg("the SVG is not valid UTF-8".into()))?;
    // nesting_depth already bounded the parser and rejected entity-held markup. Keep the
    // text-only DTD entities supported by usvg, which the general XML helper refuses.
    let doc = roxmltree::Document::parse_with_options(text, ParsingOptions { allow_dtd: true, nodes_limit: MAX_NODES, ..Default::default() })
        .map_err(|error| IoError::Svg(error.to_string()))?;
    let mut ids = ResourceIds::new();
    for node in doc.descendants() {
        if let Some(id) = node.attribute("id") {
            // usvg resolves use-elements against the first duplicate id and paints against
            // the last. Inspect both possibilities before either resolution can recurse.
            ids.entry(id).and_modify(|(_, last)| *last = node.id()).or_insert((node.id(), node.id()));
        }
    }
    if ids.is_empty() {
        return Ok(depth);
    }
    let graph = reference_graph(&doc, &ids)?;
    let mut best = vec![0usize; graph.len()];
    let mut queue = VecDeque::from([(doc.root().id().get_usize(), 0usize)]);
    let mut deepest = depth;
    let mut work = 0usize;
    while let Some((index, depth)) = queue.pop_front() {
        let Some(edges) = graph.get(index) else { return Err(too_costly()) };
        for &(child, weight) in edges {
            work = work.saturating_add(1);
            let next = depth.saturating_add(weight);
            if next > limit || work > MAX_WORK {
                return Err(too_costly());
            }
            let Some(previous) = best.get_mut(child) else { return Err(too_costly()) };
            if next > *previous {
                *previous = next;
                deepest = deepest.max(next);
                queue.push_back((child, next));
            }
        }
    }
    Ok(deepest)
}

fn reference_graph(doc: &Document<'_>, ids: &ResourceIds<'_>) -> Result<Graph, IoError> {
    let mut sheet = simplecss::StyleSheet::new();
    for node in doc.descendants().filter(|node| node.has_tag_name("style")) {
        if let Some(text) = node.text() {
            sheet.parse_more(text);
        }
    }
    let mut graph = vec![Vec::new(); doc.descendants().count()];
    let mut work = 0usize;
    for node in doc.descendants() {
        let Some(edges) = graph.get_mut(node.id().get_usize()) else { return Err(too_costly()) };
        edges.extend(node.children().filter(Node::is_element).map(|child| (child.id().get_usize(), 1)));
        let mut link = |name: &str, value: &str| {
            if name == "href" && node.has_tag_name("a") {
                return;
            }
            references(name, value, |id| {
                if let Some(&(first, last)) = ids.get(id) {
                    edges.push((first.get_usize(), LINK_DEPTH));
                    if last != first {
                        edges.push((last.get_usize(), LINK_DEPTH));
                    }
                }
            });
        };
        for attribute in node.attributes() {
            if attribute.name() == "style" {
                for declaration in simplecss::DeclarationTokenizer::from(attribute.value()) {
                    link(declaration.name, declaration.value);
                }
            } else {
                link(attribute.name(), attribute.value());
            }
        }
        for rule in &sheet.rules {
            work = work.saturating_add(1);
            if work > MAX_WORK {
                return Err(too_costly());
            }
            if node.is_element() && rule.selector.matches(&Element(node)) {
                for declaration in &rule.declarations {
                    link(declaration.name, declaration.value);
                }
            }
        }
        work = work.saturating_add(edges.len());
        if work > MAX_WORK {
            return Err(too_costly());
        }
    }
    Ok(graph)
}

fn fragment<'a>(name: &str, value: &'a str) -> Option<&'a str> {
    match name {
        "href" => svgtypes::IRI::from_str(value).ok().map(|iri| iri.0),
        "fill" | "stroke" => match svgtypes::Paint::from_str(value).ok()? {
            svgtypes::Paint::FuncIRI(id, _) => Some(id),
            _ => None,
        },
        "clip-path" | "mask" | "marker" | "marker-start" | "marker-mid" | "marker-end" => svgtypes::FuncIRI::from_str(value).ok().map(|iri| iri.0),
        _ => None,
    }
}

fn references(name: &str, value: &str, mut add: impl FnMut(&str)) {
    if name == "filter" {
        for value in svgtypes::FilterValueListParser::from(value) {
            if let Ok(svgtypes::FilterValue::Url(id)) = value {
                add(id);
            }
        }
    } else if let Some(id) = fragment(name, value) {
        add(id);
    }
}

struct Element<'a, 'input>(Node<'a, 'input>);

impl simplecss::Element for Element<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(Self)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(Self)
    }

    fn has_local_name(&self, name: &str) -> bool {
        self.0.has_tag_name(name)
    }

    fn attribute_matches(&self, name: &str, operator: simplecss::AttributeOperator<'_>) -> bool {
        self.0.attribute(name).is_some_and(|value| operator.matches(value))
    }

    fn pseudo_class_matches(&self, class: simplecss::PseudoClass<'_>) -> bool {
        match class {
            simplecss::PseudoClass::FirstChild => self.0.prev_sibling_element().is_none(),
            simplecss::PseudoClass::Link => self.0.has_tag_name("a"),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::checked;

    fn patterns(depth: usize, mode: &str) -> String {
        let mut definitions = String::new();
        let mut css = String::new();
        for index in 0..depth {
            let next = index + 1;
            let (attributes, body) = if next == depth {
                (String::new(), "<rect width=\"1\" height=\"1\"/>".into())
            } else {
                match mode {
                    "href" => (format!(" href=\"#p{next}\""), String::new()),
                    "inline" => (String::new(), format!("<rect style=\"fill:url('#p{next}') red\"/>")),
                    "css" => {
                        css.push_str(&format!(".next{index} {{fill:url(#p{next})}}"));
                        (String::new(), format!("<rect class=\"next{index}\"/>"))
                    }
                    _ => (String::new(), format!("<rect fill=\"url(#p{next})\"/>")),
                }
            };
            definitions.push_str(&format!("<pattern id=\"p{index}\"{attributes}>{body}</pattern>"));
        }
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><style>{css}</style>{definitions}<rect fill=\"url(#p0)\"/></svg>")
    }

    #[test]
    fn resource_chains_are_bounded_in_attributes_and_css() {
        for mode in ["attribute", "inline", "css", "href"] {
            assert!(checked(patterns(210, mode).as_bytes(), 128).is_err(), "{mode}");
            assert!(checked(patterns(4, mode).as_bytes(), 128).is_ok(), "{mode}");
        }
    }

    #[test]
    fn independent_resources_and_hyperlinks_do_not_consume_a_chain_budget() {
        let patterns: String = (0..300).map(|index| format!("<pattern id=\"p{index}\"><rect/></pattern>")).collect();
        let xml = format!("<svg>{patterns}<a id=\"link\" href=\"#link\"><rect/></a></svg>");
        assert!(checked(xml.as_bytes(), 128).is_ok());
    }

    #[test]
    fn resource_and_embedded_xml_depth_share_the_budget() {
        let xml = patterns(4, "attribute");
        let depth = checked(xml.as_bytes(), 128).unwrap();
        assert!(depth > 4);
        assert!(checked(xml.as_bytes(), depth - 1).is_err());
        assert!(checked(b"<svg><pattern id='p'><rect fill='url(#p)'/></pattern></svg>", 128).is_err());
    }

    #[test]
    fn duplicate_ids_and_filter_lists_cannot_hide_resource_links() {
        let xml = patterns(210, "attribute");
        for xml in [xml.replace("<style>", "<rect id='p0'/><style>"), xml.replace("</svg>", "<rect id='p0'/></svg>")] {
            assert!(checked(xml.as_bytes(), 128).is_err());
        }
        assert!(checked(b"<svg><filter id='a'><g filter='url(#missing) url(#a)'/></filter></svg>", 128).is_err());
    }
}
