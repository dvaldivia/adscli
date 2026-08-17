//! k9s-style table browser for campaigns → asset groups → assets.

mod app;
mod event;
mod input;
mod theme;
mod view;

pub use app::{AdsBrowser, App, FixtureBrowser, Level, LiveBrowser, Row};
pub use event::AppEvent;

use std::io::{self, IsTerminal};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

const FRAME_BUDGET: Duration = Duration::from_millis(16);

pub fn start<B: AdsBrowser + Send + Sync + 'static>(browser: B) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "no terminal detected — the default command opens an interactive TUI and requires a TTY",
        ));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::sync_channel::<AppEvent>(1024);
    let mut app = App::new(browser);
    app.attach_sender(tx.clone());
    app.reload();

    let input_tx = tx.clone();
    let _input = thread::Builder::new()
        .name("adscli-tui-input".into())
        .spawn(move || input::input_loop(input_tx))?;

    let tick_tx = tx;
    let _tick = thread::Builder::new()
        .name("adscli-tui-tick".into())
        .spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(250));
                if tick_tx.send(AppEvent::Tick).is_err() {
                    return;
                }
            }
        })?;

    let mut next_frame = Instant::now();
    loop {
        if app.dirty && Instant::now() >= next_frame {
            terminal.draw(|f| view::render(f, &app))?;
            app.dirty = false;
            next_frame = Instant::now() + FRAME_BUDGET;
        }
        let wait = if app.dirty {
            FRAME_BUDGET
        } else {
            Duration::from_millis(250)
        };
        match rx.recv_timeout(wait) {
            Ok(ev) => {
                if !app.handle(ev) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    drop(terminal);
    Ok(())
}

/// Render into an off-screen buffer (tests).
pub fn render_to_lines<B: AdsBrowser>(app: &App<B>, width: u16, height: u16) -> Vec<String> {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut term = ratatui::Terminal::new(backend).expect("test terminal");
    term.draw(|f| view::render(f, app)).expect("draw");
    let buffer = term.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}
