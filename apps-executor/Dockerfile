# Build stage
FROM rust:1.82 AS builder

WORKDIR /app

# Install dependencies for protobuf
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY proto-gen/ ./proto-gen/
COPY apps-executor/ ./apps-executor/
COPY apps-api/ ./apps-api/
COPY proto/ ./proto/

# Build the apps-executor binary
RUN cargo build --release -p apps-executor

# Runtime stage
FROM debian:bookworm-slim

# Install docker CLI (needed to run containers)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    gnupg \
    lsb-release \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null \
    && apt-get update && apt-get install -y docker-ce-cli \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/apps-executor /app/apps-executor

ENV EXECUTOR_ADDR=0.0.0.0:50051
ENV RUNNER_IMAGE=powershell-runner:dev
ENV RUST_LOG=info,apps_executor=debug,tonic=info

EXPOSE 50051

ENTRYPOINT ["/app/apps-executor"]