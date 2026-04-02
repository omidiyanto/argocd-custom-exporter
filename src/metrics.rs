use std::fmt::Write;

use kube::{api::DynamicObject, runtime::reflector};

use crate::collector;

/// Render Prometheus metrics text from the current reflector store state.
///
/// This is called on each GET /metrics request. It iterates the in-memory
/// cache (populated by the K8s Watch reflector) and computes drift on the fly.
/// No interval-based polling — the cache is always up-to-date via events.
pub fn render(store: &reflector::Store<DynamicObject>) -> String {
    let apps = store.state();
    let mut out = String::with_capacity(4096);

    // Per-application drift metric
    let _ = writeln!(
        out,
        "# HELP argocd_autosync_drift Whether the autosync policy has drifted from Git state (1=drift, 0=healthy)"
    );
    let _ = writeln!(out, "# TYPE argocd_autosync_drift gauge");

    let mut drift_count: u64 = 0;
    let mut tracked_count: u64 = 0;

    for app in apps.iter() {
        let Some(info) = collector::analyze(app) else {
            continue;
        };

        tracked_count += 1;
        let drift_value: u8 = if info.is_drift() {
            drift_count += 1;
            1
        } else {
            0
        };

        let git_str = if info.git_autosync { "true" } else { "false" };
        let actual_str = if info.actual_autosync {
            "true"
        } else {
            "false"
        };

        let _ = writeln!(
            out,
            "argocd_autosync_drift{{app=\"{}\",environment=\"{}\",tenant=\"{}\",git_autosync=\"{}\",actual_autosync=\"{}\"}} {}",
            info.app_name, info.environment, info.tenant, git_str, actual_str, drift_value
        );
    }

    // Aggregate metrics
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "# HELP argocd_autosync_drift_total Total number of applications with autosync drift"
    );
    let _ = writeln!(out, "# TYPE argocd_autosync_drift_total gauge");
    let _ = writeln!(out, "argocd_autosync_drift_total {drift_count}");

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "# HELP argocd_autosync_tracked_total Total number of tracked applications with git-autosync annotation"
    );
    let _ = writeln!(out, "# TYPE argocd_autosync_tracked_total gauge");
    let _ = writeln!(out, "argocd_autosync_tracked_total {tracked_count}");

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "# HELP argocd_exporter_up Whether the exporter is operational (1=up, 0=down)"
    );
    let _ = writeln!(out, "# TYPE argocd_exporter_up gauge");
    let _ = writeln!(out, "argocd_exporter_up 1");

    out
}
