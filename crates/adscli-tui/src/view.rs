use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row as TuiRow, Table, Wrap};

use crate::app::{AdsBrowser, App};
use crate::theme;

pub fn render<B: AdsBrowser>(f: &mut Frame, app: &App<B>) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, chunks[0], app);
    render_table(f, chunks[1], app);
    render_footer(f, chunks[2], app);

    if app.filter_mode || !app.filter.is_empty() {
        render_filter(f, chunks[1], app);
    }
    if app.help {
        render_help(f, area);
    }
    if app.describe {
        render_describe(f, area, app);
    }
}

fn render_header<B: AdsBrowser>(f: &mut Frame, area: Rect, app: &App<B>) {
    let cid = app.browser.customer_id();
    let range = app.browser.date_label();
    let line = Line::from(vec![
        Span::styled(" adscli ", theme::header()),
        Span::styled(format!(" {cid} "), theme::header_dim()),
        Span::styled(format!(" {} ", app.breadcrumb()), theme::header()),
        Span::styled(format!(" {range} "), theme::header_dim()),
        Span::styled(format!(" {} ", app.status), theme::status()),
    ]);
    f.render_widget(Paragraph::new(line).style(theme::header()), area);
}

fn render_footer<B: AdsBrowser>(f: &mut Frame, area: Rect, _app: &App<B>) {
    let keys = [
        ("?:", "help"),
        ("/: ", "filter"),
        ("⏎:", "open"),
        ("esc:", "back"),
        ("e:", "enable"),
        ("p:", "pause"),
        ("d:", "describe"),
        ("r:", "reload"),
        ("q:", "quit"),
    ];
    let mut spans = Vec::new();
    for (k, v) in keys {
        spans.push(Span::styled(format!(" {k}"), theme::footer_key()));
        spans.push(Span::styled(v, theme::footer()));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::footer()),
        area,
    );
}

fn render_table<B: AdsBrowser>(f: &mut Frame, area: Rect, app: &App<B>) {
    let header = TuiRow::new([
        Cell::from("ID"),
        Cell::from("NAME"),
        Cell::from("STATUS"),
        Cell::from("TYPE"),
        Cell::from("IMPR"),
        Cell::from("CLICKS"),
        Cell::from("COST"),
        Cell::from("CONV"),
    ])
    .style(theme::table_header());

    let visible = app.visible();
    let rows = visible.iter().enumerate().map(|(i, r)| {
        let style = if i == app.selected {
            theme::selected()
        } else {
            theme::row()
        };
        let st = if i == app.selected {
            theme::selected()
        } else {
            theme::status_style(&r.status)
        };
        TuiRow::new([
            Cell::from(r.id.as_str()).style(style),
            Cell::from(r.name.as_str()).style(style),
            Cell::from(r.status.as_str()).style(st),
            Cell::from(r.extra.as_str()).style(style),
            Cell::from(r.impressions.as_str()).style(style),
            Cell::from(r.clicks.as_str()).style(style),
            Cell::from(r.cost.as_str()).style(style),
            Cell::from(r.conversions.as_str()).style(style),
        ])
    });

    let widths = [
        Constraint::Length(14),
        Constraint::Min(18),
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(8),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(match app.level() {
                    crate::Level::Campaigns => " Campaigns ",
                    crate::Level::AssetGroups => " Asset groups ",
                    crate::Level::Assets => " Assets ",
                }),
        )
        .column_spacing(1);
    f.render_widget(table, area);
}

fn render_filter<B: AdsBrowser>(f: &mut Frame, area: Rect, app: &App<B>) {
    if area.height < 3 {
        return;
    }
    let y = area.y + area.height - 1;
    let bar = Rect {
        x: area.x + 1,
        y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let prompt = if app.filter_mode { "/" } else { "filter: " };
    let text = format!("{prompt}{}", app.filter);
    f.render_widget(Paragraph::new(text).style(theme::filter()), bar);
}

fn render_help(f: &mut Frame, area: Rect) {
    let text = "\
adscli  —  k9s-style Google Ads browser

  j / ↓        move down
  k / ↑        move up
  g / G        first / last row
  Enter / l    drill into asset groups, then assets
  Esc / h / q  go back (q on the top level quits)
  /            filter visible rows
  e            enable selected campaign or asset group
  p            pause selected campaign or asset group
  d            describe selected row
  r            reload current view
  ?            toggle this help

Creates and structured edits stay on the CLI:
  adscli campaigns create --help
  adscli asset-groups create --help
  adscli assets create --help
";
    overlay(f, area, "Help", text);
}

fn render_describe<B: AdsBrowser>(f: &mut Frame, area: Rect, app: &App<B>) {
    let text = match app.selected_row() {
        Some(r) => format!(
            "id: {}\nname: {}\nstatus: {}\ntype: {}\nimpressions: {}\nclicks: {}\ncost: {}\nconversions: {}\n{}\n",
            r.id,
            r.name,
            r.status,
            r.extra,
            r.impressions,
            r.clicks,
            r.cost,
            r.conversions,
            r.detail
        ),
        None => "no row selected".into(),
    };
    overlay(f, area, "Describe", &text);
}

fn overlay(f: &mut Frame, area: Rect, title: &str, text: &str) {
    let w = area.width.saturating_mul(3) / 4;
    let h = area.height.saturating_mul(3) / 4;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rec = Rect {
        x,
        y,
        width: w.max(20),
        height: h.max(8),
    };
    f.render_widget(Clear, rec);
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .style(Style::default().add_modifier(Modifier::empty()))
            .block(
                Block::default()
                    .title(format!(" {title} "))
                    .borders(Borders::ALL)
                    .border_style(theme::border()),
            ),
        rec,
    );
}
