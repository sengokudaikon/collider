# CLI Documentation

This document describes the command-line interfaces available in the Collider project.

## Database Seeder CLI

The seeder CLI is a powerful tool for populating your database with test data. It provides progress visualization and configurable batch processing.

### Installation

Build the seeder binary:

```bash
cargo build --release -p seeder
```

### Usage

```bash
./target/release/seeder [OPTIONS] <COMMAND>
```

### Global Options

- `-d, --database-url <URL>` - Database connection URL (can also be set via `DATABASE_URL` environment variable)
- `-q, --quiet` - Run without progress visualization
- `-h, --help` - Print help
- `-V, --version` - Print version

### Commands

#### `all` - Seed all data types

Seeds users, event types, and events in sequence with progress tracking.

```bash
./target/release/seeder all [OPTIONS]
```

**Options:**
- `--min-users <NUM>` - Minimum number of users to create (default: 10000)
- `--max-users <NUM>` - Maximum number of users to create (default: 100000)
- `--min-event-types <NUM>` - Minimum number of event types (default: 50)
- `--max-event-types <NUM>` - Maximum number of event types (default: 200)
- `--target-events <NUM>` - Target number of events to create (default: 10000000)
- `--event-batch-size <NUM>` - Batch size for event insertion (default: 10000)

**Example:**
```bash
./target/release/seeder all --min-users 1000 --max-users 10000 --target-events 1000000
```

#### `users` - Seed only users

```bash
./target/release/seeder users [OPTIONS]
```

**Options:**
- `--min-users <NUM>` - Minimum number of users (default: 10000)
- `--max-users <NUM>` - Maximum number of users (default: 100000)

**Example:**
```bash
./target/release/seeder users --min-users 5000 --max-users 15000
```

#### `event-types` - Seed only event types

```bash
./target/release/seeder event-types [OPTIONS]
```

**Options:**
- `--min-types <NUM>` - Minimum number of event types (default: 50)
- `--max-types <NUM>` - Maximum number of event types (default: 200)

**Example:**
```bash
./target/release/seeder event-types --min-types 100 --max-types 300
```

#### `events` - Seed only events

```bash
./target/release/seeder events [OPTIONS]
```

**Options:**
- `--target-events <NUM>` - Target number of events (default: 10000000)
- `--batch-size <NUM>` - Batch size for insertion (default: 10000)

**Example:**
```bash
./target/release/seeder events --target-events 5000000 --batch-size 5000
```

### Configuration

#### Database Connection

The seeder uses the following precedence for database connection:

1. Command line argument: `--database-url`
2. Environment variable: `DATABASE_URL`
3. Default: `postgresql://user:password@localhost:5432/collider`

#### Progress Visualization

By default, the seeder shows a real-time progress UI with:
- Current operation status
- Progress bars for each seeding phase
- Estimated time remaining
- Throughput metrics

Use `--quiet` flag to disable the UI and run in silent mode.

### Performance Tips

1. **Batch Size**: Adjust `--event-batch-size` based on your system's memory and database performance
2. **Connection Pool**: The seeder uses a connection pool with 5-50 connections
3. **Quiet Mode**: Use `--quiet` for better performance when running in scripts
4. **Target Events**: Start with smaller numbers and scale up based on your needs

### Examples

**Quick development setup:**
```bash
./target/release/seeder all --min-users 100 --max-users 1000 --target-events 10000
```

**Production-like data:**
```bash
./target/release/seeder all --min-users 50000 --max-users 500000 --target-events 100000000
```

**Only seed users for testing:**
```bash
./target/release/seeder users --min-users 1000 --max-users 5000
```

## Database Migrator CLI

The migrator provides a TUI (Terminal User Interface) for managing database migrations.

### Usage

```bash
cargo run -p migrator
```

The migrator will start an interactive terminal interface for:
- Running pending migrations
- Rolling back migrations
- Viewing migration status
- Managing database schema versions

### Features

- Interactive TUI with keyboard navigation
- Real-time migration status
- Error handling and rollback capabilities
- Progress tracking for long-running migrations