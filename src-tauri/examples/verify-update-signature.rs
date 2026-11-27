use copilot_bar_lib::core::signature::{signed_version, verify, UPDATE_PUBLIC_KEY};
use std::process::ExitCode;

fn check(archive: &str, version: &str) -> Result<String, String> {
    let sig = format!("{archive}.sig");
    let read = |p: &str| std::fs::read(p).map_err(|e| format!("can't read {p}: {e}"));
    let comment = verify(UPDATE_PUBLIC_KEY, &read(archive)?, &read(&sig)?).map_err(|why| format!("the signature {why}"))?;
    match signed_version(&comment) {
        Some(v) if v == version => Ok(format!("{sig} verifies {archive} as version {version} with the update key in tauri.conf.json")),
        Some(v) => Err(format!("the signature is for version {v}, not {version}")),
        None => Err("the signature isn't bound to a version (sign with --app-version)".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [archive, version] = args.as_slice() else {
        eprintln!("usage: verify-update-signature <archive> <version>");
        return ExitCode::from(2);
    };
    match check(archive, version) {
        Ok(ok) => {
            println!("{ok}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("verify-update-signature: {e}");
            ExitCode::FAILURE
        }
    }
}
