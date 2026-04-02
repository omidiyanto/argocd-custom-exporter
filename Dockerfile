# syntax=docker/dockerfile:1

# Build stage
FROM rust:1-bookworm AS builder

WORKDIR /build

# Cache dependencies: copy manifest and create stub source files to build
COPY Cargo.toml ./

# Create stub source files to build and cache external dependencies
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/collector.rs \
    && echo "" > src/metrics.rs \
    && cargo generate-lockfile \
    && cargo build --release \
    && rm -rf src

# Build actual source
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage (Mencegah masalah GLIBC mismatched, kita pakai Debian Base yang terjamin sama dengan buildernya)
FROM debian:bookworm-slim

# Install ca-certificates just in case API server TLS requires external ca verification, though in-cluster usually doesn't, it is good practice
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN useradd -m -s /bin/bash appuser

COPY --from=builder /build/target/release/argocd-custom-exporter /usr/local/bin/argocd-custom-exporter

USER appuser
EXPOSE 9184

ENTRYPOINT ["argocd-custom-exporter"]
