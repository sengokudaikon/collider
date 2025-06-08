#!/bin/bash
# Collider Unified Deployment Script
# Supports local K3S and production K3S deployments via Terraform + Helm

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TERRAFORM_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(cd "$TERRAFORM_DIR/../.." && pwd)"
CHARTS_DIR="$PROJECT_ROOT/charts"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
ENVIRONMENT=""
PLAN_ONLY=false
DESTROY=false
AUTO_APPROVE=false
GCP_PROJECT_ID=""
VERIFY=true

# Logging functions
log() {
    echo -e "${GREEN}[$(date +'%H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[$(date +'%H:%M:%S')] WARNING: $1${NC}"
}

error() {
    echo -e "${RED}[$(date +'%H:%M:%S')] ERROR: $1${NC}"
    exit 1
}

info() {
    echo -e "${BLUE}[$(date +'%H:%M:%S')] $1${NC}"
}

# Usage function
usage() {
    cat << EOF
Collider Deployment Script

USAGE:
    $0 -e ENVIRONMENT [OPTIONS]

ENVIRONMENTS:
    local    Deploy to local K3D cluster (auto-created)
    prod     Deploy to production K3S cluster on GCP

OPTIONS:
    -e, --environment ENV    Environment to deploy (local|prod) [REQUIRED]
    -p, --plan-only         Show deployment plan without applying
    -d, --destroy           Destroy infrastructure instead of creating
    -a, --auto-approve      Auto-approve Terraform changes (use with caution)
    -g, --gcp-project ID    GCP project ID (required for prod environment)
    --no-verify             Skip post-deployment verification
    -h, --help              Show this help message

EXAMPLES:
    # Local development deployment
    $0 -e local

    # Production deployment with project ID
    $0 -e prod -g my-gcp-project-id

    # Show production deployment plan
    $0 -e prod -g my-gcp-project-id -p

    # Destroy local environment
    $0 -e local -d

REQUIREMENTS:
    Local:  k3d, kubectl, helm, terraform
    Prod:   kubectl, helm, terraform, gcloud
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -e|--environment)
            ENVIRONMENT="$2"
            shift 2
            ;;
        -p|--plan-only)
            PLAN_ONLY=true
            shift
            ;;
        -d|--destroy)
            DESTROY=true
            shift
            ;;
        -a|--auto-approve)
            AUTO_APPROVE=true
            shift
            ;;
        -g|--gcp-project)
            GCP_PROJECT_ID="$2"
            shift 2
            ;;
        --no-verify)
            VERIFY=false
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            ;;
    esac
done

# Validate required parameters
if [[ -z "$ENVIRONMENT" ]]; then
    error "Environment is required. Use -e local or -e prod"
fi

if [[ "$ENVIRONMENT" != "local" && "$ENVIRONMENT" != "prod" ]]; then
    error "Environment must be 'local' or 'prod'"
fi

if [[ "$ENVIRONMENT" == "prod" && -z "$GCP_PROJECT_ID" ]]; then
    error "GCP project ID is required for prod environment. Use -g YOUR_PROJECT_ID"
fi

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites for $ENVIRONMENT deployment..."
    
    local missing_tools=()
    
    # Common tools
    command -v terraform >/dev/null 2>&1 || missing_tools+=("terraform")
    command -v kubectl >/dev/null 2>&1 || missing_tools+=("kubectl")
    command -v helm >/dev/null 2>&1 || missing_tools+=("helm")
    
    # Environment-specific tools
    if [[ "$ENVIRONMENT" == "local" ]]; then
        command -v k3d >/dev/null 2>&1 || missing_tools+=("k3d")
        command -v docker >/dev/null 2>&1 || missing_tools+=("docker")
    else
        command -v gcloud >/dev/null 2>&1 || missing_tools+=("gcloud")
    fi
    
    if [[ ${#missing_tools[@]} -ne 0 ]]; then
        error "Missing required tools: ${missing_tools[*]}"
    fi
    
    log "All prerequisites satisfied"
}

# Build application image for local deployment
build_local_image() {
    if [[ "$ENVIRONMENT" == "local" ]]; then
        log "Building application image for local deployment..."
        
        # Build the Docker image
        cd "$PROJECT_ROOT"
        docker build -t localhost:5001/collider:latest -f server/Dockerfile .
        
        # Push to local registry (created by k3d)
        if docker ps --format '{{.Names}}' | grep -q "k3d-collider-registry"; then
            docker push localhost:5001/collider:latest
            log "Application image pushed to local registry"
        else
            warn "Local registry not running yet, image will be pushed after cluster creation"
        fi
    fi
}

# Initialize Terraform
init_terraform() {
    log "Initializing Terraform..."
    cd "$TERRAFORM_DIR"
    terraform init
}

# Prepare Terraform variables
prepare_terraform_vars() {
    log "Preparing Terraform variables for $ENVIRONMENT..."
    
    cd "$TERRAFORM_DIR"
    
    # Create terraform.tfvars file
    cat > terraform.tfvars << EOF
# Collider Infrastructure Configuration
environment = "$ENVIRONMENT"
EOF
    
    if [[ "$ENVIRONMENT" == "prod" ]]; then
        cat >> terraform.tfvars << EOF
gcp_project_id = "$GCP_PROJECT_ID"
EOF
    fi
    
    log "Terraform variables prepared"
}

# Plan deployment
plan_deployment() {
    log "Planning $ENVIRONMENT deployment..."
    cd "$TERRAFORM_DIR"
    terraform plan -var-file=terraform.tfvars
}

# Apply deployment
apply_deployment() {
    log "Applying $ENVIRONMENT deployment..."
    cd "$TERRAFORM_DIR"
    
    local tf_args=("-var-file=terraform.tfvars")
    
    if [[ "$AUTO_APPROVE" == true ]]; then
        tf_args+=("-auto-approve")
    fi
    
    if [[ "$DESTROY" == true ]]; then
        terraform destroy "${tf_args[@]}"
        log "Infrastructure destroyed successfully"
        return 0
    else
        # For local deployment, apply in stages to handle cluster creation
        if [[ "$ENVIRONMENT" == "local" ]]; then
            log "Creating K3D cluster first..."
            terraform apply "${tf_args[@]}" -target=null_resource.k3d_cluster -auto-approve
            
            # Wait for cluster to be ready and set context
            sleep 5
            kubectl config use-context k3d-collider-local 2>/dev/null || log "Context will be set after cluster creation"
            
            log "Deploying applications..."
            terraform apply "${tf_args[@]}"
        else
            terraform apply "${tf_args[@]}"
        fi
        log "Infrastructure deployed successfully"
    fi
    
    # Get outputs
    terraform output -json > /tmp/terraform-outputs.json
    log "Terraform outputs saved to /tmp/terraform-outputs.json"
}

# Post-deployment tasks
post_deployment() {
    if [[ "$DESTROY" == true ]]; then
        return 0
    fi
    
    log "Running post-deployment tasks..."
    
    # Push local image after cluster is created
    if [[ "$ENVIRONMENT" == "local" ]]; then
        # Wait for registry to be ready
        sleep 10
        docker push localhost:5001/collider:latest || warn "Failed to push image to local registry"
    fi
    
    # Wait for all pods to be ready
    log "Waiting for all pods to be ready..."
    kubectl wait --for=condition=ready pod --all -n collider --timeout=300s || warn "Some pods may not be ready"
    
    log "Post-deployment tasks completed"
}

# Verify deployment
verify_deployment() {
    if [[ "$VERIFY" == false || "$DESTROY" == true ]]; then
        return 0
    fi
    
    log "Verifying deployment..."
    
    # Check if verification script exists
    local verify_script="$SCRIPT_DIR/verify-deployment.sh"
    if [[ -f "$verify_script" ]]; then
        "$verify_script" -e "$ENVIRONMENT"
    else
        warn "Verification script not found at $verify_script"
        
        # Basic verification
        log "Running basic verification..."
        kubectl get pods -n collider
        kubectl get services -n collider
        
        if [[ "$ENVIRONMENT" == "local" ]]; then
            info "Local endpoints:"
            info "  Application: http://localhost:30080"
            info "  Prometheus:  http://localhost:30090"
            info "  Grafana:     http://localhost:30300"
            info "  Jaeger:      http://localhost:30686"
        fi
    fi
}

# Show deployment summary
show_summary() {
    if [[ "$DESTROY" == true ]]; then
        log "Deployment destroyed successfully"
        return 0
    fi
    
    log "Deployment Summary"
    echo "=================="
    echo "Environment: $ENVIRONMENT"
    echo "Status: $(if [[ "$PLAN_ONLY" == true ]]; then echo "PLANNED"; else echo "DEPLOYED"; fi)"
    echo
    
    if [[ -f "/tmp/terraform-outputs.json" ]]; then
        info "Endpoints:"
        if command -v jq >/dev/null 2>&1; then
            jq -r '.endpoints.value | to_entries[] | "  \(.key): \(.value)"' /tmp/terraform-outputs.json 2>/dev/null || echo "  (Use terraform output to see endpoints)"
        else
            echo "  (Install jq to see formatted endpoints or use: cd $TERRAFORM_DIR && terraform output)"
        fi
    fi
    
    echo
    info "Next steps:"
    if [[ "$ENVIRONMENT" == "local" ]]; then
        echo "  1. Access your application at http://localhost:30080"
        echo "  2. View monitoring at http://localhost:30300 (Grafana)"
        echo "  3. Check logs: kubectl logs -n collider -l app=collider-app"
    else
        echo "  1. Get external IP: kubectl get nodes -o wide"
        echo "  2. Access via NodePort or configure LoadBalancer"
        echo "  3. Set up DNS for production domain"
    fi
    echo "  4. Run tests: just test"
    echo "  5. Check status: kubectl get pods -n collider"
}

# Main deployment flow
main() {
    log "Starting Collider deployment"
    log "Environment: $ENVIRONMENT"
    log "Operation: $(if [[ "$DESTROY" == true ]]; then echo "DESTROY"; elif [[ "$PLAN_ONLY" == true ]]; then echo "PLAN"; else echo "DEPLOY"; fi)"
    
    check_prerequisites
    
    if [[ "$DESTROY" == false ]]; then
        build_local_image
    fi
    
    init_terraform
    prepare_terraform_vars
    
    if [[ "$PLAN_ONLY" == true ]]; then
        plan_deployment
    else
        apply_deployment
        post_deployment
        verify_deployment
    fi
    
    show_summary
    
    log "Deployment completed successfully!"
}

# Trap to cleanup on exit
trap 'echo -e "\n${YELLOW}Deployment interrupted${NC}"' INT TERM

# Run main function
main "$@"