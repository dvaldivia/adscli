use std::process::ExitCode;

use adscli_api::{CreateAssetGroup, UpdateAssetGroup};

use crate::cli::{AssetGroupsCmd, Cli};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &AssetGroupsCmd) -> ExitCode {
    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let cid = match client.customer_id() {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };

    let result = (|| match cmd {
        AssetGroupsCmd::List { opts, campaign } => {
            let mut filter = crate::cli::filter_from(opts)?;
            filter.campaign_id = campaign.clone();
            let rows = client.list_asset_groups(&cid, &filter)?;
            if cli.quiet {
                output::write_quiet(&rows.iter().map(|g| g.id.clone()).collect::<Vec<_>>());
                return Ok(());
            }
            if cli.json {
                output::write_json(&rows)?;
            } else {
                let table: Vec<Vec<String>> = rows
                    .iter()
                    .map(|g| {
                        vec![
                            g.id.clone(),
                            g.name.clone(),
                            g.status.clone(),
                            g.campaign_id.clone().unwrap_or_default(),
                            g.ad_strength.clone().unwrap_or_default(),
                            g.metrics
                                .as_ref()
                                .and_then(|m| m.impressions)
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::write_table(
                    &["ID", "NAME", "STATUS", "CAMPAIGN", "STRENGTH", "IMPR"],
                    &table,
                )?;
            }
            Ok(())
        }
        AssetGroupsCmd::Get { id } => {
            let g = client.get_asset_group(&cid, id)?;
            if cli.json {
                output::write_json(&g)?;
            } else {
                println!("{}\t{}\t{}", g.id, g.name, g.status);
            }
            Ok(())
        }
        AssetGroupsCmd::Create {
            name,
            campaign,
            status,
            final_urls,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "asset-groups create")?;
            }
            let r = client.create_asset_group(
                &cid,
                &CreateAssetGroup {
                    name: name.clone(),
                    campaign: campaign.clone(),
                    status: status.clone(),
                    final_urls: final_urls.clone(),
                    dry_run: *dry_run,
                },
            )?;
            output::write_json(&r)?;
            Ok(())
        }
        AssetGroupsCmd::Update {
            id,
            name,
            status,
            final_urls,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "asset-groups update")?;
            }
            let r = client.update_asset_group(
                &cid,
                id,
                &UpdateAssetGroup {
                    name: name.clone(),
                    status: status.clone(),
                    final_urls: if final_urls.is_empty() {
                        None
                    } else {
                        Some(final_urls.clone())
                    },
                    dry_run: *dry_run,
                },
            )?;
            output::write_json(&r)?;
            Ok(())
        }
        AssetGroupsCmd::Enable { id, dry_run, yes } => {
            status_change(&client, &cid, id, "ENABLED", *dry_run, *yes)
        }
        AssetGroupsCmd::Pause { id, dry_run, yes } => {
            status_change(&client, &cid, id, "PAUSED", *dry_run, *yes)
        }
        AssetGroupsCmd::Remove { id, dry_run, yes } => {
            if !*dry_run {
                runtime::require_yes(*yes, "asset-groups remove")?;
            }
            let r = client.remove_asset_group(&cid, id, *dry_run)?;
            output::write_json(&r)?;
            Ok(())
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => output::emit_error(cli.json, &e),
    }
}

fn status_change<T: adscli_api::Transport>(
    client: &adscli_api::AdsClient<T>,
    cid: &str,
    id: &str,
    status: &str,
    dry_run: bool,
    yes: bool,
) -> Result<(), adscli_api::ApiError> {
    if !dry_run {
        runtime::require_yes(yes, "asset-groups status")?;
    }
    let r = client.set_asset_group_status(cid, id, status, dry_run)?;
    output::write_json(&r)?;
    Ok(())
}
