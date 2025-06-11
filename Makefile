# Docsee - Docker TUI Manager
# Simple Makefile for common development tasks

.PHONY: build run test clean check fmt clippy install help

# Default target
help:
	@echo "Docsee - Docker TUI Manager"
	@echo ""
	@echo "Available commands:"
	@echo "  build     - Build the application"
	@echo "  run       - Run the application"
	@echo "  test      - Run tests"
	@echo "  check     - Run cargo check"
	@echo "  fmt       - Format code"
	@echo "  clippy    - Run clippy linter"
	@echo "  clean     - Clean build artifacts"
	@echo "  install   - Install the binary"
	@echo "  help      - Show this help"

# Build the application
build:
	@echo "🔨 Building Docsee..."
	cargo build --release

# Run the application in development mode
run:
	@echo "🚀 Running Docsee..."
	cargo run

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test

# Check code without building
check:
	@echo "🔍 Checking code..."
	cargo check

# Format code
fmt:
	@echo "🎨 Formatting code..."
	cargo fmt

# Run clippy linter
clippy:
	@echo "📎 Running clippy..."
	cargo clippy -- -D warnings

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean

# Install the binary to ~/.cargo/bin
install: build
	@echo "📦 Installing Docsee..."
	cargo install --path .

# Development workflow - check everything
dev-check: fmt clippy test
	@echo "✅ All development checks passed!"

# Release build with optimizations
release: clean
	@echo "🏗️ Building release version..."
	cargo build --release --locked
	@echo "✅ Release build complete: target/release/docsee"
