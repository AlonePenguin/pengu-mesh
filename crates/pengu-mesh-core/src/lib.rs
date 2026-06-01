use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

use pengu_mesh_artifacts::{ArtifactStore, BatchCropArtifact, baseline_artifacts};
use pengu_mesh_cdp::{
    CdpSession, DebugTarget, NavigationEvidence, VersionMetadata, activate_tab, close_tab,
    debug_http_url, discover_installations, find_installation, launch_managed_browser,
    list_targets, open_tab, reserve_debug_port, wait_for_debug_endpoint,
};
use pengu_mesh_http::{RouteSurface, bootstrap_routes};
use pengu_mesh_macos::{
    browser_surface_action as macos_browser_surface_action,
    browser_surface_list as macos_browser_surface_list,
    browser_surface_snapshot as macos_browser_surface_snapshot, capture_bytes_from_snapshot,
    cleanup_snapshot_capture, host_access_setup as macos_host_access_setup,
    host_access_status as macos_host_access_status,
};
use pengu_mesh_shared::{
    ArtifactHandle, ArtifactKind, ArtifactListEntry, ArtifactListPayload, ArtifactVerifyPayload,
    AttachContinuityFreshness, AttachContinuityOutcome, AttachContinuityStatus,
    AttachResolutionKind, BrowserChannel, BrowserInstall, BrowserInstance,
    BrowserSurfaceActionCatalogPayload, BrowserSurfaceActionContract,
    BrowserSurfaceActionPathContract, BrowserSurfaceActionPayload, BrowserSurfaceActionRequest,
    BrowserSurfaceDescriptor, BrowserSurfaceFailureAttempt, BrowserSurfaceListPayload,
    BrowserSurfaceSnapshot, BrowserTab, CapabilityDecision, CapabilityGatePolicy,
    CapabilityRiskTier, CaptureRun, DaemonMetadata, DiagnoseBrowserChannel, DiagnoseCapability,
    DiagnosePermission, DiagnoseRemediation, DiagnoseReport, DiagnoseService, DiagnoseServiceState,
    DiagnoseState, EmptyPayload, EventLevel, EventTailPayload, ExecutionChannel, HostAccessProbe,
    HostAccessService, HostAccessSetupMode, HostAccessSetupRequest, HostAccessSetupResult,
    HostAccessStatus, IdKind, InstanceMode, InstanceStatus, LeaseAcquirePayload,
    LeaseCoverageEntry, LeaseDisposition, LeaseMode, LeaseRecord, LeaseReleasePayload,
    LeaseResourceKind, LeaseStatusPayload, LeaseTransferPayload, ManagedProfile, NormalizedRegion,
    OperationOutcome, OutcomeCode, PermissionState, ReplayArtifactRecord, ReplayBundleMetadata,
    ReplayExportMode, ReplayManifest, ReplayManifestExport, RunListPayload, RunStatus,
    RuntimeEvent, RuntimePaths, RuntimePosture, ScenarioListPayload, ScenarioRunDetailPayload,
    StableId, SurfaceActionKind, TabActionCatalogPayload, TabActionContract, TabActionKind,
    TabActionPayload, TabActionRequest, browser_surface_failure_payload, default_capabilities,
    inspection_modes, utc_timestamp,
};
use pengu_mesh_state::{StatePlan, StateStore};

mod allocation;
mod autorestart;
mod scenario_recorder;
mod semantic;
mod task_queue;
mod threshold;
#[allow(dead_code)]
mod webhook;

pub use allocation::*;
pub use autorestart::{RestartConfig, RestartDecision, RestartTracker};
pub use scenario_recorder::{
    ScenarioRecorder, StepRecorder, scenario_finish_run, scenario_finish_step,
    scenario_record_assertion, scenario_record_latency, scenario_record_run, scenario_record_step,
};
pub use semantic::LexicalMatcher;
pub use task_queue::{QueueError, QueuedTask, TaskQueue, TaskQueueConfig};
pub use threshold::{
    PerformanceThreshold, ThresholdResult, ThresholdViolation, evaluate_threshold,
    evaluate_thresholds,
};

const DEFAULT_LEASE_TTL_SECONDS: u64 = 300;
const DEFAULT_HTTP_BIND_ADDR: &str = "127.0.0.1:43127";
pub const CAPABILITY_GRANTS_ENV: &str = "PENGU_MESH_CAPABILITY_GRANTS";

#[derive(Debug, Clone, Serialize)]
pub struct ContinuityStatus {
    pub continuity_enabled: bool,
    pub recovered_run: bool,
    pub reused_operator_id: bool,
    pub recovered_run_id: Option<String>,
    pub recovered_lease_count: usize,
    pub recovered_instance_count: usize,
    pub stale_instance_count: usize,
    pub stale_instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthPayload {
    pub posture: RuntimePosture,
    pub paths: RuntimePaths,
    pub operator_id: String,
    pub daemon: Option<DaemonMetadata>,
    pub continuity: ContinuityStatus,
    pub attach_continuity: AttachContinuityStatus,
    pub state: StatePlan,
    pub capture_run: CaptureRun,
    pub inspection_modes: Vec<pengu_mesh_shared::InspectionModeContract>,
    pub capability_posture: CapabilityPosture,
    pub routes: Vec<RouteSurface>,
    pub lease_coverage: Vec<LeaseCoverageEntry>,
    pub host_access: HostAccessStatus,
    pub artifacts: Vec<pengu_mesh_artifacts::ArtifactDescriptor>,
    pub installations: Vec<BrowserInstall>,
    pub profiles: Vec<ManagedProfile>,
    pub instances: Vec<BrowserInstance>,
    pub leases: Vec<LeaseRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityPosture {
    pub policy: CapabilityGatePolicy,
    pub total: usize,
    pub safe: usize,
    pub elevated: usize,
    pub dangerous: usize,
    pub allowed: usize,
    pub denied: usize,
    pub requires_grant: usize,
    pub capabilities: Vec<CapabilityPostureEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityPostureEntry {
    pub name: String,
    pub risk_tier: CapabilityRiskTier,
    pub description: String,
    pub requires_explicit_grant: bool,
    pub decision: CapabilityDecision,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityPreflightPayload {
    pub policy: CapabilityGatePolicy,
    pub requested_capability: Option<String>,
    pub ready: bool,
    pub grant_env: &'static str,
    pub capabilities: Vec<CapabilityPreflightEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityPreflightEntry {
    pub name: String,
    pub risk_tier: CapabilityRiskTier,
    pub description: String,
    pub requires_explicit_grant: bool,
    pub decision: CapabilityDecision,
    pub allowed: bool,
    pub grant_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotPayload {
    pub tab: BrowserTab,
    pub artifact: ArtifactHandle,
    pub snapshot: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextPayload {
    pub tab: BrowserTab,
    pub artifact: ArtifactHandle,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactPayload {
    pub tab: BrowserTab,
    pub artifact: ArtifactHandle,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactCropPayload {
    pub source_artifact: ArtifactHandle,
    pub artifact: ArtifactHandle,
    pub crop_region: NormalizedRegion,
    pub page_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactCropGridPayload {
    pub source_artifact: ArtifactHandle,
    pub artifacts: Vec<BatchCropArtifact>,
    pub rows: u16,
    pub cols: u16,
    pub overlap: u16,
    pub page_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceCapturePayload {
    pub tab: BrowserTab,
    pub artifact: ArtifactHandle,
    pub duration_ms: u64,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingCapturePayload {
    pub tab: BrowserTab,
    pub artifact: ArtifactHandle,
    pub duration_ms: u64,
    pub interval_ms: u64,
    pub frame_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredLease {
    Writer,
    Observer,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorToolStatus {
    pub name: &'static str,
    pub found: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorPermissionStatus {
    pub sudo_nopasswd: bool,
    pub devtools_security: String,
    pub external_attach_enabled: bool,
    pub host_access: HostAccessStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub posture: RuntimePosture,
    pub paths: RuntimePaths,
    pub operator_id: String,
    pub daemon: Option<DaemonMetadata>,
    pub continuity: ContinuityStatus,
    pub attach_continuity: AttachContinuityStatus,
    pub capture_run: CaptureRun,
    pub inspection_modes: Vec<pengu_mesh_shared::InspectionModeContract>,
    pub capability_posture: CapabilityPosture,
    pub tools: Vec<DoctorToolStatus>,
    pub lease_coverage: Vec<LeaseCoverageEntry>,
    pub browser_installs: Vec<BrowserInstall>,
    pub permissions: DoctorPermissionStatus,
    pub instances: Vec<BrowserInstance>,
    pub leases: Vec<LeaseRecord>,
    pub replay_validations: Vec<ReplayValidationStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayValidationStatus {
    pub run_id: String,
    pub mode: ReplayExportMode,
    pub manifest_path: String,
    pub artifact_count: usize,
    pub missing_files: usize,
    pub checksum_mismatches: usize,
    pub provenance_errors: usize,
    pub ok: bool,
}

pub fn capability_policy_from_env() -> CapabilityGatePolicy {
    let mut policy = CapabilityGatePolicy::default();
    policy.explicit_grants = env::var(CAPABILITY_GRANTS_ENV)
        .map(|value| parse_capability_grants(&value))
        .unwrap_or_default();
    policy
}

pub fn capability_posture(policy: CapabilityGatePolicy) -> CapabilityPosture {
    let capabilities = default_capabilities();
    let mut safe = 0;
    let mut elevated = 0;
    let mut dangerous = 0;
    let mut allowed = 0;
    let mut denied = 0;
    let mut requires_grant = 0;

    let capabilities = capabilities
        .into_iter()
        .map(|capability| {
            match capability.risk_tier {
                CapabilityRiskTier::Safe => safe += 1,
                CapabilityRiskTier::Elevated => elevated += 1,
                CapabilityRiskTier::Dangerous => dangerous += 1,
            }

            let decision = policy.evaluate(&capability);
            match &decision {
                CapabilityDecision::Allowed => allowed += 1,
                CapabilityDecision::Denied { .. } => denied += 1,
                CapabilityDecision::RequiresGrant { .. } => requires_grant += 1,
            }

            CapabilityPostureEntry {
                name: capability.name,
                risk_tier: capability.risk_tier,
                description: capability.description,
                requires_explicit_grant: capability.requires_explicit_grant,
                decision,
            }
        })
        .collect::<Vec<_>>();

    CapabilityPosture {
        policy,
        total: capabilities.len(),
        safe,
        elevated,
        dangerous,
        allowed,
        denied,
        requires_grant,
        capabilities,
    }
}

pub fn capability_preflight(
    policy: CapabilityGatePolicy,
    capability_name: Option<&str>,
) -> Result<CapabilityPreflightPayload> {
    let mut capabilities = default_capabilities();
    if let Some(name) = capability_name {
        capabilities.retain(|capability| capability.name == name);
        if capabilities.is_empty() {
            bail!("unknown capability {name}");
        }
    }

    let capabilities = capabilities
        .into_iter()
        .map(|capability| capability_preflight_entry(&policy, capability))
        .collect::<Vec<_>>();
    let ready = capabilities.iter().all(|capability| capability.allowed);

    Ok(CapabilityPreflightPayload {
        policy,
        requested_capability: capability_name.map(ToOwned::to_owned),
        ready,
        grant_env: CAPABILITY_GRANTS_ENV,
        capabilities,
    })
}

fn capability_preflight_entry(
    policy: &CapabilityGatePolicy,
    capability: pengu_mesh_shared::CapabilityDescriptor,
) -> CapabilityPreflightEntry {
    let decision = policy.evaluate(&capability);
    let allowed = matches!(decision, CapabilityDecision::Allowed);
    let grant_hint = if allowed {
        None
    } else {
        Some(format!(
            "{CAPABILITY_GRANTS_ENV}={}",
            capability.name.as_str()
        ))
    };

    CapabilityPreflightEntry {
        name: capability.name,
        risk_tier: capability.risk_tier,
        description: capability.description,
        requires_explicit_grant: capability.requires_explicit_grant,
        decision,
        allowed,
        grant_hint,
    }
}

fn parse_capability_grants(value: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    value
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|grant| !grant.is_empty())
        .filter_map(|grant| {
            if seen.insert(grant.to_string()) {
                Some(grant.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn require_capability_allowed(capability_name: &str) -> Result<()> {
    require_capability_allowed_by_policy(capability_name, &capability_policy_from_env())
}

fn require_capability_allowed_by_policy(
    capability_name: &str,
    policy: &CapabilityGatePolicy,
) -> Result<()> {
    let capability = default_capabilities()
        .into_iter()
        .find(|capability| capability.name == capability_name)
        .with_context(|| format!("unknown capability {capability_name}"))?;

    match policy.evaluate(&capability) {
        CapabilityDecision::Allowed => Ok(()),
        CapabilityDecision::Denied { reason } => {
            bail!(
                "capability denied: {reason}; grant explicitly with {CAPABILITY_GRANTS_ENV}={capability_name}"
            )
        }
        CapabilityDecision::RequiresGrant { capability } => {
            bail!(
                "capability grant required: {capability}; set {CAPABILITY_GRANTS_ENV}={capability} for this trusted local operation"
            )
        }
    }
}

fn browser_surface_action_requires_capability_grant(request: &BrowserSurfaceActionRequest) -> bool {
    matches!(
        request.execution_channel,
        Some(ExecutionChannel::GlobalTakeover)
    ) || request.allow_takeover.unwrap_or(true)
}

#[derive(Debug, Clone)]
struct AttachSeedResolution {
    kind: AttachResolutionKind,
    instance: Option<BrowserInstance>,
}

#[derive(Debug)]
pub struct StageOneRuntime {
    store: StateStore,
    artifacts: ArtifactStore,
    entrypoint: String,
    operator_id: String,
    capture_run: Mutex<CaptureRun>,
    continuity: Mutex<ContinuityStatus>,
    attach_continuity: Mutex<AttachContinuityStatus>,
}

impl StageOneRuntime {
    pub fn new() -> Result<Self> {
        Self::new_with_entrypoint("pengu-mesh-runtime")
    }

    pub fn new_with_entrypoint(entrypoint: &str) -> Result<Self> {
        Self::new_in_root(runtime_root()?, entrypoint)
    }

    pub fn new_in_root(root: impl Into<PathBuf>, entrypoint: &str) -> Result<Self> {
        let root = root.into();
        let store = StateStore::new(&root)?;
        let artifacts = ArtifactStore::new(store.artifact_root())?;
        let attach_continuity = store.get_attach_continuity()?.unwrap_or_default();
        let continuity_enabled = continuity_enabled(entrypoint);
        let (operator_id, reused_operator_id) = if continuity_enabled {
            let (identity, reused) = store.get_or_create_runtime_identity(entrypoint)?;
            (identity.operator_id, reused)
        } else {
            (format!("{entrypoint}:pid:{}", std::process::id()), false)
        };
        let (run, recovered_run) = if continuity_enabled {
            if let Some(run) = store.latest_active_run(entrypoint)? {
                (run, true)
            } else {
                (
                    store.create_run(entrypoint, "capture recording active")?,
                    false,
                )
            }
        } else {
            (
                store.create_run(entrypoint, "capture recording active")?,
                false,
            )
        };
        let runtime = Self {
            store,
            artifacts,
            entrypoint: entrypoint.to_string(),
            operator_id,
            capture_run: Mutex::new(run),
            continuity: Mutex::new(ContinuityStatus {
                continuity_enabled,
                recovered_run,
                reused_operator_id,
                recovered_run_id: None,
                recovered_lease_count: 0,
                recovered_instance_count: 0,
                stale_instance_count: 0,
                stale_instance_ids: Vec::new(),
            }),
            attach_continuity: Mutex::new(attach_continuity),
        };
        runtime.ensure_default_profiles()?;
        let continuity = runtime.bootstrap_continuity()?;
        {
            let mut current = runtime.continuity.lock().expect("continuity lock");
            *current = continuity.clone();
        }
        let _ = runtime.record_event(
            "runtime",
            if continuity.recovered_run || continuity.recovered_lease_count > 0 {
                "bootstrap_recovered"
            } else {
                "bootstrap"
            },
            EventLevel::Info,
            format!("{entrypoint} runtime ready"),
            None,
            None,
            None,
            json!({
                "entrypoint": entrypoint,
                "runtime_root": runtime.paths().root_dir.clone(),
                "operator_id": runtime.operator_id.clone(),
                "continuity": continuity,
            }),
        )?;
        Ok(runtime)
    }

    pub fn paths(&self) -> &RuntimePaths {
        self.store.paths()
    }

    pub fn health_payload(&self) -> Result<HealthPayload> {
        let instances = self.refresh_instances()?;
        Ok(HealthPayload {
            posture: RuntimePosture::stage_one(),
            paths: self.paths().clone(),
            operator_id: self.operator_id.clone(),
            daemon: self.daemon_metadata()?,
            continuity: self.continuity_status(),
            attach_continuity: self.classify_attach_continuity(&instances),
            state: StatePlan::default(),
            capture_run: self.capture_run(),
            inspection_modes: inspection_modes(),
            capability_posture: capability_posture(capability_policy_from_env()),
            routes: bootstrap_routes(),
            lease_coverage: lease_coverage_matrix(),
            host_access: self.host_access_status()?,
            artifacts: baseline_artifacts(),
            installations: discover_installations(),
            profiles: self.store.list_profiles()?,
            instances,
            leases: self.active_leases(None)?,
        })
    }

    pub fn browser_health(&self) -> Result<OperationOutcome<HealthPayload>> {
        let payload = self.health_payload()?;
        let any_ready = payload.installations.iter().any(|item| item.installed);
        if any_ready {
            Ok(OperationOutcome::success("browser runtime ready", payload))
        } else {
            Ok(OperationOutcome::failure(
                OutcomeCode::NotReady,
                "no supported Chrome installation discovered",
                payload,
            ))
        }
    }

    pub fn diagnose_report(&self) -> Result<DiagnoseReport> {
        build_diagnose_report_in_root(PathBuf::from(&self.paths().root_dir))
    }

    pub fn capability_preflight(
        &self,
        capability_name: Option<&str>,
    ) -> Result<CapabilityPreflightPayload> {
        capability_preflight(capability_policy_from_env(), capability_name)
    }

    pub fn doctor_report(&self) -> Result<DoctorReport> {
        let instances = self.refresh_instances()?;
        Ok(DoctorReport {
            posture: RuntimePosture::stage_one(),
            paths: self.paths().clone(),
            operator_id: self.operator_id.clone(),
            daemon: self.daemon_metadata()?,
            continuity: self.continuity_status(),
            attach_continuity: self.classify_attach_continuity(&instances),
            capture_run: self.capture_run(),
            inspection_modes: inspection_modes(),
            capability_posture: capability_posture(capability_policy_from_env()),
            tools: doctor_tools(),
            lease_coverage: lease_coverage_matrix(),
            browser_installs: discover_installations(),
            permissions: DoctorPermissionStatus {
                sudo_nopasswd: command_success("sudo", &["-n", "true"]),
                devtools_security: command_output("DevToolsSecurity", &["-status"]),
                external_attach_enabled: external_attach_enabled(),
                host_access: self.host_access_status()?,
            },
            instances,
            leases: self.active_leases(None)?,
            replay_validations: self.validate_replay_exports(10)?,
        })
    }

    pub fn host_access_status(&self) -> Result<HostAccessStatus> {
        macos_host_access_status()
    }

    pub fn host_access_setup(
        &self,
        request: HostAccessSetupRequest,
    ) -> Result<HostAccessSetupResult> {
        if request.mode == HostAccessSetupMode::Apply {
            require_capability_allowed("host_access_setup")?;
        }

        let result = macos_host_access_setup(&request);
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "host_access",
                    "setup",
                    EventLevel::Info,
                    format!(
                        "host access setup completed in {:?} mode with {} changed services",
                        payload.mode,
                        payload.changed_services.len()
                    ),
                    None,
                    None,
                    None,
                    json!({
                        "mode": payload.mode,
                        "changed_services": payload.changed_services,
                        "step_count": payload.steps.len(),
                        "after_summary": payload.after.summary,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "host_access",
                    "setup",
                    EventLevel::Error,
                    format!("host access setup failed: {error}"),
                    None,
                    None,
                    None,
                    json!({
                        "mode": request.mode,
                        "services": request.services,
                        "open_settings_on_missing": request.open_settings_on_missing,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn list_profiles(&self) -> Result<Vec<ManagedProfile>> {
        self.store.list_profiles()
    }

    pub fn create_profile(&self, name: &str, channel: BrowserChannel) -> Result<ManagedProfile> {
        let channel_name = channel.as_str().to_string();
        let result: Result<ManagedProfile> = (|| {
            anyhow::ensure!(!name.trim().is_empty(), "profile name is required");
            let seed = format!("{}_{}", channel.as_str(), name.trim());
            let profile = ManagedProfile {
                id: StableId::new(IdKind::Profile, &seed).into_string(),
                name: name.trim().to_string(),
                channel,
                path: self
                    .store
                    .profile_root()
                    .join(StableId::new(IdKind::Profile, &seed).as_str())
                    .display()
                    .to_string(),
            };
            if self
                .store
                .list_profiles()?
                .iter()
                .any(|existing| existing.id == profile.id)
            {
                bail!("managed profile {} already exists", profile.id);
            }
            fs::create_dir_all(&profile.path)
                .with_context(|| format!("create profile dir {}", profile.path))?;
            self.store.upsert_profile(&profile)?;
            Ok(profile)
        })();
        match &result {
            Ok(profile) => {
                let _ = self.record_event(
                    "profile",
                    "create",
                    EventLevel::Info,
                    format!("created managed profile {}", profile.name),
                    None,
                    None,
                    None,
                    json!({
                        "profile_id": profile.id,
                        "channel": profile.channel.as_str(),
                        "path": profile.path,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "profile",
                    "create",
                    EventLevel::Error,
                    format!("managed profile create failed for {name}: {error}"),
                    None,
                    None,
                    None,
                    json!({
                        "name": name,
                        "channel": channel_name,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn list_instances(&self) -> Result<Vec<BrowserInstance>> {
        self.refresh_instances()
    }

    pub fn start_instance(
        &self,
        name: &str,
        channel: BrowserChannel,
        headless: bool,
        holder_id: Option<&str>,
    ) -> Result<BrowserInstance> {
        let channel_name = channel.as_str().to_string();
        let result: Result<BrowserInstance> = (|| {
            let install = find_installation(channel.clone())
                .ok_or_else(|| anyhow!("{} is not installed", channel.as_str()))?;
            let profile = self.ensure_profile(&channel)?;
            let port = reserve_debug_port()?;
            let instance_id = StableId::new(
                IdKind::Instance,
                format!("{name}_{}_{}", channel.as_str(), port),
            )
            .into_string();
            let user_data_dir = Path::new(&profile.path).join(instance_id.replace("inst_", ""));
            let launch =
                launch_managed_browser(&install.binary_path, &user_data_dir, port, headless)?;
            let instance = BrowserInstance {
                id: instance_id,
                name: name.to_string(),
                channel,
                mode: InstanceMode::Managed,
                status: InstanceStatus::Running,
                debug_http_url: launch.debug_http_url,
                browser_ws_url: Some(launch.browser_ws_url),
                profile_id: Some(profile.id),
                profile_path: Some(user_data_dir.display().to_string()),
                pid: Some(launch.pid),
                last_error: None,
                created_at: utc_timestamp(),
                updated_at: utc_timestamp(),
            };
            self.store.upsert_instance(&instance)?;
            let holder_id = self.lease_holder_id(holder_id);
            let _ = self.lease_acquire(
                &instance.id,
                holder_id,
                self.lease_holder_label(holder_id),
                LeaseMode::Writer,
                DEFAULT_LEASE_TTL_SECONDS,
            )?;
            let _ = self.sync_tabs(&instance.id)?;
            Ok(instance)
        })();
        match &result {
            Ok(instance) => {
                let _ = self.record_event(
                    "instance",
                    "start",
                    EventLevel::Info,
                    format!("managed browser started for {}", instance.name),
                    Some(&instance.id),
                    None,
                    None,
                    json!({
                        "name": instance.name,
                        "channel": instance.channel.as_str(),
                        "headless": headless,
                        "debug_http_url": instance.debug_http_url,
                        "profile_id": instance.profile_id,
                        "pid": instance.pid,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "instance",
                    "start",
                    EventLevel::Error,
                    format!("managed browser start failed for {name}: {error}"),
                    None,
                    None,
                    None,
                    json!({
                        "name": name,
                        "channel": channel_name,
                        "headless": headless,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn attach_instance(
        &self,
        name: &str,
        cdp_url: &str,
        holder_id: Option<&str>,
    ) -> Result<BrowserInstance> {
        let result: Result<BrowserInstance> = (|| {
            if !external_attach_enabled() {
                bail!(
                    "external attach is disabled by default; set PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 to enable it"
                );
            }
            let metadata = version_from_cdp_url(cdp_url)?;
            let debug_url = debug_url_from_cdp_url(cdp_url)?;
            let live_browser_ws_url = metadata.websocket_debugger_url.clone();
            let resolution = self.find_attached_instance_seed(&debug_url, cdp_url)?;
            let existing = resolution.instance.as_ref();
            let continuity_outcome =
                attach_continuity_outcome(existing, live_browser_ws_url.as_str());
            let holder_id = self.lease_holder_id(holder_id);
            let instance_id = existing
                .map(|instance| instance.id.clone())
                .unwrap_or_else(|| {
                    StableId::new(IdKind::Instance, format!("{name}_attached")).into_string()
                });
            if existing.is_some() {
                self.require_instance_access(
                    &instance_id,
                    holder_id,
                    RequiredLease::Writer,
                    "instance_attach",
                )?;
            }
            let created_at = existing
                .map(|instance| instance.created_at.clone())
                .unwrap_or_else(utc_timestamp);
            let instance = BrowserInstance {
                id: instance_id,
                name: existing
                    .map(|instance| instance.name.clone())
                    .unwrap_or_else(|| name.to_string()),
                channel: infer_channel_from_browser(&metadata),
                mode: InstanceMode::Attached,
                status: InstanceStatus::Attached,
                debug_http_url: debug_url,
                browser_ws_url: Some(live_browser_ws_url.clone()),
                profile_id: None,
                profile_path: None,
                pid: None,
                last_error: None,
                created_at,
                updated_at: utc_timestamp(),
            };
            let endpoint_refreshed = existing
                .and_then(|candidate| candidate.browser_ws_url.as_deref())
                .is_some_and(|stored| stored != live_browser_ws_url);
            let tabs = live_tabs_for_instance(&instance)?;
            self.store.upsert_instance(&instance)?;
            let _ = self.lease_acquire(
                &instance.id,
                holder_id,
                self.lease_holder_label(holder_id),
                LeaseMode::Writer,
                DEFAULT_LEASE_TTL_SECONDS,
            )?;
            self.store.replace_tabs(&instance.id, &tabs)?;
            self.update_attach_continuity(AttachContinuityStatus {
                outcome: Some(continuity_outcome),
                freshness: AttachContinuityFreshness::Live,
                last_resolution: Some(resolution.kind.clone()),
                last_instance_id: Some(instance.id.clone()),
                last_debug_http_url: Some(instance.debug_http_url.clone()),
                last_requested_cdp_url: Some(cdp_url.to_string()),
                last_browser_ws_url: Some(live_browser_ws_url.clone()),
                reused_existing_instance: existing.is_some(),
                endpoint_refreshed,
                updated_at: Some(utc_timestamp()),
            })?;
            Ok(instance)
        })();
        match &result {
            Ok(instance) => {
                let attach_continuity = self.attach_continuity_status();
                let _ = self.record_event(
                    "instance",
                    "attach",
                    EventLevel::Info,
                    format!("attached to external browser for {}", instance.name),
                    Some(&instance.id),
                    None,
                    None,
                    json!({
                        "name": instance.name,
                        "channel": instance.channel.as_str(),
                        "debug_http_url": instance.debug_http_url,
                        "browser_ws_url": instance.browser_ws_url,
                        "attach_resolution": attach_continuity.last_resolution,
                        "reused_existing_instance": attach_continuity.reused_existing_instance,
                        "endpoint_refreshed": attach_continuity.endpoint_refreshed,
                        "requested_cdp_url": attach_continuity.last_requested_cdp_url,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "instance",
                    "attach",
                    EventLevel::Error,
                    format!("external attach failed for {name}: {error}"),
                    None,
                    None,
                    None,
                    json!({
                        "name": name,
                        "cdp_url": cdp_url,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn stop_instance(
        &self,
        instance_id: &str,
        holder_id: Option<&str>,
    ) -> Result<BrowserInstance> {
        let instance = self.require_instance(instance_id)?;
        let result: Result<BrowserInstance> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &instance.id,
                holder_id,
                RequiredLease::Writer,
                "instance_stop",
            )?;
            if instance.mode != InstanceMode::Managed {
                bail!("instance_stop only supports managed instances");
            }
            let pid = instance
                .pid
                .ok_or_else(|| anyhow!("managed instance {instance_id} has no pid"))?;
            terminate_pid(pid)?;
            thread::sleep(Duration::from_millis(300));
            let mut updated = instance.clone();
            match host_port(&updated.debug_http_url).and_then(|(host, port)| {
                wait_for_debug_endpoint(&host, port, Duration::from_millis(300))
            }) {
                Ok(_) => bail!("instance {instance_id} still responded after stop request"),
                Err(error) => {
                    updated.status = InstanceStatus::Closed;
                    updated.last_error = Some(format!("stopped by operator: {error}"));
                    updated.updated_at = utc_timestamp();
                }
            }
            self.store.upsert_instance(&updated)?;
            self.store.replace_tabs(&updated.id, &[])?;
            let _ = self.lease_release(&updated.id, holder_id, Some(LeaseMode::Writer))?;
            Ok(updated)
        })();
        match &result {
            Ok(updated) => {
                let _ = self.record_event(
                    "instance",
                    "stop",
                    EventLevel::Info,
                    format!("stopped managed browser for {}", updated.name),
                    Some(&updated.id),
                    None,
                    None,
                    json!({
                        "pid": updated.pid,
                        "debug_http_url": updated.debug_http_url,
                        "status": updated.status,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "instance",
                    "stop",
                    EventLevel::Error,
                    format!("instance stop failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    json!({"error": error.to_string()}),
                )?;
            }
        }
        result
    }

    pub fn lease_status(&self, instance_id: Option<&str>) -> Result<LeaseStatusPayload> {
        if let Some(instance_id) = instance_id {
            let _ = self.require_instance(instance_id)?;
        }
        Ok(LeaseStatusPayload {
            operator_id: self.operator_id.clone(),
            requested_resource_id: instance_id.map(str::to_string),
            leases: self.active_leases(instance_id)?,
        })
    }

    pub fn lease_acquire(
        &self,
        instance_id: &str,
        holder_id: &str,
        holder_label: Option<&str>,
        mode: LeaseMode,
        ttl_seconds: u64,
    ) -> Result<LeaseAcquirePayload> {
        let instance = self.require_instance(instance_id)?;
        let result: Result<LeaseAcquirePayload> = (|| {
            let now = utc_timestamp();
            let lease = build_instance_lease(
                instance_id,
                holder_id,
                holder_label,
                mode.clone(),
                ttl_seconds,
            )?;
            let renewed = self.store.acquire_lease(&lease, &now)?;
            Ok(LeaseAcquirePayload {
                operator_id: self.operator_id.clone(),
                lease,
                leases: self.active_leases(Some(instance_id))?,
                renewed,
                code: OutcomeCode::Ok,
                message: if renewed {
                    format!("renewed {:?} lease for {instance_id}", mode)
                } else {
                    format!("acquired {:?} lease for {instance_id}", mode)
                },
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "lease",
                    "acquire",
                    EventLevel::Info,
                    format!(
                        "acquired {:?} lease on {} for {}",
                        payload.lease.mode, instance.name, payload.lease.holder_id
                    ),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "lease_id": payload.lease.id,
                        "holder_id": payload.lease.holder_id,
                        "holder_label": payload.lease.holder_label,
                        "mode": payload.lease.mode,
                        "expires_at": payload.lease.expires_at,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "lease",
                    "acquire",
                    EventLevel::Warning,
                    format!("lease acquire failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "holder_id": holder_id,
                        "holder_label": holder_label,
                        "mode": mode,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn lease_release(
        &self,
        instance_id: &str,
        holder_id: &str,
        mode: Option<LeaseMode>,
    ) -> Result<LeaseReleasePayload> {
        let _ = self.require_instance(instance_id)?;
        let result: Result<LeaseReleasePayload> = (|| {
            let now = utc_timestamp();
            self.store.prune_expired_leases(&now)?;
            let released_count = self
                .store
                .delete_leases(instance_id, holder_id, mode.clone())?;
            Ok(LeaseReleasePayload {
                operator_id: self.operator_id.clone(),
                requested_resource_id: instance_id.to_string(),
                holder_id: holder_id.to_string(),
                released_count,
                leases: self.active_leases(Some(instance_id))?,
                code: OutcomeCode::Ok,
                message: format!("released {released_count} lease(s) for {instance_id}"),
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "lease",
                    "release",
                    EventLevel::Info,
                    format!(
                        "released {} lease(s) on {} for {}",
                        payload.released_count, instance_id, payload.holder_id
                    ),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "holder_id": payload.holder_id,
                        "released_count": payload.released_count,
                        "mode": mode,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "lease",
                    "release",
                    EventLevel::Warning,
                    format!("lease release failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "holder_id": holder_id,
                        "mode": mode,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn lease_transfer(
        &self,
        instance_id: &str,
        from_holder_id: &str,
        to_holder_id: &str,
        to_holder_label: Option<&str>,
        ttl_seconds: u64,
    ) -> Result<LeaseTransferPayload> {
        let _ = self.require_instance(instance_id)?;
        let result: Result<LeaseTransferPayload> = (|| {
            let now = utc_timestamp();
            self.store.prune_expired_leases(&now)?;
            let replacement = build_instance_lease(
                instance_id,
                to_holder_id,
                to_holder_label,
                LeaseMode::Writer,
                ttl_seconds,
            )?;
            self.store
                .transfer_writer_lease(instance_id, from_holder_id, &replacement, &now)?;
            Ok(LeaseTransferPayload {
                operator_id: self.operator_id.clone(),
                previous_holder_id: from_holder_id.to_string(),
                lease: replacement,
                leases: self.active_leases(Some(instance_id))?,
                code: OutcomeCode::Ok,
                message: format!("transferred writer lease for {instance_id} to {to_holder_id}"),
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "lease",
                    "transfer",
                    EventLevel::Info,
                    format!(
                        "transferred writer lease on {} from {} to {}",
                        instance_id, from_holder_id, payload.lease.holder_id
                    ),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "from_holder_id": from_holder_id,
                        "to_holder_id": payload.lease.holder_id,
                        "to_holder_label": payload.lease.holder_label,
                        "expires_at": payload.lease.expires_at,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "lease",
                    "transfer",
                    EventLevel::Warning,
                    format!("lease transfer failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "from_holder_id": from_holder_id,
                        "to_holder_id": to_holder_id,
                        "to_holder_label": to_holder_label,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn list_tabs(&self, instance_id: &str, holder_id: Option<&str>) -> Result<Vec<BrowserTab>> {
        let holder_id = self.lease_holder_id(holder_id);
        self.require_instance_access(instance_id, holder_id, RequiredLease::Observer, "tab_list")?;
        self.sync_tabs(instance_id)
    }

    pub fn tab_list_actions(
        &self,
        instance_id: &str,
        tab_id: &str,
        holder_id: Option<&str>,
    ) -> Result<TabActionCatalogPayload> {
        let result: Result<TabActionCatalogPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                instance_id,
                holder_id,
                RequiredLease::Observer,
                "tab_list_actions",
            )?;
            let instance = self.require_instance(instance_id)?;
            let tab = self.require_tab(tab_id)?;
            anyhow::ensure!(
                tab.instance_id == instance.id,
                "tab {tab_id} does not belong to instance {instance_id}"
            );
            Ok(TabActionCatalogPayload {
                instance,
                tab: tab.clone(),
                actions: tab_action_contracts(&tab),
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "tab",
                    "list_actions",
                    EventLevel::Info,
                    format!(
                        "listed {} tab action contracts for {}",
                        payload.actions.len(),
                        payload.tab.id
                    ),
                    Some(&payload.instance.id),
                    Some(&payload.tab.id),
                    None,
                    json!({
                        "action_count": payload.actions.len(),
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "list_actions",
                    EventLevel::Error,
                    format!("tab action catalog failed for {tab_id}: {error}"),
                    Some(instance_id),
                    Some(tab_id),
                    None,
                    json!({"error": error.to_string()}),
                )?;
            }
        }
        result
    }

    pub fn browser_surface_list(
        &self,
        instance_id: &str,
        holder_id: Option<&str>,
    ) -> Result<BrowserSurfaceListPayload> {
        let result: Result<BrowserSurfaceListPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                instance_id,
                holder_id,
                RequiredLease::Observer,
                "browser_surface_list",
            )?;
            let instance = self.require_instance(instance_id)?;
            macos_browser_surface_list(&instance)
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "browser_surface",
                    "list",
                    EventLevel::Info,
                    format!(
                        "listed {} native browser surfaces for {}",
                        payload.surfaces.len(),
                        payload.instance.id
                    ),
                    Some(&payload.instance.id),
                    None,
                    None,
                    json!({
                        "app_name": payload.app_name,
                        "surface_count": payload.surfaces.len(),
                    }),
                )?;
            }
            Err(error) => {
                let failure = browser_surface_failure_payload(
                    "browser_surface_list",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: None,
                        root_surface_id: None,
                        action: None,
                        execution_channel: None,
                        allow_takeover: None,
                    },
                    error,
                );
                let _ = self.record_event(
                    "browser_surface",
                    "list",
                    EventLevel::Error,
                    format!("browser surface list failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    serde_json::to_value(&failure).expect("serializable browser surface failure"),
                )?;
            }
        }
        result
    }

    pub fn browser_surface_list_actions(
        &self,
        instance_id: &str,
        surface_id: &str,
        holder_id: Option<&str>,
    ) -> Result<BrowserSurfaceActionCatalogPayload> {
        let result: Result<BrowserSurfaceActionCatalogPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                instance_id,
                holder_id,
                RequiredLease::Observer,
                "browser_surface_list_actions",
            )?;
            let instance = self.require_instance(instance_id)?;
            let surface_list = macos_browser_surface_list(&instance)?;
            let surface = surface_list
                .surfaces
                .iter()
                .find(|candidate| candidate.id == surface_id)
                .cloned()
                .ok_or_else(|| anyhow!("surface not found: {surface_id}"))?;
            let host_access = self.host_access_status()?;
            Ok(BrowserSurfaceActionCatalogPayload {
                instance,
                app_name: surface_list.app_name,
                surface: surface.clone(),
                actions: surface_action_contracts(&surface, &host_access),
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "browser_surface",
                    "list_actions",
                    EventLevel::Info,
                    format!(
                        "listed {} action contracts for {}",
                        payload.actions.len(),
                        payload.surface.id
                    ),
                    Some(&payload.instance.id),
                    None,
                    None,
                    json!({
                        "surface_id": payload.surface.id,
                        "surface_role": payload.surface.role,
                        "action_count": payload.actions.len(),
                    }),
                )?;
            }
            Err(error) => {
                let failure = browser_surface_failure_payload(
                    "browser_surface_list_actions",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: Some(surface_id.to_string()),
                        root_surface_id: None,
                        action: None,
                        execution_channel: None,
                        allow_takeover: None,
                    },
                    error,
                );
                let _ = self.record_event(
                    "browser_surface",
                    "list_actions",
                    EventLevel::Error,
                    format!("browser surface list-actions failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    serde_json::to_value(&failure).expect("serializable browser surface failure"),
                )?;
            }
        }
        result
    }

    pub fn browser_surface_snapshot(
        &self,
        instance_id: &str,
        root_surface_id: Option<&str>,
        holder_id: Option<&str>,
    ) -> Result<BrowserSurfaceSnapshot> {
        let result: Result<BrowserSurfaceSnapshot> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                instance_id,
                holder_id,
                RequiredLease::Observer,
                "browser_surface_snapshot",
            )?;
            let instance = self.require_instance(instance_id)?;
            let native_snapshot = macos_browser_surface_snapshot(&instance, root_surface_id)?;
            let scope_id = native_surface_scope_id(root_surface_id);
            let snapshot_body = serde_json::to_string_pretty(&json!({
                "instance": instance,
                "app_name": native_snapshot.app_name,
                "root_surface_id": root_surface_id,
                "capture_source": native_snapshot.capture_source,
                "capture_detail": native_snapshot.capture_detail,
                "surfaces": native_snapshot.surfaces,
            }))
            .context("serialize browser surface snapshot")?;
            let snapshot_artifact = self.artifacts.write_text(
                ArtifactKind::Snapshot,
                self.active_run_id().as_deref(),
                instance_id,
                &scope_id,
                &snapshot_body,
            )?;
            self.store.upsert_artifact(&snapshot_artifact)?;
            let capture_artifact = match capture_bytes_from_snapshot(&native_snapshot) {
                Ok(Some(bytes)) => {
                    let artifact = self.artifacts.write_bytes(
                        ArtifactKind::Screenshot,
                        self.active_run_id().as_deref(),
                        instance_id,
                        &scope_id,
                        &bytes,
                    )?;
                    self.store.upsert_artifact(&artifact)?;
                    Some(artifact)
                }
                Ok(None) => None,
                Err(error) => {
                    cleanup_snapshot_capture(&native_snapshot);
                    return Err(error);
                }
            };
            Ok(BrowserSurfaceSnapshot {
                instance,
                app_name: native_snapshot.app_name,
                root_surface_id: root_surface_id.map(ToOwned::to_owned),
                snapshot_artifact,
                capture_artifact,
                capture_source: native_snapshot.capture_source,
                capture_detail: native_snapshot.capture_detail,
                surfaces: native_snapshot.surfaces,
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "browser_surface",
                    "snapshot",
                    EventLevel::Info,
                    format!(
                        "captured native browser surface snapshot for {}",
                        payload.instance.id
                    ),
                    Some(&payload.instance.id),
                    Some(&payload.snapshot_artifact.tab_id),
                    Some(&payload.snapshot_artifact.id),
                    merge_artifact_data(
                        &payload.snapshot_artifact,
                        json!({
                            "root_surface_id": payload.root_surface_id,
                            "surface_count": payload.surfaces.len(),
                            "capture_source": payload.capture_source,
                            "capture_detail": payload.capture_detail,
                            "capture_artifact_id": payload.capture_artifact.as_ref().map(|artifact| artifact.id.clone()),
                        }),
                    ),
                )?;
            }
            Err(error) => {
                let failure = browser_surface_failure_payload(
                    "browser_surface_snapshot",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: None,
                        root_surface_id: root_surface_id.map(ToOwned::to_owned),
                        action: None,
                        execution_channel: None,
                        allow_takeover: None,
                    },
                    error,
                );
                let _ = self.record_event(
                    "browser_surface",
                    "snapshot",
                    EventLevel::Error,
                    format!("browser surface snapshot failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    serde_json::to_value(&failure).expect("serializable browser surface failure"),
                )?;
            }
        }
        result
    }

    pub fn browser_surface_action(
        &self,
        instance_id: &str,
        request: BrowserSurfaceActionRequest,
        holder_id: Option<&str>,
    ) -> Result<BrowserSurfaceActionPayload> {
        let result: Result<BrowserSurfaceActionPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                instance_id,
                holder_id,
                RequiredLease::Writer,
                "browser_surface_action",
            )?;
            let instance = self.require_instance(instance_id)?;
            if browser_surface_action_requires_capability_grant(&request) {
                require_capability_allowed("browser_surface_action")?;
            }
            macos_browser_surface_action(&instance, &request)
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "browser_surface",
                    payload.requested.action.as_str(),
                    EventLevel::Info,
                    format!(
                        "executed native browser surface action {} on {}",
                        payload.requested.action.as_str(),
                        payload.instance.id
                    ),
                    Some(&payload.instance.id),
                    None,
                    None,
                    json!({
                        "app_name": payload.app_name,
                        "target_surface_id": payload.target_surface_id,
                        "resolved_channel": payload.resolved_channel,
                        "interference_level": payload.interference_level,
                        "took_focus": payload.took_focus,
                        "fallback_count": payload.fallback_count,
                        "detail": payload.detail,
                    }),
                )?;
            }
            Err(error) => {
                let failure = browser_surface_failure_payload(
                    "browser_surface_action",
                    BrowserSurfaceFailureAttempt {
                        instance_id: instance_id.to_string(),
                        surface_id: request.surface_id.clone(),
                        root_surface_id: None,
                        action: Some(request.action.clone()),
                        execution_channel: request.execution_channel.clone(),
                        allow_takeover: request.allow_takeover,
                    },
                    error,
                );
                let _ = self.record_event(
                    "browser_surface",
                    request.action.as_str(),
                    EventLevel::Error,
                    format!("browser surface action failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    serde_json::to_value(&failure).expect("serializable browser surface failure"),
                )?;
            }
        }
        result
    }

    pub fn open_tab(
        &self,
        instance_id: &str,
        url: &str,
        holder_id: Option<&str>,
    ) -> Result<BrowserTab> {
        let result: Result<BrowserTab> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                instance_id,
                holder_id,
                RequiredLease::Writer,
                "tab_open",
            )?;
            let instance = self.require_instance(instance_id)?;
            let (host, port) = host_port(&instance.debug_http_url)?;
            let target = open_tab(&host, port, url)?;
            activate_tab(&host, port, &target.id)?;
            thread::sleep(Duration::from_millis(350));
            let tabs = self.sync_tabs(instance_id)?;
            tabs.into_iter()
                .find(|tab| tab.target_id == target.id)
                .ok_or_else(|| anyhow!("opened tab not found in runtime state"))
        })();
        match &result {
            Ok(tab) => {
                let _ = self.record_event(
                    "tab",
                    "open",
                    EventLevel::Info,
                    format!("opened tab {}", tab.title),
                    Some(&tab.instance_id),
                    Some(&tab.id),
                    None,
                    json!({
                        "url": tab.url,
                        "target_id": tab.target_id,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "open",
                    EventLevel::Error,
                    format!("tab open failed for {instance_id}: {error}"),
                    Some(instance_id),
                    None,
                    None,
                    json!({
                        "url": url,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn close_tab(&self, tab_id: &str, holder_id: Option<&str>) -> Result<EmptyPayload> {
        let tab = self.require_tab(tab_id)?;
        let result: Result<EmptyPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &tab.instance_id,
                holder_id,
                RequiredLease::Writer,
                "tab_close",
            )?;
            let instance = self.require_instance(&tab.instance_id)?;
            let (host, port) = host_port(&instance.debug_http_url)?;
            close_tab(&host, port, &tab.target_id)?;
            let _ = self.sync_tabs(&tab.instance_id)?;
            Ok(EmptyPayload::new(format!("closed {tab_id}")))
        })();
        match &result {
            Ok(_) => {
                let _ = self.record_event(
                    "tab",
                    "close",
                    EventLevel::Info,
                    format!("closed tab {}", tab.title),
                    Some(&tab.instance_id),
                    Some(&tab.id),
                    None,
                    json!({
                        "url": tab.url,
                        "target_id": tab.target_id,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "close",
                    EventLevel::Error,
                    format!("tab close failed for {tab_id}: {error}"),
                    Some(&tab.instance_id),
                    Some(&tab.id),
                    None,
                    json!({"error": error.to_string()}),
                )?;
            }
        }
        result
    }

    pub fn tab_action(
        &self,
        tab_id: &str,
        request: TabActionRequest,
        holder_id: Option<&str>,
    ) -> Result<TabActionPayload> {
        let stored_tab = self.require_tab(tab_id)?;
        let result: Result<TabActionPayload> = (|| {
            validate_tab_action_request(&request)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Writer,
                "tab_action",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            let refreshed_tab = self
                .sync_tabs(&tab.instance_id)?
                .into_iter()
                .find(|candidate| candidate.id == tab.id)
                .unwrap_or(tab);
            let (resolved_target, detail, final_url, load_event_fired, duration_ms, result) =
                if matches!(request.kind, TabActionKind::Evaluate) {
                    let expression = request
                        .expression
                        .as_deref()
                        .ok_or_else(|| anyhow!("evaluate requires expression"))?;
                    let value = session
                        .evaluate_json(expression)
                        .map_err(normalize_runtime_evaluate_error)?;
                    (
                        "page".to_string(),
                        "evaluated expression over CDP".to_string(),
                        None,
                        None,
                        None,
                        Some(value),
                    )
                } else if matches!(request.kind, TabActionKind::Navigate) {
                    let url = request
                        .url
                        .as_deref()
                        .ok_or_else(|| anyhow!("navigate requires url"))?;
                    let NavigationEvidence {
                        final_url,
                        load_event_fired,
                        duration_ms,
                    } = session.navigate(
                        url,
                        Duration::from_millis(request.timeout_ms.unwrap_or(10_000)),
                    )?;
                    (
                        final_url.clone(),
                        format!("navigated to {final_url}"),
                        Some(final_url),
                        Some(load_event_fired),
                        Some(duration_ms),
                        None,
                    )
                } else {
                    let action_result = session.evaluate_json(&tab_action_script(&request)?)?;
                    let resolved_target = action_result["target"]
                        .as_str()
                        .ok_or_else(|| anyhow!("tab action response missing target"))?
                        .to_string();
                    let detail = action_result["detail"]
                        .as_str()
                        .ok_or_else(|| anyhow!("tab action response missing detail"))?
                        .to_string();
                    (resolved_target, detail, None, None, None, None)
                };
            Ok(TabActionPayload {
                tab: refreshed_tab,
                requested: request.clone(),
                resolved_target,
                detail,
                final_url,
                load_event_fired,
                duration_ms,
                result,
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "tab",
                    request.kind.as_str(),
                    EventLevel::Info,
                    format!("executed {} on {}", request.kind.as_str(), payload.tab.id),
                    Some(&payload.tab.instance_id),
                    Some(&payload.tab.id),
                    None,
                    json!({
                        "kind": request.kind.as_str(),
                        "ref": request.ref_id,
                        "selector": request.selector,
                        "url": request.url,
                        "expression": request.expression,
                        "text": request.text,
                        "value": request.value,
                        "key": request.key,
                        "resolved_target": payload.resolved_target,
                        "detail": payload.detail,
                        "final_url": payload.final_url,
                        "load_event_fired": payload.load_event_fired,
                        "duration_ms": payload.duration_ms,
                        "result": payload.result,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    request.kind.as_str(),
                    EventLevel::Error,
                    format!("tab action failed for {tab_id}: {error}"),
                    Some(&stored_tab.instance_id),
                    Some(&stored_tab.id),
                    None,
                    json!({
                        "kind": request.kind.as_str(),
                        "ref": request.ref_id,
                        "selector": request.selector,
                        "url": request.url,
                        "expression": request.expression,
                        "text": request.text,
                        "value": request.value,
                        "key": request.key,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn snapshot_tab(&self, tab_id: &str, holder_id: Option<&str>) -> Result<SnapshotPayload> {
        let result: Result<SnapshotPayload> = (|| {
            let stored_tab = self.require_tab(tab_id)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Observer,
                "tab_snapshot",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            let snapshot = session.evaluate_json(&snapshot_script())?;
            let artifact = self.artifacts.write_text(
                ArtifactKind::Snapshot,
                self.active_run_id().as_deref(),
                &tab.instance_id,
                &tab.id,
                &serde_json::to_string_pretty(&snapshot).context("serialize snapshot")?,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(SnapshotPayload {
                tab,
                artifact,
                snapshot,
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_artifact_event(
                    "tab",
                    "snapshot",
                    "captured accessibility snapshot",
                    payload,
                    json!({"node_count": payload.snapshot["nodes"].as_array().map_or(0, Vec::len)}),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "snapshot",
                    EventLevel::Error,
                    format!("tab snapshot failed for {tab_id}: {error}"),
                    None,
                    Some(tab_id),
                    None,
                    json!({"error": error.to_string()}),
                )?;
            }
        }
        result
    }

    pub fn text_tab(&self, tab_id: &str, holder_id: Option<&str>) -> Result<TextPayload> {
        let result: Result<TextPayload> = (|| {
            let stored_tab = self.require_tab(tab_id)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Observer,
                "tab_text",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            let payload = session.evaluate_json(text_script())?;
            let text = payload["text"].as_str().unwrap_or_default().to_string();
            let artifact = self.artifacts.write_text(
                ArtifactKind::Text,
                self.active_run_id().as_deref(),
                &tab.instance_id,
                &tab.id,
                &text,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(TextPayload {
                tab,
                artifact,
                text,
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_artifact_event(
                    "tab",
                    "text",
                    "captured tab text",
                    payload,
                    json!({"text_bytes": payload.text.len()}),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "text",
                    EventLevel::Error,
                    format!("tab text extraction failed for {tab_id}: {error}"),
                    None,
                    Some(tab_id),
                    None,
                    json!({"error": error.to_string()}),
                )?;
            }
        }
        result
    }

    pub fn screenshot_tab(
        &self,
        tab_id: &str,
        holder_id: Option<&str>,
        full_page: bool,
    ) -> Result<ArtifactPayload> {
        let result: Result<ArtifactPayload> = (|| {
            let stored_tab = self.require_tab(tab_id)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Observer,
                "tab_screenshot",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            let encoded = session.capture_screenshot(full_page)?;
            let artifact = self.artifacts.write_base64(
                ArtifactKind::Screenshot,
                self.active_run_id().as_deref(),
                &tab.instance_id,
                &tab.id,
                &encoded,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(ArtifactPayload { tab, artifact })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_artifact_event(
                    "tab",
                    "screenshot",
                    "captured tab screenshot",
                    payload,
                    json!({"full_page": full_page}),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "screenshot",
                    EventLevel::Error,
                    format!("tab screenshot failed for {tab_id}: {error}"),
                    None,
                    Some(tab_id),
                    None,
                    json!({"error": error.to_string(), "full_page": full_page}),
                )?;
            }
        }
        result
    }

    pub fn pdf_tab(&self, tab_id: &str, holder_id: Option<&str>) -> Result<ArtifactPayload> {
        let result: Result<ArtifactPayload> = (|| {
            let stored_tab = self.require_tab(tab_id)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Observer,
                "tab_pdf",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            let encoded = session.print_to_pdf()?;
            let artifact = self.artifacts.write_base64(
                ArtifactKind::Pdf,
                self.active_run_id().as_deref(),
                &tab.instance_id,
                &tab.id,
                &encoded,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(ArtifactPayload { tab, artifact })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_artifact_event(
                    "tab",
                    "pdf",
                    "captured tab pdf",
                    payload,
                    json!({}),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "pdf",
                    EventLevel::Error,
                    format!("tab pdf capture failed for {tab_id}: {error}"),
                    None,
                    Some(tab_id),
                    None,
                    json!({"error": error.to_string()}),
                )?;
            }
        }
        result
    }

    pub fn artifact_crop(
        &self,
        artifact_id: &str,
        crop_region: NormalizedRegion,
        page_index: Option<u32>,
        holder_id: Option<&str>,
    ) -> Result<ArtifactCropPayload> {
        let crop_region_for_error = crop_region.clone();
        let result: Result<ArtifactCropPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            let source_artifact = self
                .store
                .get_artifact(artifact_id)?
                .ok_or_else(|| anyhow!("unknown artifact {artifact_id}"))?;
            self.require_instance_access(
                &source_artifact.instance_id,
                holder_id,
                RequiredLease::Observer,
                "artifact_crop",
            )?;
            if source_artifact.kind != ArtifactKind::Pdf && page_index.is_some() {
                bail!("page_index is only valid for pdf artifacts");
            }
            let artifact = self.artifacts.crop_artifact(
                &source_artifact,
                self.active_run_id().as_deref(),
                &crop_region,
                page_index,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(ArtifactCropPayload {
                source_artifact,
                artifact,
                crop_region,
                page_index,
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "artifact",
                    "crop",
                    EventLevel::Info,
                    format!("created derived crop from {}", payload.source_artifact.id),
                    Some(&payload.artifact.instance_id),
                    Some(&payload.artifact.tab_id),
                    Some(&payload.artifact.id),
                    merge_artifact_data(
                        &payload.artifact,
                        json!({
                            "source_artifact_id": payload.source_artifact.id,
                            "crop_region": payload.crop_region,
                            "page_index": payload.page_index,
                        }),
                    ),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "artifact",
                    "crop",
                    EventLevel::Error,
                    format!("artifact crop failed for {artifact_id}: {error}"),
                    None,
                    None,
                    None,
                    json!({
                        "artifact_id": artifact_id,
                        "page_index": page_index,
                        "crop_region": crop_region_for_error,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn artifact_crop_grid(
        &self,
        artifact_id: &str,
        rows: u16,
        cols: u16,
        overlap: u16,
        page_index: Option<u32>,
        holder_id: Option<&str>,
    ) -> Result<ArtifactCropGridPayload> {
        let result: Result<ArtifactCropGridPayload> = (|| {
            let holder_id = self.lease_holder_id(holder_id);
            let source_artifact = self
                .store
                .get_artifact(artifact_id)?
                .ok_or_else(|| anyhow!("unknown artifact {artifact_id}"))?;
            self.require_instance_access(
                &source_artifact.instance_id,
                holder_id,
                RequiredLease::Observer,
                "artifact_crop_grid",
            )?;
            if source_artifact.kind != ArtifactKind::Pdf && page_index.is_some() {
                bail!("page_index is only valid for pdf artifacts");
            }
            let regions = ArtifactStore::batch_grid_regions(rows, cols, overlap)?;
            let handles = self.artifacts.crop_artifact_many(
                &source_artifact,
                self.active_run_id().as_deref(),
                &regions,
                page_index,
            )?;
            let mut artifacts = Vec::with_capacity(handles.len());
            for (crop_region, artifact) in regions.into_iter().zip(handles.into_iter()) {
                self.store.upsert_artifact(&artifact)?;
                artifacts.push(BatchCropArtifact {
                    crop_region,
                    artifact,
                });
            }
            Ok(ArtifactCropGridPayload {
                source_artifact,
                artifacts,
                rows,
                cols,
                overlap,
                page_index,
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_event(
                    "artifact",
                    "crop_grid",
                    EventLevel::Info,
                    format!(
                        "created {} derived grid crops from {}",
                        payload.artifacts.len(),
                        payload.source_artifact.id
                    ),
                    Some(&payload.source_artifact.instance_id),
                    Some(&payload.source_artifact.tab_id),
                    None,
                    json!({
                        "source_artifact_id": payload.source_artifact.id,
                        "rows": payload.rows,
                        "cols": payload.cols,
                        "overlap": payload.overlap,
                        "page_index": payload.page_index,
                        "derived_count": payload.artifacts.len(),
                        "derived_artifact_ids": payload.artifacts.iter().map(|item| item.artifact.id.clone()).collect::<Vec<_>>(),
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "artifact",
                    "crop_grid",
                    EventLevel::Error,
                    format!("artifact crop grid failed for {artifact_id}: {error}"),
                    None,
                    None,
                    None,
                    json!({
                        "artifact_id": artifact_id,
                        "rows": rows,
                        "cols": cols,
                        "overlap": overlap,
                        "page_index": page_index,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn trace_capture(
        &self,
        tab_id: &str,
        duration_ms: u64,
        categories: &[String],
        holder_id: Option<&str>,
    ) -> Result<TraceCapturePayload> {
        let duration_ms = duration_ms.clamp(100, 30_000);
        let category_values = if categories.is_empty() {
            default_trace_categories()
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        } else {
            categories.to_vec()
        };
        let category_refs = category_values
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let result: Result<TraceCapturePayload> = (|| {
            let stored_tab = self.require_tab(tab_id)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Observer,
                "trace_capture",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            session.start_tracing(&category_refs)?;
            thread::sleep(Duration::from_millis(duration_ms));
            let bytes = session.end_tracing_and_collect()?;
            let artifact = self.artifacts.write_bytes(
                ArtifactKind::Trace,
                self.active_run_id().as_deref(),
                &tab.instance_id,
                &tab.id,
                &bytes,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(TraceCapturePayload {
                tab,
                artifact,
                duration_ms,
                categories: category_values.clone(),
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_artifact_event(
                    "tab",
                    "trace_capture",
                    "captured trace artifact",
                    &ArtifactPayload {
                        tab: payload.tab.clone(),
                        artifact: payload.artifact.clone(),
                    },
                    json!({
                        "duration_ms": payload.duration_ms,
                        "categories": payload.categories,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "trace_capture",
                    EventLevel::Error,
                    format!("trace capture failed for {tab_id}: {error}"),
                    None,
                    Some(tab_id),
                    None,
                    json!({
                        "tab_id": tab_id,
                        "duration_ms": duration_ms,
                        "categories": category_values,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn recording_capture(
        &self,
        tab_id: &str,
        duration_ms: u64,
        interval_ms: u64,
        holder_id: Option<&str>,
    ) -> Result<RecordingCapturePayload> {
        let duration_ms = duration_ms.clamp(100, 30_000);
        let interval_ms = interval_ms.clamp(50, 5_000);
        let max_frames = 120_usize;
        let expected_frames = (((duration_ms.saturating_sub(1)) / interval_ms) + 1) as usize;
        anyhow::ensure!(
            expected_frames <= max_frames,
            "recording would capture {expected_frames} frames; lower duration or raise interval"
        );
        let result: Result<RecordingCapturePayload> = (|| {
            let stored_tab = self.require_tab(tab_id)?;
            let holder_id = self.lease_holder_id(holder_id);
            self.require_instance_access(
                &stored_tab.instance_id,
                holder_id,
                RequiredLease::Observer,
                "recording_capture",
            )?;
            let (tab, mut session, _) = self.connect_tab_session(tab_id)?;
            let started_at = utc_timestamp();
            let started = std::time::Instant::now();
            let mut next_capture = started;
            let mut frames = Vec::with_capacity(expected_frames);
            let mut frame_manifest = Vec::with_capacity(expected_frames);
            loop {
                let offset_ms = started.elapsed().as_millis() as u64;
                let encoded = session.capture_screenshot(false)?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .context("decode recording frame")?;
                let frame_name = format!("frames/frame-{index:04}.png", index = frames.len() + 1);
                frame_manifest.push(json!({
                    "name": frame_name.clone(),
                    "offset_ms": offset_ms,
                    "bytes": bytes.len(),
                }));
                frames.push((frame_name, bytes));
                if offset_ms >= duration_ms || frames.len() >= expected_frames {
                    break;
                }
                next_capture += Duration::from_millis(interval_ms);
                let now = std::time::Instant::now();
                if next_capture > now {
                    thread::sleep(next_capture - now);
                }
            }
            let manifest_json = serde_json::to_string_pretty(&json!({
                "schema_version": 1,
                "captured_at": started_at,
                "duration_ms": duration_ms,
                "interval_ms": interval_ms,
                "frame_count": frames.len(),
                "tab": {
                    "id": tab.id.clone(),
                    "instance_id": tab.instance_id.clone(),
                    "title": tab.title.clone(),
                    "url": tab.url.clone(),
                },
                "frames": frame_manifest,
            }))
            .context("serialize recording manifest")?;
            let artifact = self.artifacts.write_recording_archive(
                self.active_run_id().as_deref(),
                &tab.instance_id,
                &tab.id,
                &manifest_json,
                &frames,
            )?;
            self.store.upsert_artifact(&artifact)?;
            Ok(RecordingCapturePayload {
                tab,
                artifact,
                duration_ms,
                interval_ms,
                frame_count: frames.len(),
            })
        })();
        match &result {
            Ok(payload) => {
                let _ = self.record_artifact_event(
                    "tab",
                    "recording_capture",
                    "captured screenshot recording archive",
                    &ArtifactPayload {
                        tab: payload.tab.clone(),
                        artifact: payload.artifact.clone(),
                    },
                    json!({
                        "duration_ms": payload.duration_ms,
                        "interval_ms": payload.interval_ms,
                        "frame_count": payload.frame_count,
                    }),
                )?;
            }
            Err(error) => {
                let _ = self.record_event(
                    "tab",
                    "recording_capture",
                    EventLevel::Error,
                    format!("recording capture failed for {tab_id}: {error}"),
                    None,
                    Some(tab_id),
                    None,
                    json!({
                        "tab_id": tab_id,
                        "duration_ms": duration_ms,
                        "interval_ms": interval_ms,
                        "error": error.to_string(),
                    }),
                )?;
            }
        }
        result
    }

    pub fn capture_start_recording(&self) -> Result<CaptureRun> {
        let current = self.capture_run();
        if current.status == RunStatus::Active {
            let _ = self.record_event(
                "capture",
                "start",
                EventLevel::Info,
                "capture recording already active",
                None,
                None,
                None,
                json!({"run_id": current.id}),
            )?;
            return Ok(current);
        }
        let run = self
            .store
            .create_run(&self.entrypoint, "capture recording active")?;
        {
            let mut current_run = self.capture_run.lock().expect("capture run lock");
            *current_run = run.clone();
        }
        let _ = self.record_event(
            "capture",
            "start",
            EventLevel::Info,
            "capture recording started",
            None,
            None,
            None,
            json!({"run_id": run.id}),
        )?;
        Ok(run)
    }

    pub fn capture_stop_recording(&self) -> Result<CaptureRun> {
        let current = self.capture_run();
        if current.status == RunStatus::Completed {
            return Ok(current);
        }
        let _ = self.record_event(
            "capture",
            "stop",
            EventLevel::Info,
            "capture recording stopped",
            None,
            None,
            None,
            json!({"run_id": current.id}),
        )?;
        let completed = self
            .store
            .complete_run(&current.id, Some("capture recording stopped"))?
            .unwrap_or(current);
        {
            let mut current_run = self.capture_run.lock().expect("capture run lock");
            *current_run = completed.clone();
        }
        Ok(completed)
    }

    pub fn events_tail(&self, run_id: Option<&str>, limit: usize) -> Result<EventTailPayload> {
        let limit = limit.clamp(1, 200);
        let selected_run_id = run_id
            .map(str::to_string)
            .unwrap_or_else(|| self.capture_run().id);
        let run = self.store.get_run(&selected_run_id)?;
        if run_id.is_some() && run.is_none() {
            bail!("unknown run {selected_run_id}");
        }
        Ok(EventTailPayload {
            run,
            requested_limit: limit,
            events: self.store.tail_events(Some(&selected_run_id), limit)?,
        })
    }

    pub fn list_runs(&self, limit: usize) -> Result<RunListPayload> {
        self.run_list(limit)
    }

    pub fn export_replay_manifest(
        &self,
        run_id: Option<&str>,
        mode: ReplayExportMode,
    ) -> Result<ReplayManifestExport> {
        self.replay_export(run_id, mode)
    }

    pub fn run_list(&self, limit: usize) -> Result<RunListPayload> {
        let limit = limit.clamp(1, 200);
        Ok(RunListPayload {
            requested_limit: limit,
            runs: self.store.list_runs(limit)?,
        })
    }

    pub fn scenario_list(&self, family: Option<&str>, limit: usize) -> Result<ScenarioListPayload> {
        let limit = limit.clamp(1, 200);
        Ok(ScenarioListPayload {
            requested_family: family.map(str::to_string),
            requested_limit: limit,
            runs: self.store.list_scenario_runs(family, limit)?,
        })
    }

    pub fn scenario_run_detail(&self, run_id: &str) -> Result<ScenarioRunDetailPayload> {
        let run = self
            .store
            .get_scenario_run(run_id)?
            .ok_or_else(|| anyhow!("unknown scenario run {run_id}"))?;
        Ok(ScenarioRunDetailPayload {
            steps: self.store.list_scenario_steps(run_id)?,
            assertions: self.store.list_scenario_assertions(run_id)?,
            latency_samples: self.store.list_latency_samples(run_id)?,
            environment_fingerprint: self.store.get_environment_fingerprint(run_id)?,
            run,
        })
    }

    pub fn replay_export(
        &self,
        run_id: Option<&str>,
        mode: ReplayExportMode,
    ) -> Result<ReplayManifestExport> {
        let selected_run_id = run_id
            .map(str::to_string)
            .unwrap_or_else(|| self.capture_run().id);
        let run = self
            .store
            .get_run(&selected_run_id)?
            .ok_or_else(|| anyhow!("unknown run {selected_run_id}"))?;
        let events = self.store.list_events_for_run(&selected_run_id)?;
        let artifacts = self.store.list_artifacts_for_run(&selected_run_id)?;
        let export = match mode {
            ReplayExportMode::ManifestOnly => {
                self.write_manifest_only_replay(&run, events.clone(), artifacts.clone())?
            }
            ReplayExportMode::Portable => {
                self.write_portable_replay(&run, events.clone(), artifacts.clone())?
            }
        };
        let _ = self.record_event(
            "replay",
            "export",
            EventLevel::Info,
            format!("exported {:?} replay manifest for {}", mode, run.id),
            None,
            None,
            None,
            json!({
                "target_run_id": run.id,
                "mode": mode,
                "manifest_path": export.manifest_path,
                "bundle_root": export.bundle_root,
                "event_count": export.event_count,
                "artifact_count": export.artifact_count,
            }),
        )?;
        Ok(export)
    }

    pub fn artifact_handle(&self, artifact_id: &str) -> Result<ArtifactHandle> {
        self.store
            .get_artifact(artifact_id)?
            .ok_or_else(|| anyhow!("unknown artifact {artifact_id}"))
    }

    pub fn artifact_list(
        &self,
        instance_id: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<ArtifactListPayload> {
        let artifacts = self.store.list_artifacts(instance_id, run_id)?;
        Ok(ArtifactListPayload {
            instance_id: instance_id.map(str::to_string),
            run_id: run_id.map(str::to_string),
            artifacts: artifacts.iter().map(artifact_list_entry).collect(),
        })
    }

    pub fn artifact_verify(&self, artifact_id: &str) -> Result<ArtifactVerifyPayload> {
        let artifact = self.artifact_handle(artifact_id)?;
        let expected_sha256 = artifact.checksum_sha256.clone();
        anyhow::ensure!(
            expected_sha256.is_some(),
            "artifact {artifact_id} is missing stored sha256 metadata"
        );
        let actual_sha256 = sha256_path(Path::new(&artifact.path))?;
        let valid = expected_sha256.as_deref() == Some(actual_sha256.as_str());
        Ok(ArtifactVerifyPayload {
            id: artifact.id,
            path: artifact.path,
            expected_sha256,
            actual_sha256,
            valid,
        })
    }

    pub fn validate_replay_exports(&self, limit: usize) -> Result<Vec<ReplayValidationStatus>> {
        let mut manifests = discover_manifest_paths(self.replay_root())?;
        manifests.sort_by(|left, right| right.1.cmp(&left.1));
        manifests
            .into_iter()
            .take(limit)
            .map(|(manifest_path, _)| self.validate_replay_manifest_path(&manifest_path))
            .collect()
    }

    pub fn defer_tool<T: Serialize>(&self, tool_name: &str, data: T) -> OperationOutcome<T> {
        OperationOutcome::failure(
            OutcomeCode::Unsupported,
            format!("{tool_name} is intentionally deferred beyond Stage 1"),
            data,
        )
    }

    pub fn capture_run(&self) -> CaptureRun {
        self.capture_run.lock().expect("capture run lock").clone()
    }

    pub fn continuity_status(&self) -> ContinuityStatus {
        self.continuity.lock().expect("continuity lock").clone()
    }

    pub fn attach_continuity_status(&self) -> AttachContinuityStatus {
        self.attach_continuity
            .lock()
            .expect("attach continuity lock")
            .clone()
    }

    pub fn daemon_metadata(&self) -> Result<Option<DaemonMetadata>> {
        let path = daemon_metadata_path(self.paths());
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read daemon metadata {}", path.display()))?;
        let metadata = serde_json::from_str(&text)
            .with_context(|| format!("parse daemon metadata {}", path.display()))?;
        Ok(Some(metadata))
    }

    pub fn write_daemon_metadata(&self, bind_addr: &str) -> Result<DaemonMetadata> {
        let metadata = DaemonMetadata {
            bind_addr: bind_addr.to_string(),
            pid: std::process::id(),
            entrypoint: self.entrypoint.clone(),
            started_at: utc_timestamp(),
        };
        let path = daemon_metadata_path(self.paths());
        fs::write(
            &path,
            serde_json::to_vec_pretty(&metadata).context("serialize daemon metadata")?,
        )
        .with_context(|| format!("write daemon metadata {}", path.display()))?;
        Ok(metadata)
    }

    fn active_leases(&self, instance_id: Option<&str>) -> Result<Vec<LeaseRecord>> {
        let now = utc_timestamp();
        self.store.prune_expired_leases(&now)?;
        self.store.list_leases(instance_id, &now)
    }

    fn bootstrap_continuity(&self) -> Result<ContinuityStatus> {
        let continuity_enabled = continuity_enabled(&self.entrypoint);
        let current = self.continuity.lock().expect("continuity lock").clone();
        let capture_run = self.capture_run();
        let now = utc_timestamp();
        self.store.prune_expired_leases(&now)?;
        let recovered_leases = if continuity_enabled {
            self.store
                .list_active_leases_for_holder(&self.operator_id, &now)?
        } else {
            Vec::new()
        };
        let continuity_candidates = recovered_leases
            .iter()
            .map(|lease| lease.resource_id.clone())
            .collect::<BTreeSet<_>>();
        let refreshed_instances = self.refresh_instances()?;
        let stale_instance_ids = refreshed_instances
            .iter()
            .filter(|instance| {
                continuity_candidates.contains(&instance.id)
                    && matches!(
                        instance.status,
                        InstanceStatus::Closed | InstanceStatus::Error
                    )
            })
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();
        let recovered_instance_count = refreshed_instances
            .iter()
            .filter(|instance| {
                continuity_candidates.contains(&instance.id)
                    && matches!(
                        instance.status,
                        InstanceStatus::Running | InstanceStatus::Attached
                    )
            })
            .count();
        Ok(ContinuityStatus {
            continuity_enabled,
            recovered_run: capture_run.status == RunStatus::Active && current.recovered_run,
            reused_operator_id: current.reused_operator_id,
            recovered_run_id: current.recovered_run.then(|| capture_run.id.clone()),
            recovered_lease_count: recovered_leases.len(),
            recovered_instance_count,
            stale_instance_count: stale_instance_ids.len(),
            stale_instance_ids,
        })
    }

    fn lease_holder_id<'a>(&'a self, holder_id: Option<&'a str>) -> &'a str {
        holder_id.unwrap_or(self.operator_id.as_str())
    }

    fn lease_holder_label<'a>(&'a self, holder_id: &str) -> Option<&'a str> {
        if holder_id == self.operator_id {
            Some(self.entrypoint.as_str())
        } else {
            None
        }
    }

    fn update_attach_continuity(&self, status: AttachContinuityStatus) -> Result<()> {
        self.store.upsert_attach_continuity(&status)?;
        let mut current = self
            .attach_continuity
            .lock()
            .expect("attach continuity lock");
        *current = status;
        Ok(())
    }

    fn classify_attach_continuity(&self, instances: &[BrowserInstance]) -> AttachContinuityStatus {
        let mut status = self.attach_continuity_status();
        status.freshness = match status.last_instance_id.as_deref() {
            None => AttachContinuityFreshness::None,
            Some(instance_id) => {
                if let Some(instance) = instances
                    .iter()
                    .find(|candidate| candidate.id == instance_id)
                {
                    if matches!(
                        instance.status,
                        InstanceStatus::Closed | InstanceStatus::Error
                    ) {
                        AttachContinuityFreshness::StaleInstance
                    } else if status.last_browser_ws_url.as_deref()
                        != instance.browser_ws_url.as_deref()
                    {
                        AttachContinuityFreshness::StaleEndpoint
                    } else {
                        AttachContinuityFreshness::Live
                    }
                } else {
                    AttachContinuityFreshness::StaleInstance
                }
            }
        };
        status
    }

    fn require_instance_access(
        &self,
        instance_id: &str,
        holder_id: &str,
        required: RequiredLease,
        action: &str,
    ) -> Result<()> {
        let leases = self.active_leases(Some(instance_id))?;
        if let Some(existing) = leases.iter().find(|lease| {
            lease.holder_id == holder_id && matches!(lease.mode, LeaseMode::Writer)
                || lease.holder_id == holder_id
                    && matches!(required, RequiredLease::Observer)
                    && matches!(lease.mode, LeaseMode::Observer)
        }) {
            if matches!(existing.mode, LeaseMode::Writer)
                || matches!(required, RequiredLease::Observer)
            {
                return Ok(());
            }
        }
        if leases.is_empty() {
            let mode = match required {
                RequiredLease::Writer => LeaseMode::Writer,
                RequiredLease::Observer => LeaseMode::Observer,
            };
            let _ = self.lease_acquire(
                instance_id,
                holder_id,
                self.lease_holder_label(holder_id),
                mode,
                DEFAULT_LEASE_TTL_SECONDS,
            )?;
            return Ok(());
        }
        if matches!(required, RequiredLease::Observer) {
            let _ = self.lease_acquire(
                instance_id,
                holder_id,
                self.lease_holder_label(holder_id),
                LeaseMode::Observer,
                DEFAULT_LEASE_TTL_SECONDS,
            )?;
            return Ok(());
        }
        let writer = leases.iter().find(|lease| lease.mode == LeaseMode::Writer);
        if let Some(writer) = writer {
            if writer.holder_id == holder_id {
                return Ok(());
            }
            let message = format!(
                "{action} requires writer lease for {instance_id}; held by {} until {}",
                writer.holder_id, writer.expires_at
            );
            let _ = self.record_event(
                "lease",
                "conflict",
                EventLevel::Warning,
                &message,
                Some(instance_id),
                None,
                None,
                json!({
                    "action": action,
                    "holder_id": holder_id,
                    "active_writer_holder_id": writer.holder_id,
                    "active_writer_expires_at": writer.expires_at,
                }),
            )?;
            bail!(message);
        }
        if !leases.is_empty() {
            let message = format!(
                "{action} requires an explicit writer lease for {instance_id} while observers are active"
            );
            let _ = self.record_event(
                "lease",
                "conflict",
                EventLevel::Warning,
                &message,
                Some(instance_id),
                None,
                None,
                json!({
                    "action": action,
                    "holder_id": holder_id,
                    "observer_holders": leases
                        .iter()
                        .filter(|lease| lease.mode == LeaseMode::Observer)
                        .map(|lease| lease.holder_id.clone())
                        .collect::<Vec<_>>(),
                }),
            )?;
            bail!(message);
        }
        Ok(())
    }

    fn active_run_id(&self) -> Option<String> {
        let run = self.capture_run();
        (run.status == RunStatus::Active).then_some(run.id)
    }

    fn replay_root(&self) -> PathBuf {
        PathBuf::from(&self.paths().replay_dir)
    }

    fn write_manifest_only_replay(
        &self,
        run: &CaptureRun,
        events: Vec<RuntimeEvent>,
        artifacts: Vec<ArtifactHandle>,
    ) -> Result<ReplayManifestExport> {
        let replay_dir = self.replay_root().join(&run.id).join("manifest_only");
        fs::create_dir_all(&replay_dir)
            .with_context(|| format!("create replay dir {}", replay_dir.display()))?;
        let manifest_path = replay_dir.join("manifest.json");
        let manifest = self.build_replay_manifest(
            run,
            ReplayExportMode::ManifestOnly,
            ReplayBundleMetadata {
                root_path: replay_dir.display().to_string(),
                manifest_path: manifest_path.display().to_string(),
                artifact_root: None,
                staged_atomically: false,
            },
            events,
            artifacts
                .iter()
                .map(|artifact| replay_record_from_artifact(artifact, artifact.path.clone(), false))
                .collect(),
        );
        write_manifest(&manifest_path, &manifest)?;
        Ok(ReplayManifestExport {
            run: run.clone(),
            mode: ReplayExportMode::ManifestOnly,
            manifest_path: manifest_path.display().to_string(),
            bundle_root: replay_dir.display().to_string(),
            event_count: manifest.events.len(),
            artifact_count: manifest.artifacts.len(),
            exported_at: manifest.exported_at,
        })
    }

    fn write_portable_replay(
        &self,
        run: &CaptureRun,
        events: Vec<RuntimeEvent>,
        artifacts: Vec<ArtifactHandle>,
    ) -> Result<ReplayManifestExport> {
        let run_root = self.replay_root().join(&run.id);
        fs::create_dir_all(&run_root)
            .with_context(|| format!("create replay run root {}", run_root.display()))?;
        let final_root = run_root.join("portable");
        let staging_root = run_root.join(format!(
            ".portable-staging-{}",
            utc_timestamp().replace(':', "-")
        ));
        if staging_root.exists() {
            fs::remove_dir_all(&staging_root)
                .with_context(|| format!("remove stale staging {}", staging_root.display()))?;
        }
        fs::create_dir_all(staging_root.join("artifacts"))
            .with_context(|| format!("create replay staging dir {}", staging_root.display()))?;
        let mut records = Vec::with_capacity(artifacts.len());
        for artifact in &artifacts {
            let source_path = PathBuf::from(&artifact.path);
            if !source_path.exists() {
                let _ = fs::remove_dir_all(&staging_root);
                bail!("missing artifact source {}", source_path.display());
            }
            let relative_path = PathBuf::from("artifacts")
                .join(kind_dir(&artifact.kind))
                .join(
                    source_path
                        .file_name()
                        .ok_or_else(|| anyhow!("artifact path missing file name"))?,
                );
            let destination = staging_root.join(&relative_path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create replay artifact dir {}", parent.display()))?;
            }
            let checksum = copy_file_with_sha256(&source_path, &destination)?;
            if let Some(expected) = artifact.checksum_sha256.as_ref() {
                anyhow::ensure!(
                    expected == &checksum,
                    "checksum mismatch while materializing {}",
                    artifact.id
                );
            }
            records.push(replay_record_from_artifact(
                artifact,
                relative_path.to_string_lossy().to_string(),
                true,
            ));
            if let Some(last) = records.last_mut() {
                last.checksum_sha256 = Some(checksum);
            }
        }
        let final_manifest_path = final_root.join("manifest.json");
        let manifest = self.build_replay_manifest(
            run,
            ReplayExportMode::Portable,
            ReplayBundleMetadata {
                root_path: final_root.display().to_string(),
                manifest_path: final_manifest_path.display().to_string(),
                artifact_root: Some(final_root.join("artifacts").display().to_string()),
                staged_atomically: true,
            },
            events,
            records,
        );
        let staging_manifest_path = staging_root.join("manifest.json");
        write_manifest(&staging_manifest_path, &manifest)?;
        promote_portable_bundle(&staging_root, &final_root)?;
        Ok(ReplayManifestExport {
            run: run.clone(),
            mode: ReplayExportMode::Portable,
            manifest_path: final_manifest_path.display().to_string(),
            bundle_root: final_root.display().to_string(),
            event_count: manifest.events.len(),
            artifact_count: manifest.artifacts.len(),
            exported_at: manifest.exported_at,
        })
    }

    fn build_replay_manifest(
        &self,
        run: &CaptureRun,
        mode: ReplayExportMode,
        bundle: ReplayBundleMetadata,
        events: Vec<RuntimeEvent>,
        artifacts: Vec<ReplayArtifactRecord>,
    ) -> ReplayManifest {
        ReplayManifest {
            schema_version: 2,
            exported_at: utc_timestamp(),
            mode,
            bundle,
            run: run.clone(),
            inspection_modes: inspection_modes(),
            events,
            artifacts,
        }
    }

    fn validate_replay_manifest_path(
        &self,
        manifest_path: &Path,
    ) -> Result<ReplayValidationStatus> {
        let bytes = fs::read(manifest_path)
            .with_context(|| format!("read replay manifest {}", manifest_path.display()))?;
        let manifest: ReplayManifest =
            serde_json::from_slice(&bytes).context("deserialize replay manifest")?;
        let artifact_ids = manifest
            .artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect::<std::collections::HashSet<_>>();
        let mut missing_files = 0;
        let mut checksum_mismatches = 0;
        let mut provenance_errors = 0;
        for artifact in &manifest.artifacts {
            let artifact_path = if artifact.materialized {
                PathBuf::from(&manifest.bundle.root_path).join(&artifact.path)
            } else {
                PathBuf::from(&artifact.path)
            };
            if !artifact_path.exists() {
                missing_files += 1;
                continue;
            }
            if let Some(expected) = artifact.checksum_sha256.as_ref() {
                let actual = sha256_path(&artifact_path)?;
                if &actual != expected {
                    checksum_mismatches += 1;
                }
            }
            if let Some(source_artifact_id) = artifact.provenance.source_artifact_id.as_ref() {
                if !artifact_ids.contains(source_artifact_id) {
                    provenance_errors += 1;
                }
            }
        }
        Ok(ReplayValidationStatus {
            run_id: manifest.run.id,
            mode: manifest.mode,
            manifest_path: manifest_path.display().to_string(),
            artifact_count: manifest.artifacts.len(),
            missing_files,
            checksum_mismatches,
            provenance_errors,
            ok: missing_files == 0 && checksum_mismatches == 0 && provenance_errors == 0,
        })
    }

    fn record_event(
        &self,
        category: &str,
        action: &str,
        level: EventLevel,
        message: impl Into<String>,
        instance_id: Option<&str>,
        tab_id: Option<&str>,
        artifact_id: Option<&str>,
        data: Value,
    ) -> Result<Option<RuntimeEvent>> {
        let Some(run_id) = self.active_run_id() else {
            return Ok(None);
        };
        let message = message.into();
        Ok(Some(self.store.append_event(
            &run_id,
            category,
            action,
            level,
            &message,
            instance_id,
            tab_id,
            artifact_id,
            data,
        )?))
    }

    fn record_artifact_event<T: ArtifactEventPayload>(
        &self,
        category: &str,
        action: &str,
        message: &str,
        payload: &T,
        data: Value,
    ) -> Result<Option<RuntimeEvent>> {
        self.record_event(
            category,
            action,
            EventLevel::Info,
            message,
            Some(payload.instance_id()),
            Some(payload.tab_id()),
            Some(payload.artifact_id()),
            merge_artifact_data(payload.artifact_handle(), data),
        )
    }

    fn ensure_default_profiles(&self) -> Result<()> {
        for channel in discover_installations()
            .into_iter()
            .filter(|candidate| candidate.installed)
            .map(|candidate| candidate.channel)
        {
            let _ = self.ensure_profile(&channel)?;
        }
        Ok(())
    }

    fn ensure_profile(&self, channel: &BrowserChannel) -> Result<ManagedProfile> {
        let profile = ManagedProfile {
            id: StableId::new(IdKind::Profile, channel.as_str()).into_string(),
            name: format!("{} default", channel.as_str()),
            channel: channel.clone(),
            path: self
                .store
                .profile_root()
                .join(channel.as_str())
                .display()
                .to_string(),
        };
        self.store.upsert_profile(&profile)?;
        Ok(profile)
    }

    fn find_attached_instance_seed(
        &self,
        debug_http_url: &str,
        cdp_url: &str,
    ) -> Result<AttachSeedResolution> {
        let attached = self
            .store
            .list_instances()?
            .into_iter()
            .filter(|instance| instance.mode == InstanceMode::Attached)
            .collect::<Vec<_>>();
        if let Some(instance) = attached
            .iter()
            .find(|instance| instance.debug_http_url == debug_http_url)
            .cloned()
        {
            return Ok(AttachSeedResolution {
                kind: AttachResolutionKind::DebugHttpUrl,
                instance: Some(instance),
            });
        }
        if let Some(instance) = attached
            .iter()
            .find(|instance| instance.browser_ws_url.as_deref() == Some(cdp_url))
            .cloned()
        {
            return Ok(AttachSeedResolution {
                kind: AttachResolutionKind::BrowserWsUrl,
                instance: Some(instance),
            });
        }
        Ok(AttachSeedResolution {
            kind: AttachResolutionKind::NewInstance,
            instance: None,
        })
    }

    fn sync_tabs(&self, instance_id: &str) -> Result<Vec<BrowserTab>> {
        let instance = self.require_instance(instance_id)?;
        let tabs = live_tabs_for_instance(&instance)?;
        self.store.replace_tabs(instance_id, &tabs)?;
        self.store.list_tabs(Some(instance_id))
    }

    fn require_instance(&self, instance_id: &str) -> Result<BrowserInstance> {
        self.refresh_instances()?;
        self.store
            .get_instance(instance_id)?
            .ok_or_else(|| anyhow!("unknown instance {instance_id}"))
    }

    fn require_tab(&self, tab_id: &str) -> Result<BrowserTab> {
        self.store
            .get_tab(tab_id)?
            .ok_or_else(|| anyhow!("unknown tab {tab_id}"))
    }

    fn connect_tab_session(&self, tab_id: &str) -> Result<(BrowserTab, CdpSession, bool)> {
        let tab = self.require_tab(tab_id)?;
        match CdpSession::connect(&tab.websocket_url) {
            Ok(session) => Ok((tab, session, false)),
            Err(initial_error) => {
                let refreshed = self
                    .sync_tabs(&tab.instance_id)?
                    .into_iter()
                    .find(|candidate| {
                        candidate.id == tab.id || candidate.target_id == tab.target_id
                    })
                    .ok_or_else(|| anyhow!("unknown tab {tab_id} after websocket refresh"))?;
                let session = CdpSession::connect(&refreshed.websocket_url).with_context(|| {
                    format!(
                        "reconnect tab websocket for {tab_id} after refresh from {}",
                        initial_error
                    )
                })?;
                let _ = self.record_event(
                    "tab",
                    "session_recovered",
                    EventLevel::Info,
                    format!("recovered tab websocket for {tab_id} after refresh"),
                    Some(&refreshed.instance_id),
                    Some(&refreshed.id),
                    None,
                    json!({
                        "previous_websocket_url": tab.websocket_url,
                        "recovered_websocket_url": refreshed.websocket_url,
                        "target_id": refreshed.target_id,
                    }),
                )?;
                Ok((refreshed, session, true))
            }
        }
    }

    fn refresh_instances(&self) -> Result<Vec<BrowserInstance>> {
        let mut refreshed = Vec::new();
        for mut instance in self.store.list_instances()? {
            let previous_status = instance.status.clone();
            let previous_error = instance.last_error.clone();
            let state = match host_port(&instance.debug_http_url).and_then(|(host, port)| {
                wait_for_debug_endpoint(&host, port, Duration::from_millis(300))
            }) {
                Ok(metadata) => {
                    instance.status = if instance.mode == InstanceMode::Attached {
                        InstanceStatus::Attached
                    } else {
                        InstanceStatus::Running
                    };
                    instance.browser_ws_url = Some(metadata.websocket_debugger_url);
                    instance.last_error = None;
                    instance.updated_at = utc_timestamp();
                    instance
                }
                Err(error) => {
                    instance.status = InstanceStatus::Closed;
                    instance.last_error = Some(error.to_string());
                    instance.updated_at = utc_timestamp();
                    instance
                }
            };
            self.store.upsert_instance(&state)?;
            if state.status != previous_status || state.last_error != previous_error {
                let level =
                    if matches!(state.status, InstanceStatus::Closed | InstanceStatus::Error) {
                        EventLevel::Warning
                    } else {
                        EventLevel::Info
                    };
                let message = if let Some(error) = &state.last_error {
                    format!("instance {} refresh observed: {error}", state.id)
                } else {
                    format!("instance {} refresh observed {:?}", state.id, state.status)
                };
                let _ = self.record_event(
                    "instance",
                    "refresh",
                    level,
                    message,
                    Some(&state.id),
                    None,
                    None,
                    json!({
                        "status": state.status,
                        "last_error": state.last_error,
                    }),
                )?;
            }
            refreshed.push(state);
        }
        Ok(refreshed)
    }
}

pub fn lease_coverage_matrix() -> Vec<LeaseCoverageEntry> {
    vec![
        lease_coverage_entry(
            "browser_health",
            Some("pengu-mesh health"),
            Some("browser_health"),
            Some("GET"),
            Some("/health"),
            LeaseDisposition::OutsideModel,
            "runtime-wide readiness summary; not a holder-scoped browser or artifact operation",
        ),
        lease_coverage_entry(
            "browser_doctor",
            Some("pengu-mesh-doctor -- --json"),
            Some("browser_doctor"),
            Some("GET"),
            Some("/doctor"),
            LeaseDisposition::OutsideModel,
            "runtime-wide diagnostics across tools, permissions, and persisted state",
        ),
        lease_coverage_entry(
            "diagnose",
            Some("pengu-mesh diagnose"),
            Some("diagnose"),
            Some("GET"),
            Some("/diagnose"),
            LeaseDisposition::OutsideModel,
            "side-effect-free host readiness and remediation inventory for agent self-enablement",
        ),
        lease_coverage_entry(
            "capability_preflight",
            Some("pengu-mesh capability-preflight --capability ..."),
            Some("capability_preflight"),
            Some("GET"),
            Some("/capabilities/preflight"),
            LeaseDisposition::OutsideModel,
            "read-only policy preflight reports capability decisions and grant hints without touching live browser state",
        ),
        lease_coverage_entry(
            "host_access_status",
            Some("pengu-mesh host-access-status"),
            Some("host_access_status"),
            Some("GET"),
            Some("/host/access/status"),
            LeaseDisposition::OutsideModel,
            "host access status reports machine-level readiness and permissions outside browser instance coordination",
        ),
        lease_coverage_entry(
            "host_access_setup",
            Some("pengu-mesh host-access-setup --mode audit"),
            Some("host_access_setup"),
            Some("POST"),
            Some("/host/access/setup"),
            LeaseDisposition::OutsideModel,
            "host access setup configures machine-level permissions and remains outside per-instance leases",
        ),
        lease_coverage_entry(
            "profile_list",
            Some("pengu-mesh profile-list"),
            Some("profile_list"),
            Some("GET"),
            Some("/profiles"),
            LeaseDisposition::OutsideModel,
            "managed profile inventory is local runtime metadata, not shared browser control",
        ),
        lease_coverage_entry(
            "profile_create",
            Some("pengu-mesh profile-create --name ..."),
            Some("profile_create"),
            Some("POST"),
            Some("/profiles/create"),
            LeaseDisposition::OutsideModel,
            "profile creation mutates local profile metadata before any instance exists",
        ),
        lease_coverage_entry(
            "instance_list",
            Some("pengu-mesh instance-list"),
            Some("instance_list"),
            Some("GET"),
            Some("/instances"),
            LeaseDisposition::OutsideModel,
            "aggregate instance inventory refreshes readiness across the runtime instead of claiming a single instance",
        ),
        lease_coverage_entry(
            "instance_start",
            Some("pengu-mesh instance-start --name ..."),
            Some("instance_start"),
            Some("POST"),
            Some("/instances/start"),
            LeaseDisposition::WriterRequired,
            "launching a managed browser mints instance state and auto-acquires the writer lease",
        ),
        lease_coverage_entry(
            "instance_attach",
            Some("pengu-mesh instance-attach --name ... --cdp-url ..."),
            Some("instance_attach"),
            Some("POST"),
            Some("/instances/attach"),
            LeaseDisposition::WriterRequired,
            "attach mutates instance identity and continuity state and requires writer ownership on reused instances",
        ),
        lease_coverage_entry(
            "instance_stop",
            Some("pengu-mesh instance-stop --instance-id ..."),
            Some("instance_stop"),
            Some("POST"),
            Some("/instances/stop"),
            LeaseDisposition::WriterRequired,
            "stopping a managed browser mutates live browser state and releases the writer lease",
        ),
        lease_coverage_entry(
            "lease_status",
            Some("pengu-mesh lease-status --instance-id ..."),
            Some("lease_status"),
            Some("GET"),
            Some("/leases"),
            LeaseDisposition::OutsideModel,
            "lease inspection is the coordination control plane itself",
        ),
        lease_coverage_entry(
            "lease_acquire",
            Some("pengu-mesh lease-acquire --instance-id ... --holder-id ..."),
            Some("lease_acquire"),
            Some("POST"),
            Some("/leases/acquire"),
            LeaseDisposition::OutsideModel,
            "lease acquisition defines holder access and is intentionally outside holder enforcement",
        ),
        lease_coverage_entry(
            "lease_release",
            Some("pengu-mesh lease-release --instance-id ... --holder-id ..."),
            Some("lease_release"),
            Some("POST"),
            Some("/leases/release"),
            LeaseDisposition::OutsideModel,
            "lease release is a coordination-plane operation, not a protected browser action",
        ),
        lease_coverage_entry(
            "lease_transfer",
            Some("pengu-mesh lease-transfer --instance-id ..."),
            Some("lease_transfer"),
            Some("POST"),
            Some("/leases/transfer"),
            LeaseDisposition::OutsideModel,
            "writer transfer rewrites lease ownership and intentionally administers the model",
        ),
        lease_coverage_entry(
            "tab_list",
            Some("pengu-mesh tab-list --instance-id ..."),
            Some("tab_list"),
            Some("GET"),
            Some("/tabs"),
            LeaseDisposition::ObserverRequired,
            "tab inventory refresh reads shared browser state for a live instance",
        ),
        lease_coverage_entry(
            "tab_list_actions",
            Some("pengu-mesh tab-list-actions --instance-id ... --tab-id ..."),
            Some("tab_list_actions"),
            Some("GET"),
            Some("/tabs/actions"),
            LeaseDisposition::ObserverRequired,
            "tab action contracts read shared tab state to describe safe CDP affordances before mutation",
        ),
        lease_coverage_entry(
            "browser_surface_list",
            Some("pengu-mesh browser-surface-list --instance-id ..."),
            Some("browser_surface_list"),
            Some("GET"),
            Some("/browser/surfaces"),
            LeaseDisposition::ObserverRequired,
            "native browser surface inventory reads shared host/browser state for an attached instance",
        ),
        lease_coverage_entry(
            "browser_surface_list_actions",
            Some("pengu-mesh browser-surface-list-actions --instance-id ... --surface-id ..."),
            Some("browser_surface_list_actions"),
            Some("GET"),
            Some("/browser/surfaces/actions"),
            LeaseDisposition::ObserverRequired,
            "browser surface action contracts read shared host/browser state to describe action affordances safely before mutation",
        ),
        lease_coverage_entry(
            "browser_surface_snapshot",
            Some("pengu-mesh browser-surface-snapshot --instance-id ..."),
            Some("browser_surface_snapshot"),
            Some("POST"),
            Some("/browser/surfaces/snapshot"),
            LeaseDisposition::ObserverRequired,
            "native browser surface snapshot reads host/browser state and emits evidence artifacts",
        ),
        lease_coverage_entry(
            "browser_surface_action",
            Some("pengu-mesh browser-surface-action --instance-id ... --action press"),
            Some("browser_surface_action"),
            Some("POST"),
            Some("/browser/surfaces/action"),
            LeaseDisposition::WriterRequired,
            "native browser surface actions can mutate browser-native controls and require writer ownership",
        ),
        lease_coverage_entry(
            "tab_open",
            Some("pengu-mesh tab-open --instance-id ... --url ..."),
            Some("tab_open"),
            Some("POST"),
            Some("/tabs/open"),
            LeaseDisposition::WriterRequired,
            "opening a tab mutates the browser target set",
        ),
        lease_coverage_entry(
            "tab_close",
            Some("pengu-mesh tab-close --tab-id ..."),
            Some("tab_close"),
            Some("POST"),
            Some("/tabs/close"),
            LeaseDisposition::WriterRequired,
            "closing a tab mutates the browser target set",
        ),
        lease_coverage_entry(
            "tab_action",
            Some("pengu-mesh tab-action --tab-id ... --kind ..."),
            Some("tab_action"),
            Some("POST"),
            Some("/tabs/action"),
            LeaseDisposition::WriterRequired,
            "typed tab actions mutate focus, DOM state, navigation, or form inputs",
        ),
        lease_coverage_entry(
            "tab_snapshot",
            Some("pengu-mesh tab-snapshot --tab-id ..."),
            Some("tab_snapshot"),
            Some("POST"),
            Some("/tabs/snapshot"),
            LeaseDisposition::ObserverRequired,
            "snapshot capture reads shared page state and emits an artifact",
        ),
        lease_coverage_entry(
            "tab_text",
            Some("pengu-mesh tab-text --tab-id ..."),
            Some("tab_text"),
            Some("POST"),
            Some("/tabs/text"),
            LeaseDisposition::ObserverRequired,
            "text extraction reads shared page state and emits an artifact",
        ),
        lease_coverage_entry(
            "tab_screenshot",
            Some("pengu-mesh tab-screenshot --tab-id ..."),
            Some("tab_screenshot"),
            Some("POST"),
            Some("/tabs/screenshot"),
            LeaseDisposition::ObserverRequired,
            "screenshot capture reads shared page state and emits an artifact",
        ),
        lease_coverage_entry(
            "tab_pdf",
            Some("pengu-mesh tab-pdf --tab-id ..."),
            Some("tab_pdf"),
            Some("POST"),
            Some("/tabs/pdf"),
            LeaseDisposition::ObserverRequired,
            "PDF capture reads shared page state and emits an artifact",
        ),
        lease_coverage_entry(
            "artifact_crop",
            Some("pengu-mesh artifact-crop --artifact-id ..."),
            Some("artifact_crop"),
            Some("POST"),
            Some("/artifacts/crop"),
            LeaseDisposition::ObserverRequired,
            "deriving a crop reads shared artifact state for a live instance context",
        ),
        lease_coverage_entry(
            "artifact_crop_grid",
            Some("pengu-mesh artifact-crop-grid --artifact-id ..."),
            Some("artifact_crop_grid"),
            Some("POST"),
            Some("/artifacts/crop-grid"),
            LeaseDisposition::ObserverRequired,
            "batch crop derivation reads shared artifact state for a live instance context",
        ),
        lease_coverage_entry(
            "artifact_list",
            Some("pengu-mesh artifact-list --instance-id ..."),
            Some("artifact_list"),
            Some("GET"),
            Some("/artifacts"),
            LeaseDisposition::OutsideModel,
            "artifact inventory is immutable metadata lookup with no live browser contention",
        ),
        lease_coverage_entry(
            "artifact_handle",
            None,
            None,
            Some("GET"),
            Some("/artifacts/:id"),
            LeaseDisposition::OutsideModel,
            "artifact handle resolution is immutable metadata lookup with no live browser contention",
        ),
        lease_coverage_entry(
            "artifact_verify",
            Some("pengu-mesh artifact-verify --artifact-id ..."),
            Some("artifact_verify"),
            Some("GET"),
            Some("/artifacts/verify"),
            LeaseDisposition::OutsideModel,
            "artifact verification re-reads immutable evidence on disk without touching live browser state",
        ),
        lease_coverage_entry(
            "capture_start_recording",
            Some("pengu-mesh capture-start-recording"),
            Some("capture_start_recording"),
            Some("POST"),
            Some("/capture/start"),
            LeaseDisposition::OutsideModel,
            "capture run state is runtime-owned observability metadata, not shared browser control",
        ),
        lease_coverage_entry(
            "capture_stop_recording",
            Some("pengu-mesh capture-stop-recording"),
            Some("capture_stop_recording"),
            Some("POST"),
            Some("/capture/stop"),
            LeaseDisposition::OutsideModel,
            "capture run shutdown finalizes observability metadata without mutating browser state",
        ),
        lease_coverage_entry(
            "run_list",
            Some("pengu-mesh run-list"),
            Some("run_list"),
            Some("GET"),
            Some("/runs"),
            LeaseDisposition::OutsideModel,
            "run inventory reads immutable replay metadata rather than shared live browser state",
        ),
        lease_coverage_entry(
            "scenario_list",
            Some("pengu-mesh scenario-list"),
            Some("scenario_list"),
            Some("GET"),
            Some("/scenarios"),
            LeaseDisposition::OutsideModel,
            "scenario inventory reads stored metrics metadata without coordinating live browser state",
        ),
        lease_coverage_entry(
            "scenario_run_detail",
            Some("pengu-mesh scenario-run-detail --run-id ..."),
            Some("scenario_run_detail"),
            Some("GET"),
            Some("/scenarios/:id"),
            LeaseDisposition::OutsideModel,
            "scenario detail reads stored metrics and assertions without mutating live browser state",
        ),
        lease_coverage_entry(
            "events_tail",
            Some("pengu-mesh events-tail"),
            Some("events_tail"),
            Some("GET"),
            Some("/events"),
            LeaseDisposition::OutsideModel,
            "event tailing reads append-only observability state rather than coordinating browser access",
        ),
        lease_coverage_entry(
            "replay_export",
            Some("pengu-mesh replay-export"),
            Some("replay_export"),
            Some("POST"),
            Some("/replay/export"),
            LeaseDisposition::OutsideModel,
            "replay export packages persisted evidence after capture rather than reading shared live tabs",
        ),
        lease_coverage_entry(
            "trace_capture",
            Some("pengu-mesh trace-capture --tab-id ..."),
            Some("trace_capture"),
            Some("POST"),
            Some("/trace/capture"),
            LeaseDisposition::ObserverRequired,
            "trace capture reads a live tab and emits bounded evidence artifacts",
        ),
        lease_coverage_entry(
            "recording_capture",
            Some("pengu-mesh recording-capture --tab-id ..."),
            Some("recording_capture"),
            Some("POST"),
            Some("/recording/capture"),
            LeaseDisposition::ObserverRequired,
            "recording capture reads a live tab repeatedly and emits bounded evidence artifacts",
        ),
        lease_coverage_entry(
            "tool_catalog",
            None,
            None,
            Some("GET"),
            Some("/tools"),
            LeaseDisposition::OutsideModel,
            "generic catalog route enumerates tool contracts rather than touching runtime state",
        ),
        lease_coverage_entry(
            "generic_tool_dispatch",
            None,
            None,
            Some("POST"),
            Some("/tools/:tool"),
            LeaseDisposition::OutsideModel,
            "generic dispatch route delegates to the underlying tool-specific lease policy",
        ),
    ]
}

fn lease_coverage_entry(
    operation: &str,
    cli_command: Option<&str>,
    mcp_tool: Option<&str>,
    http_method: Option<&str>,
    http_route: Option<&str>,
    disposition: LeaseDisposition,
    rationale: &str,
) -> LeaseCoverageEntry {
    LeaseCoverageEntry {
        operation: operation.to_string(),
        cli_command: cli_command.map(str::to_string),
        mcp_tool: mcp_tool.map(str::to_string),
        http_method: http_method.map(str::to_string),
        http_route: http_route.map(str::to_string),
        disposition,
        rationale: rationale.to_string(),
    }
}

fn native_surface_scope_id(root_surface_id: Option<&str>) -> String {
    let suffix = root_surface_id
        .map(|value| {
            value
                .chars()
                .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "root".to_string());
    format!("native_surface_{suffix}")
}

pub(crate) fn surface_action_contracts(
    surface: &BrowserSurfaceDescriptor,
    host_access: &HostAccessStatus,
) -> Vec<BrowserSurfaceActionContract> {
    let channel_service = surface_host_access_service(&surface.channel);
    surface
        .actions
        .iter()
        .map(|action| surface_action_contract(action, channel_service.clone(), host_access))
        .collect()
}

fn surface_action_contract(
    action_name: &str,
    channel_service: HostAccessService,
    host_access: &HostAccessStatus,
) -> BrowserSurfaceActionContract {
    let Some(action_kind) = parse_surface_action_name(action_name) else {
        return BrowserSurfaceActionContract {
            action: action_name.to_string(),
            available: false,
            required_permissions: Vec::new(),
            expected_interference_level: pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
            detail:
                "unrecognized action; pengu mesh does not yet have execution-path metadata for this action"
                    .to_string(),
            execution_paths: Vec::new(),
        };
    };
    let runtime_supported = surface_action_runtime_supported(&action_kind);
    let execution_paths = surface_action_paths(action_kind.clone(), channel_service, host_access);
    let execution_paths = if runtime_supported {
        execution_paths
    } else {
        catalog_only_surface_action_paths(execution_paths)
    };
    let preferred =
        execution_paths
            .first()
            .cloned()
            .unwrap_or_else(|| BrowserSurfaceActionPathContract {
                execution_channel: ExecutionChannel::AxDirect,
                available: false,
                required_permissions: Vec::new(),
                interference_level: pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
                detail: "no execution path is currently defined for this action".to_string(),
            });
    let available = runtime_supported && execution_paths.iter().any(|path| path.available);
    let fallback_count = execution_paths.len().saturating_sub(1);
    BrowserSurfaceActionContract {
        action: action_name.to_string(),
        available,
        required_permissions: preferred.required_permissions.clone(),
        expected_interference_level: preferred.interference_level.clone(),
        detail: if runtime_supported {
            if fallback_count == 0 {
                format!(
                    "preferred path {} requires {}",
                    preferred.execution_channel.as_str(),
                    permission_labels(&preferred.required_permissions)
                )
            } else {
                format!(
                    "preferred path {} requires {}; {} fallback paths are defined",
                    preferred.execution_channel.as_str(),
                    permission_labels(&preferred.required_permissions),
                    fallback_count
                )
            }
        } else if fallback_count == 0 {
            format!(
                "recognized accessibility action; preferred path {} would require {}; runtime invocation is not yet implemented",
                preferred.execution_channel.as_str(),
                permission_labels(&preferred.required_permissions)
            )
        } else {
            format!(
                "recognized accessibility action; preferred path {} would require {}; {} fallback paths are cataloged, but runtime invocation is not yet implemented",
                preferred.execution_channel.as_str(),
                permission_labels(&preferred.required_permissions),
                fallback_count
            )
        },
        execution_paths,
    }
}

fn surface_action_runtime_supported(action: &SurfaceActionKind) -> bool {
    !matches!(
        action,
        SurfaceActionKind::Scroll
            | SurfaceActionKind::Increment
            | SurfaceActionKind::Decrement
            | SurfaceActionKind::ShowMenu
            | SurfaceActionKind::Pick
            | SurfaceActionKind::Raise
            | SurfaceActionKind::Cancel
    )
}

fn catalog_only_surface_action_paths(
    execution_paths: Vec<BrowserSurfaceActionPathContract>,
) -> Vec<BrowserSurfaceActionPathContract> {
    execution_paths
        .into_iter()
        .map(|mut path| {
            path.available = false;
            path.detail = format!(
                "recognized accessibility action; runtime invocation is not yet implemented; {}",
                path.detail
            );
            path
        })
        .collect()
}

pub(crate) fn tab_action_contracts(tab: &BrowserTab) -> Vec<TabActionContract> {
    let available = !tab.websocket_url.trim().is_empty();
    let unavailable_detail =
        "tab websocket URL is missing; reconnect the tab before issuing CDP actions";
    let writer_detail = |kind: &str, requirement: &str| -> String {
        if available {
            format!(
                "available through tab_action --kind {kind} over CDP; requires writer lease{requirement}"
            )
        } else {
            unavailable_detail.to_string()
        }
    };
    let observer_detail = |command: &str, note: &str| -> String {
        if available {
            format!("available through {command}; requires observer lease{note}")
        } else {
            unavailable_detail.to_string()
        }
    };
    vec![
        TabActionContract {
            kind: "navigate".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "navigate",
                " and --url <target>; optional --timeout-ms <milliseconds>",
            ),
        },
        TabActionContract {
            kind: "evaluate".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail("evaluate", " and --expression <javascript>"),
        },
        TabActionContract {
            kind: "click".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "click",
                " plus --ref <node-ref> or --selector <css-selector>",
            ),
        },
        TabActionContract {
            kind: "focus".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "focus",
                " plus --ref <node-ref> or --selector <css-selector>",
            ),
        },
        TabActionContract {
            kind: "hover".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "hover",
                " plus --ref <node-ref> or --selector <css-selector>",
            ),
        },
        TabActionContract {
            kind: "fill".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "fill",
                " plus --text <text> and --ref <node-ref> or --selector <css-selector>",
            ),
        },
        TabActionContract {
            kind: "type".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "type",
                " plus --text <text> and --ref <node-ref> or --selector <css-selector>",
            ),
        },
        TabActionContract {
            kind: "press".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail("press", " and --key <key>"),
        },
        TabActionContract {
            kind: "select".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: writer_detail(
                "select",
                " plus --value <option> and --ref <node-ref> or --selector <css-selector>",
            ),
        },
        TabActionContract {
            kind: "snapshot".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: observer_detail("pengu-mesh tab-snapshot --tab-id ...", ""),
        },
        TabActionContract {
            kind: "text".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: observer_detail("pengu-mesh tab-text --tab-id ...", ""),
        },
        TabActionContract {
            kind: "screenshot".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: observer_detail(
                "pengu-mesh tab-screenshot --tab-id ...",
                " and optional --full-page",
            ),
        },
        TabActionContract {
            kind: "pdf".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: observer_detail("pengu-mesh tab-pdf --tab-id ...", ""),
        },
        TabActionContract {
            kind: "trace".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: observer_detail("pengu-mesh trace-capture --tab-id ...", ""),
        },
        TabActionContract {
            kind: "recording".to_string(),
            available,
            required_permissions: Vec::new(),
            detail: observer_detail("pengu-mesh recording-capture --tab-id ...", ""),
        },
    ]
}

fn surface_action_paths(
    action: SurfaceActionKind,
    channel_service: HostAccessService,
    host_access: &HostAccessStatus,
) -> Vec<BrowserSurfaceActionPathContract> {
    match action {
        SurfaceActionKind::Press | SurfaceActionKind::Focus | SurfaceActionKind::SetValue => vec![
            surface_action_path(
                ExecutionChannel::AxDirect,
                vec![HostAccessService::Accessibility],
                pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
                host_access,
                "preferred direct Accessibility path",
            ),
            surface_action_path(
                ExecutionChannel::AppleEventsActivation,
                vec![HostAccessService::Accessibility, channel_service],
                pengu_mesh_shared::InterferenceLevel::AppTakeover,
                host_access,
                "fallback path activates the app before reusing Accessibility",
            ),
        ],
        SurfaceActionKind::Confirm => vec![
            surface_action_path(
                ExecutionChannel::AxDirect,
                vec![HostAccessService::Accessibility],
                pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
                host_access,
                "preferred direct Accessibility confirmation path",
            ),
            surface_action_path(
                ExecutionChannel::AppleEventsActivation,
                vec![HostAccessService::Accessibility, channel_service.clone()],
                pengu_mesh_shared::InterferenceLevel::AppTakeover,
                host_access,
                "fallback path activates the app before reusing Accessibility",
            ),
            surface_action_path(
                ExecutionChannel::AppScopedKeyPost,
                vec![HostAccessService::Accessibility],
                pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
                host_access,
                "fallback path posts a scoped return key through Accessibility",
            ),
            surface_action_path(
                ExecutionChannel::GlobalTakeover,
                vec![HostAccessService::ListenEvent, channel_service],
                pengu_mesh_shared::InterferenceLevel::GlobalTakeover,
                host_access,
                "fallback path takes over global keyboard focus through System Events",
            ),
        ],
        SurfaceActionKind::KeySequence => vec![
            surface_action_path(
                ExecutionChannel::AppScopedKeyPost,
                vec![HostAccessService::Accessibility],
                pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
                host_access,
                "preferred path posts the requested key sequence through Accessibility",
            ),
            surface_action_path(
                ExecutionChannel::GlobalTakeover,
                vec![HostAccessService::ListenEvent, channel_service],
                pengu_mesh_shared::InterferenceLevel::GlobalTakeover,
                host_access,
                "fallback path takes over global keyboard focus through System Events",
            ),
        ],
        SurfaceActionKind::Scroll
        | SurfaceActionKind::Increment
        | SurfaceActionKind::Decrement
        | SurfaceActionKind::Pick
        | SurfaceActionKind::Cancel => vec![surface_action_path(
            ExecutionChannel::AxDirect,
            vec![HostAccessService::Accessibility],
            pengu_mesh_shared::InterferenceLevel::BackgroundSafe,
            host_access,
            "direct Accessibility path for background-safe action",
        )],
        SurfaceActionKind::ShowMenu | SurfaceActionKind::Raise => vec![surface_action_path(
            ExecutionChannel::AxDirect,
            vec![HostAccessService::Accessibility],
            pengu_mesh_shared::InterferenceLevel::AppTakeover,
            host_access,
            "direct Accessibility path; action may take over app focus",
        )],
    }
}

fn surface_action_path(
    execution_channel: ExecutionChannel,
    required_permissions: Vec<HostAccessService>,
    interference_level: pengu_mesh_shared::InterferenceLevel,
    host_access: &HostAccessStatus,
    detail: &str,
) -> BrowserSurfaceActionPathContract {
    let available = execution_channel_status(host_access, execution_channel.clone())
        .map(|status| status.available)
        .unwrap_or(false);
    let detail = execution_channel_status(host_access, execution_channel.clone())
        .map(|status| format!("{detail}; {}", status.detail))
        .unwrap_or_else(|| format!("{detail}; execution-channel readiness is unknown"));
    BrowserSurfaceActionPathContract {
        execution_channel,
        available,
        required_permissions,
        interference_level,
        detail,
    }
}

fn surface_host_access_service(channel: &BrowserChannel) -> HostAccessService {
    match channel {
        BrowserChannel::ChromeDev => HostAccessService::AppleEventsChromeDev,
        BrowserChannel::Chrome => HostAccessService::AppleEventsChrome,
        BrowserChannel::Chromium => HostAccessService::AppleEventsChromium,
    }
}

fn parse_surface_action_name(value: &str) -> Option<SurfaceActionKind> {
    match value {
        "press" => Some(SurfaceActionKind::Press),
        "focus" => Some(SurfaceActionKind::Focus),
        "confirm" => Some(SurfaceActionKind::Confirm),
        "set_value" => Some(SurfaceActionKind::SetValue),
        "key_sequence" => Some(SurfaceActionKind::KeySequence),
        "scroll" => Some(SurfaceActionKind::Scroll),
        "increment" => Some(SurfaceActionKind::Increment),
        "decrement" => Some(SurfaceActionKind::Decrement),
        "show_menu" => Some(SurfaceActionKind::ShowMenu),
        "pick" => Some(SurfaceActionKind::Pick),
        "raise" => Some(SurfaceActionKind::Raise),
        "cancel" => Some(SurfaceActionKind::Cancel),
        _ => None,
    }
}

fn permission_labels(permissions: &[HostAccessService]) -> String {
    if permissions.is_empty() {
        return "no additional host permissions".to_string();
    }
    permissions
        .iter()
        .map(|permission| permission.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

trait ArtifactEventPayload {
    fn instance_id(&self) -> &str;
    fn tab_id(&self) -> &str;
    fn artifact_id(&self) -> &str;
    fn artifact_handle(&self) -> &ArtifactHandle;
}

impl ArtifactEventPayload for SnapshotPayload {
    fn instance_id(&self) -> &str {
        &self.tab.instance_id
    }

    fn tab_id(&self) -> &str {
        &self.tab.id
    }

    fn artifact_id(&self) -> &str {
        &self.artifact.id
    }

    fn artifact_handle(&self) -> &ArtifactHandle {
        &self.artifact
    }
}

impl ArtifactEventPayload for TextPayload {
    fn instance_id(&self) -> &str {
        &self.tab.instance_id
    }

    fn tab_id(&self) -> &str {
        &self.tab.id
    }

    fn artifact_id(&self) -> &str {
        &self.artifact.id
    }

    fn artifact_handle(&self) -> &ArtifactHandle {
        &self.artifact
    }
}

impl ArtifactEventPayload for ArtifactPayload {
    fn instance_id(&self) -> &str {
        &self.tab.instance_id
    }

    fn tab_id(&self) -> &str {
        &self.tab.id
    }

    fn artifact_id(&self) -> &str {
        &self.artifact.id
    }

    fn artifact_handle(&self) -> &ArtifactHandle {
        &self.artifact
    }
}

fn artifact_list_entry(artifact: &ArtifactHandle) -> ArtifactListEntry {
    ArtifactListEntry {
        id: artifact.id.clone(),
        kind: artifact.kind.clone(),
        path: artifact.path.clone(),
        sha256: artifact.checksum_sha256.clone(),
        size_bytes: artifact.bytes,
        created_at: artifact.created_at.clone(),
    }
}

fn merge_artifact_data(artifact: &ArtifactHandle, extra: Value) -> Value {
    let mut merged = json!({
        "artifact_id": artifact.id,
        "artifact_kind": artifact.kind,
        "artifact_path": artifact.path,
        "mime_type": artifact.mime_type,
        "bytes": artifact.bytes,
        "run_id": artifact.run_id,
        "checksum_sha256": artifact.checksum_sha256,
        "provenance": artifact.provenance,
    });
    if let (Some(base), Some(extra)) = (merged.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    merged
}

fn build_instance_lease(
    instance_id: &str,
    holder_id: &str,
    holder_label: Option<&str>,
    mode: LeaseMode,
    ttl_seconds: u64,
) -> Result<LeaseRecord> {
    let ttl_seconds = ttl_seconds.clamp(15, 3600);
    let granted_at = utc_timestamp();
    let granted_time = OffsetDateTime::parse(&granted_at, &Rfc3339).context("parse lease time")?;
    let expires_at = (granted_time + TimeDuration::seconds(ttl_seconds as i64))
        .format(&Rfc3339)
        .context("format lease expiry")?;
    let mode_suffix = match mode {
        LeaseMode::Writer => "writer",
        LeaseMode::Observer => "observer",
    };
    Ok(LeaseRecord {
        id: StableId::new(
            IdKind::Lease,
            format!("{instance_id}_{holder_id}_{mode_suffix}"),
        )
        .into_string(),
        resource_kind: LeaseResourceKind::Instance,
        resource_id: instance_id.to_string(),
        holder_id: holder_id.to_string(),
        holder_label: holder_label.map(str::to_string),
        mode,
        granted_at: granted_at.clone(),
        expires_at,
        last_heartbeat_at: granted_at,
    })
}

pub fn runtime_root() -> Result<PathBuf> {
    if let Ok(path) = env::var("PENGU_MESH_RUNTIME_ROOT") {
        return Ok(PathBuf::from(path));
    }
    Ok(workspace_root()?.join("target").join("pengu-mesh-runtime"))
}

fn daemon_metadata_path(paths: &RuntimePaths) -> PathBuf {
    Path::new(&paths.root_dir).join("daemon.json")
}

fn continuity_enabled(entrypoint: &str) -> bool {
    entrypoint == "pengu-mesh-daemon"
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("resolve workspace root from pengu-mesh-core manifest")
}

pub fn build_diagnose_report() -> Result<DiagnoseReport> {
    build_diagnose_report_in_root(runtime_root()?)
}

pub fn build_diagnose_report_in_root(root: impl Into<PathBuf>) -> Result<DiagnoseReport> {
    let root = root.into();
    let browser_installs = discover_installations();
    let (host_access, host_access_service) = match macos_host_access_status() {
        Ok(status) => (
            status,
            DiagnoseService {
                id: "native_host_access_probe".to_string(),
                state: DiagnoseServiceState::Reachable,
                detail: "native host access probe completed successfully".to_string(),
                remediation_ids: Vec::new(),
            },
        ),
        Err(error) => {
            let detail = error.to_string();
            (
                unknown_host_access_status(&detail),
                DiagnoseService {
                    id: "native_host_access_probe".to_string(),
                    state: DiagnoseServiceState::Unknown,
                    detail: format!("native host access probe could not verify state: {detail}"),
                    remediation_ids: Vec::new(),
                },
            )
        }
    };
    let http_service = diagnose_http_service(&root);
    let services = vec![host_access_service, http_service.service];
    Ok(build_diagnose_report_from_components(
        &root,
        browser_installs,
        host_access,
        services,
        http_service.bind_addr.as_deref(),
    ))
}

pub(crate) fn build_diagnose_report_from_components(
    root: &Path,
    browser_installs: Vec<BrowserInstall>,
    host_access: HostAccessStatus,
    services: Vec<DiagnoseService>,
    http_bind_addr: Option<&str>,
) -> DiagnoseReport {
    let mut remediations = BTreeMap::new();
    let permissions = build_diagnose_permissions(&host_access, &mut remediations);
    let browser_channels =
        build_diagnose_browser_channels(&browser_installs, &host_access, &mut remediations);
    let services = enrich_services_with_remediations(services, http_bind_addr, &mut remediations);
    let capabilities = build_diagnose_capabilities(
        &browser_installs,
        &host_access,
        &services,
        &mut remediations,
    );

    let full_capability = capabilities.iter().all(|capability| {
        matches!(
            capability.state,
            DiagnoseState::Ready | DiagnoseState::Unsupported
        )
    });
    let state = diagnose_overall_state(&capabilities);
    let summary = diagnose_summary(&capabilities, &browser_channels, &services);

    DiagnoseReport {
        schema_version: "diagnose.v1".to_string(),
        generated_at: utc_timestamp(),
        platform: host_access.platform.clone(),
        full_capability,
        state,
        summary,
        runtime_root: root.display().to_string(),
        permissions,
        browser_channels,
        services,
        capabilities,
        remediations: remediations.into_values().collect(),
    }
}

fn build_diagnose_permissions(
    host_access: &HostAccessStatus,
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> Vec<DiagnosePermission> {
    host_access
        .services
        .iter()
        .map(|probe| {
            let remediation_ids = permission_remediation_ids(probe, remediations);
            DiagnosePermission {
                id: permission_item_id(&probe.service),
                service: probe.service.clone(),
                state: probe.state.clone(),
                requestable: probe.requestable,
                detail: probe.detail.clone(),
                remediation_ids,
            }
        })
        .collect()
}

fn build_diagnose_browser_channels(
    browser_installs: &[BrowserInstall],
    host_access: &HostAccessStatus,
    _remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> Vec<DiagnoseBrowserChannel> {
    let accessibility = permission_state(host_access, HostAccessService::Accessibility);
    browser_installs
        .iter()
        .map(|install| {
            let native_surface_ready =
                install.installed && accessibility == Some(PermissionState::Granted);
            DiagnoseBrowserChannel {
                id: browser_channel_item_id(&install.channel),
                channel: install.channel.clone(),
                installed: install.installed,
                managed_launch_ready: install.installed,
                native_surface_ready,
                app_path: install.app_path.clone(),
                binary_path: install.binary_path.clone(),
                detail: browser_channel_detail(install, accessibility.clone()),
                remediation_ids: Vec::new(),
            }
        })
        .collect()
}

fn enrich_services_with_remediations(
    services: Vec<DiagnoseService>,
    http_bind_addr: Option<&str>,
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> Vec<DiagnoseService> {
    services
        .into_iter()
        .map(|mut service| {
            if service.id == "http_control_plane"
                && !matches!(service.state, DiagnoseServiceState::Reachable)
            {
                let remediation =
                    start_http_daemon_remediation(http_bind_addr.unwrap_or(DEFAULT_HTTP_BIND_ADDR));
                service.remediation_ids.push(remediation.id.clone());
                remediations.insert(remediation.id.clone(), remediation);
            }
            service
        })
        .collect()
}

fn build_diagnose_capabilities(
    browser_installs: &[BrowserInstall],
    host_access: &HostAccessStatus,
    services: &[DiagnoseService],
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> Vec<DiagnoseCapability> {
    let installed_channels = installed_channels(browser_installs);
    let platform_supports_native = host_access.platform == "macos";
    let accessibility = permission_state(host_access, HostAccessService::Accessibility);
    let screen_capture = permission_state(host_access, HostAccessService::ScreenCapture);
    let listen_event = permission_state(host_access, HostAccessService::ListenEvent);
    let devtools_security = permission_state(host_access, HostAccessService::DevtoolsSecurity);
    let any_installed = !installed_channels.is_empty();
    let app_takeover_entry =
        execution_channel_status(host_access, ExecutionChannel::AppleEventsActivation);
    let global_takeover_entry =
        execution_channel_status(host_access, ExecutionChannel::GlobalTakeover);
    let http_service = services
        .iter()
        .find(|service| service.id == "http_control_plane");
    let mut capabilities = Vec::new();

    capabilities.push(if any_installed {
        DiagnoseCapability {
            id: "managed_browser_launch".to_string(),
            state: DiagnoseState::Ready,
            detail: format!(
                "managed browser launch is ready on: {}",
                installed_channels
                    .iter()
                    .map(BrowserChannel::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        }
    } else {
        DiagnoseCapability {
            id: "managed_browser_launch".to_string(),
            state: DiagnoseState::Blocked,
            detail: "no supported browser channel is currently installed".to_string(),
            blockers: vec!["browser_channel:none_installed".to_string()],
            remediation_ids: browser_installs
                .iter()
                .map(|install| browser_install_remediation(&install.channel))
                .inspect(|remediation| {
                    remediations.insert(remediation.id.clone(), remediation.clone());
                })
                .map(|remediation| remediation.id)
                .collect(),
        }
    });

    capabilities.push(capability_from_permission(
        "native_surface_observe",
        "native browser-surface discovery requires Accessibility permission",
        platform_supports_native,
        any_installed,
        accessibility.clone(),
        &[HostAccessService::Accessibility],
        remediations,
    ));
    capabilities.push(capability_from_permission(
        "native_surface_background_action",
        "background-safe native browser-surface actions require Accessibility permission",
        platform_supports_native,
        any_installed,
        accessibility,
        &[HostAccessService::Accessibility],
        remediations,
    ));
    capabilities.push(capability_from_permission(
        "native_window_evidence_capture",
        "native window evidence capture requires Screen Capture permission",
        platform_supports_native,
        any_installed,
        screen_capture,
        &[HostAccessService::ScreenCapture],
        remediations,
    ));
    capabilities.push(capability_from_permission(
        "developer_tool_authorization",
        "developer-tool authorization depends on DevToolsSecurity",
        platform_supports_native,
        true,
        devtools_security,
        &[HostAccessService::DevtoolsSecurity],
        remediations,
    ));
    capabilities.push(capability_from_execution_channel(
        "native_surface_app_takeover",
        "app-takeover browser-surface actions require a granted Apple Events permission for an installed channel",
        platform_supports_native,
        any_installed,
        app_takeover_entry,
        apple_event_services_for_channels(&installed_channels),
        remediations,
    ));
    capabilities.push(capability_from_global_takeover(
        platform_supports_native,
        any_installed,
        listen_event,
        app_takeover_entry,
        global_takeover_entry,
        apple_event_services_for_channels(&installed_channels),
        remediations,
    ));
    capabilities.push(capability_from_http_service(http_service));

    capabilities
}

fn capability_from_permission(
    id: &str,
    blocked_detail: &str,
    platform_supports_native: bool,
    any_installed: bool,
    state: Option<PermissionState>,
    services: &[HostAccessService],
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> DiagnoseCapability {
    if !platform_supports_native {
        return DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Unsupported,
            detail: "native browser-surface controls are unsupported on this platform".to_string(),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        };
    }
    if !any_installed {
        return DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Blocked,
            detail: "no installed browser channel is available for this capability".to_string(),
            blockers: vec!["browser_channel:none_installed".to_string()],
            remediation_ids: Vec::new(),
        };
    }
    match state {
        Some(PermissionState::Granted) => DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Ready,
            detail: "required host permission is already granted".to_string(),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        },
        Some(PermissionState::Unsupported) => DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Unsupported,
            detail: "required host permission is unsupported on this platform".to_string(),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        },
        Some(PermissionState::Unknown) | None => DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Unknown,
            detail: format!("{blocked_detail}; the current host state could not be verified"),
            blockers: services.iter().map(permission_item_id).collect(),
            remediation_ids: services
                .iter()
                .flat_map(|service| {
                    permission_remediation_ids_for_state(
                        service,
                        PermissionState::Unknown,
                        remediations,
                    )
                })
                .collect(),
        },
        Some(PermissionState::Missing) => DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Blocked,
            detail: blocked_detail.to_string(),
            blockers: services.iter().map(permission_item_id).collect(),
            remediation_ids: services
                .iter()
                .flat_map(|service| {
                    permission_remediation_ids_for_state(
                        service,
                        PermissionState::Missing,
                        remediations,
                    )
                })
                .collect(),
        },
    }
}

fn capability_from_execution_channel(
    id: &str,
    fallback_detail: &str,
    platform_supports_native: bool,
    any_installed: bool,
    entry: Option<&pengu_mesh_shared::ExecutionChannelAvailability>,
    blockers: Vec<HostAccessService>,
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> DiagnoseCapability {
    if !platform_supports_native {
        return DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Unsupported,
            detail: "native browser-surface controls are unsupported on this platform".to_string(),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        };
    }
    if !any_installed {
        return DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Blocked,
            detail: "no installed browser channel is available for this capability".to_string(),
            blockers: vec!["browser_channel:none_installed".to_string()],
            remediation_ids: Vec::new(),
        };
    }
    match entry {
        Some(entry) if entry.available => DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Ready,
            detail: entry.detail.clone(),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        },
        Some(entry) => {
            let blocker_ids = blockers.iter().map(permission_item_id).collect::<Vec<_>>();
            let remediation_ids = blockers
                .iter()
                .flat_map(|service| {
                    permission_remediation_ids_for_state(
                        service,
                        PermissionState::Missing,
                        remediations,
                    )
                })
                .collect();
            DiagnoseCapability {
                id: id.to_string(),
                state: if blockers.is_empty() {
                    DiagnoseState::Degraded
                } else {
                    DiagnoseState::Blocked
                },
                detail: entry.detail.clone(),
                blockers: blocker_ids,
                remediation_ids,
            }
        }
        None => DiagnoseCapability {
            id: id.to_string(),
            state: DiagnoseState::Unknown,
            detail: fallback_detail.to_string(),
            blockers: blockers.iter().map(permission_item_id).collect(),
            remediation_ids: blockers
                .iter()
                .flat_map(|service| {
                    permission_remediation_ids_for_state(
                        service,
                        PermissionState::Unknown,
                        remediations,
                    )
                })
                .collect(),
        },
    }
}

fn capability_from_global_takeover(
    platform_supports_native: bool,
    any_installed: bool,
    listen_event: Option<PermissionState>,
    app_takeover_entry: Option<&pengu_mesh_shared::ExecutionChannelAvailability>,
    global_takeover_entry: Option<&pengu_mesh_shared::ExecutionChannelAvailability>,
    app_takeover_services: Vec<HostAccessService>,
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> DiagnoseCapability {
    if !platform_supports_native {
        return DiagnoseCapability {
            id: "native_surface_global_takeover".to_string(),
            state: DiagnoseState::Unsupported,
            detail: "global takeover is unsupported outside macOS native controls".to_string(),
            blockers: Vec::new(),
            remediation_ids: Vec::new(),
        };
    }
    if !any_installed {
        return DiagnoseCapability {
            id: "native_surface_global_takeover".to_string(),
            state: DiagnoseState::Blocked,
            detail: "no installed browser channel is available for global takeover".to_string(),
            blockers: vec!["browser_channel:none_installed".to_string()],
            remediation_ids: Vec::new(),
        };
    }
    if let Some(entry) = global_takeover_entry {
        if entry.available {
            return DiagnoseCapability {
                id: "native_surface_global_takeover".to_string(),
                state: DiagnoseState::Ready,
                detail: entry.detail.clone(),
                blockers: Vec::new(),
                remediation_ids: Vec::new(),
            };
        }
    }
    let mut blockers = Vec::new();
    let mut remediation_ids = Vec::new();
    match listen_event {
        Some(PermissionState::Missing) => {
            blockers.push(permission_item_id(&HostAccessService::ListenEvent));
            remediation_ids.extend(permission_remediation_ids_for_state(
                &HostAccessService::ListenEvent,
                PermissionState::Missing,
                remediations,
            ));
        }
        Some(PermissionState::Unknown) | None => {
            blockers.push(permission_item_id(&HostAccessService::ListenEvent));
            remediation_ids.extend(permission_remediation_ids_for_state(
                &HostAccessService::ListenEvent,
                PermissionState::Unknown,
                remediations,
            ));
        }
        _ => {}
    }
    let app_takeover_ready = app_takeover_entry.is_some_and(|entry| entry.available);
    if !app_takeover_ready {
        let app_takeover_state = if app_takeover_entry.is_none() {
            PermissionState::Unknown
        } else {
            PermissionState::Missing
        };
        for service in &app_takeover_services {
            blockers.push(permission_item_id(service));
            remediation_ids.extend(permission_remediation_ids_for_state(
                service,
                app_takeover_state.clone(),
                remediations,
            ));
        }
    }
    let detail = global_takeover_entry
        .map(|entry| entry.detail.clone())
        .unwrap_or_else(|| {
            "global takeover could not be verified because execution-channel readiness is unknown"
                .to_string()
        });
    DiagnoseCapability {
        id: "native_surface_global_takeover".to_string(),
        state: if blockers.is_empty() {
            DiagnoseState::Degraded
        } else if matches!(listen_event, Some(PermissionState::Unknown) | None)
            || app_takeover_entry.is_none()
        {
            DiagnoseState::Unknown
        } else {
            DiagnoseState::Blocked
        },
        detail,
        blockers,
        remediation_ids,
    }
}

fn capability_from_http_service(service: Option<&DiagnoseService>) -> DiagnoseCapability {
    match service {
        Some(service) => DiagnoseCapability {
            id: "http_control_plane".to_string(),
            state: match service.state {
                DiagnoseServiceState::Reachable => DiagnoseState::Ready,
                DiagnoseServiceState::Unreachable => DiagnoseState::Blocked,
                DiagnoseServiceState::Unknown => DiagnoseState::Unknown,
                DiagnoseServiceState::Unsupported => DiagnoseState::Unsupported,
            },
            detail: service.detail.clone(),
            blockers: if matches!(service.state, DiagnoseServiceState::Reachable) {
                Vec::new()
            } else {
                vec![format!("service:{}", service.id)]
            },
            remediation_ids: service.remediation_ids.clone(),
        },
        None => DiagnoseCapability {
            id: "http_control_plane".to_string(),
            state: DiagnoseState::Unknown,
            detail: "http control-plane reachability was not probed".to_string(),
            blockers: vec!["service:http_control_plane".to_string()],
            remediation_ids: Vec::new(),
        },
    }
}

fn diagnose_overall_state(capabilities: &[DiagnoseCapability]) -> DiagnoseState {
    if capabilities.iter().all(|capability| {
        matches!(
            capability.state,
            DiagnoseState::Ready | DiagnoseState::Unsupported
        )
    }) {
        DiagnoseState::Ready
    } else if capabilities.iter().any(|capability| {
        capability.id == "managed_browser_launch" && capability.state == DiagnoseState::Blocked
    }) {
        DiagnoseState::Blocked
    } else if capabilities
        .iter()
        .any(|capability| capability.state == DiagnoseState::Unknown)
    {
        DiagnoseState::Unknown
    } else if capabilities.iter().any(|capability| {
        matches!(
            capability.state,
            DiagnoseState::Blocked | DiagnoseState::Degraded
        )
    }) {
        DiagnoseState::Degraded
    } else {
        DiagnoseState::Unsupported
    }
}

fn diagnose_summary(
    capabilities: &[DiagnoseCapability],
    browser_channels: &[DiagnoseBrowserChannel],
    services: &[DiagnoseService],
) -> String {
    let ready_capabilities = capabilities
        .iter()
        .filter(|capability| capability.state == DiagnoseState::Ready)
        .count();
    let blocked_capabilities = capabilities
        .iter()
        .filter(|capability| capability.state == DiagnoseState::Blocked)
        .count();
    let unknown_capabilities = capabilities
        .iter()
        .filter(|capability| capability.state == DiagnoseState::Unknown)
        .count();
    let installed_channels = browser_channels
        .iter()
        .filter(|channel| channel.installed)
        .count();
    let reachable_services = services
        .iter()
        .filter(|service| service.state == DiagnoseServiceState::Reachable)
        .count();
    format!(
        "{ready_capabilities} ready capabilities, {blocked_capabilities} blocked, {unknown_capabilities} unknown; {installed_channels} installed browser channels; {reachable_services} reachable services"
    )
}

struct DiagnoseHttpServiceProbe {
    service: DiagnoseService,
    bind_addr: Option<String>,
}

fn diagnose_http_service(root: &Path) -> DiagnoseHttpServiceProbe {
    let metadata_path = root.join("daemon.json");
    if !metadata_path.exists() {
        return DiagnoseHttpServiceProbe {
            service: DiagnoseService {
                id: "http_control_plane".to_string(),
                state: DiagnoseServiceState::Unreachable,
                detail: format!(
                    "no daemon metadata found at {}; start pengu-mesh serve to expose the HTTP control plane",
                    metadata_path.display()
                ),
                remediation_ids: Vec::new(),
            },
            bind_addr: None,
        };
    }
    let text = match fs::read_to_string(&metadata_path) {
        Ok(text) => text,
        Err(error) => {
            return DiagnoseHttpServiceProbe {
                service: DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Unknown,
                    detail: format!(
                        "failed to read daemon metadata {}: {error}",
                        metadata_path.display()
                    ),
                    remediation_ids: Vec::new(),
                },
                bind_addr: None,
            };
        }
    };
    let metadata: DaemonMetadata = match serde_json::from_str(&text) {
        Ok(metadata) => metadata,
        Err(error) => {
            return DiagnoseHttpServiceProbe {
                service: DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Unknown,
                    detail: format!(
                        "failed to parse daemon metadata {}: {error}",
                        metadata_path.display()
                    ),
                    remediation_ids: Vec::new(),
                },
                bind_addr: None,
            };
        }
    };
    let bind_addr = metadata.bind_addr.clone();
    let service = match probe_http_tools_endpoint(&bind_addr) {
        Ok(status) if status == 200 => DiagnoseService {
            id: "http_control_plane".to_string(),
            state: DiagnoseServiceState::Reachable,
            detail: format!(
                "http control plane responded at {} (pid {})",
                bind_addr, metadata.pid
            ),
            remediation_ids: Vec::new(),
        },
        Ok(status) => DiagnoseService {
            id: "http_control_plane".to_string(),
            state: DiagnoseServiceState::Unreachable,
            detail: format!(
                "http control plane at {} returned unexpected status {}",
                bind_addr, status
            ),
            remediation_ids: Vec::new(),
        },
        Err(error) => DiagnoseService {
            id: "http_control_plane".to_string(),
            state: DiagnoseServiceState::Unreachable,
            detail: format!("http control plane probe failed for {}: {error}", bind_addr),
            remediation_ids: Vec::new(),
        },
    };
    DiagnoseHttpServiceProbe {
        service,
        bind_addr: Some(bind_addr),
    }
}

fn probe_http_tools_endpoint(bind_addr: &str) -> Result<u16> {
    let mut stream = std::net::TcpStream::connect(bind_addr)
        .with_context(|| format!("connect http control plane {bind_addr}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .context("set http control plane read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .context("set http control plane write timeout")?;
    let request = format!("GET /tools HTTP/1.1\r\nHost: {bind_addr}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .with_context(|| format!("write http control plane probe {bind_addr}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .with_context(|| format!("read http control plane probe {bind_addr}"))?;
    let status_line = response
        .lines()
        .next()
        .ok_or_else(|| anyhow!("missing http status line from {bind_addr}"))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("missing http status code from {bind_addr}"))?
        .parse::<u16>()
        .with_context(|| format!("parse http status code from {bind_addr}"))?;
    Ok(status)
}

pub(crate) fn unknown_host_access_status(detail: &str) -> HostAccessStatus {
    HostAccessStatus {
        platform: std::env::consts::OS.to_string(),
        app_targets: default_app_targets(),
        services: known_host_access_services()
            .into_iter()
            .map(|service| HostAccessProbe {
                service,
                state: PermissionState::Unknown,
                requestable: true,
                open_settings_url: None,
                detail: detail.to_string(),
            })
            .collect(),
        execution_channels: Vec::new(),
        assistive_overlays: Vec::new(),
        recommended_services: known_host_access_services(),
        summary: format!("host access status could not be verified: {detail}"),
    }
}

fn permission_state(
    host_access: &HostAccessStatus,
    service: HostAccessService,
) -> Option<PermissionState> {
    host_access
        .services
        .iter()
        .find(|probe| probe.service == service)
        .map(|probe| probe.state.clone())
}

fn execution_channel_status(
    host_access: &HostAccessStatus,
    channel: ExecutionChannel,
) -> Option<&pengu_mesh_shared::ExecutionChannelAvailability> {
    host_access
        .execution_channels
        .iter()
        .find(|availability| availability.channel == channel)
}

fn installed_channels(browser_installs: &[BrowserInstall]) -> Vec<BrowserChannel> {
    browser_installs
        .iter()
        .filter(|install| install.installed)
        .map(|install| install.channel.clone())
        .collect()
}

fn default_app_targets() -> Vec<String> {
    vec![
        "Google Chrome Dev".to_string(),
        "Google Chrome".to_string(),
        "Chromium".to_string(),
    ]
}

pub(crate) fn known_host_access_services() -> Vec<HostAccessService> {
    vec![
        HostAccessService::Accessibility,
        HostAccessService::ScreenCapture,
        HostAccessService::ListenEvent,
        HostAccessService::AppleEventsChrome,
        HostAccessService::AppleEventsChromeDev,
        HostAccessService::AppleEventsChromium,
        HostAccessService::DevtoolsSecurity,
    ]
}

fn permission_item_id(service: &HostAccessService) -> String {
    format!("permission:{}", service.as_str())
}

fn browser_channel_item_id(channel: &BrowserChannel) -> String {
    format!("browser_channel:{}", channel.as_str())
}

fn permission_remediation_ids(
    probe: &HostAccessProbe,
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> Vec<String> {
    if !probe.requestable {
        return Vec::new();
    }
    permission_remediation_ids_for_state(&probe.service, probe.state.clone(), remediations)
}

fn permission_remediation_ids_for_state(
    service: &HostAccessService,
    state: PermissionState,
    remediations: &mut BTreeMap<String, DiagnoseRemediation>,
) -> Vec<String> {
    let mode = match state {
        PermissionState::Missing => "apply",
        PermissionState::Unknown => "audit",
        _ => return Vec::new(),
    };
    let remediation = host_access_remediation(service, mode);
    remediations.insert(remediation.id.clone(), remediation.clone());
    vec![remediation.id]
}

fn host_access_remediation(service: &HostAccessService, mode: &str) -> DiagnoseRemediation {
    let service_name = service.as_str();
    let action = if mode == "apply" { "Request" } else { "Audit" };
    let cli_command = if mode == "apply" {
        format!(
            "{CAPABILITY_GRANTS_ENV}=host_access_setup pengu-mesh host-access-setup --mode {mode} --service {service_name}"
        )
    } else {
        format!("pengu-mesh host-access-setup --mode {mode} --service {service_name}")
    };
    DiagnoseRemediation {
        id: format!("host_access_{mode}_{service_name}"),
        title: format!("{action} {}", host_access_service_label(service)),
        summary: if mode == "apply" {
            format!(
                "Run pengu-mesh host-access-setup in apply mode for {}.",
                host_access_service_label(service)
            )
        } else {
            format!(
                "Run pengu-mesh host-access-setup in audit mode for {}.",
                host_access_service_label(service)
            )
        },
        cli_command: Some(cli_command),
        mcp_tool: Some("host_access_setup".to_string()),
        mcp_arguments: Some(json!({
            "mode": mode,
            "services": [service_name],
        })),
        http_method: Some("POST".to_string()),
        http_route: Some("/host/access/setup".to_string()),
        http_body: Some(json!({
            "mode": mode,
            "services": [service_name],
        })),
        manual_only: false,
    }
}

fn browser_install_remediation(channel: &BrowserChannel) -> DiagnoseRemediation {
    let (id_suffix, title, cli_command) = match channel {
        BrowserChannel::ChromeDev => (
            "chrome_dev",
            "Install Google Chrome Dev",
            "brew install --cask google-chrome@dev",
        ),
        BrowserChannel::Chrome => (
            "chrome",
            "Install Google Chrome",
            "brew install --cask google-chrome",
        ),
        BrowserChannel::Chromium => (
            "chromium",
            "Install Chromium",
            "brew install --cask chromium",
        ),
    };
    DiagnoseRemediation {
        id: format!("install_browser_{id_suffix}"),
        title: title.to_string(),
        summary: format!(
            "Install the {} browser channel on this host.",
            channel.as_str()
        ),
        cli_command: Some(cli_command.to_string()),
        mcp_tool: None,
        mcp_arguments: None,
        http_method: None,
        http_route: None,
        http_body: None,
        manual_only: false,
    }
}

fn start_http_daemon_remediation(bind_addr: &str) -> DiagnoseRemediation {
    DiagnoseRemediation {
        id: "start_http_daemon".to_string(),
        title: "Start HTTP control plane".to_string(),
        summary: "Launch the pengu-mesh HTTP daemon so agents can use the HTTP surface."
            .to_string(),
        cli_command: Some(format!("pengu-mesh serve --bind {bind_addr}")),
        mcp_tool: None,
        mcp_arguments: None,
        http_method: None,
        http_route: None,
        http_body: None,
        manual_only: false,
    }
}

fn host_access_service_label(service: &HostAccessService) -> &'static str {
    match service {
        HostAccessService::Accessibility => "Accessibility permission",
        HostAccessService::ScreenCapture => "Screen Capture permission",
        HostAccessService::ListenEvent => "Listen Event permission",
        HostAccessService::AppleEventsChrome => "Apple Events permission for Google Chrome",
        HostAccessService::AppleEventsChromeDev => "Apple Events permission for Google Chrome Dev",
        HostAccessService::AppleEventsChromium => "Apple Events permission for Chromium",
        HostAccessService::DevtoolsSecurity => "DevToolsSecurity authorization",
    }
}

fn browser_channel_detail(
    install: &BrowserInstall,
    accessibility: Option<PermissionState>,
) -> String {
    if !install.installed {
        return format!(
            "{} is unavailable because {} is missing",
            install.channel.as_str(),
            install.app_path
        );
    }
    match accessibility {
        Some(PermissionState::Granted) => {
            format!(
                "{} is installed and ready for managed launch plus native surface discovery",
                install.channel.as_str()
            )
        }
        Some(PermissionState::Missing) => {
            format!(
                "{} is installed and ready for managed launch, but native surface discovery requires Accessibility permission",
                install.channel.as_str()
            )
        }
        Some(PermissionState::Unknown) | None => {
            format!(
                "{} is installed, but native surface readiness could not be verified",
                install.channel.as_str()
            )
        }
        Some(PermissionState::Unsupported) => {
            format!(
                "{} is installed, but native surface controls are unsupported on this platform",
                install.channel.as_str()
            )
        }
    }
}

fn apple_event_services_for_channels(channels: &[BrowserChannel]) -> Vec<HostAccessService> {
    channels
        .iter()
        .map(|channel| match channel {
            BrowserChannel::ChromeDev => HostAccessService::AppleEventsChromeDev,
            BrowserChannel::Chrome => HostAccessService::AppleEventsChrome,
            BrowserChannel::Chromium => HostAccessService::AppleEventsChromium,
        })
        .collect()
}

fn doctor_tools() -> Vec<DoctorToolStatus> {
    [
        "git",
        "gh",
        "rustup",
        "cargo",
        "rustc",
        "go",
        "jq",
        "sqlite3",
        "pdftoppm",
        "security",
        "DevToolsSecurity",
    ]
    .into_iter()
    .map(|name| DoctorToolStatus {
        name,
        found: which(name).is_some(),
        detail: which(name).unwrap_or_else(|| "missing".to_string()),
    })
    .collect()
}

fn terminate_pid(pid: u32) -> Result<()> {
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .context("send SIGTERM to browser pid")?;
    anyhow::ensure!(status.success(), "kill -TERM failed for pid {pid}");
    Ok(())
}

fn which(name: &str) -> Option<String> {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn command_success(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn command_output(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                stdout
            }
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn external_attach_enabled() -> bool {
    env::var("PENGU_MESH_ALLOW_EXTERNAL_ATTACH").ok().as_deref() == Some("1")
}

fn write_manifest(path: &Path, manifest: &ReplayManifest) -> Result<()> {
    fs::write(
        path,
        serde_json::to_string_pretty(manifest).context("serialize replay manifest")?,
    )
    .with_context(|| format!("write replay manifest {}", path.display()))
}

fn replay_record_from_artifact(
    artifact: &ArtifactHandle,
    path: String,
    materialized: bool,
) -> ReplayArtifactRecord {
    ReplayArtifactRecord {
        artifact_id: artifact.id.clone(),
        run_id: artifact.run_id.clone(),
        instance_id: artifact.instance_id.clone(),
        tab_id: artifact.tab_id.clone(),
        kind: artifact.kind.clone(),
        path,
        mime_type: artifact.mime_type.clone(),
        bytes: artifact.bytes,
        created_at: artifact.created_at.clone(),
        materialized,
        checksum_sha256: artifact.checksum_sha256.clone(),
        provenance: artifact.provenance.clone(),
    }
}

fn promote_portable_bundle(staging_root: &Path, final_root: &Path) -> Result<()> {
    let backup_root = final_root.with_extension("previous");
    if backup_root.exists() {
        fs::remove_dir_all(&backup_root)
            .with_context(|| format!("remove stale backup {}", backup_root.display()))?;
    }
    if final_root.exists() {
        fs::rename(final_root, &backup_root).with_context(|| {
            format!(
                "move existing portable bundle {} to backup {}",
                final_root.display(),
                backup_root.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(staging_root, final_root) {
        if backup_root.exists() {
            let _ = fs::rename(&backup_root, final_root);
        }
        return Err(error).with_context(|| {
            format!(
                "promote portable bundle {} to {}",
                staging_root.display(),
                final_root.display()
            )
        });
    }
    if backup_root.exists() {
        fs::remove_dir_all(&backup_root)
            .with_context(|| format!("remove replay backup {}", backup_root.display()))?;
    }
    Ok(())
}

fn copy_file_with_sha256(source: &Path, destination: &Path) -> Result<String> {
    let input = fs::File::open(source).with_context(|| format!("open {}", source.display()))?;
    let output = fs::File::create(destination)
        .with_context(|| format!("create {}", destination.display()))?;
    let mut reader = BufReader::new(input);
    let mut writer = BufWriter::new(output);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("read {}", source.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .with_context(|| format!("write {}", destination.display()))?;
    }
    writer
        .flush()
        .with_context(|| format!("flush {}", destination.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_path(path: &Path) -> Result<String> {
    let mut reader =
        BufReader::new(fs::File::open(path).with_context(|| format!("open {}", path.display()))?);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn discover_manifest_paths(root: PathBuf) -> Result<Vec<(PathBuf, std::time::SystemTime)>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut manifests = Vec::new();
    for run_entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
        let run_entry = run_entry?;
        if !run_entry.file_type()?.is_dir() {
            continue;
        }
        for export_entry in fs::read_dir(run_entry.path())
            .with_context(|| format!("read {}", run_entry.path().display()))?
        {
            let export_entry = export_entry?;
            if !export_entry.file_type()?.is_dir() {
                continue;
            }
            let manifest_path = export_entry.path().join("manifest.json");
            if manifest_path.exists() {
                let modified = fs::metadata(&manifest_path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                manifests.push((manifest_path, modified));
            }
        }
    }
    Ok(manifests)
}

fn kind_dir(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Screenshot => "screenshots",
        ArtifactKind::Pdf => "pdfs",
        ArtifactKind::Snapshot => "snapshots",
        ArtifactKind::Text => "text",
        ArtifactKind::Trace => "traces",
        ArtifactKind::Recording => "recordings",
    }
}

fn default_trace_categories() -> Vec<&'static str> {
    vec![
        "devtools.timeline",
        "disabled-by-default-devtools.screenshot",
        "toplevel",
    ]
}

fn tab_from_target(instance_id: &str, now: &str, target: DebugTarget) -> BrowserTab {
    let websocket_url = target
        .websocket_debugger_url
        .unwrap_or_else(|| format!("missing-websocket-for-{}", target.id));
    BrowserTab {
        id: StableId::new(IdKind::Tab, format!("{instance_id}_{}", target.id)).into_string(),
        instance_id: instance_id.to_string(),
        target_id: target.id,
        title: target.title,
        url: target.url,
        websocket_url,
        active: true,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn host_port(debug_http_url: &str) -> Result<(String, u16)> {
    let parsed = url::Url::parse(debug_http_url).context("parse debug http url")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("missing host in {debug_http_url}"))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow!("missing port in {debug_http_url}"))?;
    Ok((host, port))
}

fn attach_continuity_outcome(
    existing: Option<&BrowserInstance>,
    live_browser_ws_url: &str,
) -> AttachContinuityOutcome {
    match existing {
        None => AttachContinuityOutcome::NewInstance,
        Some(instance)
            if matches!(
                instance.status,
                InstanceStatus::Closed | InstanceStatus::Error
            ) || instance
                .browser_ws_url
                .as_deref()
                .is_some_and(|stored| stored != live_browser_ws_url) =>
        {
            AttachContinuityOutcome::ReclaimedStaleInstance
        }
        Some(_) => AttachContinuityOutcome::ReusedExistingInstance,
    }
}

fn live_tabs_for_instance(instance: &BrowserInstance) -> Result<Vec<BrowserTab>> {
    let (host, port) = host_port(&instance.debug_http_url)?;
    let targets = list_targets(&host, port)?;
    let now = utc_timestamp();
    Ok(targets
        .into_iter()
        .map(|target| tab_from_target(instance.id.as_str(), &now, target))
        .collect())
}

fn debug_url_from_cdp_url(cdp_url: &str) -> Result<String> {
    let parsed = url::Url::parse(cdp_url).context("parse cdp url")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("missing host in cdp url"))?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow!("missing port in cdp url"))?;
    Ok(debug_http_url(host, port))
}

fn version_from_cdp_url(cdp_url: &str) -> Result<VersionMetadata> {
    let parsed = url::Url::parse(cdp_url).context("parse cdp url")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("missing host in cdp url"))?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow!("missing port in cdp url"))?;
    wait_for_debug_endpoint(host, port, Duration::from_secs(3))
}

fn infer_channel_from_browser(metadata: &VersionMetadata) -> BrowserChannel {
    let browser = metadata.browser.to_ascii_lowercase();
    if browser.contains("dev") {
        BrowserChannel::ChromeDev
    } else if browser.contains("chromium") {
        BrowserChannel::Chromium
    } else {
        BrowserChannel::Chrome
    }
}

fn validate_tab_action_request(request: &TabActionRequest) -> Result<()> {
    let has_target = request.ref_id.is_some() || request.selector.is_some();
    match request.kind {
        TabActionKind::Navigate => {
            anyhow::ensure!(request.url.is_some(), "navigate requires url");
        }
        TabActionKind::Evaluate => {
            anyhow::ensure!(request.expression.is_some(), "evaluate requires expression");
        }
        TabActionKind::Press => {
            anyhow::ensure!(request.key.is_some(), "press requires key");
        }
        TabActionKind::Fill | TabActionKind::Type => {
            anyhow::ensure!(
                has_target,
                "{} requires ref or selector",
                request.kind.as_str()
            );
            anyhow::ensure!(
                request.text.is_some(),
                "{} requires text",
                request.kind.as_str()
            );
        }
        TabActionKind::Select => {
            anyhow::ensure!(has_target, "select requires ref or selector");
            anyhow::ensure!(request.value.is_some(), "select requires value");
        }
        TabActionKind::Click | TabActionKind::Focus | TabActionKind::Hover => {
            anyhow::ensure!(
                has_target,
                "{} requires ref or selector",
                request.kind.as_str()
            );
        }
    }
    Ok(())
}

fn normalize_runtime_evaluate_error(error: anyhow::Error) -> anyhow::Error {
    let reason = error.to_string();
    if reason.starts_with("Runtime.evaluate failed:") {
        error
    } else {
        anyhow!("Runtime.evaluate failed: {reason}")
    }
}

fn tab_action_script(request: &TabActionRequest) -> Result<String> {
    let request_json = serde_json::to_string(request).context("serialize tab action request")?;
    Ok(format!(
        r#"
(() => {{
  const request = {request_json};
  const root = document.body || document.documentElement;
  if (!root) {{
    throw new Error("document has no root element");
  }}
  const ordered = [];
  const walk = (node) => {{
    if (!(node instanceof Element)) return;
    ordered.push(node);
    for (const child of node.children) walk(child);
  }};
  walk(root);
  const byRef = (ref) => {{
    if (!/^e\d+$/.test(ref || "")) {{
      throw new Error(`invalid ref ${{ref}}`);
    }}
    const index = Number(ref.slice(1));
    const node = ordered[index];
    if (!(node instanceof Element)) {{
      throw new Error(`ref ${{ref}} not found`);
    }}
    return node;
  }};
  const resolveTarget = () => {{
    if (request.selector) {{
      const node = document.querySelector(request.selector);
      if (!node) throw new Error(`selector not found: ${{request.selector}}`);
      return node;
    }}
    if (request.ref) {{
      return byRef(request.ref);
    }}
    return null;
  }};
  const target = resolveTarget();
  const describe = (node) => {{
    if (!node) return "active element";
    const tag = node.tagName ? node.tagName.toLowerCase() : "node";
    const name = (node.getAttribute && (node.getAttribute("aria-label") || node.getAttribute("name") || node.getAttribute("title"))) || "";
    return name ? `${{tag}}[${{name}}]` : tag;
  }};
  const dispatch = (node, eventName) => {{
    node.dispatchEvent(new Event(eventName, {{ bubbles: true, cancelable: true }}));
  }};
  const dispatchMouse = (node, type) => {{
    node.dispatchEvent(new MouseEvent(type, {{ bubbles: true, cancelable: true, view: window }}));
  }};
  const ensureValueTarget = (node, kind) => {{
    if (!(node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement || node instanceof HTMLSelectElement)) {{
      throw new Error(`${{kind}} requires an input, textarea, or select target`);
    }}
  }};
  const focusTarget = (node) => {{
    if (node && typeof node.focus === "function") {{
      node.focus();
    }}
  }};
  switch (request.kind) {{
    case "navigate": {{
      const url = request.url;
      if (!url) throw new Error("navigate requires url");
      window.location.href = url;
      return {{ target: url, detail: `navigated to ${{url}}` }};
    }}
    case "click": {{
      focusTarget(target);
      target.click();
      return {{ target: describe(target), detail: "clicked target" }};
    }}
    case "focus": {{
      focusTarget(target);
      return {{ target: describe(target), detail: "focused target" }};
    }}
    case "hover": {{
      focusTarget(target);
      dispatchMouse(target, "mouseover");
      dispatchMouse(target, "mouseenter");
      return {{ target: describe(target), detail: "hovered target" }};
    }}
    case "fill": {{
      ensureValueTarget(target, "fill");
      focusTarget(target);
      target.value = request.text ?? "";
      dispatch(target, "input");
      dispatch(target, "change");
      return {{ target: describe(target), detail: `filled value length=${{target.value.length}}` }};
    }}
    case "type": {{
      ensureValueTarget(target, "type");
      focusTarget(target);
      const text = request.text ?? "";
      const start = typeof target.value === "string" ? target.value : "";
      for (const char of text) {{
        target.dispatchEvent(new KeyboardEvent("keydown", {{ key: char, bubbles: true }}));
        target.value = `${{target.value ?? ""}}${{char}}`;
        dispatch(target, "input");
        target.dispatchEvent(new KeyboardEvent("keyup", {{ key: char, bubbles: true }}));
      }}
      dispatch(target, "change");
      return {{ target: describe(target), detail: `typed ${{text.length}} chars (start=${{start.length}}, end=${{target.value.length}})` }};
    }}
    case "press": {{
      const active = target || document.activeElement || document.body;
      focusTarget(active);
      const key = request.key;
      if (!key) throw new Error("press requires key");
      active.dispatchEvent(new KeyboardEvent("keydown", {{ key, bubbles: true }}));
      active.dispatchEvent(new KeyboardEvent("keyup", {{ key, bubbles: true }}));
      return {{ target: describe(active), detail: `pressed ${{key}}` }};
    }}
    case "select": {{
      if (!(target instanceof HTMLSelectElement)) {{
        throw new Error("select requires a select target");
      }}
      focusTarget(target);
      target.value = request.value ?? "";
      dispatch(target, "input");
      dispatch(target, "change");
      return {{ target: describe(target), detail: `selected ${{target.value}}` }};
    }}
    default:
      throw new Error(`unsupported action kind ${{request.kind}}`);
  }}
}})()
"#
    ))
}

fn snapshot_script() -> String {
    r#"
(() => {
  const interactiveTags = new Set(["a", "button", "input", "select", "textarea", "summary"]);
  const describedByText = (node) => {
    const ids = (node.getAttribute("aria-describedby") || "")
      .split(/\s+/)
      .map((value) => value.trim())
      .filter(Boolean);
    return ids
      .map((id) => document.getElementById(id))
      .filter((candidate) => candidate instanceof Element)
      .map((candidate) => (candidate.innerText || candidate.textContent || "").replace(/\s+/g, " ").trim())
      .filter(Boolean)
      .join(" ");
  };
  const rectData = (node) => {
    const rect = node.getBoundingClientRect();
    return {
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      width: Math.round(rect.width),
      height: Math.round(rect.height)
    };
  };
  const visibilityData = (node) => {
    const style = window.getComputedStyle(node);
    const hidden =
      node.hidden ||
      node.getAttribute("aria-hidden") === "true" ||
      style.display === "none" ||
      style.visibility === "hidden" ||
      Number(style.opacity || "1") === 0;
    return {
      hidden,
      visible: !hidden
    };
  };
  const nodes = [];
  let activeRef = null;
  const walk = (node, depth) => {
    if (!(node instanceof Element)) return;
    const tag = node.tagName.toLowerCase();
    const role = node.getAttribute("role") || tag;
    const text = (node.innerText || node.textContent || "").replace(/\s+/g, " ").trim().slice(0, 240);
    const name = node.getAttribute("aria-label") || text || node.getAttribute("title") || tag;
    const description = (
      node.getAttribute("aria-description") ||
      describedByText(node) ||
      node.getAttribute("title") ||
      ""
    ).replace(/\s+/g, " ").trim().slice(0, 240);
    const interactive =
      interactiveTags.has(tag) ||
      node.hasAttribute("onclick") ||
      node.tabIndex >= 0 ||
      role === "link" ||
      role === "button";
    const ref = `e${nodes.length}`;
    const bounds = rectData(node);
    const value =
      tag === "input" || tag === "textarea" || tag === "select"
        ? String(node.value ?? "").slice(0, 240)
        : undefined;
    const checked = tag === "input" && typeof node.checked === "boolean"
      ? node.checked
      : node.getAttribute("aria-checked") === "true";
    const selected = tag === "option"
      ? !!node.selected
      : node.getAttribute("aria-selected") === "true";
    const expanded = node.hasAttribute("aria-expanded")
      ? node.getAttribute("aria-expanded") === "true"
      : null;
    const disabled =
      typeof node.matches === "function" && node.matches(":disabled")
        ? true
        : node.getAttribute("aria-disabled") === "true";
    const visibility = visibilityData(node);
    if (node === document.activeElement) activeRef = ref;
    nodes.push({
      ref,
      tag,
      role,
      name,
      description,
      text,
      interactive,
      focusable: node.tabIndex >= 0,
      active: node === document.activeElement,
      disabled,
      checked,
      selected,
      expanded,
      placeholder: node.getAttribute("placeholder"),
      value,
      url: node.getAttribute("href"),
      bounds,
      visibility,
      depth
    });
    for (const child of node.children) walk(child, depth + 1);
  };
  walk(document.body || document.documentElement, 0);
  return {
    title: document.title,
    url: location.href,
    meta: {
      lang: document.documentElement.lang || null,
      description:
        document.querySelector('meta[name="description"]')?.getAttribute("content") || null,
      canonical:
        document.querySelector('link[rel="canonical"]')?.getAttribute("href") || null
    },
    viewport: {
      inner_width: window.innerWidth,
      inner_height: window.innerHeight,
      outer_width: window.outerWidth,
      outer_height: window.outerHeight,
      device_pixel_ratio: window.devicePixelRatio
    },
    scroll: {
      x: Math.round(window.scrollX),
      y: Math.round(window.scrollY)
    },
    active_ref: activeRef,
    nodes
  };
})()
"#
    .to_string()
}

fn text_script() -> &'static str {
    r#"
(() => {
  const text = ((document.documentElement && document.documentElement.innerText) || document.body?.innerText || "").trim();
  return {
    title: document.title,
    url: location.href,
    text: text.slice(0, 12000),
    truncated: text.length > 12000
  };
})()
"#
}

#[cfg(test)]
mod tests {
    use super::{
        AttachContinuityFreshness, AttachContinuityOutcome, AttachContinuityStatus,
        AttachResolutionKind, RequiredLease, StageOneRuntime,
        browser_surface_action_requires_capability_grant, build_diagnose_report_from_components,
        build_instance_lease, capability_posture, capability_preflight, debug_url_from_cdp_url,
        known_host_access_services, parse_capability_grants, require_capability_allowed_by_policy,
        runtime_root, snapshot_script, surface_action_contracts, tab_action_script, text_script,
        unknown_host_access_status, validate_tab_action_request,
    };
    use pengu_mesh_cdp::find_installation;
    use pengu_mesh_shared::{
        ArtifactKind, BrowserChannel, BrowserInstall, BrowserInstance, BrowserSurfaceActionRequest,
        BrowserSurfaceDescriptor, BrowserTab, CapabilityDecision, CapabilityGatePolicy,
        CapabilityRiskTier, DiagnoseService, DiagnoseServiceState, DiagnoseState,
        EnvironmentFingerprint, ExecutionChannel, ExecutionChannelAvailability, HostAccessProbe,
        HostAccessService, HostAccessStatus, InstanceMode, InstanceStatus, InterferenceLevel,
        LatencySample, LeaseMode, LeaseRecord, LeaseResourceKind, PermissionState,
        ReplayExportMode, RunStatus, ScenarioAssertion, ScenarioRun, ScenarioStep,
        SurfaceActionKind, TabActionKind, TabActionRequest,
    };
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::Path;
    use std::sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, AtomicU16, Ordering},
    };
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    #[test]
    fn snapshot_script_mentions_nodes() {
        assert!(snapshot_script().contains("nodes"));
    }

    #[test]
    fn snapshot_script_mentions_viewport_and_bounds() {
        let script = snapshot_script();
        assert!(script.contains("active_ref"));
        assert!(script.contains("viewport"));
        assert!(script.contains("bounds"));
        assert!(script.contains("description"));
    }

    #[test]
    fn text_script_uses_inner_text() {
        assert!(text_script().contains("innerText"));
    }

    #[test]
    fn default_capability_posture_surfaces_safe_only_policy() {
        let posture = capability_posture(CapabilityGatePolicy::default());

        assert_eq!(posture.total, 18);
        assert_eq!(posture.safe, 8);
        assert_eq!(posture.elevated, 8);
        assert_eq!(posture.dangerous, 2);
        assert_eq!(posture.allowed, 8);
        assert_eq!(posture.denied, 8);
        assert_eq!(posture.requires_grant, 2);
        assert!(
            posture
                .capabilities
                .iter()
                .filter(|capability| capability.risk_tier == CapabilityRiskTier::Safe)
                .all(|capability| capability.decision == CapabilityDecision::Allowed)
        );
        assert!(
            posture
                .capabilities
                .iter()
                .filter(|capability| capability.risk_tier == CapabilityRiskTier::Elevated)
                .all(|capability| {
                    matches!(capability.decision, CapabilityDecision::Denied { .. })
                })
        );
    }

    #[test]
    fn capability_grants_parse_lists_and_deduplicate() {
        assert_eq!(
            parse_capability_grants("host_access_setup,browser_surface_action host_access_setup"),
            vec![
                "host_access_setup".to_string(),
                "browser_surface_action".to_string()
            ]
        );
    }

    #[test]
    fn capability_preflight_returns_actionable_grant_hints() {
        let payload =
            capability_preflight(CapabilityGatePolicy::default(), Some("host_access_setup"))
                .expect("capability preflight");

        assert!(!payload.ready);
        assert_eq!(
            payload.requested_capability.as_deref(),
            Some("host_access_setup")
        );
        assert_eq!(payload.capabilities.len(), 1);
        assert_eq!(payload.capabilities[0].name, "host_access_setup");
        assert!(!payload.capabilities[0].allowed);
        assert_eq!(
            payload.capabilities[0].grant_hint.as_deref(),
            Some("PENGU_MESH_CAPABILITY_GRANTS=host_access_setup")
        );

        let allowed_payload = capability_preflight(
            CapabilityGatePolicy {
                explicit_grants: vec!["host_access_setup".to_string()],
                ..Default::default()
            },
            Some("host_access_setup"),
        )
        .expect("granted preflight");
        assert!(allowed_payload.ready);
        assert_eq!(allowed_payload.capabilities[0].grant_hint, None);
    }

    #[test]
    fn dangerous_capabilities_require_explicit_grants() {
        let denied = require_capability_allowed_by_policy(
            "host_access_setup",
            &CapabilityGatePolicy::default(),
        )
        .expect_err("default policy should require a grant");
        assert!(denied.to_string().contains("capability grant required"));

        let policy = CapabilityGatePolicy {
            explicit_grants: vec!["host_access_setup".to_string()],
            ..Default::default()
        };
        require_capability_allowed_by_policy("host_access_setup", &policy)
            .expect("explicit grant should allow capability");
    }

    #[test]
    fn browser_surface_takeover_requests_require_capability_grants() {
        let mut request = BrowserSurfaceActionRequest {
            surface_id: Some("ax:0".to_string()),
            action: SurfaceActionKind::Focus,
            value: None,
            key_sequence: None,
            execution_channel: None,
            allow_takeover: None,
        };

        assert!(browser_surface_action_requires_capability_grant(&request));

        request.allow_takeover = Some(false);
        assert!(!browser_surface_action_requires_capability_grant(&request));

        request.execution_channel = Some(ExecutionChannel::GlobalTakeover);
        assert!(browser_surface_action_requires_capability_grant(&request));
    }

    #[test]
    fn health_and_doctor_include_capability_posture() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");

        let health = runtime.health_payload().expect("health payload");
        assert_eq!(health.capability_posture.total, 18);
        assert_eq!(health.capability_posture.allowed, 8);
        assert_eq!(health.capability_posture.requires_grant, 2);

        let doctor = runtime.doctor_report().expect("doctor report");
        assert_eq!(doctor.capability_posture.total, 18);
        assert_eq!(doctor.capability_posture.denied, 8);
    }

    #[test]
    fn diagnose_full_host_reports_ready_state() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let report = build_diagnose_report_from_components(
            tempdir.path(),
            sample_browser_installs(),
            granted_host_access_status(),
            vec![
                DiagnoseService {
                    id: "native_host_access_probe".to_string(),
                    state: DiagnoseServiceState::Reachable,
                    detail: "native host access probe completed successfully".to_string(),
                    remediation_ids: Vec::new(),
                },
                DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Reachable,
                    detail: "http control plane responded at 127.0.0.1:43127".to_string(),
                    remediation_ids: Vec::new(),
                },
            ],
            None,
        );
        let value = serde_json::to_value(&report).expect("serialize diagnose report");
        assert_diagnose_schema(&value);
        assert_eq!(report.schema_version, "diagnose.v1");
        assert_eq!(report.state, DiagnoseState::Ready);
        assert!(report.full_capability);
        assert!(report.remediations.is_empty());
    }

    #[test]
    fn diagnose_partial_host_suggests_apply_remediations() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let report = build_diagnose_report_from_components(
            tempdir.path(),
            sample_browser_installs(),
            partial_host_access_status(),
            vec![
                DiagnoseService {
                    id: "native_host_access_probe".to_string(),
                    state: DiagnoseServiceState::Reachable,
                    detail: "native host access probe completed successfully".to_string(),
                    remediation_ids: Vec::new(),
                },
                DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Unreachable,
                    detail: "http control plane probe failed for 127.0.0.1:43127: refused"
                        .to_string(),
                    remediation_ids: Vec::new(),
                },
            ],
            None,
        );
        let value = serde_json::to_value(&report).expect("serialize diagnose report");
        assert_diagnose_schema(&value);
        assert_eq!(report.state, DiagnoseState::Degraded);
        assert!(!report.full_capability);
        assert!(
            report
                .permissions
                .iter()
                .find(|permission| permission.service == HostAccessService::Accessibility)
                .is_some_and(|permission| {
                    permission
                        .remediation_ids
                        .contains(&"host_access_apply_accessibility".to_string())
                })
        );
        assert!(
            report
                .services
                .iter()
                .find(|service| service.id == "http_control_plane")
                .is_some_and(|service| {
                    service
                        .remediation_ids
                        .contains(&"start_http_daemon".to_string())
                })
        );
        assert!(
            report
                .remediations
                .iter()
                .find(|remediation| remediation.id == "host_access_apply_accessibility")
                .is_some_and(|remediation| remediation.cli_command.as_deref()
                    == Some("PENGU_MESH_CAPABILITY_GRANTS=host_access_setup pengu-mesh host-access-setup --mode apply --service accessibility"))
        );
        assert!(
            report
                .remediations
                .iter()
                .find(|remediation| remediation.id == "start_http_daemon")
                .is_some_and(|remediation| remediation.cli_command.as_deref()
                    == Some("pengu-mesh serve --bind 127.0.0.1:43127"))
        );
    }

    #[test]
    fn diagnose_ungranted_host_lists_all_requestable_permission_remediations() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let report = build_diagnose_report_from_components(
            tempdir.path(),
            sample_browser_installs(),
            ungranted_host_access_status(),
            vec![
                DiagnoseService {
                    id: "native_host_access_probe".to_string(),
                    state: DiagnoseServiceState::Reachable,
                    detail: "native host access probe completed successfully".to_string(),
                    remediation_ids: Vec::new(),
                },
                DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Unreachable,
                    detail: "no daemon metadata found".to_string(),
                    remediation_ids: Vec::new(),
                },
            ],
            None,
        );
        let value = serde_json::to_value(&report).expect("serialize diagnose report");
        assert_diagnose_schema(&value);
        assert_eq!(report.state, DiagnoseState::Degraded);
        for service in known_host_access_services() {
            let remediation_id = format!("host_access_apply_{}", service.as_str());
            assert!(
                report
                    .remediations
                    .iter()
                    .any(|remediation| remediation.id == remediation_id)
            );
        }
    }

    #[test]
    fn diagnose_unknown_host_uses_audit_remediations() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let report = build_diagnose_report_from_components(
            tempdir.path(),
            sample_browser_installs(),
            unknown_host_access_status("python bridge unavailable"),
            vec![
                DiagnoseService {
                    id: "native_host_access_probe".to_string(),
                    state: DiagnoseServiceState::Unknown,
                    detail: "native host access probe could not verify state".to_string(),
                    remediation_ids: Vec::new(),
                },
                DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Unreachable,
                    detail: "no daemon metadata found".to_string(),
                    remediation_ids: Vec::new(),
                },
            ],
            None,
        );
        let value = serde_json::to_value(&report).expect("serialize diagnose report");
        assert_diagnose_schema(&value);
        assert_eq!(report.state, DiagnoseState::Unknown);
        assert!(
            report
                .permissions
                .iter()
                .find(|permission| permission.service == HostAccessService::Accessibility)
                .is_some_and(|permission| {
                    permission
                        .remediation_ids
                        .contains(&"host_access_audit_accessibility".to_string())
                })
        );
        assert!(
            report
                .capabilities
                .iter()
                .find(|capability| capability.id == "native_surface_observe")
                .is_some_and(|capability| capability.state == DiagnoseState::Unknown)
        );
    }

    #[test]
    fn diagnose_report_threads_explicit_http_bind_into_remediation() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let report = build_diagnose_report_from_components(
            tempdir.path(),
            sample_browser_installs(),
            partial_host_access_status(),
            vec![
                DiagnoseService {
                    id: "native_host_access_probe".to_string(),
                    state: DiagnoseServiceState::Reachable,
                    detail: "native host access probe completed successfully".to_string(),
                    remediation_ids: Vec::new(),
                },
                DiagnoseService {
                    id: "http_control_plane".to_string(),
                    state: DiagnoseServiceState::Unreachable,
                    detail: "http control plane probe failed".to_string(),
                    remediation_ids: Vec::new(),
                },
            ],
            Some("127.0.0.1:44559"),
        );

        assert!(
            report
                .remediations
                .iter()
                .find(|remediation| remediation.id == "start_http_daemon")
                .is_some_and(|remediation| remediation.cli_command.as_deref()
                    == Some("pengu-mesh serve --bind 127.0.0.1:44559"))
        );
    }

    #[test]
    fn surface_action_contracts_include_permissions_and_fallbacks() {
        let surface = BrowserSurfaceDescriptor {
            id: "ax:0/4".to_string(),
            parent_id: Some("ax:0".to_string()),
            path: "0/4".to_string(),
            role: "AXWindow".to_string(),
            title: Some("Example".to_string()),
            description: None,
            value: None,
            window_title: Some("Example".to_string()),
            actions: vec![
                "focus".to_string(),
                "confirm".to_string(),
                "key_sequence".to_string(),
            ],
            focused: false,
            enabled: true,
            app_name: "Google Chrome Developer Edition".to_string(),
            bundle_id: Some("com.google.Chrome.dev".to_string()),
            channel: BrowserChannel::ChromeDev,
            instance_id: "inst_demo".to_string(),
        };
        let contracts = surface_action_contracts(&surface, &granted_host_access_status());
        let confirm = contracts
            .iter()
            .find(|contract| contract.action == "confirm")
            .expect("confirm contract");
        assert!(confirm.available);
        assert_eq!(
            confirm.expected_interference_level,
            InterferenceLevel::BackgroundSafe
        );
        assert_eq!(
            confirm.required_permissions,
            vec![HostAccessService::Accessibility]
        );
        assert!(confirm.execution_paths.iter().any(|path| {
            path.execution_channel == ExecutionChannel::GlobalTakeover
                && path.required_permissions
                    == vec![
                        HostAccessService::ListenEvent,
                        HostAccessService::AppleEventsChromeDev,
                    ]
        }));
    }

    #[test]
    fn surface_action_contracts_preserve_unknown_actions_as_unavailable() {
        let surface = BrowserSurfaceDescriptor {
            id: "ax:0/9".to_string(),
            parent_id: Some("ax:0".to_string()),
            path: "0/9".to_string(),
            role: "AXScrollArea".to_string(),
            title: Some("Scrollable".to_string()),
            description: None,
            value: None,
            window_title: Some("Example".to_string()),
            actions: vec!["totally_unknown".to_string(), "focus".to_string()],
            focused: false,
            enabled: true,
            app_name: "Unexpected Browser Label".to_string(),
            bundle_id: Some("com.google.Chrome.dev".to_string()),
            channel: BrowserChannel::ChromeDev,
            instance_id: "inst_demo".to_string(),
        };

        let contracts = surface_action_contracts(&surface, &granted_host_access_status());
        let unknown = contracts
            .iter()
            .find(|contract| contract.action == "totally_unknown")
            .expect("unknown action contract");
        assert!(!unknown.available);
        assert!(unknown.required_permissions.is_empty());
        assert_eq!(
            unknown.detail,
            "unrecognized action; pengu mesh does not yet have execution-path metadata for this action"
        );
        assert!(unknown.execution_paths.is_empty());
    }

    #[test]
    fn surface_action_contracts_scroll_is_cataloged_without_runtime_support() {
        let surface = BrowserSurfaceDescriptor {
            id: "ax:0/9".to_string(),
            parent_id: Some("ax:0".to_string()),
            path: "0/9".to_string(),
            role: "AXScrollArea".to_string(),
            title: Some("Scrollable".to_string()),
            description: None,
            value: None,
            window_title: Some("Example".to_string()),
            actions: vec!["scroll".to_string()],
            focused: false,
            enabled: true,
            app_name: "Google Chrome Developer Edition".to_string(),
            bundle_id: None,
            channel: BrowserChannel::ChromeDev,
            instance_id: "inst_demo".to_string(),
        };

        let contracts = surface_action_contracts(&surface, &granted_host_access_status());
        let scroll = contracts
            .iter()
            .find(|contract| contract.action == "scroll")
            .expect("scroll action contract");
        assert!(!scroll.available);
        assert_eq!(
            scroll.expected_interference_level,
            InterferenceLevel::BackgroundSafe
        );
        assert_eq!(
            scroll.required_permissions,
            vec![HostAccessService::Accessibility]
        );
        assert!(
            scroll
                .detail
                .contains("runtime invocation is not yet implemented")
        );
        let path = scroll
            .execution_paths
            .iter()
            .find(|path| path.execution_channel == ExecutionChannel::AxDirect)
            .expect("scroll ax_direct path");
        assert!(!path.available);
        assert!(
            path.detail
                .contains("runtime invocation is not yet implemented")
        );
    }

    #[test]
    fn surface_action_contracts_show_menu_is_cataloged_as_app_takeover() {
        let surface = BrowserSurfaceDescriptor {
            id: "ax:0/10".to_string(),
            parent_id: Some("ax:0".to_string()),
            path: "0/10".to_string(),
            role: "AXButton".to_string(),
            title: Some("Menu".to_string()),
            description: None,
            value: None,
            window_title: Some("Example".to_string()),
            actions: vec!["show_menu".to_string()],
            focused: false,
            enabled: true,
            app_name: "Google Chrome Developer Edition".to_string(),
            bundle_id: None,
            channel: BrowserChannel::ChromeDev,
            instance_id: "inst_demo".to_string(),
        };

        let contracts = surface_action_contracts(&surface, &granted_host_access_status());
        let show_menu = contracts
            .iter()
            .find(|contract| contract.action == "show_menu")
            .expect("show_menu action contract");
        assert!(!show_menu.available);
        assert_eq!(
            show_menu.expected_interference_level,
            InterferenceLevel::AppTakeover
        );
        assert_eq!(
            show_menu.required_permissions,
            vec![HostAccessService::Accessibility]
        );
        assert!(
            show_menu
                .detail
                .contains("runtime invocation is not yet implemented")
        );
        let path = show_menu
            .execution_paths
            .iter()
            .find(|path| path.execution_channel == ExecutionChannel::AxDirect)
            .expect("show_menu ax_direct path");
        assert!(!path.available);
    }

    #[test]
    fn tab_action_supports_navigate_without_target() {
        let request = TabActionRequest {
            kind: TabActionKind::Navigate,
            ref_id: None,
            selector: None,
            url: Some("https://example.com".into()),
            timeout_ms: Some(250),
            expression: None,
            text: None,
            value: None,
            key: None,
        };
        validate_tab_action_request(&request).expect("navigate request");
        let script = tab_action_script(&request).expect("navigate script");
        assert!(script.contains("window.location.href"));
        assert!(script.contains("navigated to"));
    }

    #[test]
    fn tab_list_actions_exposes_expected_contracts() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let instance_id = unique_instance_id("inst_tab_contracts");
        let instance = demo_instance(&instance_id);
        let tab = demo_tab(&instance.id, "tab_contracts_demo");
        runtime
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");
        runtime
            .store
            .replace_tabs(&instance.id, std::slice::from_ref(&tab))
            .expect("replace tabs");

        let payload = runtime
            .tab_list_actions(&instance.id, &tab.id, None)
            .expect("tab action catalog");
        assert_eq!(payload.instance.id, instance.id);
        assert_eq!(payload.tab.id, tab.id);
        assert!(
            payload
                .actions
                .iter()
                .any(|action| action.kind == "navigate" && action.detail.contains("--timeout-ms"))
        );
        assert!(
            payload
                .actions
                .iter()
                .any(|action| action.kind == "evaluate" && action.available)
        );
        assert!(
            payload
                .actions
                .iter()
                .any(|action| action.kind == "screenshot"
                    && action.detail.contains("observer lease"))
        );
        assert!(
            payload
                .actions
                .iter()
                .any(|action| action.kind == "recording" && action.available)
        );
    }

    #[test]
    fn tab_list_actions_requires_existing_instance() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let error = runtime
            .tab_list_actions("inst_missing", "tab_missing", None)
            .expect_err("missing instance should fail");
        assert!(error.to_string().contains("unknown instance inst_missing"));
    }

    #[test]
    fn tab_list_actions_requires_existing_tab() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let instance_id = unique_instance_id("inst_tab_contracts_missing_tab");
        let instance = demo_instance(&instance_id);
        runtime
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");

        let error = runtime
            .tab_list_actions(&instance.id, "tab_missing", None)
            .expect_err("missing tab should fail");
        assert!(error.to_string().contains("unknown tab tab_missing"));
    }

    #[test]
    fn attached_reconnect_prefers_existing_endpoint_identity() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let mut attached = demo_instance("inst_attached_endpoint");
        attached.name = "original-attach".into();
        attached.mode = InstanceMode::Attached;
        attached.status = InstanceStatus::Attached;
        attached.debug_http_url = "http://127.0.0.1:9555".into();
        attached.browser_ws_url = Some("ws://127.0.0.1:9555/devtools/browser/original".into());
        runtime
            .store
            .upsert_instance(&attached)
            .expect("upsert attached instance");

        let matched = runtime
            .find_attached_instance_seed(
                "http://127.0.0.1:9555",
                "ws://127.0.0.1:9555/devtools/browser/reconnected",
            )
            .expect("find attach candidate");
        assert_eq!(matched.kind, AttachResolutionKind::DebugHttpUrl);
        let instance = matched.instance.expect("matched attached instance");
        assert_eq!(instance.id, attached.id);
        assert_eq!(instance.name, attached.name);
    }

    #[test]
    fn attached_reconnect_does_not_reuse_by_name_without_endpoint_match() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let mut attached = demo_instance("inst_attached_name_only");
        attached.name = "same-name".into();
        attached.mode = InstanceMode::Attached;
        attached.status = InstanceStatus::Attached;
        attached.debug_http_url = "http://127.0.0.1:9555".into();
        attached.browser_ws_url = Some("ws://127.0.0.1:9555/devtools/browser/original".into());
        runtime
            .store
            .upsert_instance(&attached)
            .expect("upsert attached instance");

        let matched = runtime
            .find_attached_instance_seed(
                "http://127.0.0.1:9666",
                "ws://127.0.0.1:9666/devtools/browser/reconnected",
            )
            .expect("find attach candidate");
        assert_eq!(matched.kind, AttachResolutionKind::NewInstance);
        assert!(matched.instance.is_none());
    }

    #[test]
    fn attach_continuity_surfaces_in_health_and_doctor() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        runtime
            .update_attach_continuity(AttachContinuityStatus {
                outcome: Some(AttachContinuityOutcome::ReusedExistingInstance),
                freshness: AttachContinuityFreshness::Live,
                last_resolution: Some(AttachResolutionKind::BrowserWsUrl),
                last_instance_id: Some("inst_attach_demo".into()),
                last_debug_http_url: Some("http://127.0.0.1:9555".into()),
                last_requested_cdp_url: Some(
                    "ws://127.0.0.1:9555/devtools/browser/requested".into(),
                ),
                last_browser_ws_url: Some("ws://127.0.0.1:9555/devtools/browser/live".into()),
                reused_existing_instance: true,
                endpoint_refreshed: true,
                updated_at: Some("2026-03-12T02:30:00Z".into()),
            })
            .expect("store attach continuity");

        let health = runtime.health_payload().expect("health payload");
        assert_eq!(
            health.attach_continuity.outcome,
            Some(AttachContinuityOutcome::ReusedExistingInstance)
        );
        assert_eq!(
            health.attach_continuity.freshness,
            AttachContinuityFreshness::StaleInstance
        );
        assert_eq!(
            health.attach_continuity.last_resolution,
            Some(AttachResolutionKind::BrowserWsUrl)
        );
        assert!(health.attach_continuity.reused_existing_instance);
        assert!(health.attach_continuity.endpoint_refreshed);
        assert_eq!(
            health.attach_continuity.last_browser_ws_url.as_deref(),
            Some("ws://127.0.0.1:9555/devtools/browser/live")
        );

        let doctor = runtime.doctor_report().expect("doctor report");
        assert_eq!(
            doctor.attach_continuity.last_resolution,
            Some(AttachResolutionKind::BrowserWsUrl)
        );
        assert_eq!(
            doctor.attach_continuity.last_instance_id.as_deref(),
            Some("inst_attach_demo")
        );
        assert_eq!(
            doctor.attach_continuity.freshness,
            AttachContinuityFreshness::StaleInstance
        );
    }

    #[test]
    fn attach_instance_allows_first_attach_and_records_live_endpoint() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let _env = ExternalAttachEnvGuard::enable();
        let server = FakeDebugServer::spawn();
        let first_ws_url = server.browser_ws_url();

        let attached = runtime
            .attach_instance("attach-proof", &first_ws_url, Some("attach-proof-holder"))
            .expect("first attach succeeds");
        assert_eq!(attached.mode, InstanceMode::Attached);
        assert_eq!(
            attached.browser_ws_url.as_deref(),
            Some(first_ws_url.as_str())
        );

        let first_status = runtime.attach_continuity_status();
        assert_eq!(
            first_status.outcome,
            Some(AttachContinuityOutcome::NewInstance)
        );
        assert_eq!(
            first_status.last_resolution,
            Some(AttachResolutionKind::NewInstance)
        );
        assert!(!first_status.reused_existing_instance);

        let attached_again = runtime
            .attach_instance(
                "attach-proof-renamed",
                &first_ws_url,
                Some("attach-proof-holder"),
            )
            .expect("second attach reuses logical instance");
        assert_eq!(attached_again.id, attached.id);

        let second_status = runtime.attach_continuity_status();
        assert_eq!(
            second_status.outcome,
            Some(AttachContinuityOutcome::ReusedExistingInstance)
        );
        assert_eq!(
            second_status.last_resolution,
            Some(AttachResolutionKind::DebugHttpUrl)
        );
        assert!(second_status.reused_existing_instance);
        assert_eq!(
            second_status.last_browser_ws_url.as_deref(),
            Some(first_ws_url.as_str())
        );
    }

    #[test]
    fn attach_instance_reclaims_rotated_browser_ws_url_as_stale_identity() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let _env = ExternalAttachEnvGuard::enable();
        let server = FakeDebugServer::spawn();
        let first_ws_url = server.browser_ws_url();

        let first = runtime
            .attach_instance("attach-rotate", &first_ws_url, Some("attach-rotate-holder"))
            .expect("first attach");

        server.rotate_browser_ws_url("rotated-endpoint");
        let rotated_ws_url = server.browser_ws_url();
        let second = runtime
            .attach_instance(
                "attach-rotate-renamed",
                &rotated_ws_url,
                Some("attach-rotate-holder"),
            )
            .expect("rotated attach");

        assert_eq!(second.id, first.id);
        let status = runtime.attach_continuity_status();
        assert_eq!(
            status.outcome,
            Some(AttachContinuityOutcome::ReclaimedStaleInstance)
        );
        assert!(status.endpoint_refreshed);
        assert_eq!(
            status.last_browser_ws_url.as_deref(),
            Some(rotated_ws_url.as_str())
        );
    }

    #[test]
    fn attach_instance_does_not_publish_success_state_when_target_sync_fails() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-core-test")
                .expect("runtime");
        let _env = ExternalAttachEnvGuard::enable();
        let server = FakeDebugServer::spawn();
        server.set_list_status(500);
        let live_ws_url = server.browser_ws_url();

        let error = runtime
            .attach_instance("attach-fail", &live_ws_url, Some("attach-fail-holder"))
            .expect_err("attach should fail when /json/list fails");
        assert!(error.to_string().contains("/json/list"));
        assert!(
            runtime
                .store
                .list_instances()
                .expect("stored instances")
                .is_empty()
        );
        assert!(
            runtime
                .store
                .list_tabs(None)
                .expect("stored tabs")
                .is_empty()
        );
        assert_eq!(runtime.attach_continuity_status().outcome, None);

        let restarted =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-core-test")
                .expect("restarted runtime");
        let health = restarted.health_payload().expect("health payload");
        assert!(health.instances.is_empty());
        assert_eq!(health.attach_continuity.outcome, None);
        assert_eq!(
            health.attach_continuity.freshness,
            AttachContinuityFreshness::None
        );

        let doctor = restarted.doctor_report().expect("doctor report");
        assert!(doctor.instances.is_empty());
        assert_eq!(doctor.attach_continuity.outcome, None);
    }

    #[test]
    fn attach_instance_reclaims_stale_attached_identity() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let _env = ExternalAttachEnvGuard::enable();
        let server = FakeDebugServer::spawn();
        let live_ws_url = server.browser_ws_url();
        let mut stale = demo_instance("inst_attached_stale");
        stale.name = "stale-attach".into();
        stale.mode = InstanceMode::Attached;
        stale.status = InstanceStatus::Closed;
        stale.debug_http_url = "http://127.0.0.1:9222".into();
        stale.browser_ws_url = Some("ws://127.0.0.1:9222/devtools/browser/stale".into());
        stale.last_error = Some("synthetic stale proof".into());
        let stale_debug_url = debug_url_from_cdp_url(&live_ws_url).expect("debug url from ws");
        stale.debug_http_url = stale_debug_url;
        runtime
            .store
            .upsert_instance(&stale)
            .expect("upsert stale attached instance");

        let reclaimed = runtime
            .attach_instance("stale-attach", &live_ws_url, Some("stale-holder"))
            .expect("reclaim stale attach");
        assert_eq!(reclaimed.id, stale.id);
        let status = runtime.attach_continuity_status();
        assert_eq!(
            status.outcome,
            Some(AttachContinuityOutcome::ReclaimedStaleInstance)
        );
        assert!(status.endpoint_refreshed);
        assert_eq!(status.last_instance_id.as_deref(), Some(stale.id.as_str()));
        assert_eq!(
            status.last_browser_ws_url.as_deref(),
            Some(live_ws_url.as_str())
        );
    }

    #[test]
    fn attach_continuity_reclassifies_after_runtime_restart_and_endpoint_rotation() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let _env = ExternalAttachEnvGuard::enable();
        let server = FakeDebugServer::spawn();
        let first_ws_url = server.browser_ws_url();

        let runtime_one =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("first runtime");
        runtime_one
            .attach_instance("attach-restart", &first_ws_url, Some("attach-holder"))
            .expect("initial attach");
        drop(runtime_one);

        let runtime_two =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("second runtime");
        let health = runtime_two.health_payload().expect("health");
        assert_eq!(
            health.attach_continuity.freshness,
            AttachContinuityFreshness::Live
        );
        drop(runtime_two);

        server.rotate_browser_ws_url("restart-rotated");
        let runtime_three =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("third runtime");
        let doctor = runtime_three.doctor_report().expect("doctor");
        assert_eq!(
            doctor.attach_continuity.freshness,
            AttachContinuityFreshness::StaleEndpoint
        );
    }

    #[test]
    fn default_runtime_root_targets_build_output() {
        let path = runtime_root().expect("runtime root");
        assert!(path.ends_with(Path::new("target").join("pengu-mesh-runtime")));
    }

    #[test]
    fn capture_run_is_created_at_boot() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();
        assert_eq!(run.status, RunStatus::Active);
        let events = runtime.events_tail(None, 10).expect("events");
        assert!(!events.events.is_empty());
    }

    #[test]
    fn events_tail_rejects_unknown_explicit_run() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let error = runtime
            .events_tail(Some("run_missing"), 10)
            .expect_err("missing explicit run should fail");
        assert!(error.to_string().contains("unknown run run_missing"));
    }

    #[test]
    fn scenario_list_returns_filtered_runs() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let startup_run = ScenarioRun {
            id: "scenario_run_startup".into(),
            scenario_name: "startup-readiness".into(),
            scenario_family: "startup-readiness".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: None,
            commit_sha: Some("2bfd744".into()),
            branch_name: Some("main".into()),
            platform: "darwin".into(),
            started_at: "2026-03-12T10:00:00Z".into(),
            finished_at: Some("2026-03-12T10:00:15Z".into()),
            status: "passed".into(),
            summary_path: None,
        };
        let evidence_run = ScenarioRun {
            id: "scenario_run_evidence".into(),
            scenario_name: "evidence-chain".into(),
            scenario_family: "evidence-chain".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: None,
            commit_sha: Some("2bfd744".into()),
            branch_name: Some("main".into()),
            platform: "darwin".into(),
            started_at: "2026-03-12T11:00:00Z".into(),
            finished_at: Some("2026-03-12T11:00:20Z".into()),
            status: "passed".into(),
            summary_path: None,
        };

        runtime
            .store
            .insert_scenario_run(&startup_run)
            .expect("insert startup scenario run");
        runtime
            .store
            .insert_scenario_run(&evidence_run)
            .expect("insert evidence scenario run");

        let filtered = runtime
            .scenario_list(Some("startup-readiness"), 10)
            .expect("filtered scenario list");
        assert_eq!(
            filtered.requested_family.as_deref(),
            Some("startup-readiness")
        );
        assert_eq!(filtered.runs, vec![startup_run]);

        let limited = runtime
            .scenario_list(None, 1)
            .expect("limited scenario list");
        assert_eq!(limited.requested_limit, 1);
        assert_eq!(limited.runs, vec![evidence_run]);
    }

    #[test]
    fn scenario_run_detail_returns_related_records() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = ScenarioRun {
            id: "scenario_run_detail".into(),
            scenario_name: "startup-readiness".into(),
            scenario_family: "startup-readiness".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: Some("/tmp/runtime-root".into()),
            commit_sha: Some("2bfd744".into()),
            branch_name: Some("main".into()),
            platform: "darwin".into(),
            started_at: "2026-03-12T12:00:00Z".into(),
            finished_at: Some("2026-03-12T12:00:30Z".into()),
            status: "passed".into(),
            summary_path: Some("/tmp/summary.md".into()),
        };
        let step = ScenarioStep {
            id: "scenario_step_detail".into(),
            run_id: run.id.clone(),
            ordinal: 1,
            step_name: "health".into(),
            step_kind: "command".into(),
            command_line: Some("pengu-mesh health".into()),
            started_at: "2026-03-12T12:00:01Z".into(),
            finished_at: Some("2026-03-12T12:00:02Z".into()),
            status: "passed".into(),
            error_code: None,
        };
        let assertion = ScenarioAssertion {
            id: "scenario_assertion_detail".into(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            assertion_name: "health ok".into(),
            expected_value: Some("true".into()),
            actual_value: Some("true".into()),
            status: "passed".into(),
            failure_category: None,
            notes: Some("ok".into()),
        };
        let sample = LatencySample {
            id: "scenario_latency_detail".into(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            metric_name: "health".into(),
            sample_ms: 14.0.into(),
            capture_method: Some("wall_clock".into()),
        };
        let fingerprint = EnvironmentFingerprint {
            id: "scenario_env_detail".into(),
            run_id: run.id.clone(),
            platform: "darwin".into(),
            arch: "arm64".into(),
            os_version: Some("Darwin 25.0.0".into()),
            rust_version: Some("rustc 1.94.0".into()),
            cargo_version: Some("cargo 1.94.0".into()),
            chrome_channel: Some("chrome-dev".into()),
            chrome_version: Some("136.0.0.0".into()),
        };

        runtime
            .store
            .insert_scenario_run(&run)
            .expect("insert scenario run");
        runtime
            .store
            .insert_scenario_step(&step)
            .expect("insert scenario step");
        runtime
            .store
            .insert_scenario_assertion(&assertion)
            .expect("insert scenario assertion");
        runtime
            .store
            .insert_latency_sample(&sample)
            .expect("insert latency sample");
        runtime
            .store
            .insert_environment_fingerprint(&fingerprint)
            .expect("insert environment fingerprint");

        let detail = runtime
            .scenario_run_detail(&run.id)
            .expect("scenario run detail");
        assert_eq!(detail.run, run);
        assert_eq!(detail.steps, vec![step]);
        assert_eq!(detail.assertions, vec![assertion]);
        assert_eq!(detail.latency_samples, vec![sample]);
        assert_eq!(detail.environment_fingerprint, Some(fingerprint));

        let error = runtime
            .scenario_run_detail("scenario_run_missing")
            .expect_err("missing scenario run should fail");
        assert!(
            error
                .to_string()
                .contains("unknown scenario run scenario_run_missing")
        );
    }

    #[test]
    fn capture_stop_and_start_rotate_run_status() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let first = runtime.capture_run();
        let stopped = runtime.capture_stop_recording().expect("stopped");
        assert_eq!(stopped.status, RunStatus::Completed);
        assert_eq!(stopped.id, first.id);
        let restarted = runtime.capture_start_recording().expect("restarted");
        assert_eq!(restarted.status, RunStatus::Active);
        assert_ne!(restarted.id, stopped.id);
    }

    #[test]
    fn replay_export_writes_manifest() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let export = runtime
            .replay_export(None, ReplayExportMode::ManifestOnly)
            .expect("replay export");
        assert!(Path::new(&export.manifest_path).exists());
        assert!(export.event_count >= 1);
        assert_eq!(export.mode, ReplayExportMode::ManifestOnly);
    }

    #[test]
    fn portable_replay_export_materializes_artifacts_and_validates() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();
        let artifact = runtime
            .artifacts
            .write_bytes(
                pengu_mesh_shared::ArtifactKind::Screenshot,
                Some(&run.id),
                "inst_demo",
                "tab_demo",
                b"portable-artifact",
            )
            .expect("artifact");
        runtime
            .store
            .upsert_artifact(&artifact)
            .expect("upsert artifact");

        let export = runtime
            .replay_export(None, ReplayExportMode::Portable)
            .expect("portable replay export");
        assert!(Path::new(&export.manifest_path).exists());
        assert!(Path::new(&export.bundle_root).join("artifacts").exists());
        let report = runtime.doctor_report().expect("doctor report");
        assert!(report.replay_validations.iter().any(|item| {
            item.manifest_path == export.manifest_path
                && item.ok
                && item.mode == ReplayExportMode::Portable
        }));
    }

    #[test]
    fn artifact_verify_passes_then_detects_corruption() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();
        let artifact = runtime
            .artifacts
            .write_bytes(
                ArtifactKind::Screenshot,
                Some(&run.id),
                "inst_demo",
                "tab_demo",
                b"artifact-verify-demo",
            )
            .expect("artifact");
        runtime
            .store
            .upsert_artifact(&artifact)
            .expect("upsert artifact");

        let verified = runtime
            .artifact_verify(&artifact.id)
            .expect("verify payload");
        assert!(verified.valid);
        assert_eq!(
            verified.expected_sha256.as_deref(),
            Some(verified.actual_sha256.as_str())
        );

        fs::write(&artifact.path, b"artifact-verify-corrupted").expect("corrupt artifact");
        let corrupted = runtime
            .artifact_verify(&artifact.id)
            .expect("corrupted verify payload");
        assert!(!corrupted.valid);
        assert_ne!(
            corrupted.expected_sha256.as_deref(),
            Some(corrupted.actual_sha256.as_str())
        );
    }

    #[test]
    fn artifact_crop_grid_creates_multiple_derived_artifacts() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();
        let instance_id = unique_instance_id("inst_demo");
        runtime
            .store
            .upsert_instance(&demo_instance(&instance_id))
            .expect("upsert instance");
        let artifact = runtime
            .artifacts
            .write_bytes(
                ArtifactKind::Screenshot,
                Some(&run.id),
                &instance_id,
                "tab_demo",
                &sample_png_bytes(),
            )
            .expect("artifact");
        runtime
            .store
            .upsert_artifact(&artifact)
            .expect("upsert artifact");

        let payload = runtime
            .artifact_crop_grid(
                &artifact.id,
                2,
                2,
                10,
                None,
                Some(runtime.operator_id.as_str()),
            )
            .expect("grid crop payload");
        let lease_status = runtime
            .lease_status(Some(&instance_id))
            .expect("lease status after crop grid");
        assert_eq!(payload.artifacts.len(), 4);
        assert_eq!(payload.rows, 2);
        assert_eq!(payload.cols, 2);
        assert_eq!(lease_status.leases.len(), 1);
        assert_eq!(lease_status.leases[0].mode, LeaseMode::Observer);
        assert!(payload.artifacts.iter().all(|item| {
            item.artifact.kind == ArtifactKind::Screenshot
                && Path::new(&item.artifact.path).exists()
                && item.crop_region == item.artifact.provenance.crop_region.clone().unwrap()
        }));
    }

    #[test]
    fn artifact_verify_reports_valid_and_invalid_hashes() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();
        let artifact = runtime
            .artifacts
            .write_bytes(
                ArtifactKind::Screenshot,
                Some(&run.id),
                "inst_artifact_verify",
                "tab_artifact_verify",
                b"artifact-verify",
            )
            .expect("artifact");
        runtime
            .store
            .upsert_artifact(&artifact)
            .expect("upsert artifact");

        let verified = runtime.artifact_verify(&artifact.id).expect("verified");
        assert!(verified.valid);
        assert_eq!(verified.expected_sha256, artifact.checksum_sha256);
        assert_eq!(
            verified.actual_sha256,
            artifact
                .checksum_sha256
                .clone()
                .expect("stored sha256 should be present")
        );

        fs::write(&artifact.path, b"corrupted-artifact").expect("corrupt artifact");
        let invalid = runtime
            .artifact_verify(&artifact.id)
            .expect("invalid verification");
        assert!(!invalid.valid);
        assert_eq!(invalid.expected_sha256, artifact.checksum_sha256);
        assert_ne!(invalid.actual_sha256, verified.actual_sha256);
    }

    #[test]
    fn artifact_list_filters_by_instance_and_run() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();

        let artifact_one = runtime
            .artifacts
            .write_bytes(
                ArtifactKind::Screenshot,
                Some(&run.id),
                "inst_artifact_one",
                "tab_one",
                b"artifact-one",
            )
            .expect("artifact one");
        runtime
            .store
            .upsert_artifact(&artifact_one)
            .expect("upsert artifact one");

        let artifact_two = runtime
            .artifacts
            .write_bytes(
                ArtifactKind::Text,
                Some(&run.id),
                "inst_artifact_two",
                "tab_two",
                b"artifact-two",
            )
            .expect("artifact two");
        runtime
            .store
            .upsert_artifact(&artifact_two)
            .expect("upsert artifact two");

        let artifact_three = runtime
            .artifacts
            .write_bytes(
                ArtifactKind::Snapshot,
                None,
                "inst_artifact_one",
                "tab_three",
                b"{\"ok\":true}",
            )
            .expect("artifact three");
        runtime
            .store
            .upsert_artifact(&artifact_three)
            .expect("upsert artifact three");

        let by_instance = runtime
            .artifact_list(Some("inst_artifact_one"), None)
            .expect("artifacts by instance");
        assert_eq!(
            by_instance.instance_id.as_deref(),
            Some("inst_artifact_one")
        );
        assert_eq!(by_instance.run_id, None);
        assert_eq!(by_instance.artifacts.len(), 2);
        assert!(
            by_instance
                .artifacts
                .iter()
                .all(|artifact| artifact.path == artifact_one.path
                    || artifact.path == artifact_three.path)
        );

        let by_run = runtime
            .artifact_list(None, Some(&run.id))
            .expect("artifacts by run");
        assert_eq!(by_run.artifacts.len(), 2);
        assert!(
            by_run
                .artifacts
                .iter()
                .all(|artifact| artifact.created_at <= artifact_two.created_at)
        );

        let by_both = runtime
            .artifact_list(Some("inst_artifact_one"), Some(&run.id))
            .expect("artifacts by both filters");
        assert_eq!(by_both.artifacts.len(), 1);
        assert_eq!(by_both.artifacts[0].id, artifact_one.id);
        assert_eq!(by_both.artifacts[0].sha256, artifact_one.checksum_sha256);
        assert_eq!(by_both.artifacts[0].size_bytes, artifact_one.bytes);
    }

    #[test]
    fn portable_replay_export_materializes_trace_and_recording_artifacts() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let run = runtime.capture_run();
        for (kind, payload) in [
            (ArtifactKind::Trace, b"{\"traceEvents\":[]}".as_slice()),
            (ArtifactKind::Recording, b"recording-archive".as_slice()),
        ] {
            let artifact = runtime
                .artifacts
                .write_bytes(kind, Some(&run.id), "inst_demo", "tab_demo", payload)
                .expect("artifact");
            runtime
                .store
                .upsert_artifact(&artifact)
                .expect("upsert artifact");
        }

        let export = runtime
            .replay_export(None, ReplayExportMode::Portable)
            .expect("portable replay export");
        assert!(
            Path::new(&export.bundle_root)
                .join("artifacts")
                .join("traces")
                .exists()
        );
        assert!(
            Path::new(&export.bundle_root)
                .join("artifacts")
                .join("recordings")
                .exists()
        );
    }

    #[test]
    fn lease_lifecycle_surfaces_in_status_and_supports_transfer() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let instance = demo_instance(&unique_instance_id("inst_lease_status"));
        runtime
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");

        let acquired = runtime
            .lease_acquire(
                &instance.id,
                "agent_alpha",
                Some("Alpha"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire writer");
        assert_eq!(acquired.lease.holder_id, "agent_alpha");

        let conflict = runtime
            .lease_acquire(&instance.id, "agent_beta", None, LeaseMode::Writer, 120)
            .expect_err("writer conflict");
        assert!(conflict.to_string().contains("held by agent_alpha"));

        let transferred = runtime
            .lease_transfer(&instance.id, "agent_alpha", "agent_beta", Some("Beta"), 180)
            .expect("transfer writer");
        assert_eq!(transferred.lease.holder_id, "agent_beta");

        let status = runtime
            .lease_status(Some(&instance.id))
            .expect("lease status");
        assert_eq!(status.leases.len(), 1);
        assert_eq!(status.leases[0].holder_id, "agent_beta");

        let health = runtime.health_payload().expect("health");
        assert!(
            health
                .leases
                .iter()
                .any(|lease| lease.resource_id == instance.id && lease.holder_id == "agent_beta")
        );

        let released = runtime
            .lease_release(&instance.id, "agent_beta", Some(LeaseMode::Writer))
            .expect("release writer");
        assert_eq!(released.released_count, 1);
        assert!(released.leases.is_empty());
    }

    #[test]
    fn create_profile_materializes_named_profile_directory() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let profile = runtime
            .create_profile("Work Browser", BrowserChannel::ChromeDev)
            .expect("create profile");
        assert_eq!(profile.name, "Work Browser");
        assert!(profile.path.contains("prof_chrome_dev_work_browser"));
        assert!(Path::new(&profile.path).exists());
        assert!(
            runtime
                .list_profiles()
                .expect("list profiles")
                .iter()
                .any(|item| item.id == profile.id)
        );
    }

    #[test]
    fn observer_access_auto_acquires_and_coexists_with_writer() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let instance = demo_instance(&unique_instance_id("inst_observer_access"));
        runtime
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");

        runtime
            .lease_acquire(
                &instance.id,
                "writer_one",
                Some("Writer"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire writer");
        runtime
            .require_instance_access(
                &instance.id,
                "observer_two",
                RequiredLease::Observer,
                "tab_snapshot",
            )
            .expect("observer access");

        let status = runtime
            .lease_status(Some(&instance.id))
            .expect("lease status after observer");
        assert!(
            status
                .leases
                .iter()
                .any(|lease| lease.holder_id == "writer_one" && lease.mode == LeaseMode::Writer)
        );
        assert!(
            status
                .leases
                .iter()
                .any(|lease| lease.holder_id == "observer_two"
                    && lease.mode == LeaseMode::Observer)
        );
    }

    #[test]
    fn mutating_operations_require_matching_writer_when_leases_are_active() {
        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-test");
        let instance = demo_instance(&unique_instance_id("inst_lease_gate"));
        runtime
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");

        runtime
            .lease_acquire(
                &instance.id,
                "observer_one",
                Some("Observer"),
                LeaseMode::Observer,
                120,
            )
            .expect("acquire observer");
        let observer_block = runtime
            .open_tab(&instance.id, "https://example.com", None)
            .expect_err("observer should block mutation");
        assert!(
            observer_block
                .to_string()
                .contains("requires an explicit writer lease")
        );

        runtime
            .lease_acquire(
                &instance.id,
                "writer_one",
                Some("Writer"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire writer");
        let writer_block = runtime
            .open_tab(&instance.id, "https://example.com", Some("writer_two"))
            .expect_err("writer mismatch");
        assert!(writer_block.to_string().contains("held by writer_one"));

        let stop_block = runtime
            .stop_instance(&instance.id, Some("writer_two"))
            .expect_err("stop should require matching writer");
        assert!(stop_block.to_string().contains("held by writer_one"));
    }

    #[test]
    fn tab_action_validation_requires_expected_fields() {
        validate_tab_action_request(&TabActionRequest {
            kind: TabActionKind::Navigate,
            ref_id: None,
            selector: None,
            url: Some("https://example.com".into()),
            timeout_ms: Some(25),
            expression: None,
            text: None,
            value: None,
            key: None,
        })
        .expect("navigate should accept url");

        validate_tab_action_request(&TabActionRequest {
            kind: TabActionKind::Evaluate,
            ref_id: None,
            selector: None,
            url: None,
            timeout_ms: None,
            expression: Some("document.title".into()),
            text: None,
            value: None,
            key: None,
        })
        .expect("evaluate should accept expression");

        let click_error = validate_tab_action_request(&TabActionRequest {
            kind: TabActionKind::Click,
            ref_id: None,
            selector: None,
            url: None,
            timeout_ms: None,
            expression: None,
            text: None,
            value: None,
            key: None,
        })
        .expect_err("click should require target");
        assert!(click_error.to_string().contains("requires ref or selector"));

        let fill_error = validate_tab_action_request(&TabActionRequest {
            kind: TabActionKind::Fill,
            ref_id: Some("e1".into()),
            selector: None,
            url: None,
            timeout_ms: None,
            expression: None,
            text: None,
            value: None,
            key: None,
        })
        .expect_err("fill should require text");
        assert!(fill_error.to_string().contains("fill requires text"));

        let evaluate_error = validate_tab_action_request(&TabActionRequest {
            kind: TabActionKind::Evaluate,
            ref_id: None,
            selector: None,
            url: None,
            timeout_ms: None,
            expression: None,
            text: None,
            value: None,
            key: None,
        })
        .expect_err("evaluate should require expression");
        assert!(
            evaluate_error
                .to_string()
                .contains("evaluate requires expression")
        );
    }

    #[test]
    fn tab_action_navigation_honors_custom_timeout_ms() {
        if !cfg!(target_os = "macos") {
            return;
        }
        if find_installation(BrowserChannel::ChromeDev).is_none() {
            return;
        }

        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-timeout-proof");
        let instance = runtime
            .start_instance(
                "navigate-timeout-proof",
                BrowserChannel::ChromeDev,
                true,
                Some("timeout-proof-holder"),
            )
            .expect("start headless instance");
        let _instance_guard =
            ManagedInstanceGuard::new(&runtime, &instance.id, Some("timeout-proof-holder"));
        let tab = runtime
            .open_tab(
                &instance.id,
                "data:text/html,<title>Before</title><body>Before</body>",
                Some("timeout-proof-holder"),
            )
            .expect("open tab");
        let delayed_server = DelayedHttpServer::spawn(Duration::from_millis(500));

        let error = runtime
            .tab_action(
                &tab.id,
                TabActionRequest {
                    kind: TabActionKind::Navigate,
                    ref_id: None,
                    selector: None,
                    url: Some(delayed_server.url()),
                    timeout_ms: Some(50),
                    expression: None,
                    text: None,
                    value: None,
                    key: None,
                },
                Some("timeout-proof-holder"),
            )
            .expect_err("navigation should time out");
        assert!(
            error
                .to_string()
                .contains("navigation timed out waiting for load event"),
            "unexpected timeout error: {error}"
        );
    }

    #[test]
    fn tab_action_evaluate_normalizes_runtime_errors() {
        if !cfg!(target_os = "macos") {
            return;
        }
        if find_installation(BrowserChannel::ChromeDev).is_none() {
            return;
        }

        let (_tempdir, runtime) = runtime_for_test("pengu-mesh-core-evaluate-proof");
        let instance = runtime
            .start_instance(
                "evaluate-proof",
                BrowserChannel::ChromeDev,
                true,
                Some("evaluate-proof-holder"),
            )
            .expect("start headless instance");
        let _instance_guard =
            ManagedInstanceGuard::new(&runtime, &instance.id, Some("evaluate-proof-holder"));
        let tab = runtime
            .open_tab(
                &instance.id,
                "data:text/html,<title>Eval</title><body>Eval</body>",
                Some("evaluate-proof-holder"),
            )
            .expect("open tab");

        let error = runtime
            .tab_action(
                &tab.id,
                TabActionRequest {
                    kind: TabActionKind::Evaluate,
                    ref_id: None,
                    selector: None,
                    url: None,
                    timeout_ms: None,
                    expression: Some("(() => { throw new Error('boom') })()".to_string()),
                    text: None,
                    value: None,
                    key: None,
                },
                Some("evaluate-proof-holder"),
            )
            .expect_err("evaluate should fail");
        assert!(
            error.to_string().contains("Runtime.evaluate failed:"),
            "unexpected evaluate error: {error}"
        );
    }

    #[test]
    fn daemon_restart_recovers_active_run_operator_and_leases() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let runtime_one =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("first daemon runtime");
        let instance = demo_instance(&unique_instance_id("inst_daemon_recovery"));
        runtime_one
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");
        runtime_one
            .lease_acquire(
                &instance.id,
                runtime_one.operator_id.as_str(),
                Some("daemon"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire daemon writer lease");
        runtime_one
            .lease_acquire(
                &instance.id,
                "observer_after_restart",
                Some("Observer"),
                LeaseMode::Observer,
                120,
            )
            .expect("acquire observer lease");
        let first_run = runtime_one.capture_run();
        let first_operator = runtime_one.operator_id.clone();

        let runtime_two =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("second daemon runtime");
        let continuity = runtime_two.continuity_status();
        assert_eq!(runtime_two.capture_run().id, first_run.id);
        assert_eq!(runtime_two.operator_id, first_operator);
        assert!(continuity.continuity_enabled);
        assert!(continuity.recovered_run);
        assert!(continuity.reused_operator_id);
        assert_eq!(
            continuity.recovered_run_id.as_deref(),
            Some(first_run.id.as_str())
        );
        assert_eq!(continuity.recovered_lease_count, 1);

        let health = runtime_two.health_payload().expect("health");
        assert!(health.continuity.recovered_run);
        assert!(
            health
                .leases
                .iter()
                .any(|lease| lease.resource_id == instance.id && lease.holder_id == first_operator)
        );
        assert!(
            health
                .leases
                .iter()
                .any(|lease| lease.resource_id == instance.id
                    && lease.holder_id == "observer_after_restart")
        );
    }

    #[test]
    fn daemon_restart_ignores_foreign_writer_leases_in_continuity_totals() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let runtime_one =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("first daemon runtime");
        let mut daemon_instance = demo_instance(&unique_instance_id("inst_daemon_owned"));
        daemon_instance.debug_http_url = "http://127.0.0.1:65532".into();
        daemon_instance.browser_ws_url =
            Some("ws://127.0.0.1:65532/devtools/browser/daemon".into());
        let mut peer_instance = demo_instance(&unique_instance_id("inst_peer_owned"));
        peer_instance.status = InstanceStatus::Closed;
        runtime_one
            .store
            .upsert_instance(&daemon_instance)
            .expect("upsert daemon instance");
        runtime_one
            .store
            .upsert_instance(&peer_instance)
            .expect("upsert peer instance");
        runtime_one
            .lease_acquire(
                &daemon_instance.id,
                runtime_one.operator_id.as_str(),
                Some("daemon"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire daemon writer lease");
        runtime_one
            .store
            .upsert_lease(
                &build_instance_lease(
                    &peer_instance.id,
                    "peer-agent",
                    Some("peer"),
                    LeaseMode::Writer,
                    120,
                )
                .expect("build peer writer lease"),
            )
            .expect("upsert peer writer lease");

        let runtime_two =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("second daemon runtime");
        let continuity = runtime_two.continuity_status();
        assert_eq!(continuity.recovered_lease_count, 1);
        assert_eq!(continuity.recovered_instance_count, 0);
        assert_eq!(continuity.stale_instance_count, 1);
        assert!(continuity.stale_instance_ids.contains(&daemon_instance.id));
        assert!(!continuity.stale_instance_ids.contains(&peer_instance.id));

        let health = runtime_two.health_payload().expect("health");
        assert_eq!(
            health
                .leases
                .iter()
                .filter(|lease| lease.holder_id == runtime_two.operator_id)
                .count(),
            1
        );
        assert!(
            health
                .leases
                .iter()
                .any(|lease| lease.resource_id == peer_instance.id
                    && lease.holder_id == "peer-agent")
        );
    }

    #[test]
    fn daemon_restart_prunes_expired_daemon_leases_before_recovery() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let runtime_one =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("first daemon runtime");
        let instance = demo_instance(&unique_instance_id("inst_expired_restart"));
        runtime_one
            .store
            .upsert_instance(&instance)
            .expect("upsert instance");

        let now = OffsetDateTime::now_utc();
        let granted_at = (now - time::Duration::minutes(10))
            .format(&Rfc3339)
            .expect("format granted_at");
        let expires_at = (now - time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format expires_at");
        runtime_one
            .store
            .upsert_lease(&LeaseRecord {
                id: "lease_expired_daemon_writer".into(),
                resource_kind: LeaseResourceKind::Instance,
                resource_id: instance.id.clone(),
                holder_id: runtime_one.operator_id.clone(),
                holder_label: Some("daemon".into()),
                mode: LeaseMode::Writer,
                granted_at: granted_at.clone(),
                expires_at,
                last_heartbeat_at: granted_at,
            })
            .expect("upsert expired lease");

        let runtime_two =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("second daemon runtime");
        let continuity = runtime_two.continuity_status();
        assert_eq!(continuity.recovered_lease_count, 0);

        let health = runtime_two.health_payload().expect("health");
        assert!(
            !health
                .leases
                .iter()
                .any(|lease| lease.holder_id == runtime_two.operator_id)
        );

        let acquired = runtime_two
            .lease_acquire(
                &instance.id,
                "fresh-holder",
                Some("Fresh"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire fresh writer");
        assert_eq!(acquired.lease.holder_id, "fresh-holder");
    }

    #[test]
    fn daemon_restart_classifies_stale_instances() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let runtime_one =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("first daemon runtime");
        let mut instance = demo_instance(&unique_instance_id("inst_stale_recovery"));
        instance.debug_http_url = "http://127.0.0.1:65534".into();
        instance.browser_ws_url = Some("ws://127.0.0.1:65534/devtools/browser/demo".into());
        runtime_one
            .store
            .upsert_instance(&instance)
            .expect("upsert stale instance");
        runtime_one
            .lease_acquire(
                &instance.id,
                runtime_one.operator_id.as_str(),
                Some("daemon"),
                LeaseMode::Writer,
                120,
            )
            .expect("acquire daemon writer lease");

        let runtime_two =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-daemon")
                .expect("second daemon runtime");
        let continuity = runtime_two.continuity_status();
        assert_eq!(continuity.stale_instance_count, 1);
        assert!(continuity.stale_instance_ids.contains(&instance.id));

        let refreshed = runtime_two
            .list_instances()
            .expect("refreshed instances")
            .into_iter()
            .find(|candidate| candidate.id == instance.id)
            .expect("stale instance present");
        assert_eq!(refreshed.status, InstanceStatus::Closed);
        assert!(refreshed.last_error.is_some());
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

    fn unique_instance_id(prefix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos();
        format!("{prefix}_{nanos}")
    }

    fn sample_browser_installs() -> Vec<BrowserInstall> {
        vec![
            BrowserInstall {
                channel: BrowserChannel::ChromeDev,
                installed: true,
                app_path: "/Applications/Google Chrome Dev.app".to_string(),
                binary_path: "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev"
                    .to_string(),
            },
            BrowserInstall {
                channel: BrowserChannel::Chrome,
                installed: false,
                app_path: "/Applications/Google Chrome.app".to_string(),
                binary_path: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
                    .to_string(),
            },
            BrowserInstall {
                channel: BrowserChannel::Chromium,
                installed: false,
                app_path: "/Applications/Chromium.app".to_string(),
                binary_path: "/Applications/Chromium.app/Contents/MacOS/Chromium".to_string(),
            },
        ]
    }

    fn granted_host_access_status() -> HostAccessStatus {
        HostAccessStatus {
            platform: "macos".to_string(),
            app_targets: vec![
                "Google Chrome Dev".to_string(),
                "Google Chrome".to_string(),
                "Chromium".to_string(),
            ],
            services: known_host_access_services()
                .into_iter()
                .map(|service| sample_probe(service, PermissionState::Granted))
                .collect(),
            execution_channels: vec![
                sample_execution_channel(
                    ExecutionChannel::Cdp,
                    true,
                    InterferenceLevel::BackgroundSafe,
                    "Primary meshing channel for page and tab control.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AxDirect,
                    true,
                    InterferenceLevel::BackgroundSafe,
                    "Direct macOS Accessibility action and discovery path.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AppleEventsActivation,
                    true,
                    InterferenceLevel::AppTakeover,
                    "Automation fallback is ready for Google Chrome Dev.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AppScopedKeyPost,
                    true,
                    InterferenceLevel::BackgroundSafe,
                    "App-scoped key posting through Accessibility.",
                ),
                sample_execution_channel(
                    ExecutionChannel::GlobalTakeover,
                    true,
                    InterferenceLevel::GlobalTakeover,
                    "System Events takeover is ready for granted targets.",
                ),
            ],
            assistive_overlays: Vec::new(),
            recommended_services: Vec::new(),
            summary: "all tracked host access services are granted".to_string(),
        }
    }

    fn partial_host_access_status() -> HostAccessStatus {
        HostAccessStatus {
            platform: "macos".to_string(),
            app_targets: vec![
                "Google Chrome Dev".to_string(),
                "Google Chrome".to_string(),
                "Chromium".to_string(),
            ],
            services: vec![
                sample_probe(HostAccessService::Accessibility, PermissionState::Missing),
                sample_probe(HostAccessService::ScreenCapture, PermissionState::Missing),
                sample_probe(HostAccessService::ListenEvent, PermissionState::Missing),
                sample_probe(
                    HostAccessService::AppleEventsChrome,
                    PermissionState::Missing,
                ),
                sample_probe(
                    HostAccessService::AppleEventsChromeDev,
                    PermissionState::Missing,
                ),
                sample_probe(
                    HostAccessService::AppleEventsChromium,
                    PermissionState::Missing,
                ),
                sample_probe(
                    HostAccessService::DevtoolsSecurity,
                    PermissionState::Granted,
                ),
            ],
            execution_channels: vec![
                sample_execution_channel(
                    ExecutionChannel::Cdp,
                    true,
                    InterferenceLevel::BackgroundSafe,
                    "Primary meshing channel for page and tab control.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AxDirect,
                    false,
                    InterferenceLevel::BackgroundSafe,
                    "Direct macOS Accessibility action and discovery path.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AppleEventsActivation,
                    false,
                    InterferenceLevel::AppTakeover,
                    "Automation fallback is app-target specific and currently unverified.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AppScopedKeyPost,
                    false,
                    InterferenceLevel::BackgroundSafe,
                    "App-scoped key posting through Accessibility.",
                ),
                sample_execution_channel(
                    ExecutionChannel::GlobalTakeover,
                    false,
                    InterferenceLevel::GlobalTakeover,
                    "System Events takeover requires Listen Event and Apple Events permission.",
                ),
            ],
            assistive_overlays: Vec::new(),
            recommended_services: vec![
                HostAccessService::Accessibility,
                HostAccessService::ScreenCapture,
                HostAccessService::ListenEvent,
                HostAccessService::AppleEventsChrome,
                HostAccessService::AppleEventsChromeDev,
                HostAccessService::AppleEventsChromium,
            ],
            summary: "host access permissions are only partially granted".to_string(),
        }
    }

    fn ungranted_host_access_status() -> HostAccessStatus {
        HostAccessStatus {
            platform: "macos".to_string(),
            app_targets: vec![
                "Google Chrome Dev".to_string(),
                "Google Chrome".to_string(),
                "Chromium".to_string(),
            ],
            services: known_host_access_services()
                .into_iter()
                .map(|service| sample_probe(service, PermissionState::Missing))
                .collect(),
            execution_channels: vec![
                sample_execution_channel(
                    ExecutionChannel::Cdp,
                    true,
                    InterferenceLevel::BackgroundSafe,
                    "Primary meshing channel for page and tab control.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AxDirect,
                    false,
                    InterferenceLevel::BackgroundSafe,
                    "Direct macOS Accessibility action and discovery path.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AppleEventsActivation,
                    false,
                    InterferenceLevel::AppTakeover,
                    "Automation fallback is app-target specific and currently unverified.",
                ),
                sample_execution_channel(
                    ExecutionChannel::AppScopedKeyPost,
                    false,
                    InterferenceLevel::BackgroundSafe,
                    "App-scoped key posting through Accessibility.",
                ),
                sample_execution_channel(
                    ExecutionChannel::GlobalTakeover,
                    false,
                    InterferenceLevel::GlobalTakeover,
                    "System Events takeover requires Listen Event and Apple Events permission.",
                ),
            ],
            assistive_overlays: Vec::new(),
            recommended_services: known_host_access_services(),
            summary: "no tracked host access services are currently granted".to_string(),
        }
    }

    fn sample_probe(service: HostAccessService, state: PermissionState) -> HostAccessProbe {
        HostAccessProbe {
            service: service.clone(),
            state: state.clone(),
            requestable: true,
            open_settings_url: Some(format!("x-apple.systempreferences:{}", service.as_str())),
            detail: match state {
                PermissionState::Granted => "granted".to_string(),
                PermissionState::Missing => "missing".to_string(),
                PermissionState::Unsupported => "unsupported".to_string(),
                PermissionState::Unknown => "unknown".to_string(),
            },
        }
    }

    fn sample_execution_channel(
        channel: ExecutionChannel,
        available: bool,
        interference_level: InterferenceLevel,
        detail: &str,
    ) -> ExecutionChannelAvailability {
        ExecutionChannelAvailability {
            channel,
            available,
            interference_level,
            detail: detail.to_string(),
        }
    }

    fn assert_diagnose_schema(value: &Value) {
        let top_level_keys = value
            .as_object()
            .expect("diagnose report object")
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            top_level_keys,
            BTreeSet::from([
                "browser_channels".to_string(),
                "capabilities".to_string(),
                "full_capability".to_string(),
                "generated_at".to_string(),
                "permissions".to_string(),
                "platform".to_string(),
                "remediations".to_string(),
                "runtime_root".to_string(),
                "schema_version".to_string(),
                "services".to_string(),
                "state".to_string(),
                "summary".to_string(),
            ])
        );
        for permission in value["permissions"].as_array().expect("permissions array") {
            assert_eq!(
                permission
                    .as_object()
                    .expect("permission object")
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "detail".to_string(),
                    "id".to_string(),
                    "remediation_ids".to_string(),
                    "requestable".to_string(),
                    "service".to_string(),
                    "state".to_string(),
                ])
            );
        }
        for channel in value["browser_channels"]
            .as_array()
            .expect("browser channels array")
        {
            assert_eq!(
                channel
                    .as_object()
                    .expect("browser channel object")
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "app_path".to_string(),
                    "binary_path".to_string(),
                    "channel".to_string(),
                    "detail".to_string(),
                    "id".to_string(),
                    "installed".to_string(),
                    "managed_launch_ready".to_string(),
                    "native_surface_ready".to_string(),
                    "remediation_ids".to_string(),
                ])
            );
        }
        for service in value["services"].as_array().expect("services array") {
            assert_eq!(
                service
                    .as_object()
                    .expect("service object")
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "detail".to_string(),
                    "id".to_string(),
                    "remediation_ids".to_string(),
                    "state".to_string(),
                ])
            );
        }
        for capability in value["capabilities"]
            .as_array()
            .expect("capabilities array")
        {
            assert_eq!(
                capability
                    .as_object()
                    .expect("capability object")
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "blockers".to_string(),
                    "detail".to_string(),
                    "id".to_string(),
                    "remediation_ids".to_string(),
                    "state".to_string(),
                ])
            );
        }
        for remediation in value["remediations"]
            .as_array()
            .expect("remediations array")
        {
            assert_eq!(
                remediation
                    .as_object()
                    .expect("remediation object")
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "cli_command".to_string(),
                    "http_body".to_string(),
                    "http_method".to_string(),
                    "http_route".to_string(),
                    "id".to_string(),
                    "manual_only".to_string(),
                    "mcp_arguments".to_string(),
                    "mcp_tool".to_string(),
                    "summary".to_string(),
                    "title".to_string(),
                ])
            );
        }
    }

    fn runtime_for_test(entrypoint: &str) -> (TempDir, StageOneRuntime) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let runtime = StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), entrypoint)
            .expect("runtime");
        (tempdir, runtime)
    }

    fn sample_png_bytes() -> Vec<u8> {
        let image = image::RgbaImage::from_fn(32, 32, |_x, _y| image::Rgba([0, 160, 255, 255]));
        let mut buffer = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut buffer, image::ImageFormat::Png)
            .expect("encode png");
        buffer.into_inner()
    }

    struct ManagedInstanceGuard<'a> {
        runtime: &'a StageOneRuntime,
        instance_id: Option<String>,
        holder_id: Option<String>,
    }

    impl<'a> ManagedInstanceGuard<'a> {
        fn new(runtime: &'a StageOneRuntime, instance_id: &str, holder_id: Option<&str>) -> Self {
            Self {
                runtime,
                instance_id: Some(instance_id.to_string()),
                holder_id: holder_id.map(ToOwned::to_owned),
            }
        }
    }

    impl Drop for ManagedInstanceGuard<'_> {
        fn drop(&mut self) {
            if let Some(instance_id) = self.instance_id.take() {
                let _ = self
                    .runtime
                    .stop_instance(&instance_id, self.holder_id.as_deref());
            }
        }
    }

    struct ExternalAttachEnvGuard {
        _lock: Option<MutexGuard<'static, ()>>,
    }

    impl ExternalAttachEnvGuard {
        fn enable() -> Self {
            static EXTERNAL_ATTACH_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
            let lock = EXTERNAL_ATTACH_MUTEX
                .get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // Test-scoped env mutation is serialized within this process and
            // reverted in Drop immediately after the attach proof completes.
            unsafe { std::env::set_var("PENGU_MESH_ALLOW_EXTERNAL_ATTACH", "1") };
            Self { _lock: Some(lock) }
        }
    }

    impl Drop for ExternalAttachEnvGuard {
        fn drop(&mut self) {
            // See enable(); the mutation is test-scoped and reverted here.
            unsafe { std::env::remove_var("PENGU_MESH_ALLOW_EXTERNAL_ATTACH") };
            let _ = self._lock.take();
        }
    }

    struct DelayedHttpServer {
        bind_port: u16,
        stop: Arc<AtomicBool>,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    impl DelayedHttpServer {
        fn spawn(delay: Duration) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed http server");
            listener
                .set_nonblocking(true)
                .expect("set delayed http server nonblocking");
            let bind_port = listener
                .local_addr()
                .expect("delayed http server addr")
                .port();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_flag = Arc::clone(&stop);
            let join_handle = thread::spawn(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let mut buffer = [0_u8; 2048];
                            let _ = stream.read(&mut buffer);
                            thread::sleep(delay);
                            let body =
                                "<!doctype html><title>Slow</title><body>Slow response</body>";
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                bind_port,
                stop,
                join_handle: Some(join_handle),
            }
        }

        fn url(&self) -> String {
            format!("http://127.0.0.1:{}/slow", self.bind_port)
        }
    }

    impl Drop for DelayedHttpServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.bind_port));
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
            }
        }
    }

    struct FakeDebugServer {
        bind_port: u16,
        browser_ws_suffix: Arc<Mutex<String>>,
        list_status: Arc<AtomicU16>,
        stop: Arc<AtomicBool>,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    impl FakeDebugServer {
        fn spawn() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake debug server");
            listener
                .set_nonblocking(true)
                .expect("set fake debug server nonblocking");
            let bind_addr = listener.local_addr().expect("fake debug server addr");
            let list_payload = format!(
                "[{{\"id\":\"FAKEPAGE\",\"title\":\"Fake Page\",\"url\":\"https://example.com/\",\"type\":\"page\",\"webSocketDebuggerUrl\":\"ws://127.0.0.1:{}/devtools/page/FAKEPAGE\"}}]",
                bind_addr.port()
            );
            let browser_ws_suffix = Arc::new(Mutex::new("live-endpoint".to_string()));
            let list_status = Arc::new(AtomicU16::new(200));
            let stop = Arc::new(AtomicBool::new(false));
            let endpoint_state = Arc::clone(&browser_ws_suffix);
            let list_status_state = Arc::clone(&list_status);
            let stop_flag = Arc::clone(&stop);
            let join_handle = thread::spawn(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let request = read_http_request(&mut stream);
                            let request_target =
                                request.lines().next().and_then(http_request_target);
                            if request_target.is_none() {
                                continue;
                            }
                            let browser_ws_url = format!(
                                "ws://127.0.0.1:{}/devtools/browser/{}",
                                bind_addr.port(),
                                endpoint_state
                                    .lock()
                                    .expect("browser ws suffix lock")
                                    .as_str()
                            );
                            let version_payload = format!(
                                "{{\"Browser\":\"Chrome Dev 136.0\",\"Protocol-Version\":\"1.3\",\"User-Agent\":\"Fake\",\"webSocketDebuggerUrl\":\"{}\"}}",
                                browser_ws_url
                            );
                            let (status, body) =
                                if request_target_matches(request_target, "/json/version") {
                                    ("200 OK", version_payload.as_str())
                                } else if request_target_matches(request_target, "/json/list") {
                                    if list_status_state.load(Ordering::Relaxed) == 200 {
                                        ("200 OK", list_payload.as_str())
                                    } else {
                                        ("500 Internal Server Error", "{}")
                                    }
                                } else {
                                    ("404 Not Found", "{}")
                                };
                            let response = format!(
                                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                bind_port: bind_addr.port(),
                browser_ws_suffix,
                list_status,
                stop,
                join_handle: Some(join_handle),
            }
        }

        fn browser_ws_url(&self) -> String {
            format!(
                "ws://127.0.0.1:{}/devtools/browser/{}",
                self.bind_port,
                self.browser_ws_suffix
                    .lock()
                    .expect("browser ws suffix lock")
                    .as_str()
            )
        }

        fn rotate_browser_ws_url(&self, suffix: &str) {
            *self
                .browser_ws_suffix
                .lock()
                .expect("browser ws suffix lock") = suffix.to_string();
        }

        fn set_list_status(&self, status: u16) {
            self.list_status.store(status, Ordering::Relaxed);
        }
    }

    impl Drop for FakeDebugServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n")
                        || request.len() >= 8192
                    {
                        break;
                    }
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if request.is_empty() && Instant::now() < deadline {
                        continue;
                    }
                    break;
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&request).into_owned()
    }

    fn http_request_target(request_line: &str) -> Option<&str> {
        request_line.split_whitespace().nth(1)
    }

    fn request_target_matches(request_target: Option<&str>, expected_path: &str) -> bool {
        request_target
            .is_some_and(|target| target == expected_path || target.ends_with(expected_path))
    }
}
