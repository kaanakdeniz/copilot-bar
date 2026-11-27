use crate::core::settings::ensure_data_dir;
use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

const MAX_BYTES: u64 = 256 * 1024;
const LOG_FILE: &str = "copilotbar.log";
const ROTATED_FILE: &str = "copilotbar.log.1";
const PRIVATE_FILE_MODE: u32 = 0o600;

pub fn line(msg: &str) {
    let Ok(dir) = ensure_data_dir() else { return };
    append_line(&dir, msg);
}

fn append_line(dir: &Path, msg: &str) {
    let path = dir.join(LOG_FILE);
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let rotated = dir.join(ROTATED_FILE);
        if fs::rename(&path, &rotated).is_ok() {
            let _ = fs::set_permissions(&rotated, Permissions::from_mode(PRIVATE_FILE_MODE));
        }
    }
    let Ok(mut f) = OpenOptions::new().create(true).append(true).mode(PRIVATE_FILE_MODE).open(&path) else { return };
    let _ = f.set_permissions(Permissions::from_mode(PRIVATE_FILE_MODE));
    let _ = writeln!(f, "{} {msg}", jiff::ZonedDateTime::now().strftime("%Y-%m-%d %H:%M:%S"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("copilotbar-log-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_log_left_world_readable_by_an_older_version_is_made_owner_only() {
        let dir = scratch_dir("perms");
        let path = dir.join(LOG_FILE);
        fs::write(&path, "old line\n").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();
        append_line(&dir, "fresh");
        assert_eq!(mode(&path), 0o600);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("old line\n") && text.ends_with(" fresh\n"), "{text}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_full_log_rotates_to_an_owner_only_file() {
        let dir = scratch_dir("rotate");
        let path = dir.join(LOG_FILE);
        fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();
        append_line(&dir, "after rotation");
        let rotated = dir.join(ROTATED_FILE);
        assert_eq!(mode(&rotated), 0o600);
        assert_eq!(fs::metadata(&rotated).unwrap().len(), MAX_BYTES + 1);
        assert_eq!(mode(&path), 0o600);
        assert!(fs::read_to_string(&path).unwrap().ends_with(" after rotation\n"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
