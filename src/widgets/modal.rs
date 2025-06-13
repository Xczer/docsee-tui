use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

/// how scary the confirm dialog looks (border colour + icon)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Normal,
    Warning,
    Danger,
}

impl Severity {
    pub fn border_color(&self) -> Color {
        match self {
            Severity::Normal => Color::Cyan,
            Severity::Warning => Color::Yellow,
            Severity::Danger => Color::Red,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Severity::Normal => "?",
            Severity::Warning => "!",
            Severity::Danger => "!!",
        }
    }
}

/// an action waiting on a yes/no from the user
#[derive(Debug, Clone)]
pub struct PendingAction {
    pub title: String,
    pub message: String,
    pub severity: Severity,
    pub action: ActionType,
    pub confirm_selected: bool,
}

#[derive(Debug, Clone)]
pub enum ActionType {
    DeleteContainer { id: String, name: String },
    StopContainer { id: String, name: String },
    DeleteImage { id: String, name: String },
    PruneImages,
    DeleteVolume { name: String },
    PruneVolumes,
    DeleteNetwork { id: String, name: String },
    PruneNetworks,
    BulkStart { ids: Vec<String> },
    BulkStop { ids: Vec<String> },
    BulkDelete { ids: Vec<String> },
}

impl PendingAction {
    pub fn new(title: String, message: String, severity: Severity, action: ActionType) -> Self {
        Self {
            title,
            message,
            severity,
            action,
            confirm_selected: false,
        }
    }

    pub fn toggle_selection(&mut self) {
        self.confirm_selected = !self.confirm_selected;
    }
}

pub struct ConfirmationModal;

impl ConfirmationModal {
    pub fn draw(frame: &mut Frame, area: Rect, pending: &PendingAction) {
        let modal_area = centered_rect(50, 30, area);

        // wipe whatever's under the modal first
        frame.render_widget(Clear, modal_area);

        let border_color = pending.severity.border_color();
        let icon = pending.severity.icon();

        let block = Block::default()
            .title(format!(" {} {} ", icon, pending.title))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(Color::Black));

        frame.render_widget(block, modal_area);

        let inner = modal_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(2),    // Message
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Buttons
                Constraint::Length(1), // Help
            ])
            .split(inner);

        let message = Paragraph::new(Line::from(Span::styled(
            &pending.message,
            Style::default().fg(Color::White),
        )));
        frame.render_widget(message, chunks[0]);

        let (confirm_style, cancel_style) = if pending.confirm_selected {
            (
                Style::default()
                    .fg(Color::Black)
                    .bg(border_color)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Gray),
            )
        } else {
            (
                Style::default().fg(Color::Gray),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
        };

        let buttons = Line::from(vec![
            Span::raw("    "),
            Span::styled(" Confirm ", confirm_style),
            Span::raw("     "),
            Span::styled(" Cancel ", cancel_style),
        ]);
        let buttons_paragraph =
            Paragraph::new(buttons).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(buttons_paragraph, chunks[2]);

        let help = Paragraph::new(Line::from(vec![
            Span::styled(
                "Left/Right",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" select  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(help, chunks[3]);
    }
}

// carve out a centered box, sized as a % of the parent area
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
