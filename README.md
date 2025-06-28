# 🦆 Docsee - Docker Management TUI v1.0

```
██████╗  ██████╗  ██████╗███████╗███████╗███████╗
██╔══██╗██╔═══██╗██╔════╝██╔════╝██╔════╝██╔════╝
██║  ██║██║   ██║██║     ███████╗█████╗  █████╗  
██║  ██║██║   ██║██║     ╚════██║██╔══╝  ██╔══╝  
██████╔╝╚██████╔╝╚██████╗███████║███████╗███████╗
╚═════╝  ╚═════╝  ╚═════╝╚══════╝╚══════╝╚══════╝
            🦆 Docker Management TUI v1.0
```

A beautiful, feature-rich Docker management terminal user interface (TUI) application built with Rust and Ratatui. Docsee provides an intuitive interface for managing Docker containers, images, volumes, and networks with advanced features like real-time logging, shell access, resource monitoring, and powerful search capabilities.

## ✨ Features

### 🏠 Core Management
- **Container Management**: Start, stop, restart, delete containers with visual status indicators
- **Image Management**: View, delete, and prune Docker images with size information
- **Volume Management**: Manage Docker volumes with usage tracking
- **Network Management**: Handle Docker networks with connection details

### 🚀 Advanced Features
- **Real-time Logs**: Stream container logs with word wrap, timestamps, and scrolling
- **Shell Access**: Execute commands in containers with full terminal support
- **Resource Stats**: Monitor CPU, memory, network, and disk usage in real-time
- **Advanced Search**: Filter and search across all Docker resources
- **Interactive Navigation**: Intuitive keyboard shortcuts and navigation

### 🎨 User Experience
- **Beautiful ASCII Art**: Eye-catching title and visual elements
- **Enhanced Navigation**: Previous/current/next tab display instead of traditional tabs
- **Responsive Design**: Adapts to different terminal sizes
- **Status Indicators**: Clear visual feedback for all operations
- **Help System**: Comprehensive keyboard shortcuts and help

## 🚀 Installation

### Prerequisites
- Docker installed and running
- Rust toolchain (for building from source)

### From Source
```bash
# Clone the repository
git clone https://github.com/Xczer/docsee.git
cd docsee

# Build and install
cargo build --release
cargo install --path .

# Or use the provided Makefile
make install
```

### Quick Start
```bash
# Run with default Docker socket
docsee

# Run with custom Docker host
docsee --docker-host unix:///var/run/docker.sock

# Run with TCP connection
docsee --docker-host tcp://localhost:2375
```

## 🎮 Usage

### Navigation
- **←/→**: Navigate between tabs (Containers, Images, Volumes, Networks)
- **↑/↓**: Navigate items within lists
- **Enter**: Select/activate item
- **Esc**: Go back or exit
- **q**: Quit application
- **c**: Show help/cheatsheet

### Container Management
- **u**: Start container
- **d**: Stop container
- **r**: Restart container
- **D**: Delete container (when stopped)
- **l**: View real-time logs
- **e**: Shell executor mode
- **s**: Resource stats monitoring
- **i**: Interactive shell (full terminal)
- **/**: Search and filter

### Enhanced Logs Viewer
- **f**: Toggle follow mode (auto-scroll)
- **t**: Toggle timestamps
- **w**: Toggle word wrap
- **n**: Toggle line numbers
- **c**: Clear logs
- **+/-**: Adjust scroll speed
- **PgUp/PgDn**: Page navigation
- **Home/End**: Jump to start/end

### Shell Access
- **F1**: Toggle between typing and navigation modes
- **F2**: Show/hide detailed help
- **Tab**: Switch between shells (bash, sh, zsh, fish)
- **↑/↓**: Command history (in typing mode)
- **Ctrl+C**: Clear current input
- **Ctrl+L**: Clear output
- **Ctrl+A/E**: Jump to line start/end

### Resource Stats
- **←/→**: Switch between view modes (Overview, Charts, Network, Processes)
- **r**: Reset statistics
- **p**: Pause/resume monitoring
- **+/-**: Adjust update interval

## 🛠️ Configuration

Docsee can be configured via command line arguments:

```bash
docsee --help
```

### Docker Connection
- **Unix Socket**: `--docker-host unix:///var/run/docker.sock` (default)
- **TCP**: `--docker-host tcp://localhost:2375`
- **SSH**: `--docker-host ssh://user@host`

## 🔧 Development

### Building
```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with cargo
cargo run
```

### Project Structure
```
src/
├── app.rs              # Main application logic
├── docker/             # Docker client and API wrappers
├── events/             # Event handling system
├── ui/                 # User interface components
│   ├── containers.rs   # Enhanced container management
│   ├── logs_viewer.rs  # Real-time log streaming
│   ├── shell_executor.rs # Shell access functionality
│   ├── stats_viewer.rs # Resource monitoring
│   └── search_filter.rs # Advanced search/filtering
└── widgets/            # Custom UI widgets
```

## 🎯 Possible Enhancements for Future Versions

### Core Features
- **Docker Compose Support**: Manage multi-container applications
- **Image Building**: Build images from Dockerfiles within the TUI
- **Registry Integration**: Pull/push images to/from registries
- **Container Templates**: Save and reuse container configurations

### Advanced Features
- **Multi-host Support**: Manage multiple Docker hosts simultaneously
- **Backup/Restore**: Export/import container configurations
- **Health Monitoring**: Container health checks and alerts
- **Performance Profiling**: Detailed resource usage analytics

### User Experience
- **Themes**: Customizable color schemes and themes
- **Plugins**: Extensible plugin system for custom functionality
- **Configuration Files**: Persistent settings and preferences
- **Quick Actions**: Keyboard shortcuts for common operations

### Integration
- **Kubernetes Support**: Manage Kubernetes pods and services
- **CI/CD Integration**: Integrate with popular CI/CD pipelines
- **Monitoring Tools**: Integration with Prometheus, Grafana, etc.
- **Notification System**: Alerts for container events

### Advanced Shell Features
- **File Manager**: Built-in file browser for containers
- **Multi-session**: Multiple shell sessions per container
- **Session Persistence**: Save and restore shell sessions
- **Syntax Highlighting**: Enhanced command syntax highlighting

### Logging Enhancements
- **Log Aggregation**: Combine logs from multiple containers
- **Log Export**: Export logs to various formats (JSON, CSV, etc.)
- **Log Analysis**: Built-in log parsing and analysis tools
- **Real-time Filtering**: Advanced log filtering and highlighting

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Guidelines
- Follow Rust best practices
- Add tests for new features
- Update documentation
- Ensure code passes `cargo clippy` and `cargo fmt`

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Ratatui](https://github.com/ratatui-org/ratatui) - Amazing TUI framework for Rust
- Inspired by [k9s](https://github.com/derailed/k9s) - Kubernetes TUI
- Docker integration via [Bollard](https://github.com/fussybeaver/bollard) - Docker API client

## 🐛 Bug Reports

If you encounter any bugs or issues, please create an issue on GitHub with:
- Your operating system and version
- Docker version
- Steps to reproduce the issue
- Expected vs actual behavior
- Any error messages

## 🌟 Show Your Support

If you find Docsee useful, please consider:
- ⭐ Starring the repository
- 🍴 Forking the project
- 📢 Sharing with others
- 🐛 Reporting bugs
- 💡 Suggesting features

---

**Happy Docker Management with Docsee! 🦆**
