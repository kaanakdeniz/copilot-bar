pub mod core;
mod log;
mod notify;
mod tray;

use crate::core::collector::{Collector, PlanFetch, Snapshot};
use crate::core::settings::{self, Settings};
use crate::core::snapshot::SignIn;
use crate::core::updater::{self, Trigger, UpdateState, Updater};
use std::fmt::Display;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::async_runtime::Mutex as AsyncMutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

const TRAY_ID: &str = "copilot";
const POPOVER: &str = "popover";
const DASHBOARD: &str = "dashboard";
const SECONDS_PER_MINUTE: u64 = 60;
const LOCAL_REREAD_EVERY: Duration = Duration::from_secs(60);
const REOPEN_GUARD: Duration = Duration::from_millis(250);
const ONBOARDING_DELAY: Duration = Duration::from_millis(1200);
const SNAPSHOT_POLL: Duration = Duration::from_millis(30);
const SNAPSHOT_POLLS: u32 = 100;
const DEFAULTS_BIN: &str = "/usr/bin/defaults";
const POPOVER_GAP: f64 = 6.0;
const SCREEN_EDGE_MARGIN: f64 = 8.0;
const MENU_BAR_HEIGHT_FALLBACK: f64 = 40.0;
const POPOVER_MIN_WIDTH: f64 = 260.0;
const POPOVER_MAX_WIDTH: f64 = 600.0;
const POPOVER_MIN_HEIGHT: f64 = 120.0;
const POPOVER_MIN_MAX_HEIGHT: f64 = 240.0;
const POPOVER_FALLBACK_MAX_HEIGHT: f64 = 900.0;
const POPOVER_BOTTOM_INSET: f64 = 16.0;
const WIDTH_MATCH_TOLERANCE: f64 = 0.5;
const DASHBOARD_WIDTH: f64 = 1120.0;
const DASHBOARD_HEIGHT: f64 = 780.0;
const DASHBOARD_MIN_WIDTH: f64 = 820.0;
const DASHBOARD_MIN_HEIGHT: f64 = 580.0;
const DASHBOARD_TABS: [&str; 4] = ["overview", "models", "projects", "settings"];
const DEFAULT_DASHBOARD_TAB: &str = "overview";
const AUTOSTART_PATH_MARKER: &str = "autostart-path";
const EXPORT_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const EXPORT_NAME_ATTEMPTS: u32 = 1000;

pub struct AppState {
    collector: AsyncMutex<Collector>,
    settings: Mutex<Settings>,
    last: Mutex<Option<Arc<Snapshot>>>,
    tray_rect: Mutex<Option<(PhysicalPosition<f64>, PhysicalSize<f64>)>>,
    last_hidden: Mutex<Option<Instant>>,
    wake: tokio::sync::Notify,
    updater: Updater,
    update: Mutex<UpdateState>,
    last_announced_version: Mutex<Option<String>>,
    notify_gate: tokio::sync::Mutex<()>,
    dark_mode: AtomicBool,
}

impl AppState {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn last(&self) -> Option<Arc<Snapshot>> {
        self.last.lock().unwrap().clone()
    }
}

fn log_failure<T, E: Display>(what: &str, result: Result<T, E>) {
    if let Err(e) = result {
        log::line(&format!("{what} failed: {e}"));
    }
}

struct LocalRead {
    files_changed: bool,
    snapshot: Arc<Snapshot>,
    plan: PlanFetch,
}

async fn read_local(app: &AppHandle, settings: &Settings, full: bool) -> LocalRead {
    let st = app.state::<AppState>();
    let mut c = st.collector.lock().await;
    let files_changed = c.refresh_local(settings);
    if full {
        c.begin_connecting();
    }
    LocalRead { files_changed, snapshot: Arc::new(c.snapshot(settings)), plan: c.plan_fetch() }
}

async fn notify_once(app: &AppHandle, snap: &Snapshot) {
    let st = app.state::<AppState>();
    let _single_flight = st.notify_gate.lock().await;
    notify::check(app, snap);
}

async fn do_refresh(app: &AppHandle, full: bool) -> Arc<Snapshot> {
    let settings = app.state::<AppState>().settings();
    let local = read_local(app, &settings, full).await;
    publish(app, local.snapshot.clone());
    if !full {
        notify_once(app, &local.snapshot).await;
        return local.snapshot;
    }
    let outcome = local.plan.run().await;
    let snap = {
        let st = app.state::<AppState>();
        let mut c = st.collector.lock().await;
        c.apply_plan(outcome);
        Arc::new(c.snapshot(&settings))
    };
    log::line(&format!(
        "refresh: {} records, used {:.2} ({:.2} unlogged)/{:?} AIC, sign-in {}, error {:?}",
        snap.records.len(),
        snap.pace.used,
        snap.pace.unlogged,
        snap.pace.allowance,
        sign_in_state(&snap.status.sign_in),
        snap.status.plan_error
    ));
    publish(app, snap.clone());
    notify_once(app, &snap).await;
    snap
}

fn should_publish(last: Option<&Snapshot>, next: &Snapshot, files_changed: bool) -> bool {
    match last {
        None => true,
        Some(last) => {
            files_changed || last.settings != next.settings || last.github_plan != next.github_plan || last.pace.status != next.pace.status
        }
    }
}

async fn local_tick(app: &AppHandle) {
    let settings = app.state::<AppState>().settings();
    let local = read_local(app, &settings, false).await;
    if should_publish(app.state::<AppState>().last().as_deref(), &local.snapshot, local.files_changed) {
        publish(app, local.snapshot.clone());
    }
    notify_once(app, &local.snapshot).await;
}

fn publish(app: &AppHandle, snap: Arc<Snapshot>) {
    let st = app.state::<AppState>();
    *st.last.lock().unwrap() = Some(snap.clone());
    update_tray(app, &snap);
    log_failure("emit snapshot", app.emit("snapshot", &*snap));
}

fn update_tray(app: &AppHandle, snap: &Snapshot) {
    let Some(t) = app.tray_by_id(TRAY_ID) else { return };
    let dark = app.state::<AppState>().dark_mode.load(Ordering::Relaxed);
    let l = tray::look(&snap.settings, &snap.pace, dark);
    log_failure("set tray icon", t.set_icon(l.icon.map(|i| tauri::image::Image::new_owned(i.rgba, i.width, i.height))));
    log_failure("set tray template", t.set_icon_as_template(l.template));
    log_failure("set tray title", t.set_title(l.title.as_deref()));
    log_failure("set tray tooltip", t.set_tooltip(Some(&l.tooltip)));
}

fn system_dark_mode() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new(DEFAULTS_BIN)
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("dark"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn refresh_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            do_refresh(&app, true).await;
            let minutes = app.state::<AppState>().settings().refresh_minutes as u64;
            let deadline = Instant::now() + Duration::from_secs(minutes * SECONDS_PER_MINUTE);
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                let st = app.state::<AppState>();
                let woken = st.wake.notified();
                tokio::select! {
                    _ = woken => break,
                    _ = tokio::time::sleep(left.min(LOCAL_REREAD_EVERY)) => {
                        if Instant::now() < deadline { local_tick(&app).await; }
                    }
                }
            }
        }
    });
}

fn checking_state(previous: &UpdateState, trigger: Trigger) -> Option<UpdateState> {
    match (previous, trigger) {
        (UpdateState::Available { .. }, Trigger::Scheduled) => None,
        _ => Some(UpdateState::Checking),
    }
}

fn settle_check(previous: &UpdateState, trigger: Trigger, result: UpdateState) -> UpdateState {
    let scheduled_failure_keeps_update = matches!(previous, UpdateState::Available { .. })
        && trigger == Trigger::Scheduled
        && matches!(result, UpdateState::Error { .. });
    if scheduled_failure_keeps_update {
        previous.clone()
    } else {
        result
    }
}

async fn check_updates(app: &AppHandle, trigger: Trigger) -> UpdateState {
    let st = app.state::<AppState>();
    let settings = st.settings();
    let (previous, started) = {
        let mut u = st.update.lock().unwrap();
        if u.is_busy() || !updater::runs(trigger, &settings) {
            return u.clone();
        }
        let previous = u.clone();
        let started = checking_state(&previous, trigger);
        if let Some(state) = &started {
            *u = state.clone();
        }
        (previous, started)
    };
    if let Some(state) = started {
        publish_update(app, state);
    }
    let Some(checked) = st.updater.check(trigger, &settings).await else { return st.update.lock().unwrap().clone() };
    log::line(&format!("update check ({trigger:?}): {checked:?}"));
    let next = settle_check(&previous, trigger, checked);
    let note = {
        let mut announced = st.last_announced_version.lock().unwrap();
        let note = updater::announcement(announced.as_deref(), &next);
        if let UpdateState::Available { version, .. } = &next {
            *announced = Some(version.clone());
        }
        note
    };
    if let Some((title, body)) = note {
        notify::show(app, &title, &body);
    }
    publish_update(app, next.clone());
    next
}

fn publish_update(app: &AppHandle, state: UpdateState) {
    *app.state::<AppState>().update.lock().unwrap() = state.clone();
    if let (Some(t), Ok(menu)) = (app.tray_by_id(TRAY_ID), tray_menu(app, &state)) {
        log_failure("set tray menu", t.set_menu(Some(menu)));
    }
    log_failure("emit update", app.emit("update", &state));
}

fn tray_menu(app: &AppHandle, update: &UpdateState) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    for entry in tray::menu(update) {
        match entry {
            tray::Entry::Item { id, label, accelerator, enabled } => menu.append(&MenuItem::with_id(app, id, label, enabled, accelerator)?)?,
            tray::Entry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

fn take_update(app: &AppHandle) {
    let state = app.state::<AppState>().update.lock().unwrap().clone();
    match state {
        UpdateState::Available { version, .. } => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { install_update(&app, &version).await });
        }
        UpdateState::Error { url: Some(url), .. } => {
            use tauri_plugin_opener::OpenerExt;
            let _ = app.opener().open_url(url, None::<&str>);
        }
        _ => {}
    }
}

async fn install_update(app: &AppHandle, version: &str) {
    let st = app.state::<AppState>();
    {
        let mut u = st.update.lock().unwrap();
        if !matches!(&*u, UpdateState::Available { version: v, .. } if v == version) {
            return;
        }
        *u = UpdateState::Downloading { version: version.into() };
    }
    publish_update(app, UpdateState::Downloading { version: version.into() });
    let result = async {
        let running = running_bundle().ok_or("This copy of CopilotBar isn't running from an app bundle, so it can't update itself.")?;
        let user_applications = dirs::home_dir().ok_or("Can't find your home folder.")?.join("Applications");
        let verified = st.updater.download(version).await?;
        publish_update(app, UpdateState::Installing { version: version.into() });
        let installed = verified.install(&running, &user_applications).await?;
        Ok::<_, String>((running, installed))
    }
    .await;
    match result {
        Ok((running, installed)) => {
            log::line(&format!("update: installed {version} at {}", installed.bundle.display()));
            if installed.fell_back {
                let folder = running.parent().map_or_else(String::new, |p| p.display().to_string());
                notify::show(
                    app,
                    &format!("CopilotBar {version} is in ~/Applications"),
                    &format!("{folder} isn't writable, so the update went to your own Applications folder. The old copy is still in {folder}."),
                );
            }
            if let Err(e) = relaunch(&installed.bundle) {
                publish_update(app, st.updater.failed(format!("CopilotBar {version} is installed, but couldn't relaunch ({e}). Quit and open it again.")));
                return;
            }
            app.exit(0);
        }
        Err(message) => {
            log::line(&format!("update: installing {version} failed: {message}"));
            publish_update(app, st.updater.failed(message));
        }
    }
}

fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

fn relaunch(bundle: &Path) -> std::io::Result<()> {
    let script = r#"while /bin/kill -0 "$1" 2>/dev/null; do /bin/sleep 0.2; done; exec /usr/bin/open "$2""#;
    std::process::Command::new("/bin/sh")
        .args(["-c", script, "copilotbar-relaunch", &std::process::id().to_string()])
        .arg(bundle)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

fn reconcile_autostart(app: &AppHandle) {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy().into_owned();
    let marker = settings::data_dir().join(AUTOSTART_PATH_MARKER);
    if std::fs::read_to_string(&marker).is_ok_and(|recorded| recorded == exe) {
        return;
    }
    let manager = app.autolaunch();
    let reregistered = match manager.is_enabled() {
        Ok(true) => manager.disable().and_then(|()| manager.enable()),
        Ok(false) => Ok(()),
        Err(e) => Err(e),
    };
    match reregistered {
        Ok(()) => {
            log_failure("record autostart path", std::fs::create_dir_all(settings::data_dir()).and_then(|()| std::fs::write(&marker, &exe)));
        }
        Err(e) => log::line(&format!("autostart re-registration for {exe} failed: {e}")),
    }
}

fn update_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            check_updates(&app, Trigger::Scheduled).await;
            tokio::time::sleep(updater::CHECK_EVERY).await;
        }
    });
}

fn tray_anchor(app: &AppHandle, scale: f64) -> Option<(PhysicalPosition<f64>, PhysicalSize<f64>)> {
    let live: Option<(PhysicalPosition<f64>, PhysicalSize<f64>)> = app
        .tray_by_id(TRAY_ID)
        .and_then(|t| t.rect().ok().flatten())
        .map(|r| (r.position.to_physical(scale), r.size.to_physical(scale)));
    live.or_else(|| *app.state::<AppState>().tray_rect.lock().unwrap())
}

fn position_popover(app: &AppHandle) {
    let Some(w) = app.get_webview_window(POPOVER) else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    let Some((pos, size)) = tray_anchor(app, scale) else { return };
    let Ok(win) = w.outer_size() else { return };
    let gap = POPOVER_GAP * scale;
    let monitor = w
        .available_monitors()
        .ok()
        .and_then(|ms| {
            ms.into_iter().find(|m| {
                let (p, s) = (m.position(), m.size());
                pos.x >= p.x as f64 && pos.x < (p.x + s.width as i32) as f64 && pos.y >= p.y as f64 && pos.y < (p.y + s.height as i32) as f64
            })
        })
        .or_else(|| w.current_monitor().ok().flatten());
    let mut x = pos.x + size.width / 2.0 - win.width as f64 / 2.0;
    let mut y = pos.y + size.height + gap;
    if let Some(m) = monitor {
        let (mp, ms) = (m.position(), m.size());
        if pos.y > mp.y as f64 + ms.height as f64 / 2.0 {
            y = pos.y - win.height as f64 - gap;
        }
        let margin = SCREEN_EDGE_MARGIN * scale;
        x = x.clamp(mp.x as f64 + margin, (mp.x + ms.width as i32) as f64 - win.width as f64 - margin);
    }
    log_failure("position popover", w.set_position(PhysicalPosition::new(x.round(), y.round())));
}

fn toggle_popover(app: &AppHandle) {
    let Some(w) = app.get_webview_window(POPOVER) else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    if let Some(t) = *app.state::<AppState>().last_hidden.lock().unwrap() {
        if t.elapsed() < REOPEN_GUARD {
            return;
        }
    }
    position_popover(app);
    let _ = w.show();
    let _ = w.set_focus();
    log_failure("emit popover-shown", app.emit_to(POPOVER, "popover-shown", ()));
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        do_refresh(&app2, false).await;
    });
}

#[cfg(all(target_os = "macos", debug_assertions))]
fn set_dev_dock_icon() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let data = NSData::with_bytes(include_bytes!("../icons/icon.png"));
    if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: `mtm` proves this runs on the main thread, as AppKit requires; set_dev_dock_icon runs from Tauri's setup.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&img)) };
    }
}

fn known_dashboard_tab(tab: &str) -> &'static str {
    DASHBOARD_TABS.into_iter().find(|known| *known == tab).unwrap_or(DEFAULT_DASHBOARD_TAB)
}

fn open_dashboard_window(app: &AppHandle, tab: Option<String>) -> tauri::Result<()> {
    let tab = tab.as_deref().map(known_dashboard_tab);
    if let Some(p) = app.get_webview_window(POPOVER) {
        let _ = p.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(w) = app.get_webview_window(DASHBOARD) {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        if let Some(tab) = tab {
            log_failure("emit open-tab", app.emit_to(DASHBOARD, "open-tab", tab));
        }
        return Ok(());
    }
    let url = format!("index.html?view=dashboard&tab={}", tab.unwrap_or(DEFAULT_DASHBOARD_TAB));
    let w = WebviewWindowBuilder::new(app, DASHBOARD, WebviewUrl::App(url.into()))
        .title("CopilotBar")
        .inner_size(DASHBOARD_WIDTH, DASHBOARD_HEIGHT)
        .min_inner_size(DASHBOARD_MIN_WIDTH, DASHBOARD_MIN_HEIGHT)
        .center()
        .build()?;
    #[cfg(target_os = "macos")]
    {
        let app2 = app.clone();
        w.on_window_event(move |e| {
            if let WindowEvent::Destroyed = e {
                log_failure("set activation policy", app2.set_activation_policy(tauri::ActivationPolicy::Accessory));
            }
        });
    }
    w.set_focus()?;
    Ok(())
}

#[tauri::command]
async fn get_snapshot(app: AppHandle) -> Snapshot {
    for _ in 0..SNAPSHOT_POLLS {
        if let Some(s) = app.state::<AppState>().last() {
            return (*s).clone();
        }
        tokio::time::sleep(SNAPSHOT_POLL).await;
    }
    let st = app.state::<AppState>();
    let settings = st.settings();
    let snap = match st.collector.try_lock() {
        Ok(c) => c.snapshot(&settings),
        Err(_) => Collector::default().snapshot(&settings),
    };
    snap
}

#[tauri::command]
async fn refresh(app: AppHandle, full: Option<bool>) -> Snapshot {
    let snap = do_refresh(&app, full.unwrap_or(true)).await;
    (*snap).clone()
}

#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let settings = settings.sanitized();
    settings::save(&settings)?;
    let st = app.state::<AppState>();
    let old = std::mem::replace(&mut *st.settings.lock().unwrap(), settings.clone());
    let needs_network = old.sources != settings.sources || old.cli_log_dir != settings.cli_log_dir;
    if old.refresh_minutes != settings.refresh_minutes {
        st.wake.notify_one();
    }
    if old.hotkey != settings.hotkey {
        apply_hotkey(&app, &settings.hotkey);
    }
    do_refresh(&app, needs_network).await;
    Ok(())
}

#[tauri::command]
fn get_update_state(app: AppHandle) -> UpdateState {
    app.state::<AppState>().update.lock().unwrap().clone()
}

#[tauri::command]
async fn check_for_updates(app: AppHandle) -> UpdateState {
    check_updates(&app, Trigger::Manual).await
}

#[tauri::command]
fn open_update(app: AppHandle) {
    take_update(&app);
}

#[tauri::command]
fn default_settings() -> Settings {
    Settings::default()
}

#[tauri::command]
fn resize_popover(app: AppHandle, width: f64, height: f64) -> f64 {
    let Some(w) = app.get_webview_window(POPOVER) else { return height };
    let scale = w.scale_factor().unwrap_or(1.0);
    let max_h = max_popover_height(&app, &w, scale);
    let width = width.clamp(POPOVER_MIN_WIDTH, POPOVER_MAX_WIDTH);
    let applied = height.clamp(POPOVER_MIN_HEIGHT, max_h);
    let same_width = w.outer_size().map(|s| (s.width as f64 / scale - width).abs() < WIDTH_MATCH_TOLERANCE).unwrap_or(false);
    if same_width && set_popover_height(&w, applied) {
        return applied;
    }
    log_failure("resize popover", w.set_size(LogicalSize::new(width, applied)));
    position_popover(&app);
    applied
}

fn max_popover_height(app: &AppHandle, w: &tauri::WebviewWindow, scale: f64) -> f64 {
    w.current_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let bottom = (m.position().y + m.size().height as i32) as f64;
            let top = app
                .state::<AppState>()
                .tray_rect
                .lock()
                .unwrap()
                .map(|(p, s)| p.y + s.height)
                .unwrap_or(m.position().y as f64 + MENU_BAR_HEIGHT_FALLBACK * scale);
            (bottom - top) / scale - POPOVER_BOTTOM_INSET
        })
        .unwrap_or(POPOVER_FALLBACK_MAX_HEIGHT)
        .max(POPOVER_MIN_MAX_HEIGHT)
}

fn set_popover_height(w: &tauri::WebviewWindow, height: f64) -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWindow;
        use objc2_foundation::{NSPoint, NSRect, NSSize};
        if objc2::MainThreadMarker::new().is_none() {
            return false;
        }
        let Ok(ptr) = w.ns_window() else { return false };
        // SAFETY: Tauri keeps the NSWindow alive for as long as the webview window exists, and we are on the main thread.
        let ns = unsafe { &*(ptr as *const NSWindow) };
        let old = ns.frame();
        // Cocoa's origin is bottom-left, so moving it by the height change keeps the top edge fixed.
        let frame = NSRect::new(
            NSPoint::new(old.origin.x, old.origin.y + old.size.height - height),
            NSSize::new(old.size.width, height),
        );
        ns.setFrame_display(frame, true);
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (w, height);
        false
    }
}

#[tauri::command]
fn hide_popover(app: AppHandle) {
    if let Some(w) = app.get_webview_window(POPOVER) {
        let _ = w.hide();
    }
}

#[tauri::command]
fn open_dashboard(app: AppHandle, tab: Option<String>) -> Result<(), String> {
    open_dashboard_window(&app, tab).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_csv(app: AppHandle, name: String, contents: String) -> Result<String, String> {
    let dir = dirs::download_dir().or_else(dirs::home_dir).ok_or("no Downloads folder")?;
    let saved = write_export(&dir, &name, &contents).inspect_err(|e| log::line(&format!("export failed: {e}")))?;
    log_failure("reveal export", tauri_plugin_opener::OpenerExt::opener(&app).reveal_item_in_dir(&saved));
    Ok(file_name_of(&saved))
}

fn write_export(dir: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
    check_export_size(contents.len())?;
    let file_name = export_file_name(name).ok_or("The export needs a name that starts with a letter or a digit.")?;
    let (path, mut file) = create_export_file(dir, &file_name).map_err(|e| e.to_string())?;
    if let Err(e) = file.write_all(contents.as_bytes()) {
        let _ = std::fs::remove_file(&path);
        return Err(e.to_string());
    }
    Ok(path)
}

fn file_name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn check_export_size(len: usize) -> Result<(), String> {
    if len > EXPORT_LIMIT_BYTES {
        Err("The export is too large to save.".into())
    } else {
        Ok(())
    }
}

fn export_file_name(name: &str) -> Option<String> {
    let safe: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || "-_.".contains(*c)).collect();
    if !safe.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(if safe.ends_with(".csv") { safe } else { format!("{safe}.csv") })
}

fn create_export_file(dir: &Path, file_name: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    let stem = file_name.strip_suffix(".csv").unwrap_or(file_name);
    for attempt in 0..EXPORT_NAME_ATTEMPTS {
        let name = if attempt == 0 { file_name.to_string() } else { format!("{stem} ({attempt}).csv") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no free file name for the export"))
}

fn apply_hotkey(app: &AppHandle, hotkey: &str) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    log_failure("unregister hotkeys", gs.unregister_all());
    if hotkey.trim().is_empty() {
        return;
    }
    if let Err(e) = gs.register(hotkey.trim()) {
        log::line(&format!("hotkey {hotkey:?} not registered: {e}"));
    }
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn system_theme_is_dark(theme: &Theme) -> bool {
    matches!(theme, Theme::Dark)
}

pub fn run() {
    let settings = settings::load();
    let dark = system_dark_mode();
    let context = tauri::generate_context!();
    let version = context.package_info().version.to_string();
    log::line(&format!("CopilotBar {version} starting, releases from {:?}", env!("COPILOTBAR_REPOSITORY")));
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| toggle_popover(app)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_popover(app);
                    }
                })
                .build(),
        )
        .manage(AppState {
            collector: AsyncMutex::new(Collector::default()),
            settings: Mutex::new(settings),
            last: Mutex::new(None),
            tray_rect: Mutex::new(None),
            last_hidden: Mutex::new(None),
            wake: tokio::sync::Notify::new(),
            updater: Updater::new(crate::core::gh::Gh::installed(), Some(env!("COPILOTBAR_REPOSITORY")).filter(|r| !r.is_empty()), &version),
            update: Mutex::new(UpdateState::Idle),
            last_announced_version: Mutex::new(None),
            notify_gate: tokio::sync::Mutex::new(()),
            dark_mode: AtomicBool::new(dark),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh,
            save_settings,
            default_settings,
            get_update_state,
            check_for_updates,
            open_update,
            resize_popover,
            hide_popover,
            open_dashboard,
            export_csv,
            quit
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            #[cfg(all(target_os = "macos", debug_assertions))]
            set_dev_dock_icon();

            let popover = app.get_webview_window(POPOVER).expect("popover window");
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                let _ = apply_vibrancy(&popover, NSVisualEffectMaterial::Popover, Some(NSVisualEffectState::Active), Some(12.0));
            }
            let handle = app.handle().clone();
            popover.on_window_event(move |e| match e {
                WindowEvent::Focused(false) => {
                    if let Some(w) = handle.get_webview_window(POPOVER) {
                        if w.is_visible().unwrap_or(false) {
                            let _ = w.hide();
                            *handle.state::<AppState>().last_hidden.lock().unwrap() = Some(Instant::now());
                        }
                    }
                }
                WindowEvent::ThemeChanged(theme) => {
                    let st = handle.state::<AppState>();
                    st.dark_mode.store(system_theme_is_dark(theme), Ordering::Relaxed);
                    if let Some(s) = st.last() {
                        update_tray(&handle, &s);
                    }
                }
                _ => {}
            });

            let menu = tray_menu(app.handle(), &UpdateState::Idle)?;
            let s0 = app.state::<AppState>().settings();
            let dark = app.state::<AppState>().dark_mode.load(Ordering::Relaxed);
            let initial_pace = Collector::default().snapshot(&s0).pace;
            let initial = tray::look(&s0, &initial_pace, dark);
            let mut tray = TrayIconBuilder::with_id(TRAY_ID);
            if let Some(i) = initial.icon {
                tray = tray.icon(tauri::image::Image::new_owned(i.rgba, i.width, i.height)).icon_as_template(initial.template);
            }
            tray
                .tooltip("Copilot usage")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id().as_ref() {
                    "dashboard" => {
                        let _ = open_dashboard_window(app, None);
                    }
                    "settings" => {
                        let _ = open_dashboard_window(app, Some("settings".into()));
                    }
                    "refresh" => {
                        let a = app.clone();
                        tauri::async_runtime::spawn(async move {
                            do_refresh(&a, true).await;
                        });
                    }
                    "check-updates" => {
                        let a = app.clone();
                        tauri::async_runtime::spawn(async move {
                            check_updates(&a, Trigger::Manual).await;
                        });
                    }
                    "update" => take_update(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { rect, button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        let app = tray.app_handle();
                        let scale = app.get_webview_window(POPOVER).and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
                        *app.state::<AppState>().tray_rect.lock().unwrap() =
                            Some((rect.position.to_physical(scale), rect.size.to_physical(scale)));
                        toggle_popover(app);
                    }
                })
                .build(app)?;

            apply_hotkey(app.handle(), &s0.hotkey);
            reconcile_autostart(app.handle());
            refresh_loop(app.handle().clone());
            update_loop(app.handle().clone());
            if !s0.onboarded {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(ONBOARDING_DELAY).await;
                    toggle_popover(&h);
                });
            }
            Ok(())
        })
        .run(context)
        .expect("error while running CopilotBar");
}

fn sign_in_state(s: &SignIn) -> &'static str {
    match s {
        SignIn::Unknown => "unknown",
        SignIn::SignedIn { .. } => "signed-in",
        SignIn::GhMissing => "gh-missing",
        SignIn::SignedOut => "signed-out",
        SignIn::SsoRequired => "sso-required",
        SignIn::AccountMismatch { .. } => "account-mismatch",
        SignIn::Error { .. } => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::snapshot::build;
    use std::sync::atomic::AtomicUsize;

    fn snapshot_with(settings: &Settings) -> Snapshot {
        build(vec![], None, settings, Default::default(), 0)
    }

    fn scratch_dir(label: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("copilotbar-{label}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_tick_publishes_only_when_something_it_shows_changed() {
        let base = snapshot_with(&Settings::default());
        assert!(should_publish(None, &base, false), "nothing published yet");
        assert!(!should_publish(Some(&base), &base, false), "nothing changed");
        assert!(should_publish(Some(&base), &base, true), "a session log changed");
        let other = snapshot_with(&Settings { auto_update_check: false, ..Settings::default() });
        assert!(should_publish(Some(&base), &other, false), "a settings change");
    }

    #[test]
    fn a_scheduled_check_keeps_an_available_update_when_it_fails() {
        let available = UpdateState::Available { version: "1.3.0".into(), url: "https://example.test/v1.3.0".into() };
        let failed = UpdateState::Error { message: "offline".into(), url: None };
        assert_eq!(checking_state(&available, Trigger::Scheduled), None, "no Checking replaces the update");
        assert_eq!(settle_check(&available, Trigger::Scheduled, failed.clone()), available);
        assert_eq!(settle_check(&available, Trigger::Scheduled, UpdateState::UpToDate), UpdateState::UpToDate, "a successful check still decides");
        assert_eq!(checking_state(&available, Trigger::Manual), Some(UpdateState::Checking));
        assert_eq!(settle_check(&available, Trigger::Manual, failed.clone()), failed, "a manual check shows its error");
        assert_eq!(checking_state(&UpdateState::UpToDate, Trigger::Scheduled), Some(UpdateState::Checking));
        assert_eq!(settle_check(&UpdateState::UpToDate, Trigger::Scheduled, failed.clone()), failed);
    }

    #[test]
    fn dashboard_tabs_are_limited_to_the_known_ones() {
        assert_eq!(known_dashboard_tab("models"), "models");
        assert_eq!(known_dashboard_tab("settings"), "settings");
        assert_eq!(known_dashboard_tab("overview"), "overview");
        assert_eq!(known_dashboard_tab("models&studio=1"), "overview");
        assert_eq!(known_dashboard_tab(""), "overview");
    }

    #[test]
    fn export_names_drop_separators_and_need_a_leading_alphanumeric() {
        assert_eq!(export_file_name("copilot-usage-2026-11-27").as_deref(), Some("copilot-usage-2026-11-27.csv"));
        assert_eq!(export_file_name("report.csv").as_deref(), Some("report.csv"));
        assert_eq!(export_file_name("/etc/passwd").as_deref(), Some("etcpasswd.csv"));
        assert_eq!(export_file_name(".csv"), None);
        assert_eq!(export_file_name("..secret"), None);
        assert_eq!(export_file_name(""), None);
    }

    #[test]
    fn an_export_reports_the_name_it_was_saved_under() {
        let dir = scratch_dir("export-name");
        let first = write_export(&dir, "usage", "a,b").unwrap();
        let second = write_export(&dir, "usage", "c").unwrap();
        assert_eq!(file_name_of(&first), "usage.csv");
        assert_eq!(file_name_of(&second), "usage (1).csv");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "c");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exports_are_capped_at_64_mib() {
        assert!(check_export_size(EXPORT_LIMIT_BYTES).is_ok());
        assert_eq!(check_export_size(EXPORT_LIMIT_BYTES + 1), Err("The export is too large to save.".into()));
    }

    #[test]
    fn a_colliding_export_gets_a_numbered_name_and_never_overwrites() {
        let dir = scratch_dir("export-collide");
        let (first, _) = create_export_file(&dir, "usage.csv").unwrap();
        let (second, _) = create_export_file(&dir, "usage.csv").unwrap();
        let (third, _) = create_export_file(&dir, "usage.csv").unwrap();
        assert_eq!(first, dir.join("usage.csv"));
        assert_eq!(second, dir.join("usage (1).csv"));
        assert_eq!(third, dir.join("usage (2).csv"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn an_export_never_follows_a_symlink_at_its_name() {
        let dir = scratch_dir("export-symlink");
        let target = dir.join("target.txt");
        std::fs::write(&target, "keep me").unwrap();
        std::os::unix::fs::symlink(&target, dir.join("usage.csv")).unwrap();
        let (path, _) = create_export_file(&dir, "usage.csv").unwrap();
        assert_eq!(path, dir.join("usage (1).csv"));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

