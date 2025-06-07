FROM rust:1.87-alpine as builder

# Install build dependencies
RUN apk add --no-cache \
    pkgconfig \
    openssl-dev \
    postgresql-dev \
    musl-dev

WORKDIR /app

# Copy Cargo manifests
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml ./server/
COPY domains/ ./domains/
COPY libs/ ./libs/

# Copy source code
COPY server/src ./server/src
COPY domains/ ./domains/
COPY libs/ ./libs/

# Build with optimizations and cache mounts
ENV RUSTFLAGS="-C target-cpu=native -C opt-level=3"
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
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