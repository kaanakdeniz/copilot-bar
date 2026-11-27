use super::model::UsageRecord;
use jiff::{civil::Date, tz::TimeZone, Timestamp, ToSpan, ZonedDateTime};
use serde::Serialize;
use std::collections::HashSet;

const MS_PER_DAY: i64 = 86_400_000;
const DAY_MS: f64 = MS_PER_DAY as f64;
const RECENT_WINDOW: i64 = 7 * MS_PER_DAY;
const MS_PER_HOUR: i64 = 3_600_000;
const LAST_HOUR: i64 = MS_PER_HOUR;
const NOON: i8 = 12;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub start: i64,
    pub end: i64,
}

impl Cycle {
    pub fn current(reset_at: Option<i64>, now: i64) -> Cycle {
        if let Some(r) = reset_at {
            let anchor = utc(r);
            let mut k = 0;
            while ms(&month_offset(&anchor, k)) <= now {
                k += 1;
            }
            while ms(&month_offset(&anchor, k - 1)) > now {
                k -= 1;
            }
            return Cycle { start: ms(&month_offset(&anchor, k - 1)), end: ms(&month_offset(&anchor, k)) };
        }
        let start = utc(now).date().first_of_month().to_zoned_datetime(TimeZone::UTC).expect("UTC month start");
        Cycle { start: ms(&start), end: ms(&month_offset(&start, 1)) }
    }

    pub fn previous(&self) -> Cycle {
        Cycle { start: ms(&month_offset(&utc(self.start), -1)), end: self.start }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    NoAllowance,
    Under,
    OnPace,
    Ahead,
    Exhausted,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pace {
    pub cycle: Cycle,
    pub used: f64,
    pub logged_aic: f64,
    pub unlogged: f64,
    pub allowance: Option<f64>,
    pub total_days: f64,
    pub elapsed_days: f64,
    pub remaining_days: f64,
    pub avg_daily: f64,
    pub avg_recent: f64,
    pub avg_per_active_day: f64,
    pub active_days: u32,
    pub projected: f64,
    pub even_pace: Option<f64>,
    pub delta: Option<f64>,
    pub used_today: f64,
    pub last_hour: f64,
    pub daily_budget: Option<f64>,
    pub run_out_at: Option<i64>,
    pub status: Status,
    pub frac_used: Option<f64>,
    pub frac_elapsed: f64,
}

struct Progress {
    cycle: Cycle,
    used: f64,
    used_today: f64,
    elapsed: f64,
    total: f64,
    avg: f64,
    projected: f64,
}

struct AllowancePace {
    even_pace: Option<f64>,
    delta: Option<f64>,
    daily_budget: Option<f64>,
    run_out_at: Option<i64>,
    status: Status,
}

impl AllowancePace {
    fn without_allowance() -> AllowancePace {
        AllowancePace { even_pace: None, delta: None, daily_budget: None, run_out_at: None, status: Status::NoAllowance }
    }
}

fn in_zone(epoch_ms: i64, zone: &TimeZone) -> ZonedDateTime {
    Timestamp::from_millisecond(epoch_ms).expect("epoch ms in range").to_zoned_datetime(zone.clone())
}

fn utc(epoch_ms: i64) -> ZonedDateTime {
    in_zone(epoch_ms, &TimeZone::UTC)
}

fn ms(t: &ZonedDateTime) -> i64 {
    t.timestamp().as_millisecond()
}

fn month_offset(anchor: &ZonedDateTime, months: i32) -> ZonedDateTime {
    anchor.checked_add(months.months()).expect("date in range")
}

fn local_start_of_day(epoch_ms: i64, zone: &TimeZone) -> i64 {
    Timestamp::from_millisecond(epoch_ms)
        .and_then(|t| t.to_zoned_datetime(zone.clone()).start_of_day())
        .map(|d| ms(&d))
        .unwrap_or(epoch_ms)
}

fn local_midday(day: Date, zone: &TimeZone) -> i64 {
    ms(&day.at(NOON, 0, 0, 0).to_zoned_datetime(zone.clone()).expect("midday in range"))
}

fn local_date(epoch_ms: i64, zone: &TimeZone) -> Date {
    in_zone(epoch_ms, zone).date()
}

fn days_left(now: i64, cycle: Cycle, zone: &TimeZone) -> f64 {
    let mut day = local_date(now, zone);
    while local_midday(day, zone) < cycle.start {
        day = day.tomorrow().expect("date in range");
    }
    let mut days = 0;
    while local_midday(day, zone) < cycle.end {
        days += 1;
        day = day.tomorrow().expect("date in range");
    }
    days.max(1) as f64
}

fn sum_inclusive(credits: &[&UsageRecord], from: i64, until: i64) -> f64 {
    credits.iter().filter(|r| r.ts >= from && (r.ts < until || r.ts == from)).map(|r| r.aic.unwrap_or(0.0)).sum()
}

fn unlogged_scale(used: f64, logged: f64) -> f64 {
    if logged > 0.0 && used > logged {
        used / logged
    } else {
        1.0
    }
}

fn allowance_pace(allowance: f64, progress: &Progress, days_left: f64, tolerance: f64) -> AllowancePace {
    let used = progress.used;
    let exp = allowance * progress.elapsed / progress.total;
    let status = if used >= allowance {
        Status::Exhausted
    } else if (used - exp).abs() <= allowance * tolerance {
        Status::OnPace
    } else if used > exp {
        Status::Ahead
    } else {
        Status::Under
    };
    let run_out_at = (progress.avg > 0.0 && progress.projected > allowance && used < allowance)
        .then(|| progress.cycle.start + (allowance / progress.avg * DAY_MS) as i64);
    AllowancePace {
        even_pace: Some(exp),
        delta: Some(used - exp),
        daily_budget: Some((allowance - (used - progress.used_today)).max(0.0) / days_left),
        run_out_at,
        status,
    }
}

pub fn compute(
    records: &[UsageRecord],
    cycle: Cycle,
    allowance: Option<f64>,
    reported_used: Option<f64>,
    now: i64,
    tolerance: f64,
) -> Pace {
    compute_in(&TimeZone::system(), records, cycle, allowance, reported_used, now, tolerance)
}

fn compute_in(
    zone: &TimeZone,
    records: &[UsageRecord],
    cycle: Cycle,
    allowance: Option<f64>,
    reported_used: Option<f64>,
    now: i64,
    tolerance: f64,
) -> Pace {
    let credits: Vec<&UsageRecord> = records.iter().filter(|r| r.aic.is_some()).collect();
    let now_c = now.clamp(cycle.start, cycle.end);
    let logged_aic = sum_inclusive(&credits, cycle.start, now_c);
    let used = logged_aic.max(reported_used.unwrap_or(0.0));
    let total = (cycle.end - cycle.start) as f64 / DAY_MS;
    let elapsed = ((now - cycle.start) as f64 / DAY_MS).clamp(0.0, total);
    let remaining = (total - elapsed).max(0.0);
    let avg = used / elapsed.max(1.0);
    let projected = used + avg * remaining;

    let recent_start = cycle.start.max(now - RECENT_WINDOW);
    let recent_days = ((now - recent_start) as f64 / DAY_MS).max(1.0);
    let recent = sum_inclusive(&credits, recent_start, now);

    let active: HashSet<i64> = credits
        .iter()
        .filter(|r| r.ts >= cycle.start && r.ts < now_c && r.aic.unwrap_or(0.0) > 0.0)
        .map(|r| local_start_of_day(r.ts, zone))
        .collect();

    let today_start = local_start_of_day(now, zone).max(cycle.start);
    let used_today = sum_inclusive(&credits, today_start, now);
    let last_hour: f64 = credits
        .iter()
        .filter(|r| !r.legacy && r.ts > now - LAST_HOUR && r.ts <= now)
        .map(|r| r.aic.unwrap_or(0.0))
        .sum();

    let allowance = allowance.filter(|a| *a > 0.0);
    let progress = Progress { cycle, used, used_today, elapsed, total, avg, projected };
    let pace = match allowance {
        Some(a) => allowance_pace(a, &progress, days_left(now, cycle, zone), tolerance),
        None => AllowancePace::without_allowance(),
    };
    Pace {
        cycle,
        used,
        logged_aic,
        unlogged: (used - logged_aic).max(0.0),
        allowance,
        total_days: total,
        elapsed_days: elapsed,
        remaining_days: remaining,
        avg_daily: avg,
        avg_recent: recent / recent_days * unlogged_scale(used, logged_aic),
        avg_per_active_day: if active.is_empty() { 0.0 } else { used / active.len() as f64 },
        active_days: active.len() as u32,
        projected,
        even_pace: pace.even_pace,
        delta: pace.delta,
        used_today,
        last_hour,
        daily_budget: pace.daily_budget,
        run_out_at: pace.run_out_at,
        status: pace.status,
        frac_used: allowance.map(|b| used / b),
        frac_elapsed: if total > 0.0 { elapsed / total } else { 0.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{epoch_ms, Client};

    fn ms(s: &str) -> i64 {
        epoch_ms(s).unwrap()
    }
    fn nov() -> Cycle {
        Cycle { start: ms("2026-11-01T00:00:00Z"), end: ms("2026-12-01T00:00:00Z") }
    }
    fn rec(id: &str, ts: &str, aic: Option<f64>) -> UsageRecord {
        let mut r = UsageRecord::new(id.into(), ms(ts), Client::CopilotCli, "m");
        r.aic = aic;
        r
    }
    const TOLERANCE: f64 = 0.05;
    fn zone_named(name: &str) -> TimeZone {
        TimeZone::get(name).unwrap()
    }
    fn local(civil: &str, zone: &TimeZone) -> i64 {
        civil.parse::<jiff::civil::DateTime>().unwrap().to_zoned_datetime(zone.clone()).unwrap().timestamp().as_millisecond()
    }

    #[test]
    fn cycle_from_reset_date() {
        let now = ms("2026-11-20T12:00:00Z");
        assert_eq!(Cycle::current(Some(ms("2026-12-01T00:00:00Z")), now), nov());
        assert_eq!(Cycle::current(Some(ms("2026-11-01T00:00:00Z")), now), nov());
        assert_eq!(Cycle::current(None, now), nov());
        assert_eq!(nov().previous().start, ms("2026-10-01T00:00:00Z"));
    }

    #[test]
    fn cycle_keeps_the_reset_time_and_clamps_to_month_end() {
        let c = Cycle::current(Some(ms("2026-12-01T05:30:00Z")), ms("2026-12-10T00:00:00Z"));
        assert_eq!(c, Cycle { start: ms("2026-12-01T05:30:00Z"), end: ms("2027-01-01T05:30:00Z") });

        // A reset on the 31st clamps to the last day of a shorter month, and returns to the 31st afterwards.
        let jan31 = ms("2027-01-31T00:00:00Z");
        let c = Cycle::current(Some(jan31), ms("2027-02-15T00:00:00Z"));
        assert_eq!(c, Cycle { start: ms("2027-01-31T00:00:00Z"), end: ms("2027-02-28T00:00:00Z") });
        let c = Cycle::current(Some(jan31), ms("2027-03-15T00:00:00Z"));
        assert_eq!(c, Cycle { start: ms("2027-02-28T00:00:00Z"), end: ms("2027-03-31T00:00:00Z") });

        // A stale reset of 31 March, viewed on 15 June.
        let c = Cycle::current(Some(ms("2027-03-31T00:00:00Z")), ms("2027-06-15T00:00:00Z"));
        assert_eq!(c, Cycle { start: ms("2027-05-31T00:00:00Z"), end: ms("2027-06-30T00:00:00Z") });

        assert_eq!(Cycle { start: ms("2027-03-31T00:00:00Z"), end: 0 }.previous().start, ms("2027-02-28T00:00:00Z"));
        // Without a reset date: the UTC calendar month, whatever the local time zone.
        assert_eq!(Cycle::current(None, ms("2026-11-30T23:59:59Z")), nov());
    }

    #[test]
    fn local_start_of_day_is_a_local_midnight() {
        let zone = TimeZone::system();
        for t in ["2026-11-21T12:00:00Z", "2026-03-29T00:30:00Z", "2026-11-01T09:45:00Z", "2026-06-30T23:59:59Z"] {
            let now = ms(t);
            let start = local_start_of_day(now, &zone);
            assert!(start <= now && now - start < 25 * MS_PER_HOUR, "{t}");
            assert_eq!(local_start_of_day(start, &zone), start, "{t}: idempotent");
            assert!(local_start_of_day(start - 1, &zone) < start, "{t}: the instant before is yesterday");
        }
    }

    #[test]
    fn ahead_of_pace_projects_run_out() {
        let records: Vec<_> = (0..15).map(|i| rec(&i.to_string(), &format!("2026-11-{:02}T10:00:00Z", i + 1), Some(40.0))).collect();
        let p = compute(&records, nov(), Some(1000.0), None, ms("2026-11-16T00:00:00Z"), TOLERANCE);
        assert_eq!(p.status, Status::Ahead);
        assert_eq!(p.used, 600.0);
        assert_eq!(p.elapsed_days, 15.0);
        assert_eq!(p.avg_daily, 40.0);
        assert_eq!(p.projected, 1200.0);
        assert_eq!(p.even_pace, Some(500.0));
        assert_eq!(p.delta, Some(100.0));
        assert_eq!(p.active_days, 15);
        assert_eq!(p.run_out_at, Some(ms("2026-11-26T00:00:00Z")));
        assert!((p.avg_recent - 40.0).abs() < 1e-9);
    }

    #[test]
    fn statuses_and_tolerance() {
        let now = ms("2026-11-16T00:00:00Z");
        let st = |used: f64, allowance: Option<f64>| {
            compute(&[rec("a", "2026-11-02T00:00:00Z", Some(used))], nov(), allowance, None, now, TOLERANCE).status
        };
        assert_eq!(st(510.0, Some(1000.0)), Status::OnPace);
        assert_eq!(st(200.0, Some(1000.0)), Status::Under);
        assert_eq!(st(1000.0, Some(1000.0)), Status::Exhausted);
        assert_eq!(st(10.0, None), Status::NoAllowance);
    }

    #[test]
    fn reported_counter_wins_when_higher() {
        let r = [rec("a", "2026-11-10T00:00:00Z", Some(10.0))];
        let now = ms("2026-11-16T00:00:00Z");
        assert_eq!(compute(&r, nov(), Some(100.0), Some(25.0), now, TOLERANCE).used, 25.0);
        assert_eq!(compute(&r, nov(), Some(100.0), Some(3.0), now, TOLERANCE).used, 10.0);
    }

    #[test]
    fn days_left_counts_local_days_whose_midday_is_in_the_cycle() {
        let cases = [
            ("UTC", "2026-11-30T11:00:00", 1.0),
            ("Europe/Istanbul", "2026-11-30T14:00:00", 1.0),
            ("Europe/Istanbul", "2026-11-21T15:00:00", 10.0),
            ("America/Los_Angeles", "2026-11-30T03:00:00", 1.0),
            ("America/Los_Angeles", "2026-10-31T17:30:00", 30.0),
            ("Pacific/Auckland", "2026-11-01T13:30:00", 30.0),
            ("Pacific/Auckland", "2026-11-15T12:00:00", 17.0),
            ("Pacific/Auckland", "2026-11-30T10:00:00", 2.0),
            ("Pacific/Auckland", "2026-12-01T10:00:00", 1.0),
            ("Pacific/Kiritimati", "2026-11-01T14:30:00", 30.0),
            ("Pacific/Kiritimati", "2026-11-15T12:00:00", 17.0),
            ("Pacific/Kiritimati", "2026-11-30T10:00:00", 2.0),
            ("Pacific/Kiritimati", "2026-12-01T10:00:00", 1.0),
        ];
        for (name, civil, days) in cases {
            let zone = zone_named(name);
            assert_eq!(days_left(local(civil, &zone), nov(), &zone), days, "{name} {civil}");
        }
    }

    #[test]
    fn daily_budget_is_what_is_left_over_the_days_left_in_each_zone() {
        let record = [rec("a", "2026-11-05T10:00:00Z", Some(100.0))];
        let budget = |name: &str, civil: &str| {
            let zone = zone_named(name);
            let now = local(civil, &zone);
            compute_in(&zone, &record, nov(), Some(300.0), None, now, TOLERANCE).daily_budget
        };
        assert_eq!(budget("UTC", "2026-11-30T11:00:00"), Some(200.0));
        assert_eq!(budget("Europe/Istanbul", "2026-11-30T14:00:00"), Some(200.0));
        assert_eq!(budget("Europe/Istanbul", "2026-11-21T15:00:00"), Some(20.0));
    }

    #[test]
    fn daily_budget_counts_today_on_the_local_clock() {
        let zone = zone_named("Europe/Istanbul");
        let mut today = rec("today", "2026-01-01T00:00:00Z", Some(7.0));
        today.ts = local("2026-11-22T00:30:00", &zone);
        let mut yesterday = rec("yesterday", "2026-01-01T00:00:00Z", Some(20.0));
        yesterday.ts = local("2026-11-21T23:30:00", &zone);
        let p = compute_in(&zone, &[today, yesterday], nov(), Some(300.0), None, local("2026-11-22T01:00:00", &zone), TOLERANCE);
        assert_eq!((p.used, p.used_today), (27.0, 7.0));
        assert_eq!(p.daily_budget, Some(280.0 / 9.0));
    }

    #[test]
    fn daily_budget_on_the_first_local_day_of_a_utc_cycle() {
        let zone = zone_named("America/Los_Angeles");
        let r = [rec("oct", "2026-10-31T23:50:00Z", Some(50.0)), rec("nov", "2026-11-01T00:10:00Z", Some(6.0))];
        let p = compute_in(&zone, &r, nov(), Some(300.0), None, ms("2026-11-01T00:30:00Z"), TOLERANCE);
        assert_eq!((p.used, p.used_today), (6.0, 6.0));
        assert_eq!(p.daily_budget, Some(10.0));
    }

    #[test]
    fn last_hour_leaves_out_legacy_totals() {
        let now = ms("2026-11-20T12:00:00Z");
        let ten_minutes_ago = "2026-11-20T11:50:00Z";
        let per_call = rec("call", ten_minutes_ago, Some(3.0));
        let mut legacy = rec("legacy", ten_minutes_ago, Some(50.0));
        legacy.legacy = true;
        let p = compute(&[per_call.clone(), legacy.clone()], nov(), None, None, now, TOLERANCE);
        assert_eq!(p.last_hour, 3.0);
        assert_eq!(p.used, 53.0);
        assert_eq!(compute(&[legacy], nov(), None, None, now, TOLERANCE).last_hour, 0.0);
    }

    #[test]
    fn local_start_of_day_is_zone_aware() {
        let zone = zone_named("Pacific/Kiritimati");
        assert_eq!(local_start_of_day(ms("2026-11-30T12:00:00Z"), &zone), ms("2026-11-30T10:00:00Z"));
    }
}
