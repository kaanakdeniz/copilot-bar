use super::gh::{Gh, GhError};
use super::model::num;
use std::path::{Path, PathBuf};
use jiff::{civil::Date, tz::TimeZone, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPlan {
    pub login: String,
    pub plan: String,
    pub sku: Option<String>,
    pub credits_used: Option<f64>,
    pub entitlement: Option<f64>,
    pub overage_permitted: bool,
    pub unlimited: bool,
    pub reset_at: Option<i64>,
    pub fetched_at: i64,
}

fn has_credit_quota(s: &Value) -> bool {
    s["unlimited"] != true && (s["has_quota"] == true || num(s.get("entitlement")).unwrap_or(0.0) > 0.0)
}

const PAID_BUCKET: &str = "premium_interactions";
const FREE_CHAT_BUCKET: &str = "chat";

fn paid_bucket(snaps: &Value) -> Option<&str> {
    has_credit_quota(&snaps[PAID_BUCKET]).then_some(PAID_BUCKET)
}

fn free_chat_bucket(snaps: &Value) -> Option<&str> {
    let chat = &snaps[FREE_CHAT_BUCKET];
    (has_credit_quota(chat) && chat["token_based_billing"] == true).then_some(FREE_CHAT_BUCKET)
}

fn any_credit_bucket(snaps: &Value) -> Option<&str> {
    snaps
        .as_object()?
        .iter()
        .find(|(name, bucket)| (name.contains("premium") || name.contains("credit")) && has_credit_quota(bucket))
        .map(|(name, _)| name.as_str())
}

fn winning_bucket(snaps: &Value) -> &str {
    paid_bucket(snaps).or_else(|| free_chat_bucket(snaps)).or_else(|| any_credit_bucket(snaps)).unwrap_or(PAID_BUCKET)
}

fn credit_bucket(snaps: &Value) -> Option<&Value> {
    snaps.get(winning_bucket(snaps))
}

impl GitHubPlan {
    pub fn entitlement_aic(&self) -> Option<f64> {
        if self.unlimited {
            None
        } else {
            self.entitlement.filter(|e| *e > 0.0)
        }
    }
}

pub fn parse_plan(v: &Value, now_ms: i64) -> GitHubPlan {
    let snaps = &v["quota_snapshots"];
    let chosen = credit_bucket(snaps);
    let field = |k: &str| chosen.and_then(|c| num(c.get(k)));
    let entitlement = field("entitlement");
    let remaining = field("quota_remaining").or(field("remaining"));
    let used = match (entitlement, remaining) {
        (Some(ent), Some(rem)) if ent > 0.0 => Some((ent - rem).max(0.0)),
        _ => field("credits_used"),
    };
    let reset = v["quota_reset_date_utc"]
        .as_str()
        .and_then(super::model::epoch_ms)
        .or_else(|| {
            v["quota_reset_date"]
                .as_str()
                .and_then(|d| Date::strptime("%Y-%m-%d", d).ok())
                .and_then(|d| d.to_zoned_datetime(TimeZone::UTC).ok())
                .map(|d| d.timestamp().as_millisecond())
        });
    GitHubPlan {
        login: v["login"].as_str().unwrap_or("").to_string(),
        plan: v["copilot_plan"].as_str().unwrap_or("unknown").to_string(),
        sku: v["access_type_sku"].as_str().map(String::from),
        credits_used: used,
        entitlement,
        overage_permitted: chosen.and_then(|c| c["overage_permitted"].as_bool()).unwrap_or(false),
        unlimited: chosen.and_then(|c| c["unlimited"].as_bool()).unwrap_or(false),
        reset_at: reset,
        fetched_at: now_ms,
    }
}

pub async fn fetch(gh: &Gh) -> Result<GitHubPlan, GhError> {
    let v = gh.api("copilot_internal/user").await?;
    Ok(parse_plan(&v, Timestamp::now().as_millisecond()))
}

/// The Copilot CLI's config.json isn't strict JSON: it starts with `//` comment lines.
pub fn copilot_cli_login(config: &Path) -> Option<String> {
    let text = std::fs::read_to_string(config).ok()?;
    let json: String = text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    let v: Value = serde_json::from_str(&json).ok()?;
    v["lastLoggedInUser"]["login"].as_str().filter(|l| !l.is_empty()).map(String::from)
}

pub fn copilot_cli_config() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".copilot/config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_premium_snapshot() {
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/copilot-user.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let q = parse_plan(&serde_json::from_str(&text).unwrap(), 0);
        assert_eq!(q.login, "octo");
        assert_eq!(q.entitlement, Some(1000.0));
        assert_eq!(q.credits_used, Some(412.5));
        assert!(q.overage_permitted);
        assert_eq!(q.reset_at, super::super::model::epoch_ms("2026-12-01T00:00:00.000Z"));
    }

    #[test]
    fn derives_usage_from_remaining() {
        let v: Value = serde_json::from_str(r#"{"login":"a","quota_snapshots":{"premium_interactions":{"entitlement":200,"remaining":150,"credits_used":0}}}"#).unwrap();
        assert_eq!(parse_plan(&v, 0).credits_used, Some(50.0));
    }

    fn free_plan() -> GitHubPlan {
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/copilot-free-user.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        parse_plan(&serde_json::from_str(&text).unwrap(), 0)
    }

    #[test]
    fn free_plan_takes_its_allowance_from_the_chat_credits_bucket() {
        let q = free_plan();
        assert_eq!(q.sku.as_deref(), Some("free_limited_copilot"));
        assert_eq!(q.entitlement, Some(200.0), "premium_interactions has no quota on Free; chat holds the credits");
        assert!(!q.overage_permitted && !q.unlimited);
        assert_eq!(q.reset_at, super::super::model::epoch_ms("2026-12-01T00:00:00.000Z"));
    }

    #[test]
    fn free_plan_used_is_entitlement_minus_quota_remaining() {
        let used = free_plan().credits_used.unwrap();
        assert!((used - 2.7).abs() < 1e-9, "used was {used}");
    }

    #[test]
    fn completions_are_never_credits() {
        assert_ne!(free_plan().entitlement, Some(2000.0));
    }

    #[test]
    fn date_only_reset_is_utc_midnight() {
        let v: Value = serde_json::from_str(r#"{"login":"a","quota_reset_date":"2026-12-01"}"#).unwrap();
        assert_eq!(parse_plan(&v, 0).reset_at, super::super::model::epoch_ms("2026-12-01T00:00:00Z"));
        let bad: Value = serde_json::from_str(r#"{"login":"a","quota_reset_date":"soon"}"#).unwrap();
        assert_eq!(parse_plan(&bad, 0).reset_at, None);
    }

    fn fixture(name: &str) -> Value {
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn each_plan_shape_has_one_winning_bucket() {
        let cases = [
            ("paid plan with a premium quota", fixture("copilot-user.json")["quota_snapshots"].clone(), PAID_BUCKET),
            ("free plan with chat credits", fixture("copilot-free-user.json")["quota_snapshots"].clone(), FREE_CHAT_BUCKET),
            (
                "chat without token billing loses to a credit bucket with another name",
                serde_json::json!({
                    "premium_interactions": {"has_quota": false},
                    "chat": {"entitlement": 200, "has_quota": true, "token_based_billing": false},
                    "ai_credits": {"entitlement": 500}
                }),
                "ai_credits",
            ),
            (
                "unlimited premium is not a credit quota, so free chat wins",
                serde_json::json!({
                    "premium_interactions": {"unlimited": true, "entitlement": 0},
                    "chat": {"entitlement": 200, "has_quota": true, "token_based_billing": true}
                }),
                FREE_CHAT_BUCKET,
            ),
            (
                "no bucket with a quota falls back to premium_interactions",
                serde_json::json!({
                    "premium_interactions": {"has_quota": false},
                    "chat": {"entitlement": 200, "has_quota": true, "token_based_billing": false}
                }),
                PAID_BUCKET,
            ),
            (
                "no credit-like bucket at all falls back to premium_interactions",
                serde_json::json!({"completions": {"entitlement": 2000, "has_quota": true}}),
                PAID_BUCKET,
            ),
        ];
        for (label, snaps, winner) in cases {
            assert_eq!(winning_bucket(&snaps), winner, "{label}");
        }
    }

    #[test]
    fn free_plan_chat_bucket_is_the_credit_bucket() {
        let snaps = fixture("copilot-free-user.json")["quota_snapshots"].clone();
        assert_eq!(credit_bucket(&snaps).and_then(|b| b["entitlement"].as_f64()), Some(200.0));
    }

    #[test]
    fn entitlement_aic_is_none_for_unlimited_or_empty_plans() {
        let plan = |bucket: &str| parse_plan(&serde_json::json!({"quota_snapshots": {"premium_interactions": serde_json::from_str::<Value>(bucket).unwrap()}}), 0);
        assert_eq!(plan(r#"{"entitlement": 300, "has_quota": true}"#).entitlement_aic(), Some(300.0));
        assert_eq!(plan(r#"{"unlimited": true, "entitlement": 300}"#).entitlement_aic(), None);
        assert_eq!(plan(r#"{"entitlement": 0, "has_quota": true}"#).entitlement_aic(), None);
        assert_eq!(parse_plan(&serde_json::json!({"login": "a"}), 0).entitlement_aic(), None);
    }
}
