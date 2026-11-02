//! Release-check state and commands. The desktop injects a non-blocking transport; the engine
//! owns the result so CLI, MCP and the GUI query the same state without network dependencies.

use std::sync::mpsc::{Receiver, TryRecvError};

use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::{CommandSpec, always};
use crate::{EngineError, Result, Session};

pub const RELEASES: &str = "https://github.com/BLEZZ0102/photocraft-ai/releases";
pub type CheckResult = std::result::Result<Option<Release>, String>;
/// Starts background work and returns immediately. No document or personal data is sent.
pub type CheckFn = Box<dyn FnMut() -> std::result::Result<Receiver<CheckResult>, String> + Send>;
/// Download/verify first; the returned action schedules installation only after a second save guard.
pub type ReadyFn = Box<dyn FnOnce() -> std::result::Result<(), String> + Send>;
pub type PrepareResult = std::result::Result<ReadyFn, String>;
pub type PrepareFn = Box<dyn FnMut(&str) -> std::result::Result<Receiver<PrepareResult>, String> + Send>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum InstallStatus {
    #[default]
    Idle,
    Downloading,
    Ready,
    Restarting,
    Failed {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        release: Release,
    },
    Failed {
        message: String,
    },
}

#[derive(Default)]
pub struct Updates {
    pub check: Option<CheckFn>,
    pub prepare: Option<PrepareFn>,
    pub install_status: InstallStatus,
    pub status: Status,
    pending: Option<Receiver<CheckResult>>,
    preparing: Option<Receiver<PrepareResult>>,
    ready: Option<ReadyFn>,
}

impl Updates {
    pub fn poll_install(&mut self) {
        let Some(rx) = &self.preparing else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("The update download stopped before returning a result".into()),
        };
        self.preparing = None;
        self.install_status = match result {
            Ok(ready) => {
                self.ready = Some(ready);
                InstallStatus::Ready
            }
            Err(message) => InstallStatus::Failed { message },
        };
    }

    /// Consume a completed check; a disconnected worker becomes an actionable error.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else { return false };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err("The update check stopped before returning a result".into()),
        };
        self.pending = None;
        self.status = match result {
            Ok(Some(release)) => Status::Available { release },
            Ok(None) => Status::UpToDate,
            Err(message) => Status::Failed { message },
        };
        true
    }
}

/// Interpret GitHub's latest-release response. Never trust a remote URL: construct the release
/// page from a validated tag in our own repository. Build metadata doesn't affect precedence.
pub fn parse_release(bytes: &[u8], current: &str) -> CheckResult {
    #[derive(Deserialize)]
    struct GithubRelease {
        tag_name: String,
        draft: bool,
        prerelease: bool,
    }
    let release: GithubRelease = serde_json::from_slice(bytes).map_err(|e| format!("Invalid release response: {e}"))?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let tag = &release.tag_name;
    if tag.len() > 128 || !tag.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+')) {
        return Err("Invalid release tag".into());
    }
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|e| format!("Invalid release version: {e}"))?;
    let current = Version::parse(current).map_err(|e| format!("Invalid installed version: {e}"))?;
    if !version.pre.is_empty() || !version.cmp_precedence(&current).is_gt() {
        return Ok(None);
    }
    Ok(Some(Release { version: version.to_string(), url: format!("{RELEASES}/tag/{tag}") }))
}

fn empty_params(p: &Value) -> Result<()> {
    if p.as_object().is_some_and(|p| p.is_empty()) { Ok(()) } else { Err(EngineError::Other("update commands take an empty object".into())) }
}

fn enabled(s: &Session) -> std::result::Result<(), String> {
    s.updates.check.as_ref().map(|_| ()).ok_or_else(|| "Update checks are unavailable in this frontend".into())
}

fn check(s: &mut Session, p: &Value) -> Result<Value> {
    empty_params(p)?;
    if matches!(s.updates.install_status, InstallStatus::Downloading | InstallStatus::Ready | InstallStatus::Restarting) {
        return Err(EngineError::Other("Finish the current update before checking again".into()));
    }
    // Repeated automation requests must not spawn an unbounded number of network workers.
    if s.updates.pending.is_none() {
        let start = s.updates.check.as_mut().ok_or_else(|| EngineError::Other("Update checks are unavailable in this frontend".into()))?;
        match start() {
            Ok(rx) => {
                s.updates.pending = Some(rx);
                s.updates.status = Status::Checking;
            }
            Err(message) => {
                s.updates.status = Status::Failed { message: message.clone() };
                return Err(EngineError::Other(message));
            }
        }
    }
    Ok(json!(s.updates.status))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "updates.install",
            label: "Update and Restart",
            menu: &[],
            shortcut: None,
            params: "{}",
            journal: false,
            enabled: |s| {
                if s.updates.prepare.is_none() {
                    return Err("One-click updates are unavailable in this frontend".into());
                }
                if !matches!(s.updates.status, Status::Available { .. }) {
                    return Err("No AI update is available".into());
                }
                if matches!(s.updates.install_status, InstallStatus::Downloading | InstallStatus::Ready | InstallStatus::Restarting) {
                    return Err("An update is already in progress".into());
                }
                ensure_saved(s).map_err(|e| e.to_string())
            },
            run: |s, p| {
                empty_params(p)?;
                ensure_saved(s)?;
                if matches!(s.updates.install_status, InstallStatus::Downloading | InstallStatus::Ready | InstallStatus::Restarting) {
                    return Err(EngineError::Other("An update is already in progress".into()));
                }
                let Status::Available { release } = &s.updates.status else { return Err(EngineError::Other("No AI update is available".into())) };
                let start = s.updates.prepare.as_mut().ok_or_else(|| EngineError::Other("One-click updates are unavailable in this frontend".into()))?;
                match start(&release.version) {
                    Ok(rx) => {
                        s.updates.preparing = Some(rx);
                        s.updates.install_status = InstallStatus::Downloading;
                    }
                    Err(message) => {
                        s.updates.install_status = InstallStatus::Failed { message: message.clone() };
                        return Err(EngineError::Other(message));
                    }
                }
                Ok(json!(s.updates.install_status))
            },
        },
        CommandSpec {
            id: "updates.restart",
            label: "Update and Restart",
            menu: &[],
            shortcut: None,
            params: "{}",
            journal: false,
            enabled: |s| {
                if !matches!(s.updates.install_status, InstallStatus::Ready) {
                    return Err("No verified update is ready".into());
                }
                ensure_saved(s).map_err(|e| e.to_string())
            },
            run: |s, p| {
                empty_params(p)?;
                ensure_saved(s)?;
                let ready = s.updates.ready.take().ok_or_else(|| EngineError::Other("No verified update is ready".into()))?;
                match ready() {
                    Ok(()) => s.updates.install_status = InstallStatus::Restarting,
                    Err(message) => {
                        s.updates.install_status = InstallStatus::Failed { message: message.clone() };
                        return Err(EngineError::Other(message));
                    }
                }
                Ok(json!(s.updates.install_status))
            },
        },
        CommandSpec {
            id: "help.checkForUpdates",
            label: "Check for Updates…",
            menu: &["Help"],
            shortcut: None,
            params: "{}",
            enabled,
            run: check,
            journal: false,
        },
        CommandSpec {
            id: "updates.status",
            label: "Update Check Status",
            menu: &[],
            shortcut: None,
            params: "{}",
            enabled: always,
            run: |s, p| {
                empty_params(p)?;
                s.updates.poll();
                s.updates.poll_install();
                Ok(
                    json!({"supported": s.updates.check.is_some(), "oneClickSupported": s.updates.prepare.is_some(), "currentVersion": crate::build_info::VERSION, "result": s.updates.status, "installation": s.updates.install_status}),
                )
            },
            journal: false,
        },
    ]
}

fn ensure_saved(s: &Session) -> Result<()> {
    if s.documents().iter().any(|d| d.is_dirty()) {
        return Err(EngineError::Other("Save your documents before updating".into()));
    }
    if !s.jobs().is_empty() {
        return Err(EngineError::Other("Wait for background tasks before updating".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_explicit_deduplicated_and_rechecks_unsaved_work_before_restart() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut s = Session::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let scheduled = calls.clone();
        s.updates.status = Status::Available { release: Release { version: "0.19.0-ai-beta.1".into(), url: RELEASES.into() } };
        s.updates.prepare = Some(Box::new(move |version| {
            assert_eq!(version, "0.19.0-ai-beta.1");
            let (tx, rx) = std::sync::mpsc::channel();
            let count = scheduled.clone();
            tx.send(Ok(Box::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }) as ReadyFn))
                .unwrap();
            Ok(rx)
        }));
        for p in [Value::Null, json!([]), json!({"url":"evil"})] {
            assert!(s.execute("updates.install", p.clone()).is_err());
            assert!(s.execute("updates.restart", p).is_err());
        }
        s.execute("updates.install", json!({})).unwrap();
        assert!(s.execute("updates.install", json!({})).is_err());
        assert_eq!(s.execute("updates.status", json!({})).unwrap()["installation"]["status"], "ready");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "download completion never installs by itself");
        s.execute("file.new", json!({"width": 8, "height": 8})).unwrap();
        s.execute("layer.new.layer", json!({})).unwrap();
        assert!(s.execute("updates.restart", json!({})).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        s.execute("file.close", json!({})).unwrap();
        s.execute("updates.restart", json!({})).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(s.execute("updates.restart", json!({})).is_err());
    }

    #[test]
    fn failed_or_disconnected_downloads_do_not_schedule_installation() {
        for disconnect in [false, true] {
            let mut s = Session::new();
            s.updates.status = Status::Available { release: Release { version: "0.19.0-ai-beta.1".into(), url: RELEASES.into() } };
            s.updates.prepare = Some(Box::new(move |_| {
                let (tx, rx) = std::sync::mpsc::channel();
                if !disconnect {
                    tx.send(Err("checksum mismatch".into())).unwrap();
                }
                Ok(rx)
            }));
            s.execute("updates.install", json!({})).unwrap();
            s.execute("updates.status", json!({})).unwrap();
            assert!(matches!(s.updates.install_status, InstallStatus::Failed { .. }));
            assert!(s.execute("updates.restart", json!({})).is_err());
        }
    }

    fn response(tag: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({"tag_name": tag, "draft": false, "prerelease": false, "html_url": "https://untrusted.example"})).unwrap()
    }

    #[test]
    fn semantic_versions_and_trusted_links() {
        let release = parse_release(&response("v0.10.0"), "0.9.0").unwrap().unwrap();
        assert_eq!(release.version, "0.10.0");
        assert_eq!(release.url, format!("{RELEASES}/tag/v0.10.0"));
        for tag in ["v0.9.0", "v0.8.9", "v0.9.0+build.2", "v1.0.0-rc.1"] {
            assert_eq!(parse_release(&response(tag), "0.9.0").unwrap(), None, "{tag}");
        }
        assert!(parse_release(&response("0.9.0"), "0.9.0-rc.1").unwrap().is_some());
    }

    #[test]
    fn unpublished_and_hostile_responses() {
        for flag in ["draft", "prerelease"] {
            let mut v: Value = serde_json::from_slice(&response("v1.0.0")).unwrap();
            v[flag] = json!(true);
            assert_eq!(parse_release(&serde_json::to_vec(&v).unwrap(), "0.7.0").unwrap(), None);
        }
        for tag in ["", "latest", "v1.0.0/../evil", "v1.0.0?next=evil", "v1.0.0\n", "版本1.0.0", "999999999999999999999.0.0"] {
            assert!(parse_release(&response(tag), "0.7.0").is_err(), "{tag}");
        }
        for bytes in [b"not JSON".as_slice(), b"{}", b"null", b"{\"tag_name\":42}"] {
            assert!(parse_release(bytes, "0.7.0").is_err());
        }
    }

    #[test]
    fn commands_are_nonblocking_deduplicated_and_queryable_without_a_document() {
        let mut s = Session::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rx = Some(rx);
        s.updates.check = Some(Box::new(move || Ok(rx.take().expect("only one worker"))));
        for _ in 0..3 {
            assert_eq!(s.execute("help.checkForUpdates", json!({})).unwrap()["status"], "checking");
        }
        tx.send(Ok(Some(Release { version: "1.0.0".into(), url: format!("{RELEASES}/tag/v1.0.0") }))).unwrap();
        let status = s.execute("updates.status", json!({})).unwrap();
        assert_eq!(status["result"]["status"], "available");
        assert_eq!(status["result"]["release"]["version"], "1.0.0");
        assert!(s.journal.is_empty());
    }

    #[test]
    fn graceful_failures_and_preference_compatibility() {
        let mut s = Session::new();
        assert!(s.execute("help.checkForUpdates", json!({})).is_err());
        for p in [Value::Null, json!([]), json!({"url": "evil"}), json!("bad")] {
            assert!(s.execute("updates.status", p.clone()).is_err());
            assert!(check(&mut s, &p).is_err());
        }
        s.updates.check = Some(Box::new(|| Err("Could not start worker".into())));
        assert!(s.execute("help.checkForUpdates", json!({})).is_err());
        assert!(matches!(s.updates.status, Status::Failed { .. }));
        s.updates.check = Some(Box::new(|| {
            let (_tx, rx) = std::sync::mpsc::channel();
            Ok(rx)
        }));
        s.execute("help.checkForUpdates", json!({})).unwrap();
        assert_eq!(s.execute("updates.status", json!({})).unwrap()["result"]["status"], "failed");
        s.load_prefs_json("{}").unwrap();
        assert!(s.prefs().general.automatically_check_updates);
        s.execute("prefs.set", json!({"path": "general.automaticallyCheckUpdates", "value": false})).unwrap();
        let saved = s.prefs_to_json();
        let mut restored = Session::new();
        restored.load_prefs_json(&saved).unwrap();
        assert!(!restored.prefs().general.automatically_check_updates);
    }
}
