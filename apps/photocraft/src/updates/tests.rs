use super::*;
use serde_json::json;

fn release(version: &str, kind: Kind) -> Value {
    let suffix = match kind {
        Kind::Msi => "windows-x64.msi",
        Kind::Portable => "windows-x64-portable.zip",
    };
    let name = format!("photocraft-ai-{version}-{suffix}");
    json!({"tag_name":format!("v{version}"),"draft":false,"prerelease":version.contains('-'),"published_at":"2026-11-02T00:00:00Z",
        "assets":[{"name":name,"state":"uploaded","size":1234,"digest":format!("sha256:{}","a".repeat(64)),"browser_download_url":format!("https://github.com/{REPO}/releases/download/v{version}/{name}")}]})
}
fn choose(items: Vec<Value>, current: &str, kind: Kind) -> Option<Update> {
    select(&serde_json::to_string(&items).unwrap(), current, kind).unwrap()
}
#[test]
fn only_newer_published_ai_packages_for_this_installation_are_offered() {
    let beta = release("0.18.0-ai-beta.2", Kind::Msi);
    let u = choose(vec![beta.clone()], "0.18.0-ai-beta.1", Kind::Msi).unwrap();
    assert_eq!(u.version, "0.18.0-ai-beta.2");
    assert!(u.page.starts_with(&format!("https://github.com/{REPO}/releases/tag/")));
    for current in ["0.18.0-ai-beta.2", "0.18.0-ai-beta.3", "0.18.0", "0.18.0-integration.abc"] {
        assert!(choose(vec![beta.clone()], current, Kind::Msi).is_none());
    }
    assert!(choose(vec![beta.clone()], "0.18.0-ai-beta.1", Kind::Portable).is_none());
    let portable = release("0.18.0-ai-beta.2", Kind::Portable);
    assert!(choose(vec![portable], "0.18.0-ai-beta.1", Kind::Portable).is_some());
    assert!(choose(vec![release("0.19.0", Kind::Msi)], "0.18.0-ai-beta.1", Kind::Msi).is_some());
}
#[test]
fn beta_patch_suffixes_are_ordered_numerically() {
    for kind in [Kind::Msi, Kind::Portable] {
        let first_patch = "0.20.1-ai-beta.3.1.1";
        let later_patch = "0.20.1-ai-beta.3.1.10";
        for current in ["0.19.0-ai-beta.2", "0.20.1-ai-beta.3"] {
            assert_eq!(choose(vec![release(first_patch, kind)], current, kind).unwrap().version, first_patch);
        }
        assert_eq!(choose(vec![release(later_patch, kind), release(first_patch, kind)], first_patch, kind).unwrap().version, later_patch);
        assert!(choose(vec![release(first_patch, kind)], later_patch, kind).is_none());
    }
}

#[test]
fn drafts_tags_upstream_releases_and_incomplete_uploads_stay_quiet() {
    let base = release("0.19.0-ai-beta.1", Kind::Msi);
    for (key, value) in [("draft", json!(true)), ("published_at", Value::Null), ("assets", json!([])), ("tag_name", json!("v0.20.0/../evil"))] {
        let mut r = base.clone();
        r[key] = value;
        assert!(choose(vec![r], "0.18.0-ai-beta.1", Kind::Msi).is_none(), "{key}");
    }
    for (key, value) in [
        ("state", json!("new")),
        ("size", json!(0)),
        ("size", json!(MAX_PACKAGE + 1)),
        ("digest", Value::Null),
        ("digest", json!("sha256:bad")),
        ("browser_download_url", json!("https://github.com/storytold/photocraft/releases/download/v0.19.0/photocraft.msi")),
        ("name", json!("photocraft-0.19.0-windows-x64.msi")),
    ] {
        let mut r = base.clone();
        r["assets"][0][key] = value;
        assert!(choose(vec![r], "0.18.0-ai-beta.1", Kind::Msi).is_none(), "{key}");
    }
}
#[test]
fn latest_complete_package_wins_even_if_newest_release_has_no_installer_yet() {
    let mut newest = release("0.20.0-ai-beta.1", Kind::Msi);
    newest["assets"] = json!([]);
    assert_eq!(choose(vec![newest, release("0.19.0-ai-beta.1", Kind::Msi)], "0.18.0-ai-beta.1", Kind::Msi).unwrap().version, "0.19.0-ai-beta.1");
    for text in ["null", "{}", "not json"] {
        assert!(select(text, "0.18.0-ai-beta.1", Kind::Msi).is_err());
    }
}
