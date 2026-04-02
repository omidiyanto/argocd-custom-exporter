# syntax=docker/dockerfile:1

# Build stage
FROM rust:latest AS builder

WORKDIR /build

# Cache dependencies: copy manifest and generate Cargo.lock automatically
COPY Cargo.toml ./
RUN cargo generate-lockfile

# Create stub source files to build and cache external dependencies
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
