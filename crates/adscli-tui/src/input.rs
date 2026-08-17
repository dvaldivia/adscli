use std::sync::mpsc::SyncSender;
use std::time::Duration;

use crossterm::event::{Event as CtEvent, KeyEventKind};

use crate::event::AppEvent;

pub fn input_loop(tx: SyncSender<AppEvent>) {
    loop {
        match crossterm::event::poll(Duration::from_millis(100)) {
            Ok(false) => continue,
            Ok(true) => match crossterm::event::read() {
                Ok(CtEvent::Key(k)) if k.kind == KeyEventKind::Press => {
                    if tx.send(AppEvent::Key(k)).is_err() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(_) => return,
            },
            Err(_) => return,
        }
    }
}
