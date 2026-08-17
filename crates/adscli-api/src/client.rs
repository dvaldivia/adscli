//! HTTP transport + high-level Google Ads operations.

use std::fs;
use std::path::Path;

use adscli_config::{Settings, normalize_customer_id, resource_name};
use base64::Engine;
use serde_json::{Value, json};

use crate::error::ApiError;
use crate::models::{
    Asset, AssetGroup, AssetLink, Campaign, Customer, MutateResult, PerformanceRow,
};
use crate::query::{self, DateRange, ListFilter};

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub body: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

pub trait Transport: Send + Sync {
    fn send(
        &self,
        req: &HttpRequest,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, ApiError>;
}

pub struct ReqwestTransport {
    inner: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self, ApiError> {
        let inner = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| ApiError::transport(e.to_string()))?;
        Ok(Self { inner })
    }
}

impl Transport for ReqwestTransport {
    fn send(
        &self,
        req: &HttpRequest,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, ApiError> {
        let mut b = match req.method.as_str() {
            "GET" => self.inner.get(&req.url),
            _ => self.inner.post(&req.url),
        };
        for (k, v) in headers {
            b = b.header(k, v);
        }
        if let Some(body) = &req.body {
            b = b.json(body);
        }
        let resp = b.send().map_err(|e| ApiError::transport(e.to_string()))?;
        Ok(HttpResponse {
            status: resp.status().as_u16(),
            body: resp.text().unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRun {
    pub dry_run: bool,
    pub method: String,
    pub url: String,
    pub body: Value,
}

pub struct AdsClient<T: Transport> {
    pub settings: Settings,
    transport: T,
}

impl<T: Transport> AdsClient<T> {
    pub fn new(settings: Settings, transport: T) -> Result<Self, ApiError> {
        if !settings.has_developer_token() {
            return Err(ApiError::config(
                "developer token is required (ADSCLI_DEVELOPER_TOKEN or config.yaml)",
            )
            .suggest("https://developers.google.com/google-ads/api/docs/get-started/dev-token"));
        }
        if !settings.has_access_token() && !settings.has_refresh_token() {
            return Err(ApiError::auth("not authenticated"));
        }
        Ok(Self {
            settings,
            transport,
        })
    }

    pub fn customer_id(&self) -> Result<String, ApiError> {
        if self.settings.customer_id.is_empty() {
            return Err(ApiError::usage(
                "customer id is required (--customer-id, ADSCLI_CUSTOMER_ID, or config.yaml)",
            )
            .suggest("adscli customers list --json"));
        }
        Ok(self.settings.customer_id.clone())
    }

    fn headers(&self) -> Vec<(String, String)> {
        let mut h = vec![
            ("Content-Type".into(), "application/json".into()),
            (
                "Authorization".into(),
                format!("Bearer {}", self.settings.access_token),
            ),
            (
                "developer-token".into(),
                self.settings.developer_token.clone(),
            ),
        ];
        if !self.settings.login_customer_id.is_empty() {
            h.push((
                "login-customer-id".into(),
                self.settings.login_customer_id.clone(),
            ));
        }
        h
    }

    fn send(&self, req: &HttpRequest) -> Result<Value, ApiError> {
        let resp = self.transport.send(req, &self.headers())?;
        if !(200..300).contains(&resp.status) {
            return Err(ApiError::from_http(resp.status, &resp.body));
        }
        if resp.body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&resp.body)
            .map_err(|e| ApiError::transport(format!("response JSON: {e}: {}", resp.body)))
    }

    pub fn search(&self, customer_id: &str, query: &str) -> Result<Vec<Value>, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let mut page_token: Option<String> = None;
        let mut rows = Vec::new();
        loop {
            let mut body = json!({ "query": query });
            if let Some(t) = &page_token {
                body["pageToken"] = json!(t);
            }
            let url = format!(
                "{}/customers/{cid}/googleAds:search",
                self.settings.api_base
            );
            let resp = self.send(&HttpRequest {
                method: "POST".into(),
                url,
                body: Some(body),
            })?;
            if let Some(results) = resp.get("results").and_then(|r| r.as_array()) {
                rows.extend(results.iter().cloned());
            }
            match resp.get("nextPageToken").and_then(|t| t.as_str()) {
                Some(t) if !t.is_empty() => page_token = Some(t.to_string()),
                _ => break,
            }
        }
        Ok(rows)
    }

    pub fn search_raw(&self, customer_id: &str, query: &str) -> Result<Vec<Value>, ApiError> {
        self.search(customer_id, query)
    }

    pub fn mutate(
        &self,
        customer_id: &str,
        collection: &str,
        operations: Value,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let url = format!(
            "{}/customers/{cid}/{collection}:mutate",
            self.settings.api_base
        );
        let body = json!({
            "operations": operations,
            "responseContentType": "MUTABLE_RESOURCE",
        });
        if dry_run {
            return Ok(MutateResult {
                dry_run: Some(true),
                request: Some(json!({"method": "POST", "url": url, "body": body})),
                ..Default::default()
            });
        }
        let resp = self.send(&HttpRequest {
            method: "POST".into(),
            url,
            body: Some(body),
        })?;
        Ok(MutateResult::from_mutate_response(&resp))
    }

    // --- customers -------------------------------------------------------

    pub fn list_accessible_customers(&self) -> Result<Vec<Customer>, ApiError> {
        let url = format!(
            "{}/customers:listAccessibleCustomers",
            self.settings.api_base
        );
        let resp = self.send(&HttpRequest {
            method: "GET".into(),
            url,
            body: None,
        })?;
        let names = resp
            .get("resourceNames")
            .or_else(|| resp.get("resource_names"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(names
            .iter()
            .filter_map(|v| v.as_str())
            .map(Customer::from_accessible_name)
            .collect())
    }

    pub fn get_customer(&self, customer_id: &str) -> Result<Customer, ApiError> {
        let rows = self.search(customer_id, &query::customer_get())?;
        rows.first()
            .map(Customer::from_row)
            .ok_or_else(|| ApiError::not_found(format!("customer {customer_id} not found")))
    }

    // --- campaigns -------------------------------------------------------

    pub fn list_campaigns(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<Campaign>, ApiError> {
        let rows = self.search(customer_id, &query::campaign_list(filter))?;
        Ok(rows.iter().map(Campaign::from_row).collect())
    }

    pub fn get_campaign(&self, customer_id: &str, id: &str) -> Result<Campaign, ApiError> {
        let id = adscli_config::extract_resource_id(id);
        let rows = self.search(customer_id, &query::campaign_get(&id))?;
        rows.first().map(Campaign::from_row).ok_or_else(|| {
            ApiError::not_found(format!("campaign {id} not found"))
                .suggest("adscli campaigns list --json")
        })
    }

    pub fn create_campaign(
        &self,
        customer_id: &str,
        spec: &CreateCampaign,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let budget_name = spec
            .budget_resource
            .clone()
            .unwrap_or_else(|| format!("customers/{cid}/campaignBudgets/__pending__"));

        if spec.dry_run && spec.budget_resource.is_none() {
            let budget_op = json!([{
                "create": {
                    "name": spec.budget_name(),
                    "amountMicros": spec.budget_micros.to_string(),
                    "deliveryMethod": "STANDARD",
                    "explicitlyShared": false
                }
            }]);
            let camp = campaign_create_body(&cid, spec, &budget_name);
            return Ok(MutateResult {
                dry_run: Some(true),
                request: Some(json!({
                    "steps": [
                        {"collection": "campaignBudgets", "operations": budget_op},
                        {"collection": "campaigns", "operations": [{"create": camp}]}
                    ]
                })),
                ..Default::default()
            });
        }

        let budget_rn = if let Some(rn) = &spec.budget_resource {
            rn.clone()
        } else {
            let created = self.mutate(
                &cid,
                "campaignBudgets",
                json!([{
                    "create": {
                        "name": spec.budget_name(),
                        "amountMicros": spec.budget_micros.to_string(),
                        "deliveryMethod": "STANDARD",
                        "explicitlyShared": false
                    }
                }]),
                false,
            )?;
            created.resource_name.ok_or_else(|| {
                ApiError::new(
                    crate::error::ErrorKind::Google,
                    "campaign budget mutate returned no resourceName",
                )
            })?
        };

        let body = campaign_create_body(&cid, spec, &budget_rn);
        self.mutate(&cid, "campaigns", json!([{ "create": body }]), spec.dry_run)
    }

    pub fn update_campaign(
        &self,
        customer_id: &str,
        id: &str,
        fields: &UpdateCampaign,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let rn = resource_name(&cid, "campaigns", id);
        let mut mask = Vec::new();
        let mut update = json!({ "resourceName": rn });
        if let Some(n) = &fields.name {
            update["name"] = json!(n);
            mask.push("name");
        }
        if let Some(s) = &fields.status {
            update["status"] = json!(s);
            mask.push("status");
        }
        if mask.is_empty() {
            return Err(ApiError::usage(
                "nothing to update; pass --name and/or --status",
            ));
        }
        self.mutate(
            &cid,
            "campaigns",
            json!([{ "update": update, "updateMask": mask.join(",") }]),
            fields.dry_run,
        )
    }

    pub fn set_campaign_status(
        &self,
        customer_id: &str,
        id: &str,
        status: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        self.update_campaign(
            customer_id,
            id,
            &UpdateCampaign {
                name: None,
                status: Some(status.to_ascii_uppercase()),
                dry_run,
            },
        )
    }

    pub fn remove_campaign(
        &self,
        customer_id: &str,
        id: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let rn = resource_name(&cid, "campaigns", id);
        self.mutate(&cid, "campaigns", json!([{ "remove": rn }]), dry_run)
    }

    // --- asset groups ----------------------------------------------------

    pub fn list_asset_groups(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<AssetGroup>, ApiError> {
        let mut f = filter.clone();
        if let Some(id) = &f.campaign_id {
            let cid = normalize_customer_id(customer_id);
            f.campaign_id = Some(resource_name(&cid, "campaigns", id));
        }
        let rows = self.search(customer_id, &query::asset_group_list(&f))?;
        Ok(rows.iter().map(AssetGroup::from_row).collect())
    }

    pub fn get_asset_group(&self, customer_id: &str, id: &str) -> Result<AssetGroup, ApiError> {
        let id = adscli_config::extract_resource_id(id);
        let rows = self.search(customer_id, &query::asset_group_get(&id))?;
        rows.first().map(AssetGroup::from_row).ok_or_else(|| {
            ApiError::not_found(format!("asset group {id} not found"))
                .suggest("adscli asset-groups list --json")
        })
    }

    pub fn create_asset_group(
        &self,
        customer_id: &str,
        spec: &CreateAssetGroup,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let campaign = resource_name(&cid, "campaigns", &spec.campaign);
        let mut body = json!({
            "name": spec.name,
            "campaign": campaign,
            "status": spec.status,
        });
        if !spec.final_urls.is_empty() {
            body["finalUrls"] = json!(spec.final_urls);
        }
        self.mutate(
            &cid,
            "assetGroups",
            json!([{ "create": body }]),
            spec.dry_run,
        )
    }

    pub fn update_asset_group(
        &self,
        customer_id: &str,
        id: &str,
        fields: &UpdateAssetGroup,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let rn = resource_name(&cid, "assetGroups", id);
        let mut mask = Vec::new();
        let mut update = json!({ "resourceName": rn });
        if let Some(n) = &fields.name {
            update["name"] = json!(n);
            mask.push("name");
        }
        if let Some(s) = &fields.status {
            update["status"] = json!(s);
            mask.push("status");
        }
        if let Some(urls) = &fields.final_urls {
            update["finalUrls"] = json!(urls);
            mask.push("final_urls");
        }
        if mask.is_empty() {
            return Err(ApiError::usage(
                "nothing to update; pass --name, --status, and/or --final-url",
            ));
        }
        self.mutate(
            &cid,
            "assetGroups",
            json!([{ "update": update, "updateMask": mask.join(",") }]),
            fields.dry_run,
        )
    }

    pub fn set_asset_group_status(
        &self,
        customer_id: &str,
        id: &str,
        status: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        self.update_asset_group(
            customer_id,
            id,
            &UpdateAssetGroup {
                name: None,
                status: Some(status.to_ascii_uppercase()),
                final_urls: None,
                dry_run,
            },
        )
    }

    pub fn remove_asset_group(
        &self,
        customer_id: &str,
        id: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let rn = resource_name(&cid, "assetGroups", id);
        self.mutate(&cid, "assetGroups", json!([{ "remove": rn }]), dry_run)
    }

    // --- assets ----------------------------------------------------------

    pub fn list_assets(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<Asset>, ApiError> {
        let rows = self.search(customer_id, &query::asset_list(filter))?;
        Ok(rows.iter().map(Asset::from_row).collect())
    }

    pub fn get_asset(&self, customer_id: &str, id: &str) -> Result<Asset, ApiError> {
        let id = adscli_config::extract_resource_id(id);
        let rows = self.search(customer_id, &query::asset_get(&id))?;
        rows.first().map(Asset::from_row).ok_or_else(|| {
            ApiError::not_found(format!("asset {id} not found"))
                .suggest("adscli assets list --json")
        })
    }

    pub fn list_asset_links(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<AssetLink>, ApiError> {
        let mut f = filter.clone();
        if let Some(id) = &f.campaign_id {
            let cid = normalize_customer_id(customer_id);
            f.campaign_id = Some(resource_name(&cid, "campaigns", id));
        }
        let rows = self.search(customer_id, &query::asset_link_list(&f))?;
        Ok(rows.iter().map(AssetLink::from_row).collect())
    }

    pub fn create_asset(
        &self,
        customer_id: &str,
        spec: &CreateAsset,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let body = asset_create_body(spec)?;
        self.mutate(&cid, "assets", json!([{ "create": body }]), spec.dry_run)
    }

    pub fn update_asset_name(
        &self,
        customer_id: &str,
        id: &str,
        name: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let rn = resource_name(&cid, "assets", id);
        self.mutate(
            &cid,
            "assets",
            json!([{
                "update": { "resourceName": rn, "name": name },
                "updateMask": "name"
            }]),
            dry_run,
        )
    }

    pub fn link_asset(
        &self,
        customer_id: &str,
        asset_group: &str,
        asset: &str,
        field_type: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let body = json!({
            "assetGroup": resource_name(&cid, "assetGroups", asset_group),
            "asset": resource_name(&cid, "assets", asset),
            "fieldType": field_type.to_ascii_uppercase(),
        });
        self.mutate(
            &cid,
            "assetGroupAssets",
            json!([{ "create": body }]),
            dry_run,
        )
    }

    pub fn unlink_asset(
        &self,
        customer_id: &str,
        asset_group: &str,
        asset: &str,
        field_type: &str,
        dry_run: bool,
    ) -> Result<MutateResult, ApiError> {
        let cid = normalize_customer_id(customer_id);
        let rn = format!(
            "customers/{cid}/assetGroupAssets/{}_{}_{}",
            adscli_config::extract_resource_id(asset_group),
            adscli_config::extract_resource_id(asset),
            field_type.to_ascii_uppercase()
        );
        self.mutate(&cid, "assetGroupAssets", json!([{ "remove": rn }]), dry_run)
    }

    // --- performance -----------------------------------------------------

    pub fn performance_campaigns(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<PerformanceRow>, ApiError> {
        let mut f = filter.clone();
        f.with_metrics = true;
        if f.date_range.is_none() {
            f.date_range = Some(DateRange::During("LAST_30_DAYS".into()));
        }
        Ok(self
            .list_campaigns(customer_id, &f)?
            .into_iter()
            .map(|c| PerformanceRow {
                resource: "campaign".into(),
                id: c.id,
                name: c.name,
                status: Some(c.status),
                parent_id: None,
                field_type: c.channel_type,
                metrics: c.metrics.unwrap_or_default(),
            })
            .collect())
    }

    pub fn performance_asset_groups(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<PerformanceRow>, ApiError> {
        let mut f = filter.clone();
        f.with_metrics = true;
        if f.date_range.is_none() {
            f.date_range = Some(DateRange::During("LAST_30_DAYS".into()));
        }
        Ok(self
            .list_asset_groups(customer_id, &f)?
            .into_iter()
            .map(|g| PerformanceRow {
                resource: "asset_group".into(),
                id: g.id,
                name: g.name,
                status: Some(g.status),
                parent_id: g.campaign_id,
                field_type: None,
                metrics: g.metrics.unwrap_or_default(),
            })
            .collect())
    }

    pub fn performance_assets(
        &self,
        customer_id: &str,
        filter: &ListFilter,
    ) -> Result<Vec<PerformanceRow>, ApiError> {
        let mut f = filter.clone();
        f.with_metrics = true;
        if f.date_range.is_none() {
            f.date_range = Some(DateRange::During("LAST_30_DAYS".into()));
        }
        Ok(self
            .list_asset_links(customer_id, &f)?
            .into_iter()
            .map(|l| {
                let a = l.asset.clone().unwrap_or_default();
                PerformanceRow {
                    resource: "asset".into(),
                    id: l.asset_id,
                    name: a.name.or(a.text).unwrap_or_else(|| a.resource_name.clone()),
                    status: Some(l.status),
                    parent_id: Some(l.asset_group_id),
                    field_type: Some(l.field_type),
                    metrics: l.metrics.unwrap_or_default(),
                }
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct CreateCampaign {
    pub name: String,
    pub channel_type: String,
    pub status: String,
    pub budget_micros: i64,
    pub budget_resource: Option<String>,
    pub budget_display_name: Option<String>,
    pub bidding: String,
    pub target_cpa_micros: Option<i64>,
    pub target_roas: Option<f64>,
    pub contains_eu_political_advertising: String,
    pub dry_run: bool,
}

impl CreateCampaign {
    fn budget_name(&self) -> String {
        self.budget_display_name
            .clone()
            .unwrap_or_else(|| format!("Budget for {}", self.name))
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpdateCampaign {
    pub name: Option<String>,
    pub status: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct CreateAssetGroup {
    pub name: String,
    pub campaign: String,
    pub status: String,
    pub final_urls: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateAssetGroup {
    pub name: Option<String>,
    pub status: Option<String>,
    pub final_urls: Option<Vec<String>>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct CreateAsset {
    pub r#type: String,
    pub name: Option<String>,
    pub text: Option<String>,
    pub file: Option<std::path::PathBuf>,
    pub youtube_id: Option<String>,
    pub dry_run: bool,
}

fn campaign_create_body(customer_id: &str, spec: &CreateCampaign, budget_rn: &str) -> Value {
    let channel = spec.channel_type.to_ascii_uppercase();
    let mut body = json!({
        "name": spec.name,
        "status": spec.status.to_ascii_uppercase(),
        "advertisingChannelType": channel,
        "campaignBudget": budget_rn,
        "containsEuPoliticalAdvertising": spec.contains_eu_political_advertising,
    });
    apply_bidding(
        &mut body,
        &spec.bidding,
        spec.target_cpa_micros,
        spec.target_roas,
    );
    if channel == "SEARCH" {
        body["networkSettings"] = json!({
            "targetGoogleSearch": true,
            "targetSearchNetwork": true,
            "targetContentNetwork": false,
            "targetPartnerSearchNetwork": false
        });
    }
    if channel == "PERFORMANCE_MAX" {
        body["brandGuidelinesEnabled"] = json!(false);
    }
    let _ = customer_id;
    body
}

fn apply_bidding(body: &mut Value, bidding: &str, tca: Option<i64>, troas: Option<f64>) {
    match bidding.to_ascii_lowercase().as_str() {
        "maximize_conversions" | "maximize-conversions" => {
            let mut v = json!({});
            if let Some(m) = tca {
                v["targetCpaMicros"] = json!(m.to_string());
            }
            body["maximizeConversions"] = v;
        }
        "maximize_conversion_value" | "maximize-conversion-value" => {
            let mut v = json!({});
            if let Some(r) = troas {
                v["targetRoas"] = json!(r);
            }
            body["maximizeConversionValue"] = v;
        }
        "target_cpa" | "target-cpa" => {
            body["targetCpa"] = json!({
                "targetCpaMicros": tca.unwrap_or(0).to_string()
            });
        }
        "target_roas" | "target-roas" => {
            body["targetRoas"] = json!({
                "targetRoas": troas.unwrap_or(0.0)
            });
        }
        "manual_cpc" | "manual-cpc" => {
            body["manualCpc"] = json!({ "enhancedCpcEnabled": false });
        }
        "maximize_clicks" | "maximize-clicks" => {
            body["targetSpend"] = json!({});
        }
        other => {
            body["maximizeConversions"] = json!({});
            let _ = other;
        }
    }
}

fn asset_create_body(spec: &CreateAsset) -> Result<Value, ApiError> {
    let t = spec.r#type.to_ascii_uppercase();
    let mut body = json!({});
    if let Some(n) = &spec.name {
        body["name"] = json!(n);
    }
    match t.as_str() {
        "TEXT" => {
            let text = spec
                .text
                .as_deref()
                .ok_or_else(|| ApiError::usage("--text is required for --type TEXT"))?;
            body["textAsset"] = json!({ "text": text });
        }
        "IMAGE" => {
            let path = spec
                .file
                .as_deref()
                .ok_or_else(|| ApiError::usage("--file is required for --type IMAGE"))?;
            body["imageAsset"] = json!({
                "data": read_file_b64(path)?,
                "mimeType": mime_enum(path),
            });
        }
        "YOUTUBE_VIDEO" | "YOUTUBE" => {
            let id = spec.youtube_id.as_deref().ok_or_else(|| {
                ApiError::usage("--youtube-id is required for --type YOUTUBE_VIDEO")
            })?;
            body["youtubeVideoAsset"] = json!({ "youtubeVideoId": id });
        }
        other => {
            return Err(ApiError::usage(format!(
                "unsupported --type {other}; use TEXT, IMAGE, or YOUTUBE_VIDEO"
            )));
        }
    }
    Ok(body)
}

fn read_file_b64(path: &Path) -> Result<String, ApiError> {
    let bytes =
        fs::read(path).map_err(|e| ApiError::usage(format!("read {}: {e}", path.display())))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn mime_enum(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "IMAGE_JPEG",
        "gif" => "IMAGE_GIF",
        _ => "IMAGE_PNG",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Scripted {
        responses: Mutex<Vec<HttpResponse>>,
    }

    impl Transport for Scripted {
        fn send(
            &self,
            _req: &HttpRequest,
            _h: &[(String, String)],
        ) -> Result<HttpResponse, ApiError> {
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| ApiError::transport("no scripted response"))
        }
    }

    fn settings() -> Settings {
        Settings {
            developer_token: "dev".into(),
            access_token: "tok".into(),
            skip_token_refresh: true,
            customer_id: "123".into(),
            api_base: "https://example.test/v25".into(),
            ..Settings::default()
        }
    }

    #[test]
    fn lists_campaigns_from_search() {
        let body = r#"{"results":[{"campaign":{"id":"9","name":"Brand","status":"ENABLED","resourceName":"customers/123/campaigns/9"}}]}"#;
        let t = Scripted {
            responses: Mutex::new(vec![HttpResponse {
                status: 200,
                body: body.into(),
            }]),
        };
        let c = AdsClient::new(settings(), t).unwrap();
        let rows = c.list_campaigns("123", &ListFilter::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Brand");
    }

    #[test]
    fn dry_run_does_not_call_transport() {
        let t = Scripted {
            responses: Mutex::new(vec![]),
        };
        let c = AdsClient::new(settings(), t).unwrap();
        let r = c.set_campaign_status("123", "9", "PAUSED", true).unwrap();
        assert_eq!(r.dry_run, Some(true));
        assert!(r.request.is_some());
    }
}
