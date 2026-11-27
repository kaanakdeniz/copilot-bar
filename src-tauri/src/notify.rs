use crate::core::collector::Snapshot;
use crate::core::format::{fmt_aic, fmt_days};
use crate::core::pace::{Pace, Status};
use crate::core::settings::{data_dir, write_private, Notifications};
use jiff::{tz::TimeZone, Timestamp};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, PoisonError};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

const NOTIFIED_FILE: &str = "notified.json";
const BURST_COOLDOWN_MS: i64 = 60 * 60 * 1000;
const PACE_ALERT_AFTER_DAYS: f64 = 3.0;
const FULL_ALLOWANCE_PCT: u32 = 100;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fired {
    cycle_start: i64,
    thresholds: Vec<u32>,
    ahead: bool,
    burst_at: Option<i64>,
}

static FIRED: Mutex<Option<Fired>> = Mutex::new(None);

pub fn show(app: &AppHandle, title: &str, body: &str) -> bool {
    app.notification().builder().title(title).body(body).show().is_ok()
}

fn day_label(epoch_ms: i64, tz: TimeZone) -> Option<String> {
    Some(Timestamp::from_millisecond(epoch_ms).ok()?.to_zoned_datetime(tz).strftime("%b %-d").to_string())
}

fn threshold_alert(snap: &Snapshot, top: u32) -> (String, String) {
    let p = &snap.pace;
    let title = format!("Copilot: {top}% of your Allowance used");
    if top < FULL_ALLOWANCE_PCT {
        let body = format!(
            "{} of your {} AIC Allowance used. The Cycle resets in {}.",
            fmt_aic(p.used),
            fmt_aic(p.allowance.unwrap_or(0.0)),
            fmt_days(p.remaining_days.ceil() as i64)
        );
        return (title, body);
    }
    let reset = day_label(p.cycle.end, TimeZone::UTC).unwrap_or_default();
    let body = if snap.copilot_stopped {
        format!("You've used your whole Entitlement. Overage isn't permitted for your plan, so Copilot has stopped until it resets on {reset}.")
    } else if snap.entitlement_used_up {
        format!("You've used your whole Entitlement. Further usage is billed as Overage until it resets on {reset}.")
    } else if snap.github_plan.is_some() {
        format!("You've used your whole Allowance. Copilot keeps working; your Allowance resets on {reset}.")
    } else {
        format!("You've used your whole Allowance. It resets on {reset}.")
    };
    (title, body)
}

fn pace_alert(p: &Pace) -> (String, String) {
    let when = p.run_out_at.and_then(|ms| day_label(ms, TimeZone::system()));
    let body = match (when, p.daily_budget) {
        (Some(w), Some(daily)) => format!("At {} AIC a day your Allowance runs out on {w}. Today's Daily Budget is {} AIC.", fmt_aic(p.avg_daily), fmt_aic(daily)),
        _ => "You're using AIC faster than Even Pace.".into(),
    };
    ("Copilot: Ahead of Even Pace".into(), body)
}

fn burst_alert(last_hour: f64) -> (String, String) {
    (
        format!("Copilot: {} AIC in the last hour", fmt_aic(last_hour)),
        "That's a Burst. Check the Projects tab in the dashboard for a Runaway Session.".into(),
    )
}

fn cooldown_over(burst_at: Option<i64>, now: i64) -> bool {
    match burst_at {
        None => true,
        Some(at) => now - at > BURST_COOLDOWN_MS,
    }
}

fn burst_due(fired: &Fired, n: &Notifications, pace: &Pace, now: i64) -> bool {
    n.enabled && n.burst_aic > 0.0 && pace.last_hour >= n.burst_aic && cooldown_over(fired.burst_at, now)
}

fn allowance_alerts(snap: &Snapshot, fired: &mut Fired) -> Vec<(String, String)> {
    let n = &snap.settings.notifications;
    let p = &snap.pace;
    let mut out = vec![];
    let pct = p.frac_used.unwrap_or(0.0) * 100.0;
    let crossed: Vec<u32> = n.thresholds.iter().copied().filter(|t| pct >= f64::from(*t) && !fired.thresholds.contains(t)).collect();
    if let Some(&top) = crossed.iter().max() {
        out.push(threshold_alert(snap, top));
        fired.thresholds.extend(crossed);
    }
    if n.ahead_of_pace && p.status == Status::Ahead && !fired.ahead && p.elapsed_days >= PACE_ALERT_AFTER_DAYS {
        out.push(pace_alert(p));
        fired.ahead = true;
    }
    out
}

fn due(snap: &Snapshot, fired: &mut Fired) -> Vec<(String, String)> {
    let n = &snap.settings.notifications;
    let p = &snap.pace;
    if fired.cycle_start != p.cycle.start {
        fired.cycle_start = p.cycle.start;
        fired.thresholds.clear();
        fired.ahead = false;
    }
    let mut out = vec![];
    if n.enabled && p.allowance.is_some() {
        out.extend(allowance_alerts(snap, fired));
    }
    if burst_due(fired, n, p, snap.now) {
        out.push(burst_alert(p.last_hour));
        fired.burst_at = Some(snap.now);
    }
    out
}

fn notified_path() -> std::path::PathBuf {
    data_dir().join(NOTIFIED_FILE)
}

fn load_fired() -> Fired {
    let Ok(bytes) = std::fs::read(notified_path()) else { return Fired::default() };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        crate::log::line(&format!("notified.json is unreadable ({e}); alerts already sent this Cycle may repeat"));
        Fired::default()
    })
}

fn persist(fired: &Fired) -> Result<(), String> {
    let bytes = serde_json::to_vec(fired).map_err(|e| e.to_string())?;
    write_private(NOTIFIED_FILE, &bytes)
}

pub fn check(app: &AppHandle, snap: &Snapshot) {
    let mut cache = FIRED.lock().unwrap_or_else(PoisonError::into_inner);
    let fired = cache.get_or_insert_with(load_fired);
    let mut next = fired.clone();
    let messages = due(snap, &mut next);
    if next == *fired {
        return;
    }
    let delivered = messages.iter().fold(true, |all, (title, body)| show(app, title, body) && all);
    if !delivered {
        return;
    }
    *fired = next;
    if let Err(e) = persist(fired) {
        crate::log::line(&format!("notified.json not saved, alerts stay in memory: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::collector::Status as S;
    use crate::core::model::{Client, UsageRecord};
    use crate::core::pace::{compute, Cycle};
    use crate::core::settings::Settings;

    const NOW: i64 = 1_000_000_000_000;

    fn snap(used: f64) -> Snapshot {
        let cycle = Cycle { start: 0, end: 30 * 86_400_000 };
        let mut r = UsageRecord::new("a".into(), 1000, Client::CopilotCli, "m");
        r.aic = Some(used);
        Snapshot {
            records: vec![],
            github_plan: None,
            pace: compute(&[r], cycle, Some(200.0), None, 15 * 86_400_000, 0.05),
            allowance_source: crate::core::snapshot::AllowanceSource::Custom,
            allowance_capped: false,
            entitlement_used_up: false,
            copilot_stopped: false,
            overage: 0.0,
            settings: Settings::default(),
            status: S::default(),
            now: 0,
        }
    }

    fn burst_snap(last_hour: f64, now: i64) -> Snapshot {
        let mut s = snap(0.0);
        s.pace.last_hour = last_hour;
        s.now = now;
        s
    }

    #[test]
    fn fires_highest_threshold_once_per_cycle() {
        let mut f = Fired::default();
        let m = due(&snap(185.0), &mut f); // 92.5%, ahead of pace: a threshold alert plus the pace alert
        assert_eq!(m.len(), 2);
        assert!(m[0].0.contains("90%"));
        assert_eq!(f.thresholds, vec![75, 90]);
        assert!(due(&snap(186.0), &mut f).is_empty());
        let m = due(&snap(200.0), &mut f);
        assert!(m[0].0.contains("100%"));
    }

    #[test]
    fn nothing_when_disabled_or_under() {
        let mut f = Fired::default();
        assert!(due(&snap(50.0), &mut f).is_empty());
        let mut s = snap(190.0);
        s.settings.notifications.enabled = false;
        assert!(due(&s, &mut f).is_empty());
    }

    #[test]
    fn a_new_cycle_clears_the_threshold_and_pace_state_but_not_the_burst_cooldown() {
        let mut f = Fired { cycle_start: -1, thresholds: vec![75, 90], ahead: true, burst_at: Some(NOW) };
        let m = due(&snap(185.0), &mut f);
        assert_eq!(m.len(), 2);
        assert_eq!(f.burst_at, Some(NOW));
        assert_eq!(f.cycle_start, 0);
    }

    #[test]
    fn a_burst_alerts_once_and_then_only_after_the_cooldown() {
        let mut f = Fired::default();
        let first = due(&burst_snap(12.0, NOW), &mut f);
        assert_eq!(first, vec![("Copilot: 12.0 AIC in the last hour".into(), "That's a Burst. Check the Projects tab in the dashboard for a Runaway Session.".into())]);
        assert_eq!(f.burst_at, Some(NOW));
        assert!(due(&burst_snap(12.0, NOW + BURST_COOLDOWN_MS), &mut f).is_empty());
        assert_eq!(due(&burst_snap(12.0, NOW + BURST_COOLDOWN_MS + 1), &mut f).len(), 1);
    }

    #[test]
    fn a_burst_needs_the_threshold_and_the_setting() {
        let mut f = Fired::default();
        assert!(due(&burst_snap(9.9, NOW), &mut f).is_empty());
        let mut s = burst_snap(12.0, NOW);
        s.settings.notifications.burst_aic = 0.0;
        assert!(due(&s, &mut f).is_empty());
        let mut s = burst_snap(12.0, NOW);
        s.settings.notifications.enabled = false;
        assert!(due(&s, &mut f).is_empty());
        assert_eq!(f.burst_at, None);
    }

    #[test]
    fn reset_days_read_as_a_count_with_the_right_plural() {
        let mut s = snap(185.0);
        s.pace.remaining_days = 0.4;
        let m = due(&s, &mut Fired::default());
        assert!(m[0].1.ends_with("The Cycle resets in 1 day."), "{}", m[0].1);
        s.pace.remaining_days = 2.1;
        let m = due(&s, &mut Fired::default());
        assert!(m[0].1.ends_with("The Cycle resets in 3 days."), "{}", m[0].1);
    }

    fn plan_snap(used: f64, entitlement: f64, permitted: bool, allowance: f64) -> Snapshot {
        use crate::core::github::GitHubPlan;
        use crate::core::model::epoch_ms;
        let q = GitHubPlan {
            login: "me".into(),
            plan: "individual".into(),
            sku: None,
            credits_used: Some(used),
            entitlement: Some(entitlement),
            overage_permitted: permitted,
            unlimited: false,
            reset_at: Some(epoch_ms("2026-12-01T00:00:00Z").unwrap()),
            fetched_at: 0,
        };
        let settings = Settings { allowance_override: allowance, ..Settings::default() };
        crate::core::snapshot::build(vec![], Some(q), &settings, S::default(), epoch_ms("2026-11-16T00:00:00Z").unwrap())
    }

    #[test]
    fn allowance_thresholds_fire_the_same_whether_or_not_overage_is_permitted() {
        let (mut a, mut b) = (Fired::default(), Fired::default());
        let off = due(&plan_snap(276.0, 300.0, false, 0.0), &mut a);
        let on = due(&plan_snap(276.0, 300.0, true, 0.0), &mut b);
        assert_eq!(off, on);
        assert!(off[0].0.contains("90%"));
    }

    #[test]
    fn day_labels_follow_the_given_time_zone() {
        let reset = crate::core::model::epoch_ms("2026-12-01T00:00:00Z").unwrap();
        assert_eq!(day_label(reset, TimeZone::UTC).as_deref(), Some("Dec 1"));
        assert_eq!(day_label(reset, TimeZone::fixed(jiff::tz::offset(-8))).as_deref(), Some("Nov 30"));
        assert_eq!(day_label(crate::core::model::epoch_ms("2026-11-09T12:00:00Z").unwrap(), TimeZone::UTC).as_deref(), Some("Nov 9"));
    }

    #[test]
    fn full_allowance_wording_matches_the_case() {
        let full = |s: Snapshot| due(&s, &mut Fired::default()).remove(0);

        let (title, body) = full(plan_snap(300.0, 300.0, false, 0.0));
        assert!(title.contains("100%"));
        assert!(body.contains("stopped") && body.contains("Dec 1"), "{body}");

        let (_, body) = full(plan_snap(310.0, 300.0, true, 0.0));
        assert!(body.contains("Overage") && !body.contains("stopped"), "{body}");

        let (_, body) = full(plan_snap(150.0, 300.0, false, 150.0));
        assert!(body.contains("keeps working") && !body.contains("stopped") && !body.contains("Overage"), "{body}");
    }
}
