use serde::{Deserialize, Serialize};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

const DATA_DIR_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;
const SETTINGS_FILE: &str = "settings.json";

const DEFAULT_CYCLE_SUMMARY: &str = "gauge";
const DEFAULT_CYCLE_HEADLINE: &str = "used";
const DEFAULT_MENU_BAR_DISPLAY: &str = "icon-number";
const DEFAULT_MENU_BAR_ICON: &str = "disc";
const DEFAULT_MENU_BAR_NUMBER: &str = "pct";
const DEFAULT_THEME: &str = "system";
const DEFAULT_PALETTE: &str = "default";

const CYCLE_SUMMARIES: [&str; 2] = ["gauge", "bar"];
const CYCLE_HEADLINES: [&str; 2] = ["used", "remaining"];
const MENU_BAR_DISPLAYS: [&str; 3] = ["icon-number", "icon", "number"];
const MENU_BAR_ICONS: [&str; 4] = ["disc", "ring", "copilot", "progress"];
const MENU_BAR_NUMBERS: [&str; 4] = ["pct", "used", "left", "used-total"];
const THEMES: [&str; 3] = ["system", "light", "dark"];
const PALETTES: [&str; 2] = ["default", "colorblind"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub open_expanded: Option<bool>,
    pub cycle_summary: String,
    pub cycle_headline: String,
    pub menu_bar_display: String,
    pub menu_bar_icon: String,
    pub menu_bar_number: String,
    pub alert_colors: bool,
    pub theme: String,
    pub allowance_override: f64,
    pub pace_tolerance_pct: f64,
    pub week_starts_monday: bool,
    pub refresh_minutes: u32,
    pub sources: Sources,
    pub cli_log_dir: Option<String>,
    pub notifications: Notifications,
    pub onboarded: bool,
    pub text_scale: f64,
    pub palette: String,
    pub hotkey: String,
    pub auto_update_check: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Sources {
    pub cli: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Notifications {
    pub enabled: bool,
    pub thresholds: Vec<u32>,
    pub ahead_of_pace: bool,
    pub burst_aic: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            open_expanded: None,
            cycle_summary: DEFAULT_CYCLE_SUMMARY.into(),
            cycle_headline: DEFAULT_CYCLE_HEADLINE.into(),
            menu_bar_display: DEFAULT_MENU_BAR_DISPLAY.into(),
            menu_bar_icon: DEFAULT_MENU_BAR_ICON.into(),
            menu_bar_number: DEFAULT_MENU_BAR_NUMBER.into(),
            alert_colors: true,
            theme: DEFAULT_THEME.into(),
            allowance_override: 0.0,
            pace_tolerance_pct: 5.0,
            week_starts_monday: true,
            refresh_minutes: 5,
            sources: Sources::default(),
            cli_log_dir: None,
            notifications: Notifications::default(),
            onboarded: false,
            text_scale: 1.0,
            palette: DEFAULT_PALETTE.into(),
            hotkey: "Alt+Cmd+C".into(),
            auto_update_check: true,
        }
    }
}

impl Default for Sources {
    fn default() -> Self {
        Sources { cli: true }
    }
}

impl Default for Notifications {
    fn default() -> Self {
        Notifications { enabled: true, thresholds: vec![75, 90, 100], ahead_of_pace: true, burst_aic: 10.0 }
    }
}

fn keep_or_default(value: &mut String, allowed: &[&str], default: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = default.into();
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        keep_or_default(&mut self.cycle_summary, &CYCLE_SUMMARIES, DEFAULT_CYCLE_SUMMARY);
        keep_or_default(&mut self.cycle_headline, &CYCLE_HEADLINES, DEFAULT_CYCLE_HEADLINE);
        keep_or_default(&mut self.menu_bar_display, &MENU_BAR_DISPLAYS, DEFAULT_MENU_BAR_DISPLAY);
        keep_or_default(&mut self.menu_bar_icon, &MENU_BAR_ICONS, DEFAULT_MENU_BAR_ICON);
        keep_or_default(&mut self.menu_bar_number, &MENU_BAR_NUMBERS, DEFAULT_MENU_BAR_NUMBER);
        keep_or_default(&mut self.theme, &THEMES, DEFAULT_THEME);
        keep_or_default(&mut self.palette, &PALETTES, DEFAULT_PALETTE);
        self.allowance_override = self.allowance_override.max(0.0);
        self.text_scale = self.text_scale.clamp(0.9, 1.5);
        self.notifications.burst_aic = self.notifications.burst_aic.clamp(0.0, 10_000.0);
        self.pace_tolerance_pct = self.pace_tolerance_pct.clamp(0.0, 50.0);
        self.refresh_minutes = self.refresh_minutes.clamp(1, 240);
        self.notifications.thresholds.retain(|t| (1..=500).contains(t));
        self.notifications.thresholds.sort();
        self.notifications.thresholds.dedup();
        self
    }
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("dev.copilotbar")
}

pub fn ensure_data_dir() -> Result<PathBuf, String> {
    let dir = data_dir();
    ensure_private_dir(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(DATA_DIR_MODE).create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(DATA_DIR_MODE))
}

static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn write_private(file_name: &str, bytes: &[u8]) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = ensure_data_dir()?;
    write_atomic(&dir, file_name, bytes).map_err(|e| format!("write {file_name}: {e}"))
}

fn write_atomic(dir: &Path, file_name: &str, bytes: &[u8]) -> io::Result<()> {
    let target = dir.join(file_name);
    let temp = dir.join(format!("{file_name}.tmp"));
    let _ = fs::remove_file(&temp);
    let result = write_then_rename(&temp, &target, bytes);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_then_rename(temp: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(PRIVATE_FILE_MODE).open(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, target)
}

fn settings_path() -> PathBuf {
    data_dir().join(SETTINGS_FILE)
}

pub fn load() -> Settings {
    let path = settings_path();
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                crate::log::line(&format!("settings.json cannot be read ({e}); using defaults"));
            }
            return Settings::default();
        }
    };
    match serde_json::from_slice::<Settings>(&bytes) {
        Ok(settings) => settings.sanitized(),
        Err(e) => {
            let moved = quarantine(&path);
            crate::log::line(&format!("settings.json is unreadable ({e}); using defaults, moved aside: {moved}"));
            Settings::default()
        }
    }
}

fn quarantine(path: &Path) -> bool {
    fs::rename(path, path.with_extension("json.bad")).is_ok()
}

pub fn save(s: &Settings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    write_private(SETTINGS_FILE, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("copilotbar-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn partial_files_fill_defaults_and_bad_values_are_clamped() {
        let s: Settings = serde_json::from_str(r#"{"refreshMinutes":0,"menuBarDisplay":"nope","notifications":{"thresholds":[90,75,90,0]}}"#).unwrap();
        let s = s.sanitized();
        assert_eq!(s.refresh_minutes, 1);
        assert_eq!(s.menu_bar_display, "icon-number");
        assert_eq!(s.notifications.thresholds, vec![75, 90]);
        assert!(s.sources.cli && s.alert_colors);
        assert_eq!(s.open_expanded, None, "old files leave the new preference unset");
    }

    #[test]
    fn empty_or_unknown_menu_bar_values_fall_back_to_defaults() {
        let s = Settings { menu_bar_display: String::new(), menu_bar_icon: "star".into(), menu_bar_number: String::new(), ..Settings::default() }.sanitized();
        assert_eq!((s.menu_bar_display.as_str(), s.menu_bar_icon.as_str(), s.menu_bar_number.as_str()), ("icon-number", "disc", "pct"));
    }

    #[test]
    fn cycle_display_defaults_and_sanitizes() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!((s.cycle_summary.as_str(), s.cycle_headline.as_str()), ("gauge", "used"));
        let s: Settings = serde_json::from_str(r#"{"cycleSummary":"bar","cycleHeadline":"remaining"}"#).unwrap();
        assert_eq!((s.cycle_summary.as_str(), s.cycle_headline.as_str()), ("bar", "remaining"));
        let s = Settings { cycle_summary: "pie".into(), cycle_headline: "delta".into(), ..Settings::default() }.sanitized();
        assert_eq!((s.cycle_summary.as_str(), s.cycle_headline.as_str()), ("gauge", "used"));
    }

    #[test]
    fn removed_billing_and_price_settings_load_and_are_dropped() {
        let s: Settings = serde_json::from_str(r#"{"sources":{"cli":false,"billing":true},"usdPerCredit":0.04,"refreshMinutes":7}"#).unwrap();
        let s = s.sanitized();
        assert_eq!((s.sources.cli, s.refresh_minutes), (false, 7));
        let saved = serde_json::to_value(&s).unwrap();
        assert!(saved.get("usdPerCredit").is_none());
        assert!(saved["sources"].get("billing").is_none());
    }

    #[test]
    fn removed_overage_mode_loads_and_is_dropped() {
        let s: Settings = serde_json::from_str(r#"{"overageMode":"track","refreshMinutes":7}"#).unwrap();
        let s = s.sanitized();
        assert_eq!(s.refresh_minutes, 7);
        let saved = serde_json::to_string(&s).unwrap();
        assert!(!saved.contains("overage"), "Overage follows GitHub, not a setting: {saved}");
    }

    #[test]
    fn old_files_with_legacy_vscode_options_still_load() {
        let old = r#"{"refreshMinutes":7,"vscodeUserDirs":["~/Library/Application Support/Cursor/User"],"sources":{"cli":false,"vscode":true}}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.refresh_minutes, 7);
        assert!(!s.sources.cli);
        let saved = serde_json::to_string(&s).unwrap();
        assert!(!saved.contains("vscode"), "removed options are not written back: {saved}");
    }

    #[test]
    fn retired_keys_still_load_and_are_not_written_back() {
        let old = r#"{"monthlyBudget":450,"monthlySummary":"bar","monthlyHeadline":"remaining","popoverMode":"advanced","menuBarLabel":"text","dashboardRange":"week","refreshMinutes":7}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        let s = s.sanitized();
        assert_eq!((s.refresh_minutes, s.allowance_override, s.cycle_summary.as_str()), (7, 0.0, "gauge"));
        let saved = serde_json::to_string(&s).unwrap();
        for key in ["monthly", "popoverMode", "menuBarLabel", "dashboardRange"] {
            assert!(!saved.contains(key), "{key} is not written back: {saved}");
        }
    }

    #[test]
    fn removed_token_sources_load_and_are_dropped() {
        let s: Settings = serde_json::from_str(r#"{"tokenSources":{"copilotCli":false,"gh":true},"refreshMinutes":7}"#).unwrap();
        let s = s.sanitized();
        assert_eq!(s.refresh_minutes, 7);
        let saved = serde_json::to_string(&s).unwrap();
        assert!(!saved.contains("tokenSources"), "plan data comes through gh, not a token: {saved}");
    }

    #[test]
    fn old_files_check_for_updates_automatically_and_the_choice_round_trips() {
        let s: Settings = serde_json::from_str(r#"{"refreshMinutes":7,"tokenSources":{"gh":true}}"#).unwrap();
        let s = s.sanitized();
        assert!(s.auto_update_check, "on unless the user turns it off");
        let off = Settings { auto_update_check: false, ..s };
        let saved = serde_json::to_string(&off).unwrap();
        assert!(saved.contains(r#""autoUpdateCheck":false"#), "{saved}");
        let back: Settings = serde_json::from_str(&saved).unwrap();
        assert!(!back.auto_update_check);
    }

    #[test]
    fn open_expanded_round_trips() {
        let s = Settings { open_expanded: Some(true), ..Settings::default() };
        let back: Settings = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(back.open_expanded, Some(true));
    }

    #[test]
    fn writes_replace_the_file_in_one_step_with_private_mode() {
        let dir = scratch_dir("write");
        write_atomic(&dir, "state.json", b"one").unwrap();
        write_atomic(&dir, "state.json", b"two").unwrap();
        assert_eq!(fs::read(dir.join("state.json")).unwrap(), b"two");
        assert!(!dir.join("state.json.tmp").exists());
        let mode = fs::metadata(dir.join("state.json")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_the_previous_file_intact() {
        let dir = scratch_dir("failed-write");
        write_atomic(&dir, "state.json", b"kept").unwrap();
        fs::create_dir(dir.join("state.json.tmp")).unwrap();
        assert!(write_atomic(&dir, "state.json", b"new").is_err());
        assert_eq!(fs::read(dir.join("state.json")).unwrap(), b"kept");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn private_dirs_are_owner_only() {
        let dir = scratch_dir("dir").join("nested");
        ensure_private_dir(&dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_corrupt_settings_file_is_moved_aside() {
        let dir = scratch_dir("corrupt");
        let path = dir.join("settings.json");
        fs::write(&path, "{not json").unwrap();
        assert!(quarantine(&path));
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(dir.join("settings.json.bad")).unwrap(), "{not json");
        fs::remove_dir_all(&dir).unwrap();
    }
}
