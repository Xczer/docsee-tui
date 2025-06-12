use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{networks::Network, DockerClient},
    events::Key,
};

/// The networks tab widget
pub struct NetworksTab {
    /// Docker client for operations
    docker_client: DockerClient,
    /// List of networks
    networks: Vec<Network>,
    /// Table state for selection
    table_state: TableState,
    /// Status message to show
    status_message: Option<String>,
}

impl NetworksTab {
    /// Create a new networks tab
    pub async fn new(docker_client: DockerClient) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            networks: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
        };

        // Load initial data
        tab.refresh().await?;

        // Select first network if any exist
        if !tab.networks.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Refresh network data from Docker
    pub async fn refresh(&mut self) -> Result<()> {
        match self.docker_client.list_networks().await {
            Ok(networks) => {
                // Remember current selection
                let selected_id = self.get_selected_network().map(|n| n.id.clone());

                self.networks = networks;
                self.status_message = None;

                // Restore selection or select first item
                if let Some(id) = selected_id {
                    // Try to find the same network
                    let new_index = self.networks.iter().position(|n| n.id == id);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.networks.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading networks: {}", e));
            }
        }
        Ok(())
    }

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Up => self.move_selection_up(),
            Key::Down => self.move_selection_down(),
            Key::DeleteItem => self.delete_selected_network().await?,
            Key::Prune => self.prune_networks().await?,
            Key::Logs => {
                // Networks don't have logs, show helpful message
                self.status_message = Some("Networks don't have logs. Try containers instead!".to_string());
            }
            Key::Exec => {
                // Can't exec into networks, show helpful message
                self.status_message = Some("Can't execute into networks. Try containers instead!".to_string());
            }
            Key::Start => {
                // Can't start networks, show helpful message
                self.status_message = Some("Networks are not services. Try containers instead!".to_string());
            }
            Key::Stop => {
                // Can't stop networks, show helpful message
                self.status_message = Some("Networks are not running. Try containers instead!".to_string());
            }
            Key::Restart => {
                // Can't restart networks, show helpful message
                self.status_message = Some("Networks are not services. Try containers instead!".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Draw the networks tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Create table rows
        let rows: Vec<Row> = self
            .networks
            .iter()
            .map(|network| {
                let style = if network.connected_containers > 0 {
                    Style::default().fg(Color::Green) // Networks with containers in green
                } else if network.driver == "bridge" || network.driver == "host" || network.driver == "none" {
                    Style::default().fg(Color::Cyan) // Built-in networks in cyan
                } else {
                    Style::default().fg(Color::White) // Custom networks in white
                };

                let network_type = if network.internal {
                    "🔒 Internal"
                } else if network.ingress {
                    "🌐 Ingress"
                } else {
                    "🔗 External"
                };

                Row::new(vec![
                    Cell::from(network.id.clone()),
                    Cell::from(network.name.clone()),
                    Cell::from(network.driver.clone()),
                    Cell::from(network.scope.clone()),
                    Cell::from(network_type),
                    Cell::from(network.subnet.clone()),
                    Cell::from(network.connected_containers.to_string()),
                    Cell::from(network.created.clone()),
                ])
                .style(style)
            })
            .collect();

        // Create table headers
        let header = Row::new(vec![
            Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Driver").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Scope").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Subnet").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Containers").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Created").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        // Build the title string
        let count = self.networks.len();
        let active_count = self.networks.iter().filter(|n| n.connected_containers > 0).count();
        let custom_count = self.networks.iter().filter(|n| {
            !matches!(n.driver.as_str(), "bridge" | "host" | "none" | "null")
        }).count();

        let title_text = format!("Networks ({} total, {} active, {} custom)", count, active_count, custom_count);

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
                Constraint::Length(15), // Name
                Constraint::Length(10), // Driver
                Constraint::Length(8),  // Scope
                Constraint::Length(12), // Type
                Constraint::Length(16), // Subnet
                Constraint::Length(10), // Containers
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

    /// Get the currently selected network
    fn get_selected_network(&self) -> Option<&Network> {
        self.table_state
            .selected()
            .and_then(|index| self.networks.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.networks.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.networks.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.networks.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.networks.len();
        self.table_state.select(Some(new_index));
    }

    /// Delete the selected network
    async fn delete_selected_network(&mut self) -> Result<()> {
        if let Some(network) = self.get_selected_network() {
            let id = network.id.clone();
            let name = network.name.clone();

            // Safety check for built-in networks
            if matches!(network.driver.as_str(), "bridge" | "host" | "none") && 
               matches!(network.name.as_str(), "bridge" | "host" | "none") {
                self.status_message = Some(format!("Cannot delete built-in network '{}'", name));
                return Ok(());
            }

            // Safety check for networks with connected containers
            if network.connected_containers > 0 {
                self.status_message = Some(format!("Cannot delete network '{}' - {} containers still connected", name, network.connected_containers));
                return Ok(());
            }

            match self.docker_client.remove_network(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Deleted network '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to delete '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Prune unused networks
    async fn prune_networks(&mut self) -> Result<()> {
        match self.docker_client.prune_networks().await {
            Ok(deleted_networks) => {
                let count = deleted_networks.len();
                if count > 0 {
                    self.status_message = Some(format!("Pruned {} unused networks", count));
                } else {
                    self.status_message = Some("No unused networks to prune".to_string());
                }
                self.refresh().await?;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to prune networks: {}", e));
            }
        }
        Ok(())
    }
}

/*
EXPLANATION:
- NetworksTab manages the Docker networks display and interactions
- new() creates the tab and loads initial network data
- refresh() reloads networks from Docker, preserving current selection
- handle_key() processes keyboard shortcuts:
  - Up/Down arrows: navigate the network list
  - D: delete network (with safety checks for built-in networks and connected containers)
  - p: prune unused networks
  - Other container keys show helpful messages explaining they don't apply to networks
- draw() renders the networks table with color coding:
  - Green: networks with connected containers
  - Cyan: built-in Docker networks (bridge, host, none)
  - White: custom user networks
- The table shows: ID, Name, Driver, Scope, Type (Internal/Ingress/External), Subnet, Connected Containers, Created date
- Title shows total networks, active networks (with containers), and custom networks
- delete_selected_network() has safety checks:
  - Prevents deletion of built-in networks (bridge, host, none)
  - Prevents deletion of networks with connected containers
- prune_networks() cleans up unused networks and reports count
- Error handling provides user feedback for all operations
- Network type is shown with icons for visual clarity
*/
