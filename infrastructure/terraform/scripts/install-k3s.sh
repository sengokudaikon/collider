#!/bin/bash
# K3S Installation Script for GCP Production Instance
# This script installs and configures K3S on Ubuntu 22.04

set -euo pipefail

# Configuration
K3S_TOKEN="${node_token}"
K3S_VERSION="v1.28.5+k3s1"  # Stable version
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/rancher/k3s"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "$${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1$${NC}"
}

warn() {
    echo -e "$${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1$${NC}"
}

error() {
    echo -e "$${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1$${NC}"
    exit 1
}

# Update system
log "Updating system packages..."
apt-get update -y
apt-get upgrade -y

# Install required packages
log "Installing required packages..."
apt-get install -y \
    curl \
    wget \
    ca-certificates \
    gnupg \
    lsb-release \
    apt-transport-https \
    software-properties-common \
    unzip \
    htop \
    iotop \
    net-tools

# Install Docker (for image building if needed)
log "Installing Docker..."
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
apt-get update -y
apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Configure Docker
usermod -aG docker ubuntu
systemctl enable docker
systemctl start docker

# Create K3S configuration directory
log "Creating K3S configuration directory..."
mkdir -p $CONFIG_DIR

# Create K3S configuration file
log "Creating K3S configuration..."
cat > $CONFIG_DIR/config.yaml <<EOF
# K3S Configuration for Collider Production
cluster-init: true
node-token: "$K3S_TOKEN"

# Networking
cluster-cidr: "10.42.0.0/16"
service-cidr: "10.43.0.0/16"
cluster-dns: "10.43.0.10"

# Disable default components we don't need
disable:
  - traefik  # We'll use nginx-ingress instead
  - servicelb

# Enable useful components
feature-gates:
  - "MixedProtocolLBService=true"

# Node configuration
node-name: "collider-k3s-prod"
node-label:
  - "environment=prod"
  - "app=collider"

# Security
protect-kernel-defaults: true
secrets-encryption: true

# Performance tuning
kube-controller-manager-arg:
  - "node-cidr-mask-size=24"
  - "allocate-node-cidrs=true"

kube-apiserver-arg:
  - "audit-log-maxage=30"
  - "audit-log-maxbackup=3"
  - "audit-log-maxsize=100"
  - "audit-log-path=/var/log/k3s-audit.log"
  - "enable-admission-plugins=NodeRestriction,NamespaceLifecycle,ServiceAccount"

kubelet-arg:
  - "max-pods=250"
  - "node-status-update-frequency=10s"
EOF

# Install K3S
log "Installing K3S version $K3S_VERSION..."
curl -sfL https://get.k3s.io | INSTALL_K3S_VERSION="$K3S_VERSION" INSTALL_K3S_EXEC="server" sh -

# Wait for K3S to be ready
log "Waiting for K3S to be ready..."
timeout=300
counter=0
while ! kubectl get nodes > /dev/null 2>&1; do
    if [ $counter -gt $timeout ]; then
        error "K3S failed to start within $timeout seconds"
    fi
    sleep 5
    counter=$((counter + 5))
    log "Waiting for K3S... ($counter/$timeout seconds)"
done

log "K3S is ready!"

# Install Helm
log "Installing Helm..."
curl https://baltocdn.com/helm/signing.asc | gpg --dearmor | tee /usr/share/keyrings/helm.gpg > /dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/helm.gpg] https://baltocdn.com/helm/stable/debian/ all main" | tee /etc/apt/sources.list.d/helm-stable-debian.list
apt-get update -y
apt-get install -y helm

# Install nginx-ingress controller
log "Installing nginx-ingress controller..."
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-v1.8.2/deploy/static/provider/cloud/deploy.yaml

# Wait for ingress controller to be ready
log "Waiting for nginx-ingress controller..."
kubectl wait --namespace ingress-nginx \
    --for=condition=ready pod \
    --selector=app.kubernetes.io/component=controller \
    --timeout=300s

# Create system optimization
log "Optimizing system for K3S workloads..."
cat > /etc/sysctl.d/99-k3s.conf <<EOF
# K3S optimizations
vm.max_map_count = 262144
fs.inotify.max_user_watches = 524288
fs.inotify.max_user_instances = 512
net.core.somaxconn = 32768
net.ipv4.ip_local_port_range = 1024 65535
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
net.core.rmem_default = 262144
net.core.rmem_max = 16777216
net.core.wmem_default = 262144
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 65536 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216
EOF

sysctl -p /etc/sysctl.d/99-k3s.conf

# Set up log rotation for K3S
log "Setting up log rotation..."
cat > /etc/logrotate.d/k3s <<EOF
/var/log/k3s*.log {
    daily
    rotate 7
    compress
    delaycompress
    notifempty
    create 0644 root root
    postrotate
        systemctl reload k3s
    endscript
}
EOF

# Create monitoring directory
mkdir -p /var/lib/collider/monitoring

# Set up firewall (basic security)
log "Configuring firewall..."
ufw --force enable
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 6443  # K3S API
ufw allow 80    # HTTP
ufw allow 443   # HTTPS
ufw allow 30000:32767  # NodePort range

# Create kubeconfig for external access
log "Setting up kubeconfig for external access..."
cp /etc/rancher/k3s/k3s.yaml /tmp/kubeconfig
chmod 644 /tmp/kubeconfig

# Get public IP for kubeconfig
PUBLIC_IP=$(curl -s http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip -H "Metadata-Flavor: Google")
sed -i "s/127.0.0.1/$PUBLIC_IP/g" /tmp/kubeconfig

log "Kubeconfig for external access available at /tmp/kubeconfig"

# Create status script
log "Creating status monitoring script..."
cat > /usr/local/bin/k3s-status <<'EOF'
#!/bin/bash
echo "=== K3S Cluster Status ==="
echo "Node Status:"
kubectl get nodes -o wide
echo ""
echo "System Resources:"
kubectl top nodes 2>/dev/null || echo "Metrics server not available"
echo ""
echo "Running Pods:"
kubectl get pods --all-namespaces
echo ""
echo "Services:"
kubectl get services --all-namespaces
echo ""
echo "Ingress:"
kubectl get ingress --all-namespaces
EOF

chmod +x /usr/local/bin/k3s-status

# Final status check
log "Running final status check..."
kubectl get nodes
kubectl get pods --all-namespaces

log "K3S installation completed successfully!"
log "Cluster is ready for Helm deployments"
log "Use 'k3s-status' command to check cluster status"
log "Kubeconfig for external access: /tmp/kubeconfig"

# Create installation marker
echo "K3S installation completed at $(date)" > /var/lib/collider/k3s-install-complete