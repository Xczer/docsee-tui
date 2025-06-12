use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, Table, TableState},
    Frame,
};
use std::collections::HashSet;

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
    theme::{relative_time, truncate_with_ellipsis, Theme},
    widgets::modal::{ActionType, ConfirmationModal, PendingAction, Severity},
};

// the sub-views a container can drill into
use crate::ui::inspect_viewer::InspectViewer;
use crate::ui::logs_viewer::LogsViewer;
use crate::ui::search_filter::AdvancedSearch;
use crate::ui::shell_executor::ShellExecutor;
use crate::ui::stats_viewer::StatsViewer;
use crate::ui::topology::TopologyViewer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// which column we're sorting on + which way
#[derive(Debug, Clone)]
pub struct SortState {
    pub column_index: usize,
    pub direction: SortDirection,
}

/// The containers tab - the main screen, does listing + all the drill-in views.
pub struct EnhancedContainersTab {
    docker_client: DockerClient,
    all_containers: Vec<Container>,
    // what's actually shown after search/filter runs
    filtered_containers: Vec<Container>,
    table_state: TableState,
    status_message: Option<String>,
    view_mode: ContainerViewMode,
    logs_viewer: LogsViewer,
    shell_executor: ShellExecutor,
    stats_viewer: StatsViewer,
    search_filter: AdvancedSearch,
    inspect_viewer: InspectViewer,
    topology_viewer: TopologyViewer,
    pending_action: Option<PendingAction>,
    compose_grouping: bool,
    theme: Theme,
    sort_state: SortState,
    // ids ticked for bulk start/stop/delete
    selected_containers: HashSet<String>,
}

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
    /// Set up the tab and its sub-views. Docker call happens lazily on first refresh.
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

        tab.refresh().await?;

        // land on first row so the highlight isn't empty on open
        if !tab.filtered_containers.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Pull the container list again and reapply filters/sort.
    pub async fn refresh(&mut self) -> Result<()> {
        match self.docker_client.list_containers().await {
            Ok(containers) => {
                // stash the selected id so we can put the cursor back after reload
                let selected_id = self.get_selected_container().map(|c| c.id.clone());

                self.all_containers = containers;

                self.search_filter.update_suggestions(&self.all_containers);

                self.apply_filters();

                self.status_message = None;

                // try to restore the old selection, else fall back to row 0
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

        // logs/stats sub-views tail live data, so poke them every refresh too
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

    fn apply_filters(&mut self) {
        self.filtered_containers = self.search_filter.filter_containers(&self.all_containers);
        self.apply_sort();
    }

    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        // modal grabs all keys while it's up
        if self.pending_action.is_some() {
            self.handle_confirmation_key(key).await?;
            return Ok(());
        }

        match self.view_mode {
            ContainerViewMode::List => {
                // let the search bar eat the key first if it's in edit mode
                if self.search_filter.handle_key(key) {
                    if !self.search_filter.is_search_active() {
                        self.apply_filters();
                        if !self.filtered_containers.is_empty() {
                            self.table_state.select(Some(0));
                        }
                    }
                    return Ok(());
                }

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
                    Key::Char('o') => self.cycle_sort_column(),
                    Key::Char('O') => self.reverse_sort_direction(),
                    // space toggles the tick, caps A/a for select-all vs clear
                    Key::Char(' ') => self.toggle_bulk_selection(),
                    Key::Char('a') => self.select_all_visible(),
                    Key::Char('A') => self.deselect_all(),
                    Key::Char('U') => self.confirm_bulk_start(),
                    Key::Char('S') => self.confirm_bulk_stop(),
                    Key::Char('X') => self.confirm_bulk_delete(),
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

    /// runs the action the modal was confirming (delete/stop/bulk)
    async fn execute_confirmed_action(&mut self, action: ActionType) -> Result<()> {
        match action {
            ActionType::DeleteContainer { id, name } => {
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
            ActionType::StopContainer { id, name } => {
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

    /// shell mode swallows the global hotkeys, so remap them back to plain chars for the shell
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

    /// Render the container list (or whichever sub-view is active).
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match self.view_mode {
            ContainerViewMode::List => self.draw_list_view(frame, area),
            ContainerViewMode::Logs => self.logs_viewer.draw(frame, area),
            ContainerViewMode::Shell => self.shell_executor.draw(frame, area),
            ContainerViewMode::Stats => self.stats_viewer.draw(frame, area),
            ContainerViewMode::Inspect => self.inspect_viewer.draw(frame, area),
            ContainerViewMode::Topology => self.topology_viewer.draw(frame, area),
        }

        // modal goes last so it paints over whatever view is behind it
        if let Some(ref pending) = self.pending_action {
            ConfirmationModal::draw(frame, area, pending);
        }
    }

    fn draw_list_view(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // search + filter bar
                Constraint::Min(0),    // the table takes the rest
            ])
            .split(area);

        self.search_filter.draw(frame, chunks[0]);

        if self.compose_grouping {
            self.draw_grouped_table(frame, chunks[1]);
        } else {
            self.draw_container_table(frame, chunks[1]);
        }
    }

    /// same table but bucketed by compose project, with header rows between groups
    fn draw_grouped_table(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
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
                .style(Style::default().fg(t.title_3).add_modifier(Modifier::BOLD)),
            );

            for (i, container) in containers.iter().enumerate() {
                let mut style = container_row_style(container, t);
                // zebra stripe the odd rows so the table is easier to scan
                if i % 2 == 1 {
                    style = style.bg(t.highlight_alt);
                }
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

                let name_with_service = format!("{}{}", container.name, service_name);

                rows.push(
                    Row::new(vec![
                        Cell::from(checkbox.to_string()),
                        Cell::from(container.id.clone()),
                        Cell::from(truncate_with_ellipsis(&name_with_service, 25)),
                        Cell::from(truncate_with_ellipsis(&container.image, 22)),
                        Cell::from(compact_status(&container.state)),
                        Cell::from(container.ports.clone()),
                        Cell::from(relative_time(&container.created)),
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
                .border_type(BorderType::Rounded)
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

    fn draw_container_table(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let rows: Vec<Row> = self
            .filtered_containers
            .iter()
            .enumerate()
            .map(|(i, container)| {
                let mut style = container_row_style(container, t);
                // zebra stripe the odd rows so the table is easier to scan
                if i % 2 == 1 {
                    style = style.bg(t.highlight_alt);
                }
                let checkbox = if self.selected_containers.contains(&container.id) {
                    "[x]"
                } else {
                    "[ ]"
                };

                Row::new(vec![
                    Cell::from(checkbox.to_string()),
                    Cell::from(container.id.clone()),
                    Cell::from(truncate_with_ellipsis(&container.name, 20)),
                    Cell::from(truncate_with_ellipsis(&container.image, 22)),
                    Cell::from(compact_status(&container.state)),
                    Cell::from(container.ports.clone()),
                    Cell::from(relative_time(&container.created)),
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(title),
        )
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
                // -1 because the checkbox column shifts everything, col 0 has no sort
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

    async fn enter_topology_view(&mut self) -> Result<()> {
        self.topology_viewer.load().await?;
        self.view_mode = ContainerViewMode::Topology;
        self.status_message = None;
        Ok(())
    }

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

    /// drops into a real attached shell (suspends the TUI while it runs)
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

    /// back to the list, tearing down any live log/stats stream on the way out
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

    fn get_selected_container(&self) -> Option<&Container> {
        self.table_state
            .selected()
            .and_then(|index| self.filtered_containers.get(index))
    }

    // wraps around to the bottom when you go up past row 0
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

    fn move_selection_down(&mut self) {
        if self.filtered_containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.filtered_containers.len();
        self.table_state.select(Some(new_index));
    }

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

    /// brings up the whole compose project the selected container belongs to
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
            self.status_message =
                Some("Selected container is not part of a compose project".to_string());
        }
        Ok(())
    }

    /// tears down the whole compose project the selected container belongs to
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
            self.status_message =
                Some("Selected container is not part of a compose project".to_string());
        }
        Ok(())
    }

    pub fn get_view_mode(&self) -> &ContainerViewMode {
        &self.view_mode
    }

    /// status line text the parent app shows at the bottom
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

    /// true when we're in logs/shell/stats/inspect/topology rather than the list
    pub fn is_in_subview(&self) -> bool {
        self.view_mode != ContainerViewMode::List
    }

    pub async fn force_exit_subview(&mut self) -> Result<()> {
        if self.is_in_subview() {
            self.exit_to_list_view().await?;
        }
        Ok(())
    }

    /// jump the cursor to a row, used by mouse clicks
    pub fn select_row(&mut self, index: usize) {
        if index < self.filtered_containers.len() {
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
        self.filtered_containers.sort_by(|a, b| {
            let cmp = match col {
                0 => a.id.to_lowercase().cmp(&b.id.to_lowercase()),
                1 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => a.image.to_lowercase().cmp(&b.image.to_lowercase()),
                3 => a
                    .state
                    .display()
                    .to_lowercase()
                    .cmp(&b.state.display().to_lowercase()),
                4 => a.ports.to_lowercase().cmp(&b.ports.to_lowercase()),
                5 => a.created.cmp(&b.created),
                _ => std::cmp::Ordering::Equal,
            };
            if desc {
                cmp.reverse()
            } else {
                cmp
            }
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

    fn toggle_bulk_selection(&mut self) {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            if self.selected_containers.contains(&id) {
                self.selected_containers.remove(&id);
            } else {
                self.selected_containers.insert(id);
            }
            // hop to next row so you can tick a run of them fast
            self.move_selection_down();
        }
    }

    fn select_all_visible(&mut self) {
        for container in &self.filtered_containers {
            self.selected_containers.insert(container.id.clone());
        }
    }

    fn deselect_all(&mut self) {
        self.selected_containers.clear();
    }

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

    fn confirm_bulk_delete(&mut self) {
        if self.selected_containers.is_empty() {
            self.status_message = Some("No containers selected".to_string());
            return;
        }
        let count = self.selected_containers.len();
        self.pending_action = Some(PendingAction::new(
            "Bulk Delete".to_string(),
            format!(
                "Delete {} selected containers? This cannot be undone.",
                count
            ),
            Severity::Danger,
            ActionType::BulkDelete {
                ids: self.selected_containers.iter().cloned().collect(),
            },
        ));
    }

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

    /// Returns (total, running) counts from all_containers
    pub fn container_counts(&self) -> (usize, usize) {
        let total = self.all_containers.len();
        let running = self
            .all_containers
            .iter()
            .filter(|c| c.state == crate::docker::containers::ContainerState::Running)
            .count();
        (total, running)
    }

    /// Get the name of the currently selected container
    pub fn get_selected_container_name(&self) -> Option<String> {
        self.get_selected_container().map(|c| c.name.clone())
    }
}

fn compact_status(state: &crate::docker::containers::ContainerState) -> &'static str {
    match state {
        crate::docker::containers::ContainerState::Running => "● Up",
        crate::docker::containers::ContainerState::Stopped => "■ Down",
        crate::docker::containers::ContainerState::Paused => "◆ Pause",
        crate::docker::containers::ContainerState::Restarting => "↻ Restart",
        crate::docker::containers::ContainerState::Dead => "✕ Dead",
        crate::docker::containers::ContainerState::Unknown => "? N/A",
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
