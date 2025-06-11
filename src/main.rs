use clap::Parser;
use color_eyre::Result;
use docsee::app::App;

#[derive(Parser)]
#[command(name = "docsee")]
#[command(about = "A beautiful Docker manager TUI application")]
struct Cli {
    /// Docker host URL (e.g., unix:///var/run/docker.sock)
    #[arg(long, default_value = "unix:///var/run/docker.sock")]
    docker_host: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize error handling
    color_eyre::install()?;

    // Parse command line arguments
    let cli = Cli::parse();

    // Create and run the application
    let mut app = App::new(&cli.docker_host).await
        .map_err(|e| color_eyre::eyre::eyre!("Failed to create app: {}", e))?;
    app.run().await
        .map_err(|e| color_eyre::eyre::eyre!("Failed to run app: {}", e))?;

    Ok(())
}

/*
EXPLANATION:
- This is the entry point of our application
- We use `clap` to parse command line arguments (currently just Docker host)
- `color_eyre` provides beautiful error messages
- `tokio::main` makes this an async main function since we'll be doing async Docker operations
- We create an App instance and run it
- The error conversion using `map_err` converts anyhow::Error to color_eyre::eyre::Error
*/
