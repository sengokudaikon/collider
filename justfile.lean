#!/usr/bin/env just --justfile

# Streamlined justfile with core utilities only
# For full pipelines, see justfile.pipeline and justfile.k3s-pipeline

# Default recipe to display available commands
default:
    @just --list

# ==== Core Development ====

# Start development environment
dev:
    cd server && DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" REDIS_HOST="localhost" RUST_LOG=info cargo watch -x "run --bin collider"

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
    cargo run --bin migrator

# Seed database
seed:
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

# Start full development environment including app in Docker
dev-up-full:
    docker-compose -f docker-compose.yml -f docker-compose.override.yml up -d

# Stop development environment
dev-down:
    docker-compose down

# Stop full development environment
dev-down-full:
    docker-compose -f docker-compose.yml -f docker-compose.override.yml down

# Start production environment
prod-up:
    docker-compose -f docker-compose.production.yml up -d

# Stop production environment  
prod-down:
    docker-compose -f docker-compose.production.yml down

# ==== Build ====

# Build release
build:
    cargo build --release

# Build binaries
build-binaries:
    cargo build --release --bin migrator
    cargo build --release --bin seeder

# Fast binary builds with native CPU optimization  
build-binaries-native:
    @echo "🚀 Building with native CPU optimizations..."
    RUSTFLAGS="-C target-cpu=native" cargo build --release --bin migrator
    RUSTFLAGS="-C target-cpu=native" cargo build --release --bin seeder

# Ultra-optimized binary builds (requires nightly, MacOS target)
build-binaries-ultra:
    @echo "🔥 Ultra-optimized build with build-std..."
    cargo +nightly build -Z build-std=std,panic_abort --target x86_64-apple-darwin --release --bin migrator
    cargo +nightly build -Z build-std=std,panic_abort --target x86_64-apple-darwin --release --bin seeder

# Size-optimized binary builds (smallest possible)
build-binaries-small:
    @echo "📦 Building for minimum binary size..."
    RUSTFLAGS="-C opt-level=z -C lto=fat -C codegen-units=1 -C strip=symbols" cargo build --release --bin migrator
    RUSTFLAGS="-C opt-level=z -C lto=fat -C codegen-units=1 -C strip=symbols" cargo build --release --bin seeder

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
    @sleep 5
    @echo "🔄 Running database migrations..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" target/release/migrator up
    @echo "🌱 Seeding database with sample data..."
    DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" target/release/seeder all --min-users 100 --max-users 1000 --target-events 10000000
    @echo "✅ Development environment ready!"
    @just dev

# Setup full development environment with app in Docker
dev-setup-full: dev-up-full
    @echo "Setting up full development environment with app in Docker..."
    @if [ ! -f .env ]; then cp .env.example .env && echo "📄 Created .env from template"; fi
    @echo "⏳ Waiting for services to be ready..."
    @sleep 15
    @echo "🔄 Running database migrations..."
    docker-compose -f docker-compose.yml -f docker-compose.production.yml exec app cargo run --bin migrator -- up
    @echo "🌱 Seeding database with sample data..."
    docker-compose -f docker-compose.yml -f docker-compose.production.yml exec app cargo run --bin seeder -- all --min-users 100 --max-users 1000 --target-events 10000000
    @echo "✅ Full development environment ready!"
    @echo "🌐 App running at http://localhost:8880"

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
    @echo "  just dev-up           # Start docker infrastructure only"
    @echo "  just dev-up-full      # Start docker environment + app"
    @echo "  just dev-setup        # Setup dev env + migrate + seed (local)"
    @echo "  just dev-setup-full   # Setup dev env + migrate + seed (docker)"
    @echo "  just prod-up          # Start production docker environment"
    @echo "  just prod-down        # Stop production docker environment"
    @echo ""
    @echo "Testing:"
    @echo "  just test             # Run all tests"
    @echo "  just test-unit        # Unit tests only"
    @echo "  just coverage         # Tests with coverage"
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
    @echo "Performance Testing:"
    @echo "  just perf-smoke       # K6 smoke tests"
    @echo "  just perf-load        # K6 load tests"
    @echo "  just perf-stress      # K6 stress tests"
    @echo "  just perf-10k         # Full 10k+ RPS benchmark"
    @echo "  just bench            # Criterion benchmarks"
    @echo ""
    @echo "🔥 For full pipelines:"
    @echo "  just -f justfile.pipeline mega-pipeline    # Complete test + dev + bench workflow"
    @echo "  just -f justfile.k3s-pipeline k3s-pipeline # Same but on K3S"

# ==== Performance Testing ====

# Run K6 smoke tests (quick validation)
perf-smoke BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh smoke smoke {{BASE_URL}}

# Run K6 load tests 
perf-load BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh load load {{BASE_URL}}

# Run K6 stress tests
perf-stress BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh stress stress {{BASE_URL}}

# Run full 10k+ RPS benchmark suite
perf-10k BASE_URL="http://localhost:8880":
    @echo "🎯 Running full 10k+ RPS production readiness test"
    @echo "This will take 60+ minutes and stress test all endpoints"
    cd k6-tests && ./run-tests.sh 10k-rps stress {{BASE_URL}}

# Run 10 million event seeding test
perf-seed-10m BASE_URL="http://localhost:8880":
    @echo "🌱 Starting 10 million event seeding test"
    @echo "⚠️ This will take 3+ hours to complete"
    cd k6-tests && ./run-tests.sh seeding 10million {{BASE_URL}}

# Run individual POST events stress test
perf-post PROFILE="stress" BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh post {{PROFILE}} {{BASE_URL}}

# Run individual GET events stress test  
perf-get PROFILE="stress" BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh get {{PROFILE}} {{BASE_URL}}

# Run analytics endpoints stress test
perf-analytics PROFILE="stress" BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh analytics {{PROFILE}} {{BASE_URL}}

# Run delete operations stress test
perf-delete PROFILE="stress" BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh delete {{PROFILE}} {{BASE_URL}}

# Run full system mixed workload test
perf-full-system PROFILE="stress" BASE_URL="http://localhost:8880":
    cd k6-tests && ./run-tests.sh full-system {{PROFILE}} {{BASE_URL}}

# Run Criterion benchmarks
bench:
    cd benchmarks && cargo bench

# Run Criterion HTTP benchmarks only
bench-http:
    cd benchmarks && cargo bench http_benches

# Run Criterion CLI benchmarks only  
bench-cli:
    cd benchmarks && cargo bench cli_benches