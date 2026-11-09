//! Optional publisher-hosted OAuth client.
//!
//! Official release CI injects these at compile time via
//! `ADSCLI_BUNDLED_CLIENT_ID` / `ADSCLI_BUNDLED_CLIENT_SECRET` (GitHub
//! Actions secrets). Ordinary `cargo build` leaves them unset so source
//! builds stay bring-your-own. Runtime env / config / flags still
//! override these.
//!
//! The Desktop secret is public-by-design once it lives in a release
//! binary. It is kept out of git.
//!
//! The OAuth client also decides API access: since Google sunset
//! developer tokens (2026-09-09), the access level and daily operation
//! quota belong to the Cloud project that owns this client. Everyone
//! using the bundled client shares that project's quota.

pub const BUNDLED_CLIENT_ID: Option<&str> = option_env!("ADSCLI_BUNDLED_CLIENT_ID");
pub const BUNDLED_CLIENT_SECRET: Option<&str> = option_env!("ADSCLI_BUNDLED_CLIENT_SECRET");

pub fn bundled_client_id() -> Option<&'static str> {
    nonempty(BUNDLED_CLIENT_ID)
}

pub fn bundled_client_secret() -> Option<&'static str> {
    nonempty(BUNDLED_CLIENT_SECRET)
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
    fn source_builds_have_no_hardcoded_client() {
        // Release CI sets ADSCLI_BUNDLED_*; local/dev builds must not
        // compile a client into the crate from source.
        if option_env!("ADSCLI_BUNDLED_CLIENT_ID").is_none() {
            assert!(bundled_client_id().is_none());
            assert!(bundled_client_secret().is_none());
            assert!(!has_bundled_oauth());
        }
    }
}
