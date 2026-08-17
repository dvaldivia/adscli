use adscli_tui::{App, FixtureBrowser, Level, Row, render_to_lines};

fn row(id: &str, name: &str) -> Row {
    Row {
        id: id.into(),
        name: name.into(),
        status: "ENABLED".into(),
        extra: "SEARCH".into(),
        impressions: "1".into(),
        clicks: "0".into(),
        cost: "0.00".into(),
        conversions: "0.00".into(),
        detail: String::new(),
    }
}

#[test]
fn renders_campaign_names() {
    let fx = FixtureBrowser {
        customer_id: "123".into(),
        date_label: "LAST_30_DAYS".into(),
        campaigns: vec![row("9", "Brand Hunt")],
        ..Default::default()
    };
    let mut app = App::new(fx);
    app.reload_sync();
    let lines = render_to_lines(&app, 100, 16);
    let blob = lines.join("\n");
    assert!(blob.contains("Brand Hunt"), "{blob}");
    assert!(blob.contains("CAMPAIGNS"), "{blob}");
    assert_eq!(app.level(), Level::Campaigns);
}
