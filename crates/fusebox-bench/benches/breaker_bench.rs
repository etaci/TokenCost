use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fusebox_policy::{Breaker, BreakerState};
use std::sync::Arc;
use std::time::Duration;

fn breaker_state_transitions_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("breaker_state_transitions");

    let breaker = Arc::new(Breaker::new(Duration::from_secs(5), 3));

    group.bench_function("open_breaker", |b| {
        b.iter(|| {
            breaker.open(black_box("test-tenant"), black_box("budget exceeded"));
        });
    });

    group.bench_function("close_breaker", |b| {
        b.iter(|| {
            breaker.close(black_box("test-tenant"));
        });
    });

    group.bench_function("check_state", |b| {
        b.iter(|| {
            breaker.state(black_box("test-tenant"))
        });
    });

    group.finish();
}

fn breaker_concurrent_access_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("breaker_concurrent_access");

    let breaker = Arc::new(Breaker::new(Duration::from_secs(5), 3));

    for num_tenants in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_tenants),
            num_tenants,
            |b, &num_tenants| {
                b.iter(|| {
                    for i in 0..num_tenants {
                        let tenant = format!("tenant-{}", i);
                        let _ = breaker.state(black_box(&tenant));
                    }
                });
            },
        );
    }

    group.finish();
}

fn breaker_half_open_trials_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("breaker_half_open_trials");

    let breaker = Arc::new(Breaker::new(Duration::from_millis(10), 5));

    // Open the breaker first
    breaker.open("test-tenant", "test");

    // Wait for cooldown
    std::thread::sleep(Duration::from_millis(20));

    group.bench_function("trial_attempt", |b| {
        b.iter(|| {
            breaker.record_trial(black_box("test-tenant"), black_box(true));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    breaker_state_transitions_benchmark,
    breaker_concurrent_access_benchmark,
    breaker_half_open_trials_benchmark
);
criterion_main!(benches);
