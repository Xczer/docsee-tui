use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Cheatsheet modal that shows all available commands
#[derive(Default)]
pub struct CheatSheet;

impl CheatSheet {
    /// Create a new cheatsheet
    pub fn new() -> Self {
        Self
    }

    /// Draw the cheatsheet modal
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let modal_area = centered_rect(85, 85, area);

        frame.render_widget(Clear, modal_area);

        let block = Block::default()
            .title(" Docsee Cheatsheet ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        frame.render_widget(block, modal_area);

        let inner_area = modal_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),  // Global commands
                Constraint::Length(15), // Container commands
                Constraint::Length(8),  // Images commands
                Constraint::Length(5),  // Volumes/Networks
                Constraint::Length(4),  // System commands
                Constraint::Length(5),  // Logs commands
                Constraint::Min(1),    // Bottom padding
            ])
            .split(inner_area);

        self.draw_global_commands(frame, sections[0]);
        self.draw_container_commands(frame, sections[1]);
        self.draw_images_commands(frame, sections[2]);
        self.draw_resource_commands(frame, sections[3]);
        self.draw_system_commands(frame, sections[4]);
        self.draw_logs_commands(frame, sections[5]);
    }

    fn draw_global_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            section_title("Global Commands"),
            Line::from(vec![
                key("Left/Right", Color::Green), Span::raw(" Switch tabs  "),
                key("c", Color::Green), Span::raw(" Cheatsheet  "),
                key("q", Color::Red), Span::raw(" Quit  "),
                key("o", Color::Cyan), Span::raw(" Sort column  "),
                key("O", Color::Cyan), Span::raw(" Reverse sort"),
            ]),
            Line::from(vec![
                key("Up/Down", Color::Green), Span::raw(" Navigate  "),
                key("Mouse", Color::Cyan), Span::raw(" Click rows/tabs, scroll (if enabled in config)"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(content).style(Style::default().fg(Color::White)).wrap(Wrap { trim: true }),
            area,
        );
    }

    fn draw_container_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            section_title("Container Commands"),
            Line::from(""),
            Line::from(vec![
                key("u", Color::Green), Span::raw(" Start  "),
                key("d", Color::Yellow), Span::raw(" Stop  "),
                key("r", Color::Blue), Span::raw(" Restart  "),
                key("D", Color::Red), Span::raw(" Delete (stopped only)"),
            ]),
            Line::from(vec![
                key("l", Color::Magenta), Span::raw(" Logs  "),
                key("e", Color::Magenta), Span::raw(" Shell  "),
                key("s", Color::Green), Span::raw(" Stats  "),
                key("i", Color::Green), Span::raw(" Interactive shell"),
            ]),
            Line::from(vec![
                key("Enter", Color::Cyan), Span::raw(" Inspect  "),
                key("t", Color::Cyan), Span::raw(" Topology  "),
                key("g", Color::Yellow), Span::raw(" Compose grouping  "),
                key("/", Color::Cyan), Span::raw(" Search"),
            ]),
            Line::from(""),
            section_title("  Bulk Operations"),
            Line::from(vec![
                key("Space", Color::Green), Span::raw(" Toggle select  "),
                key("a", Color::Green), Span::raw(" Select all  "),
                key("A", Color::Yellow), Span::raw(" Deselect all"),
            ]),
            Line::from(vec![
                key("Shift+U", Color::Green), Span::raw(" Bulk start  "),
                key("Shift+S", Color::Yellow), Span::raw(" Bulk stop  "),
                key("Shift+X", Color::Red), Span::raw(" Bulk delete"),
            ]),
            Line::from(""),
            section_title("  Compose Operations"),
            Line::from(vec![
                key("Shift+C", Color::Green), Span::raw(" Compose up  "),
                key("Shift+W", Color::Yellow), Span::raw(" Compose down  "),
                Span::styled("(on grouped projects)", Style::default().fg(Color::DarkGray)),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(content).style(Style::default().fg(Color::White)).wrap(Wrap { trim: true }),
            area,
        );
    }

    fn draw_images_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            section_title("Images Commands"),
            Line::from(""),
            Line::from(vec![
                key("D", Color::Red), Span::raw(" Delete image  "),
                key("p", Color::Yellow), Span::raw(" Prune unused  "),
                key("Shift+P", Color::Cyan), Span::raw(" Pull image"),
            ]),
            Line::from(vec![
                key("R", Color::Green), Span::raw(" Run container from image"),
            ]),
            Line::from(""),
        ];
        frame.render_widget(
            Paragraph::new(content).style(Style::default().fg(Color::White)).wrap(Wrap { trim: true }),
            area,
        );
    }

    fn draw_resource_commands(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let volumes = vec![
            section_title("Volumes"),
            Line::from(vec![
                key("D", Color::Red), Span::raw(" Delete  "),
                key("p", Color::Yellow), Span::raw(" Prune"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(volumes).style(Style::default().fg(Color::White)),
            chunks[0],
        );

        let networks = vec![
            section_title("Networks"),
            Line::from(vec![
                key("D", Color::Red), Span::raw(" Delete  "),
                key("p", Color::Yellow), Span::raw(" Prune"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(networks).style(Style::default().fg(Color::White)),
            chunks[1],
        );
    }

    fn draw_system_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            section_title("System Dashboard"),
            Line::from(vec![
                key("Left/Right", Color::Green), Span::raw(" Switch views (Info/Disk/Events)  "),
                key("r", Color::Blue), Span::raw(" Refresh  "),
                key("Up/Down", Color::Green), Span::raw(" Scroll"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(content).style(Style::default().fg(Color::White)),
            area,
        );
    }

    fn draw_logs_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            section_title("Logs Viewer"),
            Line::from(vec![
                key("f", Color::Green), Span::raw(" Follow  "),
                key("t", Color::Green), Span::raw(" Timestamps  "),
                key("w", Color::Green), Span::raw(" Word wrap  "),
                key("n", Color::Green), Span::raw(" Line numbers"),
            ]),
            Line::from(vec![
                key("x", Color::Cyan), Span::raw(" Export as text  "),
                key("X", Color::Cyan), Span::raw(" Export as JSON  "),
                key("c", Color::Yellow), Span::raw(" Clear  "),
                key("+/-", Color::Green), Span::raw(" Speed"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(content).style(Style::default().fg(Color::White)),
            area,
        );
    }
}

fn section_title(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn key(label: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!(" {} ", label),
        Style::default()
            .fg(color)
            .add_modifier(Modifier::BOLD),
    )
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
