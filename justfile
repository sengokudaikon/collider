#!/usr/bin/env just --justfile

# Default recipe to display available commands
default:
    @just --list

# ==== Testing ====

# Install cargo-tarpaulin for coverage
install-coverage:
    cargo install cargo-tarpaulin

# Start test environment (lightweight)
test-env:
    @echo "Starting test environment..."
    cd infrastructure/docker && docker compose -f docker-compose.test.yml up -d
    @echo "Test services started. Use 'just test-env-down' to stop."
    @echo "PostgreSQL: localhost:5433"
    @echo "Redis: localhost:6380"

# Stop test environment
test-env-down:
    cd infrastructure/docker && docker compose -f docker-compose.test.yml down

# Check test environment health
test-env-health:
    @echo "Checking test environment health..."
    @docker exec collider-postgres-test pg_isready -U postgres || echo "Postgres unhealthy"
    @docker exec collider-redis-test redis-cli ping || echo "Redis unhealthy"

# Run database migrations and seeding for test environment
test-setup-db:
    @echo "Setting up test database (migrations + minimal seeding)..."
    DATABASE_URL="postgresql://postgres:password@localhost:5433/collider" \
    MIN_USERS=10 \
    MAX_USERS=50 \
    MIN_EVENT_TYPES=5 \
    MAX_EVENT_TYPES=5 \
    TARGET_EVENTS=1000 \
    BATCH_SIZE=100 \
    cargo run --bin migrate_and_seed

# Run analytics demo with test environment (includes setup)
test-analytics-demo: test-setup-db
    @echo "Running analytics demo with test environment..."
    DATABASE_URL="postgresql://postgres:password@localhost:5433/collider" \
    REDIS_HOST="127.0.0.1" \
    REDIS_PORT="6380" \
    cargo run --example analytics_usage

# Full test workflow: start environment, setup database, run demo
test-full-demo: test-env test-analytics-demo
    @echo "✅ Analytics demo completed successfully!"
    @echo "Use 'just test-env-down' to stop test environment"

# Run all tests
test:
    cargo test --all

# Run tests with output
test-verbose:
    cargo test --all -- --nocapture

# Run specific domain tests
test-events:
    cargo test --package events-dao --package events-commands --package events-queries --package events-http --package events-models

test-user:
    cargo test --package user-dao --package user-commands --package user-queries --package user-http --package user-models

test-analytics:
    cargo test --package analytics

test-persistence:
    cargo test --package database-traits --package sql-connection --package redis-connection

# Run tests with coverage
coverage:
    cargo tarpaulin --all --out Html --output-dir coverage --timeout 120

# Run coverage excluding test-utils and integration tests
coverage-core:
    cargo tarpaulin --package analytics --package user-dao --package user-commands --package user-queries --package user-http --package user-models --package events-dao --package events-commands --package events-queries --package events-http --package events-models --package database-traits --package sql-connection --package redis-connection --out Html --output-dir coverage --timeout 120

# Watch tests during development
test-watch:
    cargo watch -x "test --all"

# Clean test artifacts
clean-test:
    cargo clean
    rm -rf coverage/

# Run integration tests only
test-integration:
    cargo test integration

# Run unit tests only
test-unit:
    cargo test --lib --bins

# Check that coverage meets minimum threshold (80%)
check-coverage:
    cargo tarpaulin --all --out Json --output-dir coverage --timeout 120 --fail-under 80

# ==== Development Environment ====

# Start development environment
dev:
    @echo "Starting development environment..."
    cd infrastructure/docker && docker compose up -d
    @echo "Services started. Use 'just dev-backend' and 'just dev-frontend' in separate terminals."

# Run backend in development mode
dev-backend:
    cd server && cargo run

# Run frontend in development mode
dev-frontend:
    cd frontend && npm run dev

# ==== Docker Compose Commands ====

# Start all services with Docker Compose
up:
    cd infrastructure/docker && docker compose up -d

# Stop all services
down:
    cd infrastructure/docker && docker compose down

# View logs for all services
logs:
    cd infrastructure/docker && docker compose logs -f

# ==== Build Commands ====

# Build everything
build: build-frontend build-backend

# Build frontend
build-frontend:
    cd frontend && npm install && npm run build

# Build backend
build-backend:
    cd server && cargo build --release

# Build docker images
build-docker:
    cd infrastructure/docker && docker compose build

# Clean all build artifacts
clean:
    cd infrastructure/docker && docker compose down -v
    docker system prune -f
    cd server && cargo clean
    cd frontend && rm -rf node_modules dist

# ==== Linting and Formatting ====

# Install development tools
install-dev-tools:
    cargo install cargo-watch cargo-audit cargo-tarpaulin cargo-criterion cargo-bloat cargo-udeps cargo-llvm-lines cargo-geiger

# Run linting on backend
lint-backend:
    cargo clippy -- -D warnings

# Run linting on frontend
lint-frontend:
    cd frontend && npm run lint

# Run all linting
lint: lint-backend lint-frontend

# Format backend code
format-backend:
    cd server && cargo +nightly fmt

# Format frontend code
format-frontend:
    cd frontend && npm run format || echo "No format script in frontend"

# Format all code
format: format-backend format-frontend

# Check dependencies for security vulnerabilities
audit:
    cargo audit

# ==== Database Commands ====

# Run database migrations
db-migrate:
    cd server && cargo run --bin migrate

# Reset database (destructive)
db-reset:
    cd infrastructure/docker && docker compose down postgres
    docker volume rm collider_postgres_data
    cd infrastructure/docker && docker compose up -d postgres

# ==== Performance Testing (Essential) ====

# Install essential performance tools
install-perf-tools:
    cargo install cargo-criterion cargo-flamegraph cargo-bloat

# Quick performance analysis
quick-perf:
    @echo "Running quick performance analysis..."
    cd server && cargo bloat --release --crates
    cd infrastructure/benchmarking && timeout 30s ./run_load_test.sh

# Create a flame graph for profiling
flamegraph:
    cd server && cargo flamegraph --bin collider

# Analyze binary size
bloat:
    cd server && cargo bloat --release --crates

# Run simple HTTP benchmarks  
http-bench path="/" requests="1000" concurrency="50":
    hey -n {{requests}} -c {{concurrency}} http://localhost:8080{{path}}

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
    cd infrastructure && just deploy-prod {{project_id}}

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