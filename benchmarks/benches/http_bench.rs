use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

mod criterion_bench;
mod cli_bench;

fn bench_http(c: &mut Criterion) {
    criterion_bench::benchmark_http_requests(c);
    criterion_bench::benchmark_concurrent_requests(c);
}

fn bench_cli(c: &mut Criterion) {
    cli_bench::benchmark_seeder_performance(c);
    cli_bench::benchmark_database_insertion_rates(c);
    cli_bench::benchmark_memory_usage_patterns(c);
}

criterion_group! {
    name = http_benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(50);
    targets = bench_http
}

criterion_group! {
    name = cli_benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(60))
        .sample_size(10);
    targets = bench_cli
}

criterion_main!(http_benches, cli_benches);
