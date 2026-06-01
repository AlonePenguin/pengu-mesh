use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use pengu_mesh_core::{DoctorReport, StageOneRuntime, build_diagnose_report_in_root, runtime_root};
use pengu_mesh_shared::{
    BrowserChannel, DiagnoseRemediation, DiagnoseReport, DiagnoseState,
    ExecutionChannelAvailability, HostAccessProbe, HostAccessService, PermissionState,
};

pub fn build_report() -> Result<DoctorReport> {
    build_report_in_root(runtime_root()?)
}

pub fn build_setup_wizard() -> Result<SetupWizardReport> {
    build_setup_wizard_in_root(runtime_root()?)
}

fn build_report_in_root(root: impl Into<PathBuf>) -> Result<DoctorReport> {
    let root = root.into();
    let entrypoint = if doctor_should_follow_daemon(&root) {
        "pengu-mesh-daemon"
    } else {
        "pengu-mesh-doctor"
    };
    StageOneRuntime::new_in_root(root, entrypoint)?.doctor_report()
}

fn build_setup_wizard_in_root(root: impl Into<PathBuf>) -> Result<SetupWizardReport> {
    let root = root.into();
    let doctor = build_report_in_root(root.clone())?;
    let diagnose = build_diagnose_report_in_root(root)?;
    Ok(build_setup_wizard_from_reports(doctor, diagnose))
}

fn doctor_should_follow_daemon(root: &Path) -> bool {
    root.join("daemon.json").exists()
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupWizardReport {
    pub generated_at: String,
    pub platform: String,
    pub diagnose_state: DiagnoseState,
    pub summary: String,
    pub runtime_root: String,
    pub host_access_summary: String,
    pub browser_summary: String,
    pub read_only: bool,
    pub action_required: usize,
    pub completed: usize,
    pub steps: Vec<SetupWizardStep>,
    pub execution_channels: Vec<ExecutionChannelAvailability>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupWizardStep {
    pub service: HostAccessService,
    pub label: String,
    pub state: PermissionState,
    pub recommended: bool,
    pub requestable: bool,
    pub detail: String,
    pub why_it_matters: String,
    pub remediation_title: Option<String>,
    pub remediation_summary: Option<String>,
    pub cli_command: Option<String>,
    pub open_settings_url: Option<String>,
    pub open_settings_command: Option<String>,
}

fn build_setup_wizard_from_reports(
    doctor: DoctorReport,
    diagnose: DiagnoseReport,
) -> SetupWizardReport {
    let host_access = &doctor.permissions.host_access;
    let mut steps = host_access
        .services
        .iter()
        .map(|probe| {
            let remediation = diagnose_remediation_for_service(&diagnose, &probe.service);
            build_setup_wizard_step(
                probe,
                remediation,
                host_access.recommended_services.contains(&probe.service)
                    && service_target_is_relevant(&probe.service, &doctor),
            )
        })
        .collect::<Vec<_>>();
    steps.sort_by_key(setup_wizard_step_sort_key);

    let action_required = steps
        .iter()
        .filter(|step| step_needs_attention(step) && step.recommended)
        .count();
    let completed = steps
        .iter()
        .filter(|step| step.state == PermissionState::Granted)
        .count();
    SetupWizardReport {
        generated_at: diagnose.generated_at,
        platform: diagnose.platform,
        diagnose_state: diagnose.state,
        summary: diagnose.summary,
        runtime_root: diagnose.runtime_root,
        host_access_summary: host_access.summary.clone(),
        browser_summary: browser_summary(&doctor),
        read_only: true,
        action_required,
        completed,
        steps,
        execution_channels: host_access.execution_channels.clone(),
    }
}

fn build_setup_wizard_step(
    probe: &HostAccessProbe,
    remediation: Option<&DiagnoseRemediation>,
    recommended: bool,
) -> SetupWizardStep {
    SetupWizardStep {
        service: probe.service.clone(),
        label: host_access_service_label(&probe.service).to_string(),
        state: probe.state.clone(),
        recommended,
        requestable: probe.requestable,
        detail: probe.detail.clone(),
        why_it_matters: host_access_prerequisite_detail(&probe.service).to_string(),
        remediation_title: remediation.map(|value| value.title.clone()),
        remediation_summary: remediation.map(|value| value.summary.clone()),
        cli_command: remediation.and_then(|value| value.cli_command.clone()),
        open_settings_url: probe.open_settings_url.clone(),
        open_settings_command: probe
            .open_settings_url
            .as_deref()
            .map(open_settings_command),
    }
}

fn diagnose_remediation_for_service<'a>(
    diagnose: &'a DiagnoseReport,
    service: &HostAccessService,
) -> Option<&'a DiagnoseRemediation> {
    diagnose
        .permissions
        .iter()
        .find(|permission| &permission.service == service)
        .and_then(|permission| permission.remediation_ids.first())
        .and_then(|remediation_id| {
            diagnose
                .remediations
                .iter()
                .find(|remediation| &remediation.id == remediation_id)
        })
}

fn setup_wizard_step_sort_key(step: &SetupWizardStep) -> (u8, u8, &'static str) {
    (
        if step_needs_attention(step) { 0 } else { 1 },
        host_access_service_order(&step.service),
        step.service.as_str(),
    )
}

fn step_needs_attention(step: &SetupWizardStep) -> bool {
    matches!(
        step.state,
        PermissionState::Missing | PermissionState::Unknown
    )
}

fn browser_summary(report: &DoctorReport) -> String {
    let installed = report
        .browser_installs
        .iter()
        .filter(|install| install.installed)
        .map(|install| {
            let app_path = if install.app_path.is_empty() {
                "path unavailable"
            } else {
                install.app_path.as_str()
            };
            format!("{} ({app_path})", browser_channel_label(&install.channel))
        })
        .collect::<Vec<_>>();
    if installed.is_empty() {
        "no supported browser installs detected".to_string()
    } else {
        installed.join(", ")
    }
}

fn browser_channel_label(channel: &BrowserChannel) -> &'static str {
    match channel {
        BrowserChannel::Chrome => "Google Chrome",
        BrowserChannel::ChromeDev => "Google Chrome Dev",
        BrowserChannel::Chromium => "Chromium",
    }
}

fn service_target_is_relevant(service: &HostAccessService, report: &DoctorReport) -> bool {
    match service {
        HostAccessService::AppleEventsChrome => {
            browser_channel_installed(report, BrowserChannel::Chrome)
        }
        HostAccessService::AppleEventsChromeDev => {
            browser_channel_installed(report, BrowserChannel::ChromeDev)
        }
        HostAccessService::AppleEventsChromium => {
            browser_channel_installed(report, BrowserChannel::Chromium)
        }
        _ => true,
    }
}

fn browser_channel_installed(report: &DoctorReport, channel: BrowserChannel) -> bool {
    report
        .browser_installs
        .iter()
        .any(|install| install.channel == channel && install.installed)
}

fn host_access_service_order(service: &HostAccessService) -> u8 {
    match service {
        HostAccessService::Accessibility => 0,
        HostAccessService::ScreenCapture => 1,
        HostAccessService::ListenEvent => 2,
        HostAccessService::AppleEventsChromeDev => 3,
        HostAccessService::AppleEventsChrome => 4,
        HostAccessService::AppleEventsChromium => 5,
        HostAccessService::DevtoolsSecurity => 6,
    }
}

fn host_access_service_label(service: &HostAccessService) -> &'static str {
    match service {
        HostAccessService::Accessibility => "Accessibility permission",
        HostAccessService::ScreenCapture => "Screen Capture permission",
        HostAccessService::ListenEvent => "Listen Event permission",
        HostAccessService::AppleEventsChrome => "Apple Events for Google Chrome",
        HostAccessService::AppleEventsChromeDev => "Apple Events for Google Chrome Dev",
        HostAccessService::AppleEventsChromium => "Apple Events for Chromium",
        HostAccessService::DevtoolsSecurity => "DevToolsSecurity authorization",
    }
}

fn host_access_prerequisite_detail(service: &HostAccessService) -> &'static str {
    match service {
        HostAccessService::Accessibility => {
            "Required for native browser-surface discovery, focus, and background-safe UI actions."
        }
        HostAccessService::ScreenCapture => {
            "Required for native screen and window capture proof when browser-visible evidence matters."
        }
        HostAccessService::ListenEvent => {
            "Required for global takeover key posting when a workflow cannot stay app-scoped."
        }
        HostAccessService::AppleEventsChrome => {
            "Required for app-activation fallback against Google Chrome without pretending read-only checks can verify consent."
        }
        HostAccessService::AppleEventsChromeDev => {
            "Required for app-activation fallback against Google Chrome Dev without pretending read-only checks can verify consent."
        }
        HostAccessService::AppleEventsChromium => {
            "Required for app-activation fallback against Chromium without pretending read-only checks can verify consent."
        }
        HostAccessService::DevtoolsSecurity => {
            "Required so Apple-signed developer tooling can attach cleanly without extra authorization prompts."
        }
    }
}

fn permission_state_label(state: &PermissionState) -> &'static str {
    match state {
        PermissionState::Granted => "granted",
        PermissionState::Missing => "missing",
        PermissionState::Unsupported => "unsupported",
        PermissionState::Unknown => "unknown",
    }
}

fn diagnose_state_label(state: &DiagnoseState) -> &'static str {
    match state {
        DiagnoseState::Ready => "ready",
        DiagnoseState::Degraded => "degraded",
        DiagnoseState::Blocked => "blocked",
        DiagnoseState::Unknown => "unknown",
        DiagnoseState::Unsupported => "unsupported",
    }
}

fn open_settings_command(url: &str) -> String {
    format!("open -g {}", shell_single_quote(url))
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub fn render_setup_wizard(report: &SetupWizardReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "pengu mesh setup wizard");
    let _ = writeln!(output, "Generated: {}", report.generated_at);
    let _ = writeln!(output, "Platform: {}", report.platform);
    let _ = writeln!(
        output,
        "Readiness: {}",
        diagnose_state_label(&report.diagnose_state)
    );
    let _ = writeln!(output, "Summary: {}", report.summary);
    let _ = writeln!(output, "Runtime root: {}", report.runtime_root);
    let _ = writeln!(output, "Browsers: {}", report.browser_summary);
    let _ = writeln!(output, "Host access: {}", report.host_access_summary);
    let _ = writeln!(
        output,
        "Read-only: this flow reports truthful checks and next steps but does not request permissions or open settings."
    );
    let _ = writeln!(
        output,
        "Progress: {} action item(s), {} already granted.",
        report.action_required, report.completed
    );

    let action_items = report
        .steps
        .iter()
        .filter(|step| step_needs_attention(step) && step.recommended)
        .collect::<Vec<_>>();
    if action_items.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "Action items");
        let _ = writeln!(
            output,
            "- none; every requestable host-access prerequisite that can be checked read-only is already satisfied"
        );
    } else {
        let _ = writeln!(output);
        let _ = writeln!(output, "Action items");
        for (index, step) in action_items.iter().enumerate() {
            let _ = writeln!(
                output,
                "{}. {} [{}]",
                index + 1,
                step.label,
                permission_state_label(&step.state)
            );
            let _ = writeln!(output, "   Why: {}", step.why_it_matters);
            let _ = writeln!(output, "   Check: {}", step.detail);
            if let Some(summary) = step.remediation_summary.as_deref() {
                let _ = writeln!(output, "   Remediation: {summary}");
            }
            if let Some(cli_command) = step.cli_command.as_deref() {
                let _ = writeln!(output, "   CLI: {cli_command}");
            }
            if let Some(open_settings_url) = step.open_settings_url.as_deref() {
                let _ = writeln!(output, "   Open settings URL: {open_settings_url}");
            }
            if let Some(open_settings_command) = step.open_settings_command.as_deref() {
                let _ = writeln!(output, "   Open settings command: {open_settings_command}");
            }
        }
    }

    let optional_items = report
        .steps
        .iter()
        .filter(|step| step_needs_attention(step) && !step.recommended)
        .collect::<Vec<_>>();
    if !optional_items.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "Optional follow-up");
        for step in optional_items {
            let _ = writeln!(
                output,
                "- {} [{}]: {}",
                step.label,
                permission_state_label(&step.state),
                step.detail
            );
            if let Some(cli_command) = step.cli_command.as_deref() {
                let _ = writeln!(output, "  CLI: {cli_command}");
            }
            if let Some(open_settings_url) = step.open_settings_url.as_deref() {
                let _ = writeln!(output, "  Open settings URL: {open_settings_url}");
            }
        }
    }

    let granted = report
        .steps
        .iter()
        .filter(|step| step.state == PermissionState::Granted)
        .collect::<Vec<_>>();
    if !granted.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "Already satisfied");
        for step in granted {
            let _ = writeln!(output, "- {}: {}", step.label, step.detail);
        }
    }

    let unsupported = report
        .steps
        .iter()
        .filter(|step| step.state == PermissionState::Unsupported)
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "Unsupported");
        for step in unsupported {
            let _ = writeln!(output, "- {}: {}", step.label, step.detail);
        }
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "Execution channels");
    for channel in &report.execution_channels {
        let availability = if channel.available {
            "ready"
        } else {
            "blocked"
        };
        let _ = writeln!(
            output,
            "- {} [{}]: {}",
            channel.channel.as_str(),
            availability,
            channel.detail
        );
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{
        SetupWizardReport, SetupWizardStep, build_report, build_report_in_root,
        build_setup_wizard_in_root, build_setup_wizard_step, doctor_should_follow_daemon,
        render_setup_wizard,
    };
    use pengu_mesh_core::StageOneRuntime;
    use pengu_mesh_shared::{
        DiagnoseRemediation, DiagnoseState, ExecutionChannel, ExecutionChannelAvailability,
        HostAccessProbe, HostAccessService, InterferenceLevel, PermissionState,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let counter = TEST_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("pengu-mesh-doctor-test-{nanos}-{counter}"))
    }

    #[test]
    fn report_contains_core_sections() {
        let report = build_report().expect("doctor report");
        assert!(!report.tools.is_empty());
        assert!(!report.browser_installs.is_empty());
        assert!(!report.scenario_evidence.summary.is_empty());
    }

    #[test]
    fn doctor_follows_daemon_continuity_when_daemon_metadata_exists() {
        let root = unique_test_root();
        let runtime = StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-daemon")
            .expect("daemon runtime");
        runtime
            .write_daemon_metadata("127.0.0.1:43127")
            .expect("daemon metadata");
        assert!(doctor_should_follow_daemon(&root));

        let report = build_report_in_root(root.clone()).expect("doctor report");
        assert!(report.continuity.continuity_enabled);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn setup_wizard_contains_host_access_steps() {
        let root = unique_test_root();
        let wizard = build_setup_wizard_in_root(root.clone()).expect("setup wizard");
        assert!(!wizard.steps.is_empty());
        assert!(wizard.read_only);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn setup_wizard_step_includes_cli_and_open_command() {
        let probe = HostAccessProbe {
            service: HostAccessService::Accessibility,
            state: PermissionState::Missing,
            requestable: true,
            open_settings_url: Some(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
                    .to_string(),
            ),
            detail: "AXIsProcessTrusted".to_string(),
        };
        let remediation = DiagnoseRemediation {
            id: "host_access_apply_accessibility".to_string(),
            title: "Request Accessibility permission".to_string(),
            summary: "Run pengu-mesh host-access-setup in apply mode for Accessibility permission."
                .to_string(),
            cli_command: Some(
                "PENGU_MESH_CAPABILITY_GRANTS=host_access_setup pengu-mesh host-access-setup --mode apply --service accessibility".to_string(),
            ),
            mcp_tool: None,
            mcp_arguments: None,
            http_method: None,
            http_route: None,
            http_body: None,
            manual_only: false,
        };

        let step = build_setup_wizard_step(&probe, Some(&remediation), true);
        assert_eq!(
            step.cli_command.as_deref(),
            Some(
                "PENGU_MESH_CAPABILITY_GRANTS=host_access_setup pengu-mesh host-access-setup --mode apply --service accessibility"
            )
        );
        assert_eq!(
            step.open_settings_command.as_deref(),
            Some(
                "open -g 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'"
            )
        );
    }

    #[test]
    fn setup_wizard_renderer_includes_action_items_and_channels() {
        let wizard = SetupWizardReport {
            generated_at: "2026-03-12T00:00:00Z".to_string(),
            platform: "macos".to_string(),
            diagnose_state: DiagnoseState::Degraded,
            summary: "native surface actions are blocked by missing host access".to_string(),
            runtime_root: "/tmp/pengu-mesh".to_string(),
            host_access_summary: "3 of 7 host access services are currently granted".to_string(),
            browser_summary: "Google Chrome Dev (/Applications/Google Chrome Dev.app)".to_string(),
            read_only: true,
            action_required: 1,
            completed: 1,
            steps: vec![
                SetupWizardStep {
                    service: HostAccessService::Accessibility,
                    label: "Accessibility permission".to_string(),
                    state: PermissionState::Missing,
                    recommended: true,
                    requestable: true,
                    detail: "AXIsProcessTrusted".to_string(),
                    why_it_matters:
                        "Required for native browser-surface discovery, focus, and background-safe UI actions."
                            .to_string(),
                    remediation_title: Some("Request Accessibility permission".to_string()),
                    remediation_summary: Some(
                        "Run pengu-mesh host-access-setup in apply mode for Accessibility permission."
                            .to_string(),
                    ),
                    cli_command: Some(
                        "PENGU_MESH_CAPABILITY_GRANTS=host_access_setup pengu-mesh host-access-setup --mode apply --service accessibility"
                            .to_string(),
                    ),
                    open_settings_url: Some(
                        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
                            .to_string(),
                    ),
                    open_settings_command: Some(
                        "open -g 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'"
                            .to_string(),
                    ),
                },
                SetupWizardStep {
                    service: HostAccessService::ScreenCapture,
                    label: "Screen Capture permission".to_string(),
                    state: PermissionState::Granted,
                    recommended: false,
                    requestable: true,
                    detail: "CGPreflightScreenCaptureAccess".to_string(),
                    why_it_matters: "Required for native screen and window capture proof when browser-visible evidence matters."
                        .to_string(),
                    remediation_title: None,
                    remediation_summary: None,
                    cli_command: None,
                    open_settings_url: None,
                    open_settings_command: None,
                },
            ],
            execution_channels: vec![ExecutionChannelAvailability {
                channel: ExecutionChannel::AxDirect,
                available: false,
                interference_level: InterferenceLevel::BackgroundSafe,
                detail: "Direct macOS Accessibility action and discovery path.".to_string(),
            }],
        };

        let rendered = render_setup_wizard(&wizard);
        assert!(rendered.contains("pengu mesh setup wizard"));
        assert!(rendered.contains("Accessibility permission [missing]"));
        assert!(rendered.contains("Open settings command"));
        assert!(rendered.contains("Execution channels"));
        assert!(rendered.contains("ax_direct [blocked]"));
    }
}
