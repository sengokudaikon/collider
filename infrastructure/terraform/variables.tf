variable "environment" {
  description = "Environment name (local or prod)"
  type        = string
  validation {
    condition = contains(["local", "prod"], var.environment)
    error_message = "Environment must be local or prod"
  }
}

variable "kubeconfig_path" {
  description = "Path to kubeconfig file (prod only)"
  type        = string
  default     = "~/.kube/config"
}

variable "k3s_context" {
  description = "Kubernetes context name (prod only)"
  type        = string
  default     = "default"
}

variable "gcp_project_id" {
  description = "The GCP project ID (required for prod deployments)"
  type        = string
  default     = ""
}

variable "gcp_region" {
  description = "The GCP region for resources"
  type        = string
  default     = "europe-west4"
}

variable "gcp_zone" {
  description = "The GCP zone for zonal resources"
  type        = string
  default     = "europe-west4-a"
}

variable "gcp_instance_type" {
  description = "GCP instance type for K3S node"
  type        = string
  default     = "e2-standard-2"
}

variable "disk_size" {
  description = "Boot disk size in GB"
  type        = number
  default     = 50
}

variable "app_image_tag" {
  description = "Application image tag"
  type        = string
  default     = "latest"
}

locals {
  env_config = {
    local = {
      node_resources = {
        cpu_limit      = "2"
        memory_limit   = "4Gi"
        cpu_request    = "1"
        memory_request = "2Gi"
      }
      postgres_resources = {
        cpu_limit    = "1"
        memory_limit = "2Gi"
        storage_size = "10Gi"
      }
      dragonfly_resources = {
        cpu_limit    = "1"
        memory_limit = "1Gi"
        max_memory   = "512mb"
      }
      monitoring_resources = {
        prometheus_memory = "1Gi"
        grafana_memory    = "512Mi"
        jaeger_memory     = "512Mi"
      }
    }
    prod = {
      node_resources = {
        cpu_limit      = "4"
        memory_limit   = "8Gi"
        cpu_request    = "2"
        memory_request = "4Gi"
      }
      postgres_resources = {
        cpu_limit    = "2"
        memory_limit = "4Gi"
        storage_size = "100Gi"
      }
      dragonfly_resources = {
        cpu_limit    = "2"
        memory_limit = "4Gi"
        max_memory   = "3gb"
      }
      monitoring_resources = {
        prometheus_memory = "2Gi"
        grafana_memory    = "1Gi"
        jaeger_memory     = "1Gi"
      }
    }
  }

  current_config = local.env_config[var.environment]
}

resource "null_resource" "prod_validation" {
  count = var.environment == "prod" && var.gcp_project_id == "" ? 1 : 0

  provisioner "local-exec" {
    command = "echo 'ERROR: gcp_project_id is required for prod deployments' && exit 1"
  }
}

output "environment_info" {
  description = "Environment configuration summary"
  value = {
    environment = var.environment
    is_local    = var.environment == "local"
    is_prod     = var.environment == "prod"

    k3s_setup = var.environment == "local" ? "k3d cluster (auto-created)" : "existing K3S cluster"

    resources = local.current_config

    validation = var.environment == "prod" ? {
      gcp_project_id_set = var.gcp_project_id != ""
      required_tools = ["kubectl", "helm", "gcloud"]
    } : {
      gcp_project_id_set = false
      required_tools = ["kubectl", "helm", "k3d"]
    }
  }
}