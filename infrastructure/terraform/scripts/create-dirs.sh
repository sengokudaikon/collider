#!/bin/bash
# Create necessary data directories for Collider infrastructure

set -e

echo "Creating data directories for environment: ${environment}"

# Create PostgreSQL data directory
if [[ "${postgres_data_path}" == /mnt/* ]]; then
    # Production path - ensure it exists and has correct permissions
    sudo mkdir -p "${postgres_data_path}"
    sudo chown -R 999:999 "${postgres_data_path}" 2>/dev/null || true
else
    # Development path - create locally
    mkdir -p "${postgres_data_path}"
fi

# Create Dragonfly data directory
if [[ "${dragonfly_data_path}" == /mnt/* ]]; then
    # Production path
    sudo mkdir -p "${dragonfly_data_path}"
    sudo chown -R 1000:1000 "${dragonfly_data_path}" 2>/dev/null || true
else
    # Development path
    mkdir -p "${dragonfly_data_path}"
fi

# Create backup directory
if [[ "${environment}" == "prod" ]]; then
    sudo mkdir -p /home/deploy/backups
    sudo chown -R deploy:deploy /home/deploy/backups 2>/dev/null || true
else
    mkdir -p ./backups
fi

# Create log directory
if [[ "${environment}" == "prod" ]]; then
    sudo mkdir -p /var/log/collider
    sudo chown -R deploy:deploy /var/log/collider 2>/dev/null || true
else
    mkdir -p ./logs
fi

echo "✅ Data directories created successfully"