FROM rust:1.87-alpine as builder

# Install build dependencies including static SSL libraries for sccache
RUN apk add --no-cache \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    postgresql-dev \
    musl-dev \
    && cargo install cargo-watch sccache

WORKDIR /app

# Copy Cargo manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml ./server/
COPY domains/user/models/Cargo.toml ./domains/user/models/
COPY domains/user/dao/Cargo.toml ./domains/user/dao/
COPY domains/user/commands/Cargo.toml ./domains/user/commands/
COPY domains/user/queries/Cargo.toml ./domains/user/queries/
COPY domains/user/http/Cargo.toml ./domains/user/http/
COPY domains/events/models/Cargo.toml ./domains/events/models/
COPY domains/events/dao/Cargo.toml ./domains/events/dao/
COPY domains/events/commands/Cargo.toml ./domains/events/commands/
COPY domains/events/queries/Cargo.toml ./domains/events/queries/
COPY domains/events/http/Cargo.toml ./domains/events/http/
COPY domains/analytics/Cargo.toml ./domains/analytics/
COPY domains/analytics/http/Cargo.toml ./domains/analytics/http/
COPY libs/persistence/database_traits/Cargo.toml ./libs/persistence/database_traits/
COPY libs/persistence/sql_connection/Cargo.toml ./libs/persistence/sql_connection/
COPY libs/persistence/redis_connection/Cargo.toml ./libs/persistence/redis_connection/
COPY libs/domain/Cargo.toml ./libs/domain/
COPY libs/test-utils/Cargo.toml ./libs/test-utils/
COPY libs/seeders/Cargo.toml ./libs/seeders/
COPY binaries/migrator/Cargo.toml ./binaries/migrator/
COPY binaries/seeder/Cargo.toml ./binaries/seeder/

# Create dummy source files for dependency caching
RUN mkdir -p server/src && echo "fn main() {}" > server/src/main.rs && \
    mkdir -p domains/user/models/src && echo "" > domains/user/models/src/lib.rs && \
    mkdir -p domains/user/dao/src && echo "" > domains/user/dao/src/lib.rs && \
    mkdir -p domains/user/commands/src && echo "" > domains/user/commands/src/lib.rs && \
    mkdir -p domains/user/queries/src && echo "" > domains/user/queries/src/lib.rs && \
    mkdir -p domains/user/http/src && echo "" > domains/user/http/src/lib.rs && \
    mkdir -p domains/events/models/src && echo "" > domains/events/models/src/lib.rs && \
    mkdir -p domains/events/dao/src && echo "" > domains/events/dao/src/lib.rs && \
    mkdir -p domains/events/commands/src && echo "" > domains/events/commands/src/lib.rs && \
    mkdir -p domains/events/queries/src && echo "" > domains/events/queries/src/lib.rs && \
    mkdir -p domains/events/http/src && echo "" > domains/events/http/src/lib.rs && \
    mkdir -p domains/analytics/src && echo "" > domains/analytics/src/lib.rs && \
    mkdir -p domains/analytics/http/src && echo "" > domains/analytics/http/src/lib.rs && \
    mkdir -p libs/persistence/database_traits/src && echo "" > libs/persistence/database_traits/src/lib.rs && \
    mkdir -p libs/persistence/sql_connection/src && echo "" > libs/persistence/sql_connection/src/lib.rs && \
    mkdir -p libs/persistence/redis_connection/src && echo "" > libs/persistence/redis_connection/src/lib.rs && \
    mkdir -p libs/domain/src && echo "" > libs/domain/src/lib.rs && \
    mkdir -p libs/test-utils/src && echo "" > libs/test-utils/src/lib.rs && \
    mkdir -p libs/seeders/src && echo "" > libs/seeders/src/lib.rs && \
    mkdir -p binaries/migrator/src && echo "fn main() {}" > binaries/migrator/src/main.rs && \
    mkdir -p binaries/seeder/src && echo "fn main() {}" > binaries/seeder/src/main.rs

# Configure sccache and build environment
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/app/.sccache
ENV RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C link-arg=-s"

# Build dependencies first (this layer will be cached)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/app/.sccache \
    cargo build --release --workspace -j $(nproc)

# Copy actual source code
COPY server/src ./server/src
COPY domains/ ./domains/
COPY libs/ ./libs/
COPY binaries/ ./binaries/

# Build final binary with optimizations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/app/.sccache \
    cargo build --release --bin collider && \
    cp /app/target/release/collider /usr/local/bin/collider

# Runtime stage
FROM alpine:3.21

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libssl3 \
    libpq \
    curl

# Create non-root user
RUN addgroup -S collider && adduser -S -G collider collider

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/local/bin/collider /usr/local/bin/collider

# Set ownership
RUN chown -R collider:collider /app

# Switch to non-root user
USER collider

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

EXPOSE 8080

CMD ["collider"]