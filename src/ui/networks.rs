use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{networks::Network, DockerClient},
    events::Key,
    theme::Theme,
    widgets::modal::{ActionType, ConfirmationModal, PendingAction, Severity},
};

use super::containers::{SortDirection, SortState};

/// The networks tab widget
pub struct NetworksTab {
    docker_client: DockerClient,
    networks: Vec<Network>,
    table_state: TableState,
    status_message: Option<String>,
    pending_action: Option<PendingAction>,
    theme: Theme,
    sort_state: SortState,
}

impl NetworksTab {
    /// Create a new networks tab
    pub async fn new(docker_client: DockerClient, theme: Theme) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            networks: Vec::new(),
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
                            ActionType::DeleteNetwork { id, name } => {
                                match self.docker_client.remove_network(&id).await {
                                    Ok(_) => {
                                        self.status_message =
                                            Some(format!("Deleted network '{}'", name));
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
                            ActionType::PruneNetworks => {
                                self.prune_networks().await?;
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
            Key::Char('o') => { self.cycle_sort_column(); return Ok(()); }
            Key::Char('O') => { self.reverse_sort_direction(); return Ok(()); }
            Key::DeleteItem => {
                if let Some(network) = self.get_selected_network() {
                    let id = network.id.clone();
                    let name = network.name.clone();

                    if matches!(network.driver.as_str(), "bridge" | "host" | "none")
                        && matches!(network.name.as_str(), "bridge" | "host" | "none")
                    {
                        self.status_message =
                            Some(format!("Cannot delete built-in network '{}'", name));
                        return Ok(());
                    }

                    if network.connected_containers > 0 {
                        self.status_message = Some(format!(
                            "Cannot delete network '{}' - {} containers still connected",
                            name, network.connected_containers
                        ));
                        return Ok(());
                    }

                    self.pending_action = Some(PendingAction::new(
                        "Delete Network".to_string(),
                        format!("Delete network '{}'?", name),
                        Severity::Danger,
                        ActionType::DeleteNetwork { id, name },
                    ));
                }
            }
            Key::Prune => {
                self.pending_action = Some(PendingAction::new(
                    "Prune Networks".to_string(),
                    "Remove all unused networks?".to_string(),
                    Severity::Warning,
                    ActionType::PruneNetworks,
                ));
            }
            Key::Logs => {
                // Networks don't have logs, show helpful message
                self.status_message =
                    Some("Networks don't have logs. Try containers instead!".to_string());
            }
            Key::Exec => {
                // Can't exec into networks, show helpful message
                self.status_message =
                    Some("Can't execute into networks. Try containers instead!".to_string());
            }
            Key::Start => {
                // Can't start networks, show helpful message
                self.status_message =
                    Some("Networks are not services. Try containers instead!".to_string());
            }
            Key::Stop => {
                // Can't stop networks, show helpful message
                self.status_message =
                    Some("Networks are not running. Try containers instead!".to_string());
            }
            Key::Restart => {
                // Can't restart networks, show helpful message
                self.status_message =
                    Some("Networks are not services. Try containers instead!".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Draw the networks tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let rows: Vec<Row> = self
            .networks
            .iter()
            .map(|network| {
                let style = if network.connected_containers > 0 {
                    Style::default().fg(t.success)
                } else if network.driver == "bridge"
                    || network.driver == "host"
                    || network.driver == "none"
                {
                    Style::default().fg(t.info)
                } else {
                    Style::default().fg(t.fg)
                };

                let network_type = if network.internal {
                    "Internal"
                } else if network.ingress {
                    "Ingress"
                } else {
                    "External"
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

        let columns = ["ID", "Name", "Driver", "Scope", "Type", "Subnet", "Containers", "Created"];
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
        let count = self.networks.len();
        let active_count = self
            .networks
            .iter()
            .filter(|n| n.connected_containers > 0)
            .count();
        let custom_count = self
            .networks
            .iter()
            .filter(|n| !matches!(n.driver.as_str(), "bridge" | "host" | "none" | "null"))
            .count();

        let title_text = format!(
            "Networks ({} total, {} active, {} custom)",
            count, active_count, custom_count
        );

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

    pub fn select_row(&mut self, index: usize) {
        if index < self.networks.len() {
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
        self.networks.sort_by(|a, b| {
            let cmp = match col {
                0 => a.id.to_lowercase().cmp(&b.id.to_lowercase()),
                1 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => a.driver.to_lowercase().cmp(&b.driver.to_lowercase()),
                3 => a.scope.to_lowercase().cmp(&b.scope.to_lowercase()),
                4 => std::cmp::Ordering::Equal,
                5 => a.subnet.cmp(&b.subnet),
                6 => a.connected_containers.cmp(&b.connected_containers),
                7 => a.created.cmp(&b.created),
                _ => std::cmp::Ordering::Equal,
            };
            if desc { cmp.reverse() } else { cmp }
        });
    }

    fn cycle_sort_column(&mut self) {
        self.sort_state.column_index = (self.sort_state.column_index + 1) % 8;
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
