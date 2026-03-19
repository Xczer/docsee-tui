use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::fmt::Debug;

use crate::{
    docker::containers::{Container, ContainerState},
    events::Key,
};

/// Search and filter modes
#[derive(Debug, Clone, PartialEq)]
pub enum FilterMode {
    All,
    Running,
    Stopped,
    ByName(String),
    ByImage(String),
    ByStatus(ContainerState),
}

impl FilterMode {
    pub fn name(&self) -> String {
        match self {
            FilterMode::All => "All".to_string(),
            FilterMode::Running => "Running Only".to_string(),
            FilterMode::Stopped => "Stopped Only".to_string(),
            FilterMode::ByName(name) => format!("Name: {}", name),
            FilterMode::ByImage(image) => format!("Image: {}", image),
            FilterMode::ByStatus(status) => format!("Status: {:?}", status),
        }
    }
}

/// Search input widget
pub struct SearchInput {
    /// Current search query
    query: String,
    /// Whether the search input is active
    active: bool,
    /// Cursor position in the input
    cursor_position: usize,
    /// Search mode (name, image, etc.)
    search_mode: SearchMode,
}

/// What type of search to perform
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMode {
    Name,
    Image,
    Status,
    All, // Search across all fields
}

impl SearchMode {
    pub fn name(&self) -> &'static str {
        match self {
            SearchMode::Name => "Name",
            SearchMode::Image => "Image",
            SearchMode::Status => "Status",
            SearchMode::All => "All Fields",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SearchMode::Name => SearchMode::Image,
            SearchMode::Image => SearchMode::Status,
            SearchMode::Status => SearchMode::All,
            SearchMode::All => SearchMode::Name,
        }
    }
}

impl Default for SearchInput {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchInput {
    /// Create a new search input
    pub fn new() -> Self {
        Self {
            query: String::new(),
            active: false,
            cursor_position: 0,
            search_mode: SearchMode::All,
        }
    }

    /// Activate the search input
    pub fn activate(&mut self) {
        self.active = true;
        self.cursor_position = self.query.len();
    }

    /// Deactivate the search input
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Check if the search input is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the current query
    pub fn get_query(&self) -> &str {
        &self.query
    }

    /// Get the current search mode
    pub fn get_search_mode(&self) -> &SearchMode {
        &self.search_mode
    }

    /// Clear the search query
    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor_position = 0;
    }

    /// Handle key input for search
    pub fn handle_key(&mut self, key: Key) -> bool {
        if !self.active {
            return false;
        }

        match key {
            Key::Enter => {
                self.deactivate();
                true
            }
            Key::Esc => {
                self.clear();
                self.deactivate();
                true
            }
            Key::Char(c) => {
                self.query.insert(self.cursor_position, c);
                self.cursor_position += 1;
                true
            }
            Key::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.query.remove(self.cursor_position);
                }
                true
            }
            Key::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
                true
            }
            Key::Right => {
                if self.cursor_position < self.query.len() {
                    self.cursor_position += 1;
                }
                true
            }
            Key::Home => {
                self.cursor_position = 0;
                true
            }
            Key::End => {
                self.cursor_position = self.query.len();
                true
            }
            Key::Tab => {
                self.search_mode = self.search_mode.next();
                true
            }
            _ => false,
        }
    }

    /// Draw the search input
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let style = if self.active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let mut input_text = self.query.clone();

        // Add cursor if active
        if self.active {
            if self.cursor_position < input_text.len() {
                input_text.insert(self.cursor_position, '█');
            } else {
                input_text.push('█');
            }
        }

        let title = if self.active {
            format!(
                "🔍 Search {} (Tab to switch, Enter to apply, Esc to cancel)",
                self.search_mode.name()
            )
        } else {
            format!(
                "🔍 Search {} (Press / to activate)",
                self.search_mode.name()
            )
        };

        let search_widget = Paragraph::new(input_text)
            .style(style)
            .block(Block::default().borders(Borders::ALL).title(title));

        frame.render_widget(search_widget, area);
    }
}

/// Filter manager for managing search and filtering logic
pub struct FilterManager {
    /// Current filter mode
    current_filter: FilterMode,
    /// Search input widget
    search_input: SearchInput,
    /// Available quick filters
    quick_filters: Vec<FilterMode>,
    /// Current quick filter index
    quick_filter_index: usize,
}

impl Default for FilterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterManager {
    /// Create a new filter manager
    pub fn new() -> Self {
        let quick_filters = vec![FilterMode::All, FilterMode::Running, FilterMode::Stopped];

        Self {
            current_filter: FilterMode::All,
            search_input: SearchInput::new(),
            quick_filters,
            quick_filter_index: 0,
        }
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: Key) -> bool {
        // If search is active, handle search input first
        if self.search_input.is_active() {
            if self.search_input.handle_key(key) {
                // Update filter based on search query when search is completed
                if !self.search_input.is_active() && !self.search_input.get_query().is_empty() {
                    self.apply_search_filter();
                } else if self.search_input.get_query().is_empty() {
                    self.current_filter = FilterMode::All;
                }
                return true;
            }
            return false;
        }

        // Handle filter manager keys
        match key {
            Key::Char('/') => {
                self.search_input.activate();
                true
            }
            Key::Char('f') => {
                self.cycle_quick_filter();
                true
            }
            Key::Char('c') => {
                self.clear_filters();
                true
            }
            _ => false,
        }
    }

    /// Apply search filter based on current query and mode
    fn apply_search_filter(&mut self) {
        let query = self.search_input.get_query().to_lowercase();
        if query.is_empty() {
            self.current_filter = FilterMode::All;
            return;
        }

        match self.search_input.get_search_mode() {
            SearchMode::Name => {
                self.current_filter = FilterMode::ByName(query);
            }
            SearchMode::Image => {
                self.current_filter = FilterMode::ByImage(query);
            }
            SearchMode::Status => {
                // Try to parse status
                let status = match query.as_str() {
                    "running" => ContainerState::Running,
                    "stopped" | "exited" => ContainerState::Stopped,
                    "paused" => ContainerState::Paused,
                    "restarting" => ContainerState::Restarting,
                    "dead" => ContainerState::Dead,
                    _ => ContainerState::Unknown,
                };
                self.current_filter = FilterMode::ByStatus(status);
            }
            SearchMode::All => {
                self.current_filter = FilterMode::ByName(query); // Default to name search
            }
        }
    }

    /// Cycle through quick filters
    fn cycle_quick_filter(&mut self) {
        self.quick_filter_index = (self.quick_filter_index + 1) % self.quick_filters.len();
        self.current_filter = self.quick_filters[self.quick_filter_index].clone();
        self.search_input.clear();
    }

    /// Clear all filters
    fn clear_filters(&mut self) {
        self.current_filter = FilterMode::All;
        self.quick_filter_index = 0;
        self.search_input.clear();
        self.search_input.deactivate();
    }

    /// Filter containers based on current filter mode
    pub fn filter_containers(&self, containers: &[Container]) -> Vec<Container> {
        match &self.current_filter {
            FilterMode::All => containers.to_vec(),
            FilterMode::Running => containers
                .iter()
                .filter(|c| c.state == ContainerState::Running)
                .cloned()
                .collect(),
            FilterMode::Stopped => containers
                .iter()
                .filter(|c| c.state == ContainerState::Stopped)
                .cloned()
                .collect(),
            FilterMode::ByName(name) => containers
                .iter()
                .filter(|c| c.name.to_lowercase().contains(name))
                .cloned()
                .collect(),
            FilterMode::ByImage(image) => containers
                .iter()
                .filter(|c| c.image.to_lowercase().contains(image))
                .cloned()
                .collect(),
            FilterMode::ByStatus(status) => containers
                .iter()
                .filter(|c| c.state == *status)
                .cloned()
                .collect(),
        }
    }

    /// Get current filter description
    pub fn get_filter_description(&self) -> String {
        self.current_filter.name()
    }

    /// Draw the filter controls
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Split area for search input and filter info
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input
                Constraint::Length(3), // Filter info
            ])
            .split(area);

        // Draw search input
        self.search_input.draw(frame, chunks[0]);

        // Draw filter info
        self.draw_filter_info(frame, chunks[1]);
    }

    /// Draw filter information and shortcuts
    fn draw_filter_info(&self, frame: &mut Frame, area: Rect) {
        let filter_text = format!("Active Filter: {}", self.get_filter_description());

        let filter_info = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Current: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(filter_text, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Shortcuts: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("/", Style::default().fg(Color::Yellow)),
                Span::raw(" Search | "),
                Span::styled("f", Style::default().fg(Color::Yellow)),
                Span::raw(" Quick Filter | "),
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw(" Clear"),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL).title("Filters"));

        frame.render_widget(filter_info, area);
    }

    /// Check if search input is active
    pub fn is_search_active(&self) -> bool {
        self.search_input.is_active()
    }

    /// Get suggested filters based on current containers
    pub fn get_suggestions(&self, containers: &[Container]) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Get unique images
        let mut images: Vec<String> = containers.iter().map(|c| c.image.clone()).collect();
        images.sort();
        images.dedup();

        // Get unique statuses
        let mut statuses: Vec<String> = containers
            .iter()
            .map(|c| format!("{:?}", c.state).to_lowercase())
            .collect();
        statuses.sort();
        statuses.dedup();

        // Add to suggestions
        for image in images.iter().take(5) {
            suggestions.push(format!("image:{}", image));
        }

        for status in statuses {
            suggestions.push(format!("status:{}", status));
        }

        suggestions
    }
}

/// Advanced search widget with suggestions and history
pub struct AdvancedSearch {
    /// Filter manager
    filter_manager: FilterManager,
    /// Search history
    #[allow(dead_code)]
    search_history: Vec<String>,
    /// Current suggestions
    suggestions: Vec<String>,
    /// Whether to show suggestions
    show_suggestions: bool,
    /// Selected suggestion index
    suggestion_index: Option<usize>,
}

impl Default for AdvancedSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedSearch {
    /// Create new advanced search
    pub fn new() -> Self {
        Self {
            filter_manager: FilterManager::new(),
            search_history: Vec::new(),
            suggestions: Vec::new(),
            show_suggestions: false,
            suggestion_index: None,
        }
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: Key) -> bool {
        // Handle suggestion navigation
        if self.show_suggestions && !self.suggestions.is_empty() {
            match key {
                Key::Up => {
                    self.suggestion_index = match self.suggestion_index {
                        None => Some(self.suggestions.len() - 1),
                        Some(0) => Some(self.suggestions.len() - 1),
                        Some(i) => Some(i - 1),
                    };
                    return true;
                }
                Key::Down => {
                    self.suggestion_index = match self.suggestion_index {
                        None => Some(0),
                        Some(i) if i >= self.suggestions.len() - 1 => Some(0),
                        Some(i) => Some(i + 1),
                    };
                    return true;
                }
                Key::Enter => {
                    if let Some(index) = self.suggestion_index {
                        if let Some(suggestion) = self.suggestions.get(index) {
                            // Apply the suggestion
                            let suggestion_clone = suggestion.clone();
                            self.apply_suggestion(&suggestion_clone);
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }

        // Handle filter manager events
        let handled = self.filter_manager.handle_key(key);

        // Update suggestions when search becomes active
        if self.filter_manager.is_search_active() {
            self.show_suggestions = true;
        } else {
            self.show_suggestions = false;
            self.suggestion_index = None;
        }

        handled
    }

    /// Apply a suggestion to the search
    fn apply_suggestion(&mut self, suggestion: &str) {
        // Parse suggestion and apply appropriate filter
        if let Some(colon_pos) = suggestion.find(':') {
            let (prefix, value) = suggestion.split_at(colon_pos);
            let value = &value[1..]; // Remove the colon

            match prefix {
                "image" => {
                    self.filter_manager.current_filter = FilterMode::ByImage(value.to_string());
                }
                "status" => {
                    let status = match value {
                        "running" => ContainerState::Running,
                        "stopped" => ContainerState::Stopped,
                        "paused" => ContainerState::Paused,
                        "restarting" => ContainerState::Restarting,
                        "dead" => ContainerState::Dead,
                        _ => ContainerState::Unknown,
                    };
                    self.filter_manager.current_filter = FilterMode::ByStatus(status);
                }
                _ => {
                    self.filter_manager.current_filter = FilterMode::ByName(value.to_string());
                }
            }
        }

        self.show_suggestions = false;
        self.suggestion_index = None;
    }

    /// Update suggestions based on containers
    pub fn update_suggestions(&mut self, containers: &[Container]) {
        self.suggestions = self.filter_manager.get_suggestions(containers);
    }

    /// Filter containers
    pub fn filter_containers(&self, containers: &[Container]) -> Vec<Container> {
        self.filter_manager.filter_containers(containers)
    }

    /// Get filter description
    pub fn get_filter_description(&self) -> String {
        self.filter_manager.get_filter_description()
    }

    /// Draw the advanced search interface
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        if self.show_suggestions && !self.suggestions.is_empty() {
            // Split area for filter and suggestions
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Filter controls
                    Constraint::Min(0),    // Suggestions
                ])
                .split(area);

            // Draw filter controls
            self.filter_manager.draw(frame, chunks[0]);

            // Draw suggestions
            self.draw_suggestions(frame, chunks[1]);
        } else {
            // Just draw filter controls
            self.filter_manager.draw(frame, area);
        }
    }

    /// Draw suggestions list
    fn draw_suggestions(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if Some(i) == self.suggestion_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(suggestion.clone())).style(style)
            })
            .collect();

        let suggestions_list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Suggestions (↑/↓ to navigate, Enter to select)"),
        );

        frame.render_widget(suggestions_list, area);
    }

    /// Check if search is active
    pub fn is_search_active(&self) -> bool {
        self.filter_manager.is_search_active() || self.show_suggestions
    }

    /// Clear all filters and search
    pub fn clear_all(&mut self) {
        self.filter_manager.clear_filters();
        self.show_suggestions = false;
        self.suggestion_index = None;
    }
}
