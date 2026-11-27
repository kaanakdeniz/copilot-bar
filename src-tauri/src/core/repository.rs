
pub fn parse_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let path = ["https://github.com/", "http://github.com/", "git@github.com:", "ssh://git@github.com/"]
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    let valid = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    (parts.next().is_none() && valid(owner) && valid(name)).then(|| format!("{owner}/{name}"))
}

pub fn valid(repository: &str) -> Option<String> {
    parse_remote(&format!("https://github.com/{}", repository.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_remotes_in_every_form_give_owner_and_name() {
        for url in [
            "https://github.com/kaanakdeniz/copilot-bar",
            "https://github.com/kaanakdeniz/copilot-bar.git",
            "https://github.com/kaanakdeniz/copilot-bar/",
            "git@github.com:kaanakdeniz/copilot-bar.git",
            "ssh://git@github.com/kaanakdeniz/copilot-bar.git",
            "  https://github.com/kaanakdeniz/copilot-bar.git\n",
        ] {
            assert_eq!(parse_remote(url).as_deref(), Some("kaanakdeniz/copilot-bar"), "{url}");
        }
    }

    #[test]
    fn this_build_knows_the_repository_it_is_released_from() {
        let r = env!("COPILOTBAR_REPOSITORY");
        assert!(r.is_empty() || valid(r).as_deref() == Some(r), "{r:?} is neither empty nor an owner/name");
    }

    #[test]
    fn other_hosts_and_malformed_urls_give_nothing() {
        for url in ["https://gitlab.com/o/n.git", "https://github.com/only-owner", "https://github.com/o/n/extra", "", "/local/path"] {
            assert_eq!(parse_remote(url), None, "{url}");
        }
    }
}
