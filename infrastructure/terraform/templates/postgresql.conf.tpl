# PostgreSQL Configuration Template for Collider
# Environment: ${environment}

# Connection settings
listen_addresses = '0.0.0.0'
max_connections = ${max_connections}
superuser_reserved_connections = 5

# Memory settings
shared_buffers = ${shared_buffers}
%{ if environment == "prod" ~}
effective_cache_size = 6GB
maintenance_work_mem = 512MB
work_mem = 4MB
wal_buffers = 16MB
%{ else ~}
effective_cache_size = 1GB
maintenance_work_mem = 128MB
work_mem = 1MB
wal_buffers = 4MB
%{ endif ~}

# Checkpoint settings
%{ if environment == "prod" ~}
checkpoint_timeout = 15min
checkpoint_completion_target = 0.9
max_wal_size = 4GB
min_wal_size = 1GB
%{ else ~}
checkpoint_timeout = 5min
checkpoint_completion_target = 0.7
max_wal_size = 1GB
min_wal_size = 256MB
%{ endif ~}

# Query optimization
random_page_cost = 1.1
effective_io_concurrency = 200
default_statistics_target = 100

# Parallel queries
%{ if environment == "prod" ~}
max_worker_processes = 8
max_parallel_workers_per_gather = 4
max_parallel_workers = 8
max_parallel_maintenance_workers = 4
%{ else ~}
max_worker_processes = 4
max_parallel_workers_per_gather = 2
max_parallel_workers = 4
max_parallel_maintenance_workers = 2
%{ endif ~}

# Performance optimizations
%{ if environment == "prod" ~}
synchronous_commit = off
full_page_writes = off
%{ else ~}
synchronous_commit = on
full_page_writes = on
%{ endif ~}
fsync = on
wal_compression = on

# Logging
%{ if environment == "prod" ~}
log_min_duration_statement = 1000
log_connections = off
log_disconnections = off
%{ else ~}
log_min_duration_statement = 100
log_connections = on
log_disconnections = on
%{ endif ~}
log_checkpoints = on
log_lock_waits = on
log_temp_files = 0

# Shared preload libraries
shared_preload_libraries = 'pg_stat_statements'

# Locale
timezone = 'UTC'
lc_messages = 'en_US.utf8'
lc_monetary = 'en_US.utf8'
lc_numeric = 'en_US.utf8'
lc_time = 'en_US.utf8'
default_text_search_config = 'pg_catalog.english'