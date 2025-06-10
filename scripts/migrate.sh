#!/bin/bash
set -e

# Database Migration Script for GCP Cloud SQL

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Load configuration
if [[ ! -f "$SCRIPT_DIR/config.env" ]]; then
    echo "❌ config.env not found. Copy config.env.example and configure it."
    exit 1
fi

source "$SCRIPT_DIR/config.env"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check if Cloud SQL Proxy is available
if ! command -v cloud_sql_proxy >/dev/null 2>&1; then
    warn "Cloud SQL Proxy not found. Installing..."
    
    # Download Cloud SQL Proxy
    curl -o cloud_sql_proxy https://dl.google.com/cloudsql/cloud_sql_proxy.linux.amd64
    chmod +x cloud_sql_proxy
    sudo mv cloud_sql_proxy /usr/local/bin/
fi

# Get Cloud SQL connection name
CONNECTION_NAME=$(gcloud sql instances describe "$DB_INSTANCE_NAME" --format="value(connectionName)")

log "Starting Cloud SQL Proxy..."
cloud_sql_proxy -instances="$CONNECTION_NAME"=tcp:5433 &
PROXY_PID=$!

# Wait for proxy to be ready
sleep 5

# Get database URL from Secret Manager
log "Getting database credentials from Secret Manager..."
DATABASE_URL=$(gcloud secrets versions access latest --secret="${APP_NAME}-database-url")

# Run migrations
log "Running database migrations..."
cd "$PROJECT_ROOT"

# Use the proxy connection
PROXY_DATABASE_URL="postgresql://${DB_USER}:$(gcloud secrets versions access latest --secret="${APP_NAME}-database-url" | cut -d':' -f3 | cut -d'@' -f1)@localhost:5433/${DB_NAME}"

DATABASE_URL="$PROXY_DATABASE_URL" cargo run --bin migrator -- up

log "✅ Migrations completed successfully!"

# Kill proxy
kill $PROXY_PID 2>/dev/null || true

echo ""
echo "📋 Database is ready for the application"