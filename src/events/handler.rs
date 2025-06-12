use crossterm::event::{self, Event};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::key::Key;

/// stuff coming out of the terminal loop
#[derive(Debug)]
pub enum AppEvent {
    Key(Key),
    Mouse(crossterm::event::MouseEvent),
    /// periodic wakeup so the ui can refresh
    Tick,
    Quit,
}

pub struct EventConfig {
    pub tick_rate: Duration,
    pub timeout: Duration,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            tick_rate: Duration::from_millis(250), // 4 FPS for UI updates
            timeout: Duration::from_millis(100),   // 100ms timeout for responsiveness
        }
    }
}

/// reads terminal input on a background task and pushes AppEvents over a channel
pub struct EventHandler {
    _config: EventConfig,
    receiver: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(config: EventConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut last_tick = Instant::now();

            loop {
                let timeout = config.timeout.saturating_sub(last_tick.elapsed());

                if last_tick.elapsed() >= config.tick_rate {
                    if sender.send(AppEvent::Tick).is_err() {
                        break; // receiver gone, nothing to do
                    }
                    last_tick = Instant::now();
                }

                if event::poll(timeout).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key_event)) => {
                            let key = Key::from(key_event);

                            // quit kills the loop, don't bother forwarding it
                            if key == Key::Quit {
                                let _ = sender.send(AppEvent::Quit);
                                break;
                            }

                            if sender.send(AppEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Mouse(mouse_event)) => {
                            if sender.send(AppEvent::Mouse(mouse_event)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Resize(_, _)) => {
                            // treat resize as a tick so we redraw
                            if sender.send(AppEvent::Tick).is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        Self {
            _config: config,
            receiver,
        }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }
}
