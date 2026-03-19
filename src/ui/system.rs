use anyhow::Result;
use byte_unit::{Byte, UnitType};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::{
    docker::{
        system::{DiskUsage, DockerEvent, SystemInfo},
        DockerClient,
    },
    events::Key,
    theme::Theme,
};

/// Sub-views within the System tab
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemViewMode {
    Info,
    DiskUsage,
    Events,
}

impl SystemViewMode {
    pub fn name(&self) -> &'static str {
        match self {
            SystemViewMode::Info => "System Info",
            SystemViewMode::DiskUsage => "Disk Usage",
            SystemViewMode::Events => "Events",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SystemViewMode::Info => SystemViewMode::DiskUsage,
            SystemViewMode::DiskUsage => SystemViewMode::Events,
            SystemViewMode::Events => SystemViewMode::Info,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            SystemViewMode::Info => SystemViewMode::Events,
            SystemViewMode::DiskUsage => SystemViewMode::Info,
            SystemViewMode::Events => SystemViewMode::DiskUsage,
        }
    }
}

/// System dashboard tab
pub struct SystemTab {
    docker_client: DockerClient,
    view_mode: SystemViewMode,
    system_info: Option<SystemInfo>,
    disk_usage: Option<DiskUsage>,
    events: VecDeque<DockerEvent>,
    event_receiver: Option<mpsc::UnboundedReceiver<DockerEvent>>,
    event_handle: Option<tokio::task::JoinHandle<()>>,
    scroll_offset: u16,
    #[allow(dead_code)]
    status_message: Option<String>,
    #[allow(dead_code)]
    theme: Theme,
}

impl SystemTab {
    pub async fn new(docker_client: DockerClient, theme: Theme) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            view_mode: SystemViewMode::Info,
            system_info: None,
            disk_usage: None,
            events: VecDeque::with_capacity(500),
            event_receiver: None,
            event_handle: None,
            scroll_offset: 0,
            status_message: None,
            theme,
        };

        tab.load_info().await?;
        tab.start_event_stream();

        Ok(tab)
    }

    async fn load_info(&mut self) -> Result<()> {
        match self.docker_client.detailed_system_info().await {
            Ok(info) => self.system_info = Some(info),
            Err(e) => {
                self.status_message = Some(format!("Failed to load system info: {}", e));
            }
        }
        match self.docker_client.disk_usage().await {
            Ok(usage) => self.disk_usage = Some(usage),
            Err(e) => {
                self.status_message = Some(format!("Failed to load disk usage: {}", e));
            }
        }
        Ok(())
    }

    fn start_event_stream(&mut self) {
        self.stop_event_stream();
        let (receiver, handle) = self.docker_client.stream_events();
        self.event_receiver = Some(receiver);
        self.event_handle = Some(handle);
    }

    fn stop_event_stream(&mut self) {
        if let Some(handle) = self.event_handle.take() {
            handle.abort();
        }
        self.event_receiver = None;
    }

    pub async fn refresh(&mut self) -> Result<()> {
        // Drain events from channel
        if let Some(receiver) = &mut self.event_receiver {
            while let Ok(event) = receiver.try_recv() {
                self.events.push_front(event);
                if self.events.len() > 500 {
                    self.events.pop_back();
                }
            }
        }
        Ok(())
    }

    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Left => {
                self.view_mode = self.view_mode.previous();
                self.scroll_offset = 0;
            }
            Key::Right => {
                self.view_mode = self.view_mode.next();
                self.scroll_offset = 0;
            }
            Key::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            Key::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            Key::Restart => {
                // 'r' to refresh info
                self.load_info().await?;
                self.status_message = Some("Refreshed system info".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // View mode tabs
                Constraint::Min(0),    // Content
            ])
            .split(area);

        self.draw_view_tabs(frame, chunks[0]);

        match self.view_mode {
            SystemViewMode::Info => self.draw_info(frame, chunks[1]),
            SystemViewMode::DiskUsage => self.draw_disk_usage(frame, chunks[1]),
            SystemViewMode::Events => self.draw_events(frame, chunks[1]),
        }
    }

    fn draw_view_tabs(&self, frame: &mut Frame, area: Rect) {
        let modes = [
            SystemViewMode::Info,
            SystemViewMode::DiskUsage,
            SystemViewMode::Events,
        ];

        let tabs: Vec<Span> = modes
            .iter()
            .flat_map(|mode| {
                let style = if *mode == self.view_mode {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                vec![
                    Span::styled(format!(" {} ", mode.name()), style),
                    Span::styled(" | ", Style::default().fg(Color::DarkGray)),
                ]
            })
            .collect();

        let event_count = self.events.len();
        let title = if event_count > 0 {
            format!(" System Dashboard ({} events) ", event_count)
        } else {
            " System Dashboard ".to_string()
        };

        let paragraph = Paragraph::new(Line::from(tabs)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(paragraph, area);
    }

    fn draw_info(&self, frame: &mut Frame, area: Rect) {
        let info = match &self.system_info {
            Some(info) => info,
            None => {
                let p = Paragraph::new("Loading system info...")
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(p, area);
                return;
            }
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Docker info
                Constraint::Length(5),  // Container gauges
                Constraint::Min(0),    // System info
            ])
            .split(area);

        // Docker info section
        let info_lines = vec![
            kv_line("Docker Version", &info.docker_version),
            kv_line("API Version", &info.api_version),
            kv_line("Server Name", &info.server_name),
            kv_line("Operating System", &info.os),
            kv_line("Architecture", &info.arch),
            kv_line("Kernel Version", &info.kernel_version),
            kv_line("Storage Driver", &info.storage_driver),
            kv_line(
                "Total Memory",
                &format_bytes(info.total_memory_bytes as u64),
            ),
        ];

        let info_paragraph = Paragraph::new(info_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Docker Engine ")
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(info_paragraph, chunks[0]);

        // Container and image counts with gauges
        let gauge_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chunks[1]);

        // Container status gauge
        let total = info.total_containers.max(1) as f64;
        let running_ratio = info.running_containers as f64 / total;
        let container_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " Containers: {} running / {} stopped / {} total ",
                        info.running_containers, info.stopped_containers, info.total_containers
                    )),
            )
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .ratio(running_ratio.min(1.0))
            .label(format!(
                "{:.0}% running",
                running_ratio * 100.0
            ));
        frame.render_widget(container_gauge, gauge_chunks[0]);

        // Images gauge
        let images_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " Images: {} | CPUs: {} ",
                        info.total_images, info.cpus
                    )),
            )
            .gauge_style(Style::default().fg(Color::Blue).bg(Color::DarkGray))
            .ratio(0.0)
            .label(format!("{} images available", info.total_images));
        frame.render_widget(images_gauge, gauge_chunks[1]);

        // Help
        let help = Paragraph::new(Line::from(Span::styled(
            "Left/Right: switch views | r: refresh | Up/Down: scroll",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(help, chunks[2]);
    }

    fn draw_disk_usage(&self, frame: &mut Frame, area: Rect) {
        let usage = match &self.disk_usage {
            Some(u) => u,
            None => {
                let p = Paragraph::new("Loading disk usage...")
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(p, area);
                return;
            }
        };

        let total = usage.total_size.max(1) as f64;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Images
                Constraint::Length(3), // Containers
                Constraint::Length(3), // Volumes
                Constraint::Length(3), // Build cache
                Constraint::Length(3), // Total
                Constraint::Min(0),    // Summary
            ])
            .split(area);

        // Images gauge
        let images_ratio = usage.images_size as f64 / total;
        let images_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(
                " Images ({}) ",
                usage.images_count
            )))
            .gauge_style(Style::default().fg(Color::Blue).bg(Color::DarkGray))
            .ratio(images_ratio.min(1.0))
            .label(format_bytes(usage.images_size));
        frame.render_widget(images_gauge, chunks[0]);

        // Containers gauge
        let containers_ratio = usage.containers_size as f64 / total;
        let containers_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(
                " Containers ({}) ",
                usage.containers_count
            )))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .ratio(containers_ratio.min(1.0))
            .label(format_bytes(usage.containers_size));
        frame.render_widget(containers_gauge, chunks[1]);

        // Volumes gauge
        let volumes_ratio = usage.volumes_size as f64 / total;
        let volumes_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(
                " Volumes ({}) ",
                usage.volumes_count
            )))
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .ratio(volumes_ratio.min(1.0))
            .label(format_bytes(usage.volumes_size));
        frame.render_widget(volumes_gauge, chunks[2]);

        // Build cache gauge
        let cache_ratio = usage.build_cache_size as f64 / total;
        let cache_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(
                " Build Cache ({}) ",
                usage.build_cache_count
            )))
            .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
            .ratio(cache_ratio.min(1.0))
            .label(format_bytes(usage.build_cache_size));
        frame.render_widget(cache_gauge, chunks[3]);

        // Total
        let total_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Total Disk Usage ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(1.0)
            .label(format_bytes(usage.total_size));
        frame.render_widget(total_gauge, chunks[4]);

        // Summary
        let summary = Paragraph::new(Line::from(Span::styled(
            "r: refresh disk usage",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(summary, chunks[5]);
    }

    fn draw_events(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        if self.events.is_empty() {
            lines.push(Line::from(Span::styled(
                "Waiting for Docker events... (start/stop containers to see events)",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else {
            for event in &self.events {
                let type_color = match event.event_type.as_str() {
                    "container" => Color::Green,
                    "image" => Color::Blue,
                    "volume" => Color::Yellow,
                    "network" => Color::Cyan,
                    _ => Color::White,
                };

                let action_color = if event.action.contains("destroy")
                    || event.action.contains("delete")
                    || event.action.contains("kill")
                    || event.action.contains("die")
                {
                    Color::Red
                } else if event.action.contains("start")
                    || event.action.contains("create")
                {
                    Color::Green
                } else {
                    Color::White
                };

                let mut spans = vec![
                    Span::styled(
                        format!("[{}] ", event.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:>10} ", event.event_type),
                        Style::default().fg(type_color),
                    ),
                    Span::styled(
                        format!("{:<12} ", event.action),
                        Style::default()
                            .fg(action_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];

                if !event.actor_name.is_empty() {
                    spans.push(Span::styled(
                        event.actor_name.clone(),
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::raw(" "));
                }

                if !event.actor_id.is_empty() {
                    spans.push(Span::styled(
                        format!("({})", event.actor_id),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                lines.push(Line::from(spans));
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Events ({}) ", self.events.len()))
                    .border_style(Style::default().fg(Color::Green)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }
}

impl Drop for SystemTab {
    fn drop(&mut self) {
        self.stop_event_stream();
    }
}

fn kv_line(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {}: ", key),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let byte = Byte::from_u64(bytes).get_appropriate_unit(UnitType::Binary);
    format!("{:.1}", byte)
}
