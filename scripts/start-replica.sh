#!/bin/bash
set -e

echo "🚀 Starting Collider with PostgreSQL read replica (simplified setup)..."

# Stop any existing containers and volumes
echo "🧹 Cleaning up existing containers and volumes..."
docker-compose -f docker-compose.replica.yml down -v || true
docker-compose -f docker-compose.yml down || true

# Force remove volumes to ensure fresh start
echo "🗑️  Removing old volumes..."
docker volume rm -f collider_postgres_primary_data collider_postgres_replica_data 2>/dev/null || true

# Start the services
echo "📦 Starting services..."
docker-compose -f docker-compose.replica.yml up -d

# Wait for primary to be healthy
echo "⏳ Waiting for primary database..."
until docker exec collider_postgres_primary pg_isready -U postgres >/dev/null 2>&1; do
  sleep 1
  echo -n "."
done
echo " ✅"

# Wait a bit more for replica to complete setup
echo "⏳ Waiting for replica setup to complete..."
sleep 10

# Check if replica is ready
echo "🔍 Checking replica status..."
if docker exec collider_postgres_replica pg_isready -U postgres >/dev/null 2>&1; then
  echo "✅ Replica is ready!"
else
  echo "⚠️  Replica might still be initializing. Check logs with:"
  echo "  docker logs collider_postgres_replica"
fi

# Show replication status
echo ""
echo "📊 Replication Status:"
docker exec collider_postgres_primary psql -U postgres -c "SELECT client_addr, state, sync_state FROM pg_stat_replication;" 2>/dev/null || echo "No replication info yet"

echo ""
echo "✅ Setup complete!"
echo ""
echo "🔗 Connection Details:"
echo "  Primary (writes) via PgBouncer:  postgresql://postgres:postgres@localhost:6434/postgres"
echo "  Replica (reads) via PgBouncer:   postgresql://postgres:postgres@localhost:6435/postgres"
echo "  Primary direct:                  postgresql://postgres:postgres@localhost:5434/postgres"
echo "  Replica direct:                  postgresql://postgres:postgres@localhost:5435/postgres"
echo ""
echo "📝 To use in your application:"
echo "  export DATABASE_URL=postgresql://postgres:postgres@localhost:6434/postgres"
echo "  export DATABASE_READ_REPLICA_URL=postgresql://postgres:postgres@localhost:6435/postgres"
echo ""
echo "🔧 Useful commands:"
echo "  Check replication lag:  docker exec collider_postgres_replica psql -U postgres -c \"SELECT pg_last_wal_receive_lsn() - pg_last_wal_replay_lsn() as replication_lag_bytes;\""
echo "  View logs:             docker logs collider_postgres_replica"
echo "  Stop everything:       docker-compose -f docker-compose.replica.yml down"