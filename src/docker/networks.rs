use anyhow::{Context, Result};
use bollard::{
    network::{ListNetworksOptions, PruneNetworksOptions},
    models::Network as DockerNetwork,
};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::client::DockerClient;

/// Represents a Docker network with formatted information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub created: String,
    pub internal: bool,
    pub attachable: bool,
    pub ingress: bool,
    pub ipam_driver: String,
    pub subnet: String,
    pub gateway: String,
    pub connected_containers: usize,
    pub labels: HashMap<String, String>,
}

/// Network management operations
impl DockerClient {
    /// List all networks
    pub async fn list_networks(&self) -> Result<Vec<Network>> {
        let options = Some(ListNetworksOptions::<String> {
            ..Default::default()
        });

        let networks = self
            .inner()
            .list_networks(options)
            .await
            .context("Failed to list networks")?;

        let mut result = Vec::new();
        for network in networks {
            result.push(self.format_network(network)?);
        }

        // Sort by name for consistent display
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Remove a network
    pub async fn remove_network(&self, id: &str) -> Result<()> {
        self.inner()
            .remove_network(id)
            .await
            .with_context(|| format!("Failed to remove network {}", id))
    }

    /// Get detailed information about a specific network
    pub async fn inspect_network(&self, id: &str) -> Result<DockerNetwork> {
        self.inner()
            .inspect_network::<String>(id, None)
            .await
            .with_context(|| format!("Failed to inspect network {}", id))
    }

    /// Prune unused networks
    pub async fn prune_networks(&self) -> Result<Vec<String>> {
        let options = PruneNetworksOptions::<String> {
            filters: HashMap::new(),
        };

        let prune_result = self
            .inner()
            .prune_networks(Some(options))
            .await
            .context("Failed to prune networks")?;

        Ok(prune_result.networks_deleted.unwrap_or_default())
    }

    /// Format a network into our Network struct
    fn format_network(&self, network: DockerNetwork) -> Result<Network> {
        let id = network.id.unwrap_or_default();
        let short_id = if id.len() > 12 {
            id[..12].to_string()
        } else {
            id.clone()
        };

        let name = network.name.unwrap_or_default();
        let driver = network.driver.unwrap_or_default();
        let scope = network.scope.unwrap_or_default();
        let internal = network.internal.unwrap_or(false);
        let attachable = network.attachable.unwrap_or(false);
        let ingress = network.ingress.unwrap_or(false);
        let labels = network.labels.unwrap_or_default();

        // Format creation time
        let created = if let Some(created_at) = network.created {
            match DateTime::parse_from_rfc3339(&created_at) {
                Ok(dt) => dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string(),
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };

        // Get IPAM information
        let (ipam_driver, subnet, gateway) = if let Some(ipam) = network.ipam {
            let driver = ipam.driver.unwrap_or_default();
            let mut subnet = String::new();
            let mut gateway = String::new();

            if let Some(config) = ipam.config {
                if let Some(first_config) = config.first() {
                    if let Some(sub) = &first_config.subnet {
                        subnet = sub.clone();
                    }
                    if let Some(gw) = &first_config.gateway {
                        gateway = gw.clone();
                    }
                }
            }

            (driver, subnet, gateway)
        } else {
            (String::new(), String::new(), String::new())
        };

        // Count connected containers
        let connected_containers = network.containers
            .map(|containers| containers.len())
            .unwrap_or(0);

        Ok(Network {
            id: short_id,
            name,
            driver,
            scope,
            created,
            internal,
            attachable,
            ingress,
            ipam_driver,
            subnet,
            gateway,
            connected_containers,
            labels,
        })
    }
}

/*
EXPLANATION:
- Network struct represents a Docker network with human-readable formatting
- format_network() converts Docker API response to our format
- Includes networking details like driver, scope, subnet, gateway
- Shows whether network is internal, attachable, or ingress
- Counts connected containers for each network
- created time is parsed from RFC3339 format and converted to local timezone
- Operations include: list, remove, inspect, and prune (cleanup unused networks)
- Sorting is by network name for consistent display
- Error handling provides context for debugging
- IPAM (IP Address Management) details are extracted from the nested structure

FIXES APPLIED:
- Removed RemoveNetworkOptions import (doesn't exist in bollard 0.18.1)
- Fixed remove_network call to take only network ID
- Added type annotation to inspect_network call
*/
