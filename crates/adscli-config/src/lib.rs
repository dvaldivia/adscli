//! Configuration, credential files, and Google Ads ID helpers.
//!
//! Resolution order (highest wins): CLI flags → environment → config file
//! → built-in defaults. Secrets never appear in `Display` / `show` output.

mod file;
mod ids;
mod paths;
mod store;

pub use file::{ConfigFile, CredentialsFile, redacted_map};
pub use ids::{extract_resource_id, normalize_customer_id, resource_name, strip_customer_prefix};
pub use paths::{ConfigPaths, config_paths};
pub use store::{
    ENV_FORCE_FILE_STORE, KEYRING_SERVICE, SecretBackend, delete_refresh_token, load_refresh_token,
    lock_down_file, save_refresh_token,
};

use std::path::PathBuf;

use thiserror::Error;

/// Latest Google Ads API major version this binary speaks.
pub const API_VERSION: &str = "v25";

/// Default REST origin including the API version path segment.
pub const DEFAULT_API_BASE: &str = "https://googleads.googleapis.com/v25";

/// OAuth scope required by every Google Ads API call.
pub const ADWORDS_SCOPE: &str = "https://www.googleapis.com/auth/adwords";

pub const ENV_CONFIG: &str = "ADSCLI_CONFIG";
pub const ENV_DEVELOPER_TOKEN: &str = "ADSCLI_DEVELOPER_TOKEN";
pub const ENV_CUSTOMER_ID: &str = "ADSCLI_CUSTOMER_ID";
pub const ENV_LOGIN_CUSTOMER_ID: &str = "ADSCLI_LOGIN_CUSTOMER_ID";
pub const ENV_CLIENT_ID: &str = "ADSCLI_CLIENT_ID";
pub const ENV_CLIENT_SECRET: &str = "ADSCLI_CLIENT_SECRET";
pub const ENV_REFRESH_TOKEN: &str = "ADSCLI_REFRESH_TOKEN";
pub const ENV_ACCESS_TOKEN: &str = "ADSCLI_ACCESS_TOKEN";
pub const ENV_API_BASE: &str = "ADSCLI_API_BASE";
pub const ENV_SKIP_TOKEN_REFRESH: &str = "ADSCLI_SKIP_TOKEN_REFRESH";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    Message(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Fully resolved runtime settings. Empty strings mean "unset".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub api_base: String,
    pub api_version: String,
    pub developer_token: String,
    pub customer_id: String,
    pub login_customer_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    pub access_token: String,
    pub access_token_expiry: Option<chrono::DateTime<chrono::Utc>>,
    pub skip_token_refresh: bool,
    /// Where the refresh token was last read from or written to.
    pub token_store: Option<SecretBackend>,
    pub config_path: Option<PathBuf>,
    pub credentials_path: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_base: DEFAULT_API_BASE.to_string(),
            api_version: API_VERSION.to_string(),
            developer_token: String::new(),
            customer_id: String::new(),
            login_customer_id: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            refresh_token: String::new(),
            access_token: String::new(),
            access_token_expiry: None,
            skip_token_refresh: false,
            token_store: None,
            config_path: None,
            credentials_path: ConfigPaths::default().credentials,
        }
    }
}

impl Settings {
    pub fn has_developer_token(&self) -> bool {
        !self.developer_token.is_empty()
    }

    pub fn has_oauth_client(&self) -> bool {
        !self.client_id.is_empty() && !self.client_secret.is_empty()
    }

    pub fn has_refresh_token(&self) -> bool {
        !self.refresh_token.is_empty()
    }

    pub fn has_access_token(&self) -> bool {
        !self.access_token.is_empty()
    }

    pub fn can_call_api(&self) -> bool {
        self.has_developer_token() && (self.has_access_token() || self.has_refresh_token())
    }
}

/// Overrides supplied by clap flags (already collapsed with clap `env`).
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub config: Option<PathBuf>,
    pub api_base: Option<String>,
    pub developer_token: Option<String>,
    pub customer_id: Option<String>,
    pub login_customer_id: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
}

pub fn load(overrides: &CliOverrides) -> Result<Settings, ConfigError> {
    let paths = config_paths(overrides.config.as_deref());
    let file = match &paths.config {
        Some(p) => ConfigFile::load(p)?,
        None => ConfigFile::default(),
    };
    let creds = CredentialsFile::load(&paths.credentials)?;

    let mut s = Settings {
        credentials_path: paths.credentials.clone(),
        config_path: paths.config.clone(),
        ..Settings::default()
    };

    // File (lowest, after defaults)
    apply_file(&mut s, &file);
    apply_creds(&mut s, &creds);
    apply_keyring(&mut s);

    // Environment (clap already maps most flags; we still read a few
    // that are not CLI flags so scripts can inject tokens).
    apply_env(&mut s);

    // CLI flags (highest)
    apply_overrides(&mut s, overrides);

    if let Some(id) = normalize_opt(&s.customer_id) {
        s.customer_id = id;
    }
    if let Some(id) = normalize_opt(&s.login_customer_id) {
        s.login_customer_id = id;
    }

    // Keep api_version in sync with a custom --api-base that ends in /vNN.
    if let Some(ver) = s.api_base.rsplit('/').next()
        && ver.starts_with('v')
        && ver[1..].bytes().all(|b| b.is_ascii_digit())
    {
        s.api_version = ver.to_string();
    }

    Ok(s)
}

fn normalize_opt(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(normalize_customer_id(s))
    }
}

fn apply_file(s: &mut Settings, f: &ConfigFile) {
    set_if(&mut s.developer_token, f.developer_token.as_deref());
    set_if(&mut s.customer_id, f.customer_id.as_deref());
    set_if(&mut s.login_customer_id, f.login_customer_id.as_deref());
    set_if(&mut s.client_id, f.client_id.as_deref());
    set_if(&mut s.client_secret, f.client_secret.as_deref());
    set_if(&mut s.refresh_token, f.refresh_token.as_deref());
    set_if(&mut s.api_base, f.api_base.as_deref());
    if let Some(v) = f.api_version.as_deref()
        && !v.is_empty()
    {
        s.api_version = v.to_string();
        if s.api_base == DEFAULT_API_BASE {
            s.api_base = format!("https://googleads.googleapis.com/{v}");
        }
    }
}

fn apply_creds(s: &mut Settings, c: &CredentialsFile) {
    if let Some(rt) = c.refresh_token.as_deref()
        && !rt.trim().is_empty()
    {
        s.refresh_token = rt.trim().to_string();
        s.token_store = Some(SecretBackend::File);
    }
    set_if(&mut s.access_token, c.access_token.as_deref());
    set_if(&mut s.client_id, c.client_id.as_deref());
    set_if(&mut s.client_secret, c.client_secret.as_deref());
    s.access_token_expiry = c.expiry;
}

fn apply_keyring(s: &mut Settings) {
    if !s.refresh_token.is_empty() {
        return;
    }
    if let Some(rt) = load_refresh_token() {
        s.refresh_token = rt;
        s.token_store = Some(SecretBackend::Keyring);
    }
}

fn apply_env(s: &mut Settings) {
    set_if(
        &mut s.developer_token,
        std::env::var(ENV_DEVELOPER_TOKEN).ok().as_deref(),
    );
    set_if(
        &mut s.customer_id,
        std::env::var(ENV_CUSTOMER_ID).ok().as_deref(),
    );
    set_if(
        &mut s.login_customer_id,
        std::env::var(ENV_LOGIN_CUSTOMER_ID).ok().as_deref(),
    );
    set_if(
        &mut s.client_id,
        std::env::var(ENV_CLIENT_ID).ok().as_deref(),
    );
    set_if(
        &mut s.client_secret,
        std::env::var(ENV_CLIENT_SECRET).ok().as_deref(),
    );
    set_if(
        &mut s.refresh_token,
        std::env::var(ENV_REFRESH_TOKEN).ok().as_deref(),
    );
    set_if(
        &mut s.access_token,
        std::env::var(ENV_ACCESS_TOKEN).ok().as_deref(),
    );
    set_if(&mut s.api_base, std::env::var(ENV_API_BASE).ok().as_deref());
    if env_truthy(ENV_SKIP_TOKEN_REFRESH) {
        s.skip_token_refresh = true;
    }
}

fn apply_overrides(s: &mut Settings, o: &CliOverrides) {
    set_if(&mut s.api_base, o.api_base.as_deref());
    set_if(&mut s.developer_token, o.developer_token.as_deref());
    set_if(&mut s.customer_id, o.customer_id.as_deref());
    set_if(&mut s.login_customer_id, o.login_customer_id.as_deref());
    set_if(&mut s.client_id, o.client_id.as_deref());
    set_if(&mut s.client_secret, o.client_secret.as_deref());
    set_if(&mut s.refresh_token, o.refresh_token.as_deref());
    set_if(&mut s.access_token, o.access_token.as_deref());
}

fn set_if(dest: &mut String, src: Option<&str>) {
    if let Some(v) = src {
        let t = v.trim();
        if !t.is_empty() {
            *dest = t.to_string();
        }
    }
}

pub fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref().map(str::trim),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_api_base_matches_version() {
        assert!(DEFAULT_API_BASE.ends_with(API_VERSION));
    }
}
