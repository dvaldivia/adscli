mod cli;
mod commands;
mod output;
mod platform;
mod runtime;

use std::process::ExitCode;

fn main() -> ExitCode {
    cli::run()
}
