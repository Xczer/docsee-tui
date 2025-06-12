use crossterm::event;
use serde::Deserialize;
use std::fmt;

#[cfg(test)]
use serde::Serialize;

/// a single keypress, normalized so the app doesn't deal with crossterm directly
#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
pub enum Key {
    /// covers both the main Enter and numpad Enter
    Enter,
    Tab,
    Backspace,
    Esc,

    Left,
    Right,
    Up,
    Down,

    Ins,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    F0,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Char(char),
    Ctrl(char),
    Alt(char),
    Unknown,
    Null,

    // app-specific actions, mapped from single chars below
    Quit,       // 'q'
    Cheatsheet, // 'c'
    DeleteItem, // 'D' - delete container/image/etc
    Start,      // 'u' - start container
    Stop,       // 'd' - stop container
    Restart,    // 'r' - restart container
    Logs,       // 'l' - view logs
    Exec,       // 'e' - shell into it
    Prune,      // 'p' - prune unused stuff
}

impl Key {
    /// n -> Fn (1 gives F1 and so on). panics if n is 0 or > 12
    pub fn from_f(n: u8) -> Key {
        match n {
            0 => Key::F0,
            1 => Key::F1,
            2 => Key::F2,
            3 => Key::F3,
            4 => Key::F4,
            5 => Key::F5,
            6 => Key::F6,
            7 => Key::F7,
            8 => Key::F8,
            9 => Key::F9,
            10 => Key::F10,
            11 => Key::F11,
            12 => Key::F12,
            _ => panic!("unknown function key: F{}", n),
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Key::Alt(' ') => write!(f, "<Alt+Space>"),
            Key::Ctrl(' ') => write!(f, "<Ctrl+Space>"),
            Key::Char(' ') => write!(f, "<Space>"),
            Key::Alt(c) => write!(f, "<Alt+{}>", c),
            Key::Ctrl(c) => write!(f, "<Ctrl+{}>", c),
            Key::Char(c) => write!(f, "{}", c),
            Key::Left => write!(f, "\u{2190}"),  //←
            Key::Right => write!(f, "\u{2192}"), //→
            Key::Up => write!(f, "\u{2191}"),    //↑
            Key::Down => write!(f, "\u{2193}"),  //↓
            Key::Enter
            | Key::Tab
            | Key::Backspace
            | Key::Esc
            | Key::Ins
            | Key::Delete
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown => write!(f, "<{:?}>", self),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl From<event::KeyEvent> for Key {
    fn from(key_event: event::KeyEvent) -> Self {
        match key_event {
            event::KeyEvent {
                code: event::KeyCode::Esc,
                ..
            } => Key::Esc,
            event::KeyEvent {
                code: event::KeyCode::Backspace,
                ..
            } => Key::Backspace,
            event::KeyEvent {
                code: event::KeyCode::Left,
                ..
            } => Key::Left,
            event::KeyEvent {
                code: event::KeyCode::Right,
                ..
            } => Key::Right,
            event::KeyEvent {
                code: event::KeyCode::Up,
                ..
            } => Key::Up,
            event::KeyEvent {
                code: event::KeyCode::Down,
                ..
            } => Key::Down,
            event::KeyEvent {
                code: event::KeyCode::Home,
                ..
            } => Key::Home,
            event::KeyEvent {
                code: event::KeyCode::End,
                ..
            } => Key::End,
            event::KeyEvent {
                code: event::KeyCode::PageUp,
                ..
            } => Key::PageUp,
            event::KeyEvent {
                code: event::KeyCode::PageDown,
                ..
            } => Key::PageDown,
            event::KeyEvent {
                code: event::KeyCode::Delete,
                ..
            } => Key::Delete,
            event::KeyEvent {
                code: event::KeyCode::Insert,
                ..
            } => Key::Ins,
            event::KeyEvent {
                code: event::KeyCode::F(n),
                ..
            } => Key::from_f(n),
            event::KeyEvent {
                code: event::KeyCode::Enter,
                ..
            } => Key::Enter,
            event::KeyEvent {
                code: event::KeyCode::Tab,
                ..
            } => Key::Tab,

            // the action keys - note 'd'/'D' differ by shift, careful here
            event::KeyEvent {
                code: event::KeyCode::Char('q'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Quit,
            event::KeyEvent {
                code: event::KeyCode::Char('c'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Cheatsheet,
            event::KeyEvent {
                code: event::KeyCode::Char('D'),
                modifiers: event::KeyModifiers::SHIFT,
                ..
            } => Key::DeleteItem,
            event::KeyEvent {
                code: event::KeyCode::Char('u'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Start,
            event::KeyEvent {
                code: event::KeyCode::Char('d'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Stop,
            event::KeyEvent {
                code: event::KeyCode::Char('r'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Restart,
            event::KeyEvent {
                code: event::KeyCode::Char('l'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Logs,
            event::KeyEvent {
                code: event::KeyCode::Char('e'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Exec,
            event::KeyEvent {
                code: event::KeyCode::Char('p'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Prune,

            // vim j/k mapped onto down/up
            event::KeyEvent {
                code: event::KeyCode::Char('j'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Down,
            event::KeyEvent {
                code: event::KeyCode::Char('k'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Up,

            // these stay as raw chars, each tab decides what to do with them
            event::KeyEvent {
                code: event::KeyCode::Char('s'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('s'), // stats

            event::KeyEvent {
                code: event::KeyCode::Char('i'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('i'), // interactive shell

            event::KeyEvent {
                code: event::KeyCode::Char('/'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('/'), // search

            event::KeyEvent {
                code: event::KeyCode::Char('f'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('f'), // filter / follow

            event::KeyEvent {
                code: event::KeyCode::Char('t'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('t'), // timestamps

            event::KeyEvent {
                code: event::KeyCode::Char('+'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('+'),

            event::KeyEvent {
                code: event::KeyCode::Char('-'),
                modifiers: event::KeyModifiers::NONE,
                ..
            } => Key::Char('-'),

            // match modifier combos before the plain-char catch-all below
            event::KeyEvent {
                code: event::KeyCode::Char(c),
                modifiers: event::KeyModifiers::ALT,
                ..
            } => Key::Alt(c),
            event::KeyEvent {
                code: event::KeyCode::Char(c),
                modifiers: event::KeyModifiers::CONTROL,
                ..
            } => Key::Ctrl(c),

            event::KeyEvent {
                code: event::KeyCode::Char(c),
                ..
            } => Key::Char(c),

            _ => Key::Unknown,
        }
    }
}
