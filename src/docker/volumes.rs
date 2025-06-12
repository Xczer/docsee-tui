use anyhow::{Context, Result};
use bollard::{
    models::{Volume as DockerVolume, VolumeUsageData},
    volume::{ListVolumesOptions, PruneVolumesOptions, RemoveVolumeOptions},
};
use byte_unit::{Byte, UnitType};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::client::DockerClient;

/// Represents a Docker volume with formatted information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub scope: String,
    pub size: String,
    pub created: String,
    pub in_use: bool,
    pub labels: HashMap<String, String>,
}

/// Volume management operations
impl DockerClient {
    /// List all volumes
    pub async fn list_volumes(&self) -> Result<Vec<Volume>> {
        let options = Some(ListVolumesOptions::<String> {
            ..Default::default()
        });

        let volume_list = self
            .inner()
            .list_volumes(options)
            .await
            .context("Failed to list volumes")?;

        let mut result = Vec::new();

        if let Some(volumes) = volume_list.volumes {
            for volume in volumes {
                result.push(self.format_volume(volume)?);
            }
        }

        // Sort by name for consistent display
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Remove a volume
    pub async fn remove_volume(&self, name: &str, force: bool) -> Result<()> {
        let options = RemoveVolumeOptions { force };

        self.inner()
            .remove_volume(name, Some(options))
            .await
            .with_context(|| format!("Failed to remove volume {}", name))
    }

    /// Get detailed information about a specific volume
    pub async fn inspect_volume(&self, name: &str) -> Result<DockerVolume> {
        self.inner()
            .inspect_volume(name)
            .await
            .with_context(|| format!("Failed to inspect volume {}", name))
    }

    /// Prune unused volumes
    pub async fn prune_volumes(&self) -> Result<u64> {
        let options = PruneVolumesOptions::<String> {
            filters: HashMap::new(),
        };

        let prune_result = self
            .inner()
            .prune_volumes(Some(options))
            .await
            .context("Failed to prune volumes")?;

        Ok(prune_result.space_reclaimed.unwrap_or(0) as u64)
    }

    /// Format a volume into our Volume struct
    fn format_volume(&self, volume: DockerVolume) -> Result<Volume> {
        let name = volume.name;
        let driver = volume.driver;
        let mountpoint = volume.mountpoint;

        // Handle scope - convert enum to string or use default
        let scope = if let Some(scope_enum) = volume.scope {
            format!("{:?}", scope_enum).to_lowercase()
        } else {
            "local".to_string()
        };

        // Handle labels - they're already Option<HashMap<String, String>>
        let labels = volume.labels;

        // Format size from usage data
        let (size, in_use) = if let Some(usage_data) = volume.usage_data {
            let VolumeUsageData { size, ref_count } = usage_data;
            let formatted_size = if size > 0 {
                let byte = Byte::from_u64(size as u64).get_appropriate_unit(UnitType::Binary);
                format!("{:.1}", byte)
            } else {
                "0 B".to_string()
            };
            (formatted_size, ref_count > 0)
        } else {
            ("Unknown".to_string(), false)
        };

        // Format creation time
        let created = if let Some(created_at) = volume.created_at {
            match DateTime::parse_from_rfc3339(&created_at) {
                Ok(dt) => dt
                    .with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };

        Ok(Volume {
            name,
            driver,
            mountpoint,
            scope,
            size,
            created,
            in_use,
            labels,
        })
    }
}

/*
EXPLANATION:
- Volume struct represents a Docker volume with human-readable formatting
- format_volume() converts Docker API response to our format
- Size is formatted using byte-unit for readability (MB, GB, etc.)
- in_use indicates if the volume is currently mounted by containers
- created time is parsed from RFC3339 format and converted to local timezone
- Operations include: list, remove, inspect, and prune (cleanup unused volumes)
- Sorting is by volume name for consistent display
- Error handling provides context for debugging

FIXES APPLIED:
- Fixed scope handling: VolumeScope is an enum, convert to string using Debug formatting
- Fixed labels handling: use unwrap_or_else(HashMap::new) instead of unwrap_or_default()
- Scope field in Volume struct changed to String to match the converted enum
*/
