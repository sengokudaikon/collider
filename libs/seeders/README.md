# Seeders

This library provides a comprehensive seeding system for generating test data in the Collider database.

## Features

- **UserSeeder**: Generates a random entropic number of users (configurable range)
- **EventTypeSeeder**: Creates a smaller random number of event types with realistic names
- **EventSeeder**: Generates millions of events with realistic timestamps and metadata
- **Batch Processing**: Efficiently inserts large amounts of data in configurable batches

## Usage

### Running the Seeder

```bash
# Use environment variables to configure seeding
export DATABASE_URL="postgresql://user:password@localhost:5432/collider"
export MIN_USERS=10000
export MAX_USERS=100000
export MIN_EVENT_TYPES=50
export MAX_EVENT_TYPES=200
export TARGET_EVENTS=10000000
export BATCH_SIZE=10000

# Run the seeder
cargo run --bin seed_database
```

### Configuration Options

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `DATABASE_URL` | `postgresql://user:password@localhost:5432/collider` | PostgreSQL connection string |
| `MIN_USERS` | `10000` | Minimum number of users to generate |
| `MAX_USERS` | `100000` | Maximum number of users to generate |
| `MIN_EVENT_TYPES` | `50` | Minimum number of event types to generate |
| `MAX_EVENT_TYPES` | `200` | Maximum number of event types to generate |
| `TARGET_EVENTS` | `10000000` | Total number of events to generate |
| `BATCH_SIZE` | `10000` | Number of events to insert per batch |

## Generated Data

### Users
- Random realistic names using the `fake` crate
- UUIDs generated with v7 for better database performance
- Distributed creation timestamps

### Event Types
- Realistic event type names following the pattern `{prefix}_{suffix}`
- Common prefixes: user, page, button, form, video, purchase, etc.
- Generated with fake word suffixes

### Events
- Random distribution across all users and event types
- Timestamps spread across the last year
- Rich metadata including:
  - Page URLs and referrers
  - Button clicks with coordinates
  - Form interactions
  - E-commerce data
  - Session information

## Implementation Details

- Uses SeaORM for database interactions
- Implements efficient batch processing to handle millions of records
- Designed to be extensible with additional seeder types
- Proper error handling and logging throughout
- Thread-safe random number generation

## Architecture

The seeding system follows a modular design:

- `Seeder` trait: Common interface for all seeders
- `SeederRunner`: Orchestrates multiple seeders in sequence
- Individual seeder implementations for each data type
- Configurable batch processing for performance