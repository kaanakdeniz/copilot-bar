#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use copilot_bar_lib::core::{collector::Collector, settings};
use std::process::ExitCode;

const DUMP_FLAG: &str = "--dump-snapshot";
const OFFLINE_FLAG: &str = "--offline";
const DEFAULT_SNAPSHOT_PATH: &str = "copilot-snapshot.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.iter().position(|a| a == DUMP_FLAG) {
        Some(i) => match dump_snapshot(&args[i + 1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{DUMP_FLAG}: {message}");
                ExitCode::FAILURE
            }
        },
        None => {
            copilot_bar_lib::run();
            ExitCode::SUCCESS
        }
    }
}

fn dump_snapshot(args: &[String]) -> Result<(), String> {
    let offline = args.iter().any(|a| a == OFFLINE_FLAG);
    let out = args.iter().find(|a| !a.starts_with("--")).map_or(DEFAULT_SNAPSHOT_PATH, String::as_str);
    let s = settings::load();
    let snap = tauri::async_runtime::block_on(async {
        let mut c = Collector::default();
        if offline {
            c.refresh_local(&s);
        } else {
            c.refresh(&s).await;
        }
        c.snapshot(&s)
    });
    let json = serde_json::to_vec_pretty(&snap).map_err(|e| format!("encode snapshot: {e}"))?;
    std::fs::write(out, json).map_err(|e| format!("write {out}: {e}"))?;
    eprintln!("wrote {out}: {} records, {:.2} AIC used this cycle", snap.records.len(), snap.pace.used);
    Ok(())
}
