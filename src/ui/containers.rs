use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    // text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

/// The containers tab widget
pub struct ContainersTab {
    /// Docker client for operations
    docker_client: DockerClient,
    /// List of containers
    containers: Vec<Container>,
    /// Table state for selection
    table_state: TableState,
    /// Status message to show
    status_message: Option<String>,
}

impl ContainersTab {
    /// Create a new containers tab
    pub async fn new(docker_client: DockerClient) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            containers: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
        };

        // Load initial data
        tab.refresh().await?;

        // Select first container if any exist
        if !tab.containers.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Refresh container data from Docker
    pub async fn refresh(&mut self) -> Result<()> {
        match self.docker_client.list_containers().await {
            Ok(containers) => {
                // Remember current selection
                let selected_id = self.get_selected_container().map(|c| c.id.clone());

                self.containers = containers;
                self.status_message = None;

                // Restore selection or select first item
                if let Some(id) = selected_id {
                    // Try to find the same container
                    let new_index = self.containers.iter().position(|c| c.id == id);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.containers.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading containers: {}", e));
            }
        }
        Ok(())
    }

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Up => self.move_selection_up(),
            Key::Down => self.move_selection_down(),
            Key::Start => self.start_selected_container().await?,
            Key::Stop => self.stop_selected_container().await?,
            Key::Restart => self.restart_selected_container().await?,
            Key::DeleteItem => self.delete_selected_container().await?,
            Key::Logs => {
                // TODO: Implement logs viewer
                self.status_message = Some("Logs viewer coming soon!".to_string());
            }
            Key::Exec => {
                // TODO: Implement shell execution
                self.status_message = Some("Shell execution coming soon!".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Draw the containers tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Create table rows
        let rows: Vec<Row> = self
            .containers
            .iter()
            .map(|container| {
                let style = match container.state {
                    crate::docker::containers::ContainerState::Running => {
                        Style::default().fg(Color::Green)
                    }
                    crate::docker::containers::ContainerState::Stopped => {
                        Style::default().fg(Color::Red)
                    }
                    crate::docker::containers::ContainerState::Paused => {
                        Style::default().fg(Color::Yellow)
                    }
                    _ => Style::default().fg(Color::Gray),
                };

                Row::new(vec![
                    Cell::from(container.id.clone()),
                    Cell::from(container.name.clone()),
                    Cell::from(container.image.clone()),
                    Cell::from(container.state.display()),
                    Cell::from(container.ports.clone()),
                    Cell::from(container.created.clone()),
                ])
                .style(style)
            })
            .collect();

        // Create table headers
        let header = Row::new(vec![
            Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Image").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Ports").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Created").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        // Build the title string
        let count = self.containers.len();
        let running_count = self
            .containers
            .iter()
            .filter(|c| c.state == crate::docker::containers::ContainerState::Running)
            .count();

        let title_text = format!("Containers ({} total, {} running)", count, running_count);

        let title = if let Some(ref message) = self.status_message {
            format!("{} - {}", title_text, message)
        } else {
            title_text
        };

        // Create the table widget
        let table = Table::new(
            rows,
            [
                Constraint::Length(12), // ID
                Constraint::Length(20), // Name
                Constraint::Length(25), // Image
                Constraint::Length(12), // Status
                Constraint::Length(15), // Ports
                Constraint::Length(20), // Created
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(">> ");

        // Render the table
        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    /// Get the currently selected container
    fn get_selected_container(&self) -> Option<&Container> {
        self.table_state
            .selected()
            .and_then(|index| self.containers.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.containers.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.containers.len();
        self.table_state.select(Some(new_index));
    }

    /// Start the selected container
    async fn start_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            match self.docker_client.start_container(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Started container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to start '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Stop the selected container
    async fn stop_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            match self.docker_client.stop_container(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Stopped container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to stop '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Restart the selected container
    async fn restart_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            match self.docker_client.restart_container(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Restarted container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to restart '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Delete the selected container
    async fn delete_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            // For safety, only allow deletion of stopped containers
            if container.state == crate::docker::containers::ContainerState::Running {
                self.status_message = Some(format!("Cannot delete running container '{}'. Stop it first.", name));
                return Ok(());
            }

            match self.docker_client.remove_container(&id, false).await {
                Ok(_) => {
                    self.status_message = Some(format!("Deleted container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to delete '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }


}

/*
EXPLANATION:
- ContainersTab manages the containers display and interactions
- new() creates the tab and loads initial container data
- refresh() reloads containers from Docker, preserving the current selection
- handle_key() processes keyboard shortcuts:
  - Up/Down arrows: navigate the container list
  - u: start container
  - d: stop container
  - r: restart container
  - D: delete container (only if stopped)
  - l/e: placeholder for logs/exec (coming soon)
- draw() renders the containers table with colored status indicators
- Container operations (start/stop/restart/delete) provide user feedback via status messages
- The table shows: ID, Name, Image, Status (with emoji), Ports, Created date
- Running containers are green, stopped are red, paused are yellow
- Selected row is highlighted with ">> " symbol
- Title shows total containers and running count
- Safety check prevents deletion of running containers
*/
