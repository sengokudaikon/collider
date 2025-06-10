#!/bin/bash
set -e

# GCP Setup Script - One-time setup for Collider

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

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

# Check dependencies
command -v gcloud >/dev/null 2>&1 || error "gcloud CLI is required"

log "Setting up GCP project: $PROJECT_ID"

# Set default project
gcloud config set project "$PROJECT_ID"

# Enable required APIs
log "Enabling required APIs..."
gcloud services enable \
    run.googleapis.com \
    sqladmin.googleapis.com \
    artifactregistry.googleapis.com \
    secretmanager.googleapis.com

# Create Artifact Registry repository
log "Creating Artifact Registry repository..."
gcloud artifacts repositories create collider \
    --repository-format=docker \
    --location="$REGION" \
    --description="Collider container images" \
    2>/dev/null || log "Repository already exists"

# Configure Docker authentication
log "Configuring Docker for Artifact Registry..."
gcloud auth configure-docker "${REGION}-docker.pkg.dev"

# Create database
log "Creating database..."
gcloud sql databases create "$DB_NAME" \
    --instance="$DB_INSTANCE_NAME" \
    2>/dev/null || log "Database already exists"

# Get Cloud SQL connection info
SQL_CONNECTION_NAME=$(gcloud sql instances describe "$DB_INSTANCE_NAME" --format="value(connectionName)")
SQL_IP=$(gcloud sql instances describe "$DB_INSTANCE_NAME" --format="value(ipAddresses[0].ipAddress)")

# Create secrets in Secret Manager
DATABASE_URL="postgresql://${DB_USER}:${DB_PASSWORD}@${SQL_IP}/${DB_NAME}"

log "Creating secrets in Secret Manager..."
echo "$DATABASE_URL" | gcloud secrets create "${APP_NAME}-database-url" --data-file=- 2>/dev/null || {
    echo "$DATABASE_URL" | gcloud secrets versions add "${APP_NAME}-database-url" --data-file=-
}

echo "$REDIS_URL" | gcloud secrets create "${APP_NAME}-dragonfly-url" --data-file=- 2>/dev/null || {
    echo "$REDIS_URL" | gcloud secrets versions add "${APP_NAME}-dragonfly-url" --data-file=-
}

# Create service account for Cloud Run
log "Creating service account..."
gcloud iam service-accounts create "${APP_NAME}-cloud-run" \
    --display-name="Service account for Cloud Run" \
    2>/dev/null || log "Service account already exists"

SERVICE_ACCOUNT="${APP_NAME}-cloud-run@${PROJECT_ID}.iam.gserviceaccount.com"

# Grant permissions
log "Granting permissions..."
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SERVICE_ACCOUNT" \
    --role="roles/cloudsql.client" \
    2>/dev/null || true

gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SERVICE_ACCOUNT" \
    --role="roles/secretmanager.secretAccessor" \
    2>/dev/null || true

log "✅ GCP setup complete!"
echo ""
echo "📋 Configuration Summary:"
echo "  Project ID: $PROJECT_ID"
echo "  Region: $REGION"
echo "  Cloud SQL Instance: $DB_INSTANCE_NAME"
echo "  Database: $DB_NAME"
echo "  Connection Name: $SQL_CONNECTION_NAME"
echo "  Service Account: $SERVICE_ACCOUNT"
echo ""
echo "🔑 Secrets created:"
echo "  ${APP_NAME}-database-url"
echo "  ${APP_NAME}-dragonfly-url"
echo ""
echo "🚀 Next steps:"
echo "  1. Run migrations: ./migrate.sh"
echo "  2. Deploy application: ./deploy.sh"