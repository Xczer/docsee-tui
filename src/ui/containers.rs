use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};
use std::collections::HashSet;

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
    theme::Theme,
    widgets::modal::{ActionType, ConfirmationModal, PendingAction, Severity},
};

// Import our Phase 2 components
use crate::ui::inspect_viewer::InspectViewer;
use crate::ui::logs_viewer::LogsViewer;
use crate::ui::search_filter::AdvancedSearch;
use crate::ui::shell_executor::ShellExecutor;
use crate::ui::stats_viewer::StatsViewer;
use crate::ui::topology::TopologyViewer;

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Sort state for table columns
#[derive(Debug, Clone)]
pub struct SortState {
    pub column_index: usize,
    pub direction: SortDirection,
}

/// Enhanced containers tab with Phase 2 features
pub struct EnhancedContainersTab {
    /// Docker client for operations
    docker_client: DockerClient,
    /// List of all containers
    all_containers: Vec<Container>,
    /// Filtered containers (based on search/filter)
    filtered_containers: Vec<Container>,
    /// Table state for selection
    table_state: TableState,
    /// Status message to show
    status_message: Option<String>,
    /// Current view mode
    view_mode: ContainerViewMode,
    /// Logs viewer
    logs_viewer: LogsViewer,
    /// Shell executor
    shell_executor: ShellExecutor,
    /// Stats viewer
    stats_viewer: StatsViewer,
    /// Advanced search and filter
    search_filter: AdvancedSearch,
    /// Inspect viewer
    inspect_viewer: InspectViewer,
    /// Topology viewer
    topology_viewer: TopologyViewer,
    /// Pending confirmation action
    pending_action: Option<PendingAction>,
    /// Whether compose grouping is enabled
    compose_grouping: bool,
    /// Color theme
    theme: Theme,
    /// Sort state
    sort_state: SortState,
    /// Selected containers for bulk operations
    selected_containers: HashSet<String>,
}

/// Different view modes for the containers tab
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerViewMode {
    List,
    Logs,
    Shell,
    Stats,
    Inspect,
    Topology,
}

impl ContainerViewMode {
    pub fn name(&self) -> &'static str {
        match self {
            ContainerViewMode::List => "Container List",
            ContainerViewMode::Logs => "Container Logs",
            ContainerViewMode::Shell => "Shell Access",
            ContainerViewMode::Stats => "Resource Stats",
            ContainerViewMode::Inspect => "Container Inspect",
            ContainerViewMode::Topology => "Network Topology",
        }
    }
}

impl EnhancedContainersTab {
    /// Create a new enhanced containers tab
    pub async fn new(docker_client: DockerClient, theme: Theme) -> Result<Self> {
        let logs_viewer = LogsViewer::new(docker_client.clone());
        let shell_executor = ShellExecutor::new(docker_client.clone());
        let stats_viewer = StatsViewer::new(docker_client.clone());
        let search_filter = AdvancedSearch::new();
        let inspect_viewer = InspectViewer::new(docker_client.clone());
        let topology_viewer = TopologyViewer::new(docker_client.clone());

        let mut tab = Self {
            docker_client,
            all_containers: Vec::new(),
            filtered_containers: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
            view_mode: ContainerViewMode::List,
            logs_viewer,
            shell_executor,
            stats_viewer,
            search_filter,
            inspect_viewer,
            topology_viewer,
            pending_action: None,
            compose_grouping: false,
            theme,
            sort_state: SortState {
                column_index: 0,
                direction: SortDirection::Ascending,
            },
            selected_containers: HashSet::new(),
        };

        // Load initial data
        tab.refresh().await?;

        // Select first container if any exist
        if !tab.filtered_containers.is_empty() {
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

                self.all_containers = containers;

                // Update search suggestions
                self.search_filter.update_suggestions(&self.all_containers);

                // Apply current filters
                self.apply_filters();

                self.status_message = None;

                // Restore selection or select first item
                if let Some(id) = selected_id {
                    let new_index = self.filtered_containers.iter().position(|c| c.id == id);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.filtered_containers.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading containers: {}", e));
            }
        }

        // Update components that need regular refresh
        match self.view_mode {
            ContainerViewMode::Logs => {
                self.logs_viewer.update().await?;
            }
            ContainerViewMode::Stats => {
                self.stats_viewer.update().await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Apply current search and filter settings
    fn apply_filters(&mut self) {
        self.filtered_containers = self.search_filter.filter_containers(&self.all_containers);
        self.apply_sort();
    }

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        // Handle confirmation modal first
        if self.pending_action.is_some() {
            self.handle_confirmation_key(key).await?;
            return Ok(());
        }

        // Handle view-specific keys
        match self.view_mode {
            ContainerViewMode::List => {
                // Handle search/filter keys
                if self.search_filter.handle_key(key) {
                    if !self.search_filter.is_search_active() {
                        self.apply_filters();
                        if !self.filtered_containers.is_empty() {
                            self.table_state.select(Some(0));
                        }
                    }
                    return Ok(());
                }

                // Handle list navigation and actions
                match key {
                    Key::Up => self.move_selection_up(),
                    Key::Down => self.move_selection_down(),
                    Key::Start => self.start_selected_container().await?,
                    Key::Stop => self.confirm_stop_container(),
                    Key::Restart => self.restart_selected_container().await?,
                    Key::DeleteItem => self.confirm_delete_container(),
                    Key::Logs => self.enter_logs_view().await?,
                    Key::Exec => self.enter_shell_view().await?,
                    Key::Char('s') => self.enter_stats_view().await?,
                    Key::Char('i') => self.start_interactive_shell().await?,
                    Key::Char('g') => {
                        self.compose_grouping = !self.compose_grouping;
                        self.status_message = Some(if self.compose_grouping {
                            "Compose grouping enabled".to_string()
                        } else {
                            "Compose grouping disabled".to_string()
                        });
                    }
                    Key::Enter => self.enter_inspect_view().await?,
                    Key::Char('t') => self.enter_topology_view().await?,
                    // Sort
                    Key::Char('o') => self.cycle_sort_column(),
                    Key::Char('O') => self.reverse_sort_direction(),
                    // Bulk selection
                    Key::Char(' ') => self.toggle_bulk_selection(),
                    Key::Char('a') => self.select_all_visible(),
                    Key::Char('A') => self.deselect_all(),
                    // Bulk operations
                    Key::Char('U') => self.confirm_bulk_start(),
                    Key::Char('S') => self.confirm_bulk_stop(),
                    Key::Char('X') => self.confirm_bulk_delete(),
                    // Compose operations
                    Key::Char('C') => self.compose_up_selected().await?,
                    Key::Char('W') => self.compose_down_selected().await?,
                    _ => {}
                }
            }
            ContainerViewMode::Logs => match key {
                Key::Esc => self.exit_to_list_view().await?,
                _ => {
                    self.logs_viewer.handle_key(key).await?;
                }
            },
            ContainerViewMode::Shell => {
                if self.shell_executor.handle_key(key).await? {
                    self.exit_to_list_view().await?;
                }
            }
            ContainerViewMode::Stats => match key {
                Key::Esc => self.exit_to_list_view().await?,
                _ => {
                    self.stats_viewer.handle_key(key).await?;
                }
            },
            ContainerViewMode::Inspect => match key {
                Key::Esc => self.exit_to_list_view().await?,
                _ => {
                    self.inspect_viewer.handle_key(key).await?;
                }
            },
            ContainerViewMode::Topology => match key {
                Key::Esc => self.exit_to_list_view().await?,
                _ => {
                    self.topology_viewer.handle_key(key).await?;
                }
            },
        }

        Ok(())
    }

    /// Handle keys when confirmation modal is active
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
                        self.execute_confirmed_action(pending.action).await?;
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

    /// Execute a confirmed action
    async fn execute_confirmed_action(&mut self, action: ActionType) -> Result<()> {
        match action {
            ActionType::DeleteContainer { id, name } => {
                match self.docker_client.remove_container(&id, false).await {
                    Ok(_) => {
                        self.status_message = Some(format!("Deleted container '{}'", name));
                        self.refresh().await?;
                    }
                    Err(e) => {
                        self.status_message =
                            Some(format!("Failed to delete '{}': {}", name, e));
                    }
                }
            }
            ActionType::StopContainer { id, name } => {
                match self.docker_client.stop_container(&id).await {
                    Ok(_) => {
                        self.status_message = Some(format!("Stopped container '{}'", name));
                        self.refresh().await?;
                    }
                    Err(e) => {
                        self.status_message =
                            Some(format!("Failed to stop '{}': {}", name, e));
                    }
                }
            }
            ActionType::BulkStart { ids } => {
                let mut ok = 0;
                let mut fail = 0;
                for id in &ids {
                    match self.docker_client.start_container(id).await {
                        Ok(_) => ok += 1,
                        Err(_) => fail += 1,
                    }
                }
                self.selected_containers.clear();
                self.status_message = Some(format!("Started {ok}, failed {fail}"));
                self.refresh().await?;
            }
            ActionType::BulkStop { ids } => {
                let mut ok = 0;
                let mut fail = 0;
                for id in &ids {
                    match self.docker_client.stop_container(id).await {
                        Ok(_) => ok += 1,
                        Err(_) => fail += 1,
                    }
                }
                self.selected_containers.clear();
                self.status_message = Some(format!("Stopped {ok}, failed {fail}"));
                self.refresh().await?;
            }
            ActionType::BulkDelete { ids } => {
                let mut ok = 0;
                let mut fail = 0;
                for id in &ids {
                    match self.docker_client.remove_container(id, false).await {
                        Ok(_) => ok += 1,
                        Err(_) => fail += 1,
                    }
                }
                self.selected_containers.clear();
                self.status_message = Some(format!("Deleted {ok}, failed {fail}"));
                self.refresh().await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Confirm stop container action
    fn confirm_stop_container(&mut self) {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            if container.state != crate::docker::containers::ContainerState::Running {
                self.status_message = Some(format!("Container '{}' is not running", name));
                return;
            }

            self.pending_action = Some(PendingAction::new(
                "Stop Container".to_string(),
                format!("Stop container '{}'? This will send SIGTERM.", name),
                Severity::Warning,
                ActionType::StopContainer { id, name },
            ));
        }
    }

    /// Confirm delete container action
    fn confirm_delete_container(&mut self) {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            if container.state == crate::docker::containers::ContainerState::Running {
                self.status_message = Some(format!(
                    "Cannot delete running container '{}'. Stop it first.",
                    name
                ));
                return;
            }

            self.pending_action = Some(PendingAction::new(
                "Delete Container".to_string(),
                format!(
                    "Permanently delete container '{}'? This cannot be undone.",
                    name
                ),
                Severity::Danger,
                ActionType::DeleteContainer { id, name },
            ));
        }
    }

    /// Handle raw key events for shell mode
    pub async fn handle_shell_key_raw(&mut self, key: Key) -> Result<bool> {
        match self.view_mode {
            ContainerViewMode::Shell => {
                let shell_key = match key {
                    Key::Cheatsheet => Key::Char('c'),
                    Key::Logs => Key::Char('l'),
                    Key::Stop => Key::Char('d'),
                    Key::Restart => Key::Char('r'),
                    Key::Start => Key::Char('u'),
                    Key::Exec => Key::Char('e'),
                    Key::Prune => Key::Char('p'),
                    _ => key,
                };

                match self.shell_executor.handle_key(shell_key).await? {
                    true => {
                        self.exit_to_list_view().await?;
                        Ok(true)
                    }
                    false => Ok(false),
                }
            }
            _ => Ok(false),
        }
    }

    /// Draw the enhanced containers tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match self.view_mode {
            ContainerViewMode::List => self.draw_list_view(frame, area),
            ContainerViewMode::Logs => self.logs_viewer.draw(frame, area),
            ContainerViewMode::Shell => self.shell_executor.draw(frame, area),
            ContainerViewMode::Stats => self.stats_viewer.draw(frame, area),
            ContainerViewMode::Inspect => self.inspect_viewer.draw(frame, area),
            ContainerViewMode::Topology => self.topology_viewer.draw(frame, area),
        }

        // Draw confirmation modal on top if active
        if let Some(ref pending) = self.pending_action {
            ConfirmationModal::draw(frame, area, pending);
        }
    }

    /// Draw the container list view with search/filter
    fn draw_list_view(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Search and filter controls
                Constraint::Min(0),    // Container table
            ])
            .split(area);

        self.search_filter.draw(frame, chunks[0]);

        if self.compose_grouping {
            self.draw_grouped_table(frame, chunks[1]);
        } else {
            self.draw_container_table(frame, chunks[1]);
        }
    }

    /// Draw the container table with compose grouping
    fn draw_grouped_table(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        // Group containers by compose project
        let mut groups: Vec<(String, Vec<&Container>)> = Vec::new();
        let mut standalone: Vec<&Container> = Vec::new();

        let mut project_map: std::collections::BTreeMap<String, Vec<&Container>> =
            std::collections::BTreeMap::new();

        for container in &self.filtered_containers {
            if let Some(ref project) = container.compose_project {
                project_map
                    .entry(project.clone())
                    .or_default()
                    .push(container);
            } else {
                standalone.push(container);
            }
        }

        for (project, containers) in project_map {
            groups.push((project, containers));
        }
        if !standalone.is_empty() {
            groups.push(("Standalone".to_string(), standalone));
        }

        let mut rows: Vec<Row> = Vec::new();

        for (group_name, containers) in &groups {
            rows.push(
                Row::new(vec![
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(format!("--- {} ({}) ---", group_name, containers.len())),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ])
                .style(
                    Style::default()
                        .fg(t.title_3)
                        .add_modifier(Modifier::BOLD),
                ),
            );

            for container in containers {
                let style = container_row_style(container, t);
                let checkbox = if self.selected_containers.contains(&container.id) {
                    "[x]"
                } else {
                    "[ ]"
                };

                let service_name = container
                    .compose_service
                    .as_deref()
                    .map(|s| format!(" [{}]", s))
                    .unwrap_or_default();

                rows.push(
                    Row::new(vec![
                        Cell::from(checkbox.to_string()),
                        Cell::from(container.id.clone()),
                        Cell::from(format!("{}{}", container.name, service_name)),
                        Cell::from(container.image.clone()),
                        Cell::from(container.state.display()),
                        Cell::from(container.ports.clone()),
                        Cell::from(container.created.clone()),
                    ])
                    .style(style),
                );
            }
        }

        let header = self.table_header_with_sort();
        let title = self.build_title();

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Length(12),
                Constraint::Length(25),
                Constraint::Length(25),
                Constraint::Length(15),
                Constraint::Length(15),
                Constraint::Length(20),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} [grouped]", title)),
        )
        .row_highlight_style(
            Style::default()
                .bg(t.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    /// Draw the container table
    fn draw_container_table(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let rows: Vec<Row> = self
            .filtered_containers
            .iter()
            .map(|container| {
                let style = container_row_style(container, t);
                let checkbox = if self.selected_containers.contains(&container.id) {
                    "[x]"
                } else {
                    "[ ]"
                };

                Row::new(vec![
                    Cell::from(checkbox.to_string()),
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

        let header = self.table_header_with_sort();
        let title = self.build_title();

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Length(12),
                Constraint::Length(20),
                Constraint::Length(25),
                Constraint::Length(15),
                Constraint::Length(15),
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

    fn table_header_with_sort(&self) -> Row<'static> {
        let columns = ["", "ID", "Name", "Image", "Status", "Ports", "Created"];
        let cells: Vec<Cell> = columns
            .iter()
            .enumerate()
            .map(|(i, name)| {
                // Sort column index is offset by 1 because of checkbox column
                let label = if i > 0 && i - 1 == self.sort_state.column_index {
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
        Row::new(cells)
    }

    fn build_title(&self) -> String {
        let total_count = self.all_containers.len();
        let filtered_count = self.filtered_containers.len();
        let running_count = self
            .filtered_containers
            .iter()
            .filter(|c| c.state == crate::docker::containers::ContainerState::Running)
            .count();

        let filter_desc = self.search_filter.get_filter_description();
        let title_text = if total_count == filtered_count {
            format!(
                "Containers ({} total, {} running)",
                total_count, running_count
            )
        } else {
            format!(
                "Containers ({}/{} shown, {} running) - Filter: {}",
                filtered_count, total_count, running_count, filter_desc
            )
        };

        if let Some(ref message) = self.status_message {
            format!("{} - {}", title_text, message)
        } else {
            title_text
        }
    }

    /// Enter inspect view for selected container
    async fn enter_inspect_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.inspect_viewer.inspect(container).await?;
            self.view_mode = ContainerViewMode::Inspect;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Enter topology view
    async fn enter_topology_view(&mut self) -> Result<()> {
        self.topology_viewer.load().await?;
        self.view_mode = ContainerViewMode::Topology;
        self.status_message = None;
        Ok(())
    }

    /// Enter logs view for selected container
    async fn enter_logs_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.logs_viewer.start_logs(container).await?;
            self.view_mode = ContainerViewMode::Logs;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Enter shell view for selected container
    async fn enter_shell_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.shell_executor.set_container(container);
            self.view_mode = ContainerViewMode::Shell;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Enter stats view for selected container
    async fn enter_stats_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.stats_viewer.start_monitoring(container).await?;
            self.view_mode = ContainerViewMode::Stats;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Start interactive shell
    async fn start_interactive_shell(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            self.shell_executor
                .start_interactive_shell(container)
                .await?;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Exit back to list view
    async fn exit_to_list_view(&mut self) -> Result<()> {
        match self.view_mode {
            ContainerViewMode::Logs => {
                self.logs_viewer.stop_logs().await;
            }
            ContainerViewMode::Stats => {
                self.stats_viewer.stop_monitoring().await;
            }
            _ => {}
        }

        self.view_mode = ContainerViewMode::List;
        self.status_message = None;
        Ok(())
    }

    /// Get the currently selected container
    fn get_selected_container(&self) -> Option<&Container> {
        self.table_state
            .selected()
            .and_then(|index| self.filtered_containers.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.filtered_containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.filtered_containers.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.filtered_containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.filtered_containers.len();
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

    /// Start all containers in the selected container's compose project
    async fn compose_up_selected(&mut self) -> Result<()> {
        let project = self
            .get_selected_container()
            .and_then(|c| c.compose_project.clone());

        if let Some(project) = project {
            match self.docker_client.compose_up(&project).await {
                Ok((ok, fail)) => {
                    self.status_message = Some(format!(
                        "Compose up '{}': started {}, failed {}",
                        project, ok, fail
                    ));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Compose up failed: {}", e));
                }
            }
        } else {
            self.status_message = Some("Selected container is not part of a compose project".to_string());
        }
        Ok(())
    }

    /// Stop all containers in the selected container's compose project
    async fn compose_down_selected(&mut self) -> Result<()> {
        let project = self
            .get_selected_container()
            .and_then(|c| c.compose_project.clone());

        if let Some(project) = project {
            match self.docker_client.compose_down(&project).await {
                Ok((ok, fail)) => {
                    self.status_message = Some(format!(
                        "Compose down '{}': stopped {}, failed {}",
                        project, ok, fail
                    ));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Compose down failed: {}", e));
                }
            }
        } else {
            self.status_message = Some("Selected container is not part of a compose project".to_string());
        }
        Ok(())
    }

    /// Get current view mode
    pub fn get_view_mode(&self) -> &ContainerViewMode {
        &self.view_mode
    }

    /// Get status for display in parent UI
    pub fn get_status(&self) -> Option<String> {
        match &self.view_mode {
            ContainerViewMode::List => self.status_message.clone(),
            ContainerViewMode::Logs => {
                if let Some(container) = self.logs_viewer.get_container() {
                    Some(format!("Viewing logs for '{}'", container.name))
                } else {
                    Some("Logs viewer".to_string())
                }
            }
            ContainerViewMode::Shell => {
                if let Some(container) = self.shell_executor.get_container() {
                    Some(format!("Shell access to '{}'", container.name))
                } else {
                    Some("Shell executor".to_string())
                }
            }
            ContainerViewMode::Stats => {
                if let Some(container) = self.stats_viewer.get_container() {
                    Some(format!("Monitoring stats for '{}'", container.name))
                } else {
                    Some("Resource stats".to_string())
                }
            }
            ContainerViewMode::Inspect => {
                if let Some(container) = self.inspect_viewer.get_container() {
                    Some(format!("Inspecting '{}'", container.name))
                } else {
                    Some("Container inspect".to_string())
                }
            }
            ContainerViewMode::Topology => Some("Network topology view".to_string()),
        }
    }

    /// Check if we're in a sub-view (not the main list)
    pub fn is_in_subview(&self) -> bool {
        self.view_mode != ContainerViewMode::List
    }

    /// Force exit from any sub-view
    pub async fn force_exit_subview(&mut self) -> Result<()> {
        if self.is_in_subview() {
            self.exit_to_list_view().await?;
        }
        Ok(())
    }

    /// Select a specific row (for mouse click)
    pub fn select_row(&mut self, index: usize) {
        if index < self.filtered_containers.len() {
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

    /// Apply current sort to filtered containers
    fn apply_sort(&mut self) {
        let col = self.sort_state.column_index;
        let desc = self.sort_state.direction == SortDirection::Descending;
        self.filtered_containers.sort_by(|a, b| {
            let cmp = match col {
                0 => a.id.to_lowercase().cmp(&b.id.to_lowercase()),
                1 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => a.image.to_lowercase().cmp(&b.image.to_lowercase()),
                3 => a.state.display().to_lowercase().cmp(&b.state.display().to_lowercase()),
                4 => a.ports.to_lowercase().cmp(&b.ports.to_lowercase()),
                5 => a.created.cmp(&b.created),
                _ => std::cmp::Ordering::Equal,
            };
            if desc { cmp.reverse() } else { cmp }
        });
    }

    /// Cycle sort column forward
    fn cycle_sort_column(&mut self) {
        self.sort_state.column_index = (self.sort_state.column_index + 1) % 6;
        self.apply_sort();
    }

    /// Reverse sort direction
    fn reverse_sort_direction(&mut self) {
        self.sort_state.direction = match self.sort_state.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        self.apply_sort();
    }

    /// Toggle bulk selection on current row
    fn toggle_bulk_selection(&mut self) {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            if self.selected_containers.contains(&id) {
                self.selected_containers.remove(&id);
            } else {
                self.selected_containers.insert(id);
            }
            // Move selection down after toggling
            self.move_selection_down();
        }
    }

    /// Select all visible containers
    fn select_all_visible(&mut self) {
        for container in &self.filtered_containers {
            self.selected_containers.insert(container.id.clone());
        }
    }

    /// Deselect all containers
    fn deselect_all(&mut self) {
        self.selected_containers.clear();
    }

    /// Confirm bulk stop
    fn confirm_bulk_stop(&mut self) {
        if self.selected_containers.is_empty() {
            self.status_message = Some("No containers selected".to_string());
            return;
        }
        let count = self.selected_containers.len();
        self.pending_action = Some(PendingAction::new(
            "Bulk Stop".to_string(),
            format!("Stop {} selected containers?", count),
            Severity::Warning,
            ActionType::BulkStop {
                ids: self.selected_containers.iter().cloned().collect(),
            },
        ));
    }

    /// Confirm bulk delete
    fn confirm_bulk_delete(&mut self) {
        if self.selected_containers.is_empty() {
            self.status_message = Some("No containers selected".to_string());
            return;
        }
        let count = self.selected_containers.len();
        self.pending_action = Some(PendingAction::new(
            "Bulk Delete".to_string(),
            format!("Delete {} selected containers? This cannot be undone.", count),
            Severity::Danger,
            ActionType::BulkDelete {
                ids: self.selected_containers.iter().cloned().collect(),
            },
        ));
    }

    /// Confirm bulk start
    fn confirm_bulk_start(&mut self) {
        if self.selected_containers.is_empty() {
            self.status_message = Some("No containers selected".to_string());
            return;
        }
        let count = self.selected_containers.len();
        self.pending_action = Some(PendingAction::new(
            "Bulk Start".to_string(),
            format!("Start {} selected containers?", count),
            Severity::Normal,
            ActionType::BulkStart {
                ids: self.selected_containers.iter().cloned().collect(),
            },
        ));
    }
}

fn container_row_style(container: &Container, theme: &Theme) -> Style {
    match container.state {
        crate::docker::containers::ContainerState::Running => Style::default().fg(theme.success),
        crate::docker::containers::ContainerState::Stopped => Style::default().fg(theme.error),
        crate::docker::containers::ContainerState::Paused => Style::default().fg(theme.warning),
        _ => Style::default().fg(theme.muted),
    }
}
