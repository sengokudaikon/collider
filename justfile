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
# Run backend in development mode
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

# Run all code quality checks
quality: lint audit udeps geiger
    @echo "✅ All code quality checks completed!"

# Run security-focused checks
security: audit geiger
    @echo "✅ Security checks completed!"

# ==== Deployment ====

# Deploy to local K3S environment (recommended)
deploy-local:
    cd infrastructure && just deploy-local

# Deploy to production K3S environment
deploy-prod project_id:
    cd infrastructure && just deploy-prod {{ project_id }}

# Quick development setup (K3S + verification)
dev-setup:
    cd infrastructure && just dev-setup

# Verify local deployment
verify-local:
    cd infrastructure && just verify-local

# Verify production deployment
verify-prod:
    cd infrastructure && just verify-prod

# Destroy local environment
destroy-local:
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

help:
    @just --list
