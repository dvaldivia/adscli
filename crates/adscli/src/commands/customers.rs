use std::process::ExitCode;

use crate::cli::{Cli, CustomersCmd};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &CustomersCmd) -> ExitCode {
    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    match cmd {
        CustomersCmd::List => match client.list_accessible_customers() {
            Ok(rows) => {
                if cli.quiet {
                    output::write_quiet(&rows.iter().map(|c| c.id.clone()).collect::<Vec<_>>());
                    return ExitCode::SUCCESS;
                }
                if cli.json {
                    if let Err(e) = output::write_json(&rows) {
                        return output::emit_error(true, &e);
                    }
                } else {
                    let table: Vec<Vec<String>> = rows
                        .iter()
                        .map(|c| vec![c.id.clone(), c.resource_name.clone().unwrap_or_default()])
                        .collect();
                    if let Err(e) = output::write_table(&["ID", "RESOURCE_NAME"], &table) {
                        return output::emit_error(cli.json, &e);
                    }
                }
                ExitCode::SUCCESS
            }
            Err(e) => output::emit_error(cli.json, &e),
        },
        CustomersCmd::Get => {
            let cid = match client.customer_id() {
                Ok(c) => c,
                Err(e) => return output::emit_error(cli.json, &e),
            };
            match client.get_customer(&cid) {
                Ok(c) => {
                    if cli.json {
                        if let Err(e) = output::write_json(&c) {
                            return output::emit_error(true, &e);
                        }
                    } else {
                        println!(
                            "{}\t{}\t{}\t{}",
                            c.id,
                            c.descriptive_name.as_deref().unwrap_or("-"),
                            c.currency_code.as_deref().unwrap_or("-"),
                            c.status.as_deref().unwrap_or("-")
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => output::emit_error(cli.json, &e),
            }
        }
    }
}
