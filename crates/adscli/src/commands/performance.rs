use std::process::ExitCode;

use adscli_api::PerformanceRow;

use crate::cli::{Cli, PerformanceCmd};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &PerformanceCmd) -> ExitCode {
    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let cid = match client.customer_id() {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };

    let result = (|| {
        let rows = match cmd {
            PerformanceCmd::Campaigns { opts } => {
                let filter = crate::cli::filter_from(opts)?;
                client.performance_campaigns(&cid, &filter)?
            }
            PerformanceCmd::AssetGroups { opts, campaign } => {
                let mut filter = crate::cli::filter_from(opts)?;
                filter.campaign_id = campaign.clone();
                client.performance_asset_groups(&cid, &filter)?
            }
            PerformanceCmd::Assets {
                opts,
                campaign,
                asset_group,
            } => {
                let mut filter = crate::cli::filter_from(opts)?;
                filter.campaign_id = campaign.clone();
                filter.asset_group_id = asset_group.clone();
                client.performance_assets(&cid, &filter)?
            }
        };
        emit(cli, &rows)
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => output::emit_error(cli.json, &e),
    }
}

fn emit(cli: &Cli, rows: &[PerformanceRow]) -> Result<(), adscli_api::ApiError> {
    if cli.quiet {
        output::write_quiet(&rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>());
        return Ok(());
    }
    if cli.json {
        return output::write_json(&rows);
    }
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.resource.clone(),
                r.id.clone(),
                r.name.clone(),
                r.metrics
                    .impressions
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                r.metrics.clicks.map(|n| n.to_string()).unwrap_or_default(),
                r.metrics
                    .cost_micros
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                r.metrics
                    .conversions
                    .map(|n| format!("{n:.2}"))
                    .unwrap_or_default(),
            ]
        })
        .collect();
    output::write_table(
        &[
            "RESOURCE",
            "ID",
            "NAME",
            "IMPR",
            "CLICKS",
            "COST_MICROS",
            "CONV",
        ],
        &table,
    )
}
