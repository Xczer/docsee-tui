use anyhow::{Context, Result};
use bollard::{
    image::{ListImagesOptions, RemoveImageOptions, PruneImagesOptions},
    models::ImageSummary,
};
use byte_unit::{Byte, UnitType};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::client::DockerClient;

/// Represents a Docker image with formatted information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
    pub labels: Option<HashMap<String, String>>,
    pub is_dangling: bool,
}

/// Image management operations
impl DockerClient {
    /// List all images
    pub async fn list_images(&self) -> Result<Vec<Image>> {
        let options = Some(ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        });

        let images = self
            .inner()
            .list_images(options)
            .await
            .context("Failed to list images")?;

        let mut result = Vec::new();
        for image in images {
            // Each image can have multiple repo:tag combinations
            let formatted_images = self.format_image(image)?;
            result.extend(formatted_images);
        }

        // Sort by repository and tag for consistent display
        result.sort_by(|a, b| {
            match a.repository.cmp(&b.repository) {
                std::cmp::Ordering::Equal => a.tag.cmp(&b.tag),
                other => other,
            }
        });

        Ok(result)
    }

    /// Remove an image
    pub async fn remove_image(&self, id: &str, force: bool) -> Result<()> {
        let options = RemoveImageOptions {
            force,
            noprune: false,
        };

        self.inner()
            .remove_image(id, Some(options), None)
            .await
            .with_context(|| format!("Failed to remove image {}", id))?;

        Ok(())
    }

    /// Get detailed information about a specific image
    pub async fn inspect_image(&self, id: &str) -> Result<bollard::models::ImageInspect> {
        self.inner()
            .inspect_image(id)
            .await
            .with_context(|| format!("Failed to inspect image {}", id))
    }

    /// Format an image summary into our Image structs
    fn format_image(&self, image: ImageSummary) -> Result<Vec<Image>> {
        let mut result = Vec::new();
        let id = image.id.clone();
        let short_id = if id.len() > 12 {
            id[7..19].to_string() // Skip "sha256:" prefix and take 12 chars
        } else {
            id.clone()
        };

        // Format size
        let size_bytes = image.size;
        let size = if size_bytes > 0 {
            let byte = Byte::from_u64(size_bytes as u64).get_appropriate_unit(UnitType::Binary);
            format!("{:.1}", byte)
        } else {
            "0 B".to_string()
        };

        // Format creation time - image.created is already i64
        let created = match Local.timestamp_opt(image.created, 0) {
            chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => "Unknown".to_string(),
        };

        // Check if image is dangling (no repo tags)
        let repo_tags = image.repo_tags.clone();
        let is_dangling = repo_tags.is_empty() || repo_tags.iter().all(|tag| tag == "<none>:<none>");

        if is_dangling {
            // Dangling image with no tags
            result.push(Image {
                id: short_id,
                repository: "<none>".to_string(),
                tag: "<none>".to_string(),
                size: size.clone(),
                created: created.clone(),
                labels: Some(image.labels.clone()),
                is_dangling: true,
            });
        } else {
            // Create an entry for each repo:tag combination
            for repo_tag in repo_tags {
                let parts: Vec<&str> = repo_tag.split(':').collect();
                let (repository, tag) = if parts.len() >= 2 {
                    (parts[0].to_string(), parts[1..].join(":"))
                } else {
                    (repo_tag.clone(), "latest".to_string())
                };

                result.push(Image {
                    id: short_id.clone(),
                    repository,
                    tag,
                    size: size.clone(),
                    created: created.clone(),
                    labels: Some(image.labels.clone()),
                    is_dangling: false,
                });
            }
        }

        Ok(result)
    }

    /// Prune unused images
    pub async fn prune_images(&self) -> Result<u64> {
        let options = PruneImagesOptions::<String> {
            filters: HashMap::new(),
        };

        let prune_result = self
            .inner()
            .prune_images(Some(options))
            .await
            .context("Failed to prune images")?;

        Ok(prune_result.space_reclaimed.unwrap_or(0) as u64)
    }
}

/*
EXPLANATION:
- Image struct represents a Docker image with human-readable formatting
- Each Docker image can have multiple repository:tag combinations
- format_image() handles this by creating multiple Image entries for one ImageSummary
- Dangling images (no tags) are marked specially with <none>:<none>
- Size is formatted using byte-unit for human readability (MB, GB, etc.)
- Created time is converted to local timezone
- Operations include: list, remove, inspect, and prune (cleanup unused images)
- Sorting is by repository name first, then tag name for consistent display
- Error handling provides context for debugging

FIXES APPLIED:
- Fixed image.created handling (it's already i64, not Option<i64>)
- Fixed repo_tags.unwrap_or_else(Vec::new) instead of unwrap_or_default()
- Fixed labels handling (image.labels is already Option<HashMap>)
- Fixed PruneImagesOptions type annotation with <String>
- Removed unnecessary .map() calls on labels
*/
