# Fusebox Performance Benchmarks

This crate contains performance benchmarks for Fusebox using Criterion.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench --package fusebox-bench

# Run specific benchmark
cargo bench --package fusebox-bench --bench policy_bench

# Run with HTML reports
cargo bench --package fusebox-bench -- --verbose
```

## Benchmark Suites

### Policy Benchmarks (`policy_bench.rs`)
- **Budget Check** - Measures policy.pre_flight() with various cost amounts
- **Breaker Check** - Measures breaker state lookup performance
- **Anomaly Detection** - Measures EWMA anomaly detection overhead
- **Concurrent Requests** - Tests scalability with multiple tenants

### Ledger Benchmarks (`ledger_bench.rs`)
- **Write Event** - Single event write performance
- **Read Spend** - Spend aggregation across time windows
- **Concurrent Writes** - Batch write performance

### Breaker Benchmarks (`breaker_bench.rs`)
- **State Transitions** - Open/close/check operations
- **Concurrent Access** - Multi-tenant concurrent state checks
- **Half-Open Trials** - Trial recording performance

## Performance Targets

Based on our testing, Fusebox should achieve:

| Operation | Target | Notes |
|-----------|--------|-------|
| Policy pre-flight | < 100µs | p99 latency |
| Breaker state check | < 10µs | p99 latency |
| Ledger write | < 1ms | SQLite in-memory |
| Ledger read (1d) | < 5ms | With 10K events |
| Concurrent (1K tenants) | < 500ms | Sequential checks |

## Viewing Results

After running benchmarks, open the HTML report:

```bash
# On macOS
open target/criterion/report/index.html

# On Linux
xdg-open target/criterion/report/index.html

# On Windows
start target/criterion/report/index.html
```

## Continuous Performance Monitoring

Benchmarks are run in CI on every commit to main to detect performance regressions.

## Profiling

For deeper analysis, use flamegraph:

```bash
cargo install flamegraph
cargo flamegraph --bench policy_bench
```

Or perf on Linux:

```bash
cargo bench --bench policy_bench -- --profile-time=10
```
