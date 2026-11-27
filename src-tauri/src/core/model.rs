use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Client {
    #[serde(rename = "cli")]
    CopilotCli,
    #[serde(rename = "vscode")]
    VsCode,
    #[serde(rename = "app")]
    CopilotApp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: u64,
}

impl Tokens {
    pub fn from_usage(usage: &Value) -> Tokens {
        Tokens {
            input: uint(usage.get("inputTokens")),
            output: uint(usage.get("outputTokens")),
            cache_read: uint(usage.get("cacheReadTokens")),
            cache_write: uint(usage.get("cacheWriteTokens")),
            reasoning: uint(usage.get("reasoningTokens")),
        }
    }

    pub fn is_zero(&self) -> bool {
        *self == Tokens::default()
    }

    pub fn minus(&self, o: &Tokens) -> Tokens {
        Tokens {
            input: self.input.saturating_sub(o.input),
            output: self.output.saturating_sub(o.output),
            cache_read: self.cache_read.saturating_sub(o.cache_read),
            cache_write: self.cache_write.saturating_sub(o.cache_write),
            reasoning: self.reasoning.saturating_sub(o.reasoning),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRef {
    pub id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    pub id: String,
    pub ts: i64,
    pub client: Client,
    pub model: String,
    pub tokens: Tokens,
    pub aic: Option<f64>,
    pub requests: u32,
    pub project: Option<String>,
    pub session: Option<SessionRef>,
    pub initiator: Option<String>,
    pub reasoning_effort: Option<String>,
    pub is_auto: Option<bool>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub legacy: bool,
}

impl UsageRecord {
    pub fn new(id: String, ts: i64, client: Client, model: &str) -> Self {
        UsageRecord {
            id,
            ts,
            client,
            model: normalize_model(model),
            tokens: Tokens::default(),
            aic: None,
            requests: 1,
            project: None,
            session: None,
            initiator: None,
            reasoning_effort: None,
            is_auto: None,
            legacy: false,
        }
    }
}

const MODEL_NAME_START: &str = "oswe";
static DATED: Lazy<Regex> = Lazy::new(|| Regex::new(r"-(\d{8}|\d{4}-\d{2}-\d{2})$").unwrap());
static DASHED_VERSION: Lazy<Regex> = Lazy::new(|| Regex::new(r"-(\d)-(\d)$").unwrap());

pub fn normalize_model(raw: &str) -> String {
    let mut s = raw.trim().to_lowercase();
    if let Some(rest) = s.strip_prefix("copilot/") {
        s = rest.to_string();
    }
    s = s.replace(' ', "-");
    if s.starts_with("capi-") {
        if let Some(i) = s.find(MODEL_NAME_START) {
            s = s[i..].to_string();
        }
    }
    s = DATED.replace(&s, "").to_string();
    s = DASHED_VERSION.replace(&s, "-$1.$2").to_string();
    if s.is_empty() {
        "unknown".into()
    } else {
        s
    }
}

pub fn epoch_ms(s: &str) -> Option<i64> {
    s.parse::<jiff::Timestamp>().ok().map(|t| t.as_millisecond())
}

pub fn num(v: Option<&Value>) -> Option<f64> {
    let f = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.parse().ok()?,
        _ => return None,
    };
    f.is_finite().then_some(f)
}

pub fn nano_aic(v: Option<&Value>) -> Option<f64> {
    num(v).filter(|n| *n >= 0.0)
}

pub fn uint(v: Option<&Value>) -> u64 {
    num(v).map(|f| f.max(0.0) as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_json_carries_client_not_source() {
        let r = UsageRecord::new("a".into(), 1, Client::CopilotCli, "m");
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["client"], "cli");
        let v = serde_json::to_value(UsageRecord::new("b".into(), 1, Client::VsCode, "m")).unwrap();
        assert_eq!(v["client"], "vscode");
        let v = serde_json::to_value(UsageRecord::new("c".into(), 1, Client::CopilotApp, "m")).unwrap();
        assert_eq!(v["client"], "app");
        assert!(v.get("source").is_none());
    }

    #[test]
    fn legacy_flag_is_written_only_when_set() {
        let per_call = serde_json::to_value(UsageRecord::new("a".into(), 1, Client::CopilotCli, "m")).unwrap();
        assert!(per_call.get("legacy").is_none());
        let mut legacy = UsageRecord::new("b".into(), 1, Client::CopilotCli, "m");
        legacy.legacy = true;
        let v = serde_json::to_value(&legacy).unwrap();
        assert_eq!(v["legacy"], true);
        let back: UsageRecord = serde_json::from_value(v).unwrap();
        assert!(back.legacy);
        let old: UsageRecord = serde_json::from_value(per_call).unwrap();
        assert!(!old.legacy);
    }

    #[test]
    fn parses_rfc3339_timestamps() {
        assert_eq!(epoch_ms("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(epoch_ms("2026-11-01T02:00:00.250+02:00"), epoch_ms("2026-11-01T00:00:00.250Z"));
        assert_eq!(epoch_ms("2026-11-01T00:00:00.123456789Z").map(|m| m % 1000), Some(123));
        assert_eq!(epoch_ms("2026-11-01"), None);
        assert_eq!(epoch_ms("nope"), None);
    }

    #[test]
    fn num_rejects_non_finite_values() {
        for raw in ["NaN", "inf", "-inf", "infinity"] {
            assert_eq!(num(Some(&Value::String(raw.into()))), None, "{raw}");
        }
        assert_eq!(num(Some(&Value::String("2.5".into()))), Some(2.5));
        assert_eq!(num(Some(&serde_json::json!(-1))), Some(-1.0));
    }

    #[test]
    fn nano_aic_rejects_negative_and_non_finite_values() {
        assert_eq!(nano_aic(Some(&serde_json::json!(-1))), None);
        assert_eq!(nano_aic(Some(&Value::String("-0.5".into()))), None);
        assert_eq!(nano_aic(Some(&Value::String("NaN".into()))), None);
        assert_eq!(nano_aic(Some(&Value::String("inf".into()))), None);
        assert_eq!(nano_aic(Some(&serde_json::json!(0))), Some(0.0));
        assert_eq!(nano_aic(Some(&serde_json::json!(150000000))), Some(150000000.0));
    }

    #[test]
    fn tokens_are_read_from_usage_keys() {
        let usage = serde_json::json!({
            "inputTokens": 1000, "outputTokens": "50", "cacheReadTokens": -4,
            "cacheWriteTokens": "NaN", "reasoningTokens": 10, "unrelated": 99
        });
        assert_eq!(
            Tokens::from_usage(&usage),
            Tokens { input: 1000, output: 50, cache_read: 0, cache_write: 0, reasoning: 10 }
        );
        assert_eq!(Tokens::from_usage(&Value::Null), Tokens::default());
    }

    #[test]
    fn normalizes_model_names() {
        for (raw, want) in [
            ("copilot/claude-sonnet-4.6", "claude-sonnet-4.6"),
            ("claude-sonnet-4-6", "claude-sonnet-4.6"),
            ("claude-haiku-4-5-20251001", "claude-haiku-4.5"),
            ("gpt-5.4-2026-03-05", "gpt-5.4"),
            ("Claude Sonnet 4.6", "claude-sonnet-4.6"),
            ("capi-noe-ptuc-h200-oswe-vscode-prime", "oswe-vscode-prime"),
            ("", "unknown"),
        ] {
            assert_eq!(normalize_model(raw), want, "{raw}");
        }
    }
}
