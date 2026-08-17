use std::io::IsTerminal;
use std::process::ExitCode;

use adscli_api::ApiError;
use adscli_api::auth::AuthStatus;
use adscli_api::auth::{
    apply_tokens, bind_localhost, build_auth_request, clear_stored_tokens, exchange_code,
    exchange_code_pkce, open_browser, poll_device_token, request_device_code, wait_for_callback,
};

use crate::cli::{AuthCmd, Cli, LoginOpts};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &AuthCmd) -> ExitCode {
    match cmd {
        AuthCmd::Status => status(cli),
        AuthCmd::Logout => logout(cli),
        AuthCmd::Login { opts } => login(cli, opts),
    }
}

fn status(cli: &Cli) -> ExitCode {
    let s = match runtime::settings(cli) {
        Ok(s) => s,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let st = AuthStatus::from_settings(&s);
    if cli.json {
        if let Err(e) = output::write_json(&st) {
            return output::emit_error(true, &e);
        }
    } else {
        println!(
            "authenticated={} developer_token={} refresh_token={} token_store={} customer_id={}",
            st.authenticated,
            st.has_developer_token,
            st.has_refresh_token,
            st.token_store.as_deref().unwrap_or("-"),
            if st.customer_id.is_empty() {
                "-"
            } else {
                &st.customer_id
            }
        );
    }
    if st.authenticated {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(4)
    }
}

pub fn logout(cli: &Cli) -> ExitCode {
    let s = match runtime::settings(cli) {
        Ok(s) => s,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    if let Err(e) = clear_stored_tokens(&s) {
        return output::emit_error(cli.json, &e);
    }
    if cli.json {
        let _ = output::write_json(&serde_json::json!({
            "logged_out": true,
            "credentials_path": s.credentials_path,
        }));
    } else {
        println!("removed cached Google Ads credentials");
    }
    ExitCode::SUCCESS
}

pub fn login(cli: &Cli, opts: &LoginOpts) -> ExitCode {
    let mut s = match runtime::settings(cli) {
        Ok(s) => s,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    if s.client_id.is_empty() {
        return output::emit_error(
            cli.json,
            &ApiError::auth("client_id is required for login").suggest(
                "set ADSCLI_CLIENT_ID and ADSCLI_CLIENT_SECRET (OAuth Desktop app client)",
            ),
        );
    }
    if s.client_secret.is_empty() {
        return output::emit_error(
            cli.json,
            &ApiError::auth("client_secret is required for login").suggest(
                "create a Desktop OAuth client in Google Cloud Console and set ADSCLI_CLIENT_SECRET",
            ),
        );
    }

    let flow = if opts.device {
        "device"
    } else if opts.code.is_some() {
        "code"
    } else {
        "desktop_pkce"
    };

    let result = if opts.device {
        device_login(&s, opts)
    } else if let Some(code) = opts.code.as_deref() {
        let uri = match opts.redirect_uri.as_deref() {
            Some(u) => u,
            None => {
                return output::emit_error(
                    cli.json,
                    &ApiError::usage("--redirect-uri is required with --code"),
                );
            }
        };
        exchange_code(&s.client_id, &s.client_secret, uri, code)
    } else {
        desktop_login(&s, opts)
    };

    let tokens = match result {
        Ok(t) => t,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    if let Err(e) = apply_tokens(&mut s, &tokens) {
        return output::emit_error(cli.json, &e);
    }
    let store = s.token_store.map(|b| b.as_str()).unwrap_or("file");
    if cli.json {
        let _ = output::write_json(&serde_json::json!({
            "authenticated": true,
            "flow": flow,
            "token_store": store,
            "credentials_path": s.credentials_path,
            "has_refresh_token": s.has_refresh_token(),
        }));
    } else {
        println!("signed in ({store})");
        if !s.has_developer_token() {
            eprintln!(
                "note: API calls still need a developer token (ADSCLI_DEVELOPER_TOKEN or config.yaml)"
            );
        }
    }
    ExitCode::SUCCESS
}

fn desktop_login(
    s: &adscli_config::Settings,
    opts: &LoginOpts,
) -> Result<adscli_api::TokenSet, ApiError> {
    if !std::io::stdout().is_terminal() && !opts.print_url && !opts.no_browser {
        return Err(ApiError::usage(
            "login is interactive; pass --device, --print-url, or set ADSCLI_REFRESH_TOKEN",
        ));
    }
    let (uri, listener) = bind_localhost(opts.port)?;
    let req = build_auth_request(&s.client_id, &uri)?;
    eprintln!("Waiting for Google to redirect to {uri}");
    eprintln!("{}", req.url);
    if !opts.print_url && !opts.no_browser {
        match open_browser(&req.url) {
            Ok(()) => eprintln!("opened the default browser"),
            Err(e) => eprintln!("could not open a browser ({e}); open the URL above"),
        }
    }
    let cb = wait_for_callback(listener, Some(&req.state))?;
    exchange_code_pkce(
        &s.client_id,
        &s.client_secret,
        &req.redirect_uri,
        &cb.code,
        Some(&req.pkce.verifier),
    )
}

fn device_login(
    s: &adscli_config::Settings,
    opts: &LoginOpts,
) -> Result<adscli_api::TokenSet, ApiError> {
    let device = request_device_code(&s.client_id)?;
    eprintln!(
        "Visit {} and enter code: {}",
        device.verification_url, device.user_code
    );
    if !opts.no_browser && !opts.print_url {
        let _ = open_browser(&device.verification_url);
    }
    poll_device_token(&s.client_id, &s.client_secret, &device)
}
