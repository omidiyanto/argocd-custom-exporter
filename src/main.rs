use std::net::SocketAddr;

use axum::{extract::State, http::header, response::IntoResponse, routing::get, Router};
use futures::StreamExt;
use kube::{
    api::{ApiResource, DynamicObject, GroupVersionKind},
    runtime::{reflector, watcher, WatchStreamExt},
    Api, Client,
};
use tracing::{error, info, warn};

mod collector;
mod metrics;

/// Shared application state — the reflector store is an Arc internally.
#[derive(Clone)]
struct AppState {
    store: reflector::Store<DynamicObject>,
}

/// Configuration parsed from environment variables.
struct Config {
    port: u16,
    namespace: String,
}

impl Config {
    fn from_env() -> Self {
        Self {
            port: std::env::var("EXPORTER_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9184),
            namespace: std::env::var("EXPORTER_NAMESPACE").unwrap_or_else(|_| "argocd".to_string()),
        }
    }
}

async fn handle_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let body = metrics::render(&state.store);
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

async fn handle_health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    let config = Config::from_env();
    info!(
        namespace = %config.namespace,
        port = config.port,
        "Starting argocd-custom-exporter"
    );

    let client = Client::try_default().await?;
    info!("Connected to Kubernetes API server");

    // Define ArgoCD Application CRD api resource
    let gvk = GroupVersionKind::gvk("argoproj.io", "v1alpha1", "Application");
    let api_resource = ApiResource::from_gvk(&gvk);
    let apps: Api<DynamicObject> = Api::namespaced_with(client, &config.namespace, &api_resource);

    // Reflector: in-memory cache backed by K8s Watch API
    // Initial LIST is paginated (500 per page) to avoid overloading kube-apiserver
    let writer = reflector::store::Writer::<DynamicObject>::new(api_resource.clone());
    let store = writer.as_reader();
    let watch_config = watcher::Config::default().page_size(500);
    let rf = reflector(writer, watcher(apps, watch_config))
        .default_backoff()
        .applied_objects();

    // Drive the reflector stream in a background task
    tokio::spawn(async move {
        info!("Reflector started — watching Application CRs");
        let mut stream = rf.boxed();
        while let Some(event) = stream.next().await {
            match event {
                Ok(app) => {
                    let name = app.metadata.name.as_deref().unwrap_or("unknown");
                    tracing::debug!(app = name, "Application event processed");
                }
                Err(e) => {
                    warn!(error = %e, "Watcher error (retrying with backoff)");
                }
            }
        }
        error!("Reflector stream ended unexpectedly");
    });

    // HTTP server
    let state = AppState { store };
    let app = Router::new()
        .route("/metrics", get(handle_metrics))
        .route("/healthz", get(handle_health))
        .route("/readyz", get(handle_health))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server listening");

    axum::serve(listener, app).await?;

    Ok(())
}
