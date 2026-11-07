//! Windows x64 AI packages. HTTPS + GitHub's SHA-256 is the trust boundary for unsigned betas.
//! The helper never runs shell text, and never touches documents or profile directories.
use super::{Kind, Update};
use craft_update::{Http, Stop, process};
use photocraft_engine::updates_cmds::{PrepareFn, ReadyFn};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WOW64_64KEY},
};

const FLAG: &str = "--ai-finish-update";
const REGISTRY: &str = r"Software\BLEZZ0102\PhotoCraftAI";

fn cache() -> Result<PathBuf, String> {
    Ok(craft_update::Paths::per_user("PhotoCraftAI")?.cache.join("ai-updates"))
}

pub(super) fn installed_kind() -> Result<Kind, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Development builds must never replace themselves or install a release over a checkout.
    if craft_update::self_update::current(&photocraft_ui_egui::UPDATE_APP).is_none() {
        return Err("Development builds do not install updates".into());
    }
    kind_at(&exe)
}

fn kind_at(exe: &Path) -> Result<Kind, String> {
    if exe.file_name().is_none_or(|n| !n.to_string_lossy().eq_ignore_ascii_case("photocraft.exe")) {
        return Err("Unknown AI executable".into());
    }
    let folder = exe.parent().ok_or("No application folder")?;
    if folder.join("portable.txt").is_file() {
        return Ok(Kind::Portable);
    }
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(REGISTRY, KEY_READ | KEY_WOW64_64KEY)
        .map_err(|_| "This copy is not an installed AI Beta or portable package")?;
    let dir: String = key.get_value("InstallDir").map_err(|e| e.to_string())?;
    if std::fs::canonicalize(dir).map_err(|e| e.to_string())? != std::fs::canonicalize(folder).map_err(|e| e.to_string())? {
        return Err("This copy does not match the AI installation folder".into());
    }
    Ok(Kind::Msi)
}

#[derive(Serialize, Deserialize)]
struct Job {
    pid: u32,
    target: PathBuf,
    old_hash: String,
    version: String,
    kind: Kind,
    package: PathBuf,
    hash: String,
}

pub(super) fn service() -> PrepareFn {
    Box::new(|version| {
        let version = version.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("AI update download".into())
            .spawn(move || {
                let _ = tx.send(prepare(&version));
            })
            .map_err(|e| e.to_string())?;
        Ok(rx)
    })
}

fn prepare(version: &str) -> Result<ReadyFn, String> {
    let kind = installed_kind()?;
    let http = Http::new("Photocraft-dev")?;
    // Re-fetch publication and digest at the explicit click, rather than trusting UI parameters.
    let update = super::fetch(&http, photocraft_engine::build_info::VERSION, kind)?
        .filter(|u| u.version == version)
        .ok_or("This AI package is no longer available; check again")?;
    let root = cache()?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let dir = tempfile::Builder::new().prefix("update-").tempdir_in(&root).map_err(|e| e.to_string())?;
    let package = dir.path().join(&update.asset.name);
    let hash = update.asset.sha256.as_deref().ok_or("No published checksum")?;
    http.download(&update.asset.url, &package, update.asset.size, hash, &Stop::default(), &mut |_| {})?;
    let target = std::env::current_exe().and_then(std::fs::canonicalize).map_err(|e| e.to_string())?;
    let job = Job {
        pid: std::process::id(),
        old_hash: craft_update::http::sha256_file(&target)?,
        target,
        version: update.version.clone(),
        kind,
        package,
        hash: hash.into(),
    };
    validate_package(&job, &update)?;
    // Reuse one helper slot across updates instead of retaining a full executable per release.
    let helper = root.join("ai-update-helper.exe");
    std::fs::copy(&job.target, &helper).map_err(|e| e.to_string())?;
    let path = dir.path().join("job.json");
    craft_update::paths::write_atomic(&path, &serde_json::to_vec(&job).map_err(|e| e.to_string())?)?;
    Ok(Box::new(move || {
        if kind_at(&job.target)? != kind
            || craft_update::http::sha256_file(&job.target)? != job.old_hash
            || craft_update::http::sha256_file(&helper)? != job.old_hash
        {
            return Err("The installed AI application changed; check again".into());
        }
        let mut command = process::command(&helper);
        command.arg(FLAG).arg(&path);
        process::spawn_detached(command)?;
        // The helper owns its staging folder after launch. Drop on all preparation failures.
        let _ = dir.keep();
        Ok(())
    }))
}

fn validate_package(job: &Job, update: &Update) -> Result<(), String> {
    if job.package.file_name() != Some(std::ffi::OsStr::new(&update.asset.name))
        || update.version != job.version
        || update.asset.sha256.as_deref() != Some(job.hash.as_str())
    {
        return Err("The update package does not match its release".into());
    }
    if craft_update::http::sha256_file(&job.package)? != job.hash {
        return Err("The update checksum does not match; nothing was installed".into());
    }
    Ok(())
}

pub(super) fn finish() -> Option<Result<(), String>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return None;
    }
    let result = (|| {
        let path = PathBuf::from(args.next().ok_or("Missing update job")?);
        if args.next().is_some() {
            return Err("Unexpected update helper arguments".into());
        }
        finish_job(&path)
    })();
    if let Err(e) = &result
        && let Ok(root) = cache()
    {
        let _ = craft_update::paths::write_atomic(&root.join("failure.txt"), e.as_bytes());
    }
    Some(result)
}

fn finish_job(path: &Path) -> Result<(), String> {
    let me = std::env::current_exe().and_then(std::fs::canonicalize).map_err(|e| e.to_string())?;
    if me.file_name() != Some(std::ffi::OsStr::new("ai-update-helper.exe")) {
        return Err("Invalid update helper location".into());
    }
    let expected_cache = cache()?.canonicalize().map_err(|e| e.to_string())?;
    validate_location(&me, path, &expected_cache)?;
    let canonical_job = path.canonicalize().map_err(|e| e.to_string())?;
    let root = canonical_job.parent().ok_or("No update job folder")?;
    let mut bytes = Vec::new();
    std::fs::File::open(path).map_err(|e| e.to_string())?.take(16 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 {
        return Err("Update job is too large".into());
    }
    let job: Job = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if job.pid == 0
        || job.pid == std::process::id()
        || job.package.parent() != Some(root)
        || !job.target.is_absolute()
        || job.target.parent() == Some(root)
        || kind_at(&job.target)? != job.kind
        || craft_update::http::sha256_file(&job.target)? != job.old_hash
    {
        return Err("Invalid AI update target".into());
    }
    let version = craft_update::Version::parse(&job.version).map_err(|e| e.to_string())?;
    let running = craft_update::Version::parse(photocraft_engine::build_info::VERSION).map_err(|e| e.to_string())?;
    if !version.cmp_precedence(&running).is_gt() {
        return Err("Refusing an AI update downgrade".into());
    }
    let suffix = match job.kind {
        Kind::Msi => "windows-x64.msi",
        Kind::Portable => "windows-x64-portable.zip",
    };
    let name = format!("photocraft-ai-{}-{suffix}", job.version);
    validate_package(
        &job,
        &Update {
            version: job.version.clone(),
            page: String::new(),
            asset: craft_update::Asset { name, sha256: Some(job.hash.clone()), ..Default::default() },
        },
    )?;
    if !process::wait_for_exit(job.pid, Duration::from_secs(120)) {
        return Err("PhotoCraft is still running; the update was cancelled".into());
    }
    let result = install(&job, root);
    if let Err(e) = &result {
        let _ = craft_update::paths::write_atomic(&cache()?.join("failure.txt"), e.as_bytes());
    }
    // Restart the old copy after failure if it still exists, so the user can see the error.
    if job.target.is_file() {
        process::spawn_detached(process::command(&job.target))?;
    }
    if result.is_ok() {
        let _ = std::fs::remove_file(&job.package);
        let _ = std::fs::remove_file(path);
    }
    result
}

fn install(job: &Job, root: &Path) -> Result<(), String> {
    // Check again after waiting: never execute a modified cached package.
    if craft_update::http::sha256_file(&job.package)? != job.hash {
        return Err("The update package changed while waiting".into());
    }
    match job.kind {
        Kind::Msi => {
            let system = std::env::var_os("SystemRoot").ok_or("No Windows system folder")?;
            let log = root.join("install.log");
            let status = msi_command(&PathBuf::from(system).join("System32/msiexec.exe"), &job.package, &log)?
                .status()
                .map_err(|e| format!("Could not start the AI installer: {e}"))?;
            match status.code() {
                Some(0 | 3010) => Ok(()),
                code => Err(format!("AI installation failed ({code:?}). Log: {}", installer_path(&log)?)),
            }
        }
        Kind::Portable => {
            let staging = root.join("staging");
            let unpacked = staging.join("unpacked");
            let folder = job.target.parent().ok_or("No portable folder")?;
            craft_update::portable::unpack_over(&job.package, &staging, folder, std::ffi::OsStr::new("photocraft.exe"), &|path| {
                let relative = path.strip_prefix(&unpacked).or_else(|_| path.strip_prefix(folder)).map_err(|_| "Unexpected portable file location")?;
                portable_file(relative)
            })
        }
    }
}

fn msi_command(exe: &Path, package: &Path, log: &Path) -> Result<std::process::Command, String> {
    // Keep canonical paths for the helper's validation. Windows Installer's service needs
    // ordinary paths for the package and log. The AI MSI restores the per-user install folder
    // from the same registry entry the helper validates, including custom folders with spaces.
    let mut command = process::command(exe);
    command
        .arg("/i")
        .arg(installer_path(package)?)
        .args(["/qn", "/norestart", "MSIINSTALLPERUSER=1", "MSIRESTARTMANAGERCONTROL=Disable"])
        .arg("/L*v")
        .arg(installer_path(log)?);
    Ok(command)
}

fn installer_path(path: &Path) -> Result<String, String> {
    let text = path.to_str().ok_or("The installer path is not valid Unicode")?;
    let ordinary =
        if let Some(share) = text.strip_prefix(r"\\?\UNC\") { format!(r"\\{share}") } else { text.strip_prefix(r"\\?\").unwrap_or(text).to_string() };
    if !Path::new(&ordinary).is_absolute() {
        return Err("The installer needs an absolute file path".into());
    }
    Ok(ordinary)
}

fn validate_location(helper: &Path, job: &Path, expected_cache: &Path) -> Result<(), String> {
    let job = job.canonicalize().map_err(|e| e.to_string())?;
    if helper.parent() != Some(expected_cache)
        || job.parent().and_then(Path::parent) != Some(expected_cache)
        || job.file_name() != Some(std::ffi::OsStr::new("job.json"))
    {
        return Err("Invalid update helper location".into());
    }
    Ok(())
}

fn portable_file(path: &Path) -> Result<(), String> {
    // Never let even a malformed release overwrite a portable user's settings/recovery data.
    if path.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("PhotoCraftData")) {
        return Err("An update tried to replace portable user data".into());
    }
    let name = path.file_name().and_then(|s| s.to_str()).ok_or("Invalid package file name")?;
    let mut parts = path.components();
    let first = parts.next();
    let second = parts.next();
    let allowed = parts.next().is_none()
        && !path.is_absolute()
        && match (first, second) {
            (Some(std::path::Component::Normal(_)), None) => matches!(name, "photocraft.exe" | "photocraft-cli.exe" | "portable.txt" | "BUILD.txt"),
            (Some(std::path::Component::Normal(dir)), Some(std::path::Component::Normal(_))) => dir == "docs",
            _ => false,
        };
    if !allowed {
        return Err(format!("Unexpected file in the portable update: {name}"));
    }
    Ok(())
}

pub(super) fn take_failure() -> Option<String> {
    let path = cache().ok()?.join("failure.txt");
    let mut text = String::new();
    std::fs::File::open(&path).ok()?.take(16 * 1024).read_to_string(&mut text).ok()?;
    let _ = std::fs::remove_file(path);
    Some(text.chars().take(4000).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn canonical_helper_accepts_the_original_job_path() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().canonicalize().unwrap();
        let stage = tempfile::tempdir_in(&cache).unwrap();
        let helper = cache.join("ai-update-helper.exe");
        let job = stage.path().join("job.json");
        std::fs::write(&helper, "helper").unwrap();
        std::fs::write(&job, "{}").unwrap();
        let normal_job = PathBuf::from(installer_path(&job).unwrap());
        assert!(validate_location(&helper.canonicalize().unwrap(), &normal_job, &cache).is_ok());
        assert!(validate_location(&helper.canonicalize().unwrap(), &normal_job, stage.path()).is_err());
    }

    #[test]
    fn msi_arguments_use_ordinary_paths_with_spaces_and_unicode() {
        let package = Path::new(r"\\?\C:\更新 缓存\update.msi");
        let log = Path::new(r"\\?\C:\更新 缓存\install.log");
        let command = msi_command(Path::new("msiexec.exe"), package, log).unwrap();
        let args: Vec<_> = command.get_args().map(|arg| arg.to_str().unwrap()).collect();
        assert_eq!(
            args,
            [
                "/i",
                r"C:\更新 缓存\update.msi",
                "/qn",
                "/norestart",
                "MSIINSTALLPERUSER=1",
                "MSIRESTARTMANAGERCONTROL=Disable",
                "/L*v",
                r"C:\更新 缓存\install.log",
            ]
        );
        assert_eq!(installer_path(Path::new(r"\\?\UNC\server\share\update.msi")).unwrap(), r"\\server\share\update.msi");
        assert!(installer_path(Path::new(r"\\?\Volume{example}\update.msi")).is_err());
    }

    #[test]
    fn portable_install_replaces_binaries_and_preserves_profiles_and_documents() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("portable");
        std::fs::create_dir_all(folder.join("PhotoCraftData")).unwrap();
        std::fs::write(folder.join("photocraft.exe"), "old app").unwrap();
        std::fs::write(folder.join("PhotoCraftData/settings.json"), "user settings").unwrap();
        std::fs::write(folder.join("work.pcraft"), "user work").unwrap();
        let package = dir.path().join("update.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&package).unwrap());
        zip.start_file("photocraft.exe", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"new app").unwrap();
        zip.finish().unwrap();
        let job = Job {
            pid: 1,
            target: folder.join("photocraft.exe"),
            old_hash: String::new(),
            version: String::new(),
            kind: Kind::Portable,
            hash: craft_update::http::sha256_file(&package).unwrap(),
            package,
        };
        install(&job, dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&job.target).unwrap(), "new app");
        assert_eq!(std::fs::read_to_string(folder.join("PhotoCraftData/settings.json")).unwrap(), "user settings");
        assert_eq!(std::fs::read_to_string(folder.join("work.pcraft")).unwrap(), "user work");
    }

    #[test]
    fn a_corrupt_or_data_overwriting_package_leaves_the_old_app_intact() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("portable");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("photocraft.exe"), "old app").unwrap();
        let package = dir.path().join("update.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&package).unwrap());
        for name in ["photocraft.exe", "PhotoCraftData/settings.json"] {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"replacement").unwrap();
        }
        zip.finish().unwrap();
        let mut job = Job {
            pid: 1,
            target: folder.join("photocraft.exe"),
            old_hash: String::new(),
            version: String::new(),
            kind: Kind::Portable,
            hash: "0".repeat(64),
            package,
        };
        assert!(install(&job, dir.path()).is_err());
        job.hash = craft_update::http::sha256_file(&job.package).unwrap();
        assert!(install(&job, dir.path()).is_err());
        assert_eq!(std::fs::read_to_string(&job.target).unwrap(), "old app");
        assert!(!folder.join("PhotoCraftData/settings.json").exists());
    }
    #[test]
    fn portable_updates_reject_user_data_and_unknown_files() {
        for file in ["PhotoCraftData/settings.json", "PhotoCraftData/docs/README.md", "other.exe", "PHOTOC~1/docs/README.md", "nested/photocraft.exe"] {
            assert!(portable_file(Path::new(file)).is_err());
        }
        for file in ["photocraft.exe", "docs/README.md", "portable.txt"] {
            assert!(portable_file(Path::new(file)).is_ok());
        }
    }
    #[test]
    fn helper_rejects_foreign_jobs_without_installing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("job.json");
        std::fs::write(&path, "{}").unwrap();
        assert!(finish_job(&path).unwrap_err().contains("location"));
    }
}
