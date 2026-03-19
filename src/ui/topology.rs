use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::collections::{BTreeMap, HashSet};

use crate::docker::DockerClient;

/// A container's network membership info
#[derive(Debug, Clone)]
struct ContainerNetInfo {
    name: String,
    id: String,
    ip_address: String,
    network_count: usize,
}

/// A network with its connected containers
#[derive(Debug, Clone)]
struct NetworkNode {
    name: String,
    driver: String,
    subnet: String,
    containers: Vec<ContainerNetInfo>,
}

/// Container network topology view
pub struct TopologyViewer {
    docker_client: DockerClient,
    networks: Vec<NetworkNode>,
    scroll_offset: u16,
    focused_container: Option<String>,
    multi_network_containers: HashSet<String>,
}

impl TopologyViewer {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            networks: Vec::new(),
            scroll_offset: 0,
            focused_container: None,
            multi_network_containers: HashSet::new(),
        }
    }

    pub async fn load(&mut self) -> Result<()> {
        let raw_networks = self
            .docker_client
            .inner()
            .list_networks::<String>(None)
            .await?;

        // Count how many networks each container belongs to
        let mut container_net_counts: BTreeMap<String, usize> = BTreeMap::new();

        for net in &raw_networks {
            if let Some(ref containers) = net.containers {
                for id in containers.keys() {
                    *container_net_counts.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        self.multi_network_containers = container_net_counts
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(id, _)| id.clone())
            .collect();

        let mut nodes = Vec::new();

        for net in raw_networks {
            let name = net.name.unwrap_or_default();
            let driver = net.driver.unwrap_or_default();

            let subnet = net
                .ipam
                .as_ref()
                .and_then(|ipam| ipam.config.as_ref())
                .and_then(|configs| configs.first())
                .and_then(|c| c.subnet.clone())
                .unwrap_or_default();

            let mut containers = Vec::new();
            if let Some(ref cont_map) = net.containers {
                for (id, endpoint) in cont_map {
                    let cname = endpoint
                        .name
                        .as_deref()
                        .unwrap_or("unknown")
                        .to_string();
                    let ip = endpoint
                        .ipv4_address
                        .as_deref()
                        .unwrap_or("")
                        .to_string();
                    let net_count = container_net_counts.get(id).copied().unwrap_or(1);

                    containers.push(ContainerNetInfo {
                        name: cname,
                        id: id[..12.min(id.len())].to_string(),
                        ip_address: ip,
                        network_count: net_count,
                    });
                }
            }

            containers.sort_by(|a, b| a.name.cmp(&b.name));

            nodes.push(NetworkNode {
                name,
                driver,
                subnet,
                containers,
            });
        }

        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        self.networks = nodes;
        self.scroll_offset = 0;
        self.focused_container = None;

        Ok(())
    }

    pub async fn handle_key(&mut self, key: crate::events::Key) -> Result<()> {
        use crate::events::Key;
        match key {
            Key::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            Key::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            Key::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            Key::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
            }
            Key::Home => {
                self.scroll_offset = 0;
            }
            Key::Enter => {
                // Toggle focus off
                self.focused_container = None;
            }
            Key::Restart => {
                // 'r' to refresh
                self.load().await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Topology
            ])
            .split(area);

        self.draw_header(frame, chunks[0]);
        self.draw_topology(frame, chunks[1]);
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let total_nets = self.networks.len();
        let total_containers: usize = self.networks.iter().map(|n| n.containers.len()).sum();
        let multi_net = self.multi_network_containers.len();

        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} Networks ", total_nets),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} Connections ", total_containers),
                Style::default().fg(Color::Green),
            ),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} Multi-network containers ", multi_net),
                Style::default().fg(Color::Yellow),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Network Topology ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(header, area);
    }

    fn draw_topology(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        for network in &self.networks {
            // Network header line
            let driver_color = match network.driver.as_str() {
                "bridge" => Color::Blue,
                "host" => Color::Magenta,
                "overlay" => Color::Green,
                _ => Color::White,
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}]", network.driver),
                    Style::default().fg(driver_color),
                ),
                Span::styled(
                    format!(" {} ", network.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                if !network.subnet.is_empty() {
                    Span::styled(
                        format!("({})", network.subnet),
                        Style::default().fg(Color::DarkGray),
                    )
                } else {
                    Span::raw("")
                },
            ]));

            if network.containers.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    (no containers)",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )));
            } else {
                for (i, container) in network.containers.iter().enumerate() {
                    let is_last = i == network.containers.len() - 1;
                    let connector = if is_last { "    +-- " } else { "    |-- " };

                    let name_style = if container.network_count > 1 {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let mut spans = vec![
                        Span::styled(connector.to_string(), Style::default().fg(Color::DarkGray)),
                        Span::styled(container.name.clone(), name_style),
                    ];

                    if !container.ip_address.is_empty() {
                        spans.push(Span::styled(
                            format!(" [{}]", container.ip_address),
                            Style::default().fg(Color::Green),
                        ));
                    }

                    spans.push(Span::styled(
                        format!(" ({})", container.id),
                        Style::default().fg(Color::DarkGray),
                    ));

                    if container.network_count > 1 {
                        spans.push(Span::styled(
                            format!(" *{} nets*", container.network_count),
                            Style::default().fg(Color::Yellow),
                        ));
                    }

                    lines.push(Line::from(spans));
                }
            }

            lines.push(Line::from(""));
        }

        // Help line at bottom
        lines.push(Line::from(Span::styled(
            "Up/Down: scroll | r: refresh | Esc: back",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }
}
