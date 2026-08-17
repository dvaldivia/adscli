use std::fmt;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Config,
    Auth,
    NotFound,
    Permission,
    Conflict,
    Usage,
    Google,
    Transport,
}

impl ErrorKind {
    /// Process exit code. Documented in `adscli --help`.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Usage => 2,
            Self::NotFound => 3,
            Self::Auth | Self::Permission => 4,
            Self::Conflict => 5,
            Self::Config | Self::Google | Self::Transport => 1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Auth => "auth",
            Self::NotFound => "not_found",
            Self::Permission => "permission",
            Self::Conflict => "conflict",
            Self::Usage => "usage",
            Self::Google => "google_ads",
            Self::Transport => "transport",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiError {
    pub kind: ErrorKind,
    pub message: String,
    pub suggestion: Option<String>,
    pub google_code: Option<String>,
    pub status: Option<u16>,
}

impl ApiError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            suggestion: None,
            google_code: None,
            status: None,
        }
    }

    pub fn suggest(mut self, s: impl Into<String>) -> Self {
        self.suggestion = Some(s.into());
        self
    }

    pub fn config(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Config, m)
    }

    pub fn auth(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Auth, m)
            .suggest("run `adscli auth login` or set ADSCLI_REFRESH_TOKEN / ADSCLI_ACCESS_TOKEN")
    }

    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, m)
    }

    pub fn usage(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, m)
    }

    pub fn transport(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Transport, m)
    }

    pub fn from_http(status: u16, body: &str) -> Self {
        let value: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let (gcode, gmsg) = parse_google_failure(&value);
        let message = gmsg
            .or_else(|| {
                value
                    .pointer("/error/message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| {
                if body.trim().is_empty() {
                    format!("HTTP {status}")
                } else {
                    truncate(body, 400)
                }
            });

        let kind = match status {
            401 => ErrorKind::Auth,
            403 => ErrorKind::Permission,
            404 => ErrorKind::NotFound,
            409 => ErrorKind::Conflict,
            _ => ErrorKind::Google,
        };
        let mut err = Self::new(kind, message);
        err.status = Some(status);
        err.google_code = gcode;
        if kind == ErrorKind::Auth {
            err.suggestion = Some(
                "run `adscli auth status --json` then `adscli auth login` or refresh ADSCLI_ACCESS_TOKEN"
                    .into(),
            );
        }
        err
    }

    pub fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("error".into(), Value::String(self.kind.as_str().into()));
        m.insert("message".into(), Value::String(self.message.clone()));
        if let Some(s) = &self.suggestion {
            m.insert("suggestion".into(), Value::String(s.clone()));
        }
        if let Some(c) = &self.google_code {
            m.insert("google_code".into(), Value::String(c.clone()));
        }
        if let Some(s) = self.status {
            m.insert("http_status".into(), Value::from(s));
        }
        Value::Object(m)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(s) = &self.suggestion {
            write!(f, " ({s})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

impl From<adscli_config::ConfigError> for ApiError {
    fn from(e: adscli_config::ConfigError) -> Self {
        Self::config(e.to_string())
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        Self::transport(e.to_string())
    }
}

fn parse_google_failure(v: &Value) -> (Option<String>, Option<String>) {
    let details = v.pointer("/error/details").and_then(|d| d.as_array());
    let Some(details) = details else {
        return (None, None);
    };
    for d in details {
        let errors = d.get("errors").and_then(|e| e.as_array());
        let Some(errors) = errors else {
            continue;
        };
        if let Some(first) = errors.first() {
            let msg = first
                .get("message")
                .and_then(|m| m.as_str())
                .map(str::to_string);
            let code = first.get("errorCode").cloned().map(|c| c.to_string());
            return (code, msg);
        }
    }
    (None, None)
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_ads_failure() {
        let body = r#"{
          "error": {
            "code": 400,
            "message": "Request contains an invalid argument.",
            "status": "INVALID_ARGUMENT",
            "details": [{
              "@type": "type.googleapis.com/google.ads.googleads.v25.errors.GoogleAdsFailure",
              "errors": [{
                "errorCode": {"queryError": "UNRECOGNIZED_FIELD"},
                "message": "Unrecognized field in the query: 'campaign.foo'."
              }]
            }]
          }
        }"#;
        let err = ApiError::from_http(400, body);
        assert_eq!(err.kind, ErrorKind::Google);
        assert!(err.message.contains("Unrecognized field"));
        assert!(err.google_code.unwrap().contains("UNRECOGNIZED_FIELD"));
    }

    #[test]
    fn maps_401_to_auth() {
        let err = ApiError::from_http(401, r#"{"error":{"message":"invalid token"}}"#);
        assert_eq!(err.kind, ErrorKind::Auth);
        assert_eq!(err.kind.exit_code(), 4);
    }
}
