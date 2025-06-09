#!/usr/bin/env just --justfile

# Main justfile - delegates to specialized justfiles for different workflows
# This is a thin wrapper that routes commands to the appropriate specialized justfile

# Default recipe to display available commands
default:
    @just --list

# ==== Daily Development (delegate to justfile.lean) ====

# Start development environment
dev:
    just -f justfile.lean dev

# Watch and rebuild on changes
watch:
    just -f justfile.lean watch

# Watch and check syntax
watch-check:
    just -f justfile.lean watch-check

# ==== Code Quality (delegate to justfile.lean) ====

# Format code
format:
    just -f justfile.lean format

# Lint code
lint:
    just -f justfile.lean lint

# Security audit
audit:
    just -f justfile.lean audit

# Check unused dependencies
udeps:
    just -f justfile.lean udeps

# Check unsafe code
geiger:
    just -f justfile.lean geiger

# Run all quality checks
quality:
    just -f justfile.lean quality

# ==== Testing (delegate to justfile.lean) ====

# Run unit tests only
test-unit:
    just -f justfile.lean test-unit

# Run all tests
test:
    just -f justfile.lean test

# Run tests with coverage
coverage:
    just -f justfile.lean coverage

# ==== Database (delegate to justfile.lean) ====

# Run migrations
migrate:
    just -f justfile.lean migrate

# Seed database
seed:
    just -f justfile.lean seed

# ==== Environment Management (delegate to justfile.lean) ====

# Start test environment
test-env:
    just -f justfile.lean test-env

# Stop test environment
test-env-down:
    just -f justfile.lean test-env-down

# Start development infrastructure only (for local cargo development)
dev-up:
    just -f justfile.lean dev-up

# Start full development environment including app in Docker
dev-up-full:
    just -f justfile.lean dev-up-full

# Stop development environment
dev-down:
    just -f justfile.lean dev-down

# Stop full development environment
dev-down-full:
    just -f justfile.lean dev-down-full

# Setup development environment (migrations + seeding)
dev-setup:
    just -f justfile.lean dev-setup

# Setup full development environment with app in Docker
dev-setup-full:
    just -f justfile.lean dev-setup-full

# ==== Build (delegate to justfile.lean) ====

# Build release
build:
    just -f justfile.lean build

# Build binaries
build-binaries:
    just -f justfile.lean build-binaries

# ==== Mega Pipelines (delegate to justfile.pipeline) ====

# Run complete docker pipeline (test → coverage → dev → benchmarks)
mega-pipeline:
    @echo "🚀 Starting mega pipeline - this will take 2-3 hours"
    @echo "For more control, use: just -f justfile.pipeline <command>"
    just -f justfile.pipeline mega-pipeline

# Quick docker pipeline
quick-pipeline:
    just -f justfile.pipeline quick-pipeline

# Coverage-only pipeline
coverage-pipeline:
    just -f justfile.pipeline coverage-pipeline

# Benchmark-only pipeline
benchmark-pipeline:
    just -f justfile.pipeline benchmark-pipeline

# ==== K3S Pipelines (delegate to justfile.k3s-pipeline) ====

# Run complete K3S pipeline (test → coverage → K3S deploy → benchmarks)  
k3s-mega-pipeline:
    @echo "☸️ Starting K3S mega pipeline - this will take 3-4 hours"
    @echo "For more control, use: just -f justfile.k3s-pipeline <command>"
    just -f justfile.k3s-pipeline k3s-pipeline

# Quick K3S pipeline
quick-k3s-pipeline:
    just -f justfile.k3s-pipeline quick-k3s-pipeline

# K3S benchmark-only pipeline
k3s-benchmark-pipeline:
    just -f justfile.k3s-pipeline k3s-benchmark-pipeline

# K3S load testing with parameters
k3s-load-test duration="300s" rate="100" users="1000":
    just -f justfile.k3s-pipeline k3s-load-test {{duration}} {{rate}} {{users}}

# ==== K3S Management (delegate to infrastructure/justfile) ====

# Deploy to K3S locally
k3s-deploy:
    cd infrastructure && just deploy-local

# Destroy K3S cluster
k3s-destroy:
    cd infrastructure && just destroy-local

# Check K3S status
k3s-status:
    cd infrastructure && just status

# ==== Utilities ====

# Install development tools
install-tools:
    just -f justfile.lean install-tools

# Clean artifacts
clean:
    just -f justfile.lean clean
    just -f justfile.pipeline clean
    just -f justfile.k3s-pipeline clean

# ==== Help ====

help:
    @echo "🚀 Collider - Streamlined Development Commands"
    @echo "============================================="
    @echo ""
    @echo "📋 Daily Development (→ justfile.lean):"
    @echo "  just dev              # Run server locally"
    @echo "  just watch            # Watch and rebuild"
    @echo "  just dev-up           # Start docker dev infrastructure only"
    @echo "  just dev-up-full      # Start docker dev environment + app"
    @echo "  just dev-setup        # Setup dev env + migrate + seed (local)"
    @echo "  just dev-setup-full   # Setup dev env + migrate + seed (docker)"
    @echo "  just test             # Run all tests"
    @echo "  just quality          # All quality checks"
    @echo ""
    @echo "🔥 Mega Pipelines (→ justfile.pipeline):"
    @echo "  just mega-pipeline       # Complete docker workflow (2-3h)"
    @echo "  just quick-pipeline      # Fast docker workflow (15min)"
    @echo "  just coverage-pipeline   # Coverage testing only"
    @echo "  just benchmark-pipeline  # Benchmarking only"
    @echo ""
    @echo "☸️ K3S Pipelines (→ justfile.k3s-pipeline):"
    @echo "  just k3s-mega-pipeline   # Complete K3S workflow (3-4h)"
    @echo "  just quick-k3s-pipeline  # Fast K3S workflow (20min)"
    @echo "  just k3s-benchmark-pipeline # K3S benchmarking only"
    @echo "  just k3s-load-test 300s 200 1000  # Custom load test"
    @echo ""
    @echo "🔧 K3S Management (→ infrastructure/justfile):"
    @echo "  just k3s-deploy       # Deploy to local K3S"
    @echo "  just k3s-status       # Check K3S cluster"
    @echo "  just k3s-destroy      # Destroy K3S cluster"
    @echo ""
    @echo "📚 Direct Access to Specialized Justfiles:"
    @echo "  just -f justfile.lean help           # Core utilities help"
    @echo "  just -f justfile.pipeline help       # Docker pipeline help"
    @echo "  just -f justfile.k3s-pipeline help   # K3S pipeline help"
    @echo ""
    @echo "💡 Examples:"
    @echo "  just dev && just test                # Develop and test"
    @echo "  just mega-pipeline                   # Full docker workflow"
    @echo "  just k3s-load-test 600s 500 2000    # 10min K3S load test"
    @echo "  just -f justfile.pipeline regression-pipeline baseline_dir"