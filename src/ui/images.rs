use anyhow::Result;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{images::Image, DockerClient},
    events::Key,
};

/// The images tab widget
pub struct ImagesTab {
    /// Docker client for operations
    docker_client: DockerClient,
    /// List of images
    images: Vec<Image>,
    /// Table state for selection
    table_state: TableState,
    /// Status message to show
    status_message: Option<String>,
}

impl ImagesTab {
    /// Create a new images tab
    pub async fn new(docker_client: DockerClient) -> Result<Self> {
        let mut tab = Self {
            docker_client,
            images: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
        };

        // Load initial data
        tab.refresh().await?;

        // Select first image if any exist
        if !tab.images.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Refresh image data from Docker
    pub async fn refresh(&mut self) -> Result<()> {
        match self.docker_client.list_images().await {
            Ok(images) => {
                // Remember current selection
                let selected_id = self.get_selected_image().map(|i| i.id.clone());

                self.images = images;
                self.status_message = None;

                // Restore selection or select first item
                if let Some(id) = selected_id {
                    // Try to find the same image
                    let new_index = self.images.iter().position(|i| i.id == id);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.images.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading images: {}", e));
            }
        }
        Ok(())
    }

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Up => self.move_selection_up(),
            Key::Down => self.move_selection_down(),
            Key::DeleteItem => self.delete_selected_image().await?,
            Key::Logs => {
                // Images don't have logs, show helpful message
                self.status_message = Some("Images don't have logs. Try containers instead!".to_string());
            }
            Key::Exec => {
                // Can't exec into images, show helpful message
                self.status_message = Some("Can't execute into images. Run as container first!".to_string());
            }
            Key::Start => {
                // Can't start images directly, show helpful message
                self.status_message = Some("Can't start images. Create container first!".to_string());
            }
            Key::Stop => {
                // Can't stop images, show helpful message
                self.status_message = Some("Images are not running. Try containers instead!".to_string());
            }
            Key::Restart => {
                // Can't restart images, show helpful message
                self.status_message = Some("Images are not running. Try containers instead!".to_string());
            }
            Key::Prune => {
                // Prune unused images
                self.prune_images().await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Draw the images tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Create table rows
        let rows: Vec<Row> = self
            .images
            .iter()
            .map(|image| {
                let style = if image.is_dangling {
                    Style::default().fg(Color::Red) // Dangling images in red
                } else if image.repository == "<none>" {
                    Style::default().fg(Color::Yellow) // Untagged images in yellow
                } else {
                    Style::default().fg(Color::White) // Normal images in white
                };

                Row::new(vec![
                    Cell::from(image.id.clone()),
                    Cell::from(image.repository.clone()),
                    Cell::from(image.tag.clone()),
                    Cell::from(image.size.clone()),
                    Cell::from(image.created.clone()),
                ])
                .style(style)
            })
            .collect();

        // Create table headers
        let header = Row::new(vec![
            Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Repository").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Tag").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Size").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Created").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        // Build the title string
        let count = self.images.len();
        let dangling_count = self.images.iter().filter(|i| i.is_dangling).count();

        let title_text = format!("Images ({} total, {} dangling)", count, dangling_count);

        let title = if let Some(ref message) = self.status_message {
            format!("{} - {}", title_text, message)
        } else {
            title_text
        };

        // Create the table widget
        let table = Table::new(
            rows,
            [
                Constraint::Length(12), // ID
                Constraint::Length(30), // Repository
                Constraint::Length(15), // Tag
                Constraint::Length(10), // Size
                Constraint::Length(20), // Created
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(">> ");

        // Render the table
        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    /// Get the currently selected image
    fn get_selected_image(&self) -> Option<&Image> {
        self.table_state
            .selected()
            .and_then(|index| self.images.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.images.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.images.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.images.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.images.len();
        self.table_state.select(Some(new_index));
    }

    /// Delete the selected image
    async fn delete_selected_image(&mut self) -> Result<()> {
        if let Some(image) = self.get_selected_image() {
            let id = image.id.clone();
            let name = if image.repository == "<none>" {
                format!("{}", image.id)
            } else {
                format!("{}:{}", image.repository, image.tag)
            };

            match self.docker_client.remove_image(&id, false).await {
                Ok(_) => {
                    self.status_message = Some(format!("Deleted image '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to delete '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Prune unused images
    async fn prune_images(&mut self) -> Result<()> {
        match self.docker_client.prune_images().await {
            Ok(space_reclaimed) => {
                let space_mb = space_reclaimed as f64 / 1_048_576.0; // Convert to MB
                self.status_message = Some(format!("Pruned images, reclaimed {:.1} MB", space_mb));
                self.refresh().await?;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to prune images: {}", e));
            }
        }
        Ok(())
    }
}

/*
EXPLANATION:
- ImagesTab manages the Docker images display and interactions
- new() creates the tab and loads initial image data
- refresh() reloads images from Docker, preserving current selection
- handle_key() processes keyboard shortcuts:
  - Up/Down arrows: navigate the image list
  - D: delete image
  - p: prune unused images
  - Other container keys show helpful messages explaining they don't apply to images
- draw() renders the images table with color coding:
  - Red: dangling images (no tags)
  - Yellow: untagged images (<none>)
  - White: normal images
- The table shows: ID, Repository, Tag, Size, Created date
- Title shows total images and dangling count
- delete_selected_image() removes images with user feedback
- prune_images() cleans up unused images and shows space reclaimed
- Error handling provides user feedback for all operations
*/
