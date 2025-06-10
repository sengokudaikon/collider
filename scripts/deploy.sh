#!/bin/bash
set -e

# GCP Deployment Script - Deploy Collider to Cloud Run

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

# Check dependencies
command -v gcloud >/dev/null 2>&1 || error "gcloud CLI is required"
command -v docker >/dev/null 2>&1 || error "Docker is required"

log "Deploying $APP_NAME to GCP Cloud Run"

# Set default project
gcloud config set project "$PROJECT_ID"

# Build and push image
IMAGE_URL="${REGION}-docker.pkg.dev/${PROJECT_ID}/collider/server:latest"
log "Building Docker image..."

cd "$PROJECT_ROOT"
docker build -f Dockerfile -t collider:latest .
docker tag collider:latest "$IMAGE_URL"

log "Pushing image to Artifact Registry..."
docker push "$IMAGE_URL"

# Deploy to Cloud Run
log "Deploying to Cloud Run..."
gcloud run deploy "$SERVICE_NAME" \
    --image="$IMAGE_URL" \
    --region="$REGION" \
    --service-account="${APP_NAME}-cloud-run@${PROJECT_ID}.iam.gserviceaccount.com" \
    --set-env-vars="ENVIRONMENT=production,PORT=8880,RUST_LOG=info" \
    --set-secrets="DATABASE_URL=${APP_NAME}-database-url:latest,REDIS_URL=${APP_NAME}-dragonfly-url:latest" \
    --min-instances="$MIN_INSTANCES" \
    --max-instances="$MAX_INSTANCES" \
    --cpu="$CPU" \
    --memory="$MEMORY" \
    --port=8880 \
    --allow-unauthenticated \
    --timeout=300

# Get service URL
SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" --region="$REGION" --format="value(status.url)")

log "✅ Deployment complete!"
echo ""
echo "🌐 Application URL: $SERVICE_URL"
echo "🔍 Health check: $SERVICE_URL/health"
echo ""
echo "📊 Monitor with:"
echo "  gcloud logging read \"resource.type=cloud_run_revision\" --limit 50"
echo "  gcloud run services logs tail $SERVICE_NAME --region=$REGION"