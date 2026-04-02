# argocd-custom-exporter

Lightweight Prometheus exporter for detecting **ArgoCD autosync policy drift** between Git-defined state and actual Application state.

## Problem

ArgoCD `ApplicationSet` can use `ignoreApplicationDifferences` to allow admins to toggle autosync via UI without being overwritten. However, there's **no native warning** when the actual state drifts from Git.

## Solution

This exporter watches ArgoCD Application CRs via K8s Watch API (event-driven, not polling) and compares:
- **Git expected state**: `argocd-exporter/git-autosync` annotation (set by ApplicationSet template)
- **Actual state**: `spec.syncPolicy.automated` field

### Metrics

```prometheus
# Per-application drift (1=drift, 0=healthy)
argocd_autosync_drift{app="lumina-docs-dev-asus",environment="dev",tenant="asus",git_autosync="true",actual_autosync="false"} 1

# Aggregates
argocd_autosync_drift_total 1
argocd_autosync_tracked_total 5
argocd_exporter_up 1
```

## Architecture

- **Event-driven** via K8s Watch API (`kube::runtime::reflector`)
- Initial LIST is **paginated** (500/page) — safe for 100k+ applications
- **Stateless** — all state in memory, restarts cleanly
- **~15MB** Docker image (distroless)

## Configuration

| Env Var | Default | Description |
|---|---|---|
| `EXPORTER_PORT` | `9184` | HTTP server port |
| `EXPORTER_NAMESPACE` | `argocd` | Namespace to watch Applications |
| `RUST_LOG` | `info` | Log level (debug, info, warn, error) |

## Prerequisites

Your ApplicationSet templates must include the annotation:

```yaml
template:
  metadata:
    annotations:
      argocd-exporter/git-autosync: '{{ .autosync }}'
```

And `ignoreApplicationDifferences` for syncPolicy:

```yaml
spec:
  ignoreApplicationDifferences:
    - jsonPointers:
        - /spec/syncPolicy
```

## Build

```bash
cargo build --release
```

## Docker

```bash
docker build -t argocd-custom-exporter:latest .
```

## Deploy

```bash
kubectl apply -f deploy/rbac.yaml
kubectl apply -f deploy/deployment.yaml
kubectl apply -f deploy/service.yaml
kubectl apply -f deploy/servicemonitor.yaml  # if using Prometheus Operator
```

## Test

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
