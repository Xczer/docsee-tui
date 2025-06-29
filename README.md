# 🦆 Docsee - Docker Management TUI

```
██████╗  ██████╗  ██████╗███████╗███████╗███████╗
██╔══██╗██╔═══██╗██╔════╝██╔════╝██╔════╝██╔════╝
██║  ██║██║   ██║██║     ███████╗█████╗  █████╗
██║  ██║██║   ██║██║     ╚════██║██╔══╝  ██╔══╝
██████╔╝╚██████╔╝╚██████╗███████║███████╗███████╗
╚═════╝  ╚═════╝  ╚═════╝╚══════╝╚══════╝╚══════╝
            🦆 Docker Management TUI
```

A beautiful, feature-rich Docker management terminal user interface built with Rust and Ratatui. Manage Docker containers, images, volumes, and networks with an intuitive interface featuring real-time logging, shell access, and resource monitoring.

[![CI](https://github.com/Xczer/docsee-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/Xczer/docsee-tui/actions/workflows/ci.yml)
[![Release](https://github.com/Xczer/docsee-tui/actions/workflows/release.yml/badge.svg)](https://github.com/Xczer/docsee-tui/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 📸 Screenshots

![Docsee Container Management](docs/screenshots/containers.png)
*Container management with real-time status indicators*

![Docsee Logs Viewer](docs/screenshots/logs.png)
*Real-time log streaming with filtering*

![Docsee Shell Access](docs/screenshots/shell.png)
*Interactive shell access within containers*

## ✨ Features

### 🏠 Core Management
- **Container Management**: Start, stop, restart, delete containers with visual status indicators
- **Image Management**: View, delete, and prune Docker images with detailed information
- **Volume Management**: Manage Docker volumes with usage tracking
- **Network Management**: Handle Docker networks with connection details

### 🚀 Advanced Features
- **Real-time Logs**: Stream container logs with timestamps, word wrap, and filtering
- **Shell Access**: Execute commands in containers with full terminal support
- **Resource Monitoring**: Live CPU, memory, network, and disk usage statistics
- **Advanced Search**: Powerful filtering and search across all Docker resources
- **Intuitive Navigation**: Keyboard-driven interface with helpful shortcuts

### 🎨 User Experience
- **Beautiful Interface**: ASCII art and clean visual design
- **Responsive Design**: Adapts to different terminal sizes
- **Status Feedback**: Clear visual indicators for all operations
- **Help System**: Comprehensive keyboard shortcuts guide
- **Multi-platform**: Works on Linux, macOS, and Windows

## 🚀 Installation

### 📦 Pre-built Binaries

Download the latest release for your platform:

```bash
# Linux x86_64
curl -L https://github.com/Xczer/docsee-tui/releases/latest/download/docsee-linux-x86_64 -o docsee
chmod +x docsee

# Linux ARM64
curl -L https://github.com/Xczer/docsee-tui/releases/latest/download/docsee-linux-aarch64 -o docsee
chmod +x docsee

# macOS Intel
curl -L https://github.com/Xczer/docsee-tui/releases/latest/download/docsee-macos-x86_64 -o docsee
chmod +x docsee

# macOS Apple Silicon
curl -L https://github.com/Xczer/docsee-tui/releases/latest/download/docsee-macos-aarch64 -o docsee
chmod +x docsee

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/Xczer/docsee-tui/releases/latest/download/docsee-windows-x86_64.exe" -OutFile "docsee.exe"
```

### 🦀 From Crates.io

```bash
cargo install docsee
```

### 🔨 From Source

```bash
# Clone and build
git clone https://github.com/Xczer/docsee-tui.git
cd docsee-tui
cargo build --release

# Install globally
cargo install --path .
```

### 🐧 Package Managers

```bash
# Homebrew (macOS/Linux)
brew install docsee

# Arch Linux (AUR)
yay -S docsee

# Ubuntu/Debian (coming soon)
# apt install docsee
```

## 🎮 Quick Start

### Prerequisites
- Docker installed and running
- Terminal with color support
- Minimum terminal size: 80x24

### Basic Usage

```bash
# Start with default Docker socket
docsee

# Connect to remote Docker host
docsee --docker-host tcp://remote-host:2375

# Connect via SSH
docsee --docker-host ssh://user@remote-host
```

## 📋 Keyboard Shortcuts

### Global Navigation
| Key | Action |
|-----|--------|
| `←/→` | Switch tabs |
| `↑/↓` | Navigate lists |
| `Enter` | Select item |
| `Esc` | Go back |
| `q` | Quit |
| `c` | Help/cheatsheet |

### Container Management
| Key | Action |
|-----|--------|
| `u` | Start container |
| `d` | Stop container |
| `r` | Restart container |
| `D` | Delete container |
| `l` | View logs |
| `e` | Shell access |
| `s` | Resource stats |
| `/` | Search/filter |

### Logs Viewer
| Key | Action |
|-----|--------|
| `f` | Toggle follow mode |
| `t` | Toggle timestamps |
| `w` | Toggle word wrap |
| `c` | Clear logs |
| `+/-` | Scroll speed |
| `PgUp/PgDn` | Page navigation |

### Shell Access
| Key | Action |
|-----|--------|
| `F1` | Toggle input mode |
| `Tab` | Switch shell type |
| `↑/↓` | Command history |
| `Ctrl+C` | Clear input |
| `Ctrl+L` | Clear output |

## 🔧 Configuration

### Docker Connection

Docsee supports various Docker connection methods:

```bash
# Unix socket (default)
docsee --docker-host unix:///var/run/docker.sock

# TCP connection
docsee --docker-host tcp://localhost:2375

# TLS connection
docsee --docker-host tcp://localhost:2376

# SSH connection
docsee --docker-host ssh://user@host
```

### Environment Variables

```bash
# Set default Docker host
export DOCKER_HOST=tcp://localhost:2375

# Enable Docker TLS
export DOCKER_TLS_VERIFY=1
export DOCKER_CERT_PATH=/path/to/certs
```

## 🏗️ Architecture

Docsee is built with modern Rust practices:

- **Async/Await**: Non-blocking Docker API operations
- **TUI Framework**: Ratatui for terminal interface
- **Event-driven**: Responsive keyboard and timer events
- **Modular Design**: Separated concerns for maintainability
- **Error Handling**: Comprehensive error management
- **Cross-platform**: Works on all major operating systems

### Project Structure

```
src/
├── app.rs              # Main application logic
├── docker/             # Docker API client and operations
├── events/             # Event handling system
├── ui/                 # User interface components
└── widgets/            # Reusable UI components
```

## 🧪 Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/Xczer/docsee-tui.git
cd docsee-tui

# Build debug version
cargo build

# Build release version
cargo build --release

# Run tests
cargo test

# Run with cargo
cargo run -- --docker-host unix:///var/run/docker.sock
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy

# Check for security vulnerabilities
cargo audit
```

## 🤝 Contributing

Contributions are welcome! Here's how to get started:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'Add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

### Guidelines

- Follow Rust best practices and idioms
- Add tests for new features
- Update documentation as needed
- Ensure code passes `cargo clippy` and `cargo fmt`
- Write clear commit messages

## 🐛 Troubleshooting

### Common Issues

**Connection Error**
```bash
# Check if Docker is running
docker info

# Verify Docker socket permissions
ls -la /var/run/docker.sock

# Try with sudo (not recommended for production)
sudo docsee
```

**Terminal Display Issues**
```bash
# Ensure terminal supports colors
echo $TERM

# Try with explicit terminal type
TERM=xterm-256color docsee

# Check terminal size
echo $COLUMNS x $LINES
```

**Performance Issues**
```bash
# Run with reduced update frequency
docsee --refresh-rate 2000  # 2 second intervals
```

### Getting Help

- 📚 **Documentation**: Check this README and help system (`c` key)
- 🐛 **Bug Reports**: [Open an issue](https://github.com/Xczer/docsee-tui/issues)
- 💬 **Discussions**: [GitHub Discussions](https://github.com/Xczer/docsee-tui/discussions)
- 📧 **Contact**: xczermax@gmail.com

## 📊 Roadmap

### Current Version (v1.0)
- ✅ Container management
- ✅ Real-time logs
- ✅ Shell access
- ✅ Resource monitoring
- ✅ Advanced search

### Future Versions
- 🔄 Docker Compose support
- 🔄 Image building interface
- 🔄 Registry integration
- 🔄 Themes and customization
- 🔄 Plugin system

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🌟 Acknowledgments

- **[Ratatui](https://github.com/ratatui-org/ratatui)** - Excellent TUI framework for Rust
- **[Bollard](https://github.com/fussybeaver/bollard)** - Docker API client for Rust
- **[Tokio](https://tokio.rs/)** - Async runtime for Rust
- **[k9s](https://github.com/derailed/k9s)** - Inspiration for Kubernetes TUI design

## 💝 Support

If you find Docsee useful, please consider:

- ⭐ **Starring** the repository
- 🐛 **Reporting** bugs and issues
- 💡 **Suggesting** new features
- 🔄 **Contributing** code improvements
- 📢 **Sharing** with others

---

**Happy Docker Management! 🦆**

Built with ❤️ in Rust
