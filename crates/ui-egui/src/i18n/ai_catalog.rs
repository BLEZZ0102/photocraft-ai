//! PhotoCraft AI's own interface translations, layered cleanly over upstream `craft-i18n`.
//!
//! Keeping AI-specific keys in standalone per-locale TSV files under `src/i18n/ai/` prevents merge
//! conflicts with `storytold/craft-i18n` while preserving full localization for every supported
//! language.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Embedded AI overlay catalogs for every non-English locale in [`super::LANGUAGES`].
pub(crate) const AI_CATALOGS: &[(&str, &str, &str)] = &[
    ("ar", "", include_str!("ai/ar.tsv")),
    ("az", "", include_str!("ai/az.tsv")),
    ("bs", "", include_str!("ai/bs.tsv")),
    ("cs", "", include_str!("ai/cs.tsv")),
    ("da", "", include_str!("ai/da.tsv")),
    ("de", "", include_str!("ai/de.tsv")),
    ("el", "", include_str!("ai/el.tsv")),
    ("es", "", include_str!("ai/es.tsv")),
    ("fi", "", include_str!("ai/fi.tsv")),
    ("fr", "", include_str!("ai/fr.tsv")),
    ("he", "", include_str!("ai/he.tsv")),
    ("hi", "", include_str!("ai/hi.tsv")),
    ("hu", "", include_str!("ai/hu.tsv")),
    ("id", "", include_str!("ai/id.tsv")),
    ("it", "", include_str!("ai/it.tsv")),
    ("ja", "", include_str!("ai/ja.tsv")),
    ("ka", "", include_str!("ai/ka.tsv")),
    ("ko", "", include_str!("ai/ko.tsv")),
    ("nb", "", include_str!("ai/nb.tsv")),
    ("nl", "", include_str!("ai/nl.tsv")),
    ("pl", "", include_str!("ai/pl.tsv")),
    ("pt-br", "", include_str!("ai/pt-br.tsv")),
    ("ro", "", include_str!("ai/ro.tsv")),
    ("ru", "", include_str!("ai/ru.tsv")),
    ("sr-latn", "", include_str!("ai/sr-latn.tsv")),
    ("sv", "", include_str!("ai/sv.tsv")),
    ("ta", "", include_str!("ai/ta.tsv")),
    ("th", "", include_str!("ai/th.tsv")),
    ("tr", "", include_str!("ai/tr.tsv")),
    ("uk", "", include_str!("ai/uk.tsv")),
    ("ur", "", include_str!("ai/ur.tsv")),
    ("vi", "", include_str!("ai/vi.tsv")),
    ("zh-hans", "", include_str!("ai/zh-hans.tsv")),
    ("zh-hant", "", include_str!("ai/zh-hant.tsv")),
];

type LocaleRows = HashMap<String, HashMap<String, String>>;

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

static TABLES: LazyLock<HashMap<&'static str, LocaleRows>> = LazyLock::new(|| {
    let mut all = HashMap::with_capacity(AI_CATALOGS.len());
    for &(code, _, text) in AI_CATALOGS {
        let mut by_ctx: LocaleRows = HashMap::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let (Some(ctx), Some(src), Some(val)) = (cols.next(), cols.next(), cols.next()) else {
                continue;
            };
            by_ctx.entry(unescape(ctx)).or_default().insert(unescape(src), unescape(val));
        }
        all.insert(code, by_ctx);
    }
    all
});

/// Look up an AI-specific translation for `code`, `context`, and `source`.
pub(crate) fn get(code: &str, context: &str, source: &str) -> Option<&'static str> {
    if code.eq_ignore_ascii_case("en") {
        return None;
    }
    let tables = LazyLock::force(&TABLES);
    let rows = tables.get(code).or_else(|| tables.iter().find(|(k, _)| k.eq_ignore_ascii_case(code)).map(|(_, v)| v))?;
    rows.get(context)?.get(source).map(String::as_str)
}

#[cfg(test)]
pub(crate) fn test_lang(code: &str) -> Option<super::Lang> {
    static TEST_I18N: LazyLock<craft_i18n::I18n> = LazyLock::new(|| {
        let offered: Vec<&str> = super::LANGUAGES.iter().map(|l| l.code).collect();
        craft_i18n::I18n::new(AI_CATALOGS, &offered).0
    });
    let i18n = LazyLock::force(&TEST_I18N);
    let lang = i18n.from_code(code)?;
    Some(super::Lang { lang, catalogs: Some(i18n) })
}

#[cfg(test)]
pub(crate) fn raw(code: &str) -> Option<&'static str> {
    AI_CATALOGS.iter().find(|(c, ..)| c.eq_ignore_ascii_case(code)).map(|(_, _, text)| *text)
}
