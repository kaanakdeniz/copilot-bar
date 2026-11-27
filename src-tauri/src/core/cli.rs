use super::model::{epoch_ms, nano_aic, uint, Client, SessionRef, Tokens, UsageRecord};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

pub const NANO_PER_CREDIT: f64 = 1_000_000_000.0;
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
const TRACKED_EVENT_TYPES: [&str; 7] = [
    "session.usage_record",
    "session.start",
    "session.resume",
    "session.usage_checkpoint",
    "session.shutdown",
    "session.model_change",
    "session.auto_mode_resolved",
];

pub fn default_root() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".copilot/session-state")
}

pub fn session_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return vec![] };
    entries
        .flatten()
        .map(|e| e.path().join("events.jsonl"))
        .filter(|p| p.is_file())
        .collect()
}

pub fn parse_file(path: &Path) -> Vec<UsageRecord> {
    let Ok(file) = File::open(path) else { return vec![] };
    let dir = path.parent().unwrap_or(Path::new(""));
    let mut ws = std::fs::read_to_string(dir.join("workspace.yaml"))
        .map(|y| WorkspaceYaml::parse(&y))
        .unwrap_or_default();
    if ws.id.is_none() {
        ws.id = dir.file_name().map(|s| s.to_string_lossy().to_string());
    }
    parse_reader(BufReader::new(file), &ws)
}

pub fn parse(text: &str, fallback_session: &str) -> Vec<UsageRecord> {
    let ws = Workspace { id: Some(fallback_session.to_string()), ..Default::default() };
    parse_reader(text.as_bytes(), &ws)
}

#[derive(Debug, Default, Clone)]
struct Workspace {
    id: Option<String>,
    cwd: Option<String>,
    name: Option<String>,
    client_name: Option<String>,
}

struct WorkspaceYaml;

impl WorkspaceYaml {
    fn parse(yaml: &str) -> Workspace {
        let mut fields: HashMap<String, String> = HashMap::new();
        let mut lines = yaml.lines().peekable();
        while let Some(line) = lines.next() {
            if line.starts_with(' ') || line.starts_with('\t') {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else { continue };
            let mut value = value.trim().to_string();
            if value.starts_with('|') || value.starts_with('>') {
                let mut parts = vec![];
                while let Some(next) = lines.peek().filter(|l| l.starts_with(' ') || l.trim().is_empty()) {
                    parts.push(next.trim().to_string());
                    lines.next();
                }
                value = parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
            } else if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value = value[1..value.len() - 1].to_string();
            }
            fields.insert(key.trim().to_string(), value);
        }
        let get = |k: &str| fields.get(k).filter(|v| !v.is_empty()).cloned();
        Workspace { id: get("id"), cwd: get("cwd"), name: get("name"), client_name: get("client_name") }
    }
}

const CLIENTS: &[(&str, Client)] = &[
    ("github/cli", Client::CopilotCli),
    ("sdk", Client::CopilotCli),
    ("vscode-agent-host", Client::VsCode),
    ("github/autopilot", Client::CopilotApp),
];

fn client_of(client_name: Option<&str>) -> Client {
    client_name.and_then(|n| CLIENTS.iter().find(|(k, _)| *k == n)).map_or(Client::CopilotCli, |(_, c)| *c)
}

const APP_SCRATCH_FOLDERS: &str = "/.copilot/chats/";

fn project_of(cwd: &str) -> Option<String> {
    if cwd.contains(APP_SCRATCH_FOLDERS) {
        return None;
    }
    Path::new(cwd).file_name().map(|s| s.to_string_lossy().to_string())
}

struct SessionState<'a> {
    ws: &'a Workspace,
    session: String,
    cwd: Option<String>,
}

impl<'a> SessionState<'a> {
    fn new(ws: &'a Workspace) -> Self {
        SessionState { ws, session: ws.id.clone().unwrap_or_default(), cwd: None }
    }

    fn start(&mut self, payload: &Value) {
        if let Some(s) = payload["sessionId"].as_str() {
            self.session = s.to_string();
        }
        if let Some(c) = payload["context"]["cwd"].as_str() {
            self.cwd = Some(c.to_string());
        }
    }

    fn client(&self) -> Client {
        client_of(self.ws.client_name.as_deref())
    }

    fn tag(&self, r: &mut UsageRecord) {
        r.project = self.cwd.as_deref().or(self.ws.cwd.as_deref()).and_then(project_of);
        r.session = Some(SessionRef { id: self.session.clone(), label: self.ws.name.clone() });
    }
}

enum Line {
    Complete,
    Oversized,
}

fn read_line<R: BufRead>(reader: &mut R, buf: &mut Vec<u8>) -> io::Result<Option<Line>> {
    buf.clear();
    let mut oversized = false;
    let mut read_any = false;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            break;
        }
        read_any = true;
        let newline = chunk.iter().position(|&b| b == b'\n');
        let body = &chunk[..newline.unwrap_or(chunk.len())];
        if !oversized {
            if buf.len() + body.len() > MAX_LINE_BYTES {
                oversized = true;
                buf.clear();
            } else {
                buf.extend_from_slice(body);
            }
        }
        let consumed = newline.map_or(chunk.len(), |i| i + 1);
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }
    Ok(read_any.then_some(if oversized { Line::Oversized } else { Line::Complete }))
}

fn for_each_event<R: BufRead>(mut reader: R, mut visit: impl FnMut(usize, Value)) {
    let mut buf = Vec::new();
    let mut line_no = 0;
    while let Ok(Some(line)) = read_line(&mut reader, &mut buf) {
        line_no += 1;
        if matches!(line, Line::Oversized) {
            continue;
        }
        let text = String::from_utf8_lossy(&buf);
        if !TRACKED_EVENT_TYPES.iter().any(|t| text.contains(*t)) {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<Value>(&text) {
            visit(line_no, event);
        }
    }
}

fn parse_reader<R: BufRead>(reader: R, ws: &Workspace) -> Vec<UsageRecord> {
    let mut legacy = LegacyParser::new(ws);
    let mut per_call = PerCallParser::new(ws);
    let mut seen_usage = false;
    for_each_event(reader, |line_no, event| {
        if event["type"] == "session.usage_record" {
            seen_usage = true;
        }
        if !seen_usage {
            legacy.feed(&event);
        }
        per_call.feed(line_no, &event);
    });
    let mut out = legacy.finish();
    out.extend(per_call.finish());
    out
}

fn ts(e: &Value) -> i64 {
    e["timestamp"].as_str().and_then(epoch_ms).unwrap_or(0)
}

struct PerCallParser<'a> {
    ctx: SessionState<'a>,
    seen: HashSet<String>,
    out: Vec<UsageRecord>,
}

impl<'a> PerCallParser<'a> {
    fn new(ws: &'a Workspace) -> Self {
        PerCallParser { ctx: SessionState::new(ws), seen: HashSet::new(), out: vec![] }
    }

    fn feed(&mut self, line_no: usize, event: &Value) {
        match event["type"].as_str() {
            Some("session.start") => self.ctx.start(&event["data"]),
            Some("session.usage_record") => self.record(line_no, event),
            _ => {}
        }
    }

    fn record(&mut self, line_no: usize, event: &Value) {
        let u = &event["data"]["usage"];
        if !u.is_object() {
            return;
        }
        // providerCallId is shared by every call on one connection, so it can't identify a call.
        let usage_id = u["accounting"]["usageId"]
            .as_str()
            .or(u["serviceRequestId"].as_str())
            .or(u["apiCallId"].as_str())
            .or(event["id"].as_str());
        let key = match usage_id.filter(|id| !id.is_empty()) {
            Some(id) if !self.seen.insert(id.to_owned()) => return,
            Some(id) => id.to_owned(),
            None => format!("line:{line_no}"),
        };
        let model = u["model"].as_str().or(u["responseModel"].as_str()).unwrap_or("unknown");
        let mut r = UsageRecord::new(format!("log:{}:{key}", self.ctx.session), ts(event), self.ctx.client(), model);
        r.tokens = Tokens::from_usage(u);
        r.aic = Some(nano_aic(u["copilotUsage"].get("totalNanoAiu")).unwrap_or(0.0) / NANO_PER_CREDIT);
        self.ctx.tag(&mut r);
        r.initiator = u["initiator"].as_str().map(String::from);
        r.reasoning_effort = u["reasoningEffort"].as_str().map(String::from);
        r.is_auto = u["isAuto"].as_bool();
        self.out.push(r);
    }

    fn finish(self) -> Vec<UsageRecord> {
        self.out
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
struct Metric {
    tokens: Tokens,
    nano: f64,
    requests: u64,
}

impl Metric {
    fn from_shutdown_model(m: &Value) -> Metric {
        Metric {
            tokens: Tokens::from_usage(&m["usage"]),
            nano: nano_aic(m.get("totalNanoAiu")).unwrap_or(0.0),
            requests: uint(m["requests"].get("count")),
        }
    }

    fn is_empty(&self) -> bool {
        self.tokens.is_zero() && self.nano == 0.0 && self.requests == 0
    }
}

fn is_cumulative(current: &HashMap<String, Metric>, previous: &HashMap<String, Metric>) -> bool {
    !previous.is_empty()
        && previous.iter().all(|(k, p)| {
            current.get(k).is_some_and(|c| c.nano >= p.nano && c.tokens.input >= p.tokens.input && c.requests >= p.requests)
        })
}

fn decumulate(current: &HashMap<String, Metric>, previous: &HashMap<String, Metric>) -> Vec<(String, Metric)> {
    let cumulative = is_cumulative(current, previous);
    let mut names: Vec<&String> = current.keys().collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let c = &current[name];
            let m = match (cumulative, previous.get(name)) {
                (true, Some(p)) => Metric {
                    tokens: c.tokens.minus(&p.tokens),
                    nano: c.nano - p.nano,
                    requests: c.requests.saturating_sub(p.requests),
                },
                _ => c.clone(),
            };
            (name.clone(), m)
        })
        .collect()
}

struct LegacyParser<'a> {
    ctx: SessionState<'a>,
    model: String,
    last_checkpoint: f64,
    pending: Vec<UsageRecord>,
    segment_has_shutdown: bool,
    previous: HashMap<String, Metric>,
    out: Vec<UsageRecord>,
}

impl<'a> LegacyParser<'a> {
    fn new(ws: &'a Workspace) -> Self {
        LegacyParser {
            ctx: SessionState::new(ws),
            model: "unknown".to_string(),
            last_checkpoint: 0.0,
            pending: vec![],
            segment_has_shutdown: false,
            previous: HashMap::new(),
            out: vec![],
        }
    }

    fn feed(&mut self, event: &Value) {
        let payload = &event["data"];
        let id = event["id"].as_str().unwrap_or("");
        match event["type"].as_str() {
            Some("session.start") | Some("session.resume") => {
                if !self.segment_has_shutdown {
                    self.out.append(&mut self.pending);
                }
                self.pending.clear();
                self.last_checkpoint = 0.0;
                self.segment_has_shutdown = false;
                self.ctx.start(payload);
            }
            Some("session.model_change") => {
                if let Some(m) = payload["newModel"].as_str().filter(|m| !m.is_empty()) {
                    self.model = m.to_string();
                }
            }
            Some("session.auto_mode_resolved") => {
                if let Some(m) = payload["chosenModel"].as_str().filter(|m| !m.is_empty()) {
                    self.model = m.to_string();
                }
            }
            Some("session.usage_checkpoint") => {
                let Some(total) = nano_aic(payload.get("totalNanoAiu")) else { return };
                let restarted = total < self.last_checkpoint;
                let delta = if restarted { total } else { total - self.last_checkpoint };
                self.last_checkpoint = total;
                if delta > 0.0 {
                    let mut r =
                        UsageRecord::new(format!("log:{}:{id}", self.ctx.session), ts(event), self.ctx.client(), &self.model);
                    r.aic = Some(delta / NANO_PER_CREDIT);
                    r.requests = 0;
                    r.legacy = true;
                    self.ctx.tag(&mut r);
                    self.pending.push(r);
                }
            }
            Some("session.shutdown") => {
                self.segment_has_shutdown = true;
                self.pending.clear();
                let Some(metrics) = payload["modelMetrics"].as_object() else { return };
                let current: HashMap<String, Metric> =
                    metrics.iter().map(|(name, m)| (name.clone(), Metric::from_shutdown_model(m))).collect();
                for (name, m) in decumulate(&current, &self.previous) {
                    if m.is_empty() {
                        continue;
                    }
                    let mut r =
                        UsageRecord::new(format!("log:{}:{id}:{name}", self.ctx.session), ts(event), self.ctx.client(), &name);
                    r.tokens = m.tokens;
                    r.aic = Some(m.nano / NANO_PER_CREDIT);
                    r.requests = m.requests as u32;
                    r.legacy = true;
                    self.ctx.tag(&mut r);
                    self.out.push(r);
                }
                self.previous = current;
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<UsageRecord> {
        if !self.segment_has_shutdown {
            self.out.append(&mut self.pending);
        }
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn parse_bytes(bytes: &[u8]) -> Vec<UsageRecord> {
        parse_reader(bytes, &Workspace { id: Some("x".into()), ..Default::default() })
    }

    fn usage_record_line(usage_id: &str, nano: &str) -> String {
        format!(
            r#"{{"type":"session.usage_record","data":{{"usage":{{"model":"gpt-5.6-luna","inputTokens":10,"accounting":{{"usageId":"{usage_id}"}},"copilotUsage":{{"totalNanoAiu":{nano}}}}}}},"id":"e-{usage_id}","timestamp":"2026-11-20T10:00:05.000Z"}}"#
        )
    }

    fn padded_usage_line(usage_id: &str, pad: usize) -> String {
        format!(
            r#"{{"type":"session.usage_record","data":{{"usage":{{"accounting":{{"usageId":"{usage_id}"}},"copilotUsage":{{"totalNanoAiu":100000000}}}},"pad":"{}"}}}}"#,
            "x".repeat(pad)
        )
    }

    #[test]
    fn per_call_records_are_used_and_deduplicated() {
        let recs = parse(&fixture("cli-usage-records.jsonl"), "x");
        assert_eq!(recs.len(), 2);
        let total: f64 = recs.iter().map(|r| r.aic.unwrap()).sum();
        assert!((total - 0.4).abs() < 1e-9);
        let r = &recs[0];
        assert_eq!(r.model, "gpt-5.6-luna");
        assert_eq!(r.tokens, Tokens { input: 1000, output: 50, cache_read: 800, cache_write: 100, reasoning: 10 });
        assert_eq!(r.initiator.as_deref(), Some("user"));
        assert_eq!(r.is_auto, Some(true));
        assert_eq!(r.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(r.project.as_deref(), Some("demo"));
        assert_eq!(r.session.as_ref().map(|s| s.id.as_str()), Some("s1"));
        assert_eq!(recs[1].model, "claude-sonnet-4.6");
    }

    fn parse_session(id: &str) -> Vec<UsageRecord> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session-state");
        assert!(session_files(&root).contains(&root.join(id).join("events.jsonl")));
        parse_file(&root.join(id).join("events.jsonl"))
    }

    #[test]
    fn two_sessions_in_the_same_project_keep_their_own_session() {
        let a = parse_session("aaaa1111");
        let b = parse_session("bbbb2222");
        assert_eq!(a.len(), 2);
        assert_eq!(b.len(), 1);
        for r in a.iter().chain(&b) {
            assert_eq!(r.project.as_deref(), Some("atlas"));
        }
        let sa = a[0].session.as_ref().unwrap();
        assert_eq!(sa.id, "aaaa1111");
        assert_eq!(sa.label.as_deref(), Some("Fix the flaky login test"));
        assert_eq!(a[1].session, a[0].session);
        // No cwd in the session context: the project comes from workspace.yaml, and the name is a block scalar.
        let sb = b[0].session.as_ref().unwrap();
        assert_eq!(sb.id, "bbbb2222");
        assert_eq!(sb.label.as_deref(), Some("Add retries to the upload client"));
    }

    #[test]
    fn copilot_app_scratch_folder_is_not_a_project() {
        let c = parse_session("cccc3333");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].project, None);
        let s = c[0].session.as_ref().unwrap();
        assert_eq!(s.id, "cccc3333");
        assert_eq!(s.label, None);
    }

    #[test]
    fn client_comes_from_workspace_client_name() {
        for id in ["aaaa1111", "dddd4444", "eeee5555", "ffff6666"] {
            let recs = parse_session(id);
            assert!(!recs.is_empty(), "{id}");
            assert!(recs.iter().all(|r| r.client == Client::CopilotCli), "{id}");
        }
    }

    #[test]
    fn vscode_copilot_harness_sessions_are_vs_code() {
        let recs = parse_session("vscode01");
        assert_eq!(recs.len(), 4);
        assert!(recs.iter().all(|r| r.client == Client::VsCode));
        assert!(recs.iter().all(|r| r.project.as_deref() == Some("atlas")));
        assert_eq!(recs[0].session.as_ref().unwrap().label.as_deref(), Some("Pick the strongest model"));
        assert_eq!(recs[0].model, "mai-code-2-flash");
        assert_eq!(recs[0].tokens.input, 18400);
        let total: f64 = recs.iter().map(|r| r.aic.unwrap()).sum();
        assert!((total - 0.555984).abs() < 1e-9, "{total}");
    }

    #[test]
    fn copilot_app_sessions_are_the_github_copilot_app() {
        let recs = parse_session("app00001");
        assert_eq!(recs.len(), 2);
        assert!(recs.iter().all(|r| r.client == Client::CopilotApp));
        assert!(recs.iter().all(|r| r.project.is_none()));
        assert_eq!(recs[0].session.as_ref().unwrap().label.as_deref(), Some("Say hello"));
        assert_eq!(recs.iter().map(|r| r.initiator.as_deref()).collect::<Vec<_>>(), vec![Some("user"), Some("agent")]);
    }

    #[test]
    fn each_session_is_read_once_from_its_events_log() {
        // session-store.db mirrors some of these Sessions, so it is never read.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session-state");
        let mut ids: Vec<String> = session_files(&root)
            .iter()
            .map(|p| {
                assert_eq!(p.file_name().unwrap(), "events.jsonl");
                p.parent().unwrap().file_name().unwrap().to_string_lossy().to_string()
            })
            .collect();
        ids.sort();
        assert_eq!(ids, ["aaaa1111", "app00001", "bbbb2222", "cccc3333", "dddd4444", "eeee5555", "ffff6666", "vscode01"]);
        let recs: Vec<UsageRecord> = session_files(&root).iter().flat_map(|p| parse_file(p)).collect();
        let unique: HashSet<&str> = recs.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(unique.len(), recs.len());
    }

    #[test]
    fn record_ids_name_no_client() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session-state");
        let recs: Vec<UsageRecord> = session_files(&root).iter().flat_map(|p| parse_file(p)).collect();
        assert!(recs.iter().any(|r| r.client != Client::CopilotCli));
        for r in &recs {
            assert!(r.id.starts_with("log:"), "{}", r.id);
        }
        assert!(parse(&fixture("cli-legacy.jsonl"), "legacy").iter().all(|r| r.id.starts_with("log:")));
    }

    #[test]
    fn client_names_map_through_one_table() {
        assert_eq!(client_of(Some("github/cli")), Client::CopilotCli);
        assert_eq!(client_of(Some("sdk")), Client::CopilotCli);
        assert_eq!(client_of(Some("vscode-agent-host")), Client::VsCode);
        assert_eq!(client_of(Some("github/autopilot")), Client::CopilotApp);
        assert_eq!(client_of(Some("acme/some-new-client")), Client::CopilotCli);
        assert_eq!(client_of(None), Client::CopilotCli);
    }

    #[test]
    fn legacy_shutdowns_are_decumulated_and_checkpoints_fill_gaps() {
        let recs = parse(&fixture("cli-legacy.jsonl"), "legacy");
        let total: f64 = recs.iter().map(|r| r.aic.unwrap()).sum();
        assert!((total - 0.21).abs() < 1e-9, "{total}");
        let opus: Vec<_> = recs.iter().filter(|r| r.model == "claude-opus-4.5").collect();
        assert_eq!(opus.iter().map(|r| r.tokens.input).collect::<Vec<_>>(), vec![500, 300]);
        assert_eq!(opus.iter().map(|r| r.requests).collect::<Vec<_>>(), vec![2, 1]);
        let luna: Vec<_> = recs.iter().filter(|r| r.model == "gpt-5.6-luna").collect();
        assert_eq!(luna.len(), 2);
        assert!((luna.iter().map(|r| r.aic.unwrap()).sum::<f64>() - 0.05).abs() < 1e-9);
    }

    #[test]
    fn only_legacy_totals_are_marked_legacy() {
        let legacy = parse(&fixture("cli-legacy.jsonl"), "legacy");
        assert!(!legacy.is_empty());
        assert!(legacy.iter().all(|r| r.legacy));
        let per_call = parse(&fixture("cli-usage-records.jsonl"), "x");
        assert!(!per_call.is_empty());
        assert!(per_call.iter().all(|r| !r.legacy));
    }

    #[test]
    fn legacy_usage_before_the_first_per_call_record_is_kept() {
        let recs = parse(&fixture("cli-mixed.jsonl"), "mixed");
        assert_eq!(recs.len(), 2);
        let legacy = &recs[0];
        assert_eq!(legacy.id, "log:mixed:m3:claude-opus-4.5");
        assert_eq!(legacy.model, "claude-opus-4.5");
        assert_eq!(legacy.aic, Some(5.0));
        assert_eq!(legacy.requests, 4);
        assert_eq!(legacy.tokens.input, 900);
        let per_call = &recs[1];
        assert_eq!(per_call.id, "log:mixed:u9");
        assert_eq!(per_call.model, "gpt-5.6-luna");
        assert_eq!(per_call.aic, Some(0.15));
        assert_eq!(per_call.session.as_ref().map(|s| s.id.as_str()), Some("mixed"));
        assert_eq!(per_call.project.as_deref(), Some("demo"));
        let total: f64 = recs.iter().map(|r| r.aic.unwrap()).sum();
        assert!((total - 5.15).abs() < 1e-9, "{total}");
    }

    #[test]
    fn invalid_utf8_inside_a_string_keeps_the_record() {
        let mut bytes = usage_record_line("a", "100000000").into_bytes();
        bytes.extend_from_slice(b"\n{\"type\":\"session.usage_record\",\"data\":{\"usage\":{\"note\":\"caf\xff\",\"model\":\"gpt-5.6-luna\",\"accounting\":{\"usageId\":\"b\"},\"copilotUsage\":{\"totalNanoAiu\":200000000}}},\"id\":\"e-b\"}\n");
        bytes.extend_from_slice(usage_record_line("c", "300000000").as_bytes());
        bytes.push(b'\n');
        let recs = parse_bytes(&bytes);
        assert_eq!(recs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["log:x:a", "log:x:b", "log:x:c"]);
        assert_eq!(recs.iter().map(|r| r.aic).collect::<Vec<_>>(), [Some(0.1), Some(0.2), Some(0.3)]);
    }

    #[test]
    fn invalid_utf8_in_a_structural_position_loses_only_that_line() {
        let mut bytes = usage_record_line("a", "100000000").into_bytes();
        bytes.extend_from_slice(b"\n{\"type\":\"session.usage_record\xff\",\"data\":{\"usage\":{\"accounting\":{\"usageId\":\"b\"}}}}\n");
        bytes.extend_from_slice(usage_record_line("c", "300000000").as_bytes());
        bytes.push(b'\n');
        let recs = parse_bytes(&bytes);
        assert_eq!(recs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["log:x:a", "log:x:c"]);
        assert_eq!(recs.iter().map(|r| r.aic).collect::<Vec<_>>(), [Some(0.1), Some(0.3)]);
    }

    #[test]
    fn truncated_final_line_loses_only_that_line() {
        let text = format!(
            "{}\n{}\n{}",
            usage_record_line("a", "100000000"),
            usage_record_line("b", "200000000"),
            r#"{"type":"session.usage_record","data":{"usage":{"model":"gpt-5.6-luna","copilotUsage":{"totalNanoAiu":30"#
        );
        let recs = parse_bytes(text.as_bytes());
        assert_eq!(recs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["log:x:a", "log:x:b"]);
        assert_eq!(recs.iter().map(|r| r.aic).collect::<Vec<_>>(), [Some(0.1), Some(0.2)]);
    }

    #[test]
    fn usage_records_without_any_id_get_their_line_number() {
        let no_id = r#"{"type":"session.usage_record","data":{"usage":{"model":"gpt-5.6-luna","copilotUsage":{"totalNanoAiu":100000000}}}}"#;
        let recs = parse_bytes(format!("{no_id}\n{no_id}\n").as_bytes());
        assert_eq!(recs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["log:x:line:1", "log:x:line:2"]);
    }

    #[test]
    fn non_finite_and_negative_aic_counts_as_zero() {
        let text = [
            usage_record_line("nan", r#""NaN""#),
            usage_record_line("inf", r#""inf""#),
            usage_record_line("neg", "-1"),
            usage_record_line("ok", "150000000"),
        ]
        .join("\n");
        let recs = parse_bytes(text.as_bytes());
        assert_eq!(recs.iter().map(|r| r.aic).collect::<Vec<_>>(), [Some(0.0), Some(0.0), Some(0.0), Some(0.15)]);
    }

    #[test]
    fn non_finite_legacy_checkpoints_are_ignored() {
        let text = [
            r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":"NaN"},"id":"c1"}"#,
            r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":-1},"id":"c2"}"#,
            r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":"50000000"},"id":"c3"}"#,
        ]
        .join("\n");
        let recs = parse_bytes(text.as_bytes());
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, "log:x:c3");
        assert_eq!(recs[0].aic, Some(0.05));
    }

    #[test]
    fn lines_over_four_mebibytes_are_skipped() {
        let base = padded_usage_line("bbbbb", 0).len();
        let exact = padded_usage_line("aaaaa", MAX_LINE_BYTES - base);
        assert_eq!(exact.len(), MAX_LINE_BYTES);
        let over = padded_usage_line("bbbbb", MAX_LINE_BYTES - base + 1);
        let text = format!("{exact}\n{over}\n{}\n", usage_record_line("after", "200000000"));
        let recs = parse_bytes(text.as_bytes());
        assert_eq!(recs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["log:x:aaaaa", "log:x:after"]);
    }

    #[test]
    fn decumulate_subtracts_the_previous_shutdown_when_totals_grew() {
        let metric = |input, nano, requests| Metric {
            tokens: Tokens { input, ..Default::default() },
            nano,
            requests,
        };
        let previous = HashMap::from([("opus".to_string(), metric(500, 100.0, 2))]);
        let current = HashMap::from([("opus".to_string(), metric(800, 160.0, 3))]);
        assert_eq!(decumulate(&current, &previous), vec![("opus".to_string(), metric(300, 60.0, 1))]);
    }

    #[test]
    fn decumulate_keeps_raw_totals_when_a_model_went_backwards_or_is_new() {
        let metric = |input, nano, requests| Metric {
            tokens: Tokens { input, ..Default::default() },
            nano,
            requests,
        };
        let previous = HashMap::from([("opus".to_string(), metric(500, 100.0, 2))]);
        let restarted = HashMap::from([("opus".to_string(), metric(50, 10.0, 1))]);
        assert_eq!(decumulate(&restarted, &previous), vec![("opus".to_string(), metric(50, 10.0, 1))]);

        let current = HashMap::from([
            ("sonnet".to_string(), metric(7, 3.0, 1)),
            ("opus".to_string(), metric(800, 160.0, 3)),
        ]);
        let names: Vec<String> = decumulate(&current, &HashMap::new()).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["opus", "sonnet"]);
        assert_eq!(decumulate(&current, &HashMap::new())[1].1, metric(7, 3.0, 1));
    }
}
