use std::io::IsTerminal;
use std::process::ExitCode;

use adscli_api::ApiError;
use adscli_api::query::DateRange;
use adscli_tui::LiveBrowser;

use crate::cli::Cli;
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli) -> ExitCode {
    if !std::io::stdout().is_terminal() {
        return output::emit_error(
            cli.json,
            &ApiError::usage(
                "no terminal detected — the default command opens an interactive TUI and requires a TTY",
            )
            .suggest(
                "use a subcommand: version, schema, customers, campaigns, asset-groups, assets, performance, gaql",
            ),
        );
    }

    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let cid = match client.customer_id() {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };

    let browser = LiveBrowser {
        client,
        customer_id: cid,
        date_range: DateRange::During("LAST_30_DAYS".into()),
    };
    match adscli_tui::start(browser) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: TUI: {e}");
            ExitCode::from(1)
        }
    }
}
