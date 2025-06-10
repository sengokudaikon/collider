use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion};
use tokio::runtime::Runtime;

pub fn benchmark_http_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("http_requests");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    // Health check benchmark - targeting docker-compose environment
    group.bench_function("health_check", |b| {
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            async move {
                let response = client
                    .get("http://localhost:8880/health")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // Users endpoint benchmark
    group.bench_function("list_users", |b| {
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            async move {
                let response = client
                    .get("http://localhost:8880/api/users")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // Events endpoint benchmark
    group.bench_function("list_events", |b| {
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            async move {
                let response = client
                    .get("http://localhost:8880/api/events")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
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
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            let payload = event_payload.clone();
            async move {
                let response = client
                    .post("http://localhost:8880/api/events")
                    .json(&payload)
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // User analytics endpoint benchmark - use valid UUID
    group.bench_function("user_analytics", |b| {
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            async move {
                let response = client
                    .get("http://localhost:8880/api/users/550e8400-e29b-41d4-a716-446655440000/analytics")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // DELETE single event benchmark
    group.bench_function("delete_event", |b| {
        b.to_async(&rt).iter(|| {
            async {
                let response = client
                    .delete("http://localhost:8880/api/events/550e8400-e29b-41d4-a716-446655440000")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // Bulk DELETE events benchmark
    group.bench_function("bulk_delete_events", |b| {
        b.to_async(&rt).iter(|| {
            async {
                let before_date =
                    chrono::Utc::now() - chrono::Duration::hours(1);
                let response = client
                    .delete(&format!(
                        "http://localhost:8880/api/events?before={}",
                        before_date.to_rfc3339()
                    ))
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // Stats endpoint benchmark
    group.bench_function("get_stats", |b| {
        b.to_async(&rt).iter(|| {
            async {
                let response = client
                    .get("http://localhost:8880/api/analytics/stats")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    // Real-time metrics benchmark
    group.bench_function("realtime_metrics", |b| {
        b.to_async(&rt).iter(|| {
            async {
                let response = client
                    .get("http://localhost:8880/api/analytics/metrics/realtime")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            }
        });
    });

    group.finish();
}

pub fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("concurrent_requests");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for concurrency in [10, 50, 100].iter() {
        // Concurrent health checks
        group.bench_with_input(
            BenchmarkId::new("health_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| {
                    async {
                        let client = &client;
                        let futures: Vec<_> = (0..concurrency)
                            .map(|_| {
                                async move {
                                    client
                                        .get("http://localhost:8880/health")
                                        .send()
                                        .await
                                        .unwrap()
                                        .status()
                                }
                            })
                            .collect();

                        let results =
                            futures::future::join_all(futures).await;
                        black_box(results);
                    }
                });
            },
        );

        // Concurrent user listing
        group.bench_with_input(
            BenchmarkId::new("users_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| {
                    async {
                        let client = &client;
                        let futures: Vec<_> =
                            (0..concurrency)
                                .map(|_| {
                                    async move {
                                        client
                                .get("http://localhost:8880/api/users")
                                .send()
                                .await
                                .unwrap()
                                .status()
                                    }
                                })
                                .collect();

                        let results =
                            futures::future::join_all(futures).await;
                        black_box(results);
                    }
                });
            },
        );

        // Concurrent event listing
        group.bench_with_input(
            BenchmarkId::new("events_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| {
                    async {
                        let client = &client;
                        let futures: Vec<_> = (0..concurrency)
                            .map(|_| {
                                async move {
                                    client
                                .get("http://localhost:8880/api/events")
                                .send()
                                .await
                                .unwrap()
                                .status()
                                }
                            })
                            .collect();

                        let results =
                            futures::future::join_all(futures).await;
                        black_box(results);
                    }
                });
            },
        );

        // Concurrent bulk delete events
        group.bench_with_input(
            BenchmarkId::new("bulk_delete_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| {
                    async {
                        let client = &client;
                        let futures: Vec<_> = (0..concurrency)
                            .map(|_| {
                                async move {
                                    let before_date = chrono::Utc::now() - chrono::Duration::minutes(30);
                                    client
                                        .delete(&format!("http://localhost:8880/api/events?before={}", before_date.to_rfc3339()))
                                        .send()
                                        .await
                                        .unwrap()
                                        .status()
                                }
                            })
                            .collect();

                        let results =
                            futures::future::join_all(futures).await;
                        black_box(results);
                    }
                });
            },
        );

        // Concurrent stats requests
        group.bench_with_input(
            BenchmarkId::new("stats_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| {
                    async {
                        let client = &client;
                        let futures: Vec<_> = (0..concurrency)
                            .map(|_| {
                                async move {
                                    client
                                        .get("http://localhost:8880/api/analytics/stats")
                                        .send()
                                        .await
                                        .unwrap()
                                        .status()
                                }
                            })
                            .collect();

                        let results =
                            futures::future::join_all(futures).await;
                        black_box(results);
                    }
                });
            },
        );
    }

    group.finish();
}
