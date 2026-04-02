# syntax=docker/dockerfile:1

# Build stage
FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Cache dependencies: copy manifests and create stub source files
COPY Cargo.toml ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/collector.rs \
    && echo "" > src/metrics.rs \
    && cargo build --release \
    && rm -rf src

# Build actual source
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage — distroless for minimal attack surface (~15MB)
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /build/target/release/argocd-custom-exporter /argocd-custom-exporter

EXPOSE 9184

ENTRYPOINT ["/argocd-custom-exporter"]
