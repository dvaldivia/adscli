//! Config and credential file locations.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub config: Option<PathBuf>,
    pub credentials: PathBuf,
}

impl Default for ConfigPaths {
    fn default() -> Self {
        config_paths(None)
    }
}

/// Search order for the YAML config (first existing file wins):
/// 1. `--config` / `ADSCLI_CONFIG`
/// 2. `./.adscli.yaml`, `./.adscli.yml`, `./google-ads.yaml`
/// 3. `$XDG_CONFIG_HOME/adscli/config.yaml`
/// 4. `~/.config/adscli/config.yaml`
/// 5. `~/.adscli.yaml`
///
/// Credentials always live next to the chosen config when one exists,
/// otherwise in `$XDG_CONFIG_HOME/adscli/credentials.json`.
pub fn config_paths(explicit: Option<&Path>) -> ConfigPaths {
    if let Some(p) = explicit {
        let p = expand(p);
        let credentials = p
            .parent()
            .map(|d| d.join("credentials.json"))
            .unwrap_or_else(|| PathBuf::from("credentials.json"));
        return ConfigPaths {
            config: Some(p),
            credentials,
        };
    }

    if let Ok(p) = std::env::var(super::ENV_CONFIG) {
        let t = p.trim();
        if !t.is_empty() {
            return config_paths(Some(Path::new(t)));
        }
    }

    let cwd_candidates = [
        PathBuf::from(".adscli.yaml"),
        PathBuf::from(".adscli.yml"),
        PathBuf::from("google-ads.yaml"),
    ];
    for c in cwd_candidates {
        if c.is_file() {
            return ConfigPaths {
                credentials: PathBuf::from("credentials.json"),
                config: Some(c),
            };
        }
    }

    let dir = config_dir();
    let default_cfg = dir.join("config.yaml");
    let credentials = dir.join("credentials.json");
    if default_cfg.is_file() {
        return ConfigPaths {
            config: Some(default_cfg),
            credentials,
        };
    }

    if let Some(home) = dirs::home_dir() {
        let home_cfg = home.join(".adscli.yaml");
        if home_cfg.is_file() {
            return ConfigPaths {
                config: Some(home_cfg),
                credentials,
            };
        }
    }

    ConfigPaths {
        config: None,
        credentials,
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(p) = dirs::config_dir() {
        return p.join("adscli");
    }
    if let Some(h) = dirs::home_dir() {
        return h.join(".config").join("adscli");
    }
    PathBuf::from(".adscli")
}

fn expand(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    p.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_sets_sibling_credentials() {
        let p = config_paths(Some(Path::new("/tmp/foo/ads.yaml")));
        assert_eq!(p.config.as_deref(), Some(Path::new("/tmp/foo/ads.yaml")));
        assert_eq!(p.credentials, PathBuf::from("/tmp/foo/credentials.json"));
    }
}
