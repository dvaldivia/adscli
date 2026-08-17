use ratatui::style::{Color, Modifier, Style};

pub fn header() -> Style {
    Style::default()
        .bg(Color::Rgb(0x1a, 0x1b, 0x26))
        .fg(Color::Rgb(0x7d, 0xcf, 0xff))
        .add_modifier(Modifier::BOLD)
}

pub fn header_dim() -> Style {
    Style::default()
        .bg(Color::Rgb(0x1a, 0x1b, 0x26))
        .fg(Color::Rgb(0x56, 0x5f, 0x89))
}

pub fn footer() -> Style {
    Style::default()
        .bg(Color::Rgb(0x1a, 0x1b, 0x26))
        .fg(Color::Rgb(0x9e, 0xce, 0x6a))
}

pub fn footer_key() -> Style {
    Style::default()
        .bg(Color::Rgb(0x1a, 0x1b, 0x26))
        .fg(Color::Rgb(0xe0, 0xaf, 0x68))
        .add_modifier(Modifier::BOLD)
}

pub fn table_header() -> Style {
    Style::default()
        .fg(Color::Rgb(0xe0, 0xaf, 0x68))
        .add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .bg(Color::Rgb(0x3d, 0x59, 0xa1))
        .fg(Color::Rgb(0xc0, 0xca, 0xf5))
        .add_modifier(Modifier::BOLD)
}

pub fn row() -> Style {
    Style::default().fg(Color::Rgb(0xa9, 0xb1, 0xd6))
}

pub fn enabled() -> Style {
    Style::default().fg(Color::Rgb(0x9e, 0xce, 0x6a))
}

pub fn paused() -> Style {
    Style::default().fg(Color::Rgb(0xe0, 0xaf, 0x68))
}

pub fn removed() -> Style {
    Style::default().fg(Color::Rgb(0xf7, 0x76, 0x8e))
}

pub fn border() -> Style {
    Style::default().fg(Color::Rgb(0x56, 0x5f, 0x89))
}

pub fn filter() -> Style {
    Style::default()
        .fg(Color::Rgb(0xbb, 0x9a, 0xf7))
        .add_modifier(Modifier::BOLD)
}

pub fn status() -> Style {
    Style::default()
        .fg(Color::Rgb(0x56, 0x5f, 0x89))
        .add_modifier(Modifier::ITALIC)
}

pub fn status_style(s: &str) -> Style {
    match s {
        "ENABLED" => enabled(),
        "PAUSED" => paused(),
        "REMOVED" => removed(),
        _ => row(),
    }
}
