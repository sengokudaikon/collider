#!/bin/bash
# Main deployment script for Collider
# Routes to the unified Terraform deployment

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Route to unified deployment script
exec "$PROJECT_ROOT/infrastructure/terraform/scripts/deploy.sh" "$@"