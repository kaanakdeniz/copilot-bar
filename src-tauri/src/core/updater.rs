use super::gh::{Gh, GhError, HOST};
use super::settings::Settings;
use super::signature;
use semver::Version;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const ARCHIVE: &str = "CopilotBar.app.tar.gz";
pub const SIGNATURE: &str = "CopilotBar.app.tar.gz.sig";
pub const APP: &str = "CopilotBar.app";

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_ARCHIVE_MIB: u64 = 200;
const MAX_ARCHIVE_BYTES: u64 = MAX_ARCHIVE_MIB * 1024 * 1024;

pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Scheduled,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available { version: String, url: String },
    Downloading { version: String },
    Installing { version: String },
    Error { message: String, url: Option<String> },
}

impl UpdateState {
    pub fn is_busy(&self) -> bool {
        matches!(self, UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing { .. })
    }
}

const NO_REPOSITORY: &str = "This build doesn't know which repository it's released from, so it can't check for updates or update itself.";

#[derive(Debug, Clone)]
pub struct Updater {
    gh: Gh,
    repository: Option<String>,
    running: String,
    public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub bundle: PathBuf,
    pub fell_back: bool,
}

impl Updater {
    pub fn new(gh: Gh, repository: Option<&str>, running: &str) -> Updater {
        Updater { gh, repository: repository.map(Into::into), running: running.into(), public_key: signature::UPDATE_PUBLIC_KEY.into() }
    }

    pub fn with_public_key(self, public_key: &str) -> Updater {
        Updater { public_key: public_key.into(), ..self }
    }

    pub fn failed(&self, message: String) -> UpdateState {
        UpdateState::Error { message, url: self.release_page() }
    }

    pub async fn install(&self, version: &str, running: &Path, user_applications: &Path) -> Result<Installed, String> {
        self.download(version).await?.install(running, user_applications).await
    }

    pub async fn download(&self, version: &str) -> Result<Verified, String> {
        let repository = self.repository.as_deref().ok_or(NO_REPOSITORY)?;
        let version = Version::parse(version).ok().filter(|v| v.pre.is_empty()).ok_or_else(|| format!("{version} isn't a release version."))?;
        let download = Scratch::new(&std::env::temp_dir(), "copilotbar-update")?;
        let dir = download.0.to_str().ok_or("The temporary folder's path isn't valid UTF-8.")?;
        let tag = format!("v{version}");
        let repo = format!("{HOST}/{repository}");
        let args = ["release", "download", &tag, "--repo", &repo, "--pattern", ARCHIVE, "--pattern", SIGNATURE, "--dir", dir];
        self.gh.run(&args, DOWNLOAD_TIMEOUT).await.map_err(|e| format!("Couldn't download the update: {}", e.explanation()))?;

        let archive = read_archive(&download.0.join(ARCHIVE), &tag)?;
        let sig = std::fs::read(download.0.join(SIGNATURE))
            .map_err(|_| format!("Release {tag} has no signature for its update, so nothing was installed."))?;
        let signed = signature::verify(&self.public_key, &archive, &sig).map_err(|why| format!("The update's signature {why}, so nothing was installed."))?;
        // A validly signed archive of another version must not pass as this release.
        match signature::signed_version(&signed) {
            Some(v) if v == version.to_string() => {}
            Some(v) => return Err(format!("The update archive is signed for version {v}, not {version}, so nothing was installed.")),
            None => return Err("The update archive isn't signed for a version, so nothing was installed.".into()),
        }

        Ok(Verified { archive, version: version.to_string() })
    }

    pub async fn check(&self, trigger: Trigger, settings: &Settings) -> Option<UpdateState> {
        if !runs(trigger, settings) {
            return None;
        }
        Some(self.newest().await.unwrap_or_else(|message| self.failed(message)))
    }

    pub fn release_page(&self) -> Option<String> {
        self.repository.as_ref().map(|r| format!("https://github.com/{r}/releases/latest"))
    }

    async fn newest(&self) -> Result<UpdateState, String> {
        let repository = self.repository.as_deref().ok_or(NO_REPOSITORY)?;
        let running = Version::parse(&self.running).map_err(|e| format!("The running version {} isn't a version: {e}", self.running))?;
        let releases = self.gh.api(&format!("repos/{repository}/releases?per_page=100")).await.map_err(|e| e.explanation())?;
        let releases = releases.as_array().ok_or_else(|| GhError::Failed("unexpected response: not a list of releases".into()).to_string())?;
        Ok(match releases.iter().filter_map(published).max_by(|a, b| a.0.cmp(&b.0)) {
            Some((version, url)) if version > running => UpdateState::Available { version: version.to_string(), url },
            _ => UpdateState::UpToDate,
        })
    }
}

/// Held in memory, so what's installed is exactly what was verified.
pub struct Verified {
    archive: Vec<u8>,
    version: String,
}

impl Verified {
    pub async fn install(self, running: &Path, user_applications: &Path) -> Result<Installed, String> {
        let (running, user_applications) = (running.to_path_buf(), user_applications.to_path_buf());
        tokio::task::spawn_blocking(move || put_in_place(&self.archive, &self.version, &running, &user_applications))
            .await
            .map_err(|e| format!("The install stopped unexpectedly: {e}"))?
    }
}

fn read_archive(path: &Path, tag: &str) -> Result<Vec<u8>, String> {
    let missing = || format!("Release {tag} has no {ARCHIVE}, so nothing was installed.");
    let size = std::fs::metadata(path).map_err(|_| missing())?.len();
    if size > MAX_ARCHIVE_BYTES {
        return Err(format!("Release {tag}'s {ARCHIVE} is larger than {MAX_ARCHIVE_MIB} MiB, so nothing was installed."));
    }
    std::fs::read(path).map_err(|_| missing())
}

fn put_in_place(archive: &[u8], version: &str, running: &Path, user_applications: &Path) -> Result<Installed, String> {
    let (target, fell_back) = match running.parent() {
        Some(folder) if writable(folder) => (running.to_path_buf(), false),
        _ => {
            std::fs::create_dir_all(user_applications).map_err(|e| format!("Couldn't create {}: {e}", user_applications.display()))?;
            if !writable(user_applications) {
                return Err(format!("Neither {} nor {} is writable, so nothing was installed.", folder_of(running), user_applications.display()));
            }
            (user_applications.join(APP), true)
        }
    };
    let folder = target.parent().ok_or("The install location has no folder.")?;
    let staging = Scratch::new(folder, ".CopilotBar-update")?;
    unpack(archive, &staging.0)?;
    let app = staging.0.join(APP);
    let entries: Vec<_> = std::fs::read_dir(&staging.0).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    if entries != [APP] || !std::fs::symlink_metadata(&app).is_ok_and(|m| m.is_dir()) {
        return Err(format!("The update archive doesn't hold just {APP}, so nothing was installed."));
    }
    match bundle_version(&app) {
        Some(v) if v == version => {}
        other => {
            return Err(format!("The update archive holds version {}, not {version}, so nothing was installed.", other.as_deref().unwrap_or("(unknown)")))
        }
    }
    if std::fs::symlink_metadata(&target).is_ok() {
        let previous = staging.0.join("previous.app");
        std::fs::rename(&target, &previous).map_err(|e| format!("Couldn't move the installed app aside ({e}), so nothing was installed."))?;
        if let Err(e) = std::fs::rename(&app, &target) {
            let _ = std::fs::rename(&previous, &target);
            return Err(format!("Couldn't put the update in place ({e}); the installed app was left as it was."));
        }
    } else {
        std::fs::rename(&app, &target).map_err(|e| format!("Couldn't put the update in place: {e}"))?;
    }
    Ok(Installed { bundle: target, fell_back })
}

/// Unpacks with the system tar, which refuses absolute paths, `..` and writes through symlinks.
fn unpack(archive: &[u8], into: &Path) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new("/usr/bin/tar")
        .args(["-xzf", "-", "-C"])
        .arg(into)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't run tar to unpack the update: {e}"))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let data = archive.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&data));
    let out = child.wait_with_output().map_err(|e| format!("Couldn't unpack the update: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Couldn't unpack the update ({}), so nothing was installed.", why.trim()));
    }
    Ok(())
}

fn bundle_version(app: &Path) -> Option<String> {
    let out = std::process::Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn writable(folder: &Path) -> bool {
    match Scratch::new(folder, ".CopilotBar-write-test") {
        Ok(_probe) => true,
        Err(_) => false,
    }
}

fn folder_of(bundle: &Path) -> String {
    bundle.parent().map_or_else(|| bundle.display().to_string(), |p| p.display().to_string())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(parent: &Path, prefix: &str) -> Result<Scratch, String> {
        use std::os::unix::fs::DirBuilderExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        let path = parent.join(format!("{prefix}-{}-{nanos}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        std::fs::DirBuilder::new().mode(0o700).create(&path).map_err(|e| format!("Couldn't create a folder in {}: {e}", parent.display()))?;
        Ok(Scratch(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn published(release: &Value) -> Option<(Version, String)> {
    if release["draft"].as_bool() != Some(false) || release["prerelease"].as_bool() != Some(false) {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok().filter(|v| v.pre.is_empty())?;
    Some((version, release["html_url"].as_str()?.to_string()))
}

pub fn runs(trigger: Trigger, settings: &Settings) -> bool {
    trigger == Trigger::Manual || settings.auto_update_check
}

pub fn announcement(announced: Option<&str>, next: &UpdateState) -> Option<(String, String)> {
    match next {
        UpdateState::Available { version, .. } if announced != Some(version.as_str()) => Some((
            format!("CopilotBar {version} is available"),
            format!("Choose Update to {version} in CopilotBar's menu bar menu."),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(v: &str) -> UpdateState {
        UpdateState::Available { version: v.into(), url: format!("https://github.com/o/n/releases/tag/v{v}") }
    }

    #[test]
    fn checking_downloading_and_installing_are_busy() {
        let v = || "1.3.0".to_string();
        for s in [UpdateState::Checking, UpdateState::Downloading { version: v() }, UpdateState::Installing { version: v() }] {
            assert!(s.is_busy(), "{s:?}");
        }
        for s in [UpdateState::Idle, UpdateState::UpToDate, available("1.3.0"), UpdateState::Error { message: "x".into(), url: None }] {
            assert!(!s.is_busy(), "{s:?}");
        }
    }

    #[test]
    fn an_archive_over_the_size_limit_is_refused_before_it_is_read() {
        let scratch = Scratch::new(&std::env::temp_dir(), "copilotbar-size-test").expect("scratch folder");
        let path = scratch.0.join(ARCHIVE);
        std::fs::File::create(&path).unwrap().set_len(MAX_ARCHIVE_BYTES + 1).unwrap();
        assert_eq!(
            read_archive(&path, "v1.3.0").unwrap_err(),
            "Release v1.3.0's CopilotBar.app.tar.gz is larger than 200 MiB, so nothing was installed."
        );
    }

    #[test]
    fn an_archive_within_the_size_limit_is_read_whole() {
        let scratch = Scratch::new(&std::env::temp_dir(), "copilotbar-size-test").expect("scratch folder");
        let path = scratch.0.join(ARCHIVE);
        std::fs::write(&path, b"archive").unwrap();
        assert_eq!(read_archive(&path, "v1.3.0").unwrap(), b"archive");
    }

    #[test]
    fn a_newly_available_version_is_announced_once() {
        let (title, body) = announcement(None, &available("1.3.0")).expect("announced");
        assert_eq!(title, "CopilotBar 1.3.0 is available");
        assert!(body.contains("Update to 1.3.0"), "{body}");
        assert_eq!(announcement(Some("1.3.0"), &available("1.3.0")), None, "the same version again");
        assert!(announcement(Some("1.3.0"), &available("1.4.0")).is_some(), "a newer one");
        assert_eq!(announcement(None, &UpdateState::UpToDate), None);
        assert_eq!(announcement(None, &UpdateState::Error { message: "x".into(), url: None }), None, "failed checks stay quiet");
    }
}
