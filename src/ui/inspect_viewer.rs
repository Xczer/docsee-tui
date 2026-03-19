use anyhow::Result;
use bollard::service::ContainerInspectResponse;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

/// Sections available in the inspect view
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InspectSection {
    General,
    Environment,
    Mounts,
    Network,
    Config,
}

impl InspectSection {
    pub fn all() -> &'static [InspectSection] {
        &[
            InspectSection::General,
            InspectSection::Environment,
            InspectSection::Mounts,
            InspectSection::Network,
            InspectSection::Config,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            InspectSection::General => "General",
            InspectSection::Environment => "Environment",
            InspectSection::Mounts => "Mounts",
            InspectSection::Network => "Network",
            InspectSection::Config => "Config",
        }
    }

    pub fn next(&self) -> InspectSection {
        let all = Self::all();
        let idx = all.iter().position(|s| s == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    pub fn previous(&self) -> InspectSection {
        let all = Self::all();
        let idx = all.iter().position(|s| s == self).unwrap_or(0);
        if idx == 0 {
            all[all.len() - 1]
        } else {
            all[idx - 1]
        }
    }
}

/// Container inspect detail viewer
pub struct InspectViewer {
    docker_client: DockerClient,
    container: Option<Container>,
    inspect_data: Option<ContainerInspectResponse>,
    current_section: InspectSection,
    scroll_offset: u16,
}

impl InspectViewer {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            container: None,
            inspect_data: None,
            current_section: InspectSection::General,
            scroll_offset: 0,
        }
    }

    pub async fn inspect(&mut self, container: Container) -> Result<()> {
        let data = self.docker_client.inspect_container(&container.id).await?;
        self.inspect_data = Some(data);
        self.container = Some(container);
        self.current_section = InspectSection::General;
        self.scroll_offset = 0;
        Ok(())
    }

    pub fn get_container(&self) -> Option<&Container> {
        self.container.as_ref()
    }

    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Left => {
                self.current_section = self.current_section.previous();
                self.scroll_offset = 0;
            }
            Key::Right => {
                self.current_section = self.current_section.next();
                self.scroll_offset = 0;
            }
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
            _ => {}
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Section tabs
                Constraint::Min(0),    // Content
            ])
            .split(area);

        self.draw_section_tabs(frame, chunks[0]);
        self.draw_section_content(frame, chunks[1]);
    }

    fn draw_section_tabs(&self, frame: &mut Frame, area: Rect) {
        let tabs: Vec<Span> = InspectSection::all()
            .iter()
            .flat_map(|section| {
                let style = if *section == self.current_section {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                vec![
                    Span::styled(format!(" {} ", section.name()), style),
                    Span::styled(" | ", Style::default().fg(Color::DarkGray)),
                ]
            })
            .collect();

        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("Unknown");

        let line = Line::from(tabs);
        let paragraph = Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Inspect: {} ", container_name))
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(paragraph, area);
    }

    fn draw_section_content(&self, frame: &mut Frame, area: Rect) {
        let lines = match self.inspect_data {
            Some(ref data) => match self.current_section {
                InspectSection::General => self.build_general_lines(data),
                InspectSection::Environment => self.build_env_lines(data),
                InspectSection::Mounts => self.build_mounts_lines(data),
                InspectSection::Network => self.build_network_lines(data),
                InspectSection::Config => self.build_config_lines(data),
            },
            None => vec![Line::from("No inspect data available")],
        };

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", self.current_section.name()))
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }

    fn build_general_lines(&self, data: &ContainerInspectResponse) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(ref id) = data.id {
            lines.push(kv_line("ID", id));
        }
        if let Some(ref name) = data.name {
            lines.push(kv_line("Name", name.trim_start_matches('/')));
        }
        if let Some(ref created) = data.created {
            lines.push(kv_line("Created", created));
        }

        if let Some(ref state) = data.state {
            lines.push(Line::from(""));
            lines.push(section_header("State"));
            if let Some(ref status) = state.status {
                lines.push(kv_line("  Status", &format!("{:?}", status)));
            }
            if let Some(running) = state.running {
                lines.push(kv_line("  Running", &running.to_string()));
            }
            if let Some(ref pid) = state.pid {
                lines.push(kv_line("  PID", &pid.to_string()));
            }
            if let Some(ref started_at) = state.started_at {
                lines.push(kv_line("  Started At", started_at));
            }
            if let Some(ref finished_at) = state.finished_at {
                if !finished_at.starts_with("0001") {
                    lines.push(kv_line("  Finished At", finished_at));
                }
            }
            if let Some(exit_code) = state.exit_code {
                lines.push(kv_line("  Exit Code", &exit_code.to_string()));
            }
        }

        if let Some(ref config) = data.config {
            if let Some(ref image) = config.image {
                lines.push(Line::from(""));
                lines.push(kv_line("Image", image));
            }
        }

        if let Some(ref driver) = data.driver {
            lines.push(kv_line("Driver", driver));
        }
        if let Some(ref platform) = data.platform {
            lines.push(kv_line("Platform", platform));
        }

        lines.push(Line::from(""));
        lines.push(hint_line("Left/Right: switch sections | Up/Down: scroll | Esc: back"));

        lines
    }

    fn build_env_lines(&self, data: &ContainerInspectResponse) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(ref config) = data.config {
            if let Some(ref env) = config.env {
                if env.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No environment variables set",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for var in env {
                        if let Some((key, value)) = var.split_once('=') {
                            lines.push(Line::from(vec![
                                Span::styled(
                                    key.to_string(),
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(" = ", Style::default().fg(Color::DarkGray)),
                                Span::styled(value.to_string(), Style::default().fg(Color::White)),
                            ]));
                        } else {
                            lines.push(Line::from(Span::styled(
                                var.clone(),
                                Style::default().fg(Color::White),
                            )));
                        }
                    }
                }
            }
        }

        lines
    }

    fn build_mounts_lines(&self, data: &ContainerInspectResponse) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(ref mounts) = data.mounts {
            if mounts.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No mounts configured",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for (i, mount) in mounts.iter().enumerate() {
                    if i > 0 {
                        lines.push(Line::from(""));
                    }
                    lines.push(section_header(&format!("Mount #{}", i + 1)));

                    if let Some(ref typ) = mount.typ {
                        lines.push(kv_line("  Type", &format!("{:?}", typ)));
                    }
                    if let Some(ref source) = mount.source {
                        lines.push(kv_line("  Source", source));
                    }
                    if let Some(ref dest) = mount.destination {
                        lines.push(kv_line("  Destination", dest));
                    }
                    if let Some(ref mode) = mount.mode {
                        lines.push(kv_line("  Mode", mode));
                    }
                    if let Some(rw) = mount.rw {
                        lines.push(kv_line(
                            "  Read/Write",
                            if rw { "Yes" } else { "No" },
                        ));
                    }
                }
            }
        }

        lines
    }

    fn build_network_lines(&self, data: &ContainerInspectResponse) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(ref net_settings) = data.network_settings {
            if let Some(ref networks) = net_settings.networks {
                if networks.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No networks connected",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for (name, config) in networks {
                        lines.push(section_header(name));

                        if let Some(ref net_id) = config.network_id {
                            lines.push(kv_line("  Network ID", &net_id[..12.min(net_id.len())]));
                        }
                        if let Some(ref ip) = config.ip_address {
                            lines.push(kv_line("  IP Address", ip));
                        }
                        if let Some(ref gateway) = config.gateway {
                            lines.push(kv_line("  Gateway", gateway));
                        }
                        if let Some(ref mac) = config.mac_address {
                            lines.push(kv_line("  MAC Address", mac));
                        }
                        if let Some(prefix) = config.ip_prefix_len {
                            lines.push(kv_line("  IP Prefix Len", &prefix.to_string()));
                        }
                        lines.push(Line::from(""));
                    }
                }
            }

            if let Some(ref ports) = net_settings.ports {
                if !ports.is_empty() {
                    lines.push(section_header("Port Bindings"));
                    for (port, bindings) in ports {
                        let binding_str = if let Some(bindings) = bindings {
                            bindings
                                .iter()
                                .map(|b| {
                                    format!(
                                        "{}:{}",
                                        b.host_ip.as_deref().unwrap_or("0.0.0.0"),
                                        b.host_port.as_deref().unwrap_or("?")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        } else {
                            "none".to_string()
                        };
                        lines.push(kv_line(&format!("  {}", port), &binding_str));
                    }
                }
            }
        }

        lines
    }

    fn build_config_lines(&self, data: &ContainerInspectResponse) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(ref config) = data.config {
            if let Some(ref cmd) = config.cmd {
                lines.push(kv_line("Cmd", &cmd.join(" ")));
            }
            if let Some(ref entrypoint) = config.entrypoint {
                lines.push(kv_line("Entrypoint", &entrypoint.join(" ")));
            }
            if let Some(ref working_dir) = config.working_dir {
                if !working_dir.is_empty() {
                    lines.push(kv_line("Working Dir", working_dir));
                }
            }
            if let Some(ref user) = config.user {
                if !user.is_empty() {
                    lines.push(kv_line("User", user));
                }
            }
            if let Some(ref hostname) = config.hostname {
                lines.push(kv_line("Hostname", hostname));
            }
            if let Some(ref domainname) = config.domainname {
                if !domainname.is_empty() {
                    lines.push(kv_line("Domain Name", domainname));
                }
            }
            if let Some(tty) = config.tty {
                lines.push(kv_line("TTY", &tty.to_string()));
            }
            if let Some(open_stdin) = config.open_stdin {
                lines.push(kv_line("Open Stdin", &open_stdin.to_string()));
            }

            // Labels
            if let Some(ref labels) = config.labels {
                if !labels.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(section_header("Labels"));
                    for (k, v) in labels {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("  {}", k),
                                Style::default().fg(Color::Cyan),
                            ),
                            Span::styled(" = ", Style::default().fg(Color::DarkGray)),
                            Span::styled(v.clone(), Style::default().fg(Color::White)),
                        ]));
                    }
                }
            }

            // Exposed ports
            if let Some(ref exposed_ports) = config.exposed_ports {
                if !exposed_ports.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(section_header("Exposed Ports"));
                    for port in exposed_ports.keys() {
                        lines.push(kv_line("  Port", port));
                    }
                }
            }
        }

        // Host config
        if let Some(ref host_config) = data.host_config {
            lines.push(Line::from(""));
            lines.push(section_header("Host Config"));

            if let Some(ref restart_policy) = host_config.restart_policy {
                if let Some(ref name) = restart_policy.name {
                    lines.push(kv_line("  Restart Policy", &format!("{:?}", name)));
                }
            }
            if let Some(ref memory) = host_config.memory {
                if *memory > 0 {
                    let mb = *memory as f64 / 1_048_576.0;
                    lines.push(kv_line("  Memory Limit", &format!("{:.0} MB", mb)));
                }
            }
            if let Some(ref cpu_shares) = host_config.cpu_shares {
                if *cpu_shares > 0 {
                    lines.push(kv_line("  CPU Shares", &cpu_shares.to_string()));
                }
            }
            if let Some(ref nano_cpus) = host_config.nano_cpus {
                if *nano_cpus > 0 {
                    let cpus = *nano_cpus as f64 / 1_000_000_000.0;
                    lines.push(kv_line("  CPUs", &format!("{:.2}", cpus)));
                }
            }
            if let Some(privileged) = host_config.privileged {
                lines.push(kv_line("  Privileged", &privileged.to_string()));
            }
        }

        lines
    }
}

fn kv_line(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}: ", key),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("--- {} ---", title),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ))
}

fn hint_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    ))
}
