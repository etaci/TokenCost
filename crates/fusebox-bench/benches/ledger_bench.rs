use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fusebox_ledger::{LedgerStore, sqlite::SqliteLedger, SpendEvent};
use fusebox_core::Window;
use std::time::{SystemTime, UNIX_EPOCH};

fn ledger_write_event_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ledger_write_event");

    // Setup in-memory SQLite
    let ledger = SqliteLedger::open(":memory:").unwrap();

    group.bench_function("single_event", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let event = SpendEvent {
                tenant: format!("tenant-{}", counter % 100),
                model: "gpt-4o-mini".to_string(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                input_tokens: 1000,
                output_tokens: 500,
                cost_usd: 0.015,
                request_id: format!("req-{}", counter),
            };
            ledger.write_spend(black_box(event)).unwrap();
        });
    });

    group.finish();
}

fn ledger_read_spend_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ledger_read_spend");

    let ledger = SqliteLedger::open(":memory:").unwrap();

    // Pre-populate with events
    for i in 0..1000 {
        let event = SpendEvent {
            tenant: format!("tenant-{}", i % 10),
            model: "gpt-4o-mini".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            input_tokens: 1000,
            output_tokens: 500,
            cost_usd: 0.015,
            request_id: format!("req-{}", i),
        };
        ledger.write_spend(event).unwrap();
    }

    // Benchmark reads
    for window in [Window::Minute, Window::Hour, Window::Day].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", window)),
            window,
            |b, window| {
                b.iter(|| {
                    ledger.total_spend(
                        black_box("tenant-5"),
                        black_box(*window)
                    ).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn ledger_concurrent_writes_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ledger_concurrent_writes");

    let ledger = SqliteLedger::open(":memory:").unwrap();

    for num_writes in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_writes),
            num_writes,
            |b, &num_writes| {
                let mut counter = 0u64;
                b.iter(|| {
                    for _ in 0..num_writes {
                        counter += 1;
                        let event = SpendEvent {
                            tenant: format!("tenant-{}", counter % 10),
                            model: "gpt-4o-mini".to_string(),
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64,
                            input_tokens: 1000,
                            output_tokens: 500,
                            cost_usd: 0.015,
                            request_id: format!("req-{}", counter),
                        };
                        ledger.write_spend(black_box(event)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    ledger_write_event_benchmark,
    ledger_read_spend_benchmark,
    ledger_concurrent_writes_benchmark
);
criterion_main!(benches);
