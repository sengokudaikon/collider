use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_http_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = reqwest::Client::new();
    
    let mut group = c.benchmark_group("http_requests");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));
    
    // Health check benchmark
    group.bench_function("health_check", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .get("http://app:8080/health")
                .send()
                .await
                .unwrap();
            black_box(response.status());
        });
    });
    
    // Event creation benchmark - use correct CreateEventCommand format
    let event_payload = serde_json::json!({
        "user_id": "550e8400-e29b-41d4-a716-446655440000",
        "event_type": "user_action",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "metadata": {
            "session_id": "bench_session",
            "action": "click",
            "element": "button_submit",
            "page": "/dashboard"
        }
    });
    
    group.bench_function("create_event", |b| {
        b.to_async(&rt).iter(|| {
            let payload = event_payload.clone();
            async move {
                let response = client
                    .post("http://app:8080/api/events")
                    .json(&payload)
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });
    
    group.finish();
}

fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = reqwest::Client::new();
    
    let mut group = c.benchmark_group("concurrent_requests");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    
    for concurrency in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("health_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let client = &client;
                    let futures: Vec<_> = (0..concurrency)
                        .map(|_| async move {
                            client
                                .get("http://app:8080/health")
                                .send()
                                .await
                                .unwrap()
                                .status()
                        })
                        .collect();
                    
                    let results = futures::future::join_all(futures).await;
                    black_box(results);
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, benchmark_http_requests, benchmark_concurrent_requests);
criterion_main!(benches);