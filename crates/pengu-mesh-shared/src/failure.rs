use anyhow::Error;

use crate::types::{
    ArtifactFailureAttempt, ArtifactFailurePayload, BrowserSurfaceFailureAttempt,
    BrowserSurfaceFailurePayload, OperationFailureAttempt, OperationFailurePayload,
    OwnershipDenialAttempt, OwnershipDenialPayload, OwnershipScope, TabFailureAttempt,
    TabFailurePayload,
};

pub fn operation_failure_payload(
    operation: &str,
    attempted: OperationFailureAttempt,
    error: &Error,
) -> OperationFailurePayload {
    let reason = error.to_string();
    let recovery = operation_recovery(&attempted, &reason);
    OperationFailurePayload {
        operation: operation.to_string(),
        attempted,
        reason: reason.clone(),
        recovery,
        retry_likely: operation_retry_likely(&reason),
    }
}

pub fn operation_recovery(attempted: &OperationFailureAttempt, reason: &str) -> Vec<String> {
    let mut recovery = Vec::new();
    if reason.contains("unknown instance") {
        push_unique(&mut recovery, "run pengu-mesh instance-list".to_string());
    }
    if reason.contains("unknown run") {
        push_unique(
            &mut recovery,
            "run pengu-mesh run-list --limit 25".to_string(),
        );
    }
    if reason.contains("managed profile") && reason.contains("already exists") {
        push_unique(&mut recovery, "run pengu-mesh profile-list".to_string());
        push_unique(
            &mut recovery,
            "retry profile-create with a different --name".to_string(),
        );
    }
    if reason.contains(" is not installed") {
        push_unique(
            &mut recovery,
            "run pengu-mesh-doctor to inspect browser installation readiness".to_string(),
        );
        push_unique(
            &mut recovery,
            "retry with --channel chrome-dev, --channel chrome, or --channel chromium after confirming the browser is installed".to_string(),
        );
    }
    if reason.contains("external attach is disabled by default") {
        push_unique(
            &mut recovery,
            "set PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 before retrying instance-attach".to_string(),
        );
    }
    if reason.contains("parse cdp url")
        || reason.contains("missing host in cdp url")
        || reason.contains("missing port in cdp url")
    {
        push_unique(
            &mut recovery,
            "retry instance-attach with --cdp-url ws://127.0.0.1:<port>/devtools/browser/<id>"
                .to_string(),
        );
    }
    if reason.contains("connect CDP websocket")
        || reason.contains("GET http://")
        || reason.contains("timed out waiting for http://")
        || reason.contains("parse websocket URL")
    {
        push_unique(
            &mut recovery,
            "verify the remote debugging endpoint is reachable before retrying instance-attach"
                .to_string(),
        );
    }
    if reason.contains("requires writer lease")
        || reason.contains("requires an explicit writer lease")
        || reason.contains("held by")
    {
        if let Some(instance_id) = attempted.instance_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh lease-status --instance-id {instance_id}"),
            );
        } else {
            push_unique(
                &mut recovery,
                "run pengu-mesh lease-status --instance-id <instance-id>".to_string(),
            );
        }
    }
    if reason.contains("unknown tab") {
        if let Some(instance_id) = attempted.instance_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh tab-list --instance-id {instance_id}"),
            );
        } else {
            push_unique(
                &mut recovery,
                "run pengu-mesh instance-list, then run pengu-mesh tab-list --instance-id <instance-id>"
                    .to_string(),
            );
        }
    }
    if reason.contains("recording would capture") {
        push_unique(
            &mut recovery,
            "lower --duration-ms or raise --interval-ms before retrying recording-capture"
                .to_string(),
        );
    }
    if recovery.is_empty() {
        push_unique(
            &mut recovery,
            "run pengu-mesh diagnose to inspect host and runtime readiness".to_string(),
        );
    }
    recovery
}

pub fn operation_retry_likely(reason: &str) -> bool {
    if reason.contains("unknown instance")
        || reason.contains("unknown run")
        || reason.contains("managed profile")
        || reason.contains("already exists")
        || reason.contains(" is not installed")
        || reason.contains("external attach is disabled by default")
        || reason.contains("parse cdp url")
        || reason.contains("missing host in cdp url")
        || reason.contains("missing port in cdp url")
        || reason.contains("requires writer lease")
        || reason.contains("requires an explicit writer lease")
        || reason.contains("held by")
        || reason.contains("unknown tab")
        || reason.contains("recording would capture")
    {
        return false;
    }
    if reason.contains("timed out waiting for http://")
        || reason.contains("GET http://")
        || reason.contains("connect CDP websocket")
        || reason.contains("parse websocket URL")
        || reason.contains("browser app not running")
    {
        return true;
    }
    false
}

pub fn artifact_failure_payload(
    operation: &str,
    attempted: ArtifactFailureAttempt,
    error: &Error,
) -> ArtifactFailurePayload {
    let reason = error.to_string();
    let recovery = artifact_recovery(&attempted, &reason);
    ArtifactFailurePayload {
        operation: operation.to_string(),
        attempted,
        reason: reason.clone(),
        recovery,
        retry_likely: artifact_retry_likely(&reason),
    }
}

pub fn artifact_recovery(attempted: &ArtifactFailureAttempt, reason: &str) -> Vec<String> {
    let mut recovery = Vec::new();
    if reason.contains("unknown artifact") {
        if let Some(run_id) = attempted.run_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh artifact-list --run-id {run_id}"),
            );
        }
        if let Some(instance_id) = attempted.instance_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh artifact-list --instance-id {instance_id}"),
            );
        }
        push_unique(
            &mut recovery,
            "run pengu-mesh run-list --limit 25".to_string(),
        );
    }
    if reason.contains("artifact-list requires at least one filter") {
        push_unique(
            &mut recovery,
            "retry with --instance-id <instance-id> and/or --run-id <run-id>".to_string(),
        );
    }
    if reason.contains("missing stored sha256 metadata") {
        push_unique(
            &mut recovery,
            "capture a fresh artifact before running artifact-verify".to_string(),
        );
    }
    if reason.contains("No such file") || reason.contains("missing artifact source") {
        if let Some(artifact_id) = attempted.artifact_id.as_deref() {
            push_unique(
                &mut recovery,
                format!(
                    "treat artifact {artifact_id} as missing on disk and recreate it from the source capture"
                ),
            );
        } else {
            push_unique(
                &mut recovery,
                "treat the artifact as missing on disk and recreate it from the source capture"
                    .to_string(),
            );
        }
    }
    if reason.contains("checksum mismatch") {
        if let Some(artifact_id) = attempted.artifact_id.as_deref() {
            push_unique(
                &mut recovery,
                format!(
                    "treat artifact {artifact_id} as corrupted and recreate it from the source capture"
                ),
            );
        } else {
            push_unique(
                &mut recovery,
                "treat the artifact as corrupted and recreate it from the source capture"
                    .to_string(),
            );
        }
    }
    if recovery.is_empty() {
        push_unique(
            &mut recovery,
            "run pengu-mesh diagnose to inspect host and runtime readiness".to_string(),
        );
    }
    recovery
}

pub fn artifact_retry_likely(reason: &str) -> bool {
    if reason.contains("unknown artifact")
        || reason.contains("artifact-list requires at least one filter")
        || reason.contains("missing stored sha256 metadata")
        || reason.contains("No such file")
        || reason.contains("missing artifact source")
        || reason.contains("checksum mismatch")
    {
        return false;
    }
    if reason.contains("temporarily unavailable") || reason.contains("timed out") {
        return true;
    }
    false
}

pub fn browser_surface_failure_payload(
    operation: &str,
    attempted: BrowserSurfaceFailureAttempt,
    error: &Error,
) -> BrowserSurfaceFailurePayload {
    let reason = error.to_string();
    let recovery = browser_surface_recovery(&attempted, &reason);
    BrowserSurfaceFailurePayload {
        operation: operation.to_string(),
        attempted,
        reason: reason.clone(),
        recovery,
        retry_likely: browser_surface_retry_likely(&reason),
    }
}

pub fn browser_surface_recovery(
    attempted: &BrowserSurfaceFailureAttempt,
    reason: &str,
) -> Vec<String> {
    let mut recovery = Vec::new();
    if reason.contains("requires accessibility permission") {
        recovery.push(
            "run PENGU_MESH_CAPABILITY_GRANTS=host_access_setup pengu-mesh host-access-setup --mode apply --service accessibility".to_string(),
        );
    }
    if reason.contains("surface not found") {
        recovery.push(format!(
            "run pengu-mesh browser-surface-list --instance-id {}",
            attempted.instance_id
        ));
    }
    if reason.contains("unknown instance") {
        recovery.push("run pengu-mesh instance-list".to_string());
    }
    if reason.contains("requires writer lease") || reason.contains("held by") {
        recovery.push(format!(
            "run pengu-mesh lease-status --instance-id {}",
            attempted.instance_id
        ));
    }
    if reason.contains("browser app not running") {
        recovery.push(format!(
            "restart or reattach the browser for instance {} before retrying browser-surface commands",
            attempted.instance_id
        ));
    }
    if reason.contains("surface action failed to focus target") {
        recovery.push(format!(
            "rerun pengu-mesh browser-surface-list-actions --instance-id {} --surface-id {} to inspect safer fallback channels",
            attempted.instance_id,
            attempted.surface_id.as_deref().unwrap_or("ax:0")
        ));
    }
    if reason.contains("set_value requires value") {
        recovery.push("retry the action with --value <text>".to_string());
    }
    if reason.contains("unsupported execution channel") {
        recovery.push(
            "rerun pengu-mesh browser-surface-list-actions to inspect supported execution channels"
                .to_string(),
        );
    }
    if recovery.is_empty() {
        recovery.push("run pengu-mesh diagnose to inspect host and browser readiness".to_string());
    }
    recovery
}

pub fn browser_surface_retry_likely(reason: &str) -> bool {
    if reason.contains("surface not found")
        || reason.contains("requires accessibility permission")
        || reason.contains("unknown instance")
        || reason.contains("set_value requires value")
        || reason.contains("unsupported execution channel")
    {
        return false;
    }
    if reason.contains("browser app not running")
        || reason.contains("surface action failed to focus target")
        || reason.contains("timed out waiting for http://")
    {
        return true;
    }
    false
}

pub fn tab_failure_payload(
    operation: &str,
    attempted: TabFailureAttempt,
    error: &Error,
) -> TabFailurePayload {
    let reason = error.to_string();
    let recovery = tab_recovery(&attempted, &reason);
    TabFailurePayload {
        operation: operation.to_string(),
        attempted,
        reason: reason.clone(),
        recovery,
        retry_likely: tab_retry_likely(&reason),
    }
}

fn push_unique(recovery: &mut Vec<String>, item: String) {
    if !recovery.contains(&item) {
        recovery.push(item);
    }
}

pub fn tab_recovery(attempted: &TabFailureAttempt, reason: &str) -> Vec<String> {
    let mut recovery = Vec::new();
    if reason.contains("unknown instance") {
        push_unique(&mut recovery, "run pengu-mesh instance-list".to_string());
    }
    if reason.contains("unknown tab") || reason.contains("after websocket refresh") {
        if let Some(instance_id) = attempted.instance_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh tab-list --instance-id {instance_id}"),
            );
        } else {
            push_unique(
                &mut recovery,
                "run pengu-mesh tab-list --instance-id <instance-id>".to_string(),
            );
        }
    }
    if reason.contains("requires writer lease") || reason.contains("held by") {
        if let Some(instance_id) = attempted.instance_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh lease-status --instance-id {instance_id}"),
            );
        } else {
            push_unique(
                &mut recovery,
                "run pengu-mesh lease-status --instance-id <instance-id>".to_string(),
            );
        }
    }
    if reason.contains("navigate requires url") {
        push_unique(
            &mut recovery,
            "retry the action with --url <target>".to_string(),
        );
    }
    if reason.contains("press requires key") {
        push_unique(
            &mut recovery,
            "retry the action with --key <key>".to_string(),
        );
    }
    if reason.contains("requires ref or selector") {
        push_unique(
            &mut recovery,
            "retry the action with --ref <node-ref> or --selector <css-selector>".to_string(),
        );
    }
    if reason.contains("requires text") {
        push_unique(
            &mut recovery,
            "retry the action with --text <text>".to_string(),
        );
    }
    if reason.contains("select requires value") {
        push_unique(
            &mut recovery,
            "retry the action with --value <value>".to_string(),
        );
    }
    if (reason.contains("selector not found")
        || reason.contains("invalid ref")
        || (reason.starts_with("ref e") && reason.ends_with(" not found")))
        && let Some(tab_id) = attempted.tab_id.as_deref()
    {
        push_unique(
            &mut recovery,
            format!("run pengu-mesh tab-snapshot --tab-id {tab_id}"),
        );
        push_unique(
            &mut recovery,
            format!("run pengu-mesh tab-text --tab-id {tab_id}"),
        );
    }
    if reason.contains("navigation timed out") {
        if let Some(tab_id) = attempted.tab_id.as_deref() {
            push_unique(
                &mut recovery,
                format!("run pengu-mesh tab-snapshot --tab-id {tab_id}"),
            );
        }
        if let (Some(instance_id), Some(tab_id)) = (
            attempted.instance_id.as_deref(),
            attempted.tab_id.as_deref(),
        ) {
            push_unique(
                &mut recovery,
                format!(
                    "run pengu-mesh tab-list-actions --instance-id {instance_id} --tab-id {tab_id}"
                ),
            );
        }
    }
    if (reason.contains("Runtime.evaluate failed")
        || reason.contains("document has no root element")
        || reason.contains("requires an input, textarea, or select target")
        || reason.contains("select requires a select target"))
        && let Some(tab_id) = attempted.tab_id.as_deref()
    {
        push_unique(
            &mut recovery,
            format!("run pengu-mesh tab-snapshot --tab-id {tab_id}"),
        );
    }
    if reason.contains("missing screenshot data")
        && let Some(tab_id) = attempted.tab_id.as_deref()
    {
        push_unique(
            &mut recovery,
            format!("run pengu-mesh tab-snapshot --tab-id {tab_id}"),
        );
        push_unique(
            &mut recovery,
            format!("retry pengu-mesh tab-screenshot --tab-id {tab_id} --full-page"),
        );
    }
    if reason.contains("missing PDF data")
        && let Some(tab_id) = attempted.tab_id.as_deref()
    {
        push_unique(
            &mut recovery,
            format!("run pengu-mesh tab-screenshot --tab-id {tab_id} --full-page"),
        );
        push_unique(
            &mut recovery,
            format!("retry pengu-mesh tab-pdf --tab-id {tab_id}"),
        );
    }
    if recovery.is_empty() {
        push_unique(
            &mut recovery,
            "run pengu-mesh diagnose to inspect host and browser readiness".to_string(),
        );
    }
    recovery
}

pub fn tab_retry_likely(reason: &str) -> bool {
    if reason.contains("unknown instance")
        || reason.contains("unknown tab")
        || reason.contains("requires writer lease")
        || reason.contains("held by")
        || reason.contains("navigate requires url")
        || reason.contains("press requires key")
        || reason.contains("requires ref or selector")
        || reason.contains("requires text")
        || reason.contains("select requires value")
        || reason.contains("selector not found")
        || reason.contains("invalid ref")
        || (reason.starts_with("ref e") && reason.ends_with(" not found"))
        || reason.contains("missing screenshot data")
        || reason.contains("missing PDF data")
    {
        return false;
    }
    if reason.contains("navigation timed out")
        || reason.contains("connect CDP websocket")
        || reason.contains("reconnect tab websocket")
        || reason.contains("CDP websocket closed")
        || reason.contains("timed out waiting for http://")
    {
        return true;
    }
    false
}

pub fn ownership_denial_payload(
    operation: &str,
    holder_id: &str,
    scope: OwnershipScope,
    reason: &str,
) -> OwnershipDenialPayload {
    OwnershipDenialPayload {
        operation: operation.to_string(),
        attempted: OwnershipDenialAttempt {
            holder_id: holder_id.to_string(),
            scope,
        },
        reason: reason.to_string(),
        recovery: vec!["re-authenticate the holder before retrying the operation".to_string()],
        retry_likely: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        artifact_failure_payload, artifact_recovery, artifact_retry_likely,
        browser_surface_failure_payload, browser_surface_recovery, browser_surface_retry_likely,
        operation_failure_payload, operation_recovery, operation_retry_likely,
        ownership_denial_payload, tab_failure_payload, tab_recovery, tab_retry_likely,
    };
    use crate::{
        ArtifactFailureAttempt, BrowserSurfaceFailureAttempt, ExecutionChannel,
        OperationFailureAttempt, OwnershipScope, SurfaceActionKind, TabFailureAttempt,
    };
    use anyhow::anyhow;

    fn operation_attempted() -> OperationFailureAttempt {
        OperationFailureAttempt {
            operation: "instance_attach".to_string(),
            instance_id: Some("inst_demo".to_string()),
            holder_id: Some("agent_alpha".to_string()),
            detail: Some("cdp_url=ws://127.0.0.1:9222/devtools/browser/demo".to_string()),
        }
    }

    #[test]
    fn operation_payload_builds_recovery_for_attach_failures() {
        let payload = operation_failure_payload(
            "instance attached",
            operation_attempted(),
            &anyhow!(
                "external attach is disabled by default; set PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 to enable it"
            ),
        );
        assert_eq!(payload.operation, "instance attached");
        assert_eq!(payload.attempted.operation, "instance_attach");
        assert_eq!(
            payload.recovery,
            vec![
                "set PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 before retrying instance-attach"
                    .to_string()
            ]
        );
        assert!(!payload.retry_likely);
    }

    #[test]
    fn operation_recovery_handles_unknown_runs_and_leases() {
        let unknown_run = operation_recovery(&operation_attempted(), "unknown run run_missing");
        assert!(unknown_run.contains(&"run pengu-mesh run-list --limit 25".to_string()));

        let lease_conflict = operation_recovery(
            &OperationFailureAttempt {
                operation: "instance_stop".to_string(),
                instance_id: Some("inst_demo".to_string()),
                holder_id: Some("agent_alpha".to_string()),
                detail: None,
            },
            "instance_stop requires writer lease for inst_demo; held by agent_beta until 2026-03-12T00:00:00Z",
        );
        assert!(
            lease_conflict
                .contains(&"run pengu-mesh lease-status --instance-id inst_demo".to_string())
        );
    }

    #[test]
    fn operation_retry_likely_distinguishes_attach_readiness_from_static_input_errors() {
        assert!(operation_retry_likely(
            "timed out waiting for http://127.0.0.1:9222/json/version"
        ));
        assert!(!operation_retry_likely("parse cdp url"));
        assert!(!operation_retry_likely("unknown run run_missing"));
    }

    fn attempted() -> BrowserSurfaceFailureAttempt {
        BrowserSurfaceFailureAttempt {
            instance_id: "inst_demo".to_string(),
            surface_id: Some("ax:0/4".to_string()),
            root_surface_id: None,
            action: Some(SurfaceActionKind::Focus),
            execution_channel: Some(ExecutionChannel::AppleEventsActivation),
            allow_takeover: Some(false),
        }
    }

    #[test]
    fn payload_builds_recovery_for_known_errors() {
        let payload = browser_surface_failure_payload(
            "browser_surface_list_actions",
            attempted(),
            &anyhow!("surface not found"),
        );
        assert_eq!(payload.operation, "browser_surface_list_actions");
        assert_eq!(
            payload.recovery,
            vec!["run pengu-mesh browser-surface-list --instance-id inst_demo".to_string()]
        );
        assert!(!payload.retry_likely);
    }

    #[test]
    fn recovery_falls_back_to_diagnose() {
        let recovery = browser_surface_recovery(&attempted(), "unexpected bridge failure");
        assert_eq!(
            recovery,
            vec!["run pengu-mesh diagnose to inspect host and browser readiness".to_string()]
        );
    }

    #[test]
    fn retry_likely_is_true_for_running_browser_retries() {
        assert!(browser_surface_retry_likely("browser app not running"));
        assert!(!browser_surface_retry_likely("unknown instance"));
    }

    fn tab_attempted() -> TabFailureAttempt {
        TabFailureAttempt {
            instance_id: Some("inst_demo".to_string()),
            tab_id: Some("tab_demo".to_string()),
            action_kind: Some("navigate".to_string()),
        }
    }

    #[test]
    fn tab_payload_builds_recovery_for_missing_instance() {
        let payload = tab_failure_payload(
            "tab list",
            tab_attempted(),
            &anyhow!("unknown instance inst_demo"),
        );
        assert_eq!(payload.operation, "tab list");
        assert_eq!(
            payload.recovery,
            vec!["run pengu-mesh instance-list".to_string()]
        );
        assert!(!payload.retry_likely);
    }

    #[test]
    fn tab_recovery_handles_navigation_timeout_and_dom_errors() {
        let timeout_recovery = tab_recovery(
            &tab_attempted(),
            "navigation timed out waiting for load event",
        );
        assert!(
            timeout_recovery.contains(&"run pengu-mesh tab-snapshot --tab-id tab_demo".to_string())
        );
        assert!(
            timeout_recovery.contains(
                &"run pengu-mesh tab-list-actions --instance-id inst_demo --tab-id tab_demo"
                    .to_string()
            )
        );

        let dom_recovery = tab_recovery(&tab_attempted(), "selector not found: #missing");
        assert!(
            dom_recovery.contains(&"run pengu-mesh tab-snapshot --tab-id tab_demo".to_string())
        );
        assert!(dom_recovery.contains(&"run pengu-mesh tab-text --tab-id tab_demo".to_string()));
    }

    #[test]
    fn tab_recovery_handles_runtime_evaluate_failures() {
        let recovery = tab_recovery(&tab_attempted(), "Runtime.evaluate failed: Error: boom");
        assert!(recovery.contains(&"run pengu-mesh tab-snapshot --tab-id tab_demo".to_string()));
    }

    #[test]
    fn tab_recovery_handles_screenshot_and_pdf_failures() {
        let screenshot_recovery = tab_recovery(&tab_attempted(), "missing screenshot data");
        assert!(screenshot_recovery.contains(
            &"retry pengu-mesh tab-screenshot --tab-id tab_demo --full-page".to_string()
        ));

        let pdf_recovery = tab_recovery(&tab_attempted(), "missing PDF data");
        assert!(pdf_recovery.contains(&"retry pengu-mesh tab-pdf --tab-id tab_demo".to_string()));
        assert!(
            pdf_recovery.contains(
                &"run pengu-mesh tab-screenshot --tab-id tab_demo --full-page".to_string()
            )
        );
    }

    #[test]
    fn tab_retry_likely_distinguishes_timeout_from_input_errors() {
        assert!(tab_retry_likely(
            "navigation timed out waiting for load event"
        ));
        assert!(!tab_retry_likely("selector not found: #missing"));
        assert!(!tab_retry_likely("missing PDF data"));
    }

    fn artifact_attempted() -> ArtifactFailureAttempt {
        ArtifactFailureAttempt {
            artifact_id: Some("artifact_demo".to_string()),
            instance_id: Some("inst_demo".to_string()),
            run_id: Some("run_demo".to_string()),
            action_kind: Some("verify".to_string()),
        }
    }

    #[test]
    fn artifact_payload_builds_recovery_for_missing_artifacts() {
        let payload = artifact_failure_payload(
            "artifact verify",
            artifact_attempted(),
            &anyhow!("unknown artifact artifact_demo"),
        );
        assert_eq!(payload.operation, "artifact verify");
        assert!(
            payload
                .recovery
                .contains(&"run pengu-mesh artifact-list --run-id run_demo".to_string())
        );
        assert!(
            payload
                .recovery
                .contains(&"run pengu-mesh artifact-list --instance-id inst_demo".to_string())
        );
        assert!(!payload.retry_likely);
    }

    #[test]
    fn artifact_recovery_handles_missing_files_and_corruption() {
        let missing = artifact_recovery(&artifact_attempted(), "No such file or directory");
        assert!(missing.iter().any(|item| item.contains("missing on disk")));

        let corrupted = artifact_recovery(&artifact_attempted(), "checksum mismatch for artifact");
        assert!(corrupted.iter().any(|item| item.contains("corrupted")));
    }

    #[test]
    fn artifact_retry_likely_distinguishes_transient_from_static_errors() {
        assert!(!artifact_retry_likely("unknown artifact artifact_demo"));
        assert!(!artifact_retry_likely(
            "checksum mismatch for artifact_demo"
        ));
        assert!(artifact_retry_likely("storage temporarily unavailable"));
    }

    #[test]
    fn ownership_denial_payload_builds_structured_denial() {
        let payload = ownership_denial_payload(
            "lease_acquire",
            "agent_alpha",
            OwnershipScope::Instance,
            "token expired",
        );
        assert_eq!(payload.operation, "lease_acquire");
        assert_eq!(payload.attempted.holder_id, "agent_alpha");
        assert_eq!(payload.attempted.scope, OwnershipScope::Instance);
        assert_eq!(payload.reason, "token expired");
        assert_eq!(
            payload.recovery,
            vec!["re-authenticate the holder before retrying the operation".to_string()]
        );
        assert!(!payload.retry_likely);

        let encoded = serde_json::to_value(&payload).expect("serialize ownership denial");
        assert_eq!(encoded["attempted"]["scope"], "instance");
    }
}
