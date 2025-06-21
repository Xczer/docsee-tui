use anyhow::Result;
use byte_unit::Byte;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, Gauge, List, ListItem, Paragraph},
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
    theme::gauge_gradient_color,
};

/// keep last 60 points, roughly a minute of history at 1s interval
const MAX_CHART_POINTS: usize = 60;

/// Container resource statistics
#[derive(Debug, Clone)]
pub struct ContainerStats {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub memory_usage_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u64,
    pub timestamp: f64,
}

impl Default for ContainerStats {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
            memory_limit_bytes: 0,
            memory_usage_percent: 0.0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
            pids: 0,
            timestamp: 0.0,
        }
    }
}

/// one point on the chart
#[derive(Debug, Clone)]
struct DataPoint {
    #[allow(dead_code)]
    timestamp: f64,
    value: f64,
}

/// Container statistics viewer
pub struct StatsViewer {
    docker_client: DockerClient,
    container: Option<Container>,
    current_stats: ContainerStats,
    cpu_history: VecDeque<DataPoint>,
    memory_history: VecDeque<DataPoint>,
    network_rx_history: VecDeque<DataPoint>,
    network_tx_history: VecDeque<DataPoint>,
    stats_handle: Option<tokio::task::JoinHandle<()>>,
    stats_receiver: Option<mpsc::UnboundedReceiver<ContainerStats>>,
    status_message: Option<String>,
    view_mode: StatsViewMode,
    // in seconds, user can bump with +/-
    update_interval: u64,
    is_streaming: bool,
}

/// Different view modes for stats
#[derive(Debug, Clone, PartialEq)]
pub enum StatsViewMode {
    Overview,
    Charts,
    Network,
    Processes,
}

impl StatsViewMode {
    pub fn name(&self) -> &'static str {
        match self {
            StatsViewMode::Overview => "Overview",
            StatsViewMode::Charts => "Charts",
            StatsViewMode::Network => "Network",
            StatsViewMode::Processes => "Processes",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            StatsViewMode::Overview => StatsViewMode::Charts,
            StatsViewMode::Charts => StatsViewMode::Network,
            StatsViewMode::Network => StatsViewMode::Processes,
            StatsViewMode::Processes => StatsViewMode::Overview,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            StatsViewMode::Overview => StatsViewMode::Processes,
            StatsViewMode::Charts => StatsViewMode::Overview,
            StatsViewMode::Network => StatsViewMode::Charts,
            StatsViewMode::Processes => StatsViewMode::Network,
        }
    }
}

impl StatsViewer {
    /// Create a new stats viewer
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            container: None,
            current_stats: ContainerStats::default(),
            cpu_history: VecDeque::with_capacity(MAX_CHART_POINTS),
            memory_history: VecDeque::with_capacity(MAX_CHART_POINTS),
            network_rx_history: VecDeque::with_capacity(MAX_CHART_POINTS),
            network_tx_history: VecDeque::with_capacity(MAX_CHART_POINTS),
            stats_handle: None,
            stats_receiver: None,
            status_message: None,
            view_mode: StatsViewMode::Overview,
            update_interval: 1,
            is_streaming: false,
        }
    }

    /// Start monitoring stats for a container
    pub async fn start_monitoring(&mut self, container: Container) -> Result<()> {
        // kill old stream first, warna do streams chalte rehte hai
        self.stop_monitoring().await;

        self.container = Some(container.clone());
        self.current_stats = ContainerStats::default();
        self.clear_history();

        let (stats_sender, stats_receiver) = mpsc::unbounded_channel();
        self.stats_receiver = Some(stats_receiver);

        let docker_client = self.docker_client.clone();
        let container_id = container.id.clone();
        let container_name = container.name.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::stream_stats(docker_client, &container_id, stats_sender).await {
                eprintln!("Error streaming stats for {}: {}", container_name, e);
            }
        });

        self.stats_handle = Some(handle);
        self.is_streaming = true;
        self.status_message = Some(format!("Monitoring stats for '{}'...", container.name));

        Ok(())
    }

    /// Stop stats monitoring
    pub async fn stop_monitoring(&mut self) {
        if let Some(handle) = self.stats_handle.take() {
            handle.abort();
        }
        self.stats_receiver = None;
        self.is_streaming = false;
        self.status_message = None;
    }

    /// call this from the main event loop
    pub async fn update(&mut self) -> Result<()> {
        let mut stats_to_update = Vec::new();
        if let Some(receiver) = &mut self.stats_receiver {
            while let Ok(stats) = receiver.try_recv() {
                stats_to_update.push(stats);
            }
        }

        // collect first, then apply - borrow checker rota hai otherwise
        for stats in stats_to_update {
            self.update_stats(stats);
        }

        Ok(())
    }

    /// Handle key events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Left => {
                self.view_mode = self.view_mode.previous();
            }
            Key::Right => {
                self.view_mode = self.view_mode.next();
            }
            Key::Char('r') => {
                self.reset_stats();
            }
            Key::Char('+') => {
                if self.update_interval > 1 {
                    self.update_interval -= 1;
                    self.status_message =
                        Some(format!("Update interval: {}s", self.update_interval));
                }
            }
            Key::Char('-') => {
                if self.update_interval < 10 {
                    self.update_interval += 1;
                    self.status_message =
                        Some(format!("Update interval: {}s", self.update_interval));
                }
            }
            Key::Char('p') => {
                self.is_streaming = !self.is_streaming;
                let status = if self.is_streaming {
                    "resumed"
                } else {
                    "paused"
                };
                self.status_message = Some(format!("Monitoring {}", status));
            }
            _ => {}
        }

        Ok(())
    }

    /// Draw the stats viewer
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
            ])
            .split(area);

        self.draw_header(frame, chunks[0]);

        match self.view_mode {
            StatsViewMode::Overview => self.draw_overview(frame, chunks[1]),
            StatsViewMode::Charts => self.draw_charts(frame, chunks[1]),
            StatsViewMode::Network => self.draw_network(frame, chunks[1]),
            StatsViewMode::Processes => self.draw_processes(frame, chunks[1]),
        }
    }

    /// Draw the header
    fn draw_header(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("No Container");

        let status = if self.is_streaming {
            "🟢 Live"
        } else {
            "🔴 Paused"
        };

        let title = format!(
            "Stats: {} | {} | View: {} | Update: {}s",
            container_name,
            status,
            self.view_mode.name(),
            self.update_interval
        );

        let title_text = if let Some(ref message) = self.status_message {
            format!("{} - {}", title, message)
        } else {
            title
        };

        let header = Paragraph::new(vec![
            Line::from(vec![Span::raw(title_text)]),
            Line::from(vec![
                Span::styled("Controls: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("←/→", Style::default().fg(Color::Yellow)),
                Span::raw(" Switch View | "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw(" Reset | "),
                Span::styled("p", Style::default().fg(Color::Yellow)),
                Span::raw(" Pause/Resume | "),
                Span::styled("+/-", Style::default().fg(Color::Yellow)),
                Span::raw(" Interval | "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" Back"),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );

        frame.render_widget(header, area);
    }

    /// Draw overview mode
    fn draw_overview(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Gauges
                Constraint::Min(0),     // Details
            ])
            .split(area);

        self.draw_gauges(frame, chunks[0]);
        self.draw_details_table(frame, chunks[1]);
    }

    /// Draw resource gauges
    fn draw_gauges(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // CPU & Memory
                Constraint::Percentage(50), // Network & Block I/O
            ])
            .split(area);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // CPU
                Constraint::Percentage(50), // Memory
            ])
            .split(chunks[0]);

        let cpu_ratio = self.current_stats.cpu_usage_percent / 100.0;
        let cpu_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("CPU Usage"),
            )
            .gauge_style(Style::default().fg(gauge_gradient_color(cpu_ratio)))
            .ratio(cpu_ratio)
            .label(format!("{:.1}%", self.current_stats.cpu_usage_percent));

        frame.render_widget(cpu_gauge, left_chunks[0]);

        let mem_ratio = self.current_stats.memory_usage_percent / 100.0;
        let memory_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Memory Usage"),
            )
            .gauge_style(Style::default().fg(gauge_gradient_color(mem_ratio)))
            .ratio(mem_ratio)
            .label(format!("{:.1}%", self.current_stats.memory_usage_percent));

        frame.render_widget(memory_gauge, left_chunks[1]);

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Network RX
                Constraint::Percentage(50), // Network TX
            ])
            .split(chunks[1]);

        let rx_bytes = Byte::from_u64(self.current_stats.network_rx_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Decimal);
        let network_rx = Paragraph::new(format!("RX: {:.2}", rx_bytes))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Network Received"),
            )
            .style(Style::default().fg(Color::Cyan));

        frame.render_widget(network_rx, right_chunks[0]);

        let tx_bytes = Byte::from_u64(self.current_stats.network_tx_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Decimal);
        let network_tx = Paragraph::new(format!("TX: {:.2}", tx_bytes))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Network Transmitted"),
            )
            .style(Style::default().fg(Color::Magenta));

        frame.render_widget(network_tx, right_chunks[1]);
    }

    /// Draw details table
    fn draw_details_table(&mut self, frame: &mut Frame, area: Rect) {
        let memory_used = Byte::from_u64(self.current_stats.memory_usage_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Binary);
        let memory_limit = Byte::from_u64(self.current_stats.memory_limit_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Binary);
        let block_read = Byte::from_u64(self.current_stats.block_read_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Decimal);
        let block_write = Byte::from_u64(self.current_stats.block_write_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Decimal);

        let info_text = format!(
            "CPU Usage: {:.2}%\nMemory Used: {:.2}\nMemory Limit: {:.2}\nMemory Percent: {:.2}%\nBlock Read: {:.2}\nBlock Write: {:.2}\nPIDs: {}",
            self.current_stats.cpu_usage_percent,
            memory_used,
            memory_limit,
            self.current_stats.memory_usage_percent,
            block_read,
            block_write,
            self.current_stats.pids
        );

        let details = Paragraph::new(info_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Resource Details"),
        );

        frame.render_widget(details, area);
    }

    /// Draw charts mode
    fn draw_charts(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // CPU chart
                Constraint::Percentage(50), // Memory chart
            ])
            .split(area);

        self.draw_cpu_chart(frame, chunks[0]);
        self.draw_memory_chart(frame, chunks[1]);
    }

    /// Draw CPU usage chart
    fn draw_cpu_chart(&mut self, frame: &mut Frame, area: Rect) {
        if self.cpu_history.is_empty() {
            let placeholder = Paragraph::new("No CPU data available yet...").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("CPU Usage Over Time"),
            );
            frame.render_widget(placeholder, area);
            return;
        }

        let data: Vec<(f64, f64)> = self
            .cpu_history
            .iter()
            .enumerate()
            .map(|(i, point)| (i as f64, point.value))
            .collect();

        let max_cpu = data.iter().map(|(_, v)| *v).fold(0.0, f64::max).max(100.0);

        let dataset = Dataset::default()
            .name("CPU %")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Green))
            .data(&data);

        let chart = Chart::new(vec![dataset])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("CPU Usage Over Time"),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .bounds([0.0, MAX_CHART_POINTS as f64]),
            )
            .y_axis(Axis::default().title("CPU %").bounds([0.0, max_cpu]));

        frame.render_widget(chart, area);
    }

    /// Draw memory usage chart
    fn draw_memory_chart(&mut self, frame: &mut Frame, area: Rect) {
        if self.memory_history.is_empty() {
            let placeholder = Paragraph::new("No memory data available yet...").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Memory Usage Over Time"),
            );
            frame.render_widget(placeholder, area);
            return;
        }

        let data: Vec<(f64, f64)> = self
            .memory_history
            .iter()
            .enumerate()
            .map(|(i, point)| (i as f64, point.value))
            .collect();

        let max_memory = data.iter().map(|(_, v)| *v).fold(0.0, f64::max).max(100.0);

        let dataset = Dataset::default()
            .name("Memory %")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Blue))
            .data(&data);

        let chart = Chart::new(vec![dataset])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Memory Usage Over Time"),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .bounds([0.0, MAX_CHART_POINTS as f64]),
            )
            .y_axis(Axis::default().title("Memory %").bounds([0.0, max_memory]));

        frame.render_widget(chart, area);
    }

    /// Draw network mode
    fn draw_network(&mut self, frame: &mut Frame, area: Rect) {
        let rx_bytes = Byte::from_u64(self.current_stats.network_rx_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Decimal);
        let tx_bytes = Byte::from_u64(self.current_stats.network_tx_bytes)
            .get_appropriate_unit(byte_unit::UnitType::Decimal);

        let items = vec![
            ListItem::new(Line::from(vec![
                Span::styled(
                    "Network RX: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:.2}", rx_bytes), Style::default().fg(Color::Cyan)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled(
                    "Network TX: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:.2}", tx_bytes),
                    Style::default().fg(Color::Magenta),
                ),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled(
                    "Total Transfer: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "{:.2}",
                    Byte::from_u64(
                        self.current_stats.network_rx_bytes + self.current_stats.network_tx_bytes
                    )
                    .get_appropriate_unit(byte_unit::UnitType::Decimal)
                )),
            ])),
        ];

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Network Summary"),
        );

        frame.render_widget(list, area);
    }

    /// Draw processes mode
    fn draw_processes(&mut self, frame: &mut Frame, area: Rect) {
        let info = vec![
            ListItem::new(Line::from(vec![
                Span::styled("PIDs: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.current_stats.pids.to_string()),
            ])),
            ListItem::new(Line::from(vec![Span::raw(
                "Process listing requires additional Docker permissions.",
            )])),
            ListItem::new(Line::from(vec![Span::raw(
                "Use 'docker exec -it <container> ps aux' for detailed process info.",
            )])),
        ];

        let list = List::new(info).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Process Information"),
        );

        frame.render_widget(list, area);
    }

    fn update_stats(&mut self, stats: ContainerStats) {
        self.current_stats = stats.clone();
        self.add_to_history(&stats);
    }

    /// push latest stats into the ring buffers, dropping oldest if full
    fn add_to_history(&mut self, stats: &ContainerStats) {
        let timestamp = stats.timestamp;

        if self.cpu_history.len() >= MAX_CHART_POINTS {
            self.cpu_history.pop_front();
        }
        self.cpu_history.push_back(DataPoint {
            timestamp,
            value: stats.cpu_usage_percent,
        });

        if self.memory_history.len() >= MAX_CHART_POINTS {
            self.memory_history.pop_front();
        }
        self.memory_history.push_back(DataPoint {
            timestamp,
            value: stats.memory_usage_percent,
        });

        if self.network_rx_history.len() >= MAX_CHART_POINTS {
            self.network_rx_history.pop_front();
        }
        self.network_rx_history.push_back(DataPoint {
            timestamp,
            value: stats.network_rx_bytes as f64,
        });

        if self.network_tx_history.len() >= MAX_CHART_POINTS {
            self.network_tx_history.pop_front();
        }
        self.network_tx_history.push_back(DataPoint {
            timestamp,
            value: stats.network_tx_bytes as f64,
        });
    }

    fn clear_history(&mut self) {
        self.cpu_history.clear();
        self.memory_history.clear();
        self.network_rx_history.clear();
        self.network_tx_history.clear();
    }

    fn reset_stats(&mut self) {
        self.current_stats = ContainerStats::default();
        self.clear_history();
        self.status_message = Some("Stats reset".to_string());
    }

    /// Stream stats from Docker
    async fn stream_stats(
        docker_client: DockerClient,
        container_id: &str,
        sender: mpsc::UnboundedSender<ContainerStats>,
    ) -> Result<()> {
        use bollard::container::StatsOptions;

        let options = Some(StatsOptions {
            stream: true,
            one_shot: false,
        });

        let mut stream = docker_client.inner().stats(container_id, options);

        while let Some(stats_result) = futures::stream::TryStreamExt::try_next(&mut stream).await? {
            let stats = Self::parse_docker_stats(stats_result)?;

            if sender.send(stats).is_err() {
                // receiver gaya, ab stream karke kya fayda
                break;
            }
        }

        Ok(())
    }

    /// turn bollard's raw stats into our own struct
    fn parse_docker_stats(docker_stats: bollard::container::Stats) -> Result<ContainerStats> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        // same formula docker cli uses - delta over system delta times ncpus
        let cpu_usage_percent = {
            let cpu_delta = docker_stats.cpu_stats.cpu_usage.total_usage as f64
                - docker_stats.precpu_stats.cpu_usage.total_usage as f64;
            let system_delta = docker_stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
                - docker_stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;

            if system_delta > 0.0 && cpu_delta > 0.0 {
                let num_cpus = docker_stats
                    .cpu_stats
                    .cpu_usage
                    .percpu_usage
                    .as_ref()
                    .map(|v| v.len() as f64)
                    .unwrap_or(1.0);
                (cpu_delta / system_delta) * num_cpus * 100.0
            } else {
                0.0
            }
        };

        let memory_stats = &docker_stats.memory_stats;
        let usage = memory_stats.usage.unwrap_or(0);
        // limit default 1 not 0, warna divide by zero ho jayega neeche
        let limit = memory_stats.limit.unwrap_or(1);
        let memory_usage_percent = (usage as f64 / limit as f64) * 100.0;
        let (memory_usage_bytes, memory_limit_bytes) = (usage, limit);

        // sum rx/tx across all the container's networks
        let (network_rx_bytes, network_tx_bytes) = if let Some(networks) = &docker_stats.networks {
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;

            for network in networks.values() {
                total_rx += network.rx_bytes;
                total_tx += network.tx_bytes;
            }

            (total_rx, total_tx)
        } else {
            (0, 0)
        };

        let blkio_stats = &docker_stats.blkio_stats;
        let mut total_read = 0u64;
        let mut total_write = 0u64;

        if let Some(io_service_bytes_recursive) = &blkio_stats.io_service_bytes_recursive {
            for stat in io_service_bytes_recursive {
                // bollard gives op as plain String here, so as_str() not as_deref()
                match stat.op.as_str() {
                    "Read" => total_read += stat.value,
                    "Write" => total_write += stat.value,
                    _ => {}
                }
            }
        }

        let (block_read_bytes, block_write_bytes) = (total_read, total_write);

        let pids = docker_stats.pids_stats.current.unwrap_or(0);

        Ok(ContainerStats {
            cpu_usage_percent,
            memory_usage_bytes,
            memory_limit_bytes,
            memory_usage_percent,
            network_rx_bytes,
            network_tx_bytes,
            block_read_bytes,
            block_write_bytes,
            pids,
            timestamp,
        })
    }

    pub fn get_container(&self) -> Option<&Container> {
        self.container.as_ref()
    }

    /// streaming AND a container attached
    pub fn is_monitoring(&self) -> bool {
        self.is_streaming && self.container.is_some()
    }

    pub fn get_current_stats(&self) -> &ContainerStats {
        &self.current_stats
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
