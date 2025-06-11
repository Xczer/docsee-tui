# Docsee 🐳

A beautiful Docker manager TUI (Terminal User Interface) application built with Rust and Ratatui.

## Features

✅ **Container Management**
- View all containers with status indicators
- Start, stop, restart containers
- Delete stopped containers
- Navigate with arrow keys

🔄 **Coming Soon**
- Images management
- Volumes management
- Networks management
- Live logs viewer
- Container shell execution

## Installation

Make sure you have Rust installed, then:

```bash
git clone <your-repo-url>
cd docsee
cargo build --release
```

## Usage

Make sure Docker is running, then:

```bash
# Run with default Docker socket
cargo run

# Or specify a custom Docker host
cargo run -- --docker-host unix:///var/run/docker.sock
cargo run -- --docker-host tcp://localhost:2375
```

## Controls

### Global Commands
- `←/→` - Switch between tabs
- `c` - Show cheatsheet
- `q` - Quit application

### Container Tab
- `↑/↓` - Navigate containers
- `u` - Start container
- `d` - Stop container
- `r` - Restart container
- `D` - Delete container (only if stopped)
- `l` - View logs (coming soon)
- `e` - Execute shell (coming soon)

## Architecture

The application is structured as follows:

```
src/
├── main.rs          # Entry point
├── app.rs           # Main application logic
├── events/          # Event handling system
├── docker/          # Docker API operations
├── ui/              # User interface components
└── widgets/         # Custom UI widgets
```

## Dependencies

- **ratatui** - Terminal UI framework
- **crossterm** - Cross-platform terminal manipulation
- **bollard** - Docker API client
- **tokio** - Async runtime
- **color-eyre** - Better error handling
- **clap** - Command line parsing

## Development

### Running in Development
```bash
cargo run
```

### Running Tests
```bash
cargo test
```

### Check for Issues
```bash
cargo clippy
cargo fmt
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details.
