use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use adscli_api::query::{DateRange, ListFilter};
use adscli_api::{AdsClient, Asset, AssetGroup, Campaign, Transport};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::event::AppEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Campaigns,
    AssetGroups,
    Assets,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub status: String,
    pub extra: String,
    pub impressions: String,
    pub clicks: String,
    pub cost: String,
    pub conversions: String,
    pub detail: String,
}

pub trait AdsBrowser: Send + Sync + 'static {
    fn customer_id(&self) -> String;
    fn date_label(&self) -> String;
    fn list_campaigns(&self) -> Result<Vec<Row>, String>;
    fn list_asset_groups(&self, campaign_id: &str) -> Result<Vec<Row>, String>;
    fn list_assets(&self, asset_group_id: &str) -> Result<Vec<Row>, String>;
    fn set_status(&self, level: Level, id: &str, status: &str) -> Result<(), String>;
}

pub struct LiveBrowser<T: Transport> {
    pub client: AdsClient<T>,
    pub customer_id: String,
    pub date_range: DateRange,
}

impl<T: Transport + 'static> AdsBrowser for LiveBrowser<T> {
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn date_label(&self) -> String {
        self.date_range.label()
    }
    fn list_campaigns(&self) -> Result<Vec<Row>, String> {
        let filter = ListFilter {
            with_metrics: true,
            date_range: Some(self.date_range.clone()),
            ..Default::default()
        };
        self.client
            .list_campaigns(&self.customer_id, &filter)
            .map(|v| v.into_iter().map(campaign_row).collect())
            .map_err(|e| e.to_string())
    }
    fn list_asset_groups(&self, campaign_id: &str) -> Result<Vec<Row>, String> {
        let filter = ListFilter {
            with_metrics: true,
            date_range: Some(self.date_range.clone()),
            campaign_id: Some(campaign_id.to_string()),
            ..Default::default()
        };
        self.client
            .list_asset_groups(&self.customer_id, &filter)
            .map(|v| v.into_iter().map(asset_group_row).collect())
            .map_err(|e| e.to_string())
    }
    fn list_assets(&self, asset_group_id: &str) -> Result<Vec<Row>, String> {
        let filter = ListFilter {
            with_metrics: true,
            date_range: Some(self.date_range.clone()),
            asset_group_id: Some(asset_group_id.to_string()),
            ..Default::default()
        };
        self.client
            .list_asset_links(&self.customer_id, &filter)
            .map(|v| {
                v.into_iter()
                    .map(|l| {
                        let a = l.asset.clone().unwrap_or_default();
                        asset_link_row(
                            &l.asset_id,
                            &l.field_type,
                            &l.status,
                            &a,
                            l.metrics.as_ref(),
                        )
                    })
                    .collect()
            })
            .map_err(|e| e.to_string())
    }
    fn set_status(&self, level: Level, id: &str, status: &str) -> Result<(), String> {
        match level {
            Level::Campaigns => self
                .client
                .set_campaign_status(&self.customer_id, id, status, false)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Level::AssetGroups => self
                .client
                .set_asset_group_status(&self.customer_id, id, status, false)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Level::Assets => {
                Err("asset status is owned by the asset-group link; use the CLI".into())
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct FixtureBrowser {
    pub customer_id: String,
    pub date_label: String,
    pub campaigns: Vec<Row>,
    pub asset_groups: Vec<(String, Row)>,
    pub assets: Vec<(String, Row)>,
    pub last_status: Arc<Mutex<Option<(String, String, String)>>>,
}

impl AdsBrowser for FixtureBrowser {
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn date_label(&self) -> String {
        self.date_label.clone()
    }
    fn list_campaigns(&self) -> Result<Vec<Row>, String> {
        Ok(self.campaigns.clone())
    }
    fn list_asset_groups(&self, campaign_id: &str) -> Result<Vec<Row>, String> {
        Ok(self
            .asset_groups
            .iter()
            .filter(|(c, _)| c == campaign_id)
            .map(|(_, r)| r.clone())
            .collect())
    }
    fn list_assets(&self, asset_group_id: &str) -> Result<Vec<Row>, String> {
        Ok(self
            .assets
            .iter()
            .filter(|(g, _)| g == asset_group_id)
            .map(|(_, r)| r.clone())
            .collect())
    }
    fn set_status(&self, level: Level, id: &str, status: &str) -> Result<(), String> {
        *self.last_status.lock().unwrap() =
            Some((format!("{level:?}"), id.to_string(), status.to_string()));
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Crumb {
    level: Level,
    parent_id: Option<String>,
    parent_name: Option<String>,
}

pub struct App<B: AdsBrowser> {
    pub browser: Arc<B>,
    pub stack: Vec<Crumb>,
    pub rows: Vec<Row>,
    pub selected: usize,
    pub filter: String,
    pub filter_mode: bool,
    pub help: bool,
    pub describe: bool,
    pub status: String,
    pub loading: bool,
    pub dirty: bool,
    tx: Option<SyncSender<AppEvent>>,
}

impl<B: AdsBrowser + 'static> App<B> {
    pub fn new(browser: B) -> Self {
        Self {
            browser: Arc::new(browser),
            stack: vec![Crumb {
                level: Level::Campaigns,
                parent_id: None,
                parent_name: None,
            }],
            rows: Vec::new(),
            selected: 0,
            filter: String::new(),
            filter_mode: false,
            help: false,
            describe: false,
            status: String::new(),
            loading: false,
            dirty: true,
            tx: None,
        }
    }

    pub fn attach_sender(&mut self, tx: SyncSender<AppEvent>) {
        self.tx = Some(tx);
    }

    pub fn level(&self) -> Level {
        self.stack
            .last()
            .map(|c| c.level)
            .unwrap_or(Level::Campaigns)
    }

    pub fn breadcrumb(&self) -> String {
        let mut parts = vec!["CAMPAIGNS".to_string()];
        for c in &self.stack {
            if let Some(n) = &c.parent_name {
                parts.push(n.clone());
            }
        }
        match self.level() {
            Level::Campaigns => {}
            Level::AssetGroups => parts.push("ASSET GROUPS".into()),
            Level::Assets => parts.push("ASSETS".into()),
        }
        parts.join(" > ")
    }

    pub fn visible(&self) -> Vec<&Row> {
        if self.filter.is_empty() {
            return self.rows.iter().collect();
        }
        let q = self.filter.to_ascii_lowercase();
        self.rows
            .iter()
            .filter(|r| {
                r.name.to_ascii_lowercase().contains(&q)
                    || r.id.contains(&q)
                    || r.status.to_ascii_lowercase().contains(&q)
                    || r.extra.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn selected_row(&self) -> Option<&Row> {
        self.visible().get(self.selected).copied()
    }

    pub fn reload(&mut self) {
        self.reload_sync();
    }

    pub fn reload_sync(&mut self) {
        let level = self.level();
        let parent = self.stack.last().and_then(|c| c.parent_id.clone());
        let res = match (level, parent.as_deref()) {
            (Level::Campaigns, _) => self.browser.list_campaigns(),
            (Level::AssetGroups, Some(id)) => self.browser.list_asset_groups(id),
            (Level::Assets, Some(id)) => self.browser.list_assets(id),
            _ => Ok(Vec::new()),
        };
        match res {
            Ok(rows) => {
                self.rows = rows;
                if self.selected >= self.visible().len() && self.selected > 0 {
                    self.selected = self.visible().len().saturating_sub(1);
                }
                self.status = format!("{} rows", self.visible().len());
            }
            Err(e) => {
                self.rows.clear();
                self.status = format!("error: {e}");
            }
        }
        self.loading = false;
        self.dirty = true;
    }

    pub fn handle(&mut self, ev: AppEvent) -> bool {
        match ev {
            AppEvent::Tick => {
                self.dirty = true;
                true
            }
            AppEvent::ReloadDone { error } => {
                if let Some(e) = error {
                    self.status = format!("error: {e}");
                }
                self.loading = false;
                self.dirty = true;
                true
            }
            AppEvent::MutateDone { error } => {
                if let Some(e) = error {
                    self.status = format!("mutate failed: {e}");
                } else {
                    self.status = "updated".into();
                    self.reload_sync();
                }
                self.dirty = true;
                true
            }
            AppEvent::Key(k) => self.handle_key(k),
        }
    }

    fn handle_key(&mut self, k: KeyEvent) -> bool {
        if self.help {
            if matches!(
                k.code,
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')
            ) {
                self.help = false;
                self.dirty = true;
            }
            return true;
        }
        if self.describe {
            if matches!(
                k.code,
                KeyCode::Esc | KeyCode::Char('d') | KeyCode::Char('q')
            ) {
                self.describe = false;
                self.dirty = true;
            }
            return true;
        }
        if self.filter_mode {
            match k.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    self.filter.clear();
                    self.selected = 0;
                }
                KeyCode::Enter => self.filter_mode = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.selected = 0;
                }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.filter.push(c);
                    self.selected = 0;
                }
                _ => {}
            }
            self.dirty = true;
            return true;
        }

        match k.code {
            KeyCode::Char('q') | KeyCode::Esc if self.stack.len() == 1 => return false,
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => return false,
            KeyCode::Char('q') | KeyCode::Esc => {
                self.stack.pop();
                self.selected = 0;
                self.filter.clear();
                self.reload_sync();
            }
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.filter.clear();
            }
            KeyCode::Char('d') => self.describe = true,
            KeyCode::Char('r') => self.reload_sync(),
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.visible().len();
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Home | KeyCode::Char('g') => self.selected = 0,
            KeyCode::End | KeyCode::Char('G') => {
                let n = self.visible().len();
                if n > 0 {
                    self.selected = n - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.drill(),
            KeyCode::Backspace | KeyCode::Char('h') | KeyCode::Left => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                    self.selected = 0;
                    self.filter.clear();
                    self.reload_sync();
                }
            }
            KeyCode::Char('e') => self.mutate_status("ENABLED"),
            KeyCode::Char('p') => self.mutate_status("PAUSED"),
            _ => {}
        }
        self.dirty = true;
        true
    }

    fn drill(&mut self) {
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        let next = match self.level() {
            Level::Campaigns => Level::AssetGroups,
            Level::AssetGroups => Level::Assets,
            Level::Assets => {
                self.describe = true;
                return;
            }
        };
        self.stack.push(Crumb {
            level: next,
            parent_id: Some(row.id),
            parent_name: Some(row.name),
        });
        self.selected = 0;
        self.filter.clear();
        self.reload_sync();
    }

    fn mutate_status(&mut self, status: &str) {
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        let level = self.level();
        match self.browser.set_status(level, &row.id, status) {
            Ok(()) => {
                self.status = format!("{} → {status}", row.name);
                self.reload_sync();
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }
}

fn campaign_row(c: Campaign) -> Row {
    let m = c.metrics.unwrap_or_default();
    Row {
        id: c.id,
        name: c.name,
        status: c.status,
        extra: c.channel_type.unwrap_or_default(),
        impressions: fmt_i(m.impressions),
        clicks: fmt_i(m.clicks),
        cost: fmt_micros(m.cost_micros),
        conversions: fmt_f(m.conversions),
        detail: format!("budget_micros={}", c.budget_micros.unwrap_or(0)),
    }
}

fn asset_group_row(g: AssetGroup) -> Row {
    let m = g.metrics.unwrap_or_default();
    Row {
        id: g.id,
        name: g.name,
        status: g.status,
        extra: g.ad_strength.unwrap_or_default(),
        impressions: fmt_i(m.impressions),
        clicks: fmt_i(m.clicks),
        cost: fmt_micros(m.cost_micros),
        conversions: fmt_f(m.conversions),
        detail: g.final_urls.join(","),
    }
}

fn asset_link_row(
    id: &str,
    field_type: &str,
    status: &str,
    a: &Asset,
    metrics: Option<&adscli_api::Metrics>,
) -> Row {
    let m = metrics.cloned().unwrap_or_default();
    Row {
        id: id.to_string(),
        name: a
            .name
            .clone()
            .or_else(|| a.text.clone())
            .unwrap_or_else(|| a.resource_name.clone()),
        status: status.to_string(),
        extra: field_type.to_string(),
        impressions: fmt_i(m.impressions),
        clicks: fmt_i(m.clicks),
        cost: fmt_micros(m.cost_micros),
        conversions: fmt_f(m.conversions),
        detail: a.r#type.clone().unwrap_or_default(),
    }
}

fn fmt_i(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
}

fn fmt_f(v: Option<f64>) -> String {
    v.map(|n| format!("{n:.2}")).unwrap_or_else(|| "-".into())
}

fn fmt_micros(v: Option<i64>) -> String {
    v.map(|n| format!("{:.2}", n as f64 / 1_000_000.0))
        .unwrap_or_else(|| "-".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key(c: char) -> AppEvent {
        AppEvent::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        })
    }

    fn enter() -> AppEvent {
        AppEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        })
    }

    fn fixture() -> FixtureBrowser {
        FixtureBrowser {
            customer_id: "123".into(),
            date_label: "LAST_30_DAYS".into(),
            campaigns: vec![Row {
                id: "9".into(),
                name: "Brand".into(),
                status: "ENABLED".into(),
                extra: "SEARCH".into(),
                impressions: "10".into(),
                clicks: "1".into(),
                cost: "0.50".into(),
                conversions: "0.00".into(),
                detail: String::new(),
            }],
            asset_groups: vec![(
                "9".into(),
                Row {
                    id: "20".into(),
                    name: "Homepage".into(),
                    status: "ENABLED".into(),
                    extra: "EXCELLENT".into(),
                    impressions: "10".into(),
                    clicks: "1".into(),
                    cost: "0.50".into(),
                    conversions: "0.00".into(),
                    detail: String::new(),
                },
            )],
            assets: vec![],
            last_status: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn drills_into_asset_groups() {
        let mut app = App::new(fixture());
        app.reload_sync();
        assert_eq!(app.rows.len(), 1);
        app.handle(enter());
        assert_eq!(app.level(), Level::AssetGroups);
        assert_eq!(app.rows[0].name, "Homepage");
    }

    #[test]
    fn filter_narrows_rows() {
        let mut app = App::new(fixture());
        app.reload_sync();
        app.handle(key('/'));
        app.handle(key('z'));
        assert!(app.visible().is_empty());
        app.handle(AppEvent::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }));
        assert_eq!(app.visible().len(), 1);
    }

    #[test]
    fn pause_records_status() {
        let fx = fixture();
        let flag = Arc::clone(&fx.last_status);
        let mut app = App::new(fx);
        app.reload_sync();
        app.handle(key('p'));
        let got = flag.lock().unwrap().clone().unwrap();
        assert_eq!(got.1, "9");
        assert_eq!(got.2, "PAUSED");
    }
}
