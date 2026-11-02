//! The AI fork has its own feed. A release is offered only with a complete, hashed package.
use craft_update::{Asset, Http};
use photocraft_engine::updates_cmds::{CheckFn, CheckResult, PrepareFn, Release};
use serde_json::Value;

pub const REPO: &str = "BLEZZ0102/photocraft-ai";
const API: &str = "https://api.github.com/repos/BLEZZ0102/photocraft-ai/releases?per_page=100";
const MAX_PACKAGE: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Msi,
    Portable,
}

#[derive(Clone, Debug)]
struct Update {
    version: String,
    asset: Asset,
    page: String,
}

pub fn service() -> CheckFn {
    Box::new(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("AI update check".into())
            .spawn(move || {
                let result = check();
                if let Err(e) = &result {
                    log::warn!("AI update check: {e}");
                }
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Could not start update check: {e}"))?;
        Ok(rx)
    })
}

fn check() -> CheckResult {
    let Some(kind) = installed_kind() else { return Ok(None) };
    let http = Http::new("Photocraft-dev")?;
    Ok(fetch(&http, photocraft_engine::build_info::VERSION, kind)?.map(|u| Release { version: u.version, url: u.page }))
}

fn fetch(http: &Http, current: &str, kind: Kind) -> Result<Option<Update>, String> {
    match http.get_api(API, None)? {
        craft_update::http::Fetched::Body { text, .. } => select(&text, current, kind),
        craft_update::http::Fetched::NotFound => Ok(None),
        _ => Err("Could not read AI releases; try again later".into()),
    }
}

/// Strict names and URLs bind the package to this release and fork. An asset without its final
/// GitHub digest/upload state is invisible, including while a maintainer is uploading it.
fn select(text: &str, current: &str, kind: Kind) -> Result<Option<Update>, String> {
    let current = craft_update::Version::parse(current).map_err(|e| format!("Invalid installed version: {e}"))?;
    let list: Vec<Value> = serde_json::from_str(text).map_err(|e| format!("Invalid AI release feed: {e}"))?;
    let mut best: Option<(craft_update::Version, Update)> = None;
    for item in list.into_iter().take(100) {
        if item.get("draft").and_then(Value::as_bool) != Some(false) || item.get("published_at").and_then(Value::as_str).is_none() {
            continue;
        }
        let Some(tag) = item.get("tag_name").and_then(Value::as_str) else { continue };
        let Some(raw) = tag.strip_prefix('v') else { continue };
        if raw.len() > 64 {
            continue;
        }
        let Ok(version) = craft_update::Version::parse(raw) else { continue };
        // Beta users receive AI betas and stable AI packages. Stable users stay on stable.
        if (!version.pre.is_empty() && (!version.pre.as_str().starts_with("ai-beta.") || current.pre.is_empty()))
            || (!current.pre.is_empty() && !current.pre.as_str().starts_with("ai-beta."))
            || !version.cmp_precedence(&current).is_gt()
        {
            continue;
        }
        if version.pre.is_empty() && item.get("prerelease").and_then(Value::as_bool) != Some(false) {
            continue;
        }
        let suffix = match kind {
            Kind::Msi => "windows-x64.msi",
            Kind::Portable => "windows-x64-portable.zip",
        };
        let name = format!("photocraft-ai-{raw}-{suffix}");
        let url = format!("https://github.com/{REPO}/releases/download/{tag}/{name}");
        let Some(assets) = item.get("assets").and_then(Value::as_array) else { continue };
        let Some(a) =
            assets.iter().find(|a| a.get("name").and_then(Value::as_str) == Some(name.as_str()) && a.get("state").and_then(Value::as_str) == Some("uploaded"))
        else {
            continue;
        };
        if a.get("browser_download_url").and_then(Value::as_str) != Some(url.as_str()) {
            continue;
        }
        let Some(size) = a.get("size").and_then(Value::as_u64).filter(|s| (1..=MAX_PACKAGE).contains(s)) else { continue };
        let Some(hash) = a
            .get("digest")
            .and_then(Value::as_str)
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            continue;
        };
        let update = Update {
            version: raw.into(),
            asset: Asset { name, size, url, sha256: Some(hash.to_ascii_lowercase()) },
            page: format!("https://github.com/{REPO}/releases/tag/{tag}"),
        };
        if best.as_ref().is_none_or(|(v, _)| version.cmp_precedence(v).is_gt()) {
            best = Some((version, update));
        }
    }
    Ok(best.map(|(_, u)| u))
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;

fn installed_kind() -> Option<Kind> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::installed_kind().ok()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        None
    }
}

pub fn prepare_service() -> Option<PrepareFn> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        installed_kind().map(|_| windows::service())
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        None
    }
}

pub fn finish() -> Option<Result<(), String>> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::finish()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        None
    }
}

pub fn take_failure() -> Option<String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::take_failure()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        None
    }
}

#[cfg(test)]
mod tests;
