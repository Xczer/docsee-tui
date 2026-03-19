use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{volumes::Volume, DockerClient},
    events::Key,
    theme::Theme,
    widgets::modal::{ActionType, ConfirmationModal, PendingAction, Severity},
};

use super::containers::{SortDirection, SortState};

/// The volumes tab widget
pub struct VolumesTab {
    docker_client: DockerClient,
    volumes: Vec<Volume>,
    table_state: TableState,
    status_message: Option<String>,
    pending_action: Option<PendingAction>,
    theme: Theme,
    sort_state: SortState,
}

impl VolumesTab {
    /// Create a new volumes tab
    pub async fn new(docker_client: DockerClient, theme: Theme) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            volumes: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
            pending_action: None,
            theme,
            sort_state: SortState {
                column_index: 0,
                direction: SortDirection::Ascending,
            },
        };

        // Load initial data
        tab.refresh().await?;

        // Select first volume if any exist
        if !tab.volumes.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Refresh volume data from Docker
    pub async fn refresh(&mut self) -> Result<()> {
        match self.docker_client.list_volumes().await {
            Ok(volumes) => {
                // Remember current selection
                let selected_name = self.get_selected_volume().map(|v| v.name.clone());

                self.volumes = volumes;
                self.status_message = None;

                // Restore selection or select first item
                if let Some(name) = selected_name {
                    // Try to find the same volume
                    let new_index = self.volumes.iter().position(|v| v.name == name);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.volumes.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading volumes: {}", e));
            }
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
                            ActionType::DeleteVolume { name } => {
                                match self.docker_client.remove_volume(&name, false).await {
                                    Ok(_) => {
                                        self.status_message =
                                            Some(format!("Deleted volume '{}'", name));
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
                            ActionType::PruneVolumes => {
                                self.prune_volumes().await?;
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

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        if self.pending_action.is_some() {
            return self.handle_confirmation_key(key).await;
        }

        match key {
            Key::Up => self.move_selection_up(),
            Key::Down => self.move_selection_down(),
            Key::Char('o') => { self.cycle_sort_column(); }
            Key::Char('O') => { self.reverse_sort_direction(); }
            Key::DeleteItem => {
                if let Some(volume) = self.get_selected_volume() {
                    let name = volume.name.clone();
                    if volume.in_use {
                        self.status_message = Some(format!(
                            "Warning: Volume '{}' is in use!",
                            name
                        ));
                        return Ok(());
                    }
                    self.pending_action = Some(PendingAction::new(
                        "Delete Volume".to_string(),
                        format!("Delete volume '{}'?", name),
                        Severity::Danger,
                        ActionType::DeleteVolume { name },
                    ));
                }
            }
            Key::Prune => {
                self.pending_action = Some(PendingAction::new(
                    "Prune Volumes".to_string(),
                    "Remove all unused volumes?".to_string(),
                    Severity::Warning,
                    ActionType::PruneVolumes,
                ));
            }
            Key::Logs => {
                // Volumes don't have logs, show helpful message
                self.status_message =
                    Some("Volumes don't have logs. Try containers instead!".to_string());
            }
            Key::Exec => {
                // Can't exec into volumes, show helpful message
                self.status_message =
                    Some("Can't execute into volumes. Try containers instead!".to_string());
            }
            Key::Start => {
                // Can't start volumes, show helpful message
                self.status_message =
                    Some("Volumes are not services. Try containers instead!".to_string());
            }
            Key::Stop => {
                // Can't stop volumes, show helpful message
                self.status_message =
                    Some("Volumes are not running. Try containers instead!".to_string());
            }
            Key::Restart => {
                // Can't restart volumes, show helpful message
                self.status_message =
                    Some("Volumes are not services. Try containers instead!".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Draw the volumes tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let rows: Vec<Row> = self
            .volumes
            .iter()
            .map(|volume| {
                let style = if volume.in_use {
                    Style::default().fg(t.success)
                } else {
                    Style::default().fg(t.fg)
                };

                Row::new(vec![
                    Cell::from(volume.name.clone()),
                    Cell::from(volume.driver.clone()),
                    Cell::from(volume.scope.clone()),
                    Cell::from(volume.size.clone()),
                    Cell::from(if volume.in_use { "Yes" } else { "No" }),
                    Cell::from(volume.created.clone()),
                ])
                .style(style)
            })
            .collect();

        let columns = ["Name", "Driver", "Scope", "Size", "In Use", "Created"];
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

        // Build the title string
        let count = self.volumes.len();
        let in_use_count = self.volumes.iter().filter(|v| v.in_use).count();

        let title_text = format!("Volumes ({} total, {} in use)", count, in_use_count);

        let title = if let Some(ref message) = self.status_message {
            format!("{} - {}", title_text, message)
        } else {
            title_text
        };

        // Create the table widget
        let table = Table::new(
            rows,
            [
                Constraint::Length(25), // Name
                Constraint::Length(12), // Driver
                Constraint::Length(8),  // Scope
                Constraint::Length(10), // Size
                Constraint::Length(8),  // In Use
                Constraint::Length(20), // Created
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

        if let Some(ref pending) = self.pending_action {
            ConfirmationModal::draw(frame, area, pending);
        }
    }

    /// Get the currently selected volume
    fn get_selected_volume(&self) -> Option<&Volume> {
        self.table_state
            .selected()
            .and_then(|index| self.volumes.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.volumes.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.volumes.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.volumes.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.volumes.len();
        self.table_state.select(Some(new_index));
    }

    /// Prune unused volumes
    async fn prune_volumes(&mut self) -> Result<()> {
        match self.docker_client.prune_volumes().await {
            Ok(space_reclaimed) => {
                let space_mb = space_reclaimed as f64 / 1_048_576.0;
                self.status_message = Some(format!("Pruned volumes, reclaimed {:.1} MB", space_mb));
                self.refresh().await?;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to prune volumes: {}", e));
            }
        }
        Ok(())
    }

    pub fn select_row(&mut self, index: usize) {
        if index < self.volumes.len() {
            self.table_state.select(Some(index));
        }
    }

    pub fn scroll_up(&mut self) {
        self.move_selection_up();
    }

    pub fn scroll_down(&mut self) {
        self.move_selection_down();
    }

    fn apply_sort(&mut self) {
        let col = self.sort_state.column_index;
        let desc = self.sort_state.direction == SortDirection::Descending;
        self.volumes.sort_by(|a, b| {
            let cmp = match col {
                0 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                1 => a.driver.to_lowercase().cmp(&b.driver.to_lowercase()),
                2 => a.scope.to_lowercase().cmp(&b.scope.to_lowercase()),
                3 => a.size.cmp(&b.size),
                4 => a.in_use.cmp(&b.in_use),
                5 => a.created.cmp(&b.created),
                _ => std::cmp::Ordering::Equal,
            };
            if desc { cmp.reverse() } else { cmp }
        });
    }

    fn cycle_sort_column(&mut self) {
        self.sort_state.column_index = (self.sort_state.column_index + 1) % 6;
        self.apply_sort();
    }

    fn reverse_sort_direction(&mut self) {
        self.sort_state.direction = match self.sort_state.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        self.apply_sort();
    }
}
