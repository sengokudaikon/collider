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
    DATABASE_URL="postgresql://postgres:postgres@localhost:5433/test_db" \
    MIN_USERS=1000 \
    MAX_USERS=50000 \
    MIN_EVENT_TYPES=5 \
    MAX_EVENT_TYPES=50 \
    TARGET_EVENTS=10000000 \
    BATCH_SIZE=10000 \
    cargo run --bin migrate_and_seed

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

# ==== Main Test Commands ====

# 1) Unit tests (no database required)
test-unit:
    @echo "Running unit tests (no database required)..."
    cargo test --all --lib --bins

# 2) Unit tests with coverage (no database required)
test-unit-coverage:
    @echo "Running unit tests with coverage (no database required)..."
    cargo tarpaulin --all --out Html --output-dir coverage --timeout 120 --lib --bins

# 3) Tests that require database
test-db: test-env
    @echo "Running tests that require database..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo test --all
    just test-env-down

# 4) Tests that require database with coverage
test-db-coverage: test-env
    @echo "Running tests that require database with coverage..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo tarpaulin --all --out Html --output-dir coverage --timeout 180
    just test-env-down

# 5) All tests (unit + database)
test-all:
    @echo "Running all tests (unit + database)..."
    @echo "1. Running unit tests..."
    just test-unit
    @echo "2. Running database tests..."
    just test-db
    @echo "✅ All tests completed successfully!"

# 6) All tests with coverage (unit + database)
coverage:
    @echo "Running all tests with coverage (unit + database)..."
    just test-unit-coverage
    just test-db-coverage
    @echo "✅ All tests with coverage completed successfully!"

# Check that coverage meets minimum threshold (80%) - unit tests only
check-coverage:
    cargo tarpaulin --all --out Json --output-dir coverage --timeout 120 --fail-under 80 --lib --bins

# Check that coverage meets minimum threshold including database tests
check-coverage-db: test-env
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

# Start development environment
dev:
    @echo "Starting development environment..."
    cd infrastructure/docker && docker compose up -d
    @echo "Services started. Use 'just dev-backend' and 'just dev-frontend' in separate terminals."

# Run backend in development mode
dev-backend:
    cd server && cargo run

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

db-migrate:
    cargo run --bin migrate

install-perf-tools:
    cargo install cargo-criterion cargo-flamegraph cargo-bloat

quick-perf:
    @echo "Running quick performance analysis..."
    cargo bloat --release --crates
    cd infrastructure/benchmarking && timeout 30s ./run_load_test.sh

# Create a flame graph for profiling
flamegraph:
    cargo flamegraph --bin collider

# Analyze binary size
bloat:
    && cargo bloat --release --crates

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

help:
    @just --list
