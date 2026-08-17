//! Google Ads API v25 REST client.
//!
//! Talks HTTP/JSON (`googleads.googleapis.com/v25`) rather than the generated
//! gRPC stubs so the binary stays small and every request is inspectable via
//! `--dry-run`.

pub mod auth;
mod client;
mod error;
mod jsonpath;
mod models;
pub mod query;

pub use auth::{
    AuthRequest, AuthStatus, DeviceCode, Pkce, TokenSet, build_auth_request, exchange_code,
    exchange_code_pkce, login_url, refresh_access_token,
};
pub use client::{
    AdsClient, CreateAsset, CreateAssetGroup, CreateCampaign, DryRun, HttpRequest, HttpResponse,
    ReqwestTransport, Transport, UpdateAssetGroup, UpdateCampaign,
};
pub use error::{ApiError, ErrorKind};
pub use jsonpath::{as_f64, as_i64, as_string, snake_to_camel, walk};
pub use models::{
    Asset, AssetGroup, AssetLink, Campaign, Customer, Metrics, MutateResult, PerformanceRow,
};
pub use query::{DateRange, ListFilter, PRESET_DURINGS, date_range_clause};

use adscli_config::Settings;

/// Build a live HTTP client from resolved settings. Refreshes the access
/// token when needed and persists the new token to the credentials file.
pub fn connect(settings: &Settings) -> Result<AdsClient<client::ReqwestTransport>, ApiError> {
    let mut s = settings.clone();
    auth::ensure_access_token(&mut s)?;
    AdsClient::new(s, client::ReqwestTransport::new()?)
}
