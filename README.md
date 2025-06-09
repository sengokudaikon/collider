# Collider - High-Performance Event Tracking System

Collider is a high-performance event tracking and analytics system built with Rust, PostgreSQL, and Redis. It provides fast event ingestion, real-time analytics, and comprehensive user management capabilities.

## Features

- **High-throughput event ingestion** - Handle millions of events per second
- **Real-time analytics** - Live metrics and time-series data
- **User management** - Complete CRUD operations with analytics
- **CLI tools** - Database seeding and migration utilities
- **REST API** - Comprehensive API with interactive documentation
- **Monitoring** - Built-in metrics and health checks

## Quick Start

### Using docker-compose (Recommended)

```bash
# Clone the repository
git clone <repository-url>
cd collider

# Start all services
docker-compose up -d

# Check health
curl http://localhost:8080/health
```

Services will be available at:
- **API Server**: http://localhost:8080
- **API Documentation**: http://localhost:8080/docs
- **PostgreSQL**: localhost:5432
- **Redis (Dragonfly)**: localhost:6379
- **Grafana**: http://localhost:3000 (admin/admin)
- **Prometheus**: http://localhost:9090

### Manual Installation

1. **Prerequisites**
   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install PostgreSQL and Redis
   # Ubuntu/Debian:
   sudo apt install postgresql redis-server
   
   # macOS:
   brew install postgresql redis
   ```

2. **Setup Database**
   ```bash
   # Create database
   createdb collider
   
   # Set environment variables
   export DATABASE_URL="postgresql://postgres:postgres@localhost/postgres"
   export REDIS_HOST="127.0.0.1"
   export REDIS_PORT="6379"
   ```

3. **Build and Run**
   ```bash
   # Build the server
   cargo build --release -p server
   
   # Run migrations and start server
   ./target/release/server
   ```

### Using K3s (Production)

For production deployment with Kubernetes:

```bash
# Install K3s
curl -sfL https://get.k3s.io | sh -

# Deploy using Helm
helm install collider ./charts/collider
```

See [setup.md](setup.md) for detailed production deployment instructions.

## Documentation

- **[API Documentation](docs/api.md)** - Complete REST API reference
- **[CLI Documentation](docs/cli.md)** - Command-line tools and utilities
- **[Setup Guide](setup.md)** - Production deployment and configuration

## Architecture

Collider follows a domain-driven design with clear separation of concerns:

```
domains/
├── user/          # User management
├── events/        # Event tracking
└── analytics/     # Real-time analytics

libs/
├── persistence/   # Database abstractions
├── domain/        # Shared domain logic
└── test-utils/    # Testing utilities
```

## API Overview

### Events API
```bash
# Create an event
curl -X POST http://localhost:8080/api/events \
  -H "Content-Type: application/json" \
  -d '{"user_id": "550e8400-e29b-41d4-a716-446655440000", "event_type_id": 1, "metadata": {"page": "/home"}}'

# List events
curl http://localhost:8080/api/events?limit=10
```

### Analytics API
```bash
# Get real-time statistics
curl http://localhost:8080/api/analytics/stats

# Get time series data
curl "http://localhost:8080/api/analytics/metrics/timeseries?from=2024-01-01T00:00:00Z&to=2024-01-02T00:00:00Z"
```

### Users API
```bash
# Create a user
curl -X POST http://localhost:8080/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "John Doe"}'

# Get user with metrics
curl "http://localhost:8080/api/users/550e8400-e29b-41d4-a716-446655440000?include_metrics=true"
```

## CLI Tools

### Database Seeder
```bash
# Build seeder
cargo build --release -p seeder

# Seed development data
./target/release/seeder all --min-users 1000 --max-users 10000 --target-events 100000

# Seed only users
./target/release/seeder users --min-users 5000 --max-users 15000
```

### Database Migrator
```bash
# Run interactive migrator
cargo run -p migrator
```

## Performance

Collider is designed for high performance:

- **Event Ingestion**: 100K+ events/second
- **Query Performance**: Sub-10ms response times
- **Concurrency**: Thousands of concurrent connections
- **Analytics**: Real-time aggregations using materialized views

### Benchmarking

```bash
# Run load tests
cd infrastructure/benchmarking
./run_all_benchmarks.sh

# Run specific benchmarks
cargo bench -p benchmarks
```

## Development

### Running Tests
```bash
# Run all tests
cargo test

# Run tests with coverage
cargo tarpaulin --all-features --workspace --timeout 120
```

### Code Quality
```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Security audit
cargo audit
```

### Using Justfile

The project includes a `justfile` for common tasks:

```bash
# Install just
cargo install just

# See available commands
just --list

# Run development server
just dev

# Run tests
just test

# Build release
just build
```

## Configuration

### Environment Variables

- `DATABASE_URL` - PostgreSQL connection string
- `REDIS_HOST` - Redis host (default: 127.0.0.1)
- `REDIS_PORT` - Redis port (default: 6379)
- `RUST_LOG` - Logging level (default: info)
- `PORT` - Server port (default: 8080)

### Docker Environment

All configuration is managed through `docker-compose.yml` and environment files.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.
