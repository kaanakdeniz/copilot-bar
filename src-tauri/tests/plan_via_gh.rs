use copilot_bar_lib::core::collector::Collector;
use copilot_bar_lib::core::gh::Gh;
use copilot_bar_lib::core::settings::Settings;
use copilot_bar_lib::core::snapshot::{SignIn, Snapshot};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Machine {
    dir: PathBuf,
}

impl Machine {
    fn new(scenario: &str, copilot_login: &str) -> Machine {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("copilotbar-gh-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::copy(fixtures().join("gh/fake-gh"), dir.join("bin/gh")).unwrap();
        std::fs::copy(fixtures().join("copilot-user.json"), dir.join("bin/copilot-user.json")).unwrap();
        std::fs::write(dir.join("bin/scenario"), scenario).unwrap();
        // The Copilot CLI's config starts with `//` comments, like the real file.
        let config = format!("// Copilot CLI settings\n{{\"lastLoggedInUser\":{{\"host\":\"https://github.com\",\"login\":\"{copilot_login}\"}}}}\n");
        std::fs::write(dir.join("copilot-config.json"), config).unwrap();
        Machine { dir }
    }

    fn gh(&self) -> Gh {
        Gh::at(self.dir.join("bin/gh"))
    }

    fn copilot_config(&self) -> PathBuf {
        self.dir.join("copilot-config.json")
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn settings() -> Settings {
    Settings { cli_log_dir: Some(fixtures().join("session-state").to_string_lossy().into()), ..Settings::default() }
}

async fn refresh_with(gh: Gh, copilot_config: PathBuf) -> Snapshot {
    let s = settings();
    let mut c = Collector::new(gh, copilot_config);
    c.refresh(&s).await;
    c.snapshot(&s)
}

async fn refresh(m: &Machine) -> Snapshot {
    refresh_with(m.gh(), m.copilot_config()).await
}

fn error(s: &Snapshot) -> &str {
    s.status.plan_error.as_ref().map_or("", |e| e.message.as_str())
}

fn offline(s: &Snapshot) -> bool {
    s.status.plan_error.as_ref().is_some_and(|e| e.offline)
}

#[tokio::test]
async fn signed_in_gh_fills_the_plan_fields() {
    let m = Machine::new("ok", "octo");
    let s = refresh(&m).await;
    let plan = s.github_plan.as_ref().expect("plan data");
    assert_eq!(plan.login, "octo");
    assert_eq!(plan.entitlement, Some(1000.0));
    assert_eq!(plan.credits_used, Some(412.5));
    assert!(plan.overage_permitted);
    assert_eq!(plan.reset_at, Some(1_796_083_200_000), "2026-12-01T00:00:00Z");
    assert_eq!(s.status.sign_in, SignIn::SignedIn { login: "octo".into() });
    assert_eq!(s.status.plan_error, None);
    assert!(!s.records.is_empty());
}

fn assert_local_only(s: &Snapshot) {
    assert!(!s.records.is_empty(), "local usage keeps working");
    assert!(s.pace.used > 0.0, "Used comes from Logged AIC");
    assert!(s.github_plan.is_none());
}

#[tokio::test]
async fn missing_gh_says_to_install_it() {
    let m = Machine::new("ok", "octo");
    let s = refresh_with(Gh::at(m.dir.join("bin/not-gh")), m.copilot_config()).await;
    assert_eq!(s.status.sign_in, SignIn::GhMissing);
    assert!(error(&s).contains("brew install gh"), "{}", error(&s));
    assert_local_only(&s);
}

#[tokio::test]
async fn signed_out_gh_says_to_sign_in() {
    let m = Machine::new("signed-out", "octo");
    let s = refresh(&m).await;
    assert_eq!(s.status.sign_in, SignIn::SignedOut);
    assert!(error(&s).contains("gh auth login"), "{}", error(&s));
    assert_local_only(&s);
}

#[tokio::test]
async fn sign_in_not_authorised_for_sso_says_how_to_authorise() {
    let m = Machine::new("sso", "octo");
    let s = refresh(&m).await;
    assert_eq!(s.status.sign_in, SignIn::SsoRequired);
    assert!(error(&s).contains("SSO") && error(&s).contains("gh auth refresh"), "{}", error(&s));
    assert!(!offline(&s));
    assert_local_only(&s);
}

#[tokio::test]
async fn a_403_that_only_mentions_sso_is_not_an_sso_authorisation_error() {
    let m = Machine::new("forbidden-mentioning-sso", "octo");
    let s = refresh(&m).await;
    let message = "GitHub returned 403: Resource not accessible by integration. Ask an SSO admin to grant repository access.";
    assert_eq!(s.status.sign_in, SignIn::Error { message: message.into() });
    assert_local_only(&s);
}

#[tokio::test]
async fn rejected_sign_in_401_says_to_sign_in_again() {
    let m = Machine::new("bad-credentials", "octo");
    let s = refresh(&m).await;
    assert_eq!(s.status.sign_in, SignIn::SignedOut);
    assert!(error(&s).contains("401") && error(&s).contains("gh auth login"), "{}", error(&s));
    assert_local_only(&s);
}

#[tokio::test]
async fn other_errors_say_what_github_returned() {
    let m = Machine::new("server-error", "octo");
    let s = refresh(&m).await;
    assert_eq!(s.status.sign_in, SignIn::Error { message: "GitHub returned 502: Server Error".into() });
    assert_eq!(error(&s), "GitHub returned 502: Server Error");
    assert!(!offline(&s), "GitHub answered");
    assert_local_only(&s);
}

#[tokio::test]
async fn no_network_says_github_cant_be_reached_and_keeps_earlier_plan_data() {
    let m = Machine::new("ok", "octo");
    let s = settings();
    let mut c = Collector::new(m.gh(), m.copilot_config());
    c.refresh(&s).await;
    std::fs::write(m.dir.join("bin/scenario"), "offline").unwrap();
    c.refresh(&s).await;
    let snap = c.snapshot(&s);
    assert!(error(&snap).starts_with("Can't reach GitHub"), "{}", error(&snap));
    assert!(offline(&snap));
    assert!(matches!(snap.status.sign_in, SignIn::Error { .. }));
    assert!(snap.github_plan.is_some(), "the sign-in is fine, so the last plan data still stands");
    assert!(!snap.records.is_empty());
}

#[tokio::test]
async fn gh_signed_in_as_another_account_than_the_copilot_cli_is_flagged() {
    let m = Machine::new("ok", "octo-work");
    let s = refresh(&m).await;
    assert_eq!(s.status.sign_in, SignIn::AccountMismatch { gh_login: "octo".into(), copilot_login: "octo-work".into() });
    assert!(!s.records.is_empty());
}

#[tokio::test]
async fn without_a_copilot_cli_sign_in_gh_account_is_used() {
    let m = Machine::new("ok", "octo");
    let s = refresh_with(m.gh(), m.dir.join("no-config.json")).await;
    assert_eq!(s.status.sign_in, SignIn::SignedIn { login: "octo".into() });
}

#[tokio::test]
async fn losing_the_sign_in_drops_plan_data_from_the_earlier_one() {
    let m = Machine::new("ok", "octo");
    let s = settings();
    let mut c = Collector::new(m.gh(), m.copilot_config());
    c.refresh(&s).await;
    assert!(c.snapshot(&s).github_plan.is_some());
    std::fs::write(m.dir.join("bin/scenario"), "signed-out").unwrap();
    c.refresh(&s).await;
    assert_local_only(&c.snapshot(&s));
}
