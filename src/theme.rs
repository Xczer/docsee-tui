use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub accent: Color,
    pub border: Color,
    pub border_focused: Color,
    pub highlight_bg: Color,
    pub highlight_alt: Color, // alternating row background
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub muted: Color,
    pub title_1: Color,
    pub title_2: Color,
    pub title_3: Color,
}

/// Returns a gauge color based on utilization ratio (green -> yellow -> red)
pub fn gauge_gradient_color(ratio: f64) -> Color {
    if ratio < 0.5 {
        Color::Green
    } else if ratio < 0.75 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Spinner frames for loading animations
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Format a timestamp string as relative time (e.g., "2h ago", "3d ago")
pub fn relative_time(datetime_str: &str) -> String {
    use chrono::{Local, NaiveDateTime};
    if let Ok(dt) = NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%d %H:%M:%S") {
        let now = Local::now().naive_local();
        let duration = now.signed_duration_since(dt);
        let secs = duration.num_seconds();
        if secs < 0 {
            return datetime_str.to_string();
        }
        if secs < 60 {
            return format!("{}s ago", secs);
        }
        let mins = duration.num_minutes();
        if mins < 60 {
            return format!("{}m ago", mins);
        }
        let hours = duration.num_hours();
        if hours < 24 {
            return format!("{}h ago", hours);
        }
        let days = duration.num_days();
        if days < 30 {
            return format!("{}d ago", days);
        }
        let months = days / 30;
        if months < 12 {
            return format!("{}mo ago", months);
        }
        let years = days / 365;
        format!("{}y ago", years)
    } else {
        datetime_str.to_string()
    }
}

/// Truncate a string with ellipsis if it exceeds max_len
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.len() <= max_len || max_len < 4 {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
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
            highlight_alt: Color::Rgb(30, 30, 30),
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
            highlight_alt: Color::Rgb(240, 240, 240),
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
            fg: Color::Rgb(216, 222, 233),             // nord4
            bg: Color::Rgb(46, 52, 64),                // nord0
            accent: Color::Rgb(136, 192, 208),         // nord8
            border: Color::Rgb(76, 86, 106),           // nord3
            border_focused: Color::Rgb(136, 192, 208), // nord8
            highlight_bg: Color::Rgb(67, 76, 94),      // nord2
            highlight_alt: Color::Rgb(55, 62, 78),     // between nord0 and nord1
            success: Color::Rgb(163, 190, 140),        // nord14
            warning: Color::Rgb(235, 203, 139),        // nord13
            error: Color::Rgb(191, 97, 106),           // nord11
            info: Color::Rgb(129, 161, 193),           // nord9
            muted: Color::Rgb(76, 86, 106),            // nord3
            title_1: Color::Rgb(136, 192, 208),        // nord8
            title_2: Color::Rgb(129, 161, 193),        // nord9
            title_3: Color::Rgb(180, 142, 173),        // nord15
        }
    }

    pub fn dracula() -> Self {
        Self {
            fg: Color::Rgb(248, 248, 242),             // foreground
            bg: Color::Rgb(40, 42, 54),                // background
            accent: Color::Rgb(189, 147, 249),         // purple
            border: Color::Rgb(68, 71, 90),            // current line
            border_focused: Color::Rgb(189, 147, 249), // purple
            highlight_bg: Color::Rgb(68, 71, 90),      // current line
            highlight_alt: Color::Rgb(50, 52, 66),     // slightly lighter bg
            success: Color::Rgb(80, 250, 123),         // green
            warning: Color::Rgb(241, 250, 140),        // yellow
            error: Color::Rgb(255, 85, 85),            // red
            info: Color::Rgb(139, 233, 253),           // cyan
            muted: Color::Rgb(98, 114, 164),           // comment
            title_1: Color::Rgb(139, 233, 253),        // cyan
            title_2: Color::Rgb(189, 147, 249),        // purple
            title_3: Color::Rgb(255, 121, 198),        // pink
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            fg: Color::Rgb(235, 219, 178),            // fg
            bg: Color::Rgb(40, 40, 40),               // bg
            accent: Color::Rgb(250, 189, 47),         // yellow
            border: Color::Rgb(80, 73, 69),           // bg2
            border_focused: Color::Rgb(250, 189, 47), // yellow
            highlight_bg: Color::Rgb(80, 73, 69),     // bg2
            highlight_alt: Color::Rgb(50, 48, 47),    // between bg and bg1
            success: Color::Rgb(184, 187, 38),        // green
            warning: Color::Rgb(250, 189, 47),        // yellow
            error: Color::Rgb(251, 73, 52),           // red
            info: Color::Rgb(131, 165, 152),          // aqua
            muted: Color::Rgb(146, 131, 116),         // fg4
            title_1: Color::Rgb(131, 165, 152),       // aqua
            title_2: Color::Rgb(69, 133, 136),        // blue
            title_3: Color::Rgb(211, 134, 155),       // purple
        }
    }

    pub fn available_themes() -> &'static [&'static str] {
        &["default", "light", "nord", "dracula", "gruvbox"]
    }
}
