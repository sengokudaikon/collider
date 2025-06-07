use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;

mod criterion_bench;

fn bench_http(c: &mut Criterion) {
    criterion_bench::benchmark_http_requests(c);
    criterion_bench::benchmark_concurrent_requests(c);
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(50);
    targets = bench_http
}
criterion_main!(benches);