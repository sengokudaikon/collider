#!/usr/bin/env just --justfile

# Default recipe to display available commands
default:
    @just --list

# ==== Testing ====
# Test infrastructure uses different ports to avoid conflicts with local deployment:
# - PostgreSQL: 5433 (vs 30432 for local K3S NodePort, 5432 for port-forward)
# - Dragonfly: 6380 (vs 30379 for local K3S NodePort, 6379 for port-forward)
# - Local K3S services: 30080 (app), 30090 (prometheus), 30300 (grafana), 30686 (jaeger)

# Start test environment (lightweight)
test-env:
    @echo "Starting test environment..."
    docker compose -f docker-compose.test.yml up -d
    @echo "Waiting for services to be ready..."
    docker compose -f docker-compose.test.yml run --rm wait-for-services
    @echo "Test services started. Use 'just test-env-down' to stop."
    @echo "PostgreSQL: localhost:5433"
    @echo "Dragonfly (Redis): localhost:6380"

# Stop test environment
test-env-down:
    docker compose -f docker-compose.test.yml down -v

# Check test environment health
test-env-health:
    @echo "Checking test environment health..."
    @docker exec collider_postgres_test pg_isready -U postgres || echo "Postgres unhealthy"
    @docker exec collider_dragonfly_test redis-cli ping || echo "Dragonfly unhealthy"

# Run database migrations and seeding for test environment
test-setup-db:
    @echo "Setting up test database (migrations + full seeding)..."
    @echo "1. Running migrations..."
    DATABASE_URL="postgresql://postgres:postgres@localhost:5433/test_db" \
    cargo run --bin migrator -- up
    @echo "2. Running seeding..."
    DATABASE_URL="postgresql://postgres:postgres@localhost:5433/test_db" \
    cargo run --bin seeder -- all \
    --min-users 1000 \
    --max-users 50000 \
    --min-event-types 5 \
    --max-event-types 50 \
    --target-events 10000000 \
    --event-batch-size 10000

test-analytics-demo:
    @echo "Running analytics demo with test environment..."
    DATABASE_URL="postgresql://postgres:postgres@localhost:5433/test_db" \
    REDIS_HOST="127.0.0.1" \
    REDIS_PORT="6380" \
    cargo run --example analytics_usage

# Full test workflow: start environment, setup database, run demo
test-full-demo: test-env test-setup-db test-analytics-demo
    @echo "✅ Analytics demo completed successfully!"
    @echo "Use 'just test-env-down' to stop test environment"

tests: test-env
    @echo "Running all tests (unit + database)..."
    @echo "Running tests that require database..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo test --all
    just test-env-down
    @echo "✅ All tests completed successfully!"

coverage: test-env
    @echo "Running all tests with coverage (unit + database)..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo tarpaulin --all --out Html --output-dir coverage --timeout 180
    just test-env-down
    @echo "✅ All tests with coverage completed successfully!"

check-coverage: test-env
    DATABASE_URL="postgres://postgres:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo tarpaulin --all --out Json --output-dir coverage --timeout 180 --fail-under 80
    just test-env-down

# Quick test for CI/CD
test-ci:
    @echo "Running CI test suite..."
    just test-unit
    just test-db

# Clean test artifacts
clean-test:
    cargo clean
    rm -rf coverage/
    just test-env-down

# ==== Development Environment ====

# Docker Compose Local Development
# Start full development environment
dev-up:
    @echo "Starting full development environment..."
    docker compose up -d
    @echo "Development environment started!"
    @echo "Services available at:"
    @echo "  Application: http://localhost:8080"
    @echo "  PostgreSQL: localhost:5432"
    @echo "  Dragonfly (Redis): localhost:6379"
    @echo "  Prometheus: http://localhost:9090"
    @echo "  Grafana: http://localhost:3000 (admin/admin)"
    @echo "  Jaeger: http://localhost:16686"

# Start with full monitoring stack (includes nginx)
dev-up-full:
    @echo "Starting full development environment with nginx..."
    docker compose --profile full up -d
    @echo "Full environment started! App available at http://localhost (port 80)"

# Start only core services (app, db, cache)
dev-up-core:
    @echo "Starting core services only..."
    docker compose up -d postgres dragonfly app
    @echo "Core services started: app (8080), postgres (5432), dragonfly (6379)"

# Stop development environment
dev-down:
    docker compose down

# Stop and remove volumes (full cleanup)
dev-down-clean:
    docker compose down -v
    docker system prune -f

# View logs for all services
dev-logs:
    docker compose logs -f

# View logs for specific service
dev-logs-app:
    docker compose logs -f app

dev-logs-db:
    docker compose logs -f postgres

# Check health of all services
dev-health:
    @echo "Checking service health..."
    docker compose ps
    @echo ""
    @echo "Service endpoints:"
    @curl -s http://localhost:8080/health || echo "❌ App not healthy"
    @docker exec collider_postgres_dev pg_isready -U postgres -d events && echo "✅ PostgreSQL healthy" || echo "❌ PostgreSQL not healthy"
    @docker exec collider_dragonfly_dev redis-cli ping && echo "✅ Dragonfly healthy" || echo "❌ Dragonfly not healthy"

# Rebuild and restart app container
dev-rebuild:
    docker compose up -d --build app

# Setup development environment (copy env, run migrations, seed data)
dev-setup: dev-up
    @echo "Setting up development environment..."
    @if [ ! -f .env ]; then cp .env.example .env && echo "📄 Created .env from template"; fi
    @echo "⏳ Waiting for services to be ready..."
    @sleep 10
    @echo "🔄 Running database migrations..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5432/events" cargo run --bin migrator -- up
    @echo "🌱 Seeding database with sample data..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5432/events" cargo run --bin seeder -- all --min-users 100 --max-users 1000 --target-events 10000
    @echo "✅ Development environment ready!"

# Run database migrations against dev environment
dev-migrate:
    DATABASE_URL="postgres://postgres:postgres@localhost:5432/events" cargo run --bin migrator -- up

# Seed development database
dev-seed:
    DATABASE_URL="postgres://postgres:postgres@localhost:5432/events" cargo run --bin seeder -- all --min-users 100 --max-users 1000 --target-events 10000

# Connect to development database
dev-db:
    docker exec -it collider_postgres_dev psql -U postgres -d events

# Connect to development cache
dev-cache:
    docker exec -it collider_dragonfly_dev redis-cli

# Run backend in development mode (native, not containerized)
dev:
    cd server && cargo run

watch:
    cargo watch -x run

watch-check:
    cargo watch -x check


# ==== Linting and Formatting ====

# Install development tools
install-dev-tools:
    cargo install cargo-watch cargo-audit cargo-tarpaulin cargo-criterion cargo-bloat cargo-udeps cargo-llvm-lines cargo-geiger

lint:
    cargo clippy -- -D warnings

# Format backend code
format:
    cargo +nightly fmt

audit:
    cargo audit
udeps:
    cargo +nightly udeps --all-targets
geiger:
    cargo geiger
llvm-lines:
    cargo llvm-lines

db-migrate:
    cargo run --bin migrator

db-seed:
    cargo run --bin seeder

# Build release binaries for distribution
build-binaries:
    @echo "Building release binaries..."
    cargo build --release --bin migrator
    cargo build --release --bin seeder
    @echo "✅ Binaries built:"
    @echo "  Migrator: target/release/migrator"
    @echo "  Seeder: target/release/seeder"

# Install binaries to system (requires sudo/admin privileges)
install-binaries: build-binaries
    @echo "Installing binaries to system..."
    cp target/release/migrator /usr/local/bin/collider-migrator
    cp target/release/seeder /usr/local/bin/collider-seeder
    @echo "✅ Binaries installed:"
    @echo "  collider-migrator"
    @echo "  collider-seeder"

install-perf-tools:
    cargo install cargo-criterion cargo-flamegraph cargo-bloat

quick-perf:
    @echo "Running quick performance analysis..."
    cargo bloat --release --crates
    cd infrastructure/benchmarking && timeout 30s ./run_load_test.sh

# Create a flame graph for profiling
flamegraph:
    cargo flamegraph --bin collider --root

# Profile app with flamegraph while running benchmarks
profile-bench: dev-up
    #!/usr/bin/env bash
    set -e
    echo "🔥 Starting app with flamegraph profiling + benchmarks..."
    
    # Start the app with flamegraph in background
    echo "📊 Starting flamegraph profiling..."
    cd server && cargo flamegraph --bin server -- &
    APP_PID=$!
    
    # Wait for app to start
    echo "⏳ Waiting for app to start..."
    sleep 10
    
    # Check if app is ready
    until curl -f http://localhost:8080/health &>/dev/null; do
        echo "Waiting for app to be ready..."
        sleep 2
    done
    echo "✅ App is ready!"
    
    # Run benchmarks
    echo "🚀 Running benchmarks..."
    cargo bench --package collider-benchmarks || true
    
    # Stop the app and generate flamegraph
    echo "🛑 Stopping app and generating flamegraph..."
    kill $APP_PID
    wait $APP_PID 2>/dev/null || true
    
    echo "✅ Profiling complete! Check flamegraph.svg"

# Profile specific benchmark with perf
profile-perf-bench benchmark="http_bench":
    #!/usr/bin/env bash
    set -e
    echo "🔍 Profiling {{ benchmark }} with perf..."
    
    # Start dev environment
    just dev-up
    
    # Run benchmark with perf profiling
    perf record -g --call-graph=dwarf \
        cargo bench --package collider-benchmarks {{ benchmark }}
    
    # Generate perf report
    perf report --stdio > perf_report_{{ benchmark }}.txt
    echo "✅ Perf report saved to perf_report_{{ benchmark }}.txt"

# Profile app in docker while running external benchmarks
profile-docker-bench:
    #!/usr/bin/env bash
    set -e
    echo "🐳 Profiling dockerized app with external benchmarks..."
    
    # Start dev environment
    just dev-up
    
    # Start profiling the containerized app
    echo "📊 Starting profiling..."
    docker exec -d collider_app_dev sh -c "apt-get update && apt-get install -y linux-perf" || true
    
    # Run benchmarks from host
    echo "🚀 Running benchmarks..."
    cargo bench --package collider-benchmarks
    
    # Collect container stats
    echo "📈 Collecting container performance stats..."
    docker stats collider_app_dev --no-stream > docker_stats_during_bench.txt
    
    echo "✅ Docker profiling complete!"

# Advanced: Profile with multiple tools simultaneously
profile-comprehensive:
    #!/usr/bin/env bash
    set -e
    echo "🎯 Comprehensive profiling + benchmarking..."
    
    # Create results directory
    mkdir -p profiling_results/$(date +%Y%m%d_%H%M%S)
    RESULTS_DIR="profiling_results/$(date +%Y%m%d_%H%M%S)"
    
    # Start dev environment
    just dev-up
    
    # Start container monitoring
    echo "📊 Starting container monitoring..."
    docker stats collider_app_dev --no-stream > "$RESULTS_DIR/docker_stats.log" &
    STATS_PID=$!
    
    # Start application profiling (if running natively)
    if pgrep -f "target.*server" > /dev/null; then
        echo "🔥 Starting flamegraph on native app..."
        sudo perf record -g -p $(pgrep -f "target.*server") &
        PERF_PID=$!
    fi
    
    # Run benchmarks with detailed logging
    echo "🚀 Running comprehensive benchmarks..."
    {
        echo "=== Criterion Benchmarks ==="
        cargo bench --package collider-benchmarks 2>&1
        echo ""
        echo "=== K6 Load Tests ==="
        docker run --rm --network collider \
            -v $(pwd)/infrastructure/benchmarking/k6:/scripts \
            grafana/k6:latest run /scripts/load-test.js 2>&1
        echo ""
        echo "=== Goose Load Tests ==="
        cd infrastructure/benchmarking && cargo run --bin goose_load_test 2>&1
    } | tee "$RESULTS_DIR/benchmark_output.log"
    
    # Stop monitoring
    kill $STATS_PID 2>/dev/null || true
    if [[ -n "${PERF_PID:-}" ]]; then
        sudo kill $PERF_PID 2>/dev/null || true
        sudo perf report --stdio > "$RESULTS_DIR/perf_report.txt" 2>/dev/null || true
    fi
    
    echo "✅ Comprehensive profiling complete!"
    echo "📁 Results in: $RESULTS_DIR"

# Profile dockerized app with flamegraph + benchmarks (recommended)
profile-docker: dev-up
    @echo "🔥 Profiling dockerized app with benchmarks..."
    ./scripts/profile-docker-app.sh

# Profile native app with continuous monitoring
profile-native-live:
    #!/usr/bin/env bash
    set -e
    echo "🔥 Live profiling of native app with benchmarks..."
    
    # Ensure dev environment is running for external services
    just dev-up
    
    # Create results directory
    RESULTS_DIR="profiling_results/native_$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$RESULTS_DIR"
    
    # Start the app with flamegraph in background
    echo "📊 Starting app with flamegraph profiling..."
    cd server
    
    # Start app with perf profiling
    RUST_LOG=info cargo build --release
    perf record -g -F 99 ./target/release/server &
    APP_PID=$!
    
    # Wait for app to start
    echo "⏳ Waiting for app to start..."
    sleep 5
    
    # Check if app is ready
    until curl -f http://localhost:8080/health &>/dev/null; do
        echo "Waiting for app to be ready..."
        sleep 2
    done
    echo "✅ App is ready!"
    
    # Start system monitoring
    top -l 0 -s 1 | grep -E "(CPU usage|server)" > "../$RESULTS_DIR/system_stats.log" &
    TOP_PID=$!
    
    # Run benchmarks
    echo "🚀 Running benchmarks..."
    {
        cd ..
        cargo bench --package collider-benchmarks 2>&1
    } | tee "$RESULTS_DIR/benchmark_output.log"
    
    # Stop everything
    echo "🛑 Stopping profiling..."
    kill $APP_PID 2>/dev/null || true
    kill $TOP_PID 2>/dev/null || true
    
    # Generate flamegraph
    cd ..
    if [[ -f perf.data ]]; then
        echo "🔥 Generating flamegraph..."
        perf script | flamegraph > "$RESULTS_DIR/flamegraph.svg" 2>/dev/null || true
        mv perf.data "$RESULTS_DIR/" 2>/dev/null || true
    fi
    
    echo "✅ Native profiling complete!"
    echo "📁 Results in: $RESULTS_DIR"

# Real-time monitoring while running benchmarks (no profiling)
monitor-bench: dev-up
    #!/usr/bin/env bash
    set -e
    echo "📊 Real-time monitoring during benchmarks..."
    
    # Start real-time monitoring in background
    {
        echo "Starting container monitoring..."
        while true; do
            echo "=== $(date) ==="
            docker stats collider_app_dev --no-stream
            echo ""
            sleep 5
        done
    } &
    MONITOR_PID=$!
    
    # Run benchmarks
    echo "🚀 Running benchmarks with live monitoring..."
    cargo bench --package collider-benchmarks
    
    # Stop monitoring
    kill $MONITOR_PID 2>/dev/null || true
    echo "✅ Monitoring complete!"

# Analyze binary size
bloat:
    cargo bloat --release --crates

# Run simple HTTP benchmarks
http-bench path="/" requests="1000" concurrency="50":
    hey -n {{ requests }} -c {{ concurrency }} http://localhost:8080{{ path }}

# Delegate to infrastructure-specific commands
load-test:
    cd infrastructure && just load-test

criterion-bench:
    cd infrastructure && just criterion-bench

quick-bench:
    cd infrastructure && just quick-bench

# ==== Benchmarking Commands ====

# Run all benchmarks in docker-compose environment
bench-all: dev-up
    @echo "🚀 Running all benchmarks in docker-compose environment..."
    docker-compose -f docker-compose.yml -f infrastructure/benchmarking/docker-compose-bench.yml --profile bench up bench-runner
    @echo "✅ All benchmarks completed!"

# Run Criterion micro-benchmarks
bench-criterion: dev-up
    @echo "📊 Running Criterion benchmarks..."
    cargo bench --package collider-benchmarks
    @echo "✅ Criterion benchmarks completed!"

# Run K6 load tests in docker
bench-k6: dev-up
    @echo "🚀 Running K6 load tests..."
    docker-compose -f docker-compose.yml -f infrastructure/benchmarking/docker-compose-bench.yml --profile k6 run k6 run /scripts/load-test.js
    @echo "✅ K6 load tests completed!"

# Run Goose load tests
bench-goose: dev-up
    @echo "🦆 Running Goose load tests..."
    cd infrastructure/benchmarking && cargo run --bin goose_load_test
    @echo "✅ Goose load tests completed!"

# Quick benchmark validation
bench-quick: dev-up
    @echo "⚡ Running quick benchmarks..."
    cargo bench --package collider-benchmarks -- --sample-size 10 --measurement-time 5
    @echo "✅ Quick benchmarks completed!"

# Clean benchmark results
bench-clean:
    @echo "🧹 Cleaning benchmark results..."
    rm -rf target/criterion/
    rm -rf infrastructure/benchmarking/results/
    @echo "✅ Benchmark results cleaned!"

# Simple one-command profiling + benchmarking (recommended)
profile-simple:
    @echo "🔥 Simple flamegraph profiling + benchmarks..."
    ./scripts/simple-profile-bench.sh

# Live profiling dashboard with real-time metrics
profile-live:
    @echo "📊 Starting live profiling dashboard..."
    ./scripts/live-profile-dashboard.sh

# Run all code quality checks
quality: lint audit udeps geiger
    @echo "✅ All code quality checks completed!"

# Run security-focused checks
security: audit geiger
    @echo "✅ Security checks completed!"

# ==== Deployment ====

# Deploy to local K3S environment (recommended)
k3s-deploy-local:
    cd infrastructure && just deploy-local

# Deploy to production K3S environment
k3s-deploy-prod project_id:
    cd infrastructure && just deploy-prod {{ project_id }}

# Quick development setup (K3S + verification)
k3s-dev-setup:
    cd infrastructure && just dev-setup

# Verify local deployment
k3s-verify-local:
    cd infrastructure && just verify-local

# Verify production deployment
k3s-verify-prod:
    cd infrastructure && just verify-prod

# Destroy local environment
k3s-destroy-local:
    cd infrastructure && just destroy-local

# Get cluster status and endpoints
cluster-status:
    cd infrastructure && just cluster-status

# Quick infrastructure status check
infra-status:
    cd infrastructure && just status

# Performance Testing
# ===================

# Run quick performance validation (k6 smoke test)
perf-quick target="http://localhost:8080":
    ./infrastructure/benchmarking/orchestrate-performance-tests.sh {{target}} quick

# Run standard load testing suite (all tools)
perf-load target="http://localhost:8080":
    ./infrastructure/benchmarking/orchestrate-performance-tests.sh {{target}} load

# Run stress testing suite
perf-stress target="http://localhost:8080":
    ./infrastructure/benchmarking/orchestrate-performance-tests.sh {{target}} stress

# Run full performance testing suite (all tools, all test types)
perf-full target="http://localhost:8080":
    ./infrastructure/benchmarking/orchestrate-performance-tests.sh {{target}} full

# Run CRITICAL performance testing (extreme scale: 100k RPS, millions of events)
perf-critical target="http://localhost:8080" max_rps="100000" duration="600":
    ./infrastructure/benchmarking/critical-performance-test.sh {{target}} {{max_rps}} {{duration}}

# Run individual performance tools
# ================================

# Run Vegeta load testing
perf-vegeta target="localhost:8080":
    cd infrastructure/benchmarking && ./run_load_test.sh {{target}}

# Run Goose Rust-based load testing
perf-goose target="http://localhost:8080" users="1000" rate="100/1s" duration="300":
    cd infrastructure/benchmarking && ./run-goose.sh {{target}} {{users}} {{rate}} {{duration}}

# Run Criterion micro-benchmarks
perf-criterion target="http://localhost:8080" type="all":
    cd infrastructure/benchmarking && ./run-criterion.sh {{target}} {{type}}

# Run k6 JavaScript-based testing
perf-k6 target="http://localhost:8080" type="load":
    cd infrastructure/benchmarking && ./k6/run-k6.sh {{target}} {{type}}

# Run Yandex Tank comprehensive testing
perf-tank target="http://localhost:8080":
    cd infrastructure/benchmarking && ./yandex-tank/run_tank.sh {{target}}

# Performance regression detection
# ===============================

# Create performance baseline from latest results
perf-baseline:
    cd infrastructure/benchmarking && ./performance-regression-detector.sh baseline

# Run performance regression detection
perf-regression:
    cd infrastructure/benchmarking && ./performance-regression-detector.sh detect

# Performance monitoring and analysis
# ==================================

# Start monitoring stack (Prometheus + Grafana)
perf-monitoring-start:
    cd infrastructure/config && docker-compose -f docker-compose.monitoring.yml up -d

# Stop monitoring stack
perf-monitoring-stop:
    cd infrastructure/config && docker-compose -f docker-compose.monitoring.yml down

# View performance results
perf-results:
    @echo "📊 Performance Testing Results:"
    @echo "==============================="
    @ls -la infrastructure/benchmarking/orchestrated_results/ 2>/dev/null || echo "No orchestrated results found"
    @echo ""
    @echo "📈 Individual Tool Results:"
    @find infrastructure/benchmarking -name "*results*" -type d 2>/dev/null || echo "No individual results found"

# Clean performance test results
perf-clean:
    @echo "🧹 Cleaning performance test results..."
    rm -rf infrastructure/benchmarking/orchestrated_results/
    rm -rf infrastructure/benchmarking/results/
    rm -rf infrastructure/benchmarking/goose_results/
    rm -rf infrastructure/benchmarking/criterion_results/
    rm -rf infrastructure/benchmarking/k6_results/
    rm -rf infrastructure/benchmarking/tank_results/
    rm -rf infrastructure/benchmarking/yandex-tank/tank_results/
    @echo "✅ Performance test results cleaned"

# Performance testing help
perf-help:
    @echo "🎯 Collider Performance Testing Commands"
    @echo "======================================="
    @echo ""
    @echo "Quick Testing:"
    @echo "  just perf-quick                    # Quick validation (2 min)"
    @echo "  just perf-load                     # Standard load tests (20 min)"
    @echo "  just perf-stress                   # Stress testing (20 min)"
    @echo "  just perf-full                     # Complete suite (45 min)"
    @echo "  just perf-critical                 # CRITICAL: 100k RPS extreme testing"
    @echo ""
    @echo "Individual Tools:"
    @echo "  just perf-vegeta                   # Vegeta HTTP load testing"
    @echo "  just perf-goose                    # Goose Rust load testing"
    @echo "  just perf-criterion                # Criterion micro-benchmarks"
    @echo "  just perf-k6                       # k6 JavaScript testing"
    @echo "  just perf-tank                     # Yandex Tank comprehensive"
    @echo ""
    @echo "Regression Detection:"
    @echo "  just perf-baseline                 # Create performance baseline"
    @echo "  just perf-regression               # Check for regressions"
    @echo ""
    @echo "Monitoring:"
    @echo "  just perf-monitoring-start         # Start Prometheus + Grafana"
    @echo "  just perf-monitoring-stop          # Stop monitoring"
    @echo ""
    @echo "Utilities:"
    @echo "  just perf-results                  # View available results"
    @echo "  just perf-clean                    # Clean all results"
    @echo ""
    @echo "Examples:"
    @echo "  just perf-quick                                      # Test localhost"
    @echo "  just perf-load http://staging.example.com           # Test staging"
    @echo "  just perf-critical http://localhost:8080 50000      # 50k RPS critical test"
    @echo "  just perf-goose http://localhost:8080 500 50/1s     # 500 users, 50/sec"
    @echo "  just perf-criterion http://localhost:8080 quick     # Quick benchmarks"

# Show profiling + benchmarking help
profile-help:
    @echo "🔥 Collider Profiling + Benchmarking Commands"
    @echo "============================================="
    @echo ""
    @echo "🚀 Quick Start (Recommended):"
    @echo "  just profile-simple       # One-command flamegraph + benchmarks"
    @echo "  just profile-live         # Live dashboard with real-time metrics"
    @echo "  just monitor-bench        # Real-time monitoring (no profiling)"
    @echo ""
    @echo "📊 Benchmarking Only:"
    @echo "  just bench-criterion      # Criterion micro-benchmarks"
    @echo "  just bench-k6             # K6 load tests"
    @echo "  just bench-goose          # Goose load tests"
    @echo "  just bench-quick          # Quick validation"
    @echo "  just bench-all            # All benchmarks"
    @echo ""
    @echo "🔥 Advanced Profiling:"
    @echo "  just profile-docker       # Profile dockerized app"
    @echo "  just profile-native-live  # Profile native app with live monitoring"
    @echo "  just profile-comprehensive # Full profiling suite"
    @echo ""
    @echo "🧹 Cleanup:"
    @echo "  just bench-clean          # Clean benchmark results"
    @echo "  just perf-clean           # Clean all performance results"
    @echo ""
    @echo "📈 Monitoring Stack:"
    @echo "  just dev-up               # Includes Prometheus (port 9090) + Grafana (port 3000)"
    @echo "  http://localhost:9090     # Prometheus metrics"
    @echo "  http://localhost:3000     # Grafana dashboards (admin/admin)"
    @echo ""
    @echo "💡 Usage Examples:"
    @echo "  # Quick profiling + benchmarks:"
    @echo "  just profile-simple"
    @echo ""
    @echo "  # Live dashboard with continuous metrics:"
    @echo "  just profile-live"
    @echo ""
    @echo "  # Just run benchmarks without profiling:"
    @echo "  just bench-criterion"
    @echo ""
    @echo "  # Monitor resource usage during benchmarks:"
    @echo "  just monitor-bench"
    @echo ""
    @echo "📋 What Each Tool Does:"
    @echo "  • profile-simple: Runs app with flamegraph, executes benchmarks, generates SVG"
    @echo "  • profile-live: Creates web dashboard with real-time metrics + continuous benchmarks"
    @echo "  • profile-docker: Profiles the containerized app while running benchmarks"
    @echo "  • bench-*: Run specific benchmark tools against the docker-compose app"
    @echo "  • monitor-bench: Shows live container stats during benchmark execution"

help:
    @just --list
