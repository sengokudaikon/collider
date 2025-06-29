#!/usr/bin/env just --justfile

# Consolidated justfile with essential commands only

# Default recipe to display available commands
default:
    @just --list

# ==== Development ====

# Start development environment with auto-restart
dev:
    DATABASE_URL="postgres://postgres:postgres@localhost:5434/postgres" REDIS_HOST="localhost:6379" RUST_LOG=debug cargo watch -x "run --bin collider"

# Start development environment without auto-restart  
dev-static:
    cd server && DATABASE_URL="postgres://postgres:postgres@localhost:5434/postgres" REDIS_HOST="localhost:6379" RUST_LOG=debug LOG_TO_FILE=true cargo run --bin collider

prod:
    DATABASE_URL="postgres://postgres:postgres@localhost:5434/postgres" REDIS_HOST="localhost:6379" RUST_LOG=info target/release/collider
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
    cargo clippy --all-targets --all-features -- -D warnings

# Security audit
audit:
    cargo audit

# Run all quality checks
quality: format lint audit
    @echo "✅ All quality checks completed!"

# ==== Testing ====

# Run unit tests only
test-unit:
    cargo test --lib

# Run all tests (requires test environment)
test: test-env
    DATABASE_URL="postgres://test_db:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo test --all
    just test-env-down

# Run tests with coverage
coverage: test-env
    DATABASE_URL="postgres://test_db:postgres@localhost:5433/test_db" \
    REDIS_URL="redis://localhost:6380" \
    cargo tarpaulin --all --out Html --output-dir coverage --timeout 180
    just test-env-down
    @echo "✅ Coverage analysis completed!"

# ==== Database ====

# Run migrations
migrate:
    DATABASE_URL="postgres://postgres:postgres@localhost:5434/postgres" \
    cargo run --bin migrator up

# Seed database
seed:
    DATABASE_URL="postgres://postgres:postgres@localhost:5434/postgres" \
    cargo run --bin seeder

# ==== Environment Management ====

# Start test environment
test-env:
    docker-compose -f docker-compose.test.yml up -d
    docker-compose -f docker-compose.test.yml run --rm wait-for-services

# Stop test environment
test-env-down:
    docker-compose -f docker-compose.test.yml down -v

# Start development infrastructure only (for local cargo development)
dev-up:
    docker-compose up -d

# Stop development environment
dev-down:
    docker-compose down

dev-reset:
    docker compose down --remove-orphans --volumes --rmi all && docker compose up -d --build
# Start production environment
prod-up:
    docker-compose -f docker-compose.production.yml up -d

# Stop production environment  
prod-down:
    docker-compose -f docker-compose.production.yml down

# Setup development environment
dev-setup: dev-up
    @echo "Setting up development environment..."
    @if [ ! -f .env ]; then cp .env.example .env && echo "📄 Created .env from template"; fi
    @echo "⏳ Waiting for services to be ready..."
    @sleep 5
    just migrate
    just seed
    @echo "✅ Development environment ready!"

# ==== Build ====

# Build release
build:
    cargo build --release

# Build binaries
build-binaries:
    cargo build --release --bin migrator
    cargo build --release --bin seeder
    cargo build --release --bin collider
    cargo build --release --bin csv-exporter

# ==== Utilities ====

# Install development tools
install-tools:
    cargo install cargo-watch cargo-audit cargo-tarpaulin

# Clean artifacts
clean:
    cargo clean
    rm -rf coverage/

# Show help
help:
    @echo "🚀 Collider - Essential Commands"
    @echo "==============================="
    @echo ""
    @echo "Development:"
    @echo "  just dev              # Run server locally"
    @echo "  just watch            # Watch and rebuild"
    @echo "  just dev-up           # Start docker infrastructure"
    @echo "  just dev-setup        # Setup dev env + migrate + seed"
    @echo "  just prod-up          # Start production environment"
    @echo ""
    @echo "Testing:"
    @echo "  just test             # Run all tests"
    @echo "  just test-unit        # Unit tests only"
    @echo "  just coverage         # Tests with coverage"
    @echo ""
    @echo "Quality:"
    @echo "  just quality          # All checks (format + lint + audit)"
    @echo "  just format           # Format code"
    @echo "  just lint             # Lint code"
    @echo ""
    @echo "Database:"
    @echo "  just migrate          # Run migrations"
    @echo "  just seed             # Seed data"
    @echo ""
    @echo "Build:"
    @echo "  just build            # Build release"
    @echo "  just build-binaries   # Build CLI tools"