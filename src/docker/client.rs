use anyhow::{Context, Result};
use bollard::Docker;
use std::collections::HashMap;

/// Wrapper around the Docker client for easier error handling
#[derive(Clone)]
pub struct DockerClient {
    client: Docker,
}

impl DockerClient {
    /// Create a new Docker client
    pub async fn new(host: &str) -> Result<Self> {
        let client = if host.starts_with("unix://") {
            // Unix socket connection
            let socket_path = host.strip_prefix("unix://").unwrap_or(host);
            Docker::connect_with_socket(socket_path, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker via Unix socket")?
        } else if host.starts_with("tcp://") {
            // TCP connection
            Docker::connect_with_http(host, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker via TCP")?
        } else {
            // Default to Unix socket
            Docker::connect_with_socket(host, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker with default settings")?
        };

        // Test the connection by pinging Docker
        client
            .ping()
            .await
            .context("Failed to ping Docker daemon - is Docker running?")?;

        Ok(Self { client })
    }

    /// Get the underlying Docker client
    pub fn inner(&self) -> &Docker {
        &self.client
    }

    /// Get system information from Docker
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

/*
EXPLANATION:
- This is a wrapper around the bollard Docker client
- It handles connection logic for both Unix sockets and TCP connections
- The `new` method tests the connection by pinging the Docker daemon
- It provides convenient error handling with meaningful error messages
- The `inner()` method gives access to the underlying client when needed
- `system_info()` gets basic stats about the Docker system
- This wrapper makes it easier to work with Docker throughout our application
*/
