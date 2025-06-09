# Collider Infrastructure - K3S + Terraform + Helm

This directory contains the consolidated infrastructure configuration for Collider, using **Terraform** to manage **K3S clusters** with **Helm** for application deployment.

## 🏗️ Architecture Overview

**Unified K3S-based deployment** supporting two environments:
- **Local**: K3D cluster on your development machine
- **Production**: K3S single-node cluster on Google Cloud Platform

### Technology Stack
- **Terraform**: Infrastructure as Code for cluster and cloud resources
- **K3S/K3D**: Lightweight Kubernetes for both local and production
- **Helm**: Application packaging and deployment
- **GCP**: Production hosting

### Core Services
All environments deploy the same service topology:
- **Application**: Rust-based Collider server
- **PostgreSQL 16**: Event data storage with optimized configuration
- **Dragonfly**: Redis-compatible cache for sub-millisecond responses
- **Prometheus**: Metrics collection and monitoring
- **Grafana**: Visualization dashboards with pre-configured datasources
- **Jaeger**: Distributed tracing for performance monitoring

## 🚀 Quick Start

### Prerequisites

**Local Development:**
```bash
# Required tools
brew install terraform kubectl helm k3d docker
# or your package manager equivalent
```

**Production Deployment:**
```bash
# Additional for production
brew install google-cloud-sdk
gcloud auth login
gcloud config set project YOUR_PROJECT_ID
```

### Local Development (Recommended)

```bash
# Deploy complete local environment
just dev-setup

# Or step by step:
just deploy-local     # Deploy to local K3D cluster
just verify-local     # Verify everything is working
just cluster-status   # Check status
```

**Access your services:**
- Application: http://localhost:30080
- Grafana: http://localhost:30300 (admin/check terraform output)
- Prometheus: http://localhost:30090
- Jaeger: http://localhost:30686

### Production Deployment

```bash
# Deploy to production
just deploy-prod your-gcp-project-id

# Verify deployment
just verify-prod

# Get access information
just prod-info
```

## 📁 Directory Structure

```
infrastructure/
├── terraform/           # Main Terraform configuration
│   ├── main.tf         # Core infrastructure resources
│   ├── variables.tf    # Variable definitions
│   ├── helm-values.yaml # Template for Helm values
│   └── scripts/        # Deployment and utility scripts
│       ├── deploy.sh           # Main deployment script
│       ├── verify-deployment.sh # Verification script
│       └── install-k3s.sh      # Production K3S installer
├── justfile            # Infrastructure commands
└── README.md          # This file
```

## 🎯 Key Features

### Environment Parity
- **Identical service topology** across local and production
- **Same Helm charts** for consistent deployments
- **Automatic secret generation** and management
- **Environment-specific resource sizing**

### Security & Production-Ready
- **Secure secret generation** with Terraform random providers
- **Firewall rules** for production GCP deployment
- **Health checks** and liveness/readiness probes
- **Resource limits** and requests for all services

### Developer Experience
- **One-command deployment**: `just dev-setup`
- **Automatic cluster creation** with K3D for local development
- **Comprehensive verification** with health checks
- **Rich debugging commands** and log access

### Operational Excellence
- **Infrastructure as Code** with Terraform
- **Automated verification** after deployment
- **Resource monitoring** with built-in metrics
- **Easy troubleshooting** with dedicated commands

## 🔧 Configuration

### Environment Variables
The system uses sensible defaults with environment-specific overrides:

**Local Environment:**
- Optimized for development machines (reduced resource limits)
- Uses `local-path` storage class
- Enables NodePort for direct access
- Uses localhost registry

**Production Environment:**
- Production-ready resource allocation
- Uses `standard-rwo` storage class
- Configures external access via GCP Compute
- Integrates with GCP services

### Terraform Variables
Key variables in `terraform/variables.tf`:

```hcl,ignore
# Core
environment       = "local" | "prod"
gcp_project_id   = "your-project"     # Required for prod

# K3S Configuration
kubeconfig_path  = "~/.kube/config"   # Prod only
k3s_context      = "default"          # Prod only

# GCP Configuration (Prod only)
gcp_region       = "europe-west4"     # Netherlands
gcp_instance_type = "e2-standard-2"   # 2 vCPU, 8GB RAM
disk_size        = 50                 # GB
```

## 🛠️ Available Commands

### Deployment Commands
```bash
# Local development
just deploy-local         # Deploy local K3D cluster
just verify-local         # Verify local deployment
just destroy-local        # Clean up local environment

# Production
just deploy-prod PROJECT_ID    # Deploy to GCP
just verify-prod              # Verify production deployment
just prod-info               # Show access information

# Planning (dry-run)
just plan-local              # Show local changes
just plan-prod PROJECT_ID    # Show production changes
```

### Management Commands
```bash
# Cluster management
just cluster-info           # Basic cluster information
just cluster-status         # Detailed status
just cluster-resources      # Resource usage

# Application management
just restart-app           # Restart application pods
just scale-app 3           # Scale to 3 replicas
just logs                  # View application logs

# Monitoring access
just open-grafana          # Open Grafana dashboard
just open-prometheus       # Open Prometheus
just open-jaeger          # Open Jaeger UI
```

### Debugging Commands
```bash
# Troubleshooting
just troubleshoot          # Run diagnostic checks
just debug-pods           # Describe pod issues
just logs-app            # Application logs
just logs-postgres       # Database logs

# Port forwarding (for debugging)
just forward-app         # Forward app port
just forward-db          # Forward database port
```

## 🔍 Verification & Health Checks

The deployment includes comprehensive verification:

### Automated Checks
- **Cluster connectivity** and kubectl configuration
- **Namespace and resource creation**
- **Pod readiness** and health status
- **Service accessibility** and endpoints
- **Persistent volume** binding
- **Application health endpoints**
- **Log analysis** for errors

### Health Endpoints
- Application: `/health`
- Prometheus: `/-/healthy`
- Grafana: `/api/health`

### Manual Verification
```bash
# Quick status check
just status

# Comprehensive verification
just verify-local  # or verify-prod

# Application health
curl http://localhost:30080/health
```

## 📊 Monitoring & Observability

### Built-in Monitoring Stack
- **Prometheus**: Metrics collection with 30-day retention
- **Grafana**: Pre-configured dashboards and datasources
- **Jaeger**: Distributed tracing for performance analysis

### Access Information
**Local Development:**
- Grafana: http://localhost:30300 (admin/generated-password)
- Prometheus: http://localhost:30090
- Jaeger: http://localhost:30686

**Production:**
Get external IP with `just prod-info` and access via NodePort.

## 🚨 Troubleshooting

### Common Issues

**K3D cluster not starting:**
```bash
# Check Docker is running
docker info

# Recreate cluster
just destroy-local
just deploy-local
```

**Pods not ready:**
```bash
# Check pod status
kubectl get pods -n collider

# Check events
kubectl get events -n collider --sort-by='.lastTimestamp'

# Check logs
just logs-app
```

**Health checks failing:**
```bash
# Run comprehensive diagnostics
just troubleshoot

# Check specific service logs
just logs-postgres    # Database issues
just logs-dragonfly   # Cache issues
```

**Production access issues:**
```bash
# Verify GCP configuration
gcloud config list
gcloud auth list

# Check firewall rules
gcloud compute firewall-rules list | grep collider
```

### Support Commands
```bash
# Reset everything (local only)
just reset

# Generate detailed diagnostics
just verify-local    # Creates report in /tmp/

# Access Terraform state
cd infrastructure/terraform
terraform state list
terraform output
```

## 🏗️ Architecture Details

### Local Environment (K3D)
- **K3D cluster**: `collider-local` with integrated registry
- **Local registry**: `localhost:5001` for image storage
- **Nginx Ingress**: Automatic HTTP routing
- **Local storage**: `local-path` provisioner
- **Direct access**: NodePort services on localhost

### Production Environment (GCP + K3S)
- **Compute Instance**: `e2-standard-2` with Ubuntu 22.04
- **K3S installation**: Single-node cluster with optimizations
- **Persistent disks**: SSD storage for data
- **Firewall rules**: Secure access to K3S API and NodePorts
- **External access**: Public IP with NodePort services

### Security Considerations
- **Firewall rules**: Restrict access to necessary ports only
- **Secret management**: Terraform-generated secrets
- **Network policies**: Isolated namespace communication
- **Resource limits**: Prevent resource exhaustion

## 🔄 Migration from Previous Setup

This infrastructure replaces the previous docker-compose and multi-environment Terraform setup with:

### Benefits
- **Unified approach**: Same technology stack for local and production
- **Simplified environments**: Only local and prod (no dev/staging complexity)
- **Better resource utilization**: Kubernetes scheduling and management
- **Production parity**: Identical service topology everywhere
- **Improved debugging**: Rich Kubernetes tooling and commands

### Migration Steps
1. **Backup data**: Export any important data from existing deployments
2. **Deploy new infrastructure**: `just dev-setup`
3. **Verify functionality**: `just verify-local`
4. **Update CI/CD**: Point to new deployment commands
5. **Clean up old resources**: Remove docker-compose and old Terraform

## 📚 Additional Resources

- **Helm Charts**: See `../charts/collider/` for Kubernetes manifests
- **Production Best Practices**: Based on `../article.md` recommendations
- **K3S Documentation**: https://k3s.io/
- **Terraform Providers**: Uses official Google, Kubernetes, and Helm providers

---

**Need help?** Run `just troubleshoot` for automated diagnostics or check the troubleshooting section above.