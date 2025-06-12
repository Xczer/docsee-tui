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
                Constraint::Length(6), // Images commands
                Constraint::Length(5), // Volumes commands
                Constraint::Length(5), // Networks commands
                Constraint::Min(1),    // Bottom padding
            ])
            .split(inner_area);

        // Draw each section
        self.draw_global_commands(frame, sections[0]);
        self.draw_container_commands(frame, sections[1]);
        self.draw_images_commands(frame, sections[2]);
        self.draw_volumes_commands(frame, sections[3]);
        self.draw_networks_commands(frame, sections[4]);
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

    /// Draw images commands section
    fn draw_images_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(vec![
                Span::styled("Images Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("  Navigate images"),
            ]),
            Line::from(vec![
                Span::styled("D", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("   Delete image        "),
                Span::styled("p", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("   Prune unused images"),
            ]),
        ];

        let paragraph = Paragraph::new(content)
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    /// Draw volumes commands section
    fn draw_volumes_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(vec![
                Span::styled("Volumes Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("  Navigate volumes     "),
                Span::styled("D", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("   Delete volume"),
            ]),
            Line::from(vec![
                Span::styled("p", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("   Prune unused volumes"),
            ]),
        ];

        let paragraph = Paragraph::new(content)
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    /// Draw networks commands section
    fn draw_networks_commands(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(vec![
                Span::styled("Networks Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("  Navigate networks    "),
                Span::styled("D", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("   Delete network"),
            ]),
            Line::from(vec![
                Span::styled("p", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("   Prune unused networks"),
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
