//! On-disk YAML config and JSON credential cache.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ConfigError;

/// Accepts both adscli field names and the Python `google-ads.yaml` names.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConfigFile {
    #[serde(default, alias = "developerToken")]
    pub developer_token: Option<String>,
    #[serde(default, alias = "customerId")]
    pub customer_id: Option<String>,
    #[serde(default, alias = "loginCustomerId")]
    pub login_customer_id: Option<String>,
    #[serde(default, alias = "clientId")]
    pub client_id: Option<String>,
    #[serde(default, alias = "clientSecret")]
    pub client_secret: Option<String>,
    #[serde(default, alias = "refreshToken")]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
}

impl ConfigFile {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_yaml::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let raw = serde_yaml::to_string(self)?;
        fs::write(path, raw)?;
        Ok(())
    }
}

/// OAuth token cache. Also accepts `gcloud` ADC `authorized_user` JSON.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CredentialsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<DateTime<Utc>>,
}

impl CredentialsFile {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(path, raw + "\n")?;
        crate::lock_down_file(path)?;
        Ok(())
    }
}

/// Non-secret snapshot for `adscli config show --json`.
pub fn redacted_map(s: &super::Settings) -> BTreeMap<&'static str, serde_json::Value> {
    let mut m = BTreeMap::new();
    m.insert("api_base", json_str(&s.api_base));
    m.insert("api_version", json_str(&s.api_version));
    m.insert(
        "config_path",
        s.config_path
            .as_ref()
            .map(|p| json_str(&p.display().to_string()))
            .unwrap_or(serde_json::Value::Null),
    );
    m.insert(
        "credentials_path",
        json_str(&s.credentials_path.display().to_string()),
    );
    m.insert("customer_id", json_str(&s.customer_id));
    m.insert("login_customer_id", json_str(&s.login_customer_id));
    m.insert(
        "has_access_token",
        serde_json::Value::Bool(s.has_access_token()),
    );
    m.insert(
        "has_client_id",
        serde_json::Value::Bool(!s.client_id.is_empty()),
    );
    m.insert(
        "has_developer_token",
        serde_json::Value::Bool(s.has_developer_token()),
    );
    m.insert(
        "has_refresh_token",
        serde_json::Value::Bool(s.has_refresh_token()),
    );
    m.insert(
        "skip_token_refresh",
        serde_json::Value::Bool(s.skip_token_refresh),
    );
    m.insert(
        "token_store",
        s.token_store
            .map(|b| json_str(b.as_str()))
            .unwrap_or(serde_json::Value::Null),
    );
    m.insert(
        "access_token_expiry",
        s.access_token_expiry
            .map(|t| json_str(&t.to_rfc3339()))
            .unwrap_or(serde_json::Value::Null),
    );
    m
}

fn json_str(s: &str) -> serde_json::Value {
    serde_json::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_python_style_yaml() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("google-ads.yaml");
        fs::write(
            &p,
            "developer_token: tok\nlogin_customer_id: 111-222-3333\nclient_id: cid\n",
        )
        .unwrap();
        let f = ConfigFile::load(&p).unwrap();
        assert_eq!(f.developer_token.as_deref(), Some("tok"));
        assert_eq!(f.login_customer_id.as_deref(), Some("111-222-3333"));
        assert_eq!(f.client_id.as_deref(), Some("cid"));
    }

    #[test]
    fn credentials_roundtrip() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("credentials.json");
        let c = CredentialsFile {
            r#type: Some("authorized_user".into()),
            refresh_token: Some("rt".into()),
            access_token: Some("at".into()),
            ..Default::default()
        };
        c.save(&p).unwrap();
        let loaded = CredentialsFile::load(&p).unwrap();
        assert_eq!(loaded.refresh_token.as_deref(), Some("rt"));
        assert_eq!(loaded.access_token.as_deref(), Some("at"));
    }
}
