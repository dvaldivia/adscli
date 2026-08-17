//! GAQL builders. All list/get/performance commands go through here so the
//! emitted query is stable and testable.

use crate::error::ApiError;

pub const PRESET_DURINGS: &[&str] = &[
    "TODAY",
    "YESTERDAY",
    "LAST_7_DAYS",
    "LAST_14_DAYS",
    "LAST_30_DAYS",
    "THIS_WEEK_SUN_TODAY",
    "THIS_WEEK_MON_TODAY",
    "LAST_WEEK_SUN_SAT",
    "LAST_WEEK_MON_SUN",
    "THIS_MONTH",
    "LAST_MONTH",
    "THIS_QUARTER",
    "LAST_QUARTER",
    "THIS_YEAR",
    "LAST_YEAR",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateRange {
    During(String),
    Between { from: String, to: String },
}

impl DateRange {
    pub fn parse(
        during: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Self, ApiError> {
        match (during, from, to) {
            (Some(d), None, None) => {
                let u = d.trim().to_ascii_uppercase();
                if !PRESET_DURINGS.contains(&u.as_str()) {
                    return Err(ApiError::usage(format!(
                        "unknown --during {d:?}; expected one of {}",
                        PRESET_DURINGS.join(", ")
                    )));
                }
                Ok(Self::During(u))
            }
            (None, Some(f), Some(t)) => Ok(Self::Between {
                from: f.to_string(),
                to: t.to_string(),
            }),
            (None, None, None) => Ok(Self::During("LAST_30_DAYS".into())),
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(ApiError::usage(
                "use either --during OR --from/--to, not both",
            )),
            (None, Some(_), None) | (None, None, Some(_)) => Err(ApiError::usage(
                "--from and --to must be used together (YYYY-MM-DD)",
            )),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::During(d) => d.clone(),
            Self::Between { from, to } => format!("{from}..{to}"),
        }
    }
}

pub fn date_range_clause(range: &DateRange) -> String {
    match range {
        DateRange::During(d) => format!("segments.date DURING {d}"),
        DateRange::Between { from, to } => {
            format!("segments.date BETWEEN '{from}' AND '{to}'")
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub status: Option<String>,
    pub campaign_id: Option<String>,
    pub asset_group_id: Option<String>,
    pub asset_type: Option<String>,
    pub name_contains: Option<String>,
    pub limit: Option<usize>,
    pub order_by: Option<String>,
    pub with_metrics: bool,
    pub date_range: Option<DateRange>,
}

impl ListFilter {
    pub fn limit_clause(&self) -> String {
        match self.limit {
            Some(0) | None => String::new(),
            Some(n) => format!(" LIMIT {n}"),
        }
    }

    pub fn order_clause(&self) -> String {
        match &self.order_by {
            Some(f) if !f.is_empty() => {
                let (field, dir) = split_order(f);
                format!(" ORDER BY {field} {dir}")
            }
            _ => String::new(),
        }
    }
}

fn split_order(s: &str) -> (String, &'static str) {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    if let Some(f) = lower.strip_suffix(" desc") {
        return (map_order_field(f.trim()), "DESC");
    }
    if let Some(f) = lower.strip_suffix(" asc") {
        return (map_order_field(f.trim()), "ASC");
    }
    let desc = matches!(
        lower.as_str(),
        "cost" | "cost_micros" | "impressions" | "clicks" | "conversions"
    );
    (map_order_field(t), if desc { "DESC" } else { "ASC" })
}

fn map_order_field(s: &str) -> String {
    match s.to_ascii_lowercase().as_str() {
        "cost" | "cost_micros" => "metrics.cost_micros".into(),
        "impressions" => "metrics.impressions".into(),
        "clicks" => "metrics.clicks".into(),
        "conversions" => "metrics.conversions".into(),
        "name" => "campaign.name".into(),
        other => other.to_string(),
    }
}

pub fn campaign_list(filter: &ListFilter) -> String {
    let mut select = vec![
        "campaign.id",
        "campaign.name",
        "campaign.status",
        "campaign.primary_status",
        "campaign.advertising_channel_type",
        "campaign.bidding_strategy_type",
        "campaign.campaign_budget",
        "campaign.start_date_time",
        "campaign.end_date_time",
        "campaign_budget.amount_micros",
        "campaign_budget.resource_name",
    ];
    if filter.with_metrics {
        select.extend([
            "metrics.impressions",
            "metrics.clicks",
            "metrics.cost_micros",
            "metrics.conversions",
            "metrics.conversions_value",
            "metrics.ctr",
            "metrics.average_cpc",
        ]);
    }
    let mut w = Vec::new();
    if filter.with_metrics
        && let Some(r) = &filter.date_range
    {
        w.push(date_range_clause(r));
    }
    if let Some(s) = &filter.status {
        w.push(format!("campaign.status = '{}'", s.to_ascii_uppercase()));
    }
    if let Some(n) = &filter.name_contains {
        w.push(format!("campaign.name LIKE '%{}%'", escape_like(n)));
    }
    finish("campaign", &select, &w, filter)
}

pub fn campaign_get(id: &str) -> String {
    format!(
        "SELECT campaign.id, campaign.name, campaign.status, campaign.primary_status, \
         campaign.advertising_channel_type, campaign.bidding_strategy_type, \
         campaign.campaign_budget, campaign.start_date_time, campaign.end_date_time, \
         campaign_budget.amount_micros, campaign_budget.resource_name \
         FROM campaign WHERE campaign.id = {id} LIMIT 1"
    )
}

pub fn asset_group_list(filter: &ListFilter) -> String {
    let mut select = vec![
        "asset_group.id",
        "asset_group.name",
        "asset_group.status",
        "asset_group.campaign",
        "asset_group.primary_status",
        "asset_group.ad_strength",
        "asset_group.final_urls",
    ];
    if filter.with_metrics {
        select.extend([
            "metrics.impressions",
            "metrics.clicks",
            "metrics.cost_micros",
            "metrics.conversions",
            "metrics.conversions_value",
            "metrics.ctr",
        ]);
    }
    let mut w = Vec::new();
    if filter.with_metrics
        && let Some(r) = &filter.date_range
    {
        w.push(date_range_clause(r));
    }
    if let Some(s) = &filter.status {
        w.push(format!("asset_group.status = '{}'", s.to_ascii_uppercase()));
    }
    if let Some(id) = &filter.campaign_id {
        w.push(format!("asset_group.campaign = '{}'", id));
    }
    if let Some(n) = &filter.name_contains {
        w.push(format!("asset_group.name LIKE '%{}%'", escape_like(n)));
    }
    finish("asset_group", &select, &w, filter)
}

pub fn asset_group_get(id: &str) -> String {
    format!(
        "SELECT asset_group.id, asset_group.name, asset_group.status, asset_group.campaign, \
         asset_group.primary_status, asset_group.ad_strength, asset_group.final_urls \
         FROM asset_group WHERE asset_group.id = {id} LIMIT 1"
    )
}

pub fn asset_list(filter: &ListFilter) -> String {
    let mut select = vec![
        "asset.id",
        "asset.name",
        "asset.type",
        "asset.source",
        "asset.text_asset.text",
        "asset.youtube_video_asset.youtube_video_id",
    ];
    if filter.with_metrics {
        select.extend([
            "metrics.impressions",
            "metrics.clicks",
            "metrics.cost_micros",
            "metrics.conversions",
        ]);
    }
    let mut w = Vec::new();
    if filter.with_metrics
        && let Some(r) = &filter.date_range
    {
        w.push(date_range_clause(r));
    }
    if let Some(t) = &filter.asset_type {
        w.push(format!("asset.type = '{}'", t.to_ascii_uppercase()));
    }
    if let Some(n) = &filter.name_contains {
        w.push(format!("asset.name LIKE '%{}%'", escape_like(n)));
    }
    finish("asset", &select, &w, filter)
}

pub fn asset_get(id: &str) -> String {
    format!(
        "SELECT asset.id, asset.name, asset.type, asset.source, asset.text_asset.text, \
         asset.youtube_video_asset.youtube_video_id \
         FROM asset WHERE asset.id = {id} LIMIT 1"
    )
}

pub fn asset_link_list(filter: &ListFilter) -> String {
    let mut select = vec![
        "asset_group_asset.asset_group",
        "asset_group_asset.asset",
        "asset_group_asset.field_type",
        "asset_group_asset.status",
        "asset.id",
        "asset.name",
        "asset.type",
        "asset.text_asset.text",
        "asset.youtube_video_asset.youtube_video_id",
        "asset_group.id",
    ];
    if filter.with_metrics {
        select.extend([
            "metrics.impressions",
            "metrics.clicks",
            "metrics.cost_micros",
            "metrics.conversions",
        ]);
    }
    let mut w = Vec::new();
    if filter.with_metrics
        && let Some(r) = &filter.date_range
    {
        w.push(date_range_clause(r));
    }
    if let Some(id) = &filter.asset_group_id {
        w.push(format!("asset_group.id = {id}"));
    }
    if let Some(id) = &filter.campaign_id {
        w.push(format!("asset_group.campaign = '{id}'"));
    }
    if let Some(s) = &filter.status {
        w.push(format!(
            "asset_group_asset.status = '{}'",
            s.to_ascii_uppercase()
        ));
    }
    if let Some(t) = &filter.asset_type {
        w.push(format!("asset.type = '{}'", t.to_ascii_uppercase()));
    }
    finish("asset_group_asset", &select, &w, filter)
}

pub fn customer_get() -> String {
    "SELECT customer.id, customer.descriptive_name, customer.currency_code, \
     customer.time_zone, customer.status, customer.manager FROM customer LIMIT 1"
        .into()
}

fn finish(from: &str, select: &[&str], where_parts: &[String], filter: &ListFilter) -> String {
    let mut q = format!("SELECT {} FROM {from}", select.join(", "));
    if !where_parts.is_empty() {
        q.push_str(" WHERE ");
        q.push_str(&where_parts.join(" AND "));
    }
    q.push_str(&filter.order_clause());
    q.push_str(&filter.limit_clause());
    q
}

fn escape_like(s: &str) -> String {
    s.replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_during() {
        let r = DateRange::parse(None, None, None).unwrap();
        assert_eq!(r, DateRange::During("LAST_30_DAYS".into()));
    }

    #[test]
    fn rejects_mixed_range() {
        assert!(DateRange::parse(Some("TODAY"), Some("2026-01-01"), None).is_err());
    }

    #[test]
    fn campaign_query_includes_metrics_and_limit() {
        let q = campaign_list(&ListFilter {
            status: Some("ENABLED".into()),
            with_metrics: true,
            date_range: Some(DateRange::During("LAST_7_DAYS".into())),
            limit: Some(10),
            ..Default::default()
        });
        assert!(q.contains("metrics.impressions"));
        assert!(q.contains("segments.date DURING LAST_7_DAYS"));
        assert!(q.contains("campaign.status = 'ENABLED'"));
        assert!(q.contains("LIMIT 10"));
    }
}
