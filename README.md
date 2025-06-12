# 🦆 Docsee - Docker TUI Manager

A beautiful terminal user interface for managing Docker containers, images, volumes, and networks. Built with Rust and Ratatui.

![Demo](https://via.placeholder.com/800x400?text=Docsee+Demo)

## ✨ Features

### 🐳 **Containers Management**
- **View all containers** - Both running and stopped
- **Container operations** - Start, stop, restart, delete
- **Real-time status** - Color-coded status indicators
- **Quick actions** - Keyboard shortcuts for common tasks
- **Safety checks** - Prevents dangerous operations

### 🖼️ **Images Management**
- **Browse images** - All local Docker images
- **Image operations** - Delete images, prune unused
- **Size information** - Human-readable size formatting
- **Dangling detection** - Identify untagged images
- **Space cleanup** - Prune to reclaim disk space

### 💾 **Volumes Management** ⭐ *NEW*
- **Volume overview** - All Docker volumes with usage info
- **Usage tracking** - See which volumes are in use
- **Size monitoring** - Track volume disk usage
- **Safe deletion** - Warnings for volumes in use
- **Cleanup tools** - Prune unused volumes to save space

### 🌐 **Networks Management** ⭐ *NEW*
- **Network topology** - View all Docker networks
- **Network details** - Driver, scope, subnet information
- **Container connections** - See which containers are connected
- **Network types** - Distinguish between internal, ingress, and external
- **Safe operations** - Prevents deletion of networks with connected containers

### 🎨 **User Experience**
- **Intuitive navigation** - Tab-based interface with arrow keys
- **Color-coded status** - Visual indicators for resource states
- **Comprehensive help** - Built-in cheatsheet (press `c`)
- **Error handling** - Graceful error messages and recovery
- **Responsive design** - Adapts to terminal size

## 🚀 Quick Start

### Prerequisites
- **Rust** (1.70+) - [Install Rust](https://rustup.rs/)
- **Docker** - [Install Docker](https://docs.docker.com/get-docker/)
- **Terminal** - Any modern terminal emulator

### Installation

```bash
# Clone the repository
git clone https://github.com/Xczer/docsee.git
cd docsee

# Build and install
make install

# Or build manually
cargo build --release
cargo install --path .
```

### Running

```bash
# Run with default Docker socket
docsee

# Specify custom Docker host
docsee --docker-host unix:///var/run/docker.sock
docsee --docker-host tcp://localhost:2375
```

## 🎮 Usage

### Global Controls
- **`←/→`** - Switch between tabs
- **`↑/↓`** - Navigate within current tab
- **`c`** - Show/hide cheatsheet
- **`q`** - Quit application

### Container Tab
- **`u`** - Start selected container
- **`d`** - Stop selected container
- **`r`** - Restart selected container
- **`D`** - Delete selected container (if stopped)
- **`l`** - View logs *(coming soon)*
- **`e`** - Execute shell *(coming soon)*

### Images Tab
- **`D`** - Delete selected image
- **`p`** - Prune unused images

### Volumes Tab ⭐ *NEW*
- **`D`** - Delete selected volume (with safety checks)
- **`p`** - Prune unused volumes

### Networks Tab ⭐ *NEW*
- **`D`** - Delete selected network (with safety checks)
- **`p`** - Prune unused networks

## 🏗️ Architecture

### Project Structure
```
docsee/
├── src
│   ├── docker
│   │   ├── client.rs
│   │   ├── containers.rs
│   │   ├── images.rs
│   │   ├── mod.rs
│   │   ├── networks.rs
│   │   └── volumes.rs
│   ├── events
│   │   ├── handler.rs
│   │   ├── key.rs
│   │   └── mod.rs
│   ├── ui
│   │   ├── cheatsheet.rs
│   │   ├── containers.rs
│   │   ├── images.rs
│   │   ├── mod.rs
│   │   ├── networks.rs
│   │   ├── tabs.rs
│   │   └── volumes.rs
│   ├── widgets
│   │   ├── mod.rs
│   │   ├── modal.rs
│   │   └── table.rs
│   ├── app.rs
│   ├── lib.rs
│   └── main.rs
├── Cargo.lock
├── Cargo.toml
├── IMPLEMENTATION_SUMMARY.md
├── Makefile
└── README.md
```

### Key Technologies
- **[Ratatui](https://ratatui.rs/)** - Terminal UI framework
- **[Bollard](https://docs.rs/bollard/)** - Docker API client
- **[Crossterm](https://docs.rs/crossterm/)** - Cross-platform terminal
- **[Tokio](https://tokio.rs/)** - Async runtime

### Design Patterns
- **Component-based UI** - Modular, reusable components
- **Event-driven** - Async event handling with clean separation
- **Error resilience** - Comprehensive error handling and recovery
- **Type safety** - Rust's type system prevents runtime errors

## 🛠️ Development

### Building
```bash
# Development build
make build

# Release build
make release

# Run in development
make run
```

### Code Quality
```bash
# Format code
make fmt

# Lint code
make clippy

# All checks
make dev-check
```

## 🔧 Configuration

### Command Line Options
```bash
docsee --help

Options:
  --docker-host <HOST>  Docker host URL
  -h, --help           Print help
  -V, --version        Print version
```

### Environment Variables
- **`DOCKER_HOST`** - Docker daemon socket (default: `unix:///var/run/docker.sock`)

## 🎯 Roadmap

### Phase 1: Core Features ✅ *COMPLETED*
- [x] Container management (start, stop, restart, delete)
- [x] Image management (delete, prune)
- [x] Volume management (list, delete, prune) ⭐ *NEW*
- [x] Network management (list, delete, prune) ⭐ *NEW*
- [x] Tab navigation and help system

### Phase 2: Enhanced Features *(In Progress)*
- [ ] **Container logs viewer** - Real-time log streaming
- [ ] **Shell execution** - Interactive terminal access
- [ ] **Container stats** - CPU, memory, network monitoring
- [ ] **Search and filtering** - Find resources quickly

### Phase 3: Advanced Features *(Planned)*
- [ ] **Docker Compose** - Manage multi-container applications
- [ ] **Registry integration** - Pull/push images
- [ ] **Resource creation** - Create volumes, networks, containers
- [ ] **Export/import** - Backup and restore configurations

### Phase 4: Enterprise Features *(Future)*
- [ ] **Remote Docker hosts** - Manage multiple Docker daemons
- [ ] **Kubernetes support** - Extend to K8s resources
- [ ] **Team collaboration** - Shared configurations
- [ ] **Advanced monitoring** - Performance metrics and alerts

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Make** your changes
4. **Test** thoroughly (`make test`)
5. **Commit** your changes (`git commit -m 'Add amazing feature'`)
6. **Push** to the branch (`git push origin feature/amazing-feature`)
7. **Open** a Pull Request

### Development Guidelines
- Follow Rust conventions and use `rustfmt`
- Add tests for new functionality
- Update documentation for user-facing changes
- Keep commits focused and descriptive

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **[Ratatui](https://ratatui.rs/)** - Excellent TUI framework
- **[Bollard](https://docs.rs/bollard/)** - Comprehensive Docker API
- **[k9s](https://k9scli.io/)** - Inspiration for TUI design
- **Docker Community** - For the amazing container platform

## 📞 Support

- **Issues** - [GitHub Issues](https://github.com/Xczer/docsee/issues)
- **Discussions** - [GitHub Discussions](https://github.com/Xczer/docsee/discussions)
- **Documentation** - Check the built-in cheatsheet (press `c`)

---

<div align="center">

**⭐ If you find Docsee useful, please consider giving it a star on GitHub! ⭐**

Made with ❤️ and 🦀 Rust

</div>
