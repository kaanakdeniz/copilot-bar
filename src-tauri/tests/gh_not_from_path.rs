
use copilot_bar_lib::core::collector::Collector;
use copilot_bar_lib::core::gh::Gh;
use copilot_bar_lib::core::settings::Settings;
use copilot_bar_lib::core::snapshot::SignIn;
use std::path::Path;

#[tokio::test]
async fn a_gh_only_on_the_path_is_never_run() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::env::temp_dir().join(format!("copilotbar-gh-path-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::copy(fixtures.join("gh/fake-gh"), dir.join("bin/gh")).unwrap();
    std::fs::copy(fixtures.join("copilot-user.json"), dir.join("bin/copilot-user.json")).unwrap();
    std::fs::write(dir.join("bin/scenario"), "ok").unwrap();
    std::env::set_var("PATH", format!("{}:{}", dir.join("bin").display(), std::env::var("PATH").unwrap_or_default()));

    let s = Settings { cli_log_dir: Some(fixtures.join("session-state").to_string_lossy().into()), ..Settings::default() };
    for gh in [Gh::at(dir.join("missing/gh")), Gh::at("gh")] {
        let mut c = Collector::new(gh, dir.join("copilot-config.json"));
        c.refresh(&s).await;
        assert_eq!(c.snapshot(&s).status.sign_in, SignIn::GhMissing);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
