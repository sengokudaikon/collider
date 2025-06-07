FROM rust:1.87-alpine as builder

WORKDIR /app
COPY Cargo.toml .
COPY *.rs ./

# Install dependencies for compilation
RUN apk add --no-cache \
    pkgconfig \
    openssl-dev \
    musl-dev

# Build release binary
RUN cargo build --release

FROM alpine:3.21

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    curl

WORKDIR /benchmarks

# Copy binaries
COPY --from=builder /app/target/release/goose_load_test /usr/local/bin/
COPY --from=builder /app/target/release/criterion_bench /usr/local/bin/

# Copy source files for reference
COPY . .

CMD ["goose_load_test"]