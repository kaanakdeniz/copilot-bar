use copilot_bar_lib::core::gh::Gh;
use copilot_bar_lib::core::settings::Settings;
use copilot_bar_lib::core::updater::{Trigger, UpdateState, Updater};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const REPO: &str = "octo/copilot-bar";

struct Machine {
    dir: PathBuf,
}

impl Machine {
    fn new(scenario: &str, releases: Value) -> Machine {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("copilotbar-update-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        std::fs::copy(fixtures.join("gh/fake-gh"), dir.join("bin/gh")).unwrap();
        std::fs::write(dir.join("bin/scenario"), scenario).unwrap();
        std::fs::write(dir.join("bin/releases.json"), releases.to_string()).unwrap();
        Machine { dir }
    }

    fn updater(&self, running: &str) -> Updater {
        Updater::new(Gh::at(self.dir.join("bin/gh")), Some(REPO), running)
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("bin/calls")).unwrap_or_default().lines().map(String::from).collect()
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn release(tag: &str) -> Value {
    json!({ "tag_name": tag, "html_url": format!("https://github.com/{REPO}/releases/tag/{tag}"), "draft": false, "prerelease": false })
}

async fn check(m: &Machine, running: &str) -> UpdateState {
    m.updater(running).check(Trigger::Manual, &Settings::default()).await.expect("a manual check always runs")
}

#[tokio::test]
async fn a_newer_published_release_is_available() {
    let m = Machine::new("ok", json!([release("v1.3.0"), release("v1.2.0")]));
    assert_eq!(
        check(&m, "1.2.0").await,
        UpdateState::Available { version: "1.3.0".into(), url: format!("https://github.com/{REPO}/releases/tag/v1.3.0") }
    );
    assert_eq!(m.calls(), [format!("api --hostname github.com -H Accept: application/vnd.github+json -H X-GitHub-Api-Version: 2026-03-10 repos/{REPO}/releases?per_page=100")]);
}

#[tokio::test]
async fn the_same_or_an_older_release_is_up_to_date() {
    let m = Machine::new("ok", json!([release("v1.2.0"), release("v1.1.9")]));
    assert_eq!(check(&m, "1.2.0").await, UpdateState::UpToDate);
    assert_eq!(check(&m, "1.10.0").await, UpdateState::UpToDate, "versions compare as semver, not as text");
    let none = Machine::new("ok", json!([]));
    assert_eq!(check(&none, "1.2.0").await, UpdateState::UpToDate, "no release yet");
}

#[tokio::test]
async fn drafts_and_pre_releases_are_ignored() {
    let draft = json!({ "tag_name": "v2.0.0", "html_url": "https://github.com/x/draft", "draft": true, "prerelease": false });
    let pre = json!({ "tag_name": "v1.9.0", "html_url": "https://github.com/x/pre", "draft": false, "prerelease": true });
    let rc_tag = release("v1.8.0-rc.1");
    let m = Machine::new("ok", json!([draft, pre, rc_tag, release("v1.2.0")]));
    assert_eq!(check(&m, "1.2.0").await, UpdateState::UpToDate);
    let m = Machine::new("ok", json!([draft, pre, release("v1.3.0")]));
    assert!(matches!(check(&m, "1.2.0").await, UpdateState::Available { version, .. } if version == "1.3.0"));
}

const RELEASE_PAGE: &str = "https://github.com/octo/copilot-bar/releases/latest";

fn error_of(state: UpdateState) -> (String, Option<String>) {
    match state {
        UpdateState::Error { message, url } => (message, url),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_gh_is_an_error_with_the_release_page() {
    let m = Machine::new("ok", json!([release("v9.0.0")]));
    let u = Updater::new(Gh::at(m.dir.join("bin/not-gh")), Some(REPO), "1.2.0");
    let (message, url) = error_of(u.check(Trigger::Manual, &Settings::default()).await.unwrap());
    assert!(message.contains("brew install gh"), "{message}");
    assert_eq!(url.as_deref(), Some(RELEASE_PAGE));
}

#[tokio::test]
async fn failing_gh_is_an_error_saying_what_went_wrong() {
    for (scenario, says) in [("offline", "Can't reach GitHub"), ("signed-out", "gh auth login"), ("server-error", "502"), ("sso", "SSO")] {
        let m = Machine::new(scenario, json!([]));
        let (message, url) = error_of(check(&m, "1.2.0").await);
        assert!(message.contains(says), "{scenario}: {message}");
        assert_eq!(url.as_deref(), Some(RELEASE_PAGE), "{scenario}");
    }
}

#[tokio::test]
async fn an_unexpected_answer_is_an_error() {
    let m = Machine::new("ok", json!({ "message": "Not Found" }));
    let (message, _) = error_of(check(&m, "1.2.0").await);
    assert!(message.contains("unexpected"), "{message}");
}

#[tokio::test]
async fn a_build_without_a_repository_says_so_and_runs_nothing() {
    let m = Machine::new("ok", json!([release("v9.0.0")]));
    let u = Updater::new(Gh::at(m.dir.join("bin/gh")), None, "1.2.0");
    let (message, url) = error_of(u.check(Trigger::Manual, &Settings::default()).await.unwrap());
    assert!(message.contains("repository"), "{message}");
    assert_eq!(url, None);
    assert!(m.calls().is_empty());
}

#[tokio::test]
async fn scheduled_checks_are_skipped_when_automatic_checks_are_off() {
    let m = Machine::new("ok", json!([release("v1.3.0")]));
    let off = Settings { auto_update_check: false, ..Settings::default() };
    assert_eq!(m.updater("1.2.0").check(Trigger::Scheduled, &off).await, None);
    assert!(m.calls().is_empty(), "gh isn't run at all");
    assert!(matches!(m.updater("1.2.0").check(Trigger::Manual, &off).await, Some(UpdateState::Available { .. })), "manual checks still run");
    assert!(matches!(m.updater("1.2.0").check(Trigger::Scheduled, &Settings::default()).await, Some(UpdateState::Available { .. })));
    assert_eq!(m.calls().len(), 2);
}
