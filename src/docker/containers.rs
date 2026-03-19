use anyhow::{Context, Result};
use bollard::{
    container::{
        ListContainersOptions, RemoveContainerOptions, RestartContainerOptions,
        StartContainerOptions, StopContainerOptions,
    },
    models::ContainerSummary,
    service::ContainerInspectResponse,
};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};

use super::client::DockerClient;

/// Represents a Docker container with formatted information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: ContainerState,
    pub ports: String,
    pub created: String,
    pub size: Option<String>,
    /// Docker Compose project name (from label)
    pub compose_project: Option<String>,
    /// Docker Compose service name (from label)
    pub compose_service: Option<String>,
}

/// Container state for easy status checking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContainerState {
    Running,
    Stopped,
    Paused,
    Restarting,
    Dead,
    Unknown,
}

impl From<Option<&str>> for ContainerState {
    fn from(state: Option<&str>) -> Self {
        match state {
            Some("running") => ContainerState::Running,
            Some("exited") => ContainerState::Stopped,
            Some("paused") => ContainerState::Paused,
            Some("restarting") => ContainerState::Restarting,
            Some("dead") => ContainerState::Dead,
            _ => ContainerState::Unknown,
        }
    }
}

impl ContainerState {
    /// Get a display string for the state
    pub fn display(&self) -> &'static str {
        match self {
            ContainerState::Running => "🟢 Running",
            ContainerState::Stopped => "🔴 Stopped",
            ContainerState::Paused => "🟡 Paused",
            ContainerState::Restarting => "🔄 Restarting",
            ContainerState::Dead => "💀 Dead",
            ContainerState::Unknown => "❓ Unknown",
        }
    }
}

/// Container management operations
impl DockerClient {
    /// List all containers (running and stopped)
    pub async fn list_containers(&self) -> Result<Vec<Container>> {
        let options = Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        });

        let containers = self
            .inner()
            .list_containers(options)
            .await
            .context("Failed to list containers")?;

        let mut result = Vec::new();
        for container in containers {
            result.push(self.format_container(container)?);
        }

        // Sort by name for consistent display
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Get detailed information about a specific container
    pub async fn inspect_container(&self, id: &str) -> Result<ContainerInspectResponse> {
        self.inner()
            .inspect_container(id, None)
            .await
            .with_context(|| format!("Failed to inspect container {id}"))
    }

    /// Start a container
    pub async fn start_container(&self, id: &str) -> Result<()> {
        self.inner()
            .start_container(id, None::<StartContainerOptions<String>>)
            .await
            .with_context(|| format!("Failed to start container {id}"))
    }

    /// Stop a container
    pub async fn stop_container(&self, id: &str) -> Result<()> {
        let options = StopContainerOptions { t: 10 }; // 10 second timeout
        self.inner()
            .stop_container(id, Some(options))
            .await
            .with_context(|| format!("Failed to stop container {}", id))
    }

    /// Restart a container
    pub async fn restart_container(&self, id: &str) -> Result<()> {
        let options = RestartContainerOptions { t: 10 }; // 10 second timeout
        self.inner()
            .restart_container(id, Some(options))
            .await
            .with_context(|| format!("Failed to restart container {}", id))
    }

    /// Create and start a container from an image
    pub async fn create_and_start_container(&self, image: &str, name: Option<&str>) -> Result<String> {
        use bollard::container::Config;

        let config = Config {
            image: Some(image.to_string()),
            ..Default::default()
        };

        let mut options = bollard::container::CreateContainerOptions {
            name: name.unwrap_or("").to_string(),
            platform: None,
        };
        if name.is_none() || name == Some("") {
            // Let Docker generate a name
            options.name = String::new();
        }

        let response = if options.name.is_empty() {
            self.inner()
                .create_container(None::<bollard::container::CreateContainerOptions<String>>, config)
                .await
                .context("Failed to create container")?
        } else {
            self.inner()
                .create_container(Some(options), config)
                .await
                .context("Failed to create container")?
        };

        let id = response.id;
        self.start_container(&id).await?;
        Ok(id)
    }

    /// Remove a container
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<()> {
        let options = RemoveContainerOptions {
            force,
            v: true, // Remove volumes
            ..Default::default()
        };

        self.inner()
            .remove_container(id, Some(options))
            .await
            .with_context(|| format!("Failed to remove container {}", id))
    }

    /// Start all stopped containers in a compose project
    pub async fn compose_up(&self, project: &str) -> Result<(usize, usize)> {
        let containers = self.list_containers().await?;
        let mut ok = 0;
        let mut fail = 0;
        for c in &containers {
            if c.compose_project.as_deref() == Some(project)
                && c.state != ContainerState::Running
            {
                match self.start_container(&c.id).await {
                    Ok(_) => ok += 1,
                    Err(_) => fail += 1,
                }
            }
        }
        Ok((ok, fail))
    }

    /// Stop all running containers in a compose project
    pub async fn compose_down(&self, project: &str) -> Result<(usize, usize)> {
        let containers = self.list_containers().await?;
        let mut ok = 0;
        let mut fail = 0;
        for c in &containers {
            if c.compose_project.as_deref() == Some(project)
                && c.state == ContainerState::Running
            {
                match self.stop_container(&c.id).await {
                    Ok(_) => ok += 1,
                    Err(_) => fail += 1,
                }
            }
        }
        Ok((ok, fail))
    }

    /// Format a container summary into our Container struct
    fn format_container(&self, container: ContainerSummary) -> Result<Container> {
        let id = container.id.unwrap_or_default();
        let short_id = if id.len() > 12 {
            id[..12].to_string()
        } else {
            id.clone()
        };

        let name = container
            .names
            .and_then(|names| names.first().cloned())
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();

        let image = container.image.unwrap_or_default();
        let status = container.status.unwrap_or_default();
        let state = ContainerState::from(container.state.as_deref());

        // Extract Docker Compose labels
        let labels = container.labels.as_ref();
        let compose_project = labels
            .and_then(|l| l.get("com.docker.compose.project"))
            .cloned();
        let compose_service = labels
            .and_then(|l| l.get("com.docker.compose.service"))
            .cloned();

        // Format ports
        let ports = if let Some(port_list) = container.ports {
            port_list
                .iter()
                .map(|port| {
                    let private_port = port.private_port;
                    let public_port = port.public_port.map(|p| p.to_string()).unwrap_or_default();
                    let ip = port.ip.as_deref().unwrap_or("");

                    if public_port.is_empty() {
                        format!("{}", private_port)
                    } else if ip.is_empty() {
                        format!("{}:{}", public_port, private_port)
                    } else {
                        format!("{}:{}:{}", ip, public_port, private_port)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            String::new()
        };

        // Format creation time
        let created = if let Some(timestamp) = container.created {
            match Local.timestamp_opt(timestamp, 0) {
                chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                _ => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };

        Ok(Container {
            id: short_id,
            name,
            image,
            status,
            state,
            ports,
            created,
            size: None,
            compose_project,
            compose_service,
        })
    }
}
