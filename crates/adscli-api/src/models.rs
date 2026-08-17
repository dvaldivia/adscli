use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jsonpath::{get_f64, get_i64, get_string, get_string_list};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Metrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impressions: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clicks: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_micros: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversions: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversions_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_cpc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_conversions: Option<f64>,
}

impl Metrics {
    pub fn from_row(row: &Value) -> Option<Self> {
        if row.get("metrics").is_none() {
            return None;
        }
        Some(Self {
            impressions: get_i64(row, "metrics.impressions"),
            clicks: get_i64(row, "metrics.clicks"),
            cost_micros: get_i64(row, "metrics.cost_micros"),
            conversions: get_f64(row, "metrics.conversions"),
            conversions_value: get_f64(row, "metrics.conversions_value"),
            ctr: get_f64(row, "metrics.ctr"),
            average_cpc: get_f64(row, "metrics.average_cpc"),
            all_conversions: get_f64(row, "metrics.all_conversions"),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.impressions.is_none()
            && self.clicks.is_none()
            && self.cost_micros.is_none()
            && self.conversions.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Customer {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptive_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager: Option<bool>,
}

impl Customer {
    pub fn from_accessible_name(resource_name: &str) -> Self {
        let id = adscli_config::extract_resource_id(resource_name);
        Self {
            id,
            resource_name: Some(resource_name.to_string()),
            ..Default::default()
        }
    }

    pub fn from_row(row: &Value) -> Self {
        let resource_name = get_string(row, "customer.resource_name");
        let id = get_string(row, "customer.id").unwrap_or_else(|| {
            resource_name
                .as_deref()
                .map(adscli_config::extract_resource_id)
                .unwrap_or_default()
        });
        Self {
            id,
            resource_name,
            descriptive_name: get_string(row, "customer.descriptive_name"),
            currency_code: get_string(row, "customer.currency_code"),
            time_zone: get_string(row, "customer.time_zone"),
            status: get_string(row, "customer.status"),
            manager: crate::jsonpath::walk(row, "customer.manager").and_then(|v| v.as_bool()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Campaign {
    pub id: String,
    pub resource_name: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bidding_strategy_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_micros: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_resource_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
}

impl Campaign {
    pub fn from_row(row: &Value) -> Self {
        let resource_name = get_string(row, "campaign.resource_name").unwrap_or_default();
        let id = get_string(row, "campaign.id")
            .unwrap_or_else(|| adscli_config::extract_resource_id(&resource_name));
        Self {
            id,
            resource_name,
            name: get_string(row, "campaign.name").unwrap_or_default(),
            status: get_string(row, "campaign.status").unwrap_or_default(),
            primary_status: get_string(row, "campaign.primary_status"),
            channel_type: get_string(row, "campaign.advertising_channel_type"),
            bidding_strategy_type: get_string(row, "campaign.bidding_strategy_type"),
            budget_micros: get_i64(row, "campaign_budget.amount_micros"),
            budget_resource_name: get_string(row, "campaign.campaign_budget")
                .or_else(|| get_string(row, "campaign_budget.resource_name")),
            start_date_time: get_string(row, "campaign.start_date_time"),
            end_date_time: get_string(row, "campaign.end_date_time"),
            metrics: Metrics::from_row(row),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssetGroup {
    pub id: String,
    pub resource_name: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ad_strength: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub final_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
}

impl AssetGroup {
    pub fn from_row(row: &Value) -> Self {
        let resource_name = get_string(row, "asset_group.resource_name").unwrap_or_default();
        let id = get_string(row, "asset_group.id")
            .unwrap_or_else(|| adscli_config::extract_resource_id(&resource_name));
        let campaign = get_string(row, "asset_group.campaign");
        let campaign_id = campaign.as_deref().map(adscli_config::extract_resource_id);
        Self {
            id,
            resource_name,
            name: get_string(row, "asset_group.name").unwrap_or_default(),
            status: get_string(row, "asset_group.status").unwrap_or_default(),
            campaign,
            campaign_id,
            primary_status: get_string(row, "asset_group.primary_status"),
            ad_strength: get_string(row, "asset_group.ad_strength"),
            final_urls: get_string_list(row, "asset_group.final_urls"),
            metrics: Metrics::from_row(row),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub id: String,
    pub resource_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_video_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
}

impl Asset {
    pub fn from_row(row: &Value) -> Self {
        let resource_name = get_string(row, "asset.resource_name").unwrap_or_default();
        let id = get_string(row, "asset.id")
            .unwrap_or_else(|| adscli_config::extract_resource_id(&resource_name));
        Self {
            id,
            resource_name,
            name: get_string(row, "asset.name"),
            r#type: get_string(row, "asset.type"),
            text: get_string(row, "asset.text_asset.text"),
            youtube_video_id: get_string(row, "asset.youtube_video_asset.youtube_video_id"),
            source: get_string(row, "asset.source"),
            metrics: Metrics::from_row(row),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssetLink {
    pub asset_group_id: String,
    pub asset_id: String,
    pub field_type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<Asset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
}

impl AssetLink {
    pub fn from_row(row: &Value) -> Self {
        let ag = get_string(row, "asset_group_asset.asset_group").unwrap_or_default();
        let asset = get_string(row, "asset_group_asset.asset").unwrap_or_default();
        Self {
            asset_group_id: adscli_config::extract_resource_id(&ag),
            asset_id: get_string(row, "asset.id")
                .unwrap_or_else(|| adscli_config::extract_resource_id(&asset)),
            field_type: get_string(row, "asset_group_asset.field_type").unwrap_or_default(),
            status: get_string(row, "asset_group_asset.status").unwrap_or_default(),
            asset: Some(Asset::from_row(row)),
            metrics: Metrics::from_row(row),
        }
    }
}

/// Flattened performance row used by `adscli performance *`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PerformanceRow {
    pub resource: String,
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,
    #[serde(flatten)]
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MutateResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

impl MutateResult {
    pub fn from_mutate_response(body: &Value) -> Self {
        let first = body
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|a| a.first());
        let resource_name = first
            .and_then(|r| r.get("resourceName").or_else(|| r.get("resource_name")))
            .and_then(crate::jsonpath::as_string);
        let id = resource_name
            .as_deref()
            .map(adscli_config::extract_resource_id);
        Self {
            resource_name,
            id,
            dry_run: None,
            request: None,
            raw: Some(body.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_campaign_row() {
        let row = json!({
            "campaign": {
                "resourceName": "customers/1/campaigns/9",
                "id": "9",
                "name": "Brand",
                "status": "ENABLED",
                "advertisingChannelType": "SEARCH"
            },
            "campaignBudget": { "amountMicros": "5000000" },
            "metrics": { "impressions": "100", "clicks": "4", "costMicros": "250000" }
        });
        let c = Campaign::from_row(&row);
        assert_eq!(c.id, "9");
        assert_eq!(c.name, "Brand");
        assert_eq!(c.channel_type.as_deref(), Some("SEARCH"));
        assert_eq!(c.budget_micros, Some(5_000_000));
        assert_eq!(c.metrics.unwrap().impressions, Some(100));
    }
}
