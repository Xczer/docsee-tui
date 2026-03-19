use clap::Parser;
use color_eyre::Result;
use docsee::app::App;
use docsee::config::AppConfig;
use docsee::theme::Theme;

#[derive(Parser)]
#[command(name = "docsee")]
#[command(about = "A beautiful Docker manager TUI application")]
struct Cli {
    /// Docker host URL (e.g., unix:///var/run/docker.sock)
    #[arg(long)]
    docker_host: Option<String>,

    /// Path to config file
    #[arg(long)]
    config: Option<String>,

    /// Color theme (default, light, nord, dracula, gruvbox)
    #[arg(long)]
    theme: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let mut config = AppConfig::load(cli.config.as_deref())
        .map_err(|e| color_eyre::eyre::eyre!("Failed to load config: {}", e))?;

    // CLI overrides
    if let Some(host) = cli.docker_host {
        config.general.docker_host = host;
    }
    if let Some(theme_name) = cli.theme {
        config.theme.name = theme_name;
    }

    let theme = Theme::from_name(&config.theme.name);

    let mut app = App::new(&config, theme)
        .await
        .map_err(|e| color_eyre::eyre::eyre!("Failed to create app: {}", e))?;
    app.run()
        .await
        .map_err(|e| color_eyre::eyre::eyre!("Failed to run app: {}", e))?;

    Ok(())
}
