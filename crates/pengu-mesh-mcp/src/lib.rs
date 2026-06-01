use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Value, json};

use pengu_mesh_core::StageOneRuntime;
use pengu_mesh_shared::{
    ArtifactFailureAttempt, BrowserChannel, BrowserSurfaceActionRequest,
    BrowserSurfaceFailureAttempt, ExecutionChannel, HostAccessService, HostAccessSetupMode,
    HostAccessSetupRequest, LeaseMode, NormalizedRegion, OperationFailureAttempt, OperationOutcome,
    OutcomeCode, ReplayExportMode, ScenarioGatePolicy, ScenarioLatencyThreshold, SurfaceActionKind,
    TabActionKind, TabActionRequest, TabFailureAttempt, artifact_failure_payload,
    browser_surface_failure_payload, operation_failure_payload, tab_failure_payload,
};

#[derive(Debug, Clone, Serialize)]
pub struct ToolContract {
    pub name: &'static str,
    pub summary: &'static str,
    pub input_schema: Value,
    pub deferred: bool,
}

#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    pub tool: String,
    pub args: Value,
}

pub fn core_tools() -> Vec<ToolContract> {
    vec![
        tool(
            "browser_health",
            "Validate runtime readiness, browser discovery, and active instances.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "browser_doctor",
            "Capture host, browser, and permission diagnostics.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "diagnose",
            "Return a side-effect-free host readiness and remediation report.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "capability_preflight",
            "Preflight one or all built-in capabilities against the current local policy before acting.",
            json!({"type":"object","properties":{"capability":{"type":"string"}},"additionalProperties":false}),
            false,
        ),
        tool(
            "host_access_status",
            "Read host-level macOS access, permissions, and execution-channel readiness.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "host_access_setup",
            "Audit or apply host-level macOS access setup for native browser control surfaces.",
            json!({"type":"object","properties":{
                "mode":{"type":"string","enum":["audit","apply"]},
                "services":{"type":"array","items":{"type":"string","enum":["accessibility","screen_capture","listen_event","apple_events_chrome","apple_events_chrome_dev","apple_events_chromium","devtools_security"]}},
                "open_settings_on_missing":{"type":"boolean"}
            },"additionalProperties":false}),
            false,
        ),
        tool(
            "profile_list",
            "List managed browser profiles.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "profile_create",
            "Create a managed profile.",
            json!({"type":"object","properties":{"name":{"type":"string"},"channel":{"type":"string","enum":["chrome-dev","chrome","chromium"]}},"required":["name"],"additionalProperties":false}),
            false,
        ),
        tool(
            "instance_list",
            "List live and known browser instances.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "instance_start",
            "Launch a managed browser instance.",
            json!({"type":"object","properties":{"name":{"type":"string"},"channel":{"type":"string","enum":["chrome-dev","chrome","chromium"]},"headless":{"type":"boolean"},"holder_id":{"type":"string"}},"additionalProperties":false}),
            false,
        ),
        tool(
            "instance_stop",
            "Stop a managed browser instance.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"holder_id":{"type":"string"}},"required":["instance_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "instance_attach",
            "Attach to an externally managed browser endpoint.",
            json!({"type":"object","properties":{"name":{"type":"string"},"cdp_url":{"type":"string"},"holder_id":{"type":"string"}},"required":["name","cdp_url"],"additionalProperties":false}),
            false,
        ),
        tool(
            "tab_list",
            "List tabs for a live instance.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"holder_id":{"type":"string"}},"required":["instance_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "browser_surface_list",
            "List native browser surfaces discovered through macOS Accessibility.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"holder_id":{"type":"string"}},"required":["instance_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "browser_surface_list_actions",
            "Describe the available actions for a native browser surface, including required permissions and execution paths.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"surface_id":{"type":"string"},"holder_id":{"type":"string"}},"required":["instance_id","surface_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "browser_surface_snapshot",
            "Capture a native browser-surface snapshot and optional native window evidence.",
            json!({"type":"object","properties":{
                "instance_id":{"type":"string"},
                "root_surface_id":{"type":"string"},
                "holder_id":{"type":"string"}
            },"required":["instance_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "browser_surface_action",
            "Execute an action against a browser-native surface through the macOS substrate.",
            json!({"type":"object","properties":{
                "instance_id":{"type":"string"},
                "surface_id":{"type":"string"},
                "action":{"type":"string","enum":["press","focus","confirm","set_value","key_sequence"]},
                "value":{"type":"string"},
                "key_sequence":{"type":"string"},
                "execution_channel":{"type":"string","enum":["cdp","ax_direct","apple_events_activation","app_scoped_key_post","global_takeover"]},
                "allow_takeover":{"type":"boolean"},
                "holder_id":{"type":"string"}
            },"required":["instance_id","action"],"additionalProperties":false}),
            false,
        ),
        tool(
            "tab_open",
            "Open a tab in a live instance.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"url":{"type":"string"},"holder_id":{"type":"string"}},"required":["instance_id","url"],"additionalProperties":false}),
            false,
        ),
        tool(
            "tab_list_actions",
            "Describe the available tab action contracts before acting.",
            json!({
                "type":"object",
                "properties":{"instance_id":{"type":"string"},"tab_id":{"type":"string"},"holder_id":{"type":"string"}},
                "required":["instance_id","tab_id"],
                "additionalProperties":false
            }),
            false,
        ),
        tool(
            "tab_close",
            "Close a tab by stable tab id.",
            json!({"type":"object","properties":{"tab_id":{"type":"string"},"holder_id":{"type":"string"}},"required":["tab_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "tab_snapshot",
            "Capture an accessibility-oriented snapshot.",
            tab_schema(),
            false,
        ),
        tool(
            "tab_text",
            "Extract token-efficient text from a tab.",
            tab_schema(),
            false,
        ),
        tool(
            "tab_action",
            "Run a typed browser action.",
            json!({"type":"object","properties":{
                "tab_id":{"type":"string"},
                "kind":{"type":"string","enum":["navigate","evaluate","click","focus","hover","fill","type","press","select"]},
                "ref":{"type":"string"},
                "selector":{"type":"string"},
                "url":{"type":"string"},
                "timeout_ms":{"type":"integer"},
                "expression":{"type":"string"},
                "text":{"type":"string"},
                "value":{"type":"string"},
                "key":{"type":"string"},
                "holder_id":{"type":"string"}
            },"required":["tab_id","kind"],"additionalProperties":false}),
            false,
        ),
        tool(
            "tab_screenshot",
            "Capture a screenshot artifact.",
            json!({
                "type":"object",
                "properties":{"tab_id":{"type":"string"},"holder_id":{"type":"string"},"full_page":{"type":"boolean"}},
                "required":["tab_id"],
                "additionalProperties":false
            }),
            false,
        ),
        tool("tab_pdf", "Capture a PDF artifact.", tab_schema(), false),
        tool(
            "artifact_list",
            "List artifact metadata for a run and/or instance.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"}},"additionalProperties":false}),
            false,
        ),
        tool(
            "artifact_verify",
            "Recompute and compare the stored SHA-256 for an artifact.",
            json!({"type":"object","properties":{"artifact_id":{"type":"string"}},"required":["artifact_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "artifact_crop",
            "Create a derived crop artifact from a screenshot or PDF artifact.",
            json!({"type":"object","properties":{
                "artifact_id":{"type":"string"},
                "x_min":{"type":"integer","minimum":0,"maximum":999},
                "y_min":{"type":"integer","minimum":0,"maximum":999},
                "x_max":{"type":"integer","minimum":0,"maximum":999},
                "y_max":{"type":"integer","minimum":0,"maximum":999},
                "page_index":{"type":"integer","minimum":0},
                "holder_id":{"type":"string"}
            },"required":["artifact_id","x_min","y_min","x_max","y_max"],"additionalProperties":false}),
            false,
        ),
        tool(
            "artifact_crop_grid",
            "Create a deterministic batch of derived crop artifacts from a screenshot or PDF artifact.",
            json!({"type":"object","properties":{
                "artifact_id":{"type":"string"},
                "rows":{"type":"integer","minimum":1,"maximum":16},
                "cols":{"type":"integer","minimum":1,"maximum":16},
                "overlap":{"type":"integer","minimum":0,"maximum":250},
                "page_index":{"type":"integer","minimum":0},
                "holder_id":{"type":"string"}
            },"required":["artifact_id","rows","cols"],"additionalProperties":false}),
            false,
        ),
        tool(
            "capture_start_recording",
            "Begin or resume recording a run.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "capture_stop_recording",
            "Stop recording the current run.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            false,
        ),
        tool(
            "run_list",
            "List recent capture runs.",
            json!({"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":200}},"additionalProperties":false}),
            false,
        ),
        tool(
            "scenario_list",
            "List stored scenario runs, optionally filtered by family.",
            json!({"type":"object","properties":{"family":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":200}},"additionalProperties":false}),
            false,
        ),
        tool(
            "scenario_summary",
            "Summarize stored scenario evidence by family, status, assertions, and latency.",
            json!({"type":"object","properties":{"family":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":200}},"additionalProperties":false}),
            false,
        ),
        tool(
            "scenario_gate",
            "Evaluate stored scenario evidence against a release or promotion policy.",
            json!({"type":"object","properties":{
                "family":{"type":"string"},
                "limit":{"type":"integer","minimum":1,"maximum":200},
                "min_runs":{"type":"integer","minimum":0},
                "allowed_statuses":{"type":"array","items":{"type":"string"}},
                "max_assertion_failures":{"type":"integer","minimum":0},
                "min_samples_per_metric":{"type":"integer","minimum":0},
                "max_latest_age_minutes":{"type":"integer","minimum":0},
                "thresholds":{"type":"array","items":{"type":"object","properties":{
                    "name":{"type":"string"},
                    "metric":{"type":"string"},
                    "max_ms":{"type":"integer","minimum":0},
                    "p50_ms":{"type":"integer","minimum":0},
                    "p95_ms":{"type":"integer","minimum":0},
                    "p99_ms":{"type":"integer","minimum":0}
                },"required":["name","metric","max_ms"],"additionalProperties":false}},
                "threshold_name":{"type":"string"},
                "threshold_metric":{"type":"string"},
                "max_ms":{"type":"integer","minimum":0},
                "p50_ms":{"type":"integer","minimum":0},
                "p95_ms":{"type":"integer","minimum":0},
                "p99_ms":{"type":"integer","minimum":0}
            },"additionalProperties":false}),
            false,
        ),
        tool(
            "scenario_run_detail",
            "Read the stored detail for one scenario run.",
            json!({"type":"object","properties":{"run_id":{"type":"string"}},"required":["run_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "replay_export",
            "Export a replay manifest for a run.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"mode":{"type":"string","enum":["manifest_only","portable"]}},"additionalProperties":false}),
            false,
        ),
        tool(
            "trace_capture",
            "Capture a bounded Chrome trace artifact for a tab.",
            json!({"type":"object","properties":{
                "tab_id":{"type":"string"},
                "duration_ms":{"type":"integer","minimum":100,"maximum":30000},
                "categories":{"type":"array","items":{"type":"string"}},
                "holder_id":{"type":"string"}
            },"required":["tab_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "recording_capture",
            "Capture a bounded screenshot-archive recording artifact for a tab.",
            json!({"type":"object","properties":{
                "tab_id":{"type":"string"},
                "duration_ms":{"type":"integer","minimum":100,"maximum":30000},
                "interval_ms":{"type":"integer","minimum":50,"maximum":5000},
                "holder_id":{"type":"string"}
            },"required":["tab_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "events_tail",
            "Read recent orchestration events for a run.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":200}},"additionalProperties":false}),
            false,
        ),
        tool(
            "lease_status",
            "Inspect active multi-agent lease state.",
            json!({"type":"object","properties":{"instance_id":{"type":"string"}},"additionalProperties":false}),
            false,
        ),
        tool(
            "lease_acquire",
            "Acquire or renew a writer or observer lease for an instance.",
            json!({"type":"object","properties":{
                "instance_id":{"type":"string"},
                "holder_id":{"type":"string"},
                "holder_label":{"type":"string"},
                "mode":{"type":"string","enum":["writer","observer"]},
                "ttl_seconds":{"type":"integer","minimum":15,"maximum":3600}
            },"required":["instance_id","holder_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "lease_release",
            "Release one or all leases held by an operator for an instance.",
            json!({"type":"object","properties":{
                "instance_id":{"type":"string"},
                "holder_id":{"type":"string"},
                "mode":{"type":"string","enum":["writer","observer"]}
            },"required":["instance_id","holder_id"],"additionalProperties":false}),
            false,
        ),
        tool(
            "lease_transfer",
            "Transfer a writer lease from one operator to another.",
            json!({"type":"object","properties":{
                "instance_id":{"type":"string"},
                "from_holder_id":{"type":"string"},
                "to_holder_id":{"type":"string"},
                "to_holder_label":{"type":"string"},
                "ttl_seconds":{"type":"integer","minimum":15,"maximum":3600}
            },"required":["instance_id","from_holder_id","to_holder_id"],"additionalProperties":false}),
            false,
        ),
    ]
}

pub fn execute_tool(
    runtime: &StageOneRuntime,
    request: ToolCallRequest,
) -> Result<OperationOutcome<Value>> {
    let args = if request.args.is_null() {
        json!({})
    } else {
        request.args
    };
    let outcome = (|| -> Result<OperationOutcome<Value>> {
        Ok(match request.tool.as_str() {
            "browser_health" => serialize_outcome(runtime.browser_health()?),
            "browser_doctor" => operation_result(
                "browser doctor",
                operation_attempt("browser_doctor", None, None, None),
                runtime.doctor_report(),
            ),
            "diagnose" => operation_result(
                "diagnose report",
                operation_attempt("diagnose", None, None, None),
                runtime.diagnose_report(),
            ),
            "capability_preflight" => operation_result(
                "capability preflight",
                operation_attempt(
                    "capability_preflight",
                    None,
                    None,
                    optional_string(&args, "capability")
                        .map(|capability| format!("capability={capability}")),
                ),
                runtime.capability_preflight(optional_string(&args, "capability")),
            ),
            "host_access_status" => operation_result(
                "host access status",
                operation_attempt("host_access_status", None, None, None),
                runtime.host_access_status(),
            ),
            "host_access_setup" => {
                let request = parse_host_access_setup(&args)?;
                let services = request
                    .services
                    .iter()
                    .map(HostAccessService::as_str)
                    .collect::<Vec<_>>()
                    .join(",");
                operation_result(
                    "host access setup",
                    operation_attempt(
                        "host_access_setup",
                        None,
                        None,
                        Some(format!(
                            "mode={} services=[{}] open_settings_on_missing={}",
                            host_access_setup_mode_str(&request.mode),
                            services,
                            request.open_settings_on_missing
                        )),
                    ),
                    runtime.host_access_setup(request),
                )
            }
            "profile_list" => operation_result(
                "profile list",
                operation_attempt("profile_list", None, None, None),
                runtime.list_profiles(),
            ),
            "profile_create" => {
                let name = required_string(&args, "name")?;
                let channel =
                    parse_channel(optional_string(&args, "channel").unwrap_or("chrome-dev"))?;
                operation_result(
                    "profile created",
                    operation_attempt(
                        "profile_create",
                        None,
                        None,
                        Some(format!("name={name} channel={}", channel.as_str())),
                    ),
                    runtime.create_profile(name, channel),
                )
            }
            "instance_list" => operation_result(
                "instance list",
                operation_attempt("instance_list", None, None, None),
                runtime.list_instances(),
            ),
            "instance_start" => {
                let name = optional_string(&args, "name").unwrap_or("stage1");
                let channel =
                    parse_channel(optional_string(&args, "channel").unwrap_or("chrome-dev"))?;
                let channel_name = channel.as_str().to_string();
                let headless = args
                    .get("headless")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let holder_id = optional_string(&args, "holder_id").map(ToOwned::to_owned);
                operation_result(
                    "instance started",
                    operation_attempt(
                        "instance_start",
                        None,
                        holder_id.clone(),
                        Some(format!(
                            "name={name} channel={channel_name} headless={headless}"
                        )),
                    ),
                    runtime.start_instance(name, channel, headless, holder_id.as_deref()),
                )
            }
            "instance_attach" => {
                let name = required_string(&args, "name")?;
                let cdp_url = required_string(&args, "cdp_url")?;
                let holder_id = optional_string(&args, "holder_id").map(ToOwned::to_owned);
                operation_result(
                    "instance attached",
                    operation_attempt(
                        "instance_attach",
                        None,
                        holder_id.clone(),
                        Some(format!("name={name} cdp_url={cdp_url}")),
                    ),
                    runtime.attach_instance(name, cdp_url, holder_id.as_deref()),
                )
            }
            "instance_stop" => {
                let instance_id = required_string(&args, "instance_id")?.to_owned();
                let holder_id = optional_string(&args, "holder_id").map(ToOwned::to_owned);
                operation_result(
                    "instance stopped",
                    operation_attempt(
                        "instance_stop",
                        Some(instance_id.clone()),
                        holder_id.clone(),
                        None,
                    ),
                    runtime.stop_instance(&instance_id, holder_id.as_deref()),
                )
            }
            "tab_list" => {
                let instance_id = optional_string(&args, "instance_id").map(ToOwned::to_owned);
                tab_result(
                    "tab list",
                    TabFailureAttempt {
                        instance_id: instance_id.clone(),
                        tab_id: None,
                        action_kind: Some("list".to_string()),
                    },
                    (|| {
                        runtime.list_tabs(
                            instance_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("instance_id is required"))?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "tab_list_actions" => {
                let instance_id = required_string(&args, "instance_id")?.to_owned();
                let tab_id = required_string(&args, "tab_id")?.to_owned();
                tab_result(
                    "tab action catalog",
                    TabFailureAttempt {
                        instance_id: Some(instance_id.clone()),
                        tab_id: Some(tab_id.clone()),
                        action_kind: Some("list_actions".to_string()),
                    },
                    runtime.tab_list_actions(
                        &instance_id,
                        &tab_id,
                        optional_string(&args, "holder_id"),
                    ),
                )
            }
            "browser_surface_list" => {
                let instance_id = required_string(&args, "instance_id")?;
                browser_surface_result(
                    "browser surface list",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: None,
                        root_surface_id: None,
                        action: None,
                        execution_channel: None,
                        allow_takeover: None,
                    },
                    runtime.browser_surface_list(instance_id, optional_string(&args, "holder_id")),
                )
            }
            "browser_surface_list_actions" => {
                let instance_id = required_string(&args, "instance_id")?;
                let surface_id = required_string(&args, "surface_id")?;
                browser_surface_result(
                    "browser surface action catalog",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: Some(surface_id.to_string()),
                        root_surface_id: None,
                        action: None,
                        execution_channel: None,
                        allow_takeover: None,
                    },
                    runtime.browser_surface_list_actions(
                        instance_id,
                        surface_id,
                        optional_string(&args, "holder_id"),
                    ),
                )
            }
            "browser_surface_snapshot" => {
                let instance_id = required_string(&args, "instance_id")?;
                browser_surface_result(
                    "browser surface snapshot",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: None,
                        root_surface_id: optional_string(&args, "root_surface_id")
                            .map(ToOwned::to_owned),
                        action: None,
                        execution_channel: None,
                        allow_takeover: None,
                    },
                    runtime.browser_surface_snapshot(
                        instance_id,
                        optional_string(&args, "root_surface_id"),
                        optional_string(&args, "holder_id"),
                    ),
                )
            }
            "browser_surface_action" => {
                let instance_id = required_string(&args, "instance_id")?;
                let request = parse_browser_surface_action(&args)?;
                browser_surface_result(
                    "browser surface action completed",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: request.surface_id.clone(),
                        root_surface_id: None,
                        action: Some(request.action.clone()),
                        execution_channel: request.execution_channel.clone(),
                        allow_takeover: request.allow_takeover,
                    },
                    runtime.browser_surface_action(
                        instance_id,
                        request,
                        optional_string(&args, "holder_id"),
                    ),
                )
            }
            "tab_open" => {
                let instance_id = optional_string(&args, "instance_id").map(ToOwned::to_owned);
                tab_result(
                    "tab opened",
                    TabFailureAttempt {
                        instance_id: instance_id.clone(),
                        tab_id: None,
                        action_kind: Some("open".to_string()),
                    },
                    (|| {
                        runtime.open_tab(
                            instance_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("instance_id is required"))?,
                            required_string(&args, "url")?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "tab_close" => {
                let tab_id = optional_string(&args, "tab_id").map(ToOwned::to_owned);
                tab_result(
                    "tab closed",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: tab_id.clone(),
                        action_kind: Some("close".to_string()),
                    },
                    (|| {
                        runtime.close_tab(
                            tab_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("tab_id is required"))?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "tab_snapshot" => {
                let tab_id = optional_string(&args, "tab_id").map(ToOwned::to_owned);
                tab_result(
                    "tab snapshot",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: tab_id.clone(),
                        action_kind: Some("snapshot".to_string()),
                    },
                    (|| {
                        runtime.snapshot_tab(
                            tab_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("tab_id is required"))?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "tab_text" => {
                let tab_id = optional_string(&args, "tab_id").map(ToOwned::to_owned);
                tab_result(
                    "tab text",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: tab_id.clone(),
                        action_kind: Some("text".to_string()),
                    },
                    (|| {
                        runtime.text_tab(
                            tab_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("tab_id is required"))?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "tab_action" => {
                let tab_id = optional_string(&args, "tab_id").map(ToOwned::to_owned);
                let action_kind = optional_string(&args, "kind").map(ToOwned::to_owned);
                tab_result(
                    "tab action completed",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: tab_id.clone(),
                        action_kind,
                    },
                    (|| {
                        runtime.tab_action(
                            tab_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("tab_id is required"))?,
                            parse_tab_action(&args)?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "tab_screenshot" => {
                let tab_id = optional_string(&args, "tab_id").map(ToOwned::to_owned);
                tab_result(
                    "tab screenshot",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: tab_id.clone(),
                        action_kind: Some("screenshot".to_string()),
                    },
                    (|| {
                        runtime.screenshot_tab(
                            tab_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("tab_id is required"))?,
                            optional_string(&args, "holder_id"),
                            args.get("full_page")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                    })(),
                )
            }
            "tab_pdf" => {
                let tab_id = optional_string(&args, "tab_id").map(ToOwned::to_owned);
                tab_result(
                    "tab pdf",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: tab_id.clone(),
                        action_kind: Some("pdf".to_string()),
                    },
                    (|| {
                        runtime.pdf_tab(
                            tab_id
                                .as_deref()
                                .ok_or_else(|| anyhow!("tab_id is required"))?,
                            optional_string(&args, "holder_id"),
                        )
                    })(),
                )
            }
            "artifact_crop" => {
                let artifact_id = required_string(&args, "artifact_id")?.to_owned();
                artifact_result(
                    "artifact crop",
                    ArtifactFailureAttempt {
                        artifact_id: Some(artifact_id.clone()),
                        instance_id: None,
                        run_id: None,
                        action_kind: Some("crop".to_string()),
                    },
                    runtime.artifact_crop(
                        &artifact_id,
                        required_region(&args)?,
                        optional_usize(&args, "page_index").map(|value| value as u32),
                        optional_string(&args, "holder_id"),
                    ),
                )
            }
            "artifact_list" => {
                let instance_id = optional_string(&args, "instance_id").map(ToOwned::to_owned);
                let run_id = optional_string(&args, "run_id").map(ToOwned::to_owned);
                artifact_result(
                    "artifact list",
                    ArtifactFailureAttempt {
                        artifact_id: None,
                        instance_id: instance_id.clone(),
                        run_id: run_id.clone(),
                        action_kind: Some("list".to_string()),
                    },
                    runtime.artifact_list(instance_id.as_deref(), run_id.as_deref()),
                )
            }
            "artifact_verify" => {
                let artifact_id = optional_string(&args, "artifact_id").map(ToOwned::to_owned);
                artifact_result(
                    "artifact verify",
                    ArtifactFailureAttempt {
                        artifact_id: artifact_id.clone(),
                        instance_id: None,
                        run_id: None,
                        action_kind: Some("verify".to_string()),
                    },
                    runtime.artifact_verify(
                        artifact_id
                            .as_deref()
                            .ok_or_else(|| anyhow!("artifact_id is required"))?,
                    ),
                )
            }
            "artifact_crop_grid" => {
                let artifact_id = required_string(&args, "artifact_id")?.to_owned();
                artifact_result(
                    "artifact crop grid",
                    ArtifactFailureAttempt {
                        artifact_id: Some(artifact_id.clone()),
                        instance_id: None,
                        run_id: None,
                        action_kind: Some("crop_grid".to_string()),
                    },
                    runtime.artifact_crop_grid(
                        &artifact_id,
                        required_u16(&args, "rows")?,
                        required_u16(&args, "cols")?,
                        optional_usize(&args, "overlap").unwrap_or(0) as u16,
                        optional_usize(&args, "page_index").map(|value| value as u32),
                        optional_string(&args, "holder_id"),
                    ),
                )
            }
            "capture_start_recording" => operation_result(
                "capture recording active",
                operation_attempt("capture_start_recording", None, None, None),
                runtime.capture_start_recording(),
            ),
            "capture_stop_recording" => operation_result(
                "capture recording stopped",
                operation_attempt("capture_stop_recording", None, None, None),
                runtime.capture_stop_recording(),
            ),
            "run_list" => {
                let limit = optional_usize(&args, "limit").unwrap_or(25);
                operation_result(
                    "known capture runs",
                    operation_attempt("run_list", None, None, Some(format!("limit={limit}"))),
                    runtime.run_list(limit),
                )
            }
            "scenario_list" => {
                let family = optional_string(&args, "family").map(ToOwned::to_owned);
                let limit = optional_usize(&args, "limit").unwrap_or(25);
                let detail = family
                    .as_ref()
                    .map(|family| format!("family={family} limit={limit}"))
                    .unwrap_or_else(|| format!("limit={limit}"));
                operation_result(
                    "scenario runs",
                    operation_attempt("scenario_list", None, None, Some(detail)),
                    runtime.scenario_list(family.as_deref(), limit),
                )
            }
            "scenario_summary" => {
                let family = optional_string(&args, "family").map(ToOwned::to_owned);
                let limit = optional_usize(&args, "limit").unwrap_or(25);
                let detail = family
                    .as_ref()
                    .map(|family| format!("family={family} limit={limit}"))
                    .unwrap_or_else(|| format!("limit={limit}"));
                operation_result(
                    "scenario summary",
                    operation_attempt("scenario_summary", None, None, Some(detail)),
                    runtime.scenario_summary(family.as_deref(), limit),
                )
            }
            "scenario_gate" => {
                let family = optional_string(&args, "family").map(ToOwned::to_owned);
                let limit = optional_usize(&args, "limit").unwrap_or(25);
                let policy = parse_scenario_gate_policy(&args)?;
                serialize_outcome(runtime.scenario_gate(family.as_deref(), limit, policy)?)
            }
            "scenario_run_detail" => {
                let run_id = required_string(&args, "run_id")?.to_owned();
                operation_result(
                    "scenario run detail",
                    operation_attempt(
                        "scenario_run_detail",
                        None,
                        None,
                        Some(format!("run_id={run_id}")),
                    ),
                    runtime.scenario_run_detail(&run_id),
                )
            }
            "replay_export" => {
                let run_id = optional_string(&args, "run_id").map(ToOwned::to_owned);
                let mode = parse_replay_mode(optional_string(&args, "mode"))?;
                operation_result(
                    "replay manifest exported",
                    operation_attempt(
                        "replay_export",
                        None,
                        None,
                        Some(format!(
                            "run_id={} mode={}",
                            run_id.as_deref().unwrap_or("current"),
                            replay_mode_str(&mode)
                        )),
                    ),
                    runtime.replay_export(run_id.as_deref(), mode),
                )
            }
            "trace_capture" => {
                let tab_id = required_string(&args, "tab_id")?.to_owned();
                let duration_ms = optional_usize(&args, "duration_ms").unwrap_or(2_000) as u64;
                let categories = optional_string_array(&args, "categories");
                let holder_id = optional_string(&args, "holder_id").map(ToOwned::to_owned);
                tab_result(
                    "trace capture",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: Some(tab_id.clone()),
                        action_kind: Some("trace_capture".to_string()),
                    },
                    runtime.trace_capture(&tab_id, duration_ms, &categories, holder_id.as_deref()),
                )
            }
            "recording_capture" => {
                let tab_id = required_string(&args, "tab_id")?.to_owned();
                let duration_ms = optional_usize(&args, "duration_ms").unwrap_or(2_000) as u64;
                let interval_ms = optional_usize(&args, "interval_ms").unwrap_or(250) as u64;
                let holder_id = optional_string(&args, "holder_id").map(ToOwned::to_owned);
                tab_result(
                    "recording capture",
                    TabFailureAttempt {
                        instance_id: None,
                        tab_id: Some(tab_id.clone()),
                        action_kind: Some("recording_capture".to_string()),
                    },
                    runtime.recording_capture(
                        &tab_id,
                        duration_ms,
                        interval_ms,
                        holder_id.as_deref(),
                    ),
                )
            }
            "events_tail" => {
                let run_id = optional_string(&args, "run_id").map(ToOwned::to_owned);
                let limit = optional_usize(&args, "limit").unwrap_or(25);
                operation_result(
                    "capture events",
                    operation_attempt(
                        "events_tail",
                        None,
                        None,
                        Some(format!(
                            "run_id={} limit={limit}",
                            run_id.as_deref().unwrap_or("current")
                        )),
                    ),
                    runtime.events_tail(run_id.as_deref(), limit),
                )
            }
            "lease_status" => {
                let instance_id = optional_string(&args, "instance_id").map(ToOwned::to_owned);
                operation_result(
                    "active lease state",
                    operation_attempt("lease_status", instance_id.clone(), None, None),
                    runtime.lease_status(instance_id.as_deref()),
                )
            }
            "lease_acquire" => {
                let instance_id = required_string(&args, "instance_id")?.to_owned();
                let holder_id = required_string(&args, "holder_id")?.to_owned();
                let holder_label = optional_string(&args, "holder_label").map(ToOwned::to_owned);
                let mode = parse_lease_mode(optional_string(&args, "mode"))?;
                let ttl_seconds = optional_usize(&args, "ttl_seconds").unwrap_or(120) as u64;
                operation_result(
                    "lease acquired",
                    operation_attempt(
                        "lease_acquire",
                        Some(instance_id.clone()),
                        Some(holder_id.clone()),
                        Some(format!(
                            "holder_label={} mode={} ttl_seconds={ttl_seconds}",
                            holder_label.as_deref().unwrap_or("none"),
                            mode.as_str()
                        )),
                    ),
                    runtime.lease_acquire(
                        &instance_id,
                        &holder_id,
                        holder_label.as_deref(),
                        mode,
                        ttl_seconds,
                    ),
                )
            }
            "lease_release" => {
                let instance_id = required_string(&args, "instance_id")?.to_owned();
                let holder_id = required_string(&args, "holder_id")?.to_owned();
                let mode = optional_string(&args, "mode")
                    .map(|value| parse_lease_mode(Some(value)))
                    .transpose()?;
                operation_result(
                    "lease released",
                    operation_attempt(
                        "lease_release",
                        Some(instance_id.clone()),
                        Some(holder_id.clone()),
                        Some(format!(
                            "mode={}",
                            mode.as_ref().map(LeaseMode::as_str).unwrap_or("all")
                        )),
                    ),
                    runtime.lease_release(&instance_id, &holder_id, mode),
                )
            }
            "lease_transfer" => {
                let instance_id = required_string(&args, "instance_id")?.to_owned();
                let from_holder_id = required_string(&args, "from_holder_id")?.to_owned();
                let to_holder_id = required_string(&args, "to_holder_id")?.to_owned();
                let to_holder_label =
                    optional_string(&args, "to_holder_label").map(ToOwned::to_owned);
                let ttl_seconds = optional_usize(&args, "ttl_seconds").unwrap_or(120) as u64;
                operation_result(
                    "lease transferred",
                    operation_attempt(
                        "lease_transfer",
                        Some(instance_id.clone()),
                        Some(from_holder_id.clone()),
                        Some(format!(
                            "to_holder_id={to_holder_id} to_holder_label={} ttl_seconds={ttl_seconds}",
                            to_holder_label.as_deref().unwrap_or("none")
                        )),
                    ),
                    runtime.lease_transfer(
                        &instance_id,
                        &from_holder_id,
                        &to_holder_id,
                        to_holder_label.as_deref(),
                        ttl_seconds,
                    ),
                )
            }
            _ => OperationOutcome::failure(
                OutcomeCode::NotFound,
                format!("unknown tool {}", request.tool),
                json!({"supported_tools": supported_tools(), "deferred_tools": deferred_tools()}),
            ),
        })
    })();
    Ok(match outcome {
        Ok(payload) => payload,
        Err(error) => {
            OperationOutcome::failure(classify_error(&error), error.to_string(), json!({}))
        }
    })
}

pub fn mcp_tools_list() -> Value {
    json!({
        "tools": core_tools().into_iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.summary,
                "inputSchema": tool.input_schema,
            })
        }).collect::<Vec<_>>()
    })
}

pub fn supported_tools() -> Vec<&'static str> {
    core_tools()
        .into_iter()
        .filter(|tool| !tool.deferred)
        .map(|tool| tool.name)
        .collect()
}

pub fn deferred_tools() -> Vec<&'static str> {
    core_tools()
        .into_iter()
        .filter(|tool| tool.deferred)
        .map(|tool| tool.name)
        .collect()
}

fn tool(
    name: &'static str,
    summary: &'static str,
    input_schema: Value,
    deferred: bool,
) -> ToolContract {
    ToolContract {
        name,
        summary,
        input_schema,
        deferred,
    }
}

fn tab_schema() -> Value {
    json!({
        "type":"object",
        "properties":{"tab_id":{"type":"string"},"holder_id":{"type":"string"}},
        "required":["tab_id"],
        "additionalProperties":false
    })
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| anyhow!("{key} is required"))
}

fn optional_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn optional_usize(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

fn optional_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn optional_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn optional_string_array_strict(value: &Value, key: &str) -> Result<Vec<String>> {
    let Some(raw_value) = value.get(key) else {
        return Ok(Vec::new());
    };
    let items = raw_value
        .as_array()
        .ok_or_else(|| anyhow!("{key} must be an array of strings"))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| anyhow!("{key} must be an array of strings"))
        })
        .collect()
}

fn parse_scenario_gate_policy(value: &Value) -> Result<ScenarioGatePolicy> {
    let mut policy = ScenarioGatePolicy {
        min_runs: optional_usize(value, "min_runs").unwrap_or(1),
        allowed_statuses: optional_string_array_strict(value, "allowed_statuses")?,
        max_assertion_failures: optional_usize(value, "max_assertion_failures").unwrap_or(0),
        min_samples_per_metric: optional_usize(value, "min_samples_per_metric").unwrap_or(1),
        max_latest_age_minutes: optional_u64(value, "max_latest_age_minutes"),
        thresholds: Vec::new(),
    };
    if policy.allowed_statuses.is_empty() {
        policy.allowed_statuses = vec!["passed".to_string()];
    }

    if let Some(thresholds) = value.get("thresholds") {
        let thresholds = thresholds
            .as_array()
            .ok_or_else(|| anyhow!("thresholds must be an array"))?;
        for threshold in thresholds {
            policy
                .thresholds
                .push(parse_scenario_latency_threshold(threshold)?);
        }
    }

    let flat_metric =
        optional_string(value, "threshold_metric").or_else(|| optional_string(value, "metric"));
    if flat_metric.is_some() || optional_u64(value, "max_ms").is_some() {
        let metric = flat_metric.ok_or_else(|| anyhow!("threshold_metric is required"))?;
        let max_ms = optional_u64(value, "max_ms").ok_or_else(|| anyhow!("max_ms is required"))?;
        let name = optional_string(value, "threshold_name")
            .map(str::to_string)
            .unwrap_or_else(|| format!("{metric}-budget"));
        policy.thresholds.push(ScenarioLatencyThreshold {
            name,
            metric: metric.to_string(),
            max_ms,
            p50_ms: optional_u64(value, "p50_ms"),
            p95_ms: optional_u64(value, "p95_ms"),
            p99_ms: optional_u64(value, "p99_ms"),
        });
    }

    Ok(policy)
}

fn parse_scenario_latency_threshold(value: &Value) -> Result<ScenarioLatencyThreshold> {
    Ok(ScenarioLatencyThreshold {
        name: required_string(value, "name")?.to_string(),
        metric: required_string(value, "metric")?.to_string(),
        max_ms: optional_u64(value, "max_ms").ok_or_else(|| anyhow!("max_ms is required"))?,
        p50_ms: optional_u64(value, "p50_ms"),
        p95_ms: optional_u64(value, "p95_ms"),
        p99_ms: optional_u64(value, "p99_ms"),
    })
}

fn required_region(value: &Value) -> Result<NormalizedRegion> {
    Ok(NormalizedRegion {
        x_min: required_u16(value, "x_min")?,
        y_min: required_u16(value, "y_min")?,
        x_max: required_u16(value, "x_max")?,
        y_max: required_u16(value, "y_max")?,
    })
}

fn required_u16(value: &Value, key: &str) -> Result<u16> {
    value[key]
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| anyhow!("{key} is required"))
}

fn parse_replay_mode(value: Option<&str>) -> Result<ReplayExportMode> {
    match value {
        None | Some("manifest_only") => Ok(ReplayExportMode::ManifestOnly),
        Some("portable") => Ok(ReplayExportMode::Portable),
        Some(other) => bail!("unsupported replay mode {other}"),
    }
}

fn parse_channel(value: &str) -> Result<BrowserChannel> {
    match value {
        "chrome-dev" => Ok(BrowserChannel::ChromeDev),
        "chrome" => Ok(BrowserChannel::Chrome),
        "chromium" => Ok(BrowserChannel::Chromium),
        other => bail!("unsupported channel {other}"),
    }
}

fn parse_lease_mode(value: Option<&str>) -> Result<LeaseMode> {
    match value {
        None | Some("writer") => Ok(LeaseMode::Writer),
        Some("observer") => Ok(LeaseMode::Observer),
        Some(other) => bail!("unsupported lease mode {other}"),
    }
}

fn parse_host_access_setup(value: &Value) -> Result<HostAccessSetupRequest> {
    Ok(HostAccessSetupRequest {
        mode: parse_host_access_setup_mode(optional_string(value, "mode"))?,
        services: optional_string_array_strict(value, "services")?
            .into_iter()
            .map(|item| parse_host_access_service(&item))
            .collect::<Result<Vec<_>>>()?,
        open_settings_on_missing: value
            .get("open_settings_on_missing")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_host_access_setup_mode(value: Option<&str>) -> Result<HostAccessSetupMode> {
    match value {
        None | Some("audit") => Ok(HostAccessSetupMode::Audit),
        Some("apply") => Ok(HostAccessSetupMode::Apply),
        Some(other) => bail!("unsupported host access setup mode {other}"),
    }
}

fn parse_host_access_service(value: &str) -> Result<HostAccessService> {
    match value {
        "accessibility" => Ok(HostAccessService::Accessibility),
        "screen_capture" => Ok(HostAccessService::ScreenCapture),
        "listen_event" => Ok(HostAccessService::ListenEvent),
        "apple_events_chrome" => Ok(HostAccessService::AppleEventsChrome),
        "apple_events_chrome_dev" => Ok(HostAccessService::AppleEventsChromeDev),
        "apple_events_chromium" => Ok(HostAccessService::AppleEventsChromium),
        "devtools_security" => Ok(HostAccessService::DevtoolsSecurity),
        other => bail!("unsupported host access service {other}"),
    }
}

fn parse_tab_action(value: &Value) -> Result<TabActionRequest> {
    Ok(TabActionRequest {
        kind: parse_tab_action_kind(required_string(value, "kind")?)?,
        ref_id: optional_string(value, "ref").map(ToOwned::to_owned),
        selector: optional_string(value, "selector").map(ToOwned::to_owned),
        url: optional_string(value, "url").map(ToOwned::to_owned),
        timeout_ms: value.get("timeout_ms").and_then(Value::as_u64),
        expression: optional_string(value, "expression").map(ToOwned::to_owned),
        text: optional_string(value, "text").map(ToOwned::to_owned),
        value: optional_string(value, "value").map(ToOwned::to_owned),
        key: optional_string(value, "key").map(ToOwned::to_owned),
    })
}

fn parse_tab_action_kind(value: &str) -> Result<TabActionKind> {
    let kind = match value {
        "navigate" => TabActionKind::Navigate,
        "evaluate" => TabActionKind::Evaluate,
        "click" => TabActionKind::Click,
        "focus" => TabActionKind::Focus,
        "hover" => TabActionKind::Hover,
        "fill" => TabActionKind::Fill,
        "type" => TabActionKind::Type,
        "press" => TabActionKind::Press,
        "select" => TabActionKind::Select,
        _ => bail!("unsupported tab action kind {value}"),
    };
    Ok(kind)
}

fn parse_browser_surface_action(value: &Value) -> Result<BrowserSurfaceActionRequest> {
    Ok(BrowserSurfaceActionRequest {
        surface_id: optional_string(value, "surface_id").map(ToOwned::to_owned),
        action: parse_surface_action_kind(required_string(value, "action")?)?,
        value: optional_string(value, "value").map(ToOwned::to_owned),
        key_sequence: optional_string(value, "key_sequence").map(ToOwned::to_owned),
        execution_channel: optional_string(value, "execution_channel")
            .map(parse_execution_channel)
            .transpose()?,
        allow_takeover: value.get("allow_takeover").and_then(Value::as_bool),
    })
}

fn parse_surface_action_kind(value: &str) -> Result<SurfaceActionKind> {
    match value {
        "press" => Ok(SurfaceActionKind::Press),
        "focus" => Ok(SurfaceActionKind::Focus),
        "confirm" => Ok(SurfaceActionKind::Confirm),
        "set_value" => Ok(SurfaceActionKind::SetValue),
        "key_sequence" => Ok(SurfaceActionKind::KeySequence),
        other => bail!("unsupported surface action kind {other}"),
    }
}

fn parse_execution_channel(value: &str) -> Result<ExecutionChannel> {
    match value {
        "cdp" => Ok(ExecutionChannel::Cdp),
        "ax_direct" => Ok(ExecutionChannel::AxDirect),
        "apple_events_activation" => Ok(ExecutionChannel::AppleEventsActivation),
        "app_scoped_key_post" => Ok(ExecutionChannel::AppScopedKeyPost),
        "global_takeover" => Ok(ExecutionChannel::GlobalTakeover),
        other => bail!("unsupported execution channel {other}"),
    }
}

fn to_success<T: Serialize>(message: &str, data: T) -> OperationOutcome<Value> {
    OperationOutcome::success(message, serde_json::to_value(data).expect("serializable"))
}

fn serialize_outcome<T: Serialize>(outcome: OperationOutcome<T>) -> OperationOutcome<Value> {
    OperationOutcome {
        ok: outcome.ok,
        code: outcome.code,
        message: outcome.message,
        timestamp: outcome.timestamp,
        data: serde_json::to_value(outcome.data).expect("serializable"),
    }
}

fn operation_attempt(
    operation: &str,
    instance_id: Option<String>,
    holder_id: Option<String>,
    detail: Option<String>,
) -> OperationFailureAttempt {
    OperationFailureAttempt {
        operation: operation.to_string(),
        instance_id,
        holder_id,
        detail,
    }
}

fn operation_result<T: Serialize>(
    message: &str,
    attempted: OperationFailureAttempt,
    result: Result<T>,
) -> OperationOutcome<Value> {
    match result {
        Ok(data) => to_success(message, data),
        Err(error) => OperationOutcome::failure(
            classify_error(&error),
            error.to_string(),
            serde_json::to_value(operation_failure_payload(message, attempted, &error))
                .expect("serializable operation failure"),
        ),
    }
}

fn browser_surface_result<T: Serialize>(
    message: &str,
    attempted: BrowserSurfaceFailureAttempt,
    result: Result<T>,
) -> OperationOutcome<Value> {
    match result {
        Ok(data) => to_success(message, data),
        Err(error) => OperationOutcome::failure(
            classify_error(&error),
            error.to_string(),
            serde_json::to_value(browser_surface_failure_payload(message, attempted, &error))
                .expect("serializable browser surface failure"),
        ),
    }
}

fn tab_result<T: Serialize>(
    message: &str,
    attempted: TabFailureAttempt,
    result: Result<T>,
) -> OperationOutcome<Value> {
    match result {
        Ok(data) => to_success(message, data),
        Err(error) => OperationOutcome::failure(
            classify_error(&error),
            error.to_string(),
            serde_json::to_value(tab_failure_payload(message, attempted, &error))
                .expect("serializable tab failure"),
        ),
    }
}

fn artifact_result<T: Serialize>(
    message: &str,
    attempted: ArtifactFailureAttempt,
    result: Result<T>,
) -> OperationOutcome<Value> {
    match result {
        Ok(data) => to_success(message, data),
        Err(error) => OperationOutcome::failure(
            classify_error(&error),
            error.to_string(),
            serde_json::to_value(artifact_failure_payload(message, attempted, &error))
                .expect("serializable artifact failure"),
        ),
    }
}

fn host_access_setup_mode_str(mode: &HostAccessSetupMode) -> &'static str {
    match mode {
        HostAccessSetupMode::Audit => "audit",
        HostAccessSetupMode::Apply => "apply",
    }
}

fn replay_mode_str(mode: &ReplayExportMode) -> &'static str {
    match mode {
        ReplayExportMode::ManifestOnly => "manifest_only",
        ReplayExportMode::Portable => "portable",
    }
}

pub fn classify_error(error: &anyhow::Error) -> OutcomeCode {
    let message = error.to_string();
    let malformed_ref = message.contains("invalid ref ");
    let missing_ref = message.starts_with("ref e") && message.ends_with(" not found");
    if message.starts_with("unknown ") {
        OutcomeCode::NotFound
    } else if message.contains("managed profile") && message.contains("already exists") {
        OutcomeCode::Conflict
    } else if message.contains("already finished") || message.contains("still has active steps") {
        OutcomeCode::Conflict
    } else if message.contains("held by")
        || message.contains("requires writer lease")
        || message.contains("requires an explicit writer lease")
    {
        OutcomeCode::Conflict
    } else if message.contains("selector not found")
        || message.contains("surface not found")
        || missing_ref
        || message.contains("unknown tab ")
        || message.contains("unknown artifact ")
        || message.contains("unknown instance ")
    {
        OutcomeCode::NotFound
    } else if message.contains("disabled by default")
        || message.contains("capability denied")
        || message.contains("capability grant required")
        || message.contains("requires accessibility permission")
        || message.contains("missing stored sha256 metadata")
    {
        OutcomeCode::Misconfigured
    } else if message.contains("browser app not running") {
        OutcomeCode::NotReady
    } else if message.contains("parse cdp url")
        || message.contains("missing host in cdp url")
        || message.contains("missing port in cdp url")
        || malformed_ref
        || message.contains("parse json request body")
        || message.contains("must be an array of strings")
        || message.contains("does not belong to run")
    {
        OutcomeCode::InvalidInput
    } else if message.contains("timed out waiting for http://")
        || message.contains("GET http://")
        || message.contains("parse websocket URL")
        || message.contains("connect CDP websocket")
        || message.contains("reconnect tab websocket")
        || message.contains("surface action failed to focus target")
    {
        OutcomeCode::NotReady
    } else if message.contains("required")
        || message.contains(" requires ")
        || message.contains("only supports")
        || message.contains("unsupported tab action kind")
        || message.contains("unsupported surface action kind")
        || message.contains("unsupported host access setup mode")
        || message.contains("unsupported host access service")
        || message.contains("unsupported execution channel")
        || message.contains("unsupported replay mode")
        || message.contains("unsupported channel")
        || message.contains("unsupported lease mode")
        || message.contains("page_index is only valid")
        || message.contains("select requires a select target")
        || message.contains("set_value requires value")
    {
        OutcomeCode::InvalidInput
    } else if message.contains("is not installed") {
        OutcomeCode::NotReady
    } else {
        OutcomeCode::Internal
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ToolCallRequest, classify_error, execute_tool, parse_host_access_setup, supported_tools,
    };
    use pengu_mesh_core::StageOneRuntime;
    use pengu_mesh_shared::{
        BrowserChannel, BrowserInstance, BrowserTab, HostAccessService, InstanceMode,
        InstanceStatus, LatencySample, OutcomeCode, ScenarioRun,
    };
    use pengu_mesh_state::StateStore;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn exposes_capture_and_events_tools() {
        assert!(supported_tools().contains(&"profile_create"));
        assert!(supported_tools().contains(&"diagnose"));
        assert!(supported_tools().contains(&"capability_preflight"));
        assert!(supported_tools().contains(&"host_access_status"));
        assert!(supported_tools().contains(&"host_access_setup"));
        assert!(supported_tools().contains(&"browser_surface_list"));
        assert!(supported_tools().contains(&"browser_surface_list_actions"));
        assert!(supported_tools().contains(&"browser_surface_snapshot"));
        assert!(supported_tools().contains(&"browser_surface_action"));
        assert!(supported_tools().contains(&"tab_action"));
        assert!(supported_tools().contains(&"tab_list_actions"));
        assert!(supported_tools().contains(&"capture_start_recording"));
        assert!(supported_tools().contains(&"capture_stop_recording"));
        assert!(supported_tools().contains(&"events_tail"));
        assert!(supported_tools().contains(&"run_list"));
        assert!(supported_tools().contains(&"scenario_list"));
        assert!(supported_tools().contains(&"scenario_summary"));
        assert!(supported_tools().contains(&"scenario_gate"));
        assert!(supported_tools().contains(&"scenario_run_detail"));
        assert!(supported_tools().contains(&"replay_export"));
        assert!(supported_tools().contains(&"artifact_verify"));
        assert!(supported_tools().contains(&"artifact_crop"));
        assert!(supported_tools().contains(&"artifact_crop_grid"));
        assert!(supported_tools().contains(&"artifact_list"));
        assert!(supported_tools().contains(&"trace_capture"));
        assert!(supported_tools().contains(&"recording_capture"));
        assert!(supported_tools().contains(&"lease_status"));
        assert!(supported_tools().contains(&"lease_acquire"));
        assert!(supported_tools().contains(&"lease_release"));
        assert!(supported_tools().contains(&"lease_transfer"));
    }

    #[test]
    fn executes_capability_preflight_for_specific_capability() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "capability_preflight".to_string(),
                args: json!({"capability": "host_access_setup"}),
            },
        )
        .expect("preflight payload");

        assert!(payload.ok);
        assert!(!payload.data["ready"].as_bool().expect("ready bool"));
        assert_eq!(payload.data["requested_capability"], "host_access_setup");
        assert_eq!(payload.data["grant_env"], "PENGU_MESH_CAPABILITY_GRANTS");
        assert_eq!(payload.data["capabilities"].as_array().unwrap().len(), 1);
        assert_eq!(
            payload.data["capabilities"][0]["grant_hint"],
            "PENGU_MESH_CAPABILITY_GRANTS=host_access_setup"
        );
    }

    #[test]
    fn executes_events_tail_for_current_run() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "events_tail".to_string(),
                args: json!({"limit": 5}),
            },
        )
        .expect("events payload");
        assert!(payload.ok);
        assert!(payload.data["events"].is_array());
    }

    #[test]
    fn executes_scenario_list_summary_and_detail() {
        let tempdir = tempdir().expect("tempdir");
        let runtime_root = tempdir.path().to_path_buf();
        let runtime = StageOneRuntime::new_in_root(runtime_root.clone(), "pengu-mesh-mcp-test")
            .expect("runtime");
        let store = StateStore::new(runtime_root).expect("state store");
        let run = ScenarioRun {
            id: "scenario_run_startup".into(),
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
        store
            .insert_latency_sample(&LatencySample {
                id: "scenario_latency_startup_health".into(),
                run_id: run.id.clone(),
                step_id: None,
                metric_name: "health".into(),
                sample_ms: 12.0.into(),
                capture_method: Some("wall_clock".into()),
            })
            .expect("latency sample");

        let list_payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "scenario_list".to_string(),
                args: json!({"family": "startup-readiness", "limit": 5}),
            },
        )
        .expect("scenario list payload");
        assert!(list_payload.ok);
        assert_eq!(list_payload.data["requested_family"], "startup-readiness");
        assert_eq!(list_payload.data["runs"][0]["id"], run.id);

        let summary_payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "scenario_summary".to_string(),
                args: json!({"family": "startup-readiness", "limit": 5}),
            },
        )
        .expect("scenario summary payload");
        assert!(summary_payload.ok);
        assert_eq!(
            summary_payload.data["families"][0]["scenario_family"],
            "startup-readiness"
        );
        assert_eq!(
            summary_payload.data["families"][0]["statuses"][0]["status"],
            "passed"
        );

        let gate_payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "scenario_gate".to_string(),
                args: json!({
                    "family": "startup-readiness",
                    "limit": 5,
                    "max_latest_age_minutes": 1000000,
                    "threshold_metric": "health",
                    "max_ms": 20,
                    "p50_ms": 20
                }),
            },
        )
        .expect("scenario gate payload");
        assert!(gate_payload.ok);
        assert!(gate_payload.data["passed"].as_bool().expect("passed bool"));
        assert_eq!(
            gate_payload.data["policy"]["max_latest_age_minutes"],
            1000000
        );
        assert_eq!(gate_payload.data["thresholds"][0]["samples_evaluated"], 1);

        let detail_payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "scenario_run_detail".to_string(),
                args: json!({"run_id": run.id}),
            },
        )
        .expect("scenario detail payload");
        assert!(detail_payload.ok);
        assert_eq!(
            detail_payload.data["run"]["scenario_name"],
            "startup-readiness"
        );
        assert!(detail_payload.data["steps"].is_array());
    }

    #[test]
    fn executes_profile_create() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "profile_create".to_string(),
                args: json!({"name": "agent-alpha", "channel": "chrome-dev"}),
            },
        )
        .expect("profile create payload");
        assert!(payload.ok);
        assert_eq!(payload.data["name"], "agent-alpha");
        assert_eq!(payload.data["channel"], "chrome_dev");
    }

    #[test]
    fn executes_host_access_status() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "host_access_status".to_string(),
                args: json!({}),
            },
        )
        .expect("host access status payload");
        assert!(payload.ok);
        assert!(payload.data["platform"].is_string());
        assert!(payload.data["services"].is_array());
        assert!(payload.data["services"].as_array().is_some_and(|services| {
            services
                .iter()
                .any(|probe| probe["service"] == "apple_events_chromium")
        }));
        assert!(payload.data["services"].as_array().is_some_and(|services| {
            services
                .iter()
                .filter(|probe| {
                    probe["service"] == "apple_events_chrome"
                        || probe["service"] == "apple_events_chrome_dev"
                        || probe["service"] == "apple_events_chromium"
                })
                .all(|probe| {
                    probe["detail"]
                        .as_str()
                        .is_some_and(|detail| detail.contains("do not probe Automation"))
                })
        }));
    }

    #[test]
    fn diagnose_preserves_runtime_report_shape() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let expected = runtime.diagnose_report().expect("diagnose report");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "diagnose".to_string(),
                args: json!({}),
            },
        )
        .expect("diagnose payload");
        assert!(payload.ok);
        assert_eq!(payload.data["schema_version"], expected.schema_version);
        assert!(payload.data["permissions"].is_array());
        assert!(payload.data["browser_channels"].is_array());
        assert!(payload.data["services"].is_array());
        assert!(payload.data["capabilities"].is_array());
        assert!(payload.data["remediations"].is_array());
    }

    #[test]
    fn browser_surface_failures_include_actionable_payloads() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "browser_surface_list_actions".to_string(),
                args: json!({"instance_id": "inst_missing", "surface_id": "ax:0/4"}),
            },
        )
        .expect("list-actions failure payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert_eq!(payload.data["operation"], "browser surface action catalog");
        assert_eq!(payload.data["attempted"]["surface_id"], "ax:0/4");
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn tab_failures_include_actionable_payloads_for_all_existing_tab_tools() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let cases = vec![
            (
                "tab_list",
                json!({"instance_id": "inst_missing"}),
                "tab list",
            ),
            (
                "tab_open",
                json!({"instance_id": "inst_missing", "url": "data:text/plain,missing"}),
                "tab opened",
            ),
            ("tab_close", json!({"tab_id": "tab_missing"}), "tab closed"),
            (
                "tab_action",
                json!({"tab_id": "tab_missing", "kind": "navigate", "url": "data:text/plain,missing"}),
                "tab action completed",
            ),
            (
                "tab_snapshot",
                json!({"tab_id": "tab_missing"}),
                "tab snapshot",
            ),
            ("tab_text", json!({"tab_id": "tab_missing"}), "tab text"),
            (
                "tab_screenshot",
                json!({"tab_id": "tab_missing"}),
                "tab screenshot",
            ),
            ("tab_pdf", json!({"tab_id": "tab_missing"}), "tab pdf"),
            (
                "trace_capture",
                json!({"tab_id": "tab_missing", "duration_ms": 1000}),
                "trace capture",
            ),
            (
                "recording_capture",
                json!({"tab_id": "tab_missing", "duration_ms": 1000, "interval_ms": 250}),
                "recording capture",
            ),
        ];

        for (tool, args, operation) in cases {
            let payload = execute_tool(
                &runtime,
                ToolCallRequest {
                    tool: tool.to_string(),
                    args,
                },
            )
            .unwrap_or_else(|error| panic!("{tool} should return a failure envelope: {error}"));
            assert!(!payload.ok, "{tool} should fail for the missing target");
            assert_eq!(payload.data["operation"], operation, "{tool} operation");
            assert!(
                payload.data["attempted"].is_object(),
                "{tool} attempted payload"
            );
            assert!(payload.data["reason"].is_string(), "{tool} reason");
            assert!(payload.data["recovery"].is_array(), "{tool} recovery");
            assert!(
                payload.data["retry_likely"].is_boolean(),
                "{tool} retry likelihood"
            );
        }
    }

    #[test]
    fn tab_list_actions_failure_for_missing_instance_is_structured() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "tab_list_actions".to_string(),
                args: json!({"instance_id": "inst_missing", "tab_id": "tab_missing"}),
            },
        )
        .expect("missing instance payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert_eq!(payload.data["operation"], "tab action catalog");
        assert_eq!(payload.data["attempted"]["instance_id"], "inst_missing");
        assert_eq!(payload.data["attempted"]["tab_id"], "tab_missing");
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn tab_list_actions_missing_args_follow_invalid_input_path() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "tab_list_actions".to_string(),
                args: json!({}),
            },
        )
        .expect("invalid input payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::InvalidInput);
        assert_eq!(payload.message, "instance_id is required");
        assert_eq!(payload.data, json!({}));
    }

    #[test]
    fn scenario_run_detail_missing_args_follow_invalid_input_path() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "scenario_run_detail".to_string(),
                args: json!({}),
            },
        )
        .expect("invalid input payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::InvalidInput);
        assert_eq!(payload.message, "run_id is required");
        assert_eq!(payload.data, json!({}));
    }

    #[test]
    fn tab_list_actions_failure_for_missing_tab_is_structured() {
        let tempdir = tempdir().expect("tempdir");
        let runtime_root = tempdir.path().to_path_buf();
        let runtime = StageOneRuntime::new_in_root(runtime_root.clone(), "pengu-mesh-mcp-test")
            .expect("runtime");
        let store = StateStore::new(runtime_root).expect("state store");
        let instance = demo_instance("inst_tab_action_catalog");
        store.upsert_instance(&instance).expect("upsert instance");

        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "tab_list_actions".to_string(),
                args: json!({"instance_id": instance.id, "tab_id": "tab_missing"}),
            },
        )
        .expect("missing tab payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert_eq!(payload.data["operation"], "tab action catalog");
        assert_eq!(payload.data["attempted"]["instance_id"], instance.id);
        assert_eq!(payload.data["attempted"]["tab_id"], "tab_missing");
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn tab_list_actions_succeeds_for_seeded_tab() {
        let tempdir = tempdir().expect("tempdir");
        let runtime_root = tempdir.path().to_path_buf();
        let runtime = StageOneRuntime::new_in_root(runtime_root.clone(), "pengu-mesh-mcp-test")
            .expect("runtime");
        let store = StateStore::new(runtime_root).expect("state store");
        let instance = demo_instance("inst_tab_action_catalog_success");
        let tab = demo_tab(&instance.id, "tab_demo_catalog");
        store.upsert_instance(&instance).expect("upsert instance");
        store
            .replace_tabs(&instance.id, std::slice::from_ref(&tab))
            .expect("replace tabs");

        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "tab_list_actions".to_string(),
                args: json!({"instance_id": instance.id, "tab_id": tab.id}),
            },
        )
        .expect("catalog payload");
        assert!(payload.ok);
        assert_eq!(payload.data["instance"]["id"], instance.id);
        assert_eq!(payload.data["tab"]["id"], tab.id);
        assert!(payload.data["actions"].is_array());
        assert!(
            payload.data["actions"]
                .as_array()
                .is_some_and(|actions| actions.iter().any(|item| item["kind"] == "evaluate"))
        );
    }

    #[test]
    fn artifact_list_without_filters_returns_inventory() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "artifact_list".to_string(),
                args: json!({}),
            },
        )
        .expect("artifact list payload");
        assert!(payload.ok);
        assert!(payload.data["artifacts"].is_array());
        assert!(payload.data["instance_id"].is_null());
        assert!(payload.data["run_id"].is_null());
    }

    #[test]
    fn artifact_verify_failure_for_missing_artifact_is_structured() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "artifact_verify".to_string(),
                args: json!({"artifact_id": "artifact_missing"}),
            },
        )
        .expect("missing artifact payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert_eq!(payload.data["operation"], "artifact verify");
        assert_eq!(payload.data["attempted"]["artifact_id"], "artifact_missing");
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn artifact_crop_failure_for_missing_artifact_is_structured() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "artifact_crop".to_string(),
                args: json!({
                    "artifact_id": "artifact_missing",
                    "x_min": 0,
                    "y_min": 0,
                    "x_max": 100,
                    "y_max": 100,
                }),
            },
        )
        .expect("missing crop payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert_eq!(payload.data["operation"], "artifact crop");
        assert_eq!(payload.data["attempted"]["artifact_id"], "artifact_missing");
        assert_eq!(payload.data["attempted"]["action_kind"], "crop");
        assert!(payload.data["reason"].is_string());
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn artifact_crop_grid_failure_for_missing_artifact_is_structured() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "artifact_crop_grid".to_string(),
                args: json!({
                    "artifact_id": "artifact_missing",
                    "rows": 2,
                    "cols": 2,
                }),
            },
        )
        .expect("missing crop grid payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert_eq!(payload.data["operation"], "artifact crop grid");
        assert_eq!(payload.data["attempted"]["artifact_id"], "artifact_missing");
        assert_eq!(payload.data["attempted"]["action_kind"], "crop_grid");
        assert!(payload.data["reason"].is_string());
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn wraps_missing_args_in_typed_failure_envelope() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "profile_create".to_string(),
                args: json!({}),
            },
        )
        .expect("typed failure payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::InvalidInput);
        assert!(payload.message.contains("name is required"));
    }

    fn demo_instance(id: &str) -> BrowserInstance {
        BrowserInstance {
            id: id.into(),
            name: "demo".into(),
            channel: BrowserChannel::ChromeDev,
            mode: InstanceMode::Managed,
            status: InstanceStatus::Running,
            debug_http_url: "http://127.0.0.1:9222".into(),
            browser_ws_url: Some("ws://127.0.0.1:9222/devtools/browser/demo".into()),
            profile_id: None,
            profile_path: None,
            pid: Some(4321),
            last_error: None,
            created_at: "2026-03-11T12:00:00Z".into(),
            updated_at: "2026-03-11T12:00:00Z".into(),
        }
    }

    fn demo_tab(instance_id: &str, tab_id: &str) -> BrowserTab {
        BrowserTab {
            id: tab_id.into(),
            instance_id: instance_id.into(),
            target_id: "TARGET_DEMO".into(),
            title: "Demo Tab".into(),
            url: "https://example.com".into(),
            websocket_url: "ws://127.0.0.1:9222/devtools/page/demo".into(),
            active: true,
            created_at: "2026-03-11T12:00:00Z".into(),
            updated_at: "2026-03-11T12:00:00Z".into(),
        }
    }

    #[test]
    fn wraps_runtime_lookup_failures_in_typed_failure_envelope() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "tab_snapshot".to_string(),
                args: json!({"tab_id": "tab_missing"}),
            },
        )
        .expect("typed not found payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::NotFound);
        assert!(payload.message.contains("unknown tab"));
    }

    #[test]
    fn rejects_unsupported_enum_inputs_instead_of_coercing() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");

        let invalid_channel = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "profile_create".to_string(),
                args: json!({"name": "agent-alpha", "channel": "safari"}),
            },
        )
        .expect("invalid channel payload");
        assert!(!invalid_channel.ok);
        assert_eq!(invalid_channel.code, OutcomeCode::InvalidInput);

        let invalid_replay_mode = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "replay_export".to_string(),
                args: json!({"mode": "zip"}),
            },
        )
        .expect("invalid replay payload");
        assert!(!invalid_replay_mode.ok);
        assert_eq!(invalid_replay_mode.code, OutcomeCode::InvalidInput);

        let invalid_lease_mode = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "lease_acquire".to_string(),
                args: json!({
                    "instance_id": "inst_demo",
                    "holder_id": "holder_demo",
                    "mode": "watcher"
                }),
            },
        )
        .expect("invalid lease mode payload");
        assert!(!invalid_lease_mode.ok);
        assert_eq!(invalid_lease_mode.code, OutcomeCode::InvalidInput);

        let invalid_host_access_mode = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "host_access_setup".to_string(),
                args: json!({"mode": "mutate"}),
            },
        )
        .expect("invalid host access mode payload");
        assert!(!invalid_host_access_mode.ok);
        assert_eq!(invalid_host_access_mode.code, OutcomeCode::InvalidInput);

        let invalid_host_access_services = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "host_access_setup".to_string(),
                args: json!({"mode": "audit", "services": [123]}),
            },
        )
        .expect("invalid host access services payload");
        assert!(!invalid_host_access_services.ok);
        assert_eq!(invalid_host_access_services.code, OutcomeCode::InvalidInput);

        let invalid_surface_channel = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "browser_surface_action".to_string(),
                args: json!({
                    "instance_id": "inst_demo",
                    "action": "press",
                    "execution_channel": "window_server"
                }),
            },
        )
        .expect("invalid surface channel payload");
        assert!(!invalid_surface_channel.ok);
        assert_eq!(invalid_surface_channel.code, OutcomeCode::InvalidInput);
    }

    #[test]
    fn parse_host_access_setup_rejects_non_string_services() {
        let mixed = parse_host_access_setup(&json!({
            "mode": "apply",
            "services": ["accessibility", 7]
        }))
        .expect_err("mixed services should be rejected");
        assert!(
            mixed
                .to_string()
                .contains("services must be an array of strings")
        );

        let all_non_string = parse_host_access_setup(&json!({
            "mode": "apply",
            "services": [7]
        }))
        .expect_err("non-string services should be rejected");
        assert!(
            all_non_string
                .to_string()
                .contains("services must be an array of strings")
        );
    }

    #[test]
    fn parse_host_access_setup_preserves_empty_and_chromium_services() {
        let empty = parse_host_access_setup(&json!({
            "mode": "audit",
            "services": []
        }))
        .expect("empty services payload");
        assert!(empty.services.is_empty());

        let chromium = parse_host_access_setup(&json!({
            "mode": "audit",
            "services": ["apple_events_chromium"]
        }))
        .expect("chromium services payload");
        assert_eq!(
            chromium.services,
            vec![HostAccessService::AppleEventsChromium]
        );
    }

    #[test]
    fn execute_tool_rejects_invalid_host_access_services_payload() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let invalid_services = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "host_access_setup".to_string(),
                args: json!({"mode": "audit", "services": [123]}),
            },
        )
        .expect("invalid services payload");
        assert!(!invalid_services.ok);
        assert_eq!(invalid_services.code, OutcomeCode::InvalidInput);
    }

    #[test]
    fn executes_replay_export_for_current_run() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "replay_export".to_string(),
                args: json!({}),
            },
        )
        .expect("replay export payload");
        assert!(payload.ok);
        assert!(payload.data["manifest_path"].is_string());
        assert_eq!(payload.data["mode"], "manifest_only");
    }

    #[test]
    fn classifies_lease_conflicts_as_conflict_outcomes() {
        let error = anyhow::anyhow!(
            "writer lease for inst_demo is held by agent_alpha until 2026-03-12T00:00:00Z"
        );
        assert_eq!(classify_error(&error), OutcomeCode::Conflict);
    }

    #[test]
    fn classifies_duplicate_profiles_as_conflicts() {
        let error = anyhow::anyhow!("managed profile prof_demo already exists");
        assert_eq!(classify_error(&error), OutcomeCode::Conflict);
    }

    #[test]
    fn classifies_tab_lookup_errors_as_not_found_or_invalid_input() {
        let selector_error = anyhow::anyhow!("selector not found: #missing");
        assert_eq!(classify_error(&selector_error), OutcomeCode::NotFound);

        let malformed_ref_error = anyhow::anyhow!("invalid ref node-42");
        assert_eq!(
            classify_error(&malformed_ref_error),
            OutcomeCode::InvalidInput
        );

        let missing_ref_error = anyhow::anyhow!("ref e99 not found");
        assert_eq!(classify_error(&missing_ref_error), OutcomeCode::NotFound);

        let click_error = anyhow::anyhow!("click requires ref or selector");
        assert_eq!(classify_error(&click_error), OutcomeCode::InvalidInput);

        let fill_error = anyhow::anyhow!("fill requires text");
        assert_eq!(classify_error(&fill_error), OutcomeCode::InvalidInput);

        let select_error = anyhow::anyhow!("select requires a select target");
        assert_eq!(classify_error(&select_error), OutcomeCode::InvalidInput);

        let surface_kind_error = anyhow::anyhow!("unsupported surface action kind toggle");
        assert_eq!(
            classify_error(&surface_kind_error),
            OutcomeCode::InvalidInput
        );

        let surface_target_error = anyhow::anyhow!("surface not found: ax:0/4");
        assert_eq!(classify_error(&surface_target_error), OutcomeCode::NotFound);
    }

    #[test]
    fn classifies_attach_input_and_readiness_failures() {
        let malformed_url = anyhow::anyhow!("parse cdp url");
        assert_eq!(classify_error(&malformed_url), OutcomeCode::InvalidInput);

        let attach_timeout =
            anyhow::anyhow!("timed out waiting for http://127.0.0.1:9222/json/version: refused");
        assert_eq!(classify_error(&attach_timeout), OutcomeCode::NotReady);

        let list_failure = anyhow::anyhow!("GET http://127.0.0.1:9222/json/list");
        assert_eq!(classify_error(&list_failure), OutcomeCode::NotReady);

        let websocket_failure = anyhow::anyhow!("connect CDP websocket");
        assert_eq!(classify_error(&websocket_failure), OutcomeCode::NotReady);

        let browser_not_running = anyhow::anyhow!("browser app not running: Google Chrome Dev");
        assert_eq!(classify_error(&browser_not_running), OutcomeCode::NotReady);

        let missing_accessibility = anyhow::anyhow!("requires accessibility permission");
        assert_eq!(
            classify_error(&missing_accessibility),
            OutcomeCode::Misconfigured
        );

        let capability_grant = anyhow::anyhow!("capability grant required: host_access_setup");
        assert_eq!(
            classify_error(&capability_grant),
            OutcomeCode::Misconfigured
        );

        let focus_failure = anyhow::anyhow!("surface action failed to focus target: ax:0/4");
        assert_eq!(classify_error(&focus_failure), OutcomeCode::NotReady);
    }

    #[test]
    fn instance_attach_returns_typed_failure_envelope_when_attach_is_disabled() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "instance_attach".to_string(),
                args: json!({
                    "name": "attach-demo",
                    "cdp_url": "ws://127.0.0.1:9222/devtools/browser/demo"
                }),
            },
        )
        .expect("attach payload");
        assert!(!payload.ok);
        assert_eq!(payload.code, OutcomeCode::Misconfigured);
        assert_eq!(payload.data["operation"], "instance attached");
        assert_eq!(payload.data["attempted"]["operation"], "instance_attach");
        assert_eq!(
            payload.data["attempted"]["detail"],
            "name=attach-demo cdp_url=ws://127.0.0.1:9222/devtools/browser/demo"
        );
        assert!(payload.data["recovery"].is_array());
        assert!(payload.data["retry_likely"].is_boolean());
    }

    #[test]
    fn operation_failures_include_actionable_payloads_for_remaining_operation_tools() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");

        let first_profile = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "profile_create".to_string(),
                args: json!({"name": "agent-alpha", "channel": "chrome-dev"}),
            },
        )
        .expect("first profile create payload");
        assert!(first_profile.ok);

        let cases = vec![
            (
                "profile_create",
                json!({"name": "agent-alpha", "channel": "chrome-dev"}),
                OutcomeCode::Conflict,
                "profile created",
                "profile_create",
            ),
            (
                "instance_stop",
                json!({"instance_id": "inst_missing", "holder_id": "agent_alpha"}),
                OutcomeCode::NotFound,
                "instance stopped",
                "instance_stop",
            ),
            (
                "lease_release",
                json!({"instance_id": "inst_missing", "holder_id": "agent_alpha", "mode": "writer"}),
                OutcomeCode::NotFound,
                "lease released",
                "lease_release",
            ),
            (
                "events_tail",
                json!({"run_id": "run_missing", "limit": 10}),
                OutcomeCode::NotFound,
                "capture events",
                "events_tail",
            ),
            (
                "scenario_run_detail",
                json!({"run_id": "scenario_run_missing"}),
                OutcomeCode::NotFound,
                "scenario run detail",
                "scenario_run_detail",
            ),
            (
                "replay_export",
                json!({"run_id": "run_missing", "mode": "manifest_only"}),
                OutcomeCode::NotFound,
                "replay manifest exported",
                "replay_export",
            ),
        ];

        for (tool, args, code, operation, attempted_operation) in cases {
            let payload = execute_tool(
                &runtime,
                ToolCallRequest {
                    tool: tool.to_string(),
                    args,
                },
            )
            .unwrap_or_else(|error| panic!("{tool} should return a failure envelope: {error}"));
            assert!(!payload.ok, "{tool} should fail for this probe");
            assert_eq!(payload.code, code, "{tool} outcome code");
            assert_eq!(payload.data["operation"], operation, "{tool} operation");
            assert_eq!(
                payload.data["attempted"]["operation"], attempted_operation,
                "{tool} attempted operation"
            );
            assert!(payload.data["reason"].is_string(), "{tool} reason");
            assert!(payload.data["recovery"].is_array(), "{tool} recovery");
            assert!(
                payload.data["retry_likely"].is_boolean(),
                "{tool} retry likelihood"
            );
        }
    }

    #[test]
    fn browser_health_preserves_runtime_outcome_envelope() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let expected = runtime.browser_health().expect("runtime browser health");
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: "browser_health".to_string(),
                args: json!({}),
            },
        )
        .expect("browser health payload");
        assert_eq!(payload.ok, expected.ok);
        assert_eq!(payload.code, expected.code);
        assert_eq!(payload.message, expected.message);
    }

    #[test]
    fn host_access_setup_parser_rejects_non_string_service_entries() {
        let mixed = parse_host_access_setup(&json!({
            "mode": "audit",
            "services": ["accessibility", 7]
        }))
        .expect_err("mixed services should fail");
        assert!(
            mixed
                .to_string()
                .contains("services must be an array of strings")
        );

        let wrong_type = parse_host_access_setup(&json!({
            "mode": "audit",
            "services": "accessibility"
        }))
        .expect_err("non-array services should fail");
        assert!(
            wrong_type
                .to_string()
                .contains("services must be an array of strings")
        );
    }

    #[test]
    fn host_access_setup_parser_accepts_empty_and_chromium_services() {
        let omitted = parse_host_access_setup(&json!({"mode": "audit"})).expect("omitted services");
        assert!(omitted.services.is_empty());

        let empty = parse_host_access_setup(&json!({"mode": "audit", "services": []}))
            .expect("empty services");
        assert!(empty.services.is_empty());

        let chromium = parse_host_access_setup(&json!({
            "mode": "audit",
            "services": ["apple_events_chromium"]
        }))
        .expect("chromium service");
        assert_eq!(
            chromium.services,
            vec![HostAccessService::AppleEventsChromium]
        );
    }
}
