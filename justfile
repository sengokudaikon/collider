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

# Start production docker environment
prod-up:
    just -f justfile.lean prod-up

# Stop production docker environment
prod-down:
    just -f justfile.lean prod-down

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

# ==== GCP Deployment ====

# Setup GCP infrastructure (one-time)
gcp-setup:
    cd scripts && ./setup.sh

# Deploy to GCP Cloud Run
gcp-deploy:
    cd scripts && ./deploy.sh

# Run database migrations on GCP
gcp-migrate:
    cd scripts && ./migrate.sh

# Full GCP deployment (setup + migrate + deploy)
gcp-full-deploy:
    @echo "🚀 Full GCP deployment - this may take 10-15 minutes"
    just gcp-setup
    just gcp-migrate 
    just gcp-deploy

# ==== Utilities ====

# Install development tools
install-tools:
    just -f justfile.lean install-tools

# Clean artifacts
clean:
    just -f justfile.lean clean
    just -f justfile.pipeline clean

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
    @echo "  just prod-up          # Start production docker environment"
    @echo "  just test             # Run all tests"
    @echo "  just quality          # All quality checks"
    @echo ""
    @echo "☁️ GCP Deployment:"
    @echo "  just gcp-setup           # One-time GCP infrastructure setup"
    @echo "  just gcp-deploy          # Deploy app to Cloud Run"
    @echo "  just gcp-migrate         # Run database migrations on GCP"
    @echo "  just gcp-full-deploy     # Complete setup + migrate + deploy"
    @echo ""
    @echo "📚 Direct Access to Specialized Justfiles:"
    @echo "  just -f justfile.lean help           # Core utilities help"
    @echo ""
    @echo "💡 Examples:"
    @echo "  just dev && just test                # Develop and test"
    @echo "  just mega-pipeline                   # Full docker workflow"
    @echo "  just gcp-full-deploy                 # Deploy to GCP"
    @echo "  just -f justfile.pipeline regression-pipeline baseline_dir"