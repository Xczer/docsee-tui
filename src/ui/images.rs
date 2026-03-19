use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};
use tokio::sync::mpsc;

use crate::{
    docker::{images::Image, DockerClient},
    events::Key,
    theme::Theme,
    widgets::modal::{ActionType, ConfirmationModal, PendingAction, Severity},
};

use super::containers::{SortDirection, SortState};

/// Pull mode state for image pulling
pub enum PullMode {
    Inactive,
    Input { text: String },
    Pulling { name: String, status: String },
}

/// Run mode state for container creation
pub enum RunMode {
    Inactive,
    Input { text: String, image: String },
}

/// The images tab widget
pub struct ImagesTab {
    docker_client: DockerClient,
    images: Vec<Image>,
    table_state: TableState,
    status_message: Option<String>,
    pending_action: Option<PendingAction>,
    theme: Theme,
    sort_state: SortState,
    pull_mode: PullMode,
    pull_receiver: Option<mpsc::UnboundedReceiver<String>>,
    run_mode: RunMode,
}

impl ImagesTab {
    /// Create a new images tab
    pub async fn new(docker_client: DockerClient, theme: Theme) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            images: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
            pending_action: None,
            theme,
            sort_state: SortState {
                column_index: 0,
                direction: SortDirection::Ascending,
            },
            pull_mode: PullMode::Inactive,
            pull_receiver: None,
            run_mode: RunMode::Inactive,
        };

        // Load initial data
        tab.refresh().await?;

        // Select first image if any exist
        if !tab.images.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Refresh image data from Docker
    pub async fn refresh(&mut self) -> Result<()> {
        // Check pull progress from background task
        if let Some(ref mut receiver) = self.pull_receiver {
            let mut last_msg = None;
            while let Ok(msg) = receiver.try_recv() {
                last_msg = Some(msg);
            }
            if let Some(msg) = last_msg {
                if msg.starts_with("DONE:") || msg.starts_with("ERROR:") {
                    let display_msg = msg.replacen("DONE:", "", 1).replacen("ERROR:", "", 1);
                    self.status_message = Some(display_msg);
                    self.pull_mode = PullMode::Inactive;
                    self.pull_receiver = None;
                    // Fall through to refresh image list
                } else {
                    if let PullMode::Pulling { ref mut status, .. } = self.pull_mode {
                        *status = msg;
                    }
                    return Ok(());
                }
            }
        }

        match self.docker_client.list_images().await {
            Ok(images) => {
                let selected_id = self.get_selected_image().map(|i| i.id.clone());
                self.images = images;

                if let Some(id) = selected_id {
                    let new_index = self.images.iter().position(|i| i.id == id);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.images.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading images: {}", e));
            }
        }
        Ok(())
    }

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        // Handle run mode input
        if let RunMode::Input { ref mut text, ref image } = self.run_mode {
            match key {
                Key::Esc => {
                    self.run_mode = RunMode::Inactive;
                }
                Key::Enter => {
                    let container_name = if text.is_empty() { None } else { Some(text.clone()) };
                    let img = image.clone();
                    self.run_mode = RunMode::Inactive;
                    self.run_container(&img, container_name).await?;
                }
                Key::Backspace => {
                    text.pop();
                }
                Key::Char(c) => {
                    text.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle pull mode input
        if let PullMode::Input { ref mut text } = self.pull_mode {
            match key {
                Key::Esc => {
                    self.pull_mode = PullMode::Inactive;
                }
                Key::Enter => {
                    let image_name = text.clone();
                    if !image_name.is_empty() {
                        self.pull_mode = PullMode::Pulling {
                            name: image_name.clone(),
                            status: "Starting pull...".to_string(),
                        };
                        self.start_pull_async(image_name);
                    }
                }
                Key::Backspace => {
                    text.pop();
                }
                Key::Char(c) => {
                    text.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle pulling state
        if let PullMode::Pulling { .. } = self.pull_mode {
            if key == Key::Esc {
                self.pull_mode = PullMode::Inactive;
                self.pull_receiver = None;
            }
            return Ok(());
        }

        // Handle confirmation modal first
        if self.pending_action.is_some() {
            return self.handle_confirmation_key(key).await;
        }

        match key {
            Key::Up => self.move_selection_up(),
            Key::Down => self.move_selection_down(),
            Key::DeleteItem => self.confirm_delete_image(),
            Key::Char('o') => self.cycle_sort_column(),
            Key::Char('O') => self.reverse_sort_direction(),
            Key::Char('P') => {
                self.pull_mode = PullMode::Input { text: String::new() };
            }
            Key::Char('R') => {
                // Run container from selected image
                if let Some(image) = self.get_selected_image() {
                    let img_name = if image.repository != "<none>" {
                        format!("{}:{}", image.repository, image.tag)
                    } else {
                        image.id.clone()
                    };
                    self.run_mode = RunMode::Input {
                        text: String::new(),
                        image: img_name,
                    };
                } else {
                    self.status_message = Some("No image selected".to_string());
                }
            }
            Key::Logs => {
                self.status_message =
                    Some("Images don't have logs. Try containers instead!".to_string());
            }
            Key::Exec => {
                self.status_message =
                    Some("Can't execute into images. Run as container first!".to_string());
            }
            Key::Start => {
                self.status_message =
                    Some("Can't start images. Use R to run a container!".to_string());
            }
            Key::Stop => {
                self.status_message =
                    Some("Images are not running. Try containers instead!".to_string());
            }
            Key::Restart => {
                self.status_message =
                    Some("Images are not running. Try containers instead!".to_string());
            }
            Key::Prune => {
                self.pending_action = Some(PendingAction::new(
                    "Prune Images".to_string(),
                    "Remove all unused images? This will free disk space.".to_string(),
                    Severity::Warning,
                    ActionType::PruneImages,
                ));
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle confirmation modal keys
    async fn handle_confirmation_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Left | Key::Right => {
                if let Some(ref mut pending) = self.pending_action {
                    pending.toggle_selection();
                }
            }
            Key::Enter => {
                if let Some(pending) = self.pending_action.take() {
                    if pending.confirm_selected {
                        match pending.action {
                            ActionType::DeleteImage { id, name } => {
                                match self.docker_client.remove_image(&id, false).await {
                                    Ok(_) => {
                                        self.status_message =
                                            Some(format!("Deleted image '{}'", name));
                                        self.refresh().await?;
                                    }
                                    Err(e) => {
                                        self.status_message = Some(format!(
                                            "Failed to delete '{}': {}",
                                            name, e
                                        ));
                                    }
                                }
                            }
                            ActionType::PruneImages => {
                                self.prune_images().await?;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Key::Esc => {
                self.pending_action = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Confirm delete image
    fn confirm_delete_image(&mut self) {
        if let Some(image) = self.get_selected_image() {
            let id = image.id.clone();
            let name = if image.repository == "<none>" {
                image.id.to_string()
            } else {
                format!("{}:{}", image.repository, image.tag)
            };

            self.pending_action = Some(PendingAction::new(
                "Delete Image".to_string(),
                format!("Delete image '{}'?", name),
                Severity::Danger,
                ActionType::DeleteImage {
                    id,
                    name,
                },
            ));
        }
    }

    /// Draw the images tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let info_color = self.theme.info;
        let fg_color = self.theme.fg;
        let accent_color = self.theme.accent;
        let success_color = self.theme.success;

        // Extract overlay state before mutable borrow
        let pull_overlay = match &self.pull_mode {
            PullMode::Input { text } => Some((true, text.clone(), String::new())),
            PullMode::Pulling { name, status } => Some((false, name.clone(), status.clone())),
            PullMode::Inactive => None,
        };

        let run_overlay = match &self.run_mode {
            RunMode::Input { text, image } => Some((text.clone(), image.clone())),
            RunMode::Inactive => None,
        };

        self.draw_main_table(frame, area);

        // Run mode overlay
        if let Some((text, image)) = run_overlay {
            let input_area = centered_rect(60, 20, area);
            frame.render_widget(ratatui::widgets::Clear, input_area);
            let input = ratatui::widgets::Paragraph::new(vec![
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("Image: ", Style::default().fg(info_color)),
                    ratatui::text::Span::styled(image, Style::default().fg(fg_color).add_modifier(Modifier::BOLD)),
                ]),
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("Name (optional): ", Style::default().fg(success_color).add_modifier(Modifier::BOLD)),
                    ratatui::text::Span::styled(text, Style::default().fg(fg_color)),
                    ratatui::text::Span::styled("_", Style::default().fg(accent_color)),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Run Container (Enter to run, Esc to cancel) ")
                    .border_style(Style::default().fg(success_color)),
            )
            .style(Style::default().bg(ratatui::style::Color::Black));
            frame.render_widget(input, input_area);
            return;
        }

        // Pull mode overlay
        if let Some((is_input, text_or_name, status)) = pull_overlay {
            if is_input {
                let input_area = centered_rect(50, 15, area);
                frame.render_widget(ratatui::widgets::Clear, input_area);
                let input = ratatui::widgets::Paragraph::new(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("Image: ", Style::default().fg(info_color).add_modifier(Modifier::BOLD)),
                    ratatui::text::Span::styled(text_or_name, Style::default().fg(fg_color)),
                    ratatui::text::Span::styled("_", Style::default().fg(accent_color)),
                ]))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Pull Image (Enter to pull, Esc to cancel) ")
                        .border_style(Style::default().fg(info_color)),
                )
                .style(Style::default().bg(ratatui::style::Color::Black));
                frame.render_widget(input, input_area);
            } else {
                let modal_area = centered_rect(60, 20, area);
                frame.render_widget(ratatui::widgets::Clear, modal_area);
                let msg = ratatui::widgets::Paragraph::new(vec![
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        format!("Pulling: {}", text_or_name),
                        Style::default().fg(info_color).add_modifier(Modifier::BOLD),
                    )),
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        status,
                        Style::default().fg(fg_color),
                    )),
                    ratatui::text::Line::from(""),
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        "Press Esc to dismiss",
                        Style::default().fg(self.theme.muted),
                    )),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Pull Progress ")
                        .border_style(Style::default().fg(info_color)),
                )
                .style(Style::default().bg(ratatui::style::Color::Black));
                frame.render_widget(msg, modal_area);
            }
            return;
        }

        if let Some(ref pending) = self.pending_action {
            ConfirmationModal::draw(frame, area, pending);
        }
    }

    fn draw_main_table(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let rows: Vec<Row> = self
            .images
            .iter()
            .map(|image| {
                let style = if image.is_dangling {
                    Style::default().fg(t.error)
                } else if image.repository == "<none>" {
                    Style::default().fg(t.warning)
                } else {
                    Style::default().fg(t.fg)
                };

                Row::new(vec![
                    Cell::from(image.id.clone()),
                    Cell::from(image.repository.clone()),
                    Cell::from(image.tag.clone()),
                    Cell::from(image.size.clone()),
                    Cell::from(image.created.clone()),
                ])
                .style(style)
            })
            .collect();

        let columns = ["ID", "Repository", "Tag", "Size", "Created"];
        let header_cells: Vec<Cell> = columns
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let label = if i == self.sort_state.column_index {
                    let arrow = match self.sort_state.direction {
                        SortDirection::Ascending => " ^",
                        SortDirection::Descending => " v",
                    };
                    format!("{}{}", name, arrow)
                } else {
                    name.to_string()
                };
                Cell::from(label).style(Style::default().add_modifier(Modifier::BOLD))
            })
            .collect();
        let header = Row::new(header_cells);

        let count = self.images.len();
        let dangling_count = self.images.iter().filter(|i| i.is_dangling).count();
        let title_text = format!("Images ({} total, {} dangling)", count, dangling_count);
        let title = if let Some(ref message) = self.status_message {
            format!("{} - {}", title_text, message)
        } else {
            title_text
        };

        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(30),
                Constraint::Length(15),
                Constraint::Length(10),
                Constraint::Length(20),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(
            Style::default()
                .bg(t.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    /// Get the currently selected image
    fn get_selected_image(&self) -> Option<&Image> {
        self.table_state
            .selected()
            .and_then(|index| self.images.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.images.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.images.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.images.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.images.len();
        self.table_state.select(Some(new_index));
    }

    /// Prune unused images
    async fn prune_images(&mut self) -> Result<()> {
        match self.docker_client.prune_images().await {
            Ok(space_reclaimed) => {
                let space_mb = space_reclaimed as f64 / 1_048_576.0;
                self.status_message = Some(format!("Pruned images, reclaimed {:.1} MB", space_mb));
                self.refresh().await?;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to prune images: {}", e));
            }
        }
        Ok(())
    }

    /// Select a specific row (for mouse click)
    pub fn select_row(&mut self, index: usize) {
        if index < self.images.len() {
            self.table_state.select(Some(index));
        }
    }

    /// Scroll up one row (for mouse scroll)
    pub fn scroll_up(&mut self) {
        self.move_selection_up();
    }

    /// Scroll down one row (for mouse scroll)
    pub fn scroll_down(&mut self) {
        self.move_selection_down();
    }

    fn apply_sort(&mut self) {
        let col = self.sort_state.column_index;
        let desc = self.sort_state.direction == SortDirection::Descending;
        self.images.sort_by(|a, b| {
            let cmp = match col {
                0 => a.id.to_lowercase().cmp(&b.id.to_lowercase()),
                1 => a.repository.to_lowercase().cmp(&b.repository.to_lowercase()),
                2 => a.tag.to_lowercase().cmp(&b.tag.to_lowercase()),
                3 => a.size.cmp(&b.size),
                4 => a.created.cmp(&b.created),
                _ => std::cmp::Ordering::Equal,
            };
            if desc { cmp.reverse() } else { cmp }
        });
    }

    fn cycle_sort_column(&mut self) {
        self.sort_state.column_index = (self.sort_state.column_index + 1) % 5;
        self.apply_sort();
    }

    fn reverse_sort_direction(&mut self) {
        self.sort_state.direction = match self.sort_state.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        self.apply_sort();
    }

    fn start_pull_async(&mut self, image_name: String) {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.pull_receiver = Some(receiver);

        let docker = self.docker_client.clone();
        let name = image_name.clone();
        tokio::spawn(async move {
            let _ = sender.send(format!("Downloading {}...", name));
            match docker.pull_image(&name).await {
                Ok(_) => {
                    let _ = sender.send(format!("DONE:Successfully pulled '{}'", name));
                }
                Err(e) => {
                    let _ = sender.send(format!("ERROR:Failed to pull '{}': {}", name, e));
                }
            }
        });
    }

    async fn run_container(&mut self, image: &str, name: Option<String>) -> Result<()> {
        match self.docker_client.create_and_start_container(image, name.as_deref()).await {
            Ok(id) => {
                self.status_message = Some(format!("Container '{}' started ({})", name.unwrap_or_else(|| image.to_string()), &id[..12.min(id.len())]));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to run: {}", e));
            }
        }
        Ok(())
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    use ratatui::layout::{Direction, Layout};
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
