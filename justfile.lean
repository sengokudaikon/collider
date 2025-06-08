#!/usr/bin/env just --justfile

# Streamlined justfile with core utilities only
# For full pipelines, see justfile.pipeline and justfile.k3s-pipeline

# Default recipe to display available commands
default:
    @just --list

# ==== Core Development ====

# Start development environment
dev:
    cd server && cargo run

# Watch and rebuild on changes
watch:
    cargo watch -x run

# Watch and check syntax
watch-check:
    cargo watch -x check

# ==== Code Quality ====

# Format code
format:
    cargo +nightly fmt

# Lint code
lint:
    cargo clippy -- -D warnings

# Security audit
audit:
    cargo audit

# Check unused dependencies
udeps:
    cargo +nightly udeps --all-targets

# Check unsafe code
geiger:
    cargo geiger

# ==== Testing ====

# Run unit tests only
test-unit:
    cargo test --lib

# Run all tests (requires test environment)
test: test-env
    DATABASE_URL="postgres://postgres:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo test --all
    just test-env-down

# ==== Database ====

# Run migrations
migrate:
    cargo run --bin migrator

# Seed database
seed:
    cargo run --bin seeder

# ==== Environment Management ====

# Start test environment
test-env:
    docker compose -f docker-compose.test.yml up -d
    docker compose -f docker-compose.test.yml run --rm wait-for-services

# Stop test environment
test-env-down:
    docker compose -f docker-compose.test.yml down -v

# Start development environment
dev-up:
    docker compose up -d

# Stop development environment
dev-down:
    docker compose down

# ==== Build ====

# Build release
build:
    cargo build --release

# Build binaries
build-binaries:
    cargo build --release --bin migrator
    cargo build --release --bin seeder

# ==== Utilities ====

# Install development tools
install-tools:
    cargo install cargo-watch cargo-audit cargo-tarpaulin cargo-criterion cargo-bloat cargo-udeps cargo-llvm-lines cargo-geiger

# Clean artifacts
clean:
    cargo clean
    rm -rf coverage/
    rm -rf target/criterion/

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

# Run all quality checks
quality: format lint audit udeps geiger
    @echo "✅ All quality checks completed!"

# Show help
help:
    @echo "🚀 Collider - Streamlined Commands"
    @echo "================================="
    @echo ""
    @echo "Development:"
    @echo "  just dev              # Run server locally"
    @echo "  just watch            # Watch and rebuild"
    @echo "  just dev-up           # Start docker env"
    @echo ""
    @echo "Testing:"
    @echo "  just test             # Run all tests"
    @echo "  just test-unit        # Unit tests only"
    @echo ""
    @echo "Quality:"
    @echo "  just quality          # All checks"
    @echo "  just format           # Format code"
    @echo "  just lint             # Lint code"
    @echo ""
    @echo "Database:"
    @echo "  just migrate          # Run migrations"
    @echo "  just seed             # Seed data"
    @echo ""
    @echo "🔥 For full pipelines:"
    @echo "  just -f justfile.pipeline mega-pipeline    # Complete test + dev + bench workflow"
    @echo "  just -f justfile.k3s-pipeline k3s-pipeline # Same but on K3S"