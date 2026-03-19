use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub accent: Color,
    pub border: Color,
    pub border_focused: Color,
    pub highlight_bg: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub muted: Color,
    pub title_1: Color,
    pub title_2: Color,
    pub title_3: Color,
}

impl Theme {
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "light" => Self::light(),
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            "gruvbox" => Self::gruvbox(),
            _ => Self::default_theme(),
        }
    }

    pub fn default_theme() -> Self {
        Self {
            fg: Color::White,
            bg: Color::Reset,
            accent: Color::Yellow,
            border: Color::DarkGray,
            border_focused: Color::Yellow,
            highlight_bg: Color::DarkGray,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Cyan,
            muted: Color::Gray,
            title_1: Color::Cyan,
            title_2: Color::Blue,
            title_3: Color::Magenta,
        }
    }

    pub fn light() -> Self {
        Self {
            fg: Color::Black,
            bg: Color::White,
            accent: Color::Blue,
            border: Color::Gray,
            border_focused: Color::Blue,
            highlight_bg: Color::LightBlue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Blue,
            muted: Color::Gray,
            title_1: Color::Blue,
            title_2: Color::Cyan,
            title_3: Color::Magenta,
        }
    }

    pub fn nord() -> Self {
        Self {
            fg: Color::Rgb(216, 222, 233),       // nord4
            bg: Color::Rgb(46, 52, 64),          // nord0
            accent: Color::Rgb(136, 192, 208),   // nord8
            border: Color::Rgb(76, 86, 106),     // nord3
            border_focused: Color::Rgb(136, 192, 208), // nord8
            highlight_bg: Color::Rgb(67, 76, 94), // nord2
            success: Color::Rgb(163, 190, 140),  // nord14
            warning: Color::Rgb(235, 203, 139),  // nord13
            error: Color::Rgb(191, 97, 106),     // nord11
            info: Color::Rgb(129, 161, 193),     // nord9
            muted: Color::Rgb(76, 86, 106),      // nord3
            title_1: Color::Rgb(136, 192, 208),  // nord8
            title_2: Color::Rgb(129, 161, 193),  // nord9
            title_3: Color::Rgb(180, 142, 173),  // nord15
        }
    }

    pub fn dracula() -> Self {
        Self {
            fg: Color::Rgb(248, 248, 242),       // foreground
            bg: Color::Rgb(40, 42, 54),          // background
            accent: Color::Rgb(189, 147, 249),   // purple
            border: Color::Rgb(68, 71, 90),      // current line
            border_focused: Color::Rgb(189, 147, 249), // purple
            highlight_bg: Color::Rgb(68, 71, 90), // current line
            success: Color::Rgb(80, 250, 123),   // green
            warning: Color::Rgb(241, 250, 140),  // yellow
            error: Color::Rgb(255, 85, 85),      // red
            info: Color::Rgb(139, 233, 253),     // cyan
            muted: Color::Rgb(98, 114, 164),     // comment
            title_1: Color::Rgb(139, 233, 253),  // cyan
            title_2: Color::Rgb(189, 147, 249),  // purple
            title_3: Color::Rgb(255, 121, 198),  // pink
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            fg: Color::Rgb(235, 219, 178),       // fg
            bg: Color::Rgb(40, 40, 40),          // bg
            accent: Color::Rgb(250, 189, 47),    // yellow
            border: Color::Rgb(80, 73, 69),      // bg2
            border_focused: Color::Rgb(250, 189, 47), // yellow
            highlight_bg: Color::Rgb(80, 73, 69), // bg2
            success: Color::Rgb(184, 187, 38),   // green
            warning: Color::Rgb(250, 189, 47),   // yellow
            error: Color::Rgb(251, 73, 52),      // red
            info: Color::Rgb(131, 165, 152),     // aqua
            muted: Color::Rgb(146, 131, 116),    // fg4
            title_1: Color::Rgb(131, 165, 152),  // aqua
            title_2: Color::Rgb(69, 133, 136),   // blue
            title_3: Color::Rgb(211, 134, 155),  // purple
        }
    }

    pub fn available_themes() -> &'static [&'static str] {
        &["default", "light", "nord", "dracula", "gruvbox"]
    }
}
