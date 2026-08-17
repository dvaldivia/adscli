//! OAuth 2.0 Desktop / Installed-app flow (SSO-style browser login).
//!
//! - Authorization Code + PKCE (S256) + CSRF `state`
//! - Loopback redirect on `http://127.0.0.1`
//! - `access_type=offline` so we get a refresh token
//! - Optional device-code flow for machines without a browser
//!
//! Agents should set `ADSCLI_REFRESH_TOKEN` instead of running login.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use adscli_config::{
    ADWORDS_SCOPE, CredentialsFile, SecretBackend, Settings, delete_refresh_token,
    save_refresh_token,
};
use base64::Engine;
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::ApiError;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DEVICE_CODE_URL: &str = "https://oauth2.googleapis.com/device/code";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Default)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

#[derive(Debug, Clone)]
pub struct AuthRequest {
    pub url: String,
    pub redirect_uri: String,
    pub state: String,
    pub pkce: Pkce,
}

#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub has_refresh_token: bool,
    pub has_access_token: bool,
    pub has_developer_token: bool,
    pub has_oauth_client: bool,
    pub customer_id: String,
    pub login_customer_id: String,
    pub api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_store: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token_expiry: Option<String>,
}

impl AuthStatus {
    pub fn from_settings(s: &Settings) -> Self {
        Self {
            authenticated: s.can_call_api(),
            has_refresh_token: s.has_refresh_token(),
            has_access_token: s.has_access_token(),
            has_developer_token: s.has_developer_token(),
            has_oauth_client: s.has_oauth_client(),
            customer_id: s.customer_id.clone(),
            login_customer_id: s.login_customer_id.clone(),
            api_version: s.api_version.clone(),
            token_store: s.token_store.map(|b| b.as_str().to_string()),
            access_token_expiry: s.access_token_expiry.map(|t| t.to_rfc3339()),
        }
    }
}

pub fn generate_pkce() -> Result<Pkce, ApiError> {
    let verifier = random_b64(32)?;
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = b64url(&digest);
    Ok(Pkce {
        verifier,
        challenge,
    })
}

pub fn generate_state() -> Result<String, ApiError> {
    random_b64(16)
}

pub fn login_url(client_id: &str, redirect_uri: &str) -> String {
    // Tests and `--print-url` without a bound listener still need a
    // well-formed consent URL. PKCE is added by [`build_auth_request`].
    format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
        urlenc(client_id),
        urlenc(redirect_uri),
        urlenc(ADWORDS_SCOPE),
    )
}

pub fn build_auth_request(client_id: &str, redirect_uri: &str) -> Result<AuthRequest, ApiError> {
    let pkce = generate_pkce()?;
    let state = generate_state()?;
    let url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        urlenc(client_id),
        urlenc(redirect_uri),
        urlenc(ADWORDS_SCOPE),
        urlenc(&pkce.challenge),
        urlenc(&state),
    );
    Ok(AuthRequest {
        url,
        redirect_uri: redirect_uri.to_string(),
        state,
        pkce,
    })
}

pub fn exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<TokenSet, ApiError> {
    exchange_code_pkce(client_id, client_secret, redirect_uri, code, None)
}

pub fn exchange_code_pkce(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: Option<&str>,
) -> Result<TokenSet, ApiError> {
    let mut form = vec![
        ("client_id", client_id.to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("grant_type", "authorization_code".into()),
        ("code", code.to_string()),
    ];
    if !client_secret.is_empty() {
        form.push(("client_secret", client_secret.to_string()));
    }
    if let Some(v) = code_verifier {
        form.push(("code_verifier", v.to_string()));
    }
    token_request(&form)
}

pub fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenSet, ApiError> {
    let form = [
        ("client_id", client_id.to_string()),
        ("client_secret", client_secret.to_string()),
        ("refresh_token", refresh_token.to_string()),
        ("grant_type", "refresh_token".to_string()),
    ];
    token_request(&form)
}

pub fn request_device_code(client_id: &str) -> Result<DeviceCode, ApiError> {
    let client = http()?;
    let resp = client
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", client_id), ("scope", ADWORDS_SCOPE)])
        .send()
        .map_err(|e| ApiError::transport(e.to_string()))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(ApiError::auth(format!(
            "device-code endpoint HTTP {}: {body}",
            status.as_u16()
        )));
    }
    let parsed: DeviceCodeResponse = serde_json::from_str(&body)
        .map_err(|e| ApiError::auth(format!("device-code JSON: {e}")))?;
    Ok(DeviceCode {
        device_code: parsed.device_code,
        user_code: parsed.user_code,
        verification_url: parsed
            .verification_url
            .or(parsed.verification_uri)
            .unwrap_or_else(|| "https://www.google.com/device".into()),
        expires_in: parsed.expires_in.max(1),
        interval: parsed.interval.max(1),
    })
}

pub fn poll_device_token(
    client_id: &str,
    client_secret: &str,
    device: &DeviceCode,
) -> Result<TokenSet, ApiError> {
    let deadline = Instant::now() + Duration::from_secs(device.expires_in);
    let mut interval = Duration::from_secs(device.interval);
    loop {
        if Instant::now() >= deadline {
            return Err(ApiError::auth("device-code login timed out"));
        }
        thread::sleep(interval);
        let mut form = vec![
            ("client_id", client_id.to_string()),
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code".into(),
            ),
            ("device_code", device.device_code.clone()),
        ];
        if !client_secret.is_empty() {
            form.push(("client_secret", client_secret.to_string()));
        }
        match token_request(&form) {
            Ok(t) => return Ok(t),
            Err(e) => {
                let msg = e.message.to_ascii_lowercase();
                if msg.contains("authorization_pending") {
                    continue;
                }
                if msg.contains("slow_down") {
                    interval += Duration::from_secs(5);
                    continue;
                }
                return Err(e);
            }
        }
    }
}

fn token_request(form: &[(impl AsRef<str>, impl AsRef<str>)]) -> Result<TokenSet, ApiError> {
    let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();
    let resp = http()?
        .post(TOKEN_URL)
        .form(&pairs)
        .send()
        .map_err(|e| ApiError::transport(e.to_string()))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(ApiError::auth(format!(
            "token endpoint HTTP {}: {}",
            status.as_u16(),
            body
        )));
    }
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|e| ApiError::auth(format!("token JSON: {e}")))?;
    if parsed.access_token.is_empty() {
        return Err(ApiError::auth(
            "token endpoint returned an empty access_token",
        ));
    }
    Ok(TokenSet {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_in: parsed.expires_in,
    })
}

fn http() -> Result<reqwest::blocking::Client, ApiError> {
    reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| ApiError::transport(e.to_string()))
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    #[serde(default)]
    verification_url: Option<String>,
    #[serde(default)]
    verification_uri: Option<String>,
    #[serde(default)]
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

/// Ensure `settings.access_token` is usable. Writes the cache on refresh.
pub fn ensure_access_token(settings: &mut Settings) -> Result<(), ApiError> {
    if settings.skip_token_refresh && settings.has_access_token() {
        return Ok(());
    }
    if settings.has_access_token() && !token_expired(settings) {
        return Ok(());
    }
    if !settings.has_refresh_token() {
        if settings.has_access_token() {
            return Ok(());
        }
        return Err(ApiError::auth(
            "no refresh token or access token configured",
        ));
    }
    if !settings.has_oauth_client() {
        return Err(ApiError::auth(
            "client_id and client_secret are required to refresh an access token",
        ));
    }
    let tokens = refresh_access_token(
        &settings.client_id,
        &settings.client_secret,
        &settings.refresh_token,
    )?;
    apply_tokens(settings, &tokens)?;
    Ok(())
}

fn token_expired(settings: &Settings) -> bool {
    match settings.access_token_expiry {
        Some(exp) => Utc::now() + ChronoDuration::seconds(60) >= exp,
        None => true,
    }
}

pub fn apply_tokens(settings: &mut Settings, tokens: &TokenSet) -> Result<(), ApiError> {
    settings.access_token = tokens.access_token.clone();
    if let Some(rt) = &tokens.refresh_token
        && !rt.is_empty()
    {
        settings.refresh_token = rt.clone();
        settings.token_store = Some(save_refresh_token(rt));
    }
    let expiry = tokens
        .expires_in
        .map(|s| Utc::now() + ChronoDuration::seconds(s.saturating_sub(30).max(0)));
    settings.access_token_expiry = expiry;

    let mut creds = CredentialsFile::load(&settings.credentials_path)?;
    creds.r#type = Some("authorized_user".into());
    creds.access_token = Some(settings.access_token.clone());
    creds.client_id = nonempty(&settings.client_id);
    // Never persist the client secret. It belongs in config/env.
    creds.client_secret = None;
    creds.expiry = expiry;
    // Refresh token stays out of the file when the keychain accepted it.
    creds.refresh_token = match settings.token_store {
        Some(SecretBackend::Keyring) => None,
        _ if settings.has_refresh_token() => Some(settings.refresh_token.clone()),
        _ => None,
    };
    creds.save(&settings.credentials_path)?;
    Ok(())
}

pub fn clear_stored_tokens(settings: &Settings) -> Result<(), ApiError> {
    delete_refresh_token();
    if settings.credentials_path.exists() {
        std::fs::remove_file(&settings.credentials_path)
            .map_err(|e| ApiError::config(e.to_string()))?;
    }
    Ok(())
}

fn nonempty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Bind `127.0.0.1:{port}` (`0` / `None` = ephemeral).
pub fn bind_localhost(port: Option<u16>) -> Result<(String, TcpListener), ApiError> {
    let listener = TcpListener::bind(("127.0.0.1", port.unwrap_or(0)))
        .map_err(|e| ApiError::transport(format!("bind localhost: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| ApiError::transport(e.to_string()))?
        .port();
    Ok((format!("http://127.0.0.1:{port}"), listener))
}

/// Block until Google redirects back with `?code=` (or an OAuth error).
/// Ignores `/favicon.ico` so Chrome's extra request cannot steal the code.
pub fn wait_for_code(listener: TcpListener) -> Result<String, ApiError> {
    wait_for_callback(listener, None).map(|c| c.code)
}

#[derive(Debug, Clone)]
pub struct Callback {
    pub code: String,
}

pub fn wait_for_callback(
    listener: TcpListener,
    expected_state: Option<&str>,
) -> Result<Callback, ApiError> {
    listener
        .set_nonblocking(true)
        .map_err(|e| ApiError::transport(e.to_string()))?;
    let start = Instant::now();
    loop {
        if start.elapsed() > LOGIN_TIMEOUT {
            return Err(ApiError::auth(
                "timed out waiting for the browser to finish login (5 minutes)",
            ));
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 8192];
                stream
                    .set_nonblocking(false)
                    .map_err(|e| ApiError::transport(e.to_string()))?;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let first = req.lines().next().unwrap_or("");
                let params = parse_request_query(first);
                if is_noise_request(first) {
                    let _ = write_http(&mut stream, 204, "text/plain; charset=utf-8", "");
                    continue;
                }
                if let Some(err) = params.get("error") {
                    let desc = params
                        .get("error_description")
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let _ = write_http(
                        &mut stream,
                        400,
                        "text/html; charset=utf-8",
                        &html_page(
                            "adscli login failed",
                            &format!("Google returned <code>{err}</code>. {desc}"),
                        ),
                    );
                    return Err(ApiError::auth(format!("oauth error: {err} {desc}")));
                }
                let Some(code) = params.get("code").cloned() else {
                    let _ = write_http(
                        &mut stream,
                        400,
                        "text/html; charset=utf-8",
                        &html_page(
                            "adscli login failed",
                            "Redirect was missing the authorization code.",
                        ),
                    );
                    return Err(ApiError::auth(format!(
                        "oauth callback missing code: {first}"
                    )));
                };
                if let Some(want) = expected_state {
                    match params.get("state") {
                        Some(got) if got == want => {}
                        other => {
                            let _ = write_http(
                                &mut stream,
                                400,
                                "text/html; charset=utf-8",
                                &html_page("adscli login failed", "OAuth state mismatch."),
                            );
                            return Err(ApiError::auth(format!(
                                "oauth state mismatch (got {other:?})"
                            )));
                        }
                    }
                }
                let _ = write_http(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    &html_page(
                        "adscli is signed in",
                        "You can close this tab and return to the terminal.",
                    ),
                );
                return Ok(Callback { code });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(ApiError::transport(format!("oauth callback: {e}")));
            }
        }
    }
}

pub fn open_browser(url: &str) -> Result<(), ApiError> {
    webbrowser::open(url).map_err(|e| ApiError::transport(format!("open browser: {e}")))
}

fn is_noise_request(request_line: &str) -> bool {
    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    path.starts_with("/favicon")
        || path == "/"
        || path.starts_with("/?") && !path.contains("code=") && !path.contains("error=")
}

fn parse_request_query(request_line: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let path = match request_line.split_whitespace().nth(1) {
        Some(p) => p,
        None => return out,
    };
    let Some((_, q)) = path.split_once('?') else {
        return out;
    };
    for pair in q.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(urldec(k), urldec(v));
    }
    out
}

fn write_http(
    stream: &mut impl Write,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn html_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:4rem auto;max-width:36rem;color:#111}}\
         h1{{font-size:1.25rem}}code{{background:#f3f3f3;padding:.1rem .3rem}}</style></head>\
         <body><h1>{title}</h1><p>{body}</p></body></html>"
    )
}

fn random_b64(n: usize) -> Result<String, ApiError> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| ApiError::transport(format!("csprng: {e}")))?;
    Ok(b64url(&buf))
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urldec(s: &str) -> String {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        } else if b[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn parses_callback() {
        let q = parse_request_query("GET /?code=abc%2Fdef&state=s1&scope=x HTTP/1.1");
        assert_eq!(q.get("code").map(String::as_str), Some("abc/def"));
        assert_eq!(q.get("state").map(String::as_str), Some("s1"));
    }

    #[test]
    fn parses_oauth_error() {
        let q = parse_request_query("GET /?error=access_denied&error_description=nope HTTP/1.1");
        assert_eq!(q.get("error").map(String::as_str), Some("access_denied"));
    }

    #[test]
    fn login_url_contains_scope() {
        let u = login_url("cid", "http://127.0.0.1:9");
        assert!(u.contains("access_type=offline"));
        assert!(u.contains("adwords"));
        assert!(u.contains("prompt=consent"));
    }

    #[test]
    fn auth_request_includes_pkce_and_state() {
        let req = build_auth_request("cid", "http://127.0.0.1:9").unwrap();
        assert!(req.url.contains("code_challenge="));
        assert!(req.url.contains("code_challenge_method=S256"));
        assert!(req.url.contains("state="));
        assert!(req.url.contains("access_type=offline"));
        let digest = Sha256::digest(req.pkce.verifier.as_bytes());
        assert_eq!(req.pkce.challenge, b64url(&digest));
        assert!(!req.pkce.challenge.contains('+'));
        assert!(!req.pkce.challenge.contains('/'));
    }

    #[test]
    fn favicon_is_noise() {
        assert!(is_noise_request("GET /favicon.ico HTTP/1.1"));
        assert!(!is_noise_request("GET /?code=abc HTTP/1.1"));
    }
}
