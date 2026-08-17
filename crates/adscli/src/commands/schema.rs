use std::process::ExitCode;

use clap::{ArgAction, Command, CommandFactory};
use serde_json::{Value, json};

use crate::cli::Cli;
use crate::output;

pub fn run(json: bool) -> ExitCode {
    let tree = command_schema(&Cli::command());
    if json {
        if let Err(e) = output::write_json(&tree) {
            return crate::output::emit_error(true, &e);
        }
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&tree).unwrap_or_default()
        );
    }
    ExitCode::SUCCESS
}

fn command_schema(cmd: &Command) -> Value {
    let args: Vec<Value> = cmd
        .get_arguments()
        .filter(|a| !a.is_hide_set())
        .map(|a| {
            json!({
                "id": a.get_id().to_string(),
                "long": a.get_long(),
                "short": a.get_short().map(|c| c.to_string()),
                "required": a.is_required_set(),
                "global": a.is_global_set(),
                "multiple": matches!(a.get_action(), ArgAction::Append | ArgAction::Count),
                "help": a.get_help().map(|s| s.to_string()),
                "env": a.get_env().map(|e| e.to_string_lossy().into_owned()),
                "default": a.get_default_values().first().map(|v| v.to_string_lossy().into_owned()),
            })
        })
        .collect();
    let commands: Vec<Value> = cmd
        .get_subcommands()
        .filter(|c| !c.is_hide_set())
        .map(command_schema)
        .collect();
    json!({
        "name": cmd.get_name(),
        "about": cmd.get_about().map(|s| s.to_string()),
        "long_about": cmd.get_long_about().map(|s| s.to_string()),
        "after_help": cmd.get_after_help().map(|s| s.to_string()),
        "args": args,
        "commands": commands,
    })
}
