# Build stage
FROM rust:1.85 AS builder

WORKDIR /app

# Install dependencies for protobuf
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY proto-gen/ ./proto-gen/
COPY apps-executor/ ./apps-executor/
COPY apps-api/ ./apps-api/
COPY proto/ ./proto/

# Build the apps-api binary
RUN cargo build --release -p apps-api

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/apps-api /app/apps-api

ENV API_ADDR=0.0.0.0:3000
ENV EXECUTOR_ADDR=http://powershell-executor:50051
ENV RUST_LOG=info,apps_api=debug,axum=info,tower_http=info

EXPOSE 3000

ENTRYPOINT ["/app/apps-api"]