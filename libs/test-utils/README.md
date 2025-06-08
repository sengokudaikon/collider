# Test Utils

This library contains utilities for testing and database management.

## Migration Tool

The `migrator` binary provides both CLI and interactive TUI interfaces for managing database migrations.

### Usage

#### CLI Mode

```bash
# Run all pending migrations
cargo run --bin migrator up

# Roll back the last migration
cargo run --bin migrator down

# Roll back multiple migrations
cargo run --bin migrator down --steps 3

# Show migration status
cargo run --bin migrator status

# Reset all migrations (WARNING: destructive!)
cargo run --bin migrator reset

# Custom database URL
cargo run --bin migrator --database-url "postgresql://user:pass@host:port/db" status
```

#### Interactive TUI Mode

```bash
# Start interactive mode (default when no command specified)
cargo run --bin migrator

# Or explicitly
cargo run --bin migrator tui
```

The TUI provides:
- ↑/↓ or j/k to navigate menu
- Enter/Space to select actions
- q/Esc to quit
- y/n for confirmations

### Features

- **Up migrations**: Apply all pending migrations
- **Down migrations**: Roll back migrations one by one with confirmation
- **Reset**: Complete database reset (with safety confirmation)
- **Status**: View applied migrations
- **Interactive TUI**: Full-screen interface with confirmation dialogs
- **Safety checks**: Confirmation dialogs for destructive operations

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string (default: `postgresql://postgres:password@localhost:5432/collider`)