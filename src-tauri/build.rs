#[path = "src/core/repository.rs"]
mod repository;

fn repository() -> String {
    println!("cargo:rerun-if-changed=../.git/config");
    for var in ["COPILOTBAR_REPOSITORY", "GITHUB_REPOSITORY"] {
        println!("cargo:rerun-if-env-changed={var}");
        if let Some(r) = std::env::var(var).ok().as_deref().and_then(repository::valid) {
            return r;
        }
    }
    std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| repository::parse_remote(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// A build without a key fails: the updater must never run unable to check signatures.
fn update_public_key() -> String {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    let conf = std::fs::read_to_string("tauri.conf.json").expect("read tauri.conf.json");
    let conf: serde_json::Value = serde_json::from_str(&conf).expect("tauri.conf.json is JSON");
    match conf["plugins"]["updater"]["pubkey"].as_str().map(str::trim) {
        Some(key) if !key.is_empty() => key.to_string(),
        _ => panic!("tauri.conf.json has no plugins.updater.pubkey; CopilotBar can't verify updates without it"),
    }
}

fn main() {
    println!("cargo:rustc-env=COPILOTBAR_REPOSITORY={}", repository());
    println!("cargo:rustc-env=COPILOTBAR_UPDATE_PUBKEY={}", update_public_key());
    tauri_build::build()
}
