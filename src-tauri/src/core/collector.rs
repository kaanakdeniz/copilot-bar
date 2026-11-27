use super::cli;
use super::gh::{Gh, GhError};
use super::github::{self, GitHubPlan};
use super::model::UsageRecord;
use super::settings::Settings;
use super::snapshot;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

pub use super::snapshot::{PlanError, SignIn, Snapshot, Status};

struct CachedFile {
    mtime: SystemTime,
    size: u64,
    records: Arc<[UsageRecord]>,
}

#[derive(Default)]
pub struct LocalCache {
    files: HashMap<PathBuf, CachedFile>,
}

pub struct LocalLoad {
    pub records: Vec<UsageRecord>,
    pub files: usize,
    pub changed: bool,
}

impl LocalCache {
    pub fn load(&mut self, settings: &Settings) -> LocalLoad {
        let cli_root = settings.cli_log_dir.as_deref().map(expand_home).unwrap_or_else(cli::default_root);
        let paths = if settings.sources.cli { cli::session_files(&cli_root) } else { vec![] };
        let files = paths.len();

        let mut changed = false;
        let mut parts: Vec<Arc<[UsageRecord]>> = Vec::with_capacity(files);
        let mut live = HashSet::new();
        for path in paths {
            let meta = std::fs::metadata(&path).ok();
            let mtime = meta.as_ref().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
            let size = meta.map(|m| m.len()).unwrap_or(0);
            match self.files.get(&path) {
                Some(cached) if cached.mtime == mtime && cached.size == size => parts.push(cached.records.clone()),
                _ => {
                    changed = true;
                    let records: Arc<[UsageRecord]> = cli::parse_file(&path).into();
                    parts.push(records.clone());
                    self.files.insert(path.clone(), CachedFile { mtime, size, records });
                }
            }
            live.insert(path);
        }
        let before = self.files.len();
        self.files.retain(|p, _| live.contains(p));
        changed |= self.files.len() != before;

        let mut records = vec![];
        let mut seen = HashSet::new();
        for part in &parts {
            for record in part.iter() {
                if seen.insert(&record.id) {
                    records.push(record.clone());
                }
            }
        }
        LocalLoad { records, files, changed }
    }
}

pub struct Collector {
    pub local: LocalCache,
    pub local_records: Vec<UsageRecord>,
    pub github_plan: Option<GitHubPlan>,
    pub status: Status,
    gh: Gh,
    copilot_config: PathBuf,
}

impl Default for Collector {
    fn default() -> Self {
        Collector::new(Gh::installed(), github::copilot_cli_config())
    }
}

impl Collector {
    pub fn new(gh: Gh, copilot_config: PathBuf) -> Self {
        Collector { local: LocalCache::default(), local_records: vec![], github_plan: None, status: Status::default(), gh, copilot_config }
    }

    pub fn refresh_local(&mut self, settings: &Settings) -> bool {
        let loaded = self.local.load(settings);
        self.local_records = loaded.records;
        self.status.cli_files = loaded.files;
        loaded.changed
    }

    pub fn begin_connecting(&mut self) {
        if self.github_plan.is_none() {
            self.status.connecting_since = Some(now_ms());
        }
    }

    pub fn plan_fetch(&self) -> PlanFetch {
        PlanFetch { gh: self.gh.clone(), copilot_config: self.copilot_config.clone() }
    }

    pub fn apply_plan(&mut self, outcome: PlanOutcome) {
        self.status.connecting_since = None;
        self.status.plan_error = None;
        match outcome {
            PlanOutcome::Fetched { plan, sign_in } => {
                self.status.sign_in = sign_in;
                self.github_plan = Some(plan);
            }
            PlanOutcome::Failed { sign_in, error, forget_plan } => {
                if forget_plan {
                    self.github_plan = None;
                }
                self.status.sign_in = sign_in;
                self.status.plan_error = Some(error);
            }
        }
        self.status.last_refresh = Some(now_ms());
    }

    pub async fn refresh(&mut self, settings: &Settings) {
        self.refresh_local(settings);
        let outcome = self.plan_fetch().run().await;
        self.apply_plan(outcome);
    }

    pub fn snapshot(&self, settings: &Settings) -> Snapshot {
        snapshot::build(self.local_records.clone(), self.github_plan.clone(), settings, self.status.clone(), now_ms())
    }
}

pub struct PlanFetch {
    gh: Gh,
    copilot_config: PathBuf,
}

pub enum PlanOutcome {
    Fetched { plan: GitHubPlan, sign_in: SignIn },
    Failed { sign_in: SignIn, error: PlanError, forget_plan: bool },
}

impl PlanFetch {
    pub async fn run(&self) -> PlanOutcome {
        match github::fetch(&self.gh).await {
            Ok(plan) => {
                let sign_in = match github::copilot_cli_login(&self.copilot_config) {
                    Some(c) if !c.eq_ignore_ascii_case(&plan.login) => SignIn::AccountMismatch { gh_login: plan.login.clone(), copilot_login: c },
                    _ => SignIn::SignedIn { login: plan.login.clone() },
                };
                PlanOutcome::Fetched { plan, sign_in }
            }
            Err(e) => {
                let (sign_in, message) = explain(&e);
                PlanOutcome::Failed {
                    forget_plan: matches!(sign_in, SignIn::GhMissing | SignIn::SignedOut),
                    sign_in,
                    error: PlanError { message, offline: e == GhError::Offline },
                }
            }
        }
    }
}

pub fn expand_home(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if p == "~" => dirs::home_dir().unwrap_or_default(),
        _ => PathBuf::from(p),
    }
}

fn explain(e: &GhError) -> (SignIn, String) {
    let sign_in = match e {
        GhError::NotInstalled => SignIn::GhMissing,
        GhError::SignedOut | GhError::Http { status: 401, .. } => SignIn::SignedOut,
        GhError::SsoRequired => SignIn::SsoRequired,
        other => SignIn::Error { message: other.to_string() },
    };
    (sign_in, e.explanation())
}

pub fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}
