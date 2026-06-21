use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fusebox_policy::{Policy, PolicyConfig, BudgetRule, Window};
use fusebox_core::RequestContext;
use std::time::Duration;

fn policy_check_budget_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_check_budget");

    // Setup policy
    let config = PolicyConfig {
        default_budget: Some(BudgetRule {
            limit_usd: 100.0,
            window: Window::Day,
            label: "default".to_string(),
        }),
        tenant_budgets: Default::default(),
        breaker_cooldown: Duration::from_secs(60),
        halfopen_trials: 5,
    };

    let policy = Policy::new(config);

    // Benchmark different cost amounts
    for cost in [0.001, 0.01, 0.1, 1.0, 10.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(cost), cost, |b, &cost| {
            b.iter(|| {
                let ctx = RequestContext {
                    tenant: "test-tenant".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    estimated_cost_usd: cost,
                };
                policy.pre_flight(black_box(&ctx))
            });
        });
    }

    group.finish();
}

fn policy_breaker_check_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_breaker_check");

    let config = PolicyConfig {
        default_budget: Some(BudgetRule {
            limit_usd: 100.0,
            window: Window::Day,
            label: "default".to_string(),
        }),
        tenant_budgets: Default::default(),
        breaker_cooldown: Duration::from_secs(60),
        halfopen_trials: 5,
    };

    let policy = Policy::new(config);

    group.bench_function("breaker_state_check", |b| {
        b.iter(|| {
            policy.breaker_state(black_box("test-tenant"))
        });
    });

    group.finish();
}

fn policy_anomaly_detection_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_anomaly_detection");

    let config = PolicyConfig {
        default_budget: Some(BudgetRule {
            limit_usd: 100.0,
            window: Window::Day,
            label: "default".to_string(),
        }),
        tenant_budgets: Default::default(),
        breaker_cooldown: Duration::from_secs(60),
        halfopen_trials: 5,
    };

    let policy = Policy::new(config);

    // Simulate multiple requests to warm up anomaly detection
    for i in 0..100 {
        let ctx = RequestContext {
            tenant: "test-tenant".to_string(),
            model: "gpt-4o-mini".to_string(),
            estimated_cost_usd: 0.01,
        };
        let _ = policy.pre_flight(&ctx);
    }

    group.bench_function("anomaly_check", |b| {
        b.iter(|| {
            let ctx = RequestContext {
                tenant: "test-tenant".to_string(),
                model: "gpt-4o-mini".to_string(),
                estimated_cost_usd: black_box(0.5), // Anomalous cost
            };
            policy.pre_flight(&ctx)
        });
    });

    group.finish();
}

fn policy_concurrent_requests_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_concurrent_requests");

    let config = PolicyConfig {
        default_budget: Some(BudgetRule {
            limit_usd: 1000.0,
            window: Window::Day,
            label: "default".to_string(),
        }),
        tenant_budgets: Default::default(),
        breaker_cooldown: Duration::from_secs(60),
        halfopen_trials: 5,
    };

    let policy = Policy::new(config);

    // Benchmark with different numbers of tenants
    for num_tenants in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_tenants),
            num_tenants,
            |b, &num_tenants| {
                b.iter(|| {
                    for i in 0..num_tenants {
                        let ctx = RequestContext {
                            tenant: format!("tenant-{}", i),
                            model: "gpt-4o-mini".to_string(),
                            estimated_cost_usd: 0.001,
                        };
                        let _ = policy.pre_flight(black_box(&ctx));
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    policy_check_budget_benchmark,
    policy_breaker_check_benchmark,
    policy_anomaly_detection_benchmark,
    policy_concurrent_requests_benchmark
);
criterion_main!(benches);
