use base64::Engine;
use minisign_verify::{Error, PublicKey, Signature};

pub const UPDATE_PUBLIC_KEY: &str = env!("COPILOTBAR_UPDATE_PUBKEY");

// Fail-closed: anything but a valid signature by exactly this key is an error.
pub fn verify(public_key: &str, data: &[u8], signature_file: &[u8]) -> Result<String, String> {
    let key = unwrap(public_key.as_bytes()).ok_or("can't be checked: the update key built into this copy of CopilotBar is malformed")?;
    let key = PublicKey::decode(&key).map_err(|_| "can't be checked: the update key built into this copy of CopilotBar is malformed")?;
    let sig = unwrap(signature_file).ok_or("isn't a valid signature file")?;
    let sig = Signature::decode(&sig).map_err(|_| "isn't a valid signature file")?;
    // Prehashed only: legacy minisign signatures are refused.
    key.verify(data, &sig, false).map_err(|e| match e {
        Error::UnexpectedKeyId => "was made with a different key than CopilotBar's update key".to_string(),
        Error::UnexpectedAlgorithm | Error::UnsupportedLegacyMode => "uses a legacy format CopilotBar doesn't accept".to_string(),
        _ => "doesn't match the archive: it may have been tampered with".to_string(),
    })?;
    Ok(sig.trusted_comment().to_string())
}

// Covered by the signature, so the archive is bound to this version.
pub fn signed_version(trusted_comment: &str) -> Option<&str> {
    trusted_comment.split('\t').find_map(|field| field.strip_prefix("version:"))
}

/// Tauri's base64 wrapping of a minisign text file.
fn unwrap(b64: &[u8]) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim_ascii()).ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_key_is_a_minisign_public_key() {
        let key = unwrap(UPDATE_PUBLIC_KEY.as_bytes()).expect("base64 text");
        assert!(PublicKey::decode(&key).is_ok(), "{key}");
    }

    #[test]
    fn the_signed_version_comes_from_the_trusted_comment() {
        assert_eq!(signed_version("timestamp:1795394961\tfile:CopilotBar.app.tar.gz\tversion:1.3.0"), Some("1.3.0"));
        assert_eq!(signed_version("timestamp:1795394961\tfile:CopilotBar.app.tar.gz"), None);
    }

    #[test]
    fn a_missing_or_malformed_key_never_verifies() {
        for key in ["", "not base64!", "aGVsbG8="] {
            let err = verify(key, b"data", b"").unwrap_err();
            assert!(err.contains("malformed"), "{key}: {err}");
        }
    }

    #[test]
    fn an_empty_or_garbled_signature_file_never_verifies() {
        for sig in [&b""[..], b"garbage", b"aGVsbG8="] {
            assert_eq!(verify(UPDATE_PUBLIC_KEY, b"data", sig).unwrap_err(), "isn't a valid signature file");
        }
    }
}
