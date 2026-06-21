# Fusebox Makefile
# Simplifies common development tasks

.PHONY: help build test lint fmt clean run dev install docker docker-compose k8s test-integration

# Default target
help:
	@echo "Fusebox - Available Commands:"
	@echo ""
	@echo "Development:"
	@echo "  make build              - Build release binary"
	@echo "  make dev                - Build and run in dev mode"
	@echo "  make test               - Run all unit tests"
	@echo "  make test-integration   - Run integration tests"
	@echo "  make lint               - Run clippy"
	@echo "  make fmt                - Format code"
	@echo "  make clean              - Clean build artifacts"
	@echo ""
	@echo "Deployment:"
	@echo "  make docker             - Build Docker image"
	@echo "  make docker-compose     - Start with docker-compose"
	@echo "  make k8s                - Deploy to Kubernetes"
	@echo ""
	@echo "Installation:"
	@echo "  make install            - Install fusebox binary"
	@echo ""

# Build release binary
build:
	@echo "🔨 Building release binary..."
	cargo build --release --workspace

# Build and run in dev mode
dev:
	@echo "🚀 Starting Fusebox in dev mode..."
	cargo run --bin fusebox-proxy

# Run all unit tests
test:
	@echo "🧪 Running unit tests..."
	cargo test --workspace --all-features

# Run integration tests
test-integration:
	@echo "🧪 Running integration tests..."
	@chmod +x tests/integration_test.sh tests/integration_test.py
	@./tests/integration_test.sh
	@python3 tests/integration_test.py || true

# Run clippy
lint:
	@echo "🔍 Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

# Format code
fmt:
	@echo "✨ Formatting code..."
	cargo fmt --all

# Check formatting
fmt-check:
	@echo "🔍 Checking code formatting..."
	cargo fmt --all -- --check

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf .fusebox/
	rm -rf target/
	rm -rf packages/mcp-server/dist/
	rm -rf packages/mcp-server/node_modules/

# Install binary to system
install:
	@echo "📦 Installing fusebox..."
	cargo install --path crates/fusebox-cli

# Build Docker image
docker:
	@echo "🐳 Building Docker image..."
	docker build -t fusebox:latest .

# Start with docker-compose
docker-compose:
	@echo "🐳 Starting with docker-compose..."
	docker-compose up -d

# Stop docker-compose
docker-compose-down:
	@echo "🐳 Stopping docker-compose..."
	docker-compose down

# Deploy to Kubernetes
k8s:
	@echo "☸️  Deploying to Kubernetes..."
	kubectl apply -f k8s-deployment.yaml

# Run security audit
audit:
	@echo "🔒 Running security audit..."
	cargo audit

# Generate documentation
docs:
	@echo "📖 Generating documentation..."
	cargo doc --workspace --no-deps --open

# Run benchmarks
bench:
	@echo "⚡ Running benchmarks..."
	cargo bench --workspace

# Check everything (used in CI)
ci: fmt-check lint test
	@echo "✅ All checks passed!"

# MCP Server tasks
mcp-install:
	@echo "📦 Installing MCP Server dependencies..."
	cd packages/mcp-server && npm install

mcp-build:
	@echo "🔨 Building MCP Server..."
	cd packages/mcp-server && npm run build

mcp-dev:
	@echo "🚀 Starting MCP Server in dev mode..."
	cd packages/mcp-server && npm run dev

# Quick start for new users
quickstart: build
	@echo "🎉 Fusebox is ready!"
	@echo ""
	@echo "Next steps:"
	@echo "  1. Copy fusebox.yaml.example to fusebox.yaml"
	@echo "  2. Add your API keys to environment variables"
	@echo "  3. Run: make dev"
	@echo ""
	@echo "See README.md for detailed instructions."

# Database migrations (if needed)
migrate:
	@echo "🗄️  Running database migrations..."
	@# Future: Add migration commands

# Pricing sync
pricing-sync:
	@echo "💰 Syncing pricing data..."
	python3 scripts/sync-pricing.py

# Watch for changes and rebuild
watch:
	@echo "👀 Watching for changes..."
	cargo watch -x 'run --bin fusebox-proxy'

# Print version
version:
	@cargo run --bin fusebox-proxy -- --version
