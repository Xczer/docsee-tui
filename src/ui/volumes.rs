use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{volumes::Volume, DockerClient},
    events::Key,
};

/// The volumes tab widget
pub struct VolumesTab {
    /// Docker client for operations
    docker_client: DockerClient,
    /// List of volumes
    volumes: Vec<Volume>,
    /// Table state for selection
    table_state: TableState,
    /// Status message to show
    status_message: Option<String>,
}

impl VolumesTab {
    /// Create a new volumes tab
    pub async fn new(docker_client: DockerClient) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            volumes: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
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

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Up => self.move_selection_up(),
            Key::Down => self.move_selection_down(),
            Key::DeleteItem => self.delete_selected_volume().await?,
            Key::Prune => self.prune_volumes().await?,
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
        // Create table rows
        let rows: Vec<Row> = self
            .volumes
            .iter()
            .map(|volume| {
                let style = if volume.in_use {
                    Style::default().fg(Color::Green) // In-use volumes in green
                } else {
                    Style::default().fg(Color::White) // Unused volumes in white
                };

                Row::new(vec![
                    Cell::from(volume.name.clone()),
                    Cell::from(volume.driver.clone()),
                    Cell::from(volume.scope.clone()),
                    Cell::from(volume.size.clone()),
                    Cell::from(if volume.in_use { "🟢 Yes" } else { "⭕ No" }),
                    Cell::from(volume.created.clone()),
                ])
                .style(style)
            })
            .collect();

        // Create table headers
        let header = Row::new(vec![
            Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Driver").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Scope").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Size").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("In Use").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Created").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

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
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

        // Render the table
        frame.render_stateful_widget(table, area, &mut self.table_state);
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

    /// Delete the selected volume
    async fn delete_selected_volume(&mut self) -> Result<()> {
        if let Some(volume) = self.get_selected_volume() {
            let name = volume.name.clone();

            // For safety, warn about deleting in-use volumes
            if volume.in_use {
                self.status_message = Some(format!(
                    "Warning: Volume '{}' is in use! Delete anyway with force.",
                    name
                ));
                // You could implement a confirmation dialog here
                return Ok(());
            }

            match self.docker_client.remove_volume(&name, false).await {
                Ok(_) => {
                    self.status_message = Some(format!("Deleted volume '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    // Try force delete if normal delete fails
                    match self.docker_client.remove_volume(&name, true).await {
                        Ok(_) => {
                            self.status_message = Some(format!("Force deleted volume '{}'", name));
                            self.refresh().await?;
                        }
                        Err(_) => {
                            self.status_message =
                                Some(format!("Failed to delete '{}': {}", name, e));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Prune unused volumes
    async fn prune_volumes(&mut self) -> Result<()> {
        match self.docker_client.prune_volumes().await {
            Ok(space_reclaimed) => {
                let space_mb = space_reclaimed as f64 / 1_048_576.0; // Convert to MB
                self.status_message = Some(format!("Pruned volumes, reclaimed {:.1} MB", space_mb));
                self.refresh().await?;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to prune volumes: {}", e));
            }
        }
        Ok(())
    }
}

/*
EXPLANATION:
- VolumesTab manages the Docker volumes display and interactions
- new() creates the tab and loads initial volume data
- refresh() reloads volumes from Docker, preserving current selection
- handle_key() processes keyboard shortcuts:
  - Up/Down arrows: navigate the volume list
  - D: delete volume (with safety checks for in-use volumes)
  - p: prune unused volumes
  - Other container keys show helpful messages explaining they don't apply to volumes
- draw() renders the volumes table with color coding:
  - Green: volumes currently in use by containers
  - White: unused volumes
- The table shows: Name, Driver, Scope, Size, In Use status, Created date
- Title shows total volumes and in-use count
- delete_selected_volume() has safety checks and tries force delete if needed
- prune_volumes() cleans up unused volumes and shows space reclaimed
- Error handling provides user feedback for all operations
*/
