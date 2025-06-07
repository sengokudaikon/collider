terraform {
  required_version = ">= 1.5"
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.12"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.24"
    }
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    null = {
      source  = "hashicorp/null"
      version = "~> 3.2"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.4"
    }
  }
}

# Local variables
locals {
  is_local = var.environment == "local"
  is_prod  = var.environment == "prod"
  
  # K3S cluster configuration
  k3s_config = local.is_local ? {
    kubeconfig_path = "~/.kube/config"
    context         = "k3d-collider-local"
  } : {
    kubeconfig_path = var.kubeconfig_path
    context         = var.k3s_context
  }
  
  # Helm values based on environment
  helm_values = local.is_local ? {
    environment     = "local"
    image_registry  = "localhost:5000"
    storage_class   = "local-path"
    node_port       = true
    resource_limits = "development"
  } : {
    environment     = "prod"
    image_registry  = "gcr.io/${var.gcp_project_id}"
    storage_class   = "standard-rwo"
    node_port       = false
    resource_limits = "production"
  }
}

# =============================================================================
# PROVIDERS
# =============================================================================

provider "google" {
  project = local.is_prod ? var.gcp_project_id : null
  region  = var.gcp_region
  zone    = var.gcp_zone
}

provider "kubernetes" {
  # Use default kubeconfig location and current context
}

provider "helm" {
  kubernetes {
    # Use default kubeconfig location and current context
  }
}

# =============================================================================
# K3S CLUSTER SETUP
# =============================================================================

# Local: Setup K3D cluster
resource "null_resource" "k3d_cluster" {
  count = local.is_local ? 1 : 0
  
  provisioner "local-exec" {
    command = <<-EOF
      # Clean up any existing cluster
      k3d cluster delete collider-local 2>/dev/null || true
      
      # Create K3D cluster with registry
      k3d cluster create collider-local \
        --api-port 6550 \
        --port "8080:80@loadbalancer" \
        --port "8443:443@loadbalancer" \
        --registry-create collider-registry:5000 \
        --k3s-arg "--disable=traefik@server:0" \
        --wait
      
      # Wait a bit for cluster to be fully ready
      sleep 10
      
      # Install nginx ingress controller
      kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-v1.8.2/deploy/static/provider/cloud/deploy.yaml
      
      # Wait for ingress controller (with retries)
      for i in {1..30}; do
        if kubectl wait --namespace ingress-nginx \
          --for=condition=ready pod \
          --selector=app.kubernetes.io/component=controller \
          --timeout=30s 2>/dev/null; then
          echo "Ingress controller is ready"
          break
        fi
        echo "Waiting for ingress controller... ($i/30)"
        sleep 10
      done
    EOF
  }
  
  provisioner "local-exec" {
    when    = destroy
    command = "k3d cluster delete collider-local || true"
  }
}

# Production: GCP Compute Instance for K3S
resource "google_compute_instance" "k3s_node" {
  count        = local.is_prod ? 1 : 0
  name         = "collider-k3s-${var.environment}"
  machine_type = var.gcp_instance_type
  zone         = var.gcp_zone
  
  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2204-lts"
      size  = var.disk_size
      type  = "pd-ssd"
    }
  }
  
  network_interface {
    network = "default"
    access_config {
      // Ephemeral public IP
    }
  }
  
  metadata_startup_script = templatefile("${path.module}/scripts/install-k3s.sh", {
    node_token = random_password.k3s_token[0].result
  })
  
  tags = ["collider-k3s", "http-server", "https-server"]
  
  labels = {
    environment = var.environment
    app         = "collider"
  }
}

# K3S node token for clustering
resource "random_password" "k3s_token" {
  count   = local.is_prod ? 1 : 0
  length  = 64
  special = false
}

# Firewall rules for K3S
resource "google_compute_firewall" "k3s_api" {
  count   = local.is_prod ? 1 : 0
  name    = "collider-k3s-api"
  network = "default"
  
  allow {
    protocol = "tcp"
    ports    = ["6443"]
  }
  
  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["collider-k3s"]
}

resource "google_compute_firewall" "k3s_nodeport" {
  count   = local.is_prod ? 1 : 0
  name    = "collider-k3s-nodeport"
  network = "default"
  
  allow {
    protocol = "tcp"
    ports    = ["30000-32767"]
  }
  
  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["collider-k3s"]
}

resource "google_compute_firewall" "k3s_http" {
  count   = local.is_prod ? 1 : 0
  name    = "collider-k3s-http"
  network = "default"
  
  allow {
    protocol = "tcp"
    ports    = ["80", "443"]
  }
  
  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["collider-k3s"]
}

# =============================================================================
# KUBERNETES NAMESPACE
# =============================================================================

resource "kubernetes_namespace" "collider" {
  metadata {
    name = "collider"
    labels = {
      environment = var.environment
      app         = "collider"
    }
  }
  
  depends_on = [null_resource.k3d_cluster]
}

# =============================================================================
# CONFIGURATION SECRETS
# =============================================================================

# Generate secure passwords
resource "random_password" "postgres_password" {
  length  = 32
  special = true
}

resource "random_password" "dragonfly_password" {
  length  = 32
  special = false
}

resource "random_password" "jwt_secret" {
  length  = 64
  special = true
}

resource "random_password" "grafana_password" {
  length  = 16
  special = false
}

# =============================================================================
# HELM DEPLOYMENT
# =============================================================================

resource "helm_release" "collider" {
  name       = "collider"
  repository = "file://${path.module}/../../charts"
  chart      = "collider"
  namespace  = kubernetes_namespace.collider.metadata[0].name
  
  # Environment-specific values
  values = [templatefile("${path.module}/helm-values.yaml", {
    environment     = var.environment
    image_registry  = local.helm_values.image_registry
    storage_class   = local.helm_values.storage_class
    node_port       = local.helm_values.node_port
    resource_limits = local.helm_values.resource_limits
    
    # Secrets
    postgres_password  = random_password.postgres_password.result
    dragonfly_password = random_password.dragonfly_password.result
    jwt_secret         = random_password.jwt_secret.result
    grafana_password   = random_password.grafana_password.result
    
    # Production specific
    external_ip = local.is_prod ? google_compute_instance.k3s_node[0].network_interface[0].access_config[0].nat_ip : "localhost"
  })]
  
  depends_on = [
    kubernetes_namespace.collider,
    null_resource.k3d_cluster
  ]
}

# =============================================================================
# OUTPUTS
# =============================================================================

output "environment" {
  description = "Deployment environment"
  value       = var.environment
}

output "deployment_type" {
  description = "Type of deployment (local k3s or prod k3s)"
  value       = local.is_local ? "local-k3s" : "prod-k3s"
}

output "cluster_info" {
  description = "K3S cluster information"
  value = {
    kubeconfig_path = local.k3s_config.kubeconfig_path
    context         = local.k3s_config.context
    namespace       = kubernetes_namespace.collider.metadata[0].name
  }
}

output "endpoints" {
  description = "Service endpoints"
  value = {
    external_ip = local.is_prod ? google_compute_instance.k3s_node[0].network_interface[0].access_config[0].nat_ip : "localhost"
    application = local.is_local ? "http://localhost:30080" : "http://${google_compute_instance.k3s_node[0].network_interface[0].access_config[0].nat_ip}:30080"
    prometheus  = local.is_local ? "http://localhost:30090" : "http://${google_compute_instance.k3s_node[0].network_interface[0].access_config[0].nat_ip}:30090"
    grafana     = local.is_local ? "http://localhost:30300" : "http://${google_compute_instance.k3s_node[0].network_interface[0].access_config[0].nat_ip}:30300"
    jaeger      = local.is_local ? "http://localhost:30686" : "http://${google_compute_instance.k3s_node[0].network_interface[0].access_config[0].nat_ip}:30686"
  }
}

output "secrets" {
  description = "Generated secrets (sensitive)"
  value = {
    postgres_password  = random_password.postgres_password.result
    dragonfly_password = random_password.dragonfly_password.result
    jwt_secret         = random_password.jwt_secret.result
    grafana_password   = random_password.grafana_password.result
  }
  sensitive = true
}