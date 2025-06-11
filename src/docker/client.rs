use anyhow::{Context, Result};
use bollard::Docker;
use std::collections::HashMap;

/// thin wrapper over bollard so error handling stays in one place
#[derive(Clone)]
pub struct DockerClient {
    client: Docker,
}

impl DockerClient {
    pub async fn new(host: &str) -> Result<Self> {
        let client = if host.starts_with("unix://") {
            let socket_path = host.strip_prefix("unix://").unwrap_or(host);
            Docker::connect_with_socket(socket_path, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker via Unix socket")?
        } else if host.starts_with("tcp://") {
            Docker::connect_with_http(host, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker via TCP")?
        } else {
            // no scheme given, assume it's a socket path
            Docker::connect_with_socket(host, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker with default settings")?
        };

        // ping once so we fail fast if daemon isn't up
        client
            .ping()
            .await
            .context("Failed to ping Docker daemon - is Docker running?")?;

        Ok(Self { client })
    }

    /// get at the raw bollard client when the wrapper doesn't cover something
    pub fn inner(&self) -> &Docker {
        &self.client
    }

    pub async fn system_info(&self) -> Result<HashMap<String, String>> {
        let info = self
            .client
            .info()
            .await
            .context("Failed to get Docker system info")?;

        let mut result = HashMap::new();

        if let Some(version) = info.server_version {
            result.insert("Version".to_string(), version);
        }

        if let Some(containers) = info.containers {
            result.insert("Containers".to_string(), containers.to_string());
        }

        if let Some(images) = info.images {
            result.insert("Images".to_string(), images.to_string());
        }

        Ok(result)
    }
}
