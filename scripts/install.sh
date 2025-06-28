#!/usr/bin/env bash

# Installation script for docsee-tui
# This script downloads and installs the latest release of docsee

set -e

# Configuration
REPO="Xczer/docsee"
BINARY_NAME="docsee"
INSTALL_DIR="/usr/local/bin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Detect platform
detect_platform() {
    local platform
    case "$(uname -s)" in
        Darwin)
            case "$(uname -m)" in
                x86_64) platform="macos-x86_64" ;;
                arm64) platform="macos-aarch64" ;;
                *) print_error "Unsupported macOS architecture: $(uname -m)"; exit 1 ;;
            esac
            ;;
        Linux)
            case "$(uname -m)" in
                x86_64) platform="linux-x86_64" ;;
                aarch64) platform="linux-aarch64" ;;
                *) print_error "Unsupported Linux architecture: $(uname -m)"; exit 1 ;;
            esac
            ;;
        CYGWIN*|MINGW32*|MSYS*|MINGW*)
            platform="windows-x86_64.exe"
            BINARY_NAME="docsee.exe"
            ;;
        *)
            print_error "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac
    echo "$platform"
}

# Get latest release version
get_latest_version() {
    print_info "Fetching latest release information..."
    local version
    version=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed 's/.*"tag_name": "\([^"]*\)".*/\1/')
    if [ -z "$version" ]; then
        print_error "Failed to fetch latest version"
        exit 1
    fi
    echo "$version"
}

# Download and install
install_docsee() {
    local platform="$1"
    local version="$2"
    local download_url="https://github.com/$REPO/releases/download/$version/${BINARY_NAME}-$platform"
    local temp_file="/tmp/${BINARY_NAME}-$platform"

    print_info "Downloading docsee $version for $platform..."
    
    if ! curl -L "$download_url" -o "$temp_file"; then
        print_error "Failed to download docsee"
        exit 1
    fi

    # Make executable
    chmod +x "$temp_file"

    # Install
    print_info "Installing to $INSTALL_DIR..."
    
    if [ -w "$INSTALL_DIR" ]; then
        mv "$temp_file" "$INSTALL_DIR/$BINARY_NAME"
    else
        print_info "Need sudo privileges to install to $INSTALL_DIR"
        sudo mv "$temp_file" "$INSTALL_DIR/$BINARY_NAME"
    fi

    print_success "docsee installed successfully!"
}

# Verify installation
verify_installation() {
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        local installed_version
        installed_version=$("$BINARY_NAME" --version 2>/dev/null | head -n1 || echo "unknown")
        print_success "Installation verified: $installed_version"
        print_info "You can now run: $BINARY_NAME"
    else
        print_warning "Installation may have failed. Try running: $INSTALL_DIR/$BINARY_NAME"
    fi
}

# Check prerequisites
check_prerequisites() {
    print_info "Checking prerequisites..."
    
    # Check for curl
    if ! command -v curl >/dev/null 2>&1; then
        print_error "curl is required but not installed."
        exit 1
    fi

    # Check for Docker
    if ! command -v docker >/dev/null 2>&1; then
        print_warning "Docker is not installed or not in PATH."
        print_warning "docsee requires Docker to function properly."
        print_info "Install Docker from: https://docs.docker.com/get-docker/"
    else
        print_success "Docker found: $(docker --version | head -n1)"
    fi
}

# Main installation function
main() {
    echo "🦆 docsee-tui Installation Script"
    echo "=================================="
    echo

    # Check prerequisites
    check_prerequisites
    echo

    # Detect platform
    local platform
    platform=$(detect_platform)
    print_info "Detected platform: $platform"

    # Get latest version
    local version
    if [ -n "$VERSION" ]; then
        version="$VERSION"
        print_info "Using specified version: $version"
    else
        version=$(get_latest_version)
        print_info "Latest version: $version"
    fi
    echo

    # Install
    install_docsee "$platform" "$version"
    echo

    # Verify
    verify_installation
    echo

    print_success "Installation complete!"
    echo
    print_info "Quick start:"
    echo "  $BINARY_NAME                    # Run with default Docker socket"
    echo "  $BINARY_NAME --help             # Show help"
    echo "  $BINARY_NAME --docker-host tcp://localhost:2375  # Connect via TCP"
    echo
    print_info "Troubleshooting:"
    echo "  - Make sure Docker is running"
    echo "  - Check Docker socket permissions"
    echo "  - Try: docker ps (should work without errors)"
    echo
    print_info "Documentation: https://github.com/$REPO"
    echo "🦆 Happy Docker management!"
}

# Handle command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --install-dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --help)
            echo "docsee-tui Installation Script"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --install-dir DIR    Install to specific directory (default: /usr/local/bin)"
            echo "  --version VERSION    Install specific version (default: latest)"
            echo "  --help              Show this help"
            echo ""
            echo "Examples:"
            echo "  $0                          # Install latest to /usr/local/bin"
            echo "  $0 --install-dir ~/.local/bin  # Install to user directory"
            echo "  $0 --version v1.0.0         # Install specific version"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Run main installation
main
