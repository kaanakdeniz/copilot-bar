use base64::Engine;
use copilot_bar_lib::core::gh::Gh;
use copilot_bar_lib::core::updater::{Installed, Updater};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const REPO: &str = "octo/copilot-bar";
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Tauri's format: the public key and each signature file are base64 of minisign's text.
struct Key {
    pk: minisign::PublicKey,
    sk: minisign::SecretKey,
}

impl Key {
    fn generate() -> Key {
        let minisign::KeyPair { pk, sk } = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        Key { pk, sk }
    }

    fn public(&self) -> String {
        B64.encode(self.pk.to_box().unwrap().to_string())
    }

    /// Signs as `tauri signer sign --app-version 1.3.0` does.
    fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.sign_with_comment(data, "timestamp:0\tfile:CopilotBar.app.tar.gz\tversion:1.3.0")
    }

    fn sign_with_comment(&self, data: &[u8], trusted_comment: &str) -> Vec<u8> {
        let sig = minisign::sign(Some(&self.pk), &self.sk, std::io::Cursor::new(data), Some(trusted_comment), Some("signature from tauri secret key")).unwrap();
        B64.encode(sig.to_string()).into_bytes()
    }
}

struct Machine {
    dir: PathBuf,
}

impl Machine {
    fn new() -> Machine {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("copilotbar-install-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin/assets")).unwrap();
        std::fs::create_dir_all(dir.join("home")).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        std::fs::copy(fixtures.join("gh/fake-gh"), dir.join("bin/gh")).unwrap();
        std::fs::write(dir.join("bin/scenario"), "ok").unwrap();
        let m = Machine { dir };
        write_app(&m.dir.join("Applications/CopilotBar.app"), "1.2.0", "old binary");
        m
    }

    fn applications(&self) -> PathBuf {
        self.dir.join("Applications")
    }

    fn installed(&self) -> PathBuf {
        self.applications().join("CopilotBar.app")
    }

    fn user_applications(&self) -> PathBuf {
        self.dir.join("home/Applications")
    }

    fn publish_archive(&self, version: &str, binary: &str) -> Vec<u8> {
        let src = self.dir.join(format!("build-{version}-{}", binary.len()));
        write_app(&src.join("CopilotBar.app"), version, binary);
        let archive = self.dir.join("bin/assets/CopilotBar.app.tar.gz");
        let ok = std::process::Command::new("/usr/bin/tar")
            .env("COPYFILE_DISABLE", "1")
            .args(["-czf"])
            .arg(&archive)
            .arg("-C")
            .arg(&src)
            .arg("CopilotBar.app")
            .status()
            .unwrap()
            .success();
        assert!(ok, "tar");
        std::fs::remove_dir_all(&src).unwrap();
        std::fs::read(&archive).unwrap()
    }

    fn publish_signature(&self, sig: &[u8]) {
        std::fs::write(self.dir.join("bin/assets/CopilotBar.app.tar.gz.sig"), sig).unwrap();
    }

    fn updater(&self, key: &Key) -> Updater {
        Updater::new(Gh::at(self.dir.join("bin/gh")), Some(REPO), "1.2.0").with_public_key(&key.public())
    }

    async fn install(&self, key: &Key) -> Result<Installed, String> {
        self.updater(key).install("1.3.0", &self.installed(), &self.user_applications()).await
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("bin/calls")).unwrap_or_default().lines().map(String::from).collect()
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = std::process::Command::new("/bin/chmod").args(["-R", "u+w"]).arg(&self.dir).status();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn write_app(app: &Path, version: &str, binary: &str) {
    std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    std::fs::write(
        app.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>copilot-bar</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
</dict></plist>
"#
        ),
    )
    .unwrap();
    std::fs::write(app.join("Contents/MacOS/copilot-bar"), binary).unwrap();
}

fn tree(dir: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        entries.sort();
        for p in entries {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if p.is_dir() {
                out.insert(rel, None);
                walk(root, &p, out);
            } else {
                out.insert(rel, Some(std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn binary_of(app: &Path) -> String {
    std::fs::read_to_string(app.join("Contents/MacOS/copilot-bar")).unwrap()
}

fn error_of(r: Result<Installed, String>) -> String {
    match r {
        Err(message) => message,
        Ok(i) => panic!("expected the install to fail, but it installed {i:?}"),
    }
}

#[tokio::test]
async fn a_validly_signed_archive_replaces_the_installed_app() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign(&archive));

    let installed = m.install(&key).await.expect("installs");

    assert_eq!(installed, Installed { bundle: m.installed(), fell_back: false });
    assert_eq!(binary_of(&m.installed()), "new binary");
    let names: Vec<_> = std::fs::read_dir(m.applications()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["CopilotBar.app"], "no staging or backup copies are left behind");
    let calls = m.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].starts_with(&format!(
            "release download v1.3.0 --repo github.com/{REPO} --pattern CopilotBar.app.tar.gz --pattern CopilotBar.app.tar.gz.sig --dir /"
        )),
        "{}",
        calls[0]
    );
}

async fn assert_refused(m: &Machine, key: &Key) -> String {
    let before = tree(&m.applications());
    let message = error_of(m.install(key).await);
    assert_eq!(tree(&m.applications()), before, "the installed app is untouched: {message}");
    assert!(!m.user_applications().join("CopilotBar.app").exists(), "nothing went to ~/Applications: {message}");
    message
}

#[tokio::test]
async fn a_tampered_archive_installs_nothing() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign(&archive));
    m.publish_archive("1.3.0", "evil binary"); // replaced after signing

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("signature") && message.contains("tampered"), "{message}");
}

#[tokio::test]
async fn a_missing_signature_installs_nothing() {
    let m = Machine::new();
    let key = Key::generate();
    m.publish_archive("1.3.0", "new binary");

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("no signature"), "{message}");
}

#[tokio::test]
async fn a_signature_from_another_key_installs_nothing() {
    let m = Machine::new();
    let (key, other) = (Key::generate(), Key::generate());
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&other.sign(&archive));

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("different key"), "{message}");
}

#[tokio::test]
async fn a_garbled_signature_installs_nothing() {
    let m = Machine::new();
    let key = Key::generate();
    m.publish_archive("1.3.0", "new binary");
    m.publish_signature(b"not a signature");

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("isn't a valid signature"), "{message}");
}

#[tokio::test]
async fn a_signed_archive_of_another_version_installs_nothing() {
    // An old, validly signed archive re-uploaded under a newer release must not downgrade the app.
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.1.0", "old signed binary");
    m.publish_signature(&key.sign(&archive));

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("1.1.0") && message.contains("not 1.3.0"), "{message}");
}

#[tokio::test]
async fn a_signature_bound_to_another_version_installs_nothing() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign_with_comment(&archive, "timestamp:0\tfile:CopilotBar.app.tar.gz\tversion:1.2.9"));

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("signed for version 1.2.9"), "{message}");
}

#[tokio::test]
async fn a_signature_not_bound_to_a_version_installs_nothing() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign_with_comment(&archive, "timestamp:0\tfile:CopilotBar.app.tar.gz"));

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("isn't signed for a version"), "{message}");
}

#[tokio::test]
async fn a_failed_download_installs_nothing_and_says_why() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign(&archive));
    std::fs::write(m.dir.join("bin/scenario"), "offline").unwrap();

    let message = assert_refused(&m, &key).await;
    assert!(message.contains("Couldn't download") && message.contains("Can't reach GitHub"), "{message}");
}

#[tokio::test]
async fn an_unwritable_folder_installs_to_the_users_applications_folder() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign(&archive));
    let before = tree(&m.applications());
    std::fs::set_permissions(m.applications(), std::os::unix::fs::PermissionsExt::from_mode(0o555)).unwrap();

    let installed = m.install(&key).await.expect("installs");

    let fallback = m.user_applications().join("CopilotBar.app");
    assert_eq!(installed, Installed { bundle: fallback.clone(), fell_back: true });
    assert_eq!(binary_of(&fallback), "new binary");
    assert_eq!(tree(&m.applications()), before, "the old copy stays where it was");
    let names: Vec<_> = std::fs::read_dir(m.user_applications()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["CopilotBar.app"]);
}

#[tokio::test]
async fn an_installed_update_carries_no_quarantine() {
    let m = Machine::new();
    let key = Key::generate();
    let archive = m.publish_archive("1.3.0", "new binary");
    m.publish_signature(&key.sign(&archive));

    m.install(&key).await.expect("installs");

    let out = std::process::Command::new("/usr/bin/xattr").arg("-r").arg(m.installed()).output().unwrap();
    let attrs = String::from_utf8_lossy(&out.stdout);
    assert!(!attrs.contains("com.apple.quarantine"), "{attrs}");
}

#[tokio::test]
#[ignore = "needs COPILOTBAR_REAL_ARCHIVE, COPILOTBAR_REAL_VERSION and COPILOTBAR_REAL_PUBKEY; run with --ignored"]
async fn a_real_built_archive_installs_without_quarantine() {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (archive, version, pubkey) = (PathBuf::from(env("COPILOTBAR_REAL_ARCHIVE")), env("COPILOTBAR_REAL_VERSION"), env("COPILOTBAR_REAL_PUBKEY"));
    let m = Machine::new();
    std::fs::copy(&archive, m.dir.join("bin/assets/CopilotBar.app.tar.gz")).unwrap();
    std::fs::copy(archive.with_extension("gz.sig"), m.dir.join("bin/assets/CopilotBar.app.tar.gz.sig")).unwrap();

    let updater = Updater::new(Gh::at(m.dir.join("bin/gh")), Some(REPO), "0.0.1").with_public_key(&pubkey);
    let installed = updater.install(&version, &m.installed(), &m.user_applications()).await.expect("installs");

    assert_eq!(installed, Installed { bundle: m.installed(), fell_back: false });
    let xattrs = std::process::Command::new("/usr/bin/xattr").arg("-r").arg(m.installed()).output().unwrap();
    let xattrs = String::from_utf8_lossy(&xattrs.stdout);
    assert!(!xattrs.contains("com.apple.quarantine"), "{xattrs}");
    let codesign = std::process::Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(m.installed()).output().unwrap();
    assert!(codesign.status.success(), "{}", String::from_utf8_lossy(&codesign.stderr));
}
