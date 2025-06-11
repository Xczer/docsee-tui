use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Cheatsheet modal that shows all available commands
pub struct CheatSheet;

impl CheatSheet {
    /// Create a new cheatsheet
    pub fn new() -> Self {
        Self
    }

    /// Draw the cheatsheet modal
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        // Calculate centered position for modal
        let modal_area = centered_rect(80, 70, area);

        // Clear the background
        frame.render_widget(Clear, modal_area);

        // Create the main block
        let block = Block::default()
            .title("📋 Docsee Cheatsheet")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        frame.render_widget(block, modal_area);

        // Create inner area for content
        let inner_area = modal_area.inner(Margin { horizontal: 2, vertical: 1 });

        // Split into sections
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Global commands
                Constraint::Length(8), // Container commands
                Constraint::Length(6), // Images commands (placeholder)
                Constraint::Length(6), // Volumes commands (placeholder)
                Constraint::Length(6), // Networks commands (placeholder)
                Constraint::Min(1),    // Bottom padding
            ])
            .split(inner_area);

        // Draw each section
        self.draw_global_commands(frame, sections[0]);
        self.draw_container_commands(frame, sections[1]);
        self.draw_placeholder_commands(frame, sections[2], "Images");
        self.draw_placeholder_commands(frame, sections[3], "Volumes");
        self.draw_placeholder_commands(frame, sections[4], "Networks");
    }

    /// Draw global commands section
    fn draw_global_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(vec![
                Span::styled("Global Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("←/→", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("  Switch tabs  "),
                Span::styled("c", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("  Show this cheatsheet  "),
                Span::styled("q", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("  Quit application"),
            ]),
        ];

        let paragraph = Paragraph::new(content)
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    /// Draw container commands section
    fn draw_container_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(vec![
                Span::styled("Container Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("  Navigate containers"),
            ]),
            Line::from(vec![
                Span::styled("u", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("   Start container     "),
                Span::styled("d", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("   Stop container"),
            ]),
            Line::from(vec![
                Span::styled("r", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::raw("   Restart container   "),
                Span::styled("D", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("   Delete container (if stopped)"),
            ]),
            Line::from(vec![
                Span::styled("l", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw("   View logs           "),
                Span::styled("e", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw("   Execute shell"),
                Span::styled(" (coming soon)", Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
            ]),
        ];

        let paragraph = Paragraph::new(content)
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    /// Draw placeholder commands for other tabs
    fn draw_placeholder_commands(&self, frame: &mut Frame, area: Rect, tab_name: &str) {
        let content = vec![
            Line::from(vec![
                Span::styled(format!("{} Commands", tab_name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Coming soon!", Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)),
                Span::raw(" This tab will have commands for managing Docker "),
                Span::raw(tab_name.to_lowercase()),
            ]),
            Line::from(vec![
                Span::raw("Commands will include: list, create, delete, inspect, and more..."),
            ]),
        ];

        let paragraph = Paragraph::new(content)
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }
}

/// Helper function to create a centered rectangle
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

/*
EXPLANATION:
- CheatSheet is a modal widget that displays all available commands
- draw() creates a centered modal with a yellow border
- The Clear widget erases the background behind the modal
- centered_rect() calculates the position for a centered modal dialog
- Content is split into sections for different command categories:
  - Global commands: tab navigation, cheatsheet, quit
  - Container commands: all the container management shortcuts
  - Placeholder sections for other tabs (Images, Volumes, Networks)
- Each command is color-coded:
  - Green: navigation and safe operations
  - Yellow: stop operations
  - Red: destructive operations (delete, quit)
  - Blue: restart operations
  - Magenta: advanced operations (logs, exec)
  - Gray: coming soon features
- The modal uses proper styling with borders, colors, and formatting
- This provides users with a quick reference without leaving the application
*/
