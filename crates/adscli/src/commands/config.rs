use std::process::ExitCode;

use adscli_config::redacted_map;

use crate::cli::{Cli, ConfigCmd};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &ConfigCmd) -> ExitCode {
    let s = match runtime::settings(cli) {
        Ok(s) => s,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    match cmd {
        ConfigCmd::Path => {
            if cli.json {
                let _ = output::write_json(&serde_json::json!({
                    "config_path": s.config_path,
                    "credentials_path": s.credentials_path,
                }));
            } else {
                println!(
                    "config\t{}",
                    s.config_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "-".into())
                );
                println!("credentials\t{}", s.credentials_path.display());
            }
        }
        ConfigCmd::Show => {
            let map = redacted_map(&s);
            if let Err(e) = output::write_json(&map) {
                return output::emit_error(cli.json, &e);
            }
        }
    }
    ExitCode::SUCCESS
}
