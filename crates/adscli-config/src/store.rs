//! Refresh-token storage. Prefers the OS keychain; falls back to a 0600 file.

use std::fs;
use std::path::Path;

use super::{ConfigError, env_truthy};

pub const KEYRING_SERVICE: &str = "adscli";
pub const KEYRING_USER: &str = "google-ads-refresh-token";
pub const ENV_FORCE_FILE_STORE: &str = "ADSCLI_FORCE_FILE_STORE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackend {
    Keyring,
    File,
}

impl SecretBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::File => "file",
        }
    }
}

pub fn load_refresh_token() -> Option<String> {
    if env_truthy(ENV_FORCE_FILE_STORE) {
        return None;
    }
    keyring_entry()
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.is_empty())
}

pub fn save_refresh_token(token: &str) -> SecretBackend {
    if !env_truthy(ENV_FORCE_FILE_STORE)
        && let Ok(entry) = keyring_entry()
        && entry.set_password(token).is_ok()
    {
        return SecretBackend::Keyring;
    }
    SecretBackend::File
}

pub fn delete_refresh_token() {
    if let Ok(entry) = keyring_entry() {
        let _ = entry.delete_credential();
    }
}

fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
}

/// Restrict a credentials file to the current user (unix). No-op elsewhere.
pub fn lock_down_file(path: &Path) -> Result<(), ConfigError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_labels() {
        assert_eq!(SecretBackend::Keyring.as_str(), "keyring");
        assert_eq!(SecretBackend::File.as_str(), "file");
    }
}
