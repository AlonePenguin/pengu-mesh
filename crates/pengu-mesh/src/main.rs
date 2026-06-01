use anyhow::{Context, Result, bail};
use clap::{ArgAction, Parser, Subcommand};
use pengu_mesh_core::{
    StageOneRuntime, build_diagnose_report, scenario_finish_run, scenario_finish_step,
    scenario_record_assertion, scenario_record_latency, scenario_record_run, scenario_record_step,
};
use pengu_mesh_http::{HttpRequest, HttpResponse, read_request, write_response};
use pengu_mesh_mcp::{ToolCallRequest, classify_error, execute_tool, mcp_tools_list};
use pengu_mesh_shared::{
    IdKind, OperationFailureAttempt, OperationOutcome, OutcomeCode, StableId,
    operation_failure_payload,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::net::{TcpListener, TcpStream};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: CommandSet,
}

#[derive(Subcommand)]
enum CommandSet {
    Serve {
        #[arg(long, default_value = "127.0.0.1:43127")]
        bind: String,
    },
    Health,
    Diagnose,
    CapabilityPreflight {
        #[arg(long)]
        capability: Option<String>,
    },
    HostAccessStatus,
    HostAccessSetup {
        #[arg(long, default_value = "audit")]
        mode: String,
        #[arg(long = "service")]
        service: Vec<String>,
        #[arg(long)]
        open_settings_on_missing: bool,
    },
    ProfileList,
    ProfileCreate {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "chrome-dev")]
        channel: String,
    },
    InstanceList,
    InstanceStart {
        #[arg(long, default_value = "stage1")]
        name: String,
        #[arg(long, default_value = "chrome-dev")]
        channel: String,
        #[arg(long)]
        headless: bool,
        #[arg(long)]
        holder_id: Option<String>,
    },
    InstanceAttach {
        #[arg(long)]
        name: String,
        #[arg(long)]
        cdp_url: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    InstanceStop {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabList {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    BrowserSurfaceList {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    BrowserSurfaceListActions {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        surface_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    BrowserSurfaceSnapshot {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        root_surface_id: Option<String>,
        #[arg(long)]
        holder_id: Option<String>,
    },
    BrowserSurfaceAction {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        surface_id: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        key_sequence: Option<String>,
        #[arg(long)]
        execution_channel: Option<String>,
        #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_allow_takeover")]
        allow_takeover: bool,
        #[arg(long = "no-allow-takeover", action = ArgAction::SetTrue, conflicts_with = "allow_takeover")]
        no_allow_takeover: bool,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabOpen {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabListActions {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabClose {
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabAction {
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        kind: String,
        #[arg(long = "ref")]
        ref_id: Option<String>,
        #[arg(long)]
        selector: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        timeout_ms: Option<u64>,
        #[arg(long)]
        expression: Option<String>,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        key: Option<String>,
        #[arg(long)]
        holder_id: Option<String>,
    },
    LeaseStatus {
        #[arg(long)]
        instance_id: Option<String>,
    },
    LeaseAcquire {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        holder_id: String,
        #[arg(long)]
        holder_label: Option<String>,
        #[arg(long, default_value = "writer")]
        mode: String,
        #[arg(long, default_value_t = 120)]
        ttl_seconds: u64,
    },
    LeaseRelease {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        holder_id: String,
        #[arg(long)]
        mode: Option<String>,
    },
    LeaseTransfer {
        #[arg(long)]
        instance_id: String,
        #[arg(long)]
        from_holder_id: String,
        #[arg(long)]
        to_holder_id: String,
        #[arg(long)]
        to_holder_label: Option<String>,
        #[arg(long, default_value_t = 120)]
        ttl_seconds: u64,
    },
    TabSnapshot {
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabText {
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    TabScreenshot {
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        holder_id: Option<String>,
        #[arg(long)]
        full_page: bool,
    },
    TabPdf {
        #[arg(long)]
        tab_id: String,
        #[arg(long)]
        holder_id: Option<String>,
    },
    ArtifactList {
        #[arg(long)]
        instance_id: Option<String>,
        #[arg(long)]
        run_id: Option<String>,
    },
    ArtifactVerify {
        #[arg(long)]
        artifact_id: String,
    },
    ArtifactCrop {
        #[arg(long)]
        artifact_id: String,
        #[arg(long)]
        x_min: u16,
        #[arg(long)]
        y_min: u16,
        #[arg(long)]
        x_max: u16,
        #[arg(long)]
        y_max: u16,
        #[arg(long)]
        page_index: Option<u32>,
        #[arg(long)]
        holder_id: Option<String>,
    },
    ArtifactCropGrid {
        #[arg(long)]
        artifact_id: String,
        #[arg(long)]
        rows: u16,
        #[arg(long)]
        cols: u16,
        #[arg(long, default_value_t = 0)]
        overlap: u16,
        #[arg(long)]
        page_index: Option<u32>,
        #[arg(long)]
        holder_id: Option<String>,
    },
    CaptureStartRecording,
    CaptureStopRecording,
    EventsTail {
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    RunList {
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    ScenarioList {
        #[arg(long)]
        family: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    ScenarioSummary {
        #[arg(long)]
        family: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    ScenarioGate {
        #[arg(long)]
        family: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: usize,
        #[arg(long = "min-runs", default_value_t = 1)]
        min_runs: usize,
        #[arg(long = "allowed-status")]
        allowed_status: Vec<String>,
        #[arg(long = "max-assertion-failures", default_value_t = 0)]
        max_assertion_failures: usize,
        #[arg(long = "min-samples-per-metric", default_value_t = 1)]
        min_samples_per_metric: usize,
        #[arg(long = "max-latest-age-minutes")]
        max_latest_age_minutes: Option<u64>,
        #[arg(long = "threshold-name")]
        threshold_name: Option<String>,
        #[arg(long = "threshold-metric")]
        threshold_metric: Option<String>,
        #[arg(long = "max-ms")]
        max_ms: Option<u64>,
        #[arg(long = "p50-ms")]
        p50_ms: Option<u64>,
        #[arg(long = "p95-ms")]
        p95_ms: Option<u64>,
        #[arg(long = "p99-ms")]
        p99_ms: Option<u64>,
    },
    ScenarioRunDetail {
        #[arg(long = "run-id")]
        run_id: String,
    },
    ScenarioRecordRun {
        #[arg(long)]
        name: String,
        #[arg(long)]
        family: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        surface: String,
    },
    ScenarioRecordStep {
        #[arg(long = "run-id")]
        run_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(long = "command-line")]
        command_line: Option<String>,
    },
    ScenarioRecordAssertion {
        #[arg(long = "run-id")]
        run_id: String,
        #[arg(long = "step-id")]
        step_id: Option<String>,
        #[arg(long)]
        name: String,
        #[arg(long)]
        expected: Option<String>,
        #[arg(long)]
        actual: Option<String>,
        #[arg(long)]
        status: String,
        #[arg(long = "failure-category")]
        failure_category: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    ScenarioRecordLatency {
        #[arg(long = "run-id")]
        run_id: String,
        #[arg(long = "step-id")]
        step_id: Option<String>,
        #[arg(long)]
        metric: String,
        #[arg(long = "sample-ms")]
        sample_ms: f64,
        #[arg(long = "capture-method", default_value = "wall_clock")]
        capture_method: String,
    },
    ScenarioFinishStep {
        #[arg(long = "step-id")]
        step_id: String,
        #[arg(long)]
        status: String,
        #[arg(long = "error-code")]
        error_code: Option<String>,
    },
    ScenarioFinishRun {
        #[arg(long = "run-id")]
        run_id: String,
        #[arg(long)]
        status: String,
        #[arg(long = "summary-path")]
        summary_path: Option<String>,
    },
    ReplayExport {
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long, default_value = "manifest_only")]
        mode: String,
    },
    TraceCapture {
        #[arg(long)]
        tab_id: String,
        #[arg(long, default_value_t = 2000)]
        duration_ms: u64,
        #[arg(long)]
        category: Vec<String>,
        #[arg(long)]
        holder_id: Option<String>,
    },
    RecordingCapture {
        #[arg(long)]
        tab_id: String,
        #[arg(long, default_value_t = 2000)]
        duration_ms: u64,
        #[arg(long, default_value_t = 250)]
        interval_ms: u64,
        #[arg(long)]
        holder_id: Option<String>,
    },
    SampleIds,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        CommandSet::Serve { bind } => {
            let runtime = StageOneRuntime::new_with_entrypoint("pengu-mesh-daemon")?;
            serve_runtime(&runtime, &bind)
        }
        CommandSet::Diagnose => print_success("diagnose report", build_diagnose_report()?),
        CommandSet::Health => with_runtime(|runtime| print_json(&runtime.browser_health()?)),
        CommandSet::CapabilityPreflight { capability } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "capability_preflight",
                json!({
                    "capability": capability,
                }),
            )
        }),
        CommandSet::HostAccessStatus => {
            with_runtime(|runtime| print_tool(runtime, "host_access_status", json!({})))
        }
        CommandSet::HostAccessSetup {
            mode,
            service,
            open_settings_on_missing,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "host_access_setup",
                json!({
                    "mode": mode,
                    "services": service,
                    "open_settings_on_missing": open_settings_on_missing,
                }),
            )
        }),
        CommandSet::ProfileList => {
            with_runtime(|runtime| print_success("managed profiles", runtime.list_profiles()?))
        }
        CommandSet::ProfileCreate { name, channel } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "profile_create",
                json!({
                    "name": name,
                    "channel": channel,
                }),
            )
        }),
        CommandSet::InstanceList => {
            with_runtime(|runtime| print_success("known instances", runtime.list_instances()?))
        }
        CommandSet::InstanceStart {
            name,
            channel,
            headless,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "instance_start",
                json!({
                    "name": name,
                    "channel": channel,
                    "headless": headless,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::InstanceAttach {
            name,
            cdp_url,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "instance_attach",
                json!({
                    "name": name,
                    "cdp_url": cdp_url,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::InstanceStop {
            instance_id,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "instance_stop",
                json!({
                    "instance_id": instance_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabList {
            instance_id,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_list",
                json!({
                    "instance_id": instance_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::BrowserSurfaceList {
            instance_id,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "browser_surface_list",
                json!({
                    "instance_id": instance_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::BrowserSurfaceListActions {
            instance_id,
            surface_id,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "browser_surface_list_actions",
                json!({
                    "instance_id": instance_id,
                    "surface_id": surface_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::BrowserSurfaceSnapshot {
            instance_id,
            root_surface_id,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "browser_surface_snapshot",
                json!({
                    "instance_id": instance_id,
                    "root_surface_id": root_surface_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::BrowserSurfaceAction {
            instance_id,
            action,
            surface_id,
            value,
            key_sequence,
            execution_channel,
            allow_takeover,
            no_allow_takeover,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "browser_surface_action",
                json!({
                    "instance_id": instance_id,
                    "action": action,
                    "surface_id": surface_id,
                    "value": value,
                    "key_sequence": key_sequence,
                    "execution_channel": execution_channel,
                    "allow_takeover": browser_surface_takeover_value(
                        allow_takeover,
                        no_allow_takeover,
                    ),
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabOpen {
            instance_id,
            url,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_open",
                json!({
                    "instance_id": instance_id,
                    "url": url,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabListActions {
            instance_id,
            tab_id,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_list_actions",
                json!({
                    "instance_id": instance_id,
                    "tab_id": tab_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabClose { tab_id, holder_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_close",
                json!({
                    "tab_id": tab_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabAction {
            tab_id,
            kind,
            ref_id,
            selector,
            url,
            timeout_ms,
            expression,
            text,
            value,
            key,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_action",
                json!({
                    "tab_id": tab_id,
                    "kind": kind,
                    "ref": ref_id,
                    "selector": selector,
                    "url": url,
                    "timeout_ms": timeout_ms,
                    "expression": expression,
                    "text": text,
                    "value": value,
                    "key": key,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::LeaseStatus { instance_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "lease_status",
                json!({
                    "instance_id": instance_id,
                }),
            )
        }),
        CommandSet::LeaseAcquire {
            instance_id,
            holder_id,
            holder_label,
            mode,
            ttl_seconds,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "lease_acquire",
                json!({
                    "instance_id": instance_id,
                    "holder_id": holder_id,
                    "holder_label": holder_label,
                    "mode": mode,
                    "ttl_seconds": ttl_seconds,
                }),
            )
        }),
        CommandSet::LeaseRelease {
            instance_id,
            holder_id,
            mode,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "lease_release",
                json!({
                    "instance_id": instance_id,
                    "holder_id": holder_id,
                    "mode": mode,
                }),
            )
        }),
        CommandSet::LeaseTransfer {
            instance_id,
            from_holder_id,
            to_holder_id,
            to_holder_label,
            ttl_seconds,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "lease_transfer",
                json!({
                    "instance_id": instance_id,
                    "from_holder_id": from_holder_id,
                    "to_holder_id": to_holder_id,
                    "to_holder_label": to_holder_label,
                    "ttl_seconds": ttl_seconds,
                }),
            )
        }),
        CommandSet::TabSnapshot { tab_id, holder_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_snapshot",
                json!({
                    "tab_id": tab_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabText { tab_id, holder_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_text",
                json!({
                    "tab_id": tab_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::TabScreenshot {
            tab_id,
            holder_id,
            full_page,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_screenshot",
                json!({
                    "tab_id": tab_id,
                    "holder_id": holder_id,
                    "full_page": full_page,
                }),
            )
        }),
        CommandSet::TabPdf { tab_id, holder_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "tab_pdf",
                json!({
                    "tab_id": tab_id,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::ArtifactList {
            instance_id,
            run_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "artifact_list",
                json!({
                    "instance_id": instance_id,
                    "run_id": run_id,
                }),
            )
        }),
        CommandSet::ArtifactVerify { artifact_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "artifact_verify",
                json!({
                    "artifact_id": artifact_id,
                }),
            )
        }),
        CommandSet::ArtifactCrop {
            artifact_id,
            x_min,
            y_min,
            x_max,
            y_max,
            page_index,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "artifact_crop",
                json!({
                    "artifact_id": artifact_id,
                    "x_min": x_min,
                    "y_min": y_min,
                    "x_max": x_max,
                    "y_max": y_max,
                    "page_index": page_index,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::ArtifactCropGrid {
            artifact_id,
            rows,
            cols,
            overlap,
            page_index,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "artifact_crop_grid",
                json!({
                    "artifact_id": artifact_id,
                    "rows": rows,
                    "cols": cols,
                    "overlap": overlap,
                    "page_index": page_index,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::CaptureStartRecording => {
            with_runtime(|runtime| print_tool(runtime, "capture_start_recording", json!({})))
        }
        CommandSet::CaptureStopRecording => {
            with_runtime(|runtime| print_tool(runtime, "capture_stop_recording", json!({})))
        }
        CommandSet::EventsTail { run_id, limit } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "events_tail",
                json!({
                    "run_id": run_id,
                    "limit": limit,
                }),
            )
        }),
        CommandSet::RunList { limit } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "run_list",
                json!({
                    "limit": limit,
                }),
            )
        }),
        CommandSet::ScenarioList { family, limit } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "scenario_list",
                json!({
                    "family": family,
                    "limit": limit,
                }),
            )
        }),
        CommandSet::ScenarioSummary { family, limit } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "scenario_summary",
                json!({
                    "family": family,
                    "limit": limit,
                }),
            )
        }),
        CommandSet::ScenarioGate {
            family,
            limit,
            min_runs,
            allowed_status,
            max_assertion_failures,
            min_samples_per_metric,
            max_latest_age_minutes,
            threshold_name,
            threshold_metric,
            max_ms,
            p50_ms,
            p95_ms,
            p99_ms,
        } => with_runtime(|runtime| {
            print_tool_with_failure_exit(
                runtime,
                "scenario_gate",
                json!({
                    "family": family,
                    "limit": limit,
                    "min_runs": min_runs,
                    "allowed_statuses": allowed_status,
                    "max_assertion_failures": max_assertion_failures,
                    "min_samples_per_metric": min_samples_per_metric,
                    "max_latest_age_minutes": max_latest_age_minutes,
                    "threshold_name": threshold_name,
                    "threshold_metric": threshold_metric,
                    "max_ms": max_ms,
                    "p50_ms": p50_ms,
                    "p95_ms": p95_ms,
                    "p99_ms": p99_ms,
                }),
            )
        }),
        CommandSet::ScenarioRunDetail { run_id } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "scenario_run_detail",
                json!({
                    "run_id": run_id,
                }),
            )
        }),
        CommandSet::ScenarioRecordRun {
            name,
            family,
            version,
            surface,
        } => print_operation_result(
            "scenario run recorded",
            "scenario_record_run",
            Some(format!(
                "name={name}, family={family}, version={version}, surface={surface}"
            )),
            scenario_record_run(&name, &family, &version, &surface)
                .map(|run| json!({ "run_id": run.id })),
        ),
        CommandSet::ScenarioRecordStep {
            run_id,
            name,
            kind,
            command_line,
        } => print_operation_result(
            "scenario step recorded",
            "scenario_record_step",
            Some(format!(
                "run_id={run_id}, name={name}, kind={kind}, command_line={}",
                command_line.as_deref().unwrap_or("")
            )),
            scenario_record_step(&run_id, &name, &kind, command_line.as_deref())
                .map(|step| json!({ "step_id": step.id })),
        ),
        CommandSet::ScenarioRecordAssertion {
            run_id,
            step_id,
            name,
            expected,
            actual,
            status,
            failure_category,
            notes,
        } => print_operation_result(
            "scenario assertion recorded",
            "scenario_record_assertion",
            Some(format!(
                "run_id={run_id}, step_id={}, name={name}, status={status}",
                step_id.as_deref().unwrap_or("")
            )),
            scenario_record_assertion(
                &run_id,
                step_id.as_deref(),
                &name,
                expected.as_deref(),
                actual.as_deref(),
                &status,
                failure_category.as_deref(),
                notes.as_deref(),
            )
            .map(|assertion| json!({ "assertion_id": assertion.id })),
        ),
        CommandSet::ScenarioRecordLatency {
            run_id,
            step_id,
            metric,
            sample_ms,
            capture_method,
        } => print_operation_result(
            "scenario latency recorded",
            "scenario_record_latency",
            Some(format!(
                "run_id={run_id}, step_id={}, metric={metric}, sample_ms={sample_ms}",
                step_id.as_deref().unwrap_or("")
            )),
            scenario_record_latency(
                &run_id,
                step_id.as_deref(),
                &metric,
                sample_ms,
                Some(&capture_method),
            )
            .map(|sample| json!({ "latency_sample_id": sample.id })),
        ),
        CommandSet::ScenarioFinishStep {
            step_id,
            status,
            error_code,
        } => print_operation_result(
            "scenario step finished",
            "scenario_finish_step",
            Some(format!(
                "step_id={step_id}, status={status}, error_code={}",
                error_code.as_deref().unwrap_or("")
            )),
            scenario_finish_step(&step_id, &status, error_code.as_deref())
                .map(|step| json!({ "step_id": step.id, "status": step.status })),
        ),
        CommandSet::ScenarioFinishRun {
            run_id,
            status,
            summary_path,
        } => print_operation_result(
            "scenario run finished",
            "scenario_finish_run",
            Some(format!(
                "run_id={run_id}, status={status}, summary_path={}",
                summary_path.as_deref().unwrap_or("")
            )),
            scenario_finish_run(&run_id, &status, summary_path.as_deref())
                .map(|run| json!({ "run_id": run.id, "status": run.status })),
        ),
        CommandSet::ReplayExport { run_id, mode } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "replay_export",
                json!({
                    "run_id": run_id,
                    "mode": mode,
                }),
            )
        }),
        CommandSet::TraceCapture {
            tab_id,
            duration_ms,
            category,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "trace_capture",
                json!({
                    "tab_id": tab_id,
                    "duration_ms": duration_ms,
                    "categories": category,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::RecordingCapture {
            tab_id,
            duration_ms,
            interval_ms,
            holder_id,
        } => with_runtime(|runtime| {
            print_tool(
                runtime,
                "recording_capture",
                json!({
                    "tab_id": tab_id,
                    "duration_ms": duration_ms,
                    "interval_ms": interval_ms,
                    "holder_id": holder_id,
                }),
            )
        }),
        CommandSet::SampleIds => {
            let ids = [
                StableId::new(IdKind::Profile, "default").into_string(),
                StableId::new(IdKind::Instance, "chrome-dev").into_string(),
                StableId::new(IdKind::Tab, "landing-page").into_string(),
                StableId::new(IdKind::Event, "capture-start").into_string(),
                StableId::new(IdKind::Run, "capture-session").into_string(),
                StableId::new(IdKind::Artifact, "screenshot").into_string(),
            ];
            print_success("sample ids", ids)
        }
    }
}

fn print_success<T: Serialize>(message: &str, data: T) -> Result<()> {
    print_json(&OperationOutcome::success(message, data))
}

fn with_runtime<F>(f: F) -> Result<()>
where
    F: FnOnce(&StageOneRuntime) -> Result<()>,
{
    let runtime = StageOneRuntime::new_with_entrypoint("pengu-mesh")?;
    f(&runtime)
}

fn print_tool(runtime: &StageOneRuntime, tool: &str, args: Value) -> Result<()> {
    let outcome = execute_tool(
        runtime,
        ToolCallRequest {
            tool: tool.to_string(),
            args,
        },
    )?;
    print_json(&outcome)
}

fn print_tool_with_failure_exit(runtime: &StageOneRuntime, tool: &str, args: Value) -> Result<()> {
    let outcome = execute_tool(
        runtime,
        ToolCallRequest {
            tool: tool.to_string(),
            args,
        },
    )?;
    let ok = outcome.ok;
    let message = outcome.message.clone();
    print_json(&outcome)?;
    if ok { Ok(()) } else { bail!("{message}") }
}

fn print_operation_result<T: Serialize>(
    message: &str,
    operation: &str,
    detail: Option<String>,
    result: Result<T>,
) -> Result<()> {
    match result {
        Ok(data) => print_json(&OperationOutcome::success(message, data)),
        Err(error) => {
            let payload = OperationOutcome::failure(
                classify_error(&error),
                error.to_string(),
                serde_json::to_value(operation_failure_payload(
                    message,
                    OperationFailureAttempt {
                        operation: operation.to_string(),
                        instance_id: None,
                        holder_id: None,
                        detail,
                    },
                    &error,
                ))?,
            );
            print_json(&payload)
        }
    }
}

fn print_json<T: Serialize>(value: &OperationOutcome<T>) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn serve_runtime(runtime: &StageOneRuntime, bind: &str) -> Result<()> {
    let listener =
        TcpListener::bind(bind).with_context(|| format!("bind http listener at {bind}"))?;
    let bind_addr = listener
        .local_addr()
        .context("read bound daemon address")?
        .to_string();
    let metadata = runtime.write_daemon_metadata(&bind_addr)?;
    print_success("daemon listening", metadata)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let response = match handle_http_request(runtime, &mut stream) {
                    Ok(response) => response,
                    Err(error) => http_error_response(&error),
                };
                let _ = write_response(&mut stream, &response);
            }
            Err(error) => eprintln!("http accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_http_request(runtime: &StageOneRuntime, stream: &mut TcpStream) -> Result<HttpResponse> {
    let request = read_request(stream)?;
    route_http(runtime, request)
}

fn route_http(runtime: &StageOneRuntime, request: HttpRequest) -> Result<HttpResponse> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => tool_http_response(runtime, "browser_health", json!({})),
        ("GET", "/doctor") => tool_http_response(runtime, "browser_doctor", json!({})),
        ("GET", "/diagnose") => tool_http_response(runtime, "diagnose", json!({})),
        ("GET", "/capabilities/preflight") => {
            tool_http_response(runtime, "capability_preflight", json_query(&request.query))
        }
        ("GET", "/host/access/status") => {
            tool_http_response(runtime, "host_access_status", json!({}))
        }
        ("POST", "/host/access/setup") => {
            tool_http_response(runtime, "host_access_setup", parse_json_body(&request)?)
        }
        ("GET", "/profiles") => tool_http_response(runtime, "profile_list", json!({})),
        ("POST", "/profiles/create") => {
            tool_http_response(runtime, "profile_create", parse_json_body(&request)?)
        }
        ("GET", "/instances") => tool_http_response(runtime, "instance_list", json!({})),
        ("POST", "/instances/start") => {
            tool_http_response(runtime, "instance_start", parse_json_body(&request)?)
        }
        ("POST", "/instances/attach") => {
            tool_http_response(runtime, "instance_attach", parse_json_body(&request)?)
        }
        ("POST", "/instances/stop") => {
            tool_http_response(runtime, "instance_stop", parse_json_body(&request)?)
        }
        ("GET", "/leases") => {
            tool_http_response(runtime, "lease_status", json_query(&request.query))
        }
        ("POST", "/leases/acquire") => {
            tool_http_response(runtime, "lease_acquire", parse_json_body(&request)?)
        }
        ("POST", "/leases/release") => {
            tool_http_response(runtime, "lease_release", parse_json_body(&request)?)
        }
        ("POST", "/leases/transfer") => {
            tool_http_response(runtime, "lease_transfer", parse_json_body(&request)?)
        }
        ("GET", "/tabs") => tool_http_response(runtime, "tab_list", json_query(&request.query)),
        ("GET", "/tabs/actions") => {
            tool_http_response(runtime, "tab_list_actions", json_query(&request.query))
        }
        ("GET", "/browser/surfaces") => {
            tool_http_response(runtime, "browser_surface_list", json_query(&request.query))
        }
        ("GET", "/browser/surfaces/actions") => tool_http_response(
            runtime,
            "browser_surface_list_actions",
            json_query(&request.query),
        ),
        ("POST", "/browser/surfaces/snapshot") => tool_http_response(
            runtime,
            "browser_surface_snapshot",
            parse_json_body(&request)?,
        ),
        ("POST", "/browser/surfaces/action") => tool_http_response(
            runtime,
            "browser_surface_action",
            parse_json_body(&request)?,
        ),
        ("POST", "/tabs/open") => {
            tool_http_response(runtime, "tab_open", parse_json_body(&request)?)
        }
        ("POST", "/tabs/close") => {
            tool_http_response(runtime, "tab_close", parse_json_body(&request)?)
        }
        ("POST", "/tabs/action") => {
            tool_http_response(runtime, "tab_action", parse_json_body(&request)?)
        }
        ("POST", "/tabs/snapshot") => {
            tool_http_response(runtime, "tab_snapshot", parse_json_body(&request)?)
        }
        ("POST", "/tabs/text") => {
            tool_http_response(runtime, "tab_text", parse_json_body(&request)?)
        }
        ("POST", "/tabs/screenshot") => {
            tool_http_response(runtime, "tab_screenshot", parse_json_body(&request)?)
        }
        ("POST", "/tabs/pdf") => tool_http_response(runtime, "tab_pdf", parse_json_body(&request)?),
        ("GET", "/artifacts") => {
            tool_http_response(runtime, "artifact_list", json_query(&request.query))
        }
        ("GET", "/artifacts/verify") => {
            tool_http_response(runtime, "artifact_verify", json_query(&request.query))
        }
        ("POST", "/artifacts/crop") => {
            tool_http_response(runtime, "artifact_crop", parse_json_body(&request)?)
        }
        ("POST", "/artifacts/crop-grid") => {
            tool_http_response(runtime, "artifact_crop_grid", parse_json_body(&request)?)
        }
        ("GET", "/runs") => tool_http_response(runtime, "run_list", json_query(&request.query)),
        ("GET", "/scenarios") => {
            tool_http_response(runtime, "scenario_list", json_query(&request.query))
        }
        ("GET", "/scenarios/summary") => {
            tool_http_response(runtime, "scenario_summary", json_query(&request.query))
        }
        ("GET", "/scenarios/gate") => {
            tool_http_response(runtime, "scenario_gate", json_query(&request.query))
        }
        ("GET", "/events") => {
            tool_http_response(runtime, "events_tail", json_query(&request.query))
        }
        ("POST", "/capture/start") => {
            tool_http_response(runtime, "capture_start_recording", json!({}))
        }
        ("POST", "/capture/stop") => {
            tool_http_response(runtime, "capture_stop_recording", json!({}))
        }
        ("POST", "/replay/export") => {
            tool_http_response(runtime, "replay_export", parse_json_body(&request)?)
        }
        ("POST", "/trace/capture") => {
            tool_http_response(runtime, "trace_capture", parse_json_body(&request)?)
        }
        ("POST", "/recording/capture") => {
            tool_http_response(runtime, "recording_capture", parse_json_body(&request)?)
        }
        ("GET", "/tools") => {
            json_http_response(&OperationOutcome::success("tool catalog", mcp_tools_list()))
        }
        _ if request.method == "POST" && request.path.starts_with("/tools/") => {
            let tool = request.path.trim_start_matches("/tools/");
            tool_http_response(runtime, tool, parse_json_body(&request)?)
        }
        _ if request.method == "GET"
            && request.path.starts_with("/artifacts/")
            && request.path != "/artifacts/verify"
            && request.path != "/artifacts/crop"
            && request.path != "/artifacts/crop-grid" =>
        {
            let artifact_id = request.path.trim_start_matches("/artifacts/");
            json_http_response(&OperationOutcome::success(
                "artifact handle",
                runtime.artifact_handle(artifact_id)?,
            ))
        }
        _ if request.method == "GET"
            && request.path.starts_with("/scenarios/")
            && request.path != "/scenarios" =>
        {
            let run_id = request.path.trim_start_matches("/scenarios/");
            tool_http_response(runtime, "scenario_run_detail", json!({ "run_id": run_id }))
        }
        _ => Ok(error_response(
            OutcomeCode::NotFound,
            format!("unknown route {} {}", request.method, request.path),
        )),
    }
}

fn tool_http_response(runtime: &StageOneRuntime, tool: &str, args: Value) -> Result<HttpResponse> {
    let outcome = execute_tool(
        runtime,
        ToolCallRequest {
            tool: tool.to_string(),
            args,
        },
    )?;
    json_http_response(&outcome)
}

fn json_http_response<T: Serialize>(outcome: &OperationOutcome<T>) -> Result<HttpResponse> {
    Ok(HttpResponse::json(serde_json::to_vec_pretty(outcome)?)
        .with_status(status_from_code(outcome.code)))
}

fn error_response(code: OutcomeCode, message: String) -> HttpResponse {
    let payload = OperationOutcome::failure(code, message, json!({}));
    HttpResponse::json(serde_json::to_vec_pretty(&payload).expect("serialize error payload"))
        .with_status(status_from_code(code))
}

fn http_error_response(error: &anyhow::Error) -> HttpResponse {
    error_response(classify_error(error), error.to_string())
}

fn parse_json_body(request: &HttpRequest) -> Result<Value> {
    if request.body.is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_slice(&request.body).context("parse json request body")
    }
}

fn json_query(query: &std::collections::BTreeMap<String, String>) -> Value {
    let mut map = Map::new();
    for (key, value) in query {
        if let Ok(number) = value.parse::<u64>() {
            map.insert(key.clone(), Value::Number(number.into()));
        } else {
            map.insert(key.clone(), Value::String(value.clone()));
        }
    }
    Value::Object(map)
}

fn status_from_code(code: OutcomeCode) -> u16 {
    match code {
        OutcomeCode::Ok => 200,
        OutcomeCode::InvalidInput | OutcomeCode::Misconfigured => 400,
        OutcomeCode::Conflict => 409,
        OutcomeCode::Unsupported => 405,
        OutcomeCode::NotFound => 404,
        OutcomeCode::NotReady => 503,
        OutcomeCode::Internal => 500,
    }
}

fn browser_surface_takeover_value(allow_takeover: bool, no_allow_takeover: bool) -> Option<bool> {
    if allow_takeover {
        Some(true)
    } else if no_allow_takeover {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Args, HttpRequest, browser_surface_takeover_value, http_error_response, route_http,
        status_from_code, tool_http_response,
    };
    use clap::Parser;
    use pengu_mesh_core::StageOneRuntime;
    use pengu_mesh_shared::{OutcomeCode, ScenarioRun};
    use pengu_mesh_state::StateStore;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
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
        std::env::temp_dir().join(format!("pengu-mesh-http-test-{nanos}-{counter}"))
    }

    #[test]
    fn not_ready_maps_to_service_unavailable() {
        assert_eq!(status_from_code(OutcomeCode::NotReady), 503);
    }

    #[test]
    fn malformed_http_json_maps_to_invalid_input() {
        let response = http_error_response(&anyhow::anyhow!("parse json request body"));
        assert_eq!(response.status, 400);
        let payload: Value = serde_json::from_slice(&response.body).expect("error payload");
        assert_eq!(payload["code"], "invalid_input");
    }

    #[test]
    fn http_surface_preserves_typed_attach_failures() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");
        let response = tool_http_response(
            &runtime,
            "instance_attach",
            json!({
                "name": "attach-demo",
                "cdp_url": "ws://127.0.0.1:9222/devtools/browser/demo"
            }),
        )
        .expect("http response");
        assert_eq!(response.status, 400);
        let payload: Value = serde_json::from_slice(&response.body).expect("attach payload");
        assert_eq!(payload["code"], "misconfigured");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_surface_wraps_missing_args_in_failure_envelope() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");
        let response =
            tool_http_response(&runtime, "profile_create", json!({})).expect("http response");
        assert_eq!(response.status, 400);
        let payload: Value = serde_json::from_slice(&response.body).expect("error payload");
        assert_eq!(payload["code"], "invalid_input");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_router_exposes_host_access_routes() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");

        let status_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/host/access/status".to_string(),
                query: BTreeMap::new(),
                body: Vec::new(),
            },
        )
        .expect("host access status route");
        assert!(
            matches!(status_response.status, 200 | 500),
            "host access status route returned unexpected status {}",
            status_response.status
        );
        let status_payload: Value =
            serde_json::from_slice(&status_response.body).expect("status payload");
        if status_response.status == 200 {
            assert_eq!(status_payload["code"], "ok");
            assert!(status_payload["data"]["services"].is_array());
        } else {
            assert_ne!(status_payload["code"], "not_found");
        }

        let setup_response = route_http(
            &runtime,
            HttpRequest {
                method: "POST".to_string(),
                path: "/host/access/setup".to_string(),
                query: BTreeMap::new(),
                body: serde_json::to_vec(&json!({"mode":"audit"})).expect("audit body"),
            },
        )
        .expect("host access setup route");
        assert!(
            matches!(setup_response.status, 200 | 500),
            "host access setup route returned unexpected status {}",
            setup_response.status
        );
        let setup_payload: Value =
            serde_json::from_slice(&setup_response.body).expect("setup payload");
        if setup_response.status == 200 {
            assert_eq!(setup_payload["code"], "ok");
            assert_eq!(setup_payload["data"]["mode"], "audit");
        } else {
            assert_ne!(setup_payload["code"], "not_found");
        }

        let invalid_setup_response = route_http(
            &runtime,
            HttpRequest {
                method: "POST".to_string(),
                path: "/host/access/setup".to_string(),
                query: BTreeMap::new(),
                body: serde_json::to_vec(&json!({"mode":"audit","services":[123]}))
                    .expect("invalid audit body"),
            },
        )
        .expect("invalid host access setup route");
        assert_eq!(invalid_setup_response.status, 400);
        let invalid_setup_payload: Value =
            serde_json::from_slice(&invalid_setup_response.body).expect("invalid setup payload");
        assert_eq!(invalid_setup_payload["code"], "invalid_input");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_router_exposes_diagnose_route() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");

        let response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/diagnose".to_string(),
                query: BTreeMap::new(),
                body: Vec::new(),
            },
        )
        .expect("diagnose route");
        assert_eq!(response.status, 200);
        let payload: Value = serde_json::from_slice(&response.body).expect("diagnose payload");
        assert_eq!(payload["code"], "ok");
        assert_eq!(payload["data"]["schema_version"], "diagnose.v1");
        assert!(payload["data"]["scenario_evidence"].is_object());
        assert!(payload["data"]["permissions"].is_array());
        assert!(payload["data"]["browser_channels"].is_array());
        assert!(payload["data"]["services"].is_array());
        assert!(payload["data"]["capabilities"].is_array());
        assert!(payload["data"]["remediations"].is_array());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_router_exposes_capability_preflight_route() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");

        let mut query = BTreeMap::new();
        query.insert(
            "capability".to_string(),
            "browser_surface_action".to_string(),
        );
        let response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/capabilities/preflight".to_string(),
                query,
                body: Vec::new(),
            },
        )
        .expect("capability preflight route");

        assert_eq!(response.status, 200);
        let payload: Value = serde_json::from_slice(&response.body).expect("preflight payload");
        assert_eq!(payload["code"], "ok");
        assert_eq!(
            payload["data"]["requested_capability"],
            "browser_surface_action"
        );
        assert_eq!(
            payload["data"]["capabilities"][0]["grant_hint"],
            "PENGU_MESH_CAPABILITY_GRANTS=browser_surface_action"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_router_exposes_browser_surface_routes_with_typed_failures() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");

        let mut query = BTreeMap::new();
        query.insert("instance_id".to_string(), "inst_missing".to_string());
        let list_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/browser/surfaces".to_string(),
                query,
                body: Vec::new(),
            },
        )
        .expect("browser surface list route");
        assert_eq!(list_response.status, 404);
        let list_payload: Value =
            serde_json::from_slice(&list_response.body).expect("list payload");
        assert_eq!(list_payload["code"], "not_found");

        let mut action_query = BTreeMap::new();
        action_query.insert("instance_id".to_string(), "inst_missing".to_string());
        action_query.insert("surface_id".to_string(), "ax:0/4".to_string());
        let actions_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/browser/surfaces/actions".to_string(),
                query: action_query,
                body: Vec::new(),
            },
        )
        .expect("browser surface list-actions route");
        assert_eq!(actions_response.status, 404);
        let actions_payload: Value =
            serde_json::from_slice(&actions_response.body).expect("actions payload");
        assert_eq!(actions_payload["code"], "not_found");
        assert_eq!(
            actions_payload["data"]["operation"],
            "browser surface action catalog"
        );
        assert_eq!(actions_payload["data"]["attempted"]["surface_id"], "ax:0/4");
        assert!(actions_payload["data"]["recovery"].is_array());

        let snapshot_response = route_http(
            &runtime,
            HttpRequest {
                method: "POST".to_string(),
                path: "/browser/surfaces/snapshot".to_string(),
                query: BTreeMap::new(),
                body: serde_json::to_vec(&json!({"instance_id":"inst_missing"}))
                    .expect("snapshot body"),
            },
        )
        .expect("browser surface snapshot route");
        assert_eq!(snapshot_response.status, 404);
        let snapshot_payload: Value =
            serde_json::from_slice(&snapshot_response.body).expect("snapshot payload");
        assert_eq!(snapshot_payload["code"], "not_found");

        let action_response = route_http(
            &runtime,
            HttpRequest {
                method: "POST".to_string(),
                path: "/browser/surfaces/action".to_string(),
                query: BTreeMap::new(),
                body: serde_json::to_vec(&json!({"instance_id":"inst_missing","action":"focus"}))
                    .expect("action body"),
            },
        )
        .expect("browser surface action route");
        assert_eq!(action_response.status, 404);
        let action_payload: Value =
            serde_json::from_slice(&action_response.body).expect("action payload");
        assert_eq!(action_payload["code"], "not_found");
        assert_eq!(
            action_payload["data"]["operation"],
            "browser surface action completed"
        );
        assert_eq!(action_payload["data"]["attempted"]["action"], "focus");
        assert!(action_payload["data"]["recovery"].is_array());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_router_exposes_artifact_verify_route() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");

        let list_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/artifacts".to_string(),
                query: BTreeMap::new(),
                body: Vec::new(),
            },
        )
        .expect("artifact list route");
        assert_eq!(list_response.status, 200);
        let list_payload: Value =
            serde_json::from_slice(&list_response.body).expect("artifact list payload");
        assert_eq!(list_payload["code"], "ok");
        assert!(list_payload["data"]["artifacts"].is_array());

        let mut missing_query = BTreeMap::new();
        missing_query.insert("artifact_id".to_string(), "artifact_missing".to_string());
        let missing_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/artifacts/verify".to_string(),
                query: missing_query,
                body: Vec::new(),
            },
        )
        .expect("missing artifact verify route");
        assert_eq!(missing_response.status, 404);
        let missing_payload: Value =
            serde_json::from_slice(&missing_response.body).expect("missing verify payload");
        assert_eq!(missing_payload["data"]["operation"], "artifact verify");
        assert_eq!(
            missing_payload["data"]["attempted"]["artifact_id"],
            "artifact_missing"
        );
        assert!(missing_payload["data"]["recovery"].is_array());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn http_router_exposes_scenario_routes() {
        let root = unique_test_root();
        let runtime =
            StageOneRuntime::new_in_root(root.clone(), "pengu-mesh-http-test").expect("runtime");
        let store = StateStore::new(root.clone()).expect("state store");
        let run = ScenarioRun {
            id: "scenario_run_http".into(),
            scenario_name: "startup-readiness".into(),
            scenario_family: "startup-readiness".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: None,
            commit_sha: Some("3e33f76".into()),
            branch_name: Some("main".into()),
            platform: "darwin".into(),
            started_at: "2026-03-12T12:00:00Z".into(),
            finished_at: Some("2026-03-12T12:00:30Z".into()),
            status: "passed".into(),
            summary_path: None,
        };
        store.insert_scenario_run(&run).expect("scenario run");

        let mut query = BTreeMap::new();
        query.insert("family".to_string(), "startup-readiness".to_string());
        let list_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/scenarios".to_string(),
                query,
                body: Vec::new(),
            },
        )
        .expect("scenario list route");
        assert_eq!(list_response.status, 200);
        let list_payload: Value =
            serde_json::from_slice(&list_response.body).expect("scenario list payload");
        assert_eq!(list_payload["code"], "ok");
        assert_eq!(list_payload["data"]["runs"][0]["id"], run.id);

        let summary_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/scenarios/summary".to_string(),
                query: BTreeMap::new(),
                body: Vec::new(),
            },
        )
        .expect("scenario summary route");
        assert_eq!(summary_response.status, 200);
        let summary_payload: Value =
            serde_json::from_slice(&summary_response.body).expect("scenario summary payload");
        assert_eq!(summary_payload["code"], "ok");
        assert_eq!(
            summary_payload["data"]["families"][0]["scenario_family"],
            "startup-readiness"
        );
        assert_eq!(summary_payload["data"]["families"][0]["runs"], 1);

        let mut gate_query = BTreeMap::new();
        gate_query.insert("max_latest_age_minutes".to_string(), "1000000".to_string());
        let gate_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: "/scenarios/gate".to_string(),
                query: gate_query,
                body: Vec::new(),
            },
        )
        .expect("scenario gate route");
        assert_eq!(gate_response.status, 200);
        let gate_payload: Value =
            serde_json::from_slice(&gate_response.body).expect("scenario gate payload");
        assert_eq!(gate_payload["code"], "ok");
        assert_eq!(gate_payload["data"]["passed"], true);
        assert_eq!(
            gate_payload["data"]["policy"]["max_latest_age_minutes"],
            1000000
        );

        let detail_response = route_http(
            &runtime,
            HttpRequest {
                method: "GET".to_string(),
                path: format!("/scenarios/{}", run.id),
                query: BTreeMap::new(),
                body: Vec::new(),
            },
        )
        .expect("scenario detail route");
        assert_eq!(detail_response.status, 200);
        let detail_payload: Value =
            serde_json::from_slice(&detail_response.body).expect("scenario detail payload");
        assert_eq!(detail_payload["code"], "ok");
        assert_eq!(
            detail_payload["data"]["run"]["scenario_family"],
            "startup-readiness"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn browser_surface_takeover_value_preserves_tristate() {
        assert_eq!(browser_surface_takeover_value(false, false), None);
        assert_eq!(browser_surface_takeover_value(true, false), Some(true));
        assert_eq!(browser_surface_takeover_value(false, true), Some(false));
    }

    #[test]
    fn capability_preflight_cli_parses_optional_capability() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "capability-preflight",
            "--capability",
            "host_access_setup",
        ])
        .expect("capability preflight parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::CapabilityPreflight { capability }
                if capability.as_deref() == Some("host_access_setup")
        ));
    }

    #[test]
    fn browser_surface_action_cli_parses_explicit_takeover_flags() {
        let omitted = Args::try_parse_from([
            "pengu-mesh",
            "browser-surface-action",
            "--instance-id",
            "inst_demo",
            "--action",
            "focus",
        ])
        .expect("omitted takeover parse");
        let allow = Args::try_parse_from([
            "pengu-mesh",
            "browser-surface-action",
            "--instance-id",
            "inst_demo",
            "--action",
            "focus",
            "--allow-takeover",
        ])
        .expect("allow takeover parse");
        let deny = Args::try_parse_from([
            "pengu-mesh",
            "browser-surface-action",
            "--instance-id",
            "inst_demo",
            "--action",
            "focus",
            "--no-allow-takeover",
        ])
        .expect("deny takeover parse");
        assert!(matches!(
            omitted.command,
            super::CommandSet::BrowserSurfaceAction {
                allow_takeover: false,
                no_allow_takeover: false,
                ..
            }
        ));
        assert!(matches!(
            allow.command,
            super::CommandSet::BrowserSurfaceAction {
                allow_takeover: true,
                no_allow_takeover: false,
                ..
            }
        ));
        assert!(matches!(
            deny.command,
            super::CommandSet::BrowserSurfaceAction {
                allow_takeover: false,
                no_allow_takeover: true,
                ..
            }
        ));
        let conflict = Args::try_parse_from([
            "pengu-mesh",
            "browser-surface-action",
            "--instance-id",
            "inst_demo",
            "--action",
            "focus",
            "--allow-takeover",
            "--no-allow-takeover",
        ]);
        assert!(conflict.is_err());
    }

    #[test]
    fn instance_start_cli_parses_headless_flag() {
        let headed = Args::try_parse_from([
            "pengu-mesh",
            "instance-start",
            "--name",
            "demo",
            "--channel",
            "chrome-dev",
        ])
        .expect("headed parse");
        let headless = Args::try_parse_from([
            "pengu-mesh",
            "instance-start",
            "--name",
            "demo",
            "--channel",
            "chrome-dev",
            "--headless",
        ])
        .expect("headless parse");
        assert!(matches!(
            headed.command,
            super::CommandSet::InstanceStart {
                headless: false,
                ..
            }
        ));
        assert!(matches!(
            headless.command,
            super::CommandSet::InstanceStart { headless: true, .. }
        ));
    }

    #[test]
    fn artifact_list_cli_parses_optional_filters() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "artifact-list",
            "--instance-id",
            "inst_demo",
            "--run-id",
            "run_demo",
        ])
        .expect("artifact list parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ArtifactList { instance_id, run_id }
                if instance_id.as_deref() == Some("inst_demo")
                    && run_id.as_deref() == Some("run_demo")
        ));
    }

    #[test]
    fn scenario_list_cli_parses_family_and_limit() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-list",
            "--family",
            "startup-readiness",
            "--limit",
            "7",
        ])
        .expect("scenario list parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioList { family, limit }
                if family.as_deref() == Some("startup-readiness") && limit == 7
        ));
    }

    #[test]
    fn scenario_summary_cli_parses_family_and_limit() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-summary",
            "--family",
            "startup-readiness",
            "--limit",
            "7",
        ])
        .expect("scenario summary parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioSummary { family, limit }
                if family.as_deref() == Some("startup-readiness") && limit == 7
        ));
    }

    #[test]
    fn scenario_gate_cli_parses_policy_thresholds() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-gate",
            "--family",
            "startup-readiness",
            "--limit",
            "7",
            "--min-runs",
            "2",
            "--allowed-status",
            "passed",
            "--max-assertion-failures",
            "0",
            "--max-latest-age-minutes",
            "30",
            "--threshold-name",
            "health-fast",
            "--threshold-metric",
            "health",
            "--max-ms",
            "1000",
            "--p50-ms",
            "500",
        ])
        .expect("scenario gate parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioGate {
                family,
                limit: 7,
                min_runs: 2,
                allowed_status,
                max_assertion_failures: 0,
                max_latest_age_minutes: Some(30),
                threshold_name,
                threshold_metric,
                max_ms: Some(1000),
                p50_ms: Some(500),
                ..
            } if family.as_deref() == Some("startup-readiness")
                && allowed_status == vec!["passed".to_string()]
                && threshold_name.as_deref() == Some("health-fast")
                && threshold_metric.as_deref() == Some("health")
        ));
    }

    #[test]
    fn scenario_run_detail_cli_parses_run_id() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-run-detail",
            "--run-id",
            "scenario_run_demo",
        ])
        .expect("scenario run detail parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioRunDetail { run_id } if run_id == "scenario_run_demo"
        ));
    }

    #[test]
    fn scenario_record_run_cli_parses_required_fields() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-record-run",
            "--name",
            "startup-readiness",
            "--family",
            "startup-readiness",
            "--version",
            "v1",
            "--surface",
            "cli",
        ])
        .expect("scenario record run parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioRecordRun {
                name,
                family,
                version,
                surface
            } if name == "startup-readiness"
                && family == "startup-readiness"
                && version == "v1"
                && surface == "cli"
        ));
    }

    #[test]
    fn scenario_record_step_cli_parses_optional_command_line() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-record-step",
            "--run-id",
            "scenario_run_demo",
            "--name",
            "health",
            "--kind",
            "command",
            "--command-line",
            "pengu-mesh health",
        ])
        .expect("scenario record step parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioRecordStep {
                run_id,
                name,
                kind,
                command_line
            } if run_id == "scenario_run_demo"
                && name == "health"
                && kind == "command"
                && command_line.as_deref() == Some("pengu-mesh health")
        ));
    }

    #[test]
    fn scenario_finish_run_cli_parses_summary_path() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "scenario-finish-run",
            "--run-id",
            "scenario_run_demo",
            "--status",
            "passed",
            "--summary-path",
            "/tmp/scenario-summary.json",
        ])
        .expect("scenario finish run parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ScenarioFinishRun {
                run_id,
                status,
                summary_path
            } if run_id == "scenario_run_demo"
                && status == "passed"
                && summary_path.as_deref() == Some("/tmp/scenario-summary.json")
        ));
    }

    #[test]
    fn artifact_verify_cli_parses_required_id() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "artifact-verify",
            "--artifact-id",
            "artifact_demo",
        ])
        .expect("artifact verify parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::ArtifactVerify { artifact_id } if artifact_id == "artifact_demo"
        ));
    }

    #[test]
    fn tab_action_cli_parses_optional_timeout_ms() {
        let parsed = Args::try_parse_from([
            "pengu-mesh",
            "tab-action",
            "--tab-id",
            "tab_demo",
            "--kind",
            "navigate",
            "--url",
            "data:text/html,after",
            "--timeout-ms",
            "25",
        ])
        .expect("tab action parse");
        assert!(matches!(
            parsed.command,
            super::CommandSet::TabAction { timeout_ms, .. } if timeout_ms == Some(25)
        ));
    }
}
