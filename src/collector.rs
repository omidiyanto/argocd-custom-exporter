use kube::api::DynamicObject;

/// Annotation key set by ApplicationSet template to track Git-expected autosync state.
pub const ANNOTATION_KEY: &str = "argocd-exporter/git-autosync";

/// Result of drift analysis for a single ArgoCD Application.
pub struct DriftInfo {
    pub app_name: String,
    pub environment: String,
    pub tenant: String,
    pub git_autosync: bool,
    pub actual_autosync: bool,
}

impl DriftInfo {
    /// Returns true if the actual autosync state differs from Git-expected state.
    pub fn is_drift(&self) -> bool {
        self.git_autosync != self.actual_autosync
    }
}

/// Analyze a single Application CR for autosync drift.
///
/// Returns `None` if the Application doesn't have the exporter annotation
/// (i.e., it's not managed by an ApplicationSet with drift tracking).
pub fn analyze(app: &DynamicObject) -> Option<DriftInfo> {
    // Only process applications with our annotation
    let annotations = app.metadata.annotations.as_ref()?;
    let git_value = annotations.get(ANNOTATION_KEY)?;
    let git_autosync = git_value == "true";

    // Check if spec.syncPolicy.automated exists and is not explicitly set to `enabled: false`
    let actual_autosync = app
        .data
        .get("spec")
        .and_then(|spec| spec.get("syncPolicy"))
        .and_then(|sp| sp.get("automated"))
        .map(|v| {
            if v.is_null() {
                return false;
            }
            // Check if "enabled" key exists inside "automated" block (ArgoCD UI behavior)
            if let Some(enabled) = v.get("enabled") {
                if let Some(b) = enabled.as_bool() {
                    return b;
                }
            }
            // If "automated" block exists but no "enabled: false", it is implicitly enabled
            true
        })
        .unwrap_or(false);

    // Extract labels for metric dimensions
    let labels = app.metadata.labels.as_ref();
    let environment = labels
        .and_then(|l| l.get("environment"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let tenant = labels
        .and_then(|l| l.get("tenant"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let app_name = app
        .metadata
        .name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    Some(DriftInfo {
        app_name,
        environment,
        tenant,
        git_autosync,
        actual_autosync,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::api::ObjectMeta;
    use std::collections::BTreeMap;

    fn make_app(
        name: &str,
        annotation_value: Option<&str>,
        has_automated: bool,
        env: &str,
        tenant: &str,
    ) -> DynamicObject {
        let mut annotations = BTreeMap::new();
        if let Some(v) = annotation_value {
            annotations.insert(ANNOTATION_KEY.to_string(), v.to_string());
        }

        let mut labels = BTreeMap::new();
        labels.insert("environment".to_string(), env.to_string());
        labels.insert("tenant".to_string(), tenant.to_string());

        let sync_policy = if has_automated {
            serde_json::json!({
                "automated": { "prune": true, "selfHeal": true },
                "syncOptions": ["CreateNamespace=true"]
            })
        } else {
            serde_json::json!({
                "syncOptions": ["CreateNamespace=true"]
            })
        };

        DynamicObject {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                annotations: Some(annotations),
                labels: Some(labels),
                ..Default::default()
            },
            types: None,
            data: serde_json::json!({
                "spec": { "syncPolicy": sync_policy }
            }),
        }
    }

    #[test]
    fn no_drift_both_enabled() {
        let app = make_app("app1", Some("true"), true, "dev", "asus");
        let info = analyze(&app).unwrap();
        assert!(!info.is_drift());
        assert!(info.git_autosync);
        assert!(info.actual_autosync);
    }

    #[test]
    fn no_drift_both_disabled() {
        let app = make_app("app2", Some("false"), false, "dev", "asus");
        let info = analyze(&app).unwrap();
        assert!(!info.is_drift());
    }

    #[test]
    fn drift_git_enabled_actual_disabled() {
        let app = make_app("app3", Some("true"), false, "dev", "asus");
        let info = analyze(&app).unwrap();
        assert!(info.is_drift());
        assert!(info.git_autosync);
        assert!(!info.actual_autosync);
    }

    #[test]
    fn drift_git_disabled_actual_enabled() {
        let app = make_app("app4", Some("false"), true, "staging", "lenovo");
        let info = analyze(&app).unwrap();
        assert!(info.is_drift());
        assert!(!info.git_autosync);
        assert!(info.actual_autosync);
    }

    #[test]
    fn skip_without_annotation() {
        let app = make_app("app5", None, true, "dev", "asus");
        assert!(analyze(&app).is_none());
    }

    #[test]
    fn labels_extracted_correctly() {
        let app = make_app("lumina-docs-dev-asus", Some("true"), true, "dev", "asus");
        let info = analyze(&app).unwrap();
        assert_eq!(info.app_name, "lumina-docs-dev-asus");
        assert_eq!(info.environment, "dev");
        assert_eq!(info.tenant, "asus");
    }
}
