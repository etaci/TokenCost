# Contributing to Fusebox

Thank you for your interest in contributing to Fusebox! 🎉

## 🤝 Code of Conduct

We are committed to providing a welcoming and inclusive experience for everyone. Please be respectful and constructive in all interactions.

## 🚀 Getting Started

### Prerequisites

- Rust 1.78 or higher
- Node.js 18+ (for MCP Server)
- Git
- SQLite or PostgreSQL (optional)

### Setup Development Environment

```bash
# Clone the repository
git clone https://github.com/fusebox-dev/fusebox.git
cd fusebox

# Build the project
cargo build

# Run tests
cargo test --workspace

# Start the proxy in dev mode
cargo run --bin fusebox-proxy
```

## 📋 How to Contribute

### Reporting Bugs

Found a bug? Please open an issue with:

- **Clear title** - Summarize the problem
- **Steps to reproduce** - How can we trigger the bug?
- **Expected behavior** - What should happen?
- **Actual behavior** - What actually happens?
- **Environment** - OS, Rust version, etc.

### Suggesting Features

Have an idea? Open an issue with:

- **Use case** - Why do you need this feature?
- **Proposed solution** - How should it work?
- **Alternatives** - What other approaches did you consider?

### Submitting Pull Requests

1. **Fork the repository**
2. **Create a feature branch**
   ```bash
   git checkout -b feature/my-awesome-feature
   ```

3. **Make your changes**
   - Write clear, idiomatic Rust code
   - Follow existing code style
   - Add tests for new functionality
   - Update documentation

4. **Run the test suite**
   ```bash
   cargo test --workspace
   cargo clippy --all-targets --all-features
   cargo fmt -- --check
   ```

5. **Commit your changes**
   ```bash
   git commit -m "feat: add my awesome feature"
   ```
   
   Follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` - New feature
   - `fix:` - Bug fix
   - `docs:` - Documentation changes
   - `test:` - Test additions or changes
   - `refactor:` - Code refactoring
   - `perf:` - Performance improvements

6. **Push to your fork**
   ```bash
   git push origin feature/my-awesome-feature
   ```

7. **Open a Pull Request**
   - Describe your changes clearly
   - Link related issues
   - Add screenshots if applicable

## 🏗️ Project Structure

```
fusebox/
├── crates/
│   ├── fusebox-core/      # Core types and traits
│   ├── fusebox-policy/    # Policy engine + circuit breaker
│   ├── fusebox-ledger/    # Persistence layer
│   ├── fusebox-proxy/     # HTTP gateway
│   └── fusebox-cli/       # Command-line interface
├── packages/
│   └── mcp-server/        # MCP Server (TypeScript)
├── pricing/               # Model pricing data
├── examples/              # Integration examples
└── docs/                  # Documentation
```

## 🧪 Testing Guidelines

### Writing Tests

- **Unit tests** - Test individual functions/modules
- **Integration tests** - Test component interactions
- **Documentation tests** - Test code examples in docs

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_check() {
        let policy = Policy::default();
        let result = policy.check_budget("tenant", 10.0);
        assert!(result.is_ok());
    }
}
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p fusebox-policy

# With output
cargo test -- --nocapture

# Specific test
cargo test test_budget_check
```

## 📖 Documentation

### Code Documentation

- Add doc comments to public items
- Include examples where helpful
- Use `cargo doc --open` to preview

Example:
```rust
/// Checks if a request is within budget.
///
/// # Arguments
/// * `tenant` - The tenant identifier
/// * `cost_usd` - Estimated cost in USD
///
/// # Returns
/// * `Ok(())` if within budget
/// * `Err(PolicyError)` if over budget
///
/// # Examples
/// ```
/// let policy = Policy::default();
/// policy.check_budget("my-app", 5.0)?;
/// ```
pub fn check_budget(&self, tenant: &str, cost_usd: f64) -> Result<()> {
    // ...
}
```

## 🎯 Areas We Need Help

### High Priority
- [ ] Integration tests for circuit breaker
- [ ] End-to-end tests for streaming
- [ ] Performance benchmarks
- [ ] Additional provider integrations

### Medium Priority
- [ ] Web dashboard (Next.js)
- [ ] TypeScript SDK
- [ ] Python SDK
- [ ] Grafana dashboard templates

### Documentation
- [ ] More usage examples
- [ ] Video tutorials
- [ ] Architecture deep-dives
- [ ] Deployment guides

## 🔍 Code Review Process

All submissions require review. We'll look for:

- ✅ **Correctness** - Does it work as intended?
- ✅ **Tests** - Are there adequate tests?
- ✅ **Documentation** - Is it well-documented?
- ✅ **Style** - Does it follow project conventions?
- ✅ **Performance** - Any performance concerns?

## 💬 Communication

- **GitHub Issues** - Bug reports and feature requests
- **GitHub Discussions** - Questions and ideas
- **Discord** - Real-time chat (coming soon)
- **Twitter/X** - Updates and announcements

## 📜 License

By contributing to Fusebox, you agree that your contributions will be licensed under the Apache License 2.0.

## 🙏 Recognition

All contributors will be recognized in our README and release notes. Thank you for making Fusebox better! ❤️

---

Questions? Open an issue or reach out to hello@fusebox.dev
