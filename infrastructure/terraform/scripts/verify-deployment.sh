#!/bin/bash
# Collider Deployment Verification Script
# Verifies that K3S deployment is working correctly

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TERRAFORM_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
ENVIRONMENT=""
NAMESPACE="collider"
TIMEOUT=300

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

success() {
    echo -e "${GREEN}✓ $1${NC}"
}

failure() {
    echo -e "${RED}✗ $1${NC}"
}

# Usage function
usage() {
    cat << EOF
Collider Deployment Verification Script

USAGE:
    $0 -e ENVIRONMENT [OPTIONS]

ENVIRONMENTS:
    local    Verify local K3D cluster deployment
    prod     Verify production K3S cluster deployment

OPTIONS:
    -e, --environment ENV    Environment to verify (local|prod) [REQUIRED]
    -n, --namespace NS       Kubernetes namespace (default: collider)
    -t, --timeout SECONDS    Timeout for checks (default: 300)
    -h, --help              Show this help message

EXAMPLES:
    # Verify local deployment
    $0 -e local

    # Verify production deployment with custom timeout
    $0 -e prod -t 600
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -e|--environment)
            ENVIRONMENT="$2"
            shift 2
            ;;
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
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

# Check if kubectl is available and configured
check_kubectl() {
    log "Checking kubectl configuration..."
    
    if ! command -v kubectl >/dev/null 2>&1; then
        error "kubectl is not installed or not in PATH"
    fi
    
    if ! kubectl cluster-info >/dev/null 2>&1; then
        error "kubectl is not configured or cluster is not accessible"
    fi
    
    local context=$(kubectl config current-context 2>/dev/null || echo "none")
    info "Current kubectl context: $context"
    
    success "kubectl is configured and cluster is accessible"
}

# Check namespace exists
check_namespace() {
    log "Checking namespace '$NAMESPACE'..."
    
    if kubectl get namespace "$NAMESPACE" >/dev/null 2>&1; then
        success "Namespace '$NAMESPACE' exists"
    else
        failure "Namespace '$NAMESPACE' does not exist"
        return 1
    fi
}

# Check all pods are ready
check_pods() {
    log "Checking pod status in namespace '$NAMESPACE'..."
    
    local pods=$(kubectl get pods -n "$NAMESPACE" --no-headers 2>/dev/null | wc -l || echo "0")
    if [[ "$pods" -eq 0 ]]; then
        failure "No pods found in namespace '$NAMESPACE'"
        return 1
    fi
    
    info "Found $pods pods in namespace '$NAMESPACE'"
    
    # Show pod status
    kubectl get pods -n "$NAMESPACE" -o wide
    
    # Check if all pods are ready
    local not_ready=$(kubectl get pods -n "$NAMESPACE" --no-headers 2>/dev/null | grep -v "Running\|Completed" | wc -l || echo "0")
    
    if [[ "$not_ready" -eq 0 ]]; then
        success "All pods are running"
    else
        warn "$not_ready pods are not in Running state"
        kubectl get pods -n "$NAMESPACE" | grep -v "Running\|Completed" || true
        return 1
    fi
}

# Check services are accessible
check_services() {
    log "Checking services in namespace '$NAMESPACE'..."
    
    local services=$(kubectl get services -n "$NAMESPACE" --no-headers 2>/dev/null | wc -l || echo "0")
    if [[ "$services" -eq 0 ]]; then
        failure "No services found in namespace '$NAMESPACE'"
        return 1
    fi
    
    info "Found $services services in namespace '$NAMESPACE'"
    kubectl get services -n "$NAMESPACE" -o wide
    
    success "Services are configured"
}

# Check application health endpoints
check_health_endpoints() {
    log "Checking application health endpoints..."
    
    local health_checks=0
    local health_passed=0
    
    # Determine how to access services based on environment
    if [[ "$ENVIRONMENT" == "local" ]]; then
        # Local: Use NodePort
        local endpoints=(
            "http://localhost:30080/health:Application"
            "http://localhost:30090/-/healthy:Prometheus"
            "http://localhost:30300/api/health:Grafana"
        )
    else
        # Production: Get external IP and use NodePort
        local external_ip=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="ExternalIP")].address}' 2>/dev/null || echo "")
        if [[ -z "$external_ip" ]]; then
            warn "Could not determine external IP, skipping external health checks"
            return 0
        fi
        
        local endpoints=(
            "http://$external_ip:30080/health:Application"
            "http://$external_ip:30090/-/healthy:Prometheus"
            "http://$external_ip:30300/api/health:Grafana"
        )
    fi
    
    for endpoint_info in "${endpoints[@]}"; do
        local url="${endpoint_info%:*}"
        local name="${endpoint_info#*:}"
        
        health_checks=$((health_checks + 1))
        
        info "Checking $name health at $url..."
        
        # Use timeout to avoid hanging
        if timeout 10 curl -s -f "$url" >/dev/null 2>&1; then
            success "$name is healthy"
            health_passed=$((health_passed + 1))
        else
            warn "$name health check failed"
        fi
    done
    
    if [[ "$health_passed" -eq "$health_checks" ]]; then
        success "All health checks passed ($health_passed/$health_checks)"
    else
        warn "Some health checks failed ($health_passed/$health_checks)"
        return 1
    fi
}

# Check persistent volumes
check_persistent_volumes() {
    log "Checking persistent volumes..."
    
    local pvcs=$(kubectl get pvc -n "$NAMESPACE" --no-headers 2>/dev/null | wc -l || echo "0")
    if [[ "$pvcs" -eq 0 ]]; then
        info "No persistent volume claims found"
        return 0
    fi
    
    info "Found $pvcs persistent volume claims"
    kubectl get pvc -n "$NAMESPACE"
    
    # Check if all PVCs are bound
    local unbound=$(kubectl get pvc -n "$NAMESPACE" --no-headers 2>/dev/null | grep -v "Bound" | wc -l || echo "0")
    
    if [[ "$unbound" -eq 0 ]]; then
        success "All persistent volume claims are bound"
    else
        warn "$unbound persistent volume claims are not bound"
        kubectl get pvc -n "$NAMESPACE" | grep -v "Bound" || true
        return 1
    fi
}

# Check ingress configuration
check_ingress() {
    log "Checking ingress configuration..."
    
    local ingresses=$(kubectl get ingress -n "$NAMESPACE" --no-headers 2>/dev/null | wc -l || echo "0")
    if [[ "$ingresses" -eq 0 ]]; then
        info "No ingress resources found"
        return 0
    fi
    
    info "Found $ingresses ingress resources"
    kubectl get ingress -n "$NAMESPACE" -o wide
    
    success "Ingress configuration verified"
}

# Check resource usage
check_resource_usage() {
    log "Checking resource usage..."
    
    if kubectl top nodes >/dev/null 2>&1; then
        info "Node resource usage:"
        kubectl top nodes
        
        info "Pod resource usage in namespace '$NAMESPACE':"
        kubectl top pods -n "$NAMESPACE" 2>/dev/null || info "Pod metrics not available"
    else
        info "Metrics server not available, skipping resource usage check"
    fi
}

# Check logs for errors
check_logs() {
    log "Checking recent logs for errors..."
    
    # Get all pods in namespace
    local pods=$(kubectl get pods -n "$NAMESPACE" -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")
    
    if [[ -z "$pods" ]]; then
        warn "No pods found for log checking"
        return 0
    fi
    
    local error_count=0
    
    for pod in $pods; do
        info "Checking logs for pod: $pod"
        
        # Check for recent errors (last 50 lines)
        local errors=$(kubectl logs "$pod" -n "$NAMESPACE" --tail=50 2>/dev/null | grep -i "error\|failed\|exception" | wc -l || echo "0")
        
        if [[ "$errors" -gt 0 ]]; then
            warn "Found $errors error-like entries in $pod logs"
            error_count=$((error_count + 1))
        else
            success "No obvious errors in $pod logs"
        fi
    done
    
    if [[ "$error_count" -eq 0 ]]; then
        success "No errors found in recent logs"
    else
        warn "Found errors in $error_count pod logs"
        return 1
    fi
}

# Generate verification report
generate_report() {
    log "Generating verification report..."
    
    local report_file="/tmp/collider-verification-$ENVIRONMENT-$(date +%Y%m%d-%H%M%S).txt"
    
    {
        echo "Collider Deployment Verification Report"
        echo "======================================="
        echo "Environment: $ENVIRONMENT"
        echo "Namespace: $NAMESPACE"
        echo "Timestamp: $(date)"
        echo "Kubectl Context: $(kubectl config current-context 2>/dev/null || echo "unknown")"
        echo ""
        
        echo "=== Cluster Information ==="
        kubectl cluster-info
        echo ""
        
        echo "=== Nodes ==="
        kubectl get nodes -o wide
        echo ""
        
        echo "=== Pods ==="
        kubectl get pods -n "$NAMESPACE" -o wide
        echo ""
        
        echo "=== Services ==="
        kubectl get services -n "$NAMESPACE" -o wide
        echo ""
        
        echo "=== Persistent Volume Claims ==="
        kubectl get pvc -n "$NAMESPACE" 2>/dev/null || echo "No PVCs found"
        echo ""
        
        echo "=== Ingress ==="
        kubectl get ingress -n "$NAMESPACE" 2>/dev/null || echo "No ingress found"
        echo ""
        
        if kubectl top nodes >/dev/null 2>&1; then
            echo "=== Resource Usage ==="
            kubectl top nodes
            echo ""
            kubectl top pods -n "$NAMESPACE" 2>/dev/null || echo "Pod metrics not available"
            echo ""
        fi
    } > "$report_file"
    
    info "Verification report saved to: $report_file"
}

# Main verification flow
main() {
    log "Starting Collider deployment verification"
    log "Environment: $ENVIRONMENT"
    log "Namespace: $NAMESPACE"
    log "Timeout: ${TIMEOUT}s"
    echo ""
    
    local failed_checks=0
    local total_checks=0
    
    # Run all verification checks
    local checks=(
        "check_kubectl"
        "check_namespace"
        "check_pods"
        "check_services"
        "check_persistent_volumes"
        "check_ingress"
        "check_health_endpoints"
        "check_resource_usage"
        "check_logs"
    )
    
    for check in "${checks[@]}"; do
        total_checks=$((total_checks + 1))
        
        echo ""
        if ! $check; then
            failed_checks=$((failed_checks + 1))
        fi
    done
    
    echo ""
    generate_report
    
    # Summary
    echo ""
    log "Verification Summary"
    echo "===================="
    echo "Total checks: $total_checks"
    echo "Passed: $((total_checks - failed_checks))"
    echo "Failed: $failed_checks"
    
    if [[ "$failed_checks" -eq 0 ]]; then
        success "All verification checks passed! Deployment is healthy."
        
        # Show access information
        echo ""
        info "Access Information:"
        if [[ "$ENVIRONMENT" == "local" ]]; then
            echo "  Application: http://localhost:30080"
            echo "  Prometheus:  http://localhost:30090"
            echo "  Grafana:     http://localhost:30300 (admin/$(cd "$TERRAFORM_DIR" && terraform output -raw secrets 2>/dev/null | jq -r '.grafana_password' 2>/dev/null || echo 'check terraform output'))"
            echo "  Jaeger:      http://localhost:30686"
        else
            local external_ip=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="ExternalIP")].address}' 2>/dev/null || echo "EXTERNAL_IP")
            echo "  Application: http://$external_ip:30080"
            echo "  Prometheus:  http://$external_ip:30090"
            echo "  Grafana:     http://$external_ip:30300"
            echo "  Jaeger:      http://$external_ip:30686"
        fi
        
        exit 0
    else
        failure "Some verification checks failed. Please review the output above."
        exit 1
    fi
}

# Run main function
main "$@"