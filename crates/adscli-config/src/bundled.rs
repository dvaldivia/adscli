//! Default Desktop OAuth client (public by design) plus optional
//! compile-time overrides.
//!
//! The adscli Desktop `client_id` / `client_secret` ship in every binary.
//! Installed apps cannot keep that secret; PKCE protects the code exchange.
//! `ADSCLI_CLIENT_ID` / `ADSCLI_CLIENT_SECRET` (or a config file) still win
//! at runtime. Release CI can replace the defaults via `ADSCLI_BUNDLED_*`.
//!
//! The developer token is *not* hardcoded — it is the app's API permit and
//! quota. Set it at runtime or inject `ADSCLI_BUNDLED_DEVELOPER_TOKEN` in
//! official release builds.

/// Shared adscli Desktop client id (Google Cloud project 803809907507).
pub const DEFAULT_CLIENT_ID: &str =
    "REDACTED";

/// Shared adscli Desktop client secret. Not confidential.
pub const DEFAULT_CLIENT_SECRET: &str = "REDACTED";

pub const BUNDLED_CLIENT_ID: Option<&str> = option_env!("ADSCLI_BUNDLED_CLIENT_ID");
pub const BUNDLED_CLIENT_SECRET: Option<&str> = option_env!("ADSCLI_BUNDLED_CLIENT_SECRET");
pub const BUNDLED_DEVELOPER_TOKEN: Option<&str> = option_env!("ADSCLI_BUNDLED_DEVELOPER_TOKEN");

pub fn bundled_client_id() -> Option<&'static str> {
    nonempty(BUNDLED_CLIENT_ID).or(Some(DEFAULT_CLIENT_ID))
}

pub fn bundled_client_secret() -> Option<&'static str> {
    nonempty(BUNDLED_CLIENT_SECRET).or(Some(DEFAULT_CLIENT_SECRET))
}

pub fn bundled_developer_token() -> Option<&'static str> {
    nonempty(BUNDLED_DEVELOPER_TOKEN)
}

pub fn has_bundled_oauth() -> bool {
    bundled_client_id().is_some() && bundled_client_secret().is_some()
}

fn nonempty(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_desktop_client_is_present() {
        assert_eq!(bundled_client_id(), Some(DEFAULT_CLIENT_ID));
        assert_eq!(bundled_client_secret(), Some(DEFAULT_CLIENT_SECRET));
        assert!(has_bundled_oauth());
        assert!(bundled_developer_token().is_none());
    }
}
