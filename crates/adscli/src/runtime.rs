use adscli_api::{AdsClient, ApiError, ReqwestTransport};
use adscli_config::{CliOverrides, Settings, load};

use crate::cli::Cli;

pub fn settings(cli: &Cli) -> Result<Settings, ApiError> {
    let o = CliOverrides {
        config: cli.config.clone(),
        api_base: cli.api_base.clone(),
        developer_token: cli.developer_token.clone(),
        customer_id: cli.customer_id.clone(),
        login_customer_id: cli.login_customer_id.clone(),
        client_id: cli.client_id.clone(),
        client_secret: cli.client_secret.clone(),
        refresh_token: cli.refresh_token.clone(),
        access_token: None,
    };
    Ok(load(&o)?)
}

pub fn connect(cli: &Cli) -> Result<AdsClient<ReqwestTransport>, ApiError> {
    let s = settings(cli)?;
    adscli_api::connect(&s)
}

pub fn require_yes(yes: bool, action: &str) -> Result<(), ApiError> {
    if yes {
        return Ok(());
    }
    Err(ApiError::usage(format!(
        "{action} requires --yes (adscli never prompts)"
    )))
}
