# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Integration tests for circuit breaker
- Performance benchmarks
- Additional provider support

## [0.1.0] - 2026-06-21

### Added
- 🎉 Initial release of Fusebox
- ⚡ Real-time circuit breaker with three-state machine (Closed/Open/HalfOpen)
- 💰 Multi-window budget support (1m/1h/1d/1w/1mo)
- 📈 Anomaly detection using EWMA algorithm
- 🔌 Multi-provider support (OpenAI, Anthropic, Google, Bedrock, OpenRouter)
- 🧮 Precise token-level cost calculation with tiktoken-rs
- 💾 Dual backend persistence (SQLite + PostgreSQL)
- 📝 Budget request workflow with full persistence
- 🤖 MCP Server for AI Agent self-monitoring (industry first!)
- 🛠️ Full-featured CLI tool
- 📊 Prometheus metrics export
- 🐳 Docker and Kubernetes deployment configs
- 📖 Comprehensive documentation (English + Chinese)

### Core Features
- Policy engine with budget enforcement
- Circuit breaker with automatic recovery
- Ledger for audit trails and cost tracking
- HTTP proxy compatible with OpenAI and Anthropic APIs
- Tenant isolation for multi-tenant deployments
- Streaming response support with usage reconciliation

### Persistence
- SQLite backend for zero-config deployments
- PostgreSQL backend for enterprise deployments
- Full schema migrations
- Budget request tracking
- Breaker state transitions
- Spend event logging

### MCP Server
- `get_budget` - Check current budget configuration
- `get_spend` - View spending in time windows
- `get_breaker` - Check circuit breaker state
- `request_budget_increase` - Request budget increases
- Claude Desktop and Cursor integration examples
- 5 real-world usage scenarios documented

### CLI Commands
- `fusebox start` - Start the proxy server
- `fusebox status` - Check tenant status
- `fusebox doctor` - Health diagnostics
- `fusebox tail` - Real-time event stream
- `fusebox breaker` - Circuit breaker management
- `fusebox budget` - Budget configuration
- `fusebox pricing` - Pricing data management

### Documentation
- Complete README with quick start guide
- Architecture documentation (架构.md)
- Technical documentation (技术.md)
- Development guide (DEVELOPMENT_SUMMARY.md)
- Test report (TEST_REPORT.md)
- Chinese examples (examples/README_CN.md)
- MCP Server documentation and examples
- Contributing guidelines
- Security policy

### Testing
- 60 unit tests across all crates
- SQLite and PostgreSQL backend tests
- CLI integration tests
- API endpoint tests
- 100% test pass rate

### Deployment
- Dockerfile for containerized deployment
- docker-compose.yml with full stack (Postgres, Prometheus, Grafana)
- Kubernetes manifests (k8s-deployment.yaml)
- CI/CD workflows (GitHub Actions)
- Environment configuration templates

### Pricing Data
- 595+ model pricing configurations
- 5 provider pricing files (OpenAI, Anthropic, Google, Bedrock, OpenRouter)
- Automatic pricing sync script

[Unreleased]: https://github.com/fusebox-dev/fusebox/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/fusebox-dev/fusebox/releases/tag/v0.1.0
