use std::process::ExitCode;

use crate::cli::Cli;
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, query: &str) -> ExitCode {
    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let cid = match client.customer_id() {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    match client.search_raw(&cid, query) {
        Ok(rows) => {
            if let Err(e) = output::write_json(&serde_json::json!({
                "query": query,
                "count": rows.len(),
                "results": rows,
            })) {
                return output::emit_error(cli.json, &e);
            }
            ExitCode::SUCCESS
        }
        Err(e) => output::emit_error(cli.json, &e),
    }
}
