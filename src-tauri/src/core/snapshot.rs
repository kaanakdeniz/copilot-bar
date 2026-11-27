use super::github::GitHubPlan;
use super::model::UsageRecord;
use super::pace::{self, Cycle, Pace};
use super::settings::Settings;
use serde::Serialize;

#[cfg(test)]
use super::pace::Status as PaceStatus;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub records: Vec<UsageRecord>,
    pub github_plan: Option<GitHubPlan>,
    pub pace: Pace,
    pub allowance_source: AllowanceSource,
    pub allowance_capped: bool,
    pub entitlement_used_up: bool,
    pub copilot_stopped: bool,
    pub overage: f64,
    pub settings: Settings,
    pub status: Status,
    pub now: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AllowanceSource {
    Custom,
    Github,
    None,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub last_refresh: Option<i64>,
    pub plan_error: Option<PlanError>,
    pub sign_in: SignIn,
    pub cli_files: usize,
    pub connecting_since: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanError {
    pub message: String,
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SignIn {
    #[default]
    Unknown,
    SignedIn { login: String },
    GhMissing,
    SignedOut,
    SsoRequired,
    AccountMismatch { gh_login: String, copilot_login: String },
    Error { message: String },
}

pub fn build(records: Vec<UsageRecord>, github_plan: Option<GitHubPlan>, settings: &Settings, status: Status, now: i64) -> Snapshot {
    let entitlement = github_plan.as_ref().and_then(GitHubPlan::entitlement_aic);
    let custom = (settings.allowance_override > 0.0).then_some(settings.allowance_override);
    let (allowance, source) = match (custom, entitlement) {
        (Some(a), _) => (Some(a), AllowanceSource::Custom),
        (None, Some(e)) => (Some(e), AllowanceSource::Github),
        (None, None) => (None, AllowanceSource::None),
    };
    let permitted = github_plan.as_ref().is_some_and(|plan| plan.overage_permitted);
    let (allowance, allowance_capped) = match (allowance, entitlement) {
        (Some(a), Some(e)) if !permitted && a > e => (Some(e), true),
        _ => (allowance, false),
    };
    let cycle = Cycle::current(github_plan.as_ref().and_then(|plan| plan.reset_at), now);
    let counter_this_cycle = github_plan
        .as_ref()
        .filter(|plan| plan.reset_at.map_or(true, |r| r > now) && plan.reset_at.map_or(true, |r| r <= cycle.end))
        .and_then(|plan| plan.credits_used);
    let pace = pace::compute(&records, cycle, allowance, counter_this_cycle, now, settings.pace_tolerance_pct / 100.0);
    let entitlement_used_up = entitlement.is_some_and(|e| pace.used >= e);
    let overage = match entitlement {
        Some(e) if permitted => (pace.used - e).max(0.0),
        _ => 0.0,
    };
    Snapshot {
        records,
        github_plan,
        pace,
        allowance_source: source,
        allowance_capped,
        entitlement_used_up,
        copilot_stopped: entitlement_used_up && !permitted,
        overage,
        settings: settings.clone(),
        status,
        now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::github::GitHubPlan;
    use crate::core::model::{epoch_ms, Client, UsageRecord};
    use crate::core::settings::Settings;

    fn ms(s: &str) -> i64 {
        epoch_ms(s).unwrap()
    }
    fn rec(id: &str, ts: &str, aic: f64) -> UsageRecord {
        let mut r = UsageRecord::new(id.into(), ms(ts), Client::CopilotCli, "m");
        r.aic = Some(aic);
        r
    }
    fn github_plan(entitlement: Option<f64>, credits_used: Option<f64>, reset_at: &str) -> GitHubPlan {
        GitHubPlan {
            login: "me".into(),
            plan: "individual".into(),
            sku: None,
            credits_used,
            entitlement,
            overage_permitted: false,
            unlimited: false,
            reset_at: Some(ms(reset_at)),
            fetched_at: 0,
        }
    }
    fn settings(allowance: f64) -> Settings {
        Settings { allowance_override: allowance, ..Settings::default() }
    }

    #[test]
    fn allowance_comes_from_settings_then_github_then_none() {
        let now = ms("2026-11-16T00:00:00Z");
        let q = github_plan(Some(300.0), None, "2026-12-01T00:00:00Z");

        let s = build(vec![], Some(q.clone()), &settings(250.0), Status::default(), now);
        assert_eq!((s.allowance_source, s.pace.allowance), (AllowanceSource::Custom, Some(250.0)));

        let s = build(vec![], Some(q.clone()), &settings(0.0), Status::default(), now);
        assert_eq!((s.allowance_source, s.pace.allowance), (AllowanceSource::Github, Some(300.0)));

        let unlimited = GitHubPlan { unlimited: true, ..q.clone() };
        let s = build(vec![], Some(unlimited), &settings(0.0), Status::default(), now);
        assert_eq!((s.allowance_source, s.pace.allowance), (AllowanceSource::None, None));

        let no_entitlement = GitHubPlan { entitlement: Some(0.0), ..q };
        let s = build(vec![], Some(no_entitlement), &settings(0.0), Status::default(), now);
        assert_eq!((s.allowance_source, s.pace.allowance), (AllowanceSource::None, None));

        let s = build(vec![], None, &settings(0.0), Status::default(), now);
        assert_eq!((s.allowance_source, s.pace.allowance, s.pace.status), (AllowanceSource::None, None, PaceStatus::NoAllowance));
    }

    #[test]
    fn allowance_source_serialises_as_before() {
        let names: Vec<_> = [AllowanceSource::Custom, AllowanceSource::Github, AllowanceSource::None]
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect();
        assert_eq!(names, ["custom", "github", "none"]);
    }

    #[test]
    fn free_plan_has_an_allowance_from_its_chat_credits() {
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/copilot-free-user.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let plan = crate::core::github::parse_plan(&serde_json::from_str(&text).unwrap(), 0);
        let s = build(vec![], Some(plan), &settings(0.0), Status::default(), ms("2026-11-16T00:00:00Z"));
        assert_eq!((s.allowance_source, s.pace.allowance), (AllowanceSource::Github, Some(200.0)));
        assert!(!s.entitlement_used_up);
    }

    #[test]
    fn used_is_the_larger_of_logged_aic_and_githubs_counter() {
        let now = ms("2026-11-16T00:00:00Z");
        let r = || vec![rec("a", "2026-11-10T00:00:00Z", 10.0)];

        let s = build(r(), Some(github_plan(Some(100.0), Some(25.0), "2026-12-01T00:00:00Z")), &settings(0.0), Status::default(), now);
        assert_eq!((s.pace.used, s.pace.logged_aic), (25.0, 10.0));

        let s = build(r(), Some(github_plan(Some(100.0), Some(3.0), "2026-12-01T00:00:00Z")), &settings(0.0), Status::default(), now);
        assert_eq!((s.pace.used, s.pace.logged_aic), (10.0, 10.0));
    }

    #[test]
    fn unlogged_usage_is_what_githubs_counter_has_beyond_the_logs() {
        let now = ms("2026-11-16T00:00:00Z");
        let r = vec![rec("a", "2026-11-10T00:00:00Z", 10.0), rec("b", "2026-11-12T00:00:00Z", 5.0)];
        let s = build(r, Some(github_plan(Some(300.0), Some(40.0), "2026-12-01T00:00:00Z")), &settings(0.0), Status::default(), now);
        assert_eq!((s.pace.used, s.pace.logged_aic, s.pace.unlogged), (40.0, 15.0, 25.0));
    }

    #[test]
    fn logs_above_githubs_counter_leave_no_unlogged_usage() {
        // GitHub's counter lags: what was just used today is only in the logs.
        let now = ms("2026-11-16T15:00:00Z");
        let r = vec![rec("a", "2026-11-10T00:00:00Z", 10.0), rec("today", "2026-11-16T14:00:00Z", 8.0)];
        let s = build(r, Some(github_plan(Some(300.0), Some(12.0), "2026-12-01T00:00:00Z")), &settings(0.0), Status::default(), now);
        assert_eq!((s.pace.used, s.pace.logged_aic, s.pace.unlogged), (18.0, 18.0, 0.0));
    }

    #[test]
    fn without_plan_data_used_is_logged_aic_and_nothing_is_unlogged() {
        let now = ms("2026-11-16T00:00:00Z");
        let s = build(vec![rec("a", "2026-11-10T00:00:00Z", 7.0)], None, &settings(0.0), Status::default(), now);
        assert_eq!((s.pace.used, s.pace.logged_aic, s.pace.unlogged), (7.0, 7.0, 0.0));
        let s = build(vec![], None, &settings(0.0), Status::default(), now);
        assert_eq!((s.pace.used, s.pace.logged_aic, s.pace.unlogged), (0.0, 0.0, 0.0));
    }

    #[test]
    fn counter_from_a_previous_cycle_is_ignored() {
        // The plan data was read in November; GitHub reset on 1 December and hasn't been asked again.
        let q = github_plan(Some(300.0), Some(250.0), "2026-12-01T00:00:00Z");
        let r = vec![rec("nov", "2026-11-20T00:00:00Z", 30.0), rec("dec", "2026-12-01T10:00:00Z", 4.0)];
        let s = build(r, Some(q), &settings(0.0), Status::default(), ms("2026-12-02T00:00:00Z"));
        assert_eq!(s.pace.cycle.start, ms("2026-12-01T00:00:00Z"));
        assert_eq!((s.pace.used, s.pace.logged_aic), (4.0, 4.0));
    }

    #[test]
    fn exhausted_without_overage_means_copilot_has_stopped() {
        let now = ms("2026-11-16T00:00:00Z");
        let q = github_plan(Some(300.0), Some(300.0), "2026-12-01T00:00:00Z");
        let s = build(vec![], Some(q), &settings(0.0), Status::default(), now);
        assert_eq!(s.pace.status, PaceStatus::Exhausted);
        assert!(s.entitlement_used_up && s.copilot_stopped);
        assert_eq!(s.overage, 0.0);
    }

    #[test]
    fn own_allowance_above_the_entitlement_is_capped_when_overage_is_not_permitted() {
        // Copilot stops at the Entitlement, so an Allowance of 500 can't be reached: it is effectively 300.
        let now = ms("2026-11-16T00:00:00Z");
        let q = github_plan(Some(300.0), Some(300.0), "2026-12-01T00:00:00Z");
        let s = build(vec![], Some(q), &settings(500.0), Status::default(), now);
        assert_eq!(s.pace.allowance, Some(300.0));
        assert!(s.allowance_capped);
        assert_eq!(s.pace.status, PaceStatus::Exhausted);
        assert!(s.copilot_stopped);
        assert_eq!(s.settings.allowance_override, 500.0, "Settings keeps the number as entered");
    }

    #[test]
    fn own_allowance_above_the_entitlement_stands_when_overage_is_permitted() {
        let now = ms("2026-11-16T00:00:00Z");
        let q = GitHubPlan { overage_permitted: true, ..github_plan(Some(300.0), Some(300.0), "2026-12-01T00:00:00Z") };
        let s = build(vec![], Some(q), &settings(500.0), Status::default(), now);
        assert_eq!(s.pace.allowance, Some(500.0));
        assert!(!s.allowance_capped);
        assert_ne!(s.pace.status, PaceStatus::Exhausted);
    }

    #[test]
    fn with_overage_permitted_aic_above_the_entitlement_is_overage() {
        let now = ms("2026-11-16T00:00:00Z");
        let q = GitHubPlan { overage_permitted: true, ..github_plan(Some(300.0), Some(345.5), "2026-12-01T00:00:00Z") };
        let s = build(vec![], Some(q), &settings(0.0), Status::default(), now);
        assert_eq!(s.pace.status, PaceStatus::Exhausted);
        assert!(s.entitlement_used_up && !s.copilot_stopped);
        assert_eq!(s.overage, 45.5);
    }

    #[test]
    fn own_allowance_below_the_entitlement_can_be_exhausted_while_copilot_works() {
        let now = ms("2026-11-16T00:00:00Z");
        let q = github_plan(Some(300.0), Some(120.0), "2026-12-01T00:00:00Z");
        let s = build(vec![], Some(q), &settings(100.0), Status::default(), now);
        assert_eq!(s.pace.status, PaceStatus::Exhausted);
        assert!(!s.entitlement_used_up && !s.copilot_stopped);
        assert_eq!(s.overage, 0.0);
    }

    #[test]
    fn pace_status_against_even_pace() {
        // Halfway through a 30-day Cycle: Even Pace is 500 of 1000, tolerance 5% = 50.
        let now = ms("2026-11-16T00:00:00Z");
        let st = |logged_aic: f64, counter: Option<f64>| {
            let s = Settings { pace_tolerance_pct: 5.0, ..settings(1000.0) };
            let q = github_plan(None, counter, "2026-12-01T00:00:00Z");
            build(vec![rec("a", "2026-11-02T00:00:00Z", logged_aic)], Some(q), &s, Status::default(), now).pace.status
        };
        assert_eq!(st(200.0, None), PaceStatus::Under);
        assert_eq!(st(540.0, None), PaceStatus::OnPace);
        assert_eq!(st(600.0, None), PaceStatus::Ahead);
        assert_eq!(st(1000.0, None), PaceStatus::Exhausted);
        assert_eq!(st(100.0, Some(1000.0)), PaceStatus::Exhausted, "GitHub's counter counts toward Used");
    }

    #[test]
    fn daily_budget_on_the_first_local_day_of_a_utc_cycle() {
        // Any time zone: the local day starts before the Cycle, so the previous Cycle's usage that day is not Used.
        let q = github_plan(None, None, "2026-12-01T00:00:00Z");
        let r = vec![rec("oct", "2026-10-31T23:50:00Z", 50.0), rec("nov", "2026-11-01T00:10:00Z", 6.0)];
        let s = build(r, Some(q), &settings(300.0), Status::default(), ms("2026-11-01T00:30:00Z"));
        assert_eq!((s.pace.used, s.pace.used_today), (6.0, 6.0));
        assert_eq!(s.pace.daily_budget, Some(10.0));
    }

    #[test]
    fn used_today_counts_the_local_day() {
        // At 01:00 local on 22 November, 00:30 is today and 23:30 the day before is not, whatever their UTC dates.
        let local = |d: i8, h: i8, m: i8| {
            jiff::civil::date(2026, 11, d).at(h, m, 0, 0).to_zoned_datetime(jiff::tz::TimeZone::system()).unwrap().timestamp().as_millisecond()
        };
        let mut today = rec("today", "2026-01-01T00:00:00Z", 7.0);
        today.ts = local(22, 0, 30);
        let mut yesterday = rec("yesterday", "2026-01-01T00:00:00Z", 20.0);
        yesterday.ts = local(21, 23, 30);
        let q = github_plan(None, None, "2026-12-01T00:00:00Z");
        let s = build(vec![today, yesterday], Some(q), &settings(300.0), Status::default(), local(22, 1, 0));
        assert_eq!((s.pace.used, s.pace.used_today), (27.0, 7.0));
    }
}
