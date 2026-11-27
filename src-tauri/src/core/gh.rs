use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

// Fixed locations only, never a gh found on the PATH.
pub const INSTALL_LOCATIONS: [&str; 3] = ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh"];

// Host pinned: GH_HOST can't redirect it.
pub const HOST: &str = "github.com";

pub const API_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub enum GhError {
    NotInstalled,
    SignedOut,
    SsoRequired,
    Offline,
    Http { status: u16, message: String },
    Failed(String),
}

impl std::fmt::Display for GhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GhError::NotInstalled => write!(f, "GitHub CLI (gh) is not installed"),
            GhError::SignedOut => write!(f, "GitHub CLI (gh) is not signed in"),
            GhError::SsoRequired => write!(f, "GitHub CLI (gh) sign-in is not authorised for your organisation's SSO"),
            GhError::Offline => write!(f, "Can't reach GitHub: check your internet connection"),
            GhError::Http { status, message } => write!(f, "GitHub returned {status}: {message}"),
            GhError::Failed(m) => write!(f, "GitHub CLI (gh) failed: {m}"),
        }
    }
}

impl GhError {
    pub fn explanation(&self) -> String {
        match self {
            GhError::NotInstalled => "GitHub CLI (gh) is not installed. Install it with brew install gh, then run gh auth login.".into(),
            GhError::SignedOut => "GitHub CLI (gh) is not signed in. Run gh auth login.".into(),
            GhError::Http { status: 401, .. } => "GitHub rejected the GitHub CLI's sign-in (401). Run gh auth login to sign in again.".into(),
            GhError::SsoRequired => {
                "Your GitHub CLI sign-in is not authorised for your organisation's SSO. Run gh auth refresh and authorise the organisation in the browser.".into()
            }
            other => other.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Gh {
    candidates: Vec<PathBuf>,
}

impl Default for Gh {
    fn default() -> Self {
        Gh::installed()
    }
}

impl Gh {
    pub fn installed() -> Gh {
        Gh { candidates: INSTALL_LOCATIONS.iter().map(PathBuf::from).collect() }
    }

    pub fn at(path: impl Into<PathBuf>) -> Gh {
        Gh { candidates: vec![path.into()] }
    }

    pub fn path(&self) -> Option<&Path> {
        self.candidates.iter().map(PathBuf::as_path).find(|p| p.is_absolute() && p.is_file())
    }

    pub async fn run(&self, args: &[&str], timeout: Duration) -> Result<Vec<u8>, GhError> {
        let path = self.path().ok_or(GhError::NotInstalled)?;
        let mut cmd = command(path, args);
        let out = match tokio::time::timeout(timeout, cmd.output()).await {
            Err(_) => return Err(GhError::Failed(format!("no answer after {} s", timeout.as_secs()))),
            Ok(Err(e)) => return Err(GhError::Failed(e.to_string())),
            Ok(Ok(out)) => out,
        };
        if out.status.success() {
            return Ok(out.stdout);
        }
        Err(classify(out.status.code(), &String::from_utf8_lossy(&out.stderr)))
    }

    pub async fn api(&self, endpoint: &str) -> Result<Value, GhError> {
        let args = [
            "api",
            "--hostname",
            HOST,
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2026-03-10",
            endpoint,
        ];
        let out = self.run(&args, API_TIMEOUT).await?;
        serde_json::from_slice(&out).map_err(|e| GhError::Failed(format!("unexpected response: {e}")))
    }
}

const CHILD_ENV_REMOVED: [&str; 6] = ["DEBUG", "GH_DEBUG", "GH_ENTERPRISE_TOKEN", "GH_HOST", "GH_PATH", "GH_REPO"];

fn command(path: &Path, args: &[&str]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
        .env("GH_SPINNER_DISABLED", "1")
        .env("NO_COLOR", "1");
    for var in CHILD_ENV_REMOVED {
        cmd.env_remove(var);
    }
    cmd
}

const SAML_ENFORCEMENT: &str = "protected by organization SAML enforcement";

// gh exits 4 without a sign-in; SSO refusals are 403 or status-less and carry the SAML enforcement message.
fn classify(code: Option<i32>, stderr: &str) -> GhError {
    if code == Some(4) || stderr.contains("gh auth login") {
        return GhError::SignedOut;
    }
    if stderr.contains("error connecting to") {
        return GhError::Offline;
    }
    let line = stderr.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("");
    let http = line.rsplit_once(" (HTTP ").and_then(|(msg, rest)| Some((msg, rest.strip_suffix(')')?.parse::<u16>().ok()?)));
    if matches!(http, None | Some((_, 403))) && line.contains(SAML_ENFORCEMENT) {
        return GhError::SsoRequired;
    }
    if let Some((msg, status)) = http {
        return GhError::Http { status, message: msg.strip_prefix("gh: ").unwrap_or(msg).to_string() };
    }
    let line = line.strip_prefix("gh: ").unwrap_or(line);
    GhError::Failed(if line.is_empty() { format!("exit status {}", code.map_or("unknown".into(), |c| c.to_string())) } else { line.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_locations_are_absolute_and_never_the_path() {
        assert_eq!(INSTALL_LOCATIONS, ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh"]);
        assert!(Gh::at("gh").path().is_none(), "a bare name would be a PATH lookup");
    }

    #[test]
    fn gh_runs_without_debug_or_redirect_variables_and_keeps_its_tokens() {
        use std::collections::HashMap;
        use std::ffi::OsStr;
        let cmd = command(Path::new("/usr/bin/gh"), &["--version"]);
        let envs: HashMap<&OsStr, Option<&OsStr>> = cmd.as_std().get_envs().collect();
        for var in CHILD_ENV_REMOVED {
            assert_eq!(envs.get(OsStr::new(var)), Some(&None), "{var} is removed from gh's environment");
        }
        for var in ["GH_TOKEN", "GITHUB_TOKEN"] {
            assert!(!envs.contains_key(OsStr::new(var)), "{var} is inherited, not overridden");
        }
    }

    #[test]
    fn saml_enforcement_is_sso_not_authorised() {
        let rest = "gh: Resource protected by organization SAML enforcement. You must grant your OAuth token access to this organization. (HTTP 403)";
        assert_eq!(classify(Some(1), rest), GhError::SsoRequired);
        let graphql = "GraphQL: Resource protected by organization SAML enforcement. You must grant your OAuth token access to this organization. (organization)";
        assert_eq!(classify(Some(1), graphql), GhError::SsoRequired);
    }

    #[test]
    fn other_errors_that_mention_sso_or_saml_are_not_sso_not_authorised() {
        let forbidden = "gh: Ask an SSO admin for access to this repository (HTTP 403)";
        assert_eq!(classify(Some(1), forbidden), GhError::Http { status: 403, message: "Ask an SSO admin for access to this repository".into() });
        let not_found = "gh: Not Found: SAML enforcement docs moved (HTTP 404)";
        assert_eq!(classify(Some(1), not_found), GhError::Http { status: 404, message: "Not Found: SAML enforcement docs moved".into() });
        assert_eq!(classify(Some(1), "gh: unknown flag --SSO"), GhError::Failed("unknown flag --SSO".into()));
    }
}
