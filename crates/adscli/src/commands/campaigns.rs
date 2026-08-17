use std::process::ExitCode;

use adscli_api::CreateCampaign;

use crate::cli::{CampaignsCmd, Cli};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &CampaignsCmd) -> ExitCode {
    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let cid = match client.customer_id() {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };

    let result = (|| match cmd {
        CampaignsCmd::List { opts, .. } => {
            let filter = crate::cli::filter_from(opts)?;
            let rows = client.list_campaigns(&cid, &filter)?;
            if cli.quiet {
                output::write_quiet(&rows.iter().map(|c| c.id.clone()).collect::<Vec<_>>());
                return Ok(());
            }
            if cli.json {
                output::write_json(&rows)?;
            } else {
                let table: Vec<Vec<String>> = rows
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone(),
                            c.name.clone(),
                            c.status.clone(),
                            c.channel_type.clone().unwrap_or_default(),
                            c.budget_micros.map(|n| n.to_string()).unwrap_or_default(),
                            c.metrics
                                .as_ref()
                                .and_then(|m| m.impressions)
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                            c.metrics
                                .as_ref()
                                .and_then(|m| m.clicks)
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                            c.metrics
                                .as_ref()
                                .and_then(|m| m.cost_micros)
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::write_table(
                    &[
                        "ID",
                        "NAME",
                        "STATUS",
                        "CHANNEL",
                        "BUDGET_MICROS",
                        "IMPR",
                        "CLICKS",
                        "COST_MICROS",
                    ],
                    &table,
                )?;
            }
            Ok(())
        }
        CampaignsCmd::Get { id } => {
            let c = client.get_campaign(&cid, id)?;
            if cli.json {
                output::write_json(&c)?;
            } else {
                println!("{}\t{}\t{}", c.id, c.name, c.status);
            }
            Ok(())
        }
        CampaignsCmd::Create {
            name,
            channel_type,
            status,
            budget_micros,
            budget_resource,
            bidding,
            target_cpa_micros,
            target_roas,
            eu_political,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "campaigns create")?;
            }
            let r = client.create_campaign(
                &cid,
                &CreateCampaign {
                    name: name.clone(),
                    channel_type: channel_type.clone(),
                    status: status.clone(),
                    budget_micros: *budget_micros,
                    budget_resource: budget_resource.clone(),
                    budget_display_name: None,
                    bidding: bidding.clone(),
                    target_cpa_micros: *target_cpa_micros,
                    target_roas: *target_roas,
                    contains_eu_political_advertising: eu_political.clone(),
                    dry_run: *dry_run,
                },
            )?;
            output::write_json(&r)?;
            Ok(())
        }
        CampaignsCmd::Update {
            id,
            name,
            status,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "campaigns update")?;
            }
            let r = client.update_campaign(
                &cid,
                id,
                &adscli_api::UpdateCampaign {
                    name: name.clone(),
                    status: status.clone(),
                    dry_run: *dry_run,
                },
            )?;
            output::write_json(&r)?;
            Ok(())
        }
        CampaignsCmd::Enable { id, dry_run, yes } => {
            mutate_status(&client, &cid, id, "ENABLED", *dry_run, *yes)
        }
        CampaignsCmd::Pause { id, dry_run, yes } => {
            mutate_status(&client, &cid, id, "PAUSED", *dry_run, *yes)
        }
        CampaignsCmd::Remove { id, dry_run, yes } => {
            if !*dry_run {
                runtime::require_yes(*yes, "campaigns remove")?;
            }
            let r = client.remove_campaign(&cid, id, *dry_run)?;
            output::write_json(&r)?;
            Ok(())
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => output::emit_error(cli.json, &e),
    }
}

fn mutate_status<T: adscli_api::Transport>(
    client: &adscli_api::AdsClient<T>,
    cid: &str,
    id: &str,
    status: &str,
    dry_run: bool,
    yes: bool,
) -> Result<(), adscli_api::ApiError> {
    if !dry_run {
        runtime::require_yes(yes, &format!("campaigns {status}"))?;
    }
    let r = client.set_campaign_status(cid, id, status, dry_run)?;
    output::write_json(&r)?;
    Ok(())
}
