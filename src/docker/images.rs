use anyhow::{Context, Result};
use bollard::{
    image::{ListImagesOptions, PruneImagesOptions, RemoveImageOptions},
    models::ImageSummary,
};
use byte_unit::{Byte, UnitType};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::client::DockerClient;

/// one image row, already formatted for the table
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

impl DockerClient {
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
            // one summary can map to multiple repo:tag rows
            let formatted_images = self.format_image(image)?;
            result.extend(formatted_images);
        }

        result.sort_by(|a, b| match a.repository.cmp(&b.repository) {
            std::cmp::Ordering::Equal => a.tag.cmp(&b.tag),
            other => other,
        });

        Ok(result)
    }

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

    pub async fn inspect_image(&self, id: &str) -> Result<bollard::models::ImageInspect> {
        self.inner()
            .inspect_image(id)
            .await
            .with_context(|| format!("Failed to inspect image {}", id))
    }

    fn format_image(&self, image: ImageSummary) -> Result<Vec<Image>> {
        let mut result = Vec::new();
        let id = image.id.clone();
        let short_id = if id.len() > 12 {
            id[7..19].to_string() // drop the "sha256:" prefix, keep 12 chars
        } else {
            id.clone()
        };

        let size_bytes = image.size;
        let size = if size_bytes > 0 {
            let byte = Byte::from_u64(size_bytes as u64).get_appropriate_unit(UnitType::Binary);
            format!("{:.1}", byte)
        } else {
            "0 B".to_string()
        };

        // image.created is a plain i64 epoch here, not Option
        let created = match Local.timestamp_opt(image.created, 0) {
            chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => "Unknown".to_string(),
        };

        // dangling = no real tags, docker reports them as <none>:<none>
        let repo_tags = image.repo_tags.clone();
        let is_dangling =
            repo_tags.is_empty() || repo_tags.iter().all(|tag| tag == "<none>:<none>");

        if is_dangling {
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
            for repo_tag in repo_tags {
                // registry host can contain a ':' (port) so split carefully - last part is the tag
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

    pub async fn pull_image(&self, image_name: &str) -> Result<()> {
        use bollard::image::CreateImageOptions;

        let options = Some(CreateImageOptions {
            from_image: image_name,
            ..Default::default()
        });

        let mut stream = self.inner().create_image(options, None, None);

        // just drain the progress stream till pull finishes
        while let Some(result) = futures::stream::TryStreamExt::try_next(&mut stream).await? {
            let _ = result;
        }

        Ok(())
    }

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
