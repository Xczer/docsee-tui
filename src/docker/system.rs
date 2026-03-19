use anyhow::{Context, Result};
use bollard::system::EventsOptions;
use chrono::Local;
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::client::DockerClient;

/// Docker system information with rich details
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub docker_version: String,
    pub api_version: String,
    pub os: String,
    pub arch: String,
    pub kernel_version: String,
    pub storage_driver: String,
    pub total_containers: i64,
    pub running_containers: i64,
    pub stopped_containers: i64,
    pub paused_containers: i64,
    pub total_images: i64,
    pub total_memory_bytes: i64,
    pub cpus: i64,
    pub server_name: String,
}

/// Docker disk usage stats
#[derive(Debug, Clone, Default)]
pub struct DiskUsage {
    pub containers_size: u64,
    pub containers_count: usize,
    pub images_size: u64,
    pub images_count: usize,
    pub volumes_size: u64,
    pub volumes_count: usize,
    pub build_cache_size: u64,
    pub build_cache_count: usize,
    pub total_size: u64,
}

/// A Docker system event
#[derive(Debug, Clone)]
pub struct DockerEvent {
    pub timestamp: String,
    pub event_type: String,
    pub action: String,
    pub actor_id: String,
    pub actor_name: String,
}

impl DockerClient {
    /// Get detailed system information
    pub async fn detailed_system_info(&self) -> Result<SystemInfo> {
        let info = self
            .inner()
            .info()
            .await
            .context("Failed to get Docker system info")?;

        let version = self
            .inner()
            .version()
            .await
            .context("Failed to get Docker version")?;

        Ok(SystemInfo {
            docker_version: version.version.unwrap_or_default(),
            api_version: version.api_version.unwrap_or_default(),
            os: info.operating_system.unwrap_or_default(),
            arch: info.architecture.unwrap_or_default(),
            kernel_version: info.kernel_version.unwrap_or_default(),
            storage_driver: info.driver.unwrap_or_default(),
            total_containers: info.containers.unwrap_or(0),
            running_containers: info.containers_running.unwrap_or(0),
            stopped_containers: info.containers_stopped.unwrap_or(0),
            paused_containers: info.containers_paused.unwrap_or(0),
            total_images: info.images.unwrap_or(0),
            total_memory_bytes: info.mem_total.unwrap_or(0),
            cpus: info.ncpu.unwrap_or(0),
            server_name: info.name.unwrap_or_default(),
        })
    }

    /// Get disk usage information
    pub async fn disk_usage(&self) -> Result<DiskUsage> {
        let df = self
            .inner()
            .df()
            .await
            .context("Failed to get Docker disk usage")?;

        let mut usage = DiskUsage::default();

        if let Some(containers) = df.containers {
            usage.containers_count = containers.len();
            usage.containers_size = containers
                .iter()
                .map(|c| c.size_rw.unwrap_or(0) as u64)
                .sum();
        }

        if let Some(images) = df.images {
            usage.images_count = images.len();
            usage.images_size = images
                .iter()
                .map(|i| i.size.max(0) as u64)
                .sum();
        }

        if let Some(volumes) = df.volumes {
            usage.volumes_count = volumes.len();
            usage.volumes_size = volumes
                .iter()
                .map(|v| {
                    v.usage_data
                        .as_ref()
                        .map(|u| u.size as u64)
                        .unwrap_or(0)
                })
                .sum();
        }

        if let Some(build_cache) = df.build_cache {
            usage.build_cache_count = build_cache.len();
            usage.build_cache_size = build_cache
                .iter()
                .map(|b| b.size.unwrap_or(0) as u64)
                .sum();
        }

        usage.total_size =
            usage.containers_size + usage.images_size + usage.volumes_size + usage.build_cache_size;

        Ok(usage)
    }

    /// Start streaming Docker events, returns a channel receiver
    pub fn stream_events(
        &self,
    ) -> (
        mpsc::UnboundedReceiver<DockerEvent>,
        tokio::task::JoinHandle<()>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let client = self.inner().clone();

        let handle = tokio::spawn(async move {
            use futures::stream::StreamExt;

            let options = EventsOptions::<String> {
                since: None,
                until: None,
                filters: HashMap::new(),
            };

            let mut stream = client.events(Some(options));

            while let Some(event_result) = stream.next().await {
                match event_result {
                    Ok(event) => {
                        let timestamp = Local::now().format("%H:%M:%S").to_string();

                        let event_type = event
                            .typ
                            .map(|t| format!("{:?}", t).to_lowercase())
                            .unwrap_or_else(|| "unknown".to_string());

                        let action = event
                            .action
                            .unwrap_or_else(|| "unknown".to_string());

                        let (actor_id, actor_name) =
                            if let Some(actor) = event.actor {
                                let id = actor
                                    .id
                                    .as_deref()
                                    .unwrap_or("")
                                    .chars()
                                    .take(12)
                                    .collect::<String>();
                                let name = actor
                                    .attributes
                                    .as_ref()
                                    .and_then(|a| a.get("name").cloned())
                                    .unwrap_or_default();
                                (id, name)
                            } else {
                                (String::new(), String::new())
                            };

                        let docker_event = DockerEvent {
                            timestamp,
                            event_type,
                            action,
                            actor_id,
                            actor_name,
                        };

                        if sender.send(docker_event).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        (receiver, handle)
    }
}
