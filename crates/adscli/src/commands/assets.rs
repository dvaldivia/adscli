use std::process::ExitCode;

use adscli_api::CreateAsset;

use crate::cli::{AssetsCmd, Cli};
use crate::output;
use crate::runtime;

pub fn run(cli: &Cli, cmd: &AssetsCmd) -> ExitCode {
    let client = match runtime::connect(cli) {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };
    let cid = match client.customer_id() {
        Ok(c) => c,
        Err(e) => return output::emit_error(cli.json, &e),
    };

    let result = (|| match cmd {
        AssetsCmd::List {
            opts,
            campaign,
            asset_group,
            asset_type,
            links,
        } => {
            let mut filter = crate::cli::filter_from(opts)?;
            filter.campaign_id = campaign.clone();
            filter.asset_group_id = asset_group.clone();
            filter.asset_type = asset_type.clone();
            if *links || campaign.is_some() || asset_group.is_some() {
                let rows = client.list_asset_links(&cid, &filter)?;
                if cli.quiet {
                    output::write_quiet(
                        &rows.iter().map(|l| l.asset_id.clone()).collect::<Vec<_>>(),
                    );
                    return Ok(());
                }
                if cli.json {
                    output::write_json(&rows)?;
                } else {
                    let table: Vec<Vec<String>> = rows
                        .iter()
                        .map(|l| {
                            vec![
                                l.asset_id.clone(),
                                l.asset_group_id.clone(),
                                l.field_type.clone(),
                                l.status.clone(),
                                l.asset
                                    .as_ref()
                                    .and_then(|a| a.text.clone().or(a.name.clone()))
                                    .unwrap_or_default(),
                            ]
                        })
                        .collect();
                    output::write_table(
                        &["ASSET", "ASSET_GROUP", "FIELD", "STATUS", "TEXT"],
                        &table,
                    )?;
                }
            } else {
                let rows = client.list_assets(&cid, &filter)?;
                if cli.quiet {
                    output::write_quiet(&rows.iter().map(|a| a.id.clone()).collect::<Vec<_>>());
                    return Ok(());
                }
                if cli.json {
                    output::write_json(&rows)?;
                } else {
                    let table: Vec<Vec<String>> = rows
                        .iter()
                        .map(|a| {
                            vec![
                                a.id.clone(),
                                a.r#type.clone().unwrap_or_default(),
                                a.name.clone().unwrap_or_default(),
                                a.text.clone().unwrap_or_default(),
                            ]
                        })
                        .collect();
                    output::write_table(&["ID", "TYPE", "NAME", "TEXT"], &table)?;
                }
            }
            Ok(())
        }
        AssetsCmd::Get { id } => {
            let a = client.get_asset(&cid, id)?;
            if cli.json {
                output::write_json(&a)?;
            } else {
                println!(
                    "{}\t{}\t{}",
                    a.id,
                    a.r#type.as_deref().unwrap_or("-"),
                    a.text.as_deref().or(a.name.as_deref()).unwrap_or("-")
                );
            }
            Ok(())
        }
        AssetsCmd::Create {
            asset_type,
            name,
            text,
            file,
            youtube_id,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "assets create")?;
            }
            let r = client.create_asset(
                &cid,
                &CreateAsset {
                    r#type: asset_type.clone(),
                    name: name.clone(),
                    text: text.clone(),
                    file: file.clone(),
                    youtube_id: youtube_id.clone(),
                    dry_run: *dry_run,
                },
            )?;
            output::write_json(&r)?;
            Ok(())
        }
        AssetsCmd::Update {
            id,
            name,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "assets update")?;
            }
            let r = client.update_asset_name(&cid, id, name, *dry_run)?;
            output::write_json(&r)?;
            Ok(())
        }
        AssetsCmd::Link {
            asset_group,
            asset,
            field_type,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "assets link")?;
            }
            let r = client.link_asset(&cid, asset_group, asset, field_type, *dry_run)?;
            output::write_json(&r)?;
            Ok(())
        }
        AssetsCmd::Unlink {
            asset_group,
            asset,
            field_type,
            dry_run,
            yes,
        } => {
            if !*dry_run {
                runtime::require_yes(*yes, "assets unlink")?;
            }
            let r = client.unlink_asset(&cid, asset_group, asset, field_type, *dry_run)?;
            output::write_json(&r)?;
            Ok(())
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => output::emit_error(cli.json, &e),
    }
}
