//! Bounded, explicit layer references. Native documents and private metadata never leave here.
use crate::{EngineError, Result};
use photocraft_doc::{Document, Layer, LayerContent, LayerPath};
use photocraft_gen::{LayerContext, LayerReference, MAX_LAYER_IMAGES, MAX_LAYER_MANIFEST_BYTES};
use serde_json::json;
use std::path::Path;

const MAX_DEPTH: usize = 32;
const MAX_TREE_NODES: usize = 4096;
const MAX_CONTEXT_NODES: usize = 128;
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct Plan {
    manifest: String,
    images: Vec<(String, LayerPath)>,
}

fn limit(message: &str) -> EngineError {
    EngineError::Other(format!("{message}. Reduce the document or turn off individual layer references to send only the visible canvas."))
}

/// Bound the entire tree before cloning it or entering the recursive compositor.
pub(super) fn validate_tree(layers: &[Layer]) -> Result<()> {
    fn walk(layers: &[Layer], depth: usize, count: &mut usize) -> Result<()> {
        if !layers.is_empty() && depth > MAX_DEPTH {
            return Err(EngineError::Other("AI input supports layer groups up to 32 levels deep".into()));
        }
        for layer in layers {
            *count = count.saturating_add(1);
            if *count > MAX_TREE_NODES {
                return Err(EngineError::Other("AI input supports documents with up to 4096 layer entries".into()));
            }
            if let Some(children) = layer.children() {
                walk(children, depth + 1, count)?;
            }
        }
        Ok(())
    }
    walk(layers, 1, &mut 0)
}

/// Only included layer names/structure enter the manifest. Hidden ancestors hide their subtree.
pub(super) fn prepare(doc: &Document, active: Option<photocraft_doc::LayerId>, include_hidden: bool) -> Result<Plan> {
    validate_tree(&doc.layers)?;
    let mut entries = Vec::new();
    let mut images = Vec::new();
    let mut stack: Vec<_> = doc.layers.iter().enumerate().map(|(i, layer)| (layer, vec![i], None::<String>, true)).collect();
    // Pop the visually topmost entry first.
    while let Some((layer, path, parent, parent_visible)) = stack.pop() {
        let visible = parent_visible && layer.visible;
        if !include_hidden && !visible {
            continue;
        }
        if entries.len() >= MAX_CONTEXT_NODES || layer.name.len() > 1024 {
            return Err(limit("Layer names or structure exceed the reference budget (128 entries, 1024 bytes per name)"));
        }
        let reference = format!("layer-{:02}", entries.len() + 1);
        let drawable = !matches!(layer.content, LayerContent::Group(_) | LayerContent::Adjustment(_));
        if drawable {
            if images.len() >= MAX_LAYER_IMAGES {
                return Err(limit("Individual layer references support up to 15 images per request"));
            }
            images.push((reference.clone(), path.clone()));
        }
        entries.push(json!({
            "ref": reference, "id": layer.id.0.to_string(), "name": layer.name,
            "parent": parent, "orderTopToBottom": entries.len() + 1,
            "kind": layer.content.kind_name(), "visible": layer.visible, "effectiveVisible": visible,
            "active": active == Some(layer.id), "opacity": layer.opacity,
            "fillOpacity": layer.fill_opacity, "blendMode": layer.blend,
            "clipped": layer.clipped, "hasMask": layer.mask.is_some(), "hasVectorMask": layer.vector_mask.is_some(),
            "image": drawable.then(|| format!("{reference}.png")),
        }));
        if let Some(children) = layer.children() {
            for (index, child) in children.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(index);
                stack.push((child, child_path, Some(reference.clone()), visible));
            }
        }
    }
    let manifest = serde_json::to_string(&json!({
        "version": 1, "canvas": {"width":doc.size.width,"height":doc.size.height},
        "includesHiddenLayers":include_hidden, "layers":entries,
        "previewSemantics":"Full-canvas isolated leaf previews with masks and effects; normal blend, unclipped. Group effects, clipping, adjustments and backdrop blending require the canvas reference. Groups and adjustments have metadata only."
    })).map_err(|e| EngineError::Other(e.to_string()))?;
    if manifest.len() > MAX_LAYER_MANIFEST_BYTES {
        return Err(limit("Layer metadata exceeds 64 KiB"));
    }
    Ok(Plan { manifest, images })
}

/// Flatten into a fresh export document to strip file names, EXIF, XMP and opaque PSD data.
pub(super) fn png_bytes(source: &Document, intent: photocraft_cms::Intent, bpc: bool) -> Result<Vec<u8>> {
    let mut input = Document::new("Canvas", source.size, source.mode, source.depth);
    input.icc_profile = source.icc_profile.clone();
    let surface = photocraft_compose::flatten_to_surface(source, source.pixel_format(), None);
    input.layers.push(Layer::new("Canvas", LayerContent::Raster(surface)));
    crate::color_cmds::convert_document(&mut input, photocraft_cms::builtin::srgb(), intent, bpc)?;
    crate::image_cmds::for_each_surface(&mut input.layers, true, &mut |surface, _| {
        *surface = surface.convert(surface.format().with_sample(photocraft_color::SampleType::U8));
    });
    input.depth = photocraft_color::SampleType::U8;
    let opts = photocraft_io::ExportOptions { xmp: photocraft_io::XmpEmbed::None, ..Default::default() };
    let png = photocraft_io::export(&input, "png", &opts).map_err(|e| EngineError::Other(e.to_string()))?;
    if png.bytes.len() > MAX_INPUT_BYTES {
        return Err(limit("AI input images exceed 64 MiB"));
    }
    Ok(png.bytes)
}

/// Render references one at a time so peak memory does not scale with layer count.
pub(super) fn export(
    doc: &Document,
    plan: Plan,
    workspace: &Path,
    canvas_bytes: usize,
    intent: photocraft_cms::Intent,
    bpc: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<LayerContext> {
    let mut images = Vec::new();
    let mut total = canvas_bytes;
    // Keep the source colour context, patterns and global light, but never export its metadata.
    let mut isolated = Document::new("Layer", doc.size, doc.mode, doc.depth);
    isolated.icc_profile = doc.icc_profile.clone();
    isolated.patterns = doc.patterns.clone();
    isolated.global_light = doc.global_light;
    for (label, path) in plan.images {
        if cancelled() {
            return Err(EngineError::Cancelled);
        }
        let mut layer = doc.layer_at(&path).ok_or_else(|| EngineError::Other("Layer reference is no longer available".into()))?.clone();
        layer.visible = true;
        layer.clipped = false;
        layer.blend = photocraft_color::BlendMode::Normal;
        isolated.layers.clear();
        isolated.layers.push(layer);
        let bytes = png_bytes(&isolated, intent, bpc)?;
        total = total.saturating_add(bytes.len());
        if total > MAX_INPUT_BYTES {
            return Err(limit("AI input images exceed 64 MiB in total"));
        }
        if cancelled() {
            return Err(EngineError::Cancelled);
        }
        let image = workspace.join(format!("{label}.png"));
        std::fs::write(&image, bytes).map_err(|e| EngineError::Other(e.to_string()))?;
        images.push(LayerReference { label, image });
    }
    Ok(LayerContext { manifest: plan.manifest, images })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::Value;

    fn document(mode: &str, depth: u32, color: &str) -> Document {
        let mut s = Session::new();
        s.execute("file.new", json!({"width":8,"height":6,"mode":mode,"depth":depth,"background":color})).unwrap();
        s.active().unwrap().doc.as_ref().clone()
    }

    #[test]
    fn hidden_subtrees_are_absent_and_top_to_bottom_ids_are_unambiguous() {
        let mut doc = document("rgb", 8, "#ff0000");
        doc.layers[0].name = "Same name".into();
        let mut hidden = doc.layers[0].clone();
        hidden.name = "PRIVATE hidden child".into();
        let mut group = Layer::group("PRIVATE group", vec![hidden]);
        group.visible = false;
        doc.layers.push(group);
        let top = Layer::new("Same name", LayerContent::Raster(doc.layers[0].surface().unwrap().clone()));
        let id = top.id;
        doc.layers.push(top);
        let plan = prepare(&doc, Some(id), false).unwrap();
        assert!(!plan.manifest.contains("PRIVATE"));
        let manifest: Value = serde_json::from_str(&plan.manifest).unwrap();
        assert_eq!(manifest["layers"].as_array().unwrap().len(), 2);
        assert_eq!(manifest["layers"][0]["id"], id.0.to_string());
        assert_eq!(manifest["layers"][0]["active"], true);
        assert_eq!(manifest["layers"][0]["ref"], "layer-01");
        assert_eq!(manifest["layers"][1]["ref"], "layer-02");
        let plan = prepare(&doc, None, true).unwrap();
        let manifest: Value = serde_json::from_str(&plan.manifest).unwrap();
        assert_eq!(manifest["layers"].as_array().unwrap().len(), 4);
        assert_eq!(manifest["layers"][2]["parent"], "layer-02");
        assert_eq!(manifest["layers"][2]["effectiveVisible"], false);
        assert_eq!(plan.images.len(), 3);
    }

    #[test]
    fn visible_nested_children_keep_their_group_and_stack_order() {
        let mut doc = document("rgb", 8, "#ff0000");
        let bottom = doc.layers.remove(0);
        let top = Layer::new("upper", LayerContent::Raster(bottom.surface().unwrap().clone()));
        doc.layers.push(Layer::group("group", vec![bottom, top]));
        let plan = prepare(&doc, None, false).unwrap();
        let manifest: Value = serde_json::from_str(&plan.manifest).unwrap();
        assert_eq!(manifest["layers"][0]["kind"], "Group");
        assert!(manifest["layers"][0]["image"].is_null());
        assert_eq!(manifest["layers"][1]["name"], "upper");
        assert_eq!(manifest["layers"][1]["parent"], "layer-01");
        assert_eq!(plan.images[0].1, vec![0, 1]);
    }

    #[test]
    fn isolated_pngs_have_original_coordinates_and_do_not_include_other_layers() {
        let mut doc = document("rgb", 16, "#ff0000");
        doc.name = "PRIVATE document filename.psd".into();
        let mut green = document("rgb", 16, "#00ff00").layers.remove(0);
        green.name = "../../PRIVATE layer.png".into();
        doc.layers.push(green);
        let work = tempfile::tempdir().unwrap();
        let plan = prepare(&doc, None, false).unwrap();
        let context = export(&doc, plan, work.path(), 0, Default::default(), true, &|| false).unwrap();
        assert_eq!(context.images.len(), 2);
        assert!(!context.manifest.contains("PRIVATE document"));
        for (index, expected) in [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0]].iter().enumerate() {
            let image = &context.images[index].image;
            assert_eq!(image.parent(), Some(work.path()));
            assert_eq!(image.file_name().unwrap().to_str().unwrap(), format!("layer-{:02}.png", index + 1));
            let bytes = std::fs::read(image).unwrap();
            assert!(!bytes.windows(7).any(|w| w == b"PRIVATE"));
            let result = photocraft_io::import("layer.png", &bytes).unwrap().document;
            assert_eq!(result.size, doc.size);
            assert_eq!(result.depth, photocraft_color::SampleType::U8);
            let pixel = result.layers[0].surface().unwrap().read_region(photocraft_geom::Rect::new(0, 0, 1, 1));
            for (actual, expected) in pixel.iter().zip(expected) {
                assert!((actual - expected).abs() < 0.02);
            }
        }
    }

    #[test]
    fn hidden_images_are_written_only_with_explicit_opt_in() {
        let mut doc = document("rgb", 8, "#ff0000");
        doc.layers[0].visible = false;
        for include in [false, true] {
            let work = tempfile::tempdir().unwrap();
            let plan = prepare(&doc, None, include).unwrap();
            let context = export(&doc, plan, work.path(), 0, Default::default(), true, &|| false).unwrap();
            assert_eq!(context.images.len(), usize::from(include));
            assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), usize::from(include));
            if include {
                let bytes = std::fs::read(&context.images[0].image).unwrap();
                let image = photocraft_io::import("layer.png", &bytes).unwrap().document;
                assert!(image.layers[0].surface().unwrap().read_region(photocraft_geom::Rect::new(0, 0, 1, 1))[0] > 0.95);
            }
        }
        assert!(!doc.layers[0].visible);
    }

    #[test]
    fn layer_exports_manage_color_and_depth_without_mutating_the_source() {
        for (mode, depth) in [("rgb", 8), ("rgb", 16), ("rgb", 32), ("gray", 16), ("gray", 32), ("cmyk", 16), ("lab", 16)] {
            let doc = document(mode, depth, "#ff0000");
            let before = doc.clone();
            let work = tempfile::tempdir().unwrap();
            let context = export(&doc, prepare(&doc, None, false).unwrap(), work.path(), 0, Default::default(), true, &|| false).unwrap();
            let image = photocraft_io::import("layer.png", &std::fs::read(&context.images[0].image).unwrap()).unwrap().document;
            assert_eq!(image.mode, photocraft_color::ColorMode::Rgb);
            assert_eq!(image.depth, photocraft_color::SampleType::U8);
            assert_eq!(doc, before);
        }
    }

    #[test]
    fn oversized_context_and_cancellation_fail_before_provider_access() {
        let mut doc = document("rgb", 8, "#ff0000");
        let layer = doc.layers[0].clone();
        doc.layers = vec![layer.clone(); MAX_LAYER_IMAGES + 1];
        assert!(prepare(&doc, None, false).is_err());
        doc.layers = vec![layer.clone()];
        doc.layers[0].name = "x".repeat(1025);
        assert!(prepare(&doc, None, false).is_err());
        doc.layers = vec![layer.clone()];
        let work = tempfile::tempdir().unwrap();
        assert!(matches!(export(&doc, prepare(&doc, None, false).unwrap(), work.path(), 0, Default::default(), true, &|| true), Err(EngineError::Cancelled)));
        assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0);
        assert!(export(&doc, prepare(&doc, None, false).unwrap(), work.path(), MAX_INPUT_BYTES, Default::default(), true, &|| false).is_err());
        assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0);
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            let count = calls.get() + 1;
            calls.set(count);
            count > 1
        };
        assert!(matches!(export(&doc, prepare(&doc, None, false).unwrap(), work.path(), 0, Default::default(), true, &cancelled), Err(EngineError::Cancelled)));
        assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0);
        let mut group = Layer::group("x".repeat(1024), vec![]);
        doc.layers = vec![group.clone(); MAX_CONTEXT_NODES];
        assert!(prepare(&doc, None, false).is_err()); // bounded serialized manifest
        group.name.clear();
        doc.layers = vec![group; MAX_CONTEXT_NODES + 1];
        assert!(prepare(&doc, None, false).is_err()); // bounded entry count
        doc.layers = vec![layer.clone()];
        for _ in 0..MAX_DEPTH {
            doc.layers = vec![Layer::group("nested", std::mem::take(&mut doc.layers))];
        }
        assert!(validate_tree(&doc.layers).is_err());
        doc.layers = vec![layer; MAX_TREE_NODES + 1];
        assert!(validate_tree(&doc.layers).is_err());
    }
}
