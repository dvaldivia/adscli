use crossterm::event::KeyEvent;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    ReloadDone { error: Option<String> },
    MutateDone { error: Option<String> },
}
