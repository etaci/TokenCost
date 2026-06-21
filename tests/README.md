# Integration Tests

This directory contains integration tests for Fusebox that test the complete system end-to-end.

## Test Scripts

### Bash Integration Tests
```bash
# Run integration tests
./tests/integration_test.sh
```

Tests included:
- Health endpoint
- Breaker state API
- Spend tracking API
- Prometheus metrics
- OpenAI proxy structure
- Anthropic proxy structure
- Budget request workflow
- Events API

### Python Integration Tests
```bash
# Install dependencies
pip install requests

# Run tests
python3 tests/integration_test.py
```

Additional Python tests:
- Programmatic API testing
- Response validation
- Error handling

## Running Tests

### Prerequisites
- Fusebox binary built (`cargo build --release`)
- No other Fusebox instance running on port 8080
- Internet connection (for upstream API structure tests)

### Quick Run
```bash
# Run all integration tests
make test-integration

# Or manually
./tests/integration_test.sh
python3 tests/integration_test.py
```

## Test Coverage

### API Endpoints Tested
- [x] `/health` - Health check
- [x] `/metrics` - Prometheus metrics
- [x] `/v1/breaker/state` - Circuit breaker state
- [x] `/v1/breaker/reset` - Reset breaker
- [x] `/v1/spend` - Spending query
- [x] `/v1/events` - Event stream
- [x] `/v1/budget/requests` - Budget requests CRUD
- [x] `/v1/chat/completions` - OpenAI compatibility
- [x] `/v1/messages` - Anthropic compatibility

### Scenarios Tested
- [x] Server startup and health
- [x] Budget enforcement
- [x] Circuit breaker transitions
- [x] Multi-tenant isolation
- [x] API proxy forwarding
- [x] Metrics collection
- [x] Budget request workflow

## Writing New Tests

### Bash Tests
```bash
run_test "My test name" \
    "curl -sf '${FUSEBOX_URL}/my-endpoint' | grep -q 'expected'"
```

### Python Tests
```python
def test_my_feature(self):
    """Test my feature"""
    response = requests.get(f"{FUSEBOX_URL}/my-endpoint")
    assert response.status_code == 200
    assert "expected" in response.json()
```

## CI Integration

These tests run automatically in CI:
```yaml
- name: Run integration tests
  run: |
    ./tests/integration_test.sh
    python3 tests/integration_test.py
```

## Troubleshooting

### Port already in use
```bash
# Kill existing Fusebox
pkill -f fusebox-proxy
```

### Tests timing out
```bash
# Check server logs
RUST_LOG=debug ./tests/integration_test.sh
```

### Database locked
```bash
# Clean up test databases
rm -rf .fusebox/test-*.db
```
