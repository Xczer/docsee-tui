use crossterm::event::{self, Event};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::key::Key;

/// Events that can occur in our application
#[derive(Debug)]
pub enum AppEvent {
    /// A key was pressed
    Key(Key),
    /// A mouse event occurred
    Mouse(crossterm::event::MouseEvent),
    /// Application should tick (for periodic updates)
    Tick,
    /// Application should quit
    Quit,
}

/// Configuration for the event handler
pub struct EventConfig {
    /// How often to send tick events
    pub tick_rate: Duration,
    /// Timeout for reading events
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

/// Handles terminal events and converts them to application events
pub struct EventHandler {
    _config: EventConfig,
    receiver: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(config: EventConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        // Spawn a task to handle terminal events
        tokio::spawn(async move {
            let mut last_tick = Instant::now();

            loop {
                // Calculate how long to wait for events
                let timeout = config.timeout.saturating_sub(last_tick.elapsed());

                // Check if we should send a tick event
                if last_tick.elapsed() >= config.tick_rate {
                    if sender.send(AppEvent::Tick).is_err() {
                        break; // Channel closed
                    }
                    last_tick = Instant::now();
                }

                // Poll for terminal events
                if event::poll(timeout).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key_event)) => {
                            let key = Key::from(key_event);

                            // Special handling for quit key
                            if key == Key::Quit {
                                let _ = sender.send(AppEvent::Quit);
                                break;
                            }

                            if sender.send(AppEvent::Key(key)).is_err() {
                                break; // Channel closed
                            }
                        }
                        Ok(Event::Mouse(mouse_event)) => {
                            if sender.send(AppEvent::Mouse(mouse_event)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Resize(_, _)) => {
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

    /// Get the next event from the handler
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }
}

/*
EXPLANATION:
- EventHandler manages all input events from the terminal
- It runs in a separate async task to avoid blocking the main UI loop
- It converts terminal events (key presses, resizes) into our custom AppEvent enum
- The tick mechanism allows us to update the UI periodically (for live logs, status updates, etc.)
- It uses an mpsc channel to communicate events to the main application
- The handler automatically exits when 'q' is pressed or the channel is closed
- This design separates event handling from business logic, making the code cleaner
*/
