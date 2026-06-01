use crate::OutcomeCode;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BrowserChannel {
    #[serde(rename = "chrome")]
    Chrome,
    #[serde(rename = "chrome_dev", alias = "chrome-dev")]
    ChromeDev,
    #[serde(rename = "chromium")]
    Chromium,
}

impl BrowserChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::ChromeDev => "chrome-dev",
            Self::Chromium => "chromium",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceMode {
    Managed,
    Attached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Starting,
    Running,
    Attached,
    Closed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseResourceKind {
    Instance,
}

impl LeaseResourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Instance => "instance",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseMode {
    Writer,
    Observer,
}

impl LeaseMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Writer => "writer",
            Self::Observer => "observer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Snapshot,
    Text,
    Screenshot,
    Pdf,
    Trace,
    Recording,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedRegion {
    pub x_min: u16,
    pub y_min: u16,
    pub x_max: u16,
    pub y_max: u16,
}

impl NormalizedRegion {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.x_min < self.x_max, "x_min must be less than x_max");
        anyhow::ensure!(self.y_min < self.y_max, "y_min must be less than y_max");
        anyhow::ensure!(self.x_max <= 999, "x_max must be at most 999");
        anyhow::ensure!(self.y_max <= 999, "y_max must be at most 999");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactProvenance {
    pub source_artifact_id: Option<String>,
    pub crop_region: Option<NormalizedRegion>,
    pub page_index: Option<u32>,
}

impl ArtifactProvenance {
    pub fn primary() -> Self {
        Self {
            source_artifact_id: None,
            crop_region: None,
            page_index: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserInstall {
    pub channel: BrowserChannel,
    pub installed: bool,
    pub app_path: String,
    pub binary_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedProfile {
    pub id: String,
    pub name: String,
    pub channel: BrowserChannel,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserInstance {
    pub id: String,
    pub name: String,
    pub channel: BrowserChannel,
    pub mode: InstanceMode,
    pub status: InstanceStatus,
    pub debug_http_url: String,
    pub browser_ws_url: Option<String>,
    pub profile_id: Option<String>,
    pub profile_path: Option<String>,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRecord {
    pub id: String,
    pub resource_kind: LeaseResourceKind,
    pub resource_id: String,
    pub holder_id: String,
    pub holder_label: Option<String>,
    pub mode: LeaseMode,
    pub granted_at: String,
    pub expires_at: String,
    pub last_heartbeat_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseStatusPayload {
    pub operator_id: String,
    pub requested_resource_id: Option<String>,
    pub leases: Vec<LeaseRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseAcquirePayload {
    pub operator_id: String,
    pub lease: LeaseRecord,
    pub leases: Vec<LeaseRecord>,
    pub renewed: bool,
    pub code: OutcomeCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseReleasePayload {
    pub operator_id: String,
    pub requested_resource_id: String,
    pub holder_id: String,
    pub released_count: usize,
    pub leases: Vec<LeaseRecord>,
    pub code: OutcomeCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseTransferPayload {
    pub operator_id: String,
    pub previous_holder_id: String,
    pub lease: LeaseRecord,
    pub leases: Vec<LeaseRecord>,
    pub code: OutcomeCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachResolutionKind {
    DebugHttpUrl,
    BrowserWsUrl,
    Name,
    NewInstance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachContinuityOutcome {
    NewInstance,
    ReusedExistingInstance,
    ReclaimedStaleInstance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachContinuityFreshness {
    None,
    Live,
    StaleInstance,
    StaleEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseDisposition {
    WriterRequired,
    ObserverRequired,
    OutsideModel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AttachContinuityStatus {
    pub outcome: Option<AttachContinuityOutcome>,
    pub freshness: AttachContinuityFreshness,
    pub last_resolution: Option<AttachResolutionKind>,
    pub last_instance_id: Option<String>,
    pub last_debug_http_url: Option<String>,
    pub last_requested_cdp_url: Option<String>,
    pub last_browser_ws_url: Option<String>,
    pub reused_existing_instance: bool,
    pub endpoint_refreshed: bool,
    pub updated_at: Option<String>,
}

impl Default for AttachContinuityStatus {
    fn default() -> Self {
        Self {
            outcome: None,
            freshness: AttachContinuityFreshness::None,
            last_resolution: None,
            last_instance_id: None,
            last_debug_http_url: None,
            last_requested_cdp_url: None,
            last_browser_ws_url: None,
            reused_existing_instance: false,
            endpoint_refreshed: false,
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseCoverageEntry {
    pub operation: String,
    pub cli_command: Option<String>,
    pub mcp_tool: Option<String>,
    pub http_method: Option<String>,
    pub http_route: Option<String>,
    pub disposition: LeaseDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionChannel {
    Cdp,
    AxDirect,
    AppleEventsActivation,
    AppScopedKeyPost,
    GlobalTakeover,
}

impl ExecutionChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cdp => "cdp",
            Self::AxDirect => "ax_direct",
            Self::AppleEventsActivation => "apple_events_activation",
            Self::AppScopedKeyPost => "app_scoped_key_post",
            Self::GlobalTakeover => "global_takeover",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterferenceLevel {
    BackgroundSafe,
    AppTakeover,
    GlobalTakeover,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostAccessService {
    Accessibility,
    ScreenCapture,
    ListenEvent,
    AppleEventsChrome,
    AppleEventsChromeDev,
    AppleEventsChromium,
    DevtoolsSecurity,
}

impl HostAccessService {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::ScreenCapture => "screen_capture",
            Self::ListenEvent => "listen_event",
            Self::AppleEventsChrome => "apple_events_chrome",
            Self::AppleEventsChromeDev => "apple_events_chrome_dev",
            Self::AppleEventsChromium => "apple_events_chromium",
            Self::DevtoolsSecurity => "devtools_security",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Granted,
    Missing,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostAccessProbe {
    pub service: HostAccessService,
    pub state: PermissionState,
    pub requestable: bool,
    pub open_settings_url: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionChannelAvailability {
    pub channel: ExecutionChannel,
    pub available: bool,
    pub interference_level: InterferenceLevel,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistiveOverlayDescriptor {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub primary_use: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostAccessStatus {
    pub platform: String,
    pub app_targets: Vec<String>,
    pub services: Vec<HostAccessProbe>,
    pub execution_channels: Vec<ExecutionChannelAvailability>,
    pub assistive_overlays: Vec<AssistiveOverlayDescriptor>,
    pub recommended_services: Vec<HostAccessService>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostAccessSetupMode {
    Audit,
    Apply,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostAccessSetupRequest {
    pub mode: HostAccessSetupMode,
    pub services: Vec<HostAccessService>,
    pub open_settings_on_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostAccessSetupStep {
    pub service: HostAccessService,
    pub action: String,
    pub ok: bool,
    pub state: PermissionState,
    pub detail: String,
    pub opened_settings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostAccessSetupResult {
    pub mode: HostAccessSetupMode,
    pub before: HostAccessStatus,
    pub after: HostAccessStatus,
    pub steps: Vec<HostAccessSetupStep>,
    pub changed_services: Vec<HostAccessService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnoseState {
    Ready,
    Degraded,
    Blocked,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnoseServiceState {
    Reachable,
    Unreachable,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosePermission {
    pub id: String,
    pub service: HostAccessService,
    pub state: PermissionState,
    pub requestable: bool,
    pub detail: String,
    pub remediation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnoseBrowserChannel {
    pub id: String,
    pub channel: BrowserChannel,
    pub installed: bool,
    pub managed_launch_ready: bool,
    pub native_surface_ready: bool,
    pub app_path: String,
    pub binary_path: String,
    pub detail: String,
    pub remediation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnoseService {
    pub id: String,
    pub state: DiagnoseServiceState,
    pub detail: String,
    pub remediation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnoseCapability {
    pub id: String,
    pub state: DiagnoseState,
    pub detail: String,
    pub blockers: Vec<String>,
    pub remediation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnoseRemediation {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub cli_command: Option<String>,
    pub mcp_tool: Option<String>,
    pub mcp_arguments: Option<Value>,
    pub http_method: Option<String>,
    pub http_route: Option<String>,
    pub http_body: Option<Value>,
    pub manual_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnoseReport {
    pub schema_version: String,
    pub generated_at: String,
    pub platform: String,
    pub full_capability: bool,
    pub state: DiagnoseState,
    pub summary: String,
    pub runtime_root: String,
    pub scenario_evidence: ScenarioEvidenceStatus,
    pub permissions: Vec<DiagnosePermission>,
    pub browser_channels: Vec<DiagnoseBrowserChannel>,
    pub services: Vec<DiagnoseService>,
    pub capabilities: Vec<DiagnoseCapability>,
    pub remediations: Vec<DiagnoseRemediation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioEvidenceStatus {
    pub state: DiagnoseState,
    pub summary: String,
    pub total_runs: usize,
    pub passing_families: usize,
    pub degraded_families: usize,
    pub families: Vec<ScenarioFamilySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceDescriptor {
    pub id: String,
    pub parent_id: Option<String>,
    pub path: String,
    pub role: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub window_title: Option<String>,
    pub actions: Vec<String>,
    pub focused: bool,
    pub enabled: bool,
    pub app_name: String,
    #[serde(default)]
    pub bundle_id: Option<String>,
    pub channel: BrowserChannel,
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceListPayload {
    pub instance: BrowserInstance,
    pub app_name: String,
    pub surfaces: Vec<BrowserSurfaceDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceActionPathContract {
    pub execution_channel: ExecutionChannel,
    pub available: bool,
    pub required_permissions: Vec<HostAccessService>,
    pub interference_level: InterferenceLevel,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceActionContract {
    pub action: String,
    pub available: bool,
    pub required_permissions: Vec<HostAccessService>,
    pub expected_interference_level: InterferenceLevel,
    pub detail: String,
    pub execution_paths: Vec<BrowserSurfaceActionPathContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceActionCatalogPayload {
    pub instance: BrowserInstance,
    pub app_name: String,
    pub surface: BrowserSurfaceDescriptor,
    pub actions: Vec<BrowserSurfaceActionContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceSnapshot {
    pub instance: BrowserInstance,
    pub app_name: String,
    pub root_surface_id: Option<String>,
    pub snapshot_artifact: ArtifactHandle,
    pub capture_artifact: Option<ArtifactHandle>,
    pub capture_source: Option<String>,
    pub capture_detail: Option<String>,
    pub surfaces: Vec<BrowserSurfaceDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceActionKind {
    Press,
    Focus,
    Confirm,
    SetValue,
    KeySequence,
    Scroll,
    Increment,
    Decrement,
    ShowMenu,
    Pick,
    Raise,
    Cancel,
}

impl SurfaceActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::Focus => "focus",
            Self::Confirm => "confirm",
            Self::SetValue => "set_value",
            Self::KeySequence => "key_sequence",
            Self::Scroll => "scroll",
            Self::Increment => "increment",
            Self::Decrement => "decrement",
            Self::ShowMenu => "show_menu",
            Self::Pick => "pick",
            Self::Raise => "raise",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceActionRequest {
    pub surface_id: Option<String>,
    pub action: SurfaceActionKind,
    pub value: Option<String>,
    pub key_sequence: Option<String>,
    pub execution_channel: Option<ExecutionChannel>,
    pub allow_takeover: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceActionPayload {
    pub instance: BrowserInstance,
    pub app_name: String,
    pub target_surface_id: Option<String>,
    pub requested: BrowserSurfaceActionRequest,
    pub resolved_channel: ExecutionChannel,
    pub interference_level: InterferenceLevel,
    pub took_focus: bool,
    pub fallback_count: u8,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceFailureAttempt {
    pub instance_id: String,
    pub surface_id: Option<String>,
    pub root_surface_id: Option<String>,
    pub action: Option<SurfaceActionKind>,
    pub execution_channel: Option<ExecutionChannel>,
    pub allow_takeover: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSurfaceFailurePayload {
    pub operation: String,
    pub attempted: BrowserSurfaceFailureAttempt,
    pub reason: String,
    pub recovery: Vec<String>,
    pub retry_likely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabFailureAttempt {
    pub instance_id: Option<String>,
    pub tab_id: Option<String>,
    pub action_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabFailurePayload {
    pub operation: String,
    pub attempted: TabFailureAttempt,
    pub reason: String,
    pub recovery: Vec<String>,
    pub retry_likely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactFailureAttempt {
    pub artifact_id: Option<String>,
    pub instance_id: Option<String>,
    pub run_id: Option<String>,
    pub action_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactFailurePayload {
    pub operation: String,
    pub attempted: ArtifactFailureAttempt,
    pub reason: String,
    pub recovery: Vec<String>,
    pub retry_likely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationFailureAttempt {
    pub operation: String,
    pub instance_id: Option<String>,
    pub holder_id: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationFailurePayload {
    pub operation: String,
    pub attempted: OperationFailureAttempt,
    pub reason: String,
    pub recovery: Vec<String>,
    pub retry_likely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserTab {
    pub id: String,
    pub instance_id: String,
    pub target_id: String,
    pub title: String,
    pub url: String,
    pub websocket_url: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TabActionKind {
    Navigate,
    Evaluate,
    Click,
    Focus,
    Hover,
    Fill,
    Type,
    Press,
    Select,
}

impl TabActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Navigate => "navigate",
            Self::Evaluate => "evaluate",
            Self::Click => "click",
            Self::Focus => "focus",
            Self::Hover => "hover",
            Self::Fill => "fill",
            Self::Type => "type",
            Self::Press => "press",
            Self::Select => "select",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabActionRequest {
    pub kind: TabActionKind,
    #[serde(rename = "ref")]
    pub ref_id: Option<String>,
    pub selector: Option<String>,
    pub url: Option<String>,
    pub timeout_ms: Option<u64>,
    pub expression: Option<String>,
    pub text: Option<String>,
    pub value: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabActionPayload {
    pub tab: BrowserTab,
    pub requested: TabActionRequest,
    pub resolved_target: String,
    pub detail: String,
    pub final_url: Option<String>,
    pub load_event_fired: Option<bool>,
    pub duration_ms: Option<u64>,
    pub result: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabActionContract {
    pub kind: String,
    pub available: bool,
    pub required_permissions: Vec<HostAccessService>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabActionCatalogPayload {
    pub instance: BrowserInstance,
    pub tab: BrowserTab,
    pub actions: Vec<TabActionContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactHandle {
    pub id: String,
    pub run_id: Option<String>,
    pub instance_id: String,
    pub tab_id: String,
    pub kind: ArtifactKind,
    pub path: String,
    pub mime_type: String,
    pub bytes: usize,
    pub created_at: String,
    pub checksum_sha256: Option<String>,
    pub provenance: ArtifactProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactListEntry {
    pub id: String,
    pub kind: ArtifactKind,
    pub path: String,
    pub sha256: Option<String>,
    pub size_bytes: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactListPayload {
    pub instance_id: Option<String>,
    pub run_id: Option<String>,
    pub artifacts: Vec<ArtifactListEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactVerifyPayload {
    pub id: String,
    pub path: String,
    pub expected_sha256: Option<String>,
    pub actual_sha256: String,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Active,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureRun {
    pub id: String,
    pub entrypoint: String,
    pub detail: String,
    pub status: RunStatus,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayExportMode {
    ManifestOnly,
    Portable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InspectionMode {
    QuickRead,
    FaithfulExtract,
    CompositionalInspect,
    MultiPassInspect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectionModeContract {
    pub mode: InspectionMode,
    pub summary: String,
    pub recommended_tools: Vec<String>,
    pub replay_mode: ReplayExportMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeEvent {
    pub schema_version: u32,
    pub id: String,
    pub run_id: String,
    pub sequence: u64,
    pub category: String,
    pub action: String,
    pub level: EventLevel,
    pub message: String,
    pub instance_id: Option<String>,
    pub tab_id: Option<String>,
    pub artifact_id: Option<String>,
    pub data: Value,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventTailPayload {
    pub run: Option<CaptureRun>,
    pub requested_limit: usize,
    pub events: Vec<RuntimeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunListPayload {
    pub requested_limit: usize,
    pub runs: Vec<CaptureRun>,
}

// Shared task plane contract types.
// These cover both the scheduler-facing descriptors and the persisted task rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDescriptor {
    pub id: String,
    pub agent_id: String,
    pub action: String,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub params: Option<Value>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRecord {
    pub id: String,
    pub agent_id: String,
    pub action: String,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub params: Option<Value>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub output: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioRun {
    pub id: String,
    pub scenario_name: String,
    pub scenario_family: String,
    pub scenario_version: String,
    pub tool_surface: String,
    pub runtime_root: Option<String>,
    pub commit_sha: Option<String>,
    pub branch_name: Option<String>,
    pub platform: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub summary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioStep {
    pub id: String,
    pub run_id: String,
    pub ordinal: i64,
    pub step_name: String,
    pub step_kind: String,
    pub command_line: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioAssertion {
    pub id: String,
    pub run_id: String,
    pub step_id: Option<String>,
    pub assertion_name: String,
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub status: String,
    pub failure_category: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatencySample {
    pub id: String,
    pub run_id: String,
    pub step_id: Option<String>,
    pub metric_name: String,
    pub sample_ms: OrderedFloat<f64>,
    pub capture_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvironmentFingerprint {
    pub id: String,
    pub run_id: String,
    pub platform: String,
    pub arch: String,
    pub os_version: Option<String>,
    pub rust_version: Option<String>,
    pub cargo_version: Option<String>,
    pub chrome_channel: Option<String>,
    pub chrome_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioListPayload {
    pub requested_family: Option<String>,
    pub requested_limit: usize,
    pub runs: Vec<ScenarioRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioStatusCount {
    pub status: String,
    pub runs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioFamilySummary {
    pub scenario_family: String,
    pub runs: usize,
    pub statuses: Vec<ScenarioStatusCount>,
    pub assertion_total: usize,
    pub assertion_failures: usize,
    pub latency_sample_count: usize,
    pub latency_min_ms: Option<OrderedFloat<f64>>,
    pub latency_median_ms: Option<OrderedFloat<f64>>,
    pub latency_max_ms: Option<OrderedFloat<f64>>,
    pub latest_run_id: Option<String>,
    pub latest_status: Option<String>,
    pub latest_started_at: Option<String>,
    pub latest_commit_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioSummaryPayload {
    pub requested_family: Option<String>,
    pub requested_limit: usize,
    pub total_runs: usize,
    pub families: Vec<ScenarioFamilySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioLatencyThreshold {
    pub name: String,
    pub metric: String,
    pub max_ms: u64,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub p99_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioGatePolicy {
    pub min_runs: usize,
    pub allowed_statuses: Vec<String>,
    pub max_assertion_failures: usize,
    pub min_samples_per_metric: usize,
    pub max_latest_age_minutes: Option<u64>,
    pub thresholds: Vec<ScenarioLatencyThreshold>,
}

impl Default for ScenarioGatePolicy {
    fn default() -> Self {
        Self {
            min_runs: 1,
            allowed_statuses: vec!["passed".to_string()],
            max_assertion_failures: 0,
            min_samples_per_metric: 1,
            max_latest_age_minutes: None,
            thresholds: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioGateCheck {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioGateThresholdViolation {
    pub threshold_name: String,
    pub metric: String,
    pub expected_ms: u64,
    pub actual_ms: u64,
    pub percentile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioGateThresholdResult {
    pub threshold_name: String,
    pub metric: String,
    pub passed: bool,
    pub samples_evaluated: usize,
    pub violations: Vec<ScenarioGateThresholdViolation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioGatePayload {
    pub requested_family: Option<String>,
    pub requested_limit: usize,
    pub policy: ScenarioGatePolicy,
    pub passed: bool,
    pub summary: ScenarioSummaryPayload,
    pub checks: Vec<ScenarioGateCheck>,
    pub thresholds: Vec<ScenarioGateThresholdResult>,
    pub recovery: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioRunDetailPayload {
    pub run: ScenarioRun,
    pub steps: Vec<ScenarioStep>,
    pub assertions: Vec<ScenarioAssertion>,
    pub latency_samples: Vec<LatencySample>,
    pub environment_fingerprint: Option<EnvironmentFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayManifest {
    pub schema_version: u32,
    pub exported_at: String,
    pub mode: ReplayExportMode,
    pub bundle: ReplayBundleMetadata,
    pub run: CaptureRun,
    pub inspection_modes: Vec<InspectionModeContract>,
    pub events: Vec<RuntimeEvent>,
    pub artifacts: Vec<ReplayArtifactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayBundleMetadata {
    pub root_path: String,
    pub manifest_path: String,
    pub artifact_root: Option<String>,
    pub staged_atomically: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayArtifactRecord {
    pub artifact_id: String,
    pub run_id: Option<String>,
    pub instance_id: String,
    pub tab_id: String,
    pub kind: ArtifactKind,
    pub path: String,
    pub mime_type: String,
    pub bytes: usize,
    pub created_at: String,
    pub materialized: bool,
    pub checksum_sha256: Option<String>,
    pub provenance: ArtifactProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayManifestExport {
    pub run: CaptureRun,
    pub mode: ReplayExportMode,
    pub manifest_path: String,
    pub bundle_root: String,
    pub event_count: usize,
    pub artifact_count: usize,
    pub exported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonMetadata {
    pub bind_addr: String,
    pub pid: u32,
    pub entrypoint: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePaths {
    pub root_dir: String,
    pub state_db_path: String,
    pub profile_dir: String,
    pub artifact_dir: String,
    pub replay_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePosture {
    pub tier_one_platform: String,
    pub core_language: String,
    pub mcp_mode: String,
    pub control_plane: String,
    pub dashboard_status: String,
}

impl RuntimePosture {
    pub fn stage_one() -> Self {
        Self {
            tier_one_platform: "darwin/arm64".to_string(),
            core_language: "rust".to_string(),
            mcp_mode: "native-stdio".to_string(),
            control_plane: "local-http-daemon-capable-cli-runtime-active".to_string(),
            dashboard_status: "read-only-health-scaffold".to_string(),
        }
    }
}

pub fn inspection_modes() -> Vec<InspectionModeContract> {
    vec![
        InspectionModeContract {
            mode: InspectionMode::QuickRead,
            summary: "Start with the cheapest readable view before escalating capture cost."
                .to_string(),
            recommended_tools: vec![
                "tab_text".to_string(),
                "events_tail".to_string(),
                "replay_export(manifest_only)".to_string(),
            ],
            replay_mode: ReplayExportMode::ManifestOnly,
        },
        InspectionModeContract {
            mode: InspectionMode::FaithfulExtract,
            summary: "Preserve layout and visual structure when literal extraction matters."
                .to_string(),
            recommended_tools: vec![
                "tab_snapshot".to_string(),
                "tab_text".to_string(),
                "tab_screenshot".to_string(),
                "tab_pdf".to_string(),
                "replay_export(manifest_only)".to_string(),
            ],
            replay_mode: ReplayExportMode::ManifestOnly,
        },
        InspectionModeContract {
            mode: InspectionMode::CompositionalInspect,
            summary:
                "Capture richer evidence before higher-cost reasoning across multiple regions."
                    .to_string(),
            recommended_tools: vec![
                "tab_snapshot".to_string(),
                "tab_screenshot".to_string(),
                "tab_pdf".to_string(),
                "artifact_crop".to_string(),
                "artifact_crop_grid".to_string(),
                "replay_export(manifest_only)".to_string(),
            ],
            replay_mode: ReplayExportMode::ManifestOnly,
        },
        InspectionModeContract {
            mode: InspectionMode::MultiPassInspect,
            summary:
                "Use narrow derived crops and portable bundles for repeatable handoff and reruns."
                    .to_string(),
            recommended_tools: vec![
                "artifact_crop".to_string(),
                "artifact_crop_grid".to_string(),
                "trace_capture".to_string(),
                "recording_capture".to_string(),
                "events_tail".to_string(),
                "replay_export(portable)".to_string(),
            ],
            replay_mode: ReplayExportMode::Portable,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmptyPayload {
    pub detail: String,
}

impl EmptyPayload {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipScope {
    Instance,
    Tab,
    Artifact,
    Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Local,
    SharedSecret,
    Certificate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipToken {
    pub value: String,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthenticatedHolder {
    pub holder_id: String,
    pub token: OwnershipToken,
    pub scopes: Vec<OwnershipScope>,
    pub issued_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipDenialAttempt {
    pub holder_id: String,
    pub scope: OwnershipScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipDenialPayload {
    pub operation: String,
    pub attempted: OwnershipDenialAttempt,
    pub reason: String,
    pub recovery: Vec<String>,
    pub retry_likely: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactFailureAttempt, AuthenticatedHolder, BrowserChannel, BrowserSurfaceDescriptor,
        BrowserTab, DiagnoseState, EnvironmentFingerprint, LatencySample, OperationFailureAttempt,
        OwnershipDenialAttempt, OwnershipDenialPayload, OwnershipScope, OwnershipToken,
        ScenarioAssertion, ScenarioEvidenceStatus, ScenarioFamilySummary, ScenarioGateCheck,
        ScenarioGatePayload, ScenarioGatePolicy, ScenarioGateThresholdResult,
        ScenarioLatencyThreshold, ScenarioListPayload, ScenarioRun, ScenarioRunDetailPayload,
        ScenarioStatusCount, ScenarioStep, ScenarioSummaryPayload, TabActionKind, TabActionPayload,
        TabActionRequest, TaskDescriptor, TaskPriority, TaskRecord, TaskResult, TaskState,
        TokenKind,
    };
    use serde_json::json;

    #[test]
    fn browser_channel_deserializes_hyphenated_and_snake_case_inputs() {
        let hyphenated: BrowserChannel =
            serde_json::from_str("\"chrome-dev\"").expect("deserialize hyphenated channel");
        let snake_case: BrowserChannel =
            serde_json::from_str("\"chrome_dev\"").expect("deserialize snake_case channel");

        assert_eq!(hyphenated, BrowserChannel::ChromeDev);
        assert_eq!(snake_case, BrowserChannel::ChromeDev);
        assert_eq!(
            serde_json::to_string(&BrowserChannel::ChromeDev).expect("serialize channel"),
            "\"chrome_dev\""
        );
    }

    #[test]
    fn tab_action_kind_serializes_evaluate_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&TabActionKind::Evaluate).expect("serialize evaluate"),
            "\"evaluate\""
        );
    }

    #[test]
    fn task_contract_round_trips_structured_params() {
        let task = TaskRecord {
            id: "task_demo".to_string(),
            agent_id: "agent_alpha".to_string(),
            action: "tab_screenshot".to_string(),
            state: TaskState::Running,
            priority: TaskPriority::High,
            params: Some(json!({
                "tab_id": "tab_demo",
                "full_page": true
            })),
            created_at: "2026-03-12T00:00:00Z".to_string(),
            started_at: Some("2026-03-12T00:00:01Z".to_string()),
            completed_at: None,
            latency_ms: Some(125),
        };

        let encoded = serde_json::to_value(&task).expect("serialize task");
        assert_eq!(encoded["state"], "running");
        assert_eq!(encoded["priority"], "high");
        assert_eq!(encoded["params"]["tab_id"], "tab_demo");
        assert_eq!(encoded["params"]["full_page"], true);

        let decoded: TaskRecord = serde_json::from_value(encoded).expect("deserialize task");
        assert_eq!(decoded, task);
        assert_eq!(TaskState::Cancelled.as_str(), "cancelled");
        assert_eq!(TaskPriority::Normal.as_str(), "normal");
    }

    #[test]
    fn browser_surface_descriptor_defaults_bundle_id_when_missing() {
        let descriptor: BrowserSurfaceDescriptor = serde_json::from_value(json!({
            "id": "ax:0/4",
            "parent_id": "ax:0",
            "path": "0/4",
            "role": "AXWindow",
            "title": "Example",
            "description": null,
            "value": null,
            "window_title": "Example",
            "actions": ["focus"],
            "focused": false,
            "enabled": true,
            "app_name": "Google Chrome Dev",
            "channel": "chrome_dev",
            "instance_id": "inst_demo"
        }))
        .expect("deserialize surface descriptor without bundle id");

        assert_eq!(descriptor.bundle_id, None);
        assert_eq!(descriptor.channel, BrowserChannel::ChromeDev);
    }

    #[test]
    fn tab_action_payload_round_trips_evidence_and_result_fields() {
        let payload = TabActionPayload {
            tab: BrowserTab {
                id: "tab_demo".to_string(),
                instance_id: "inst_demo".to_string(),
                target_id: "target_demo".to_string(),
                title: "Demo".to_string(),
                url: "data:text/html,hello".to_string(),
                websocket_url: "ws://127.0.0.1/devtools/page/demo".to_string(),
                active: true,
                created_at: "2026-03-12T00:00:00Z".to_string(),
                updated_at: "2026-03-12T00:00:00Z".to_string(),
            },
            requested: TabActionRequest {
                kind: TabActionKind::Evaluate,
                ref_id: None,
                selector: None,
                url: Some("data:text/html,after".to_string()),
                timeout_ms: Some(250),
                expression: Some("window.location.href".to_string()),
                text: None,
                value: None,
                key: None,
            },
            resolved_target: "page".to_string(),
            detail: "evaluated expression over CDP".to_string(),
            final_url: Some("data:text/html,after".to_string()),
            load_event_fired: Some(true),
            duration_ms: Some(42),
            result: Some(json!("data:text/html,after")),
        };

        let encoded = serde_json::to_value(&payload).expect("serialize payload");
        assert_eq!(encoded["final_url"], "data:text/html,after");
        assert_eq!(encoded["load_event_fired"], true);
        assert_eq!(encoded["duration_ms"], 42);
        assert_eq!(encoded["result"], "data:text/html,after");
        assert_eq!(encoded["requested"]["kind"], "evaluate");
        assert_eq!(encoded["requested"]["expression"], "window.location.href");
        assert_eq!(encoded["requested"]["timeout_ms"], 250);

        let decoded: TabActionPayload =
            serde_json::from_value(encoded).expect("deserialize payload");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn artifact_failure_attempt_round_trips_action_kind() {
        let attempted = ArtifactFailureAttempt {
            artifact_id: Some("artifact_demo".to_string()),
            instance_id: Some("inst_demo".to_string()),
            run_id: Some("run_demo".to_string()),
            action_kind: Some("verify".to_string()),
        };

        let encoded = serde_json::to_value(&attempted).expect("serialize artifact attempt");
        assert_eq!(encoded["action_kind"], "verify");

        let decoded: ArtifactFailureAttempt =
            serde_json::from_value(encoded).expect("deserialize artifact attempt");
        assert_eq!(decoded, attempted);
    }

    #[test]
    fn operation_failure_attempt_round_trips_detail() {
        let attempted = OperationFailureAttempt {
            operation: "instance_attach".to_string(),
            instance_id: Some("inst_demo".to_string()),
            holder_id: Some("agent_alpha".to_string()),
            detail: Some("cdp_url=ws://127.0.0.1:9222/devtools/browser/demo".to_string()),
        };

        let encoded = serde_json::to_value(&attempted).expect("serialize operation attempt");
        assert_eq!(encoded["operation"], "instance_attach");
        assert_eq!(encoded["holder_id"], "agent_alpha");
        assert_eq!(
            encoded["detail"],
            "cdp_url=ws://127.0.0.1:9222/devtools/browser/demo"
        );

        let decoded: OperationFailureAttempt =
            serde_json::from_value(encoded).expect("deserialize operation attempt");
        assert_eq!(decoded, attempted);
    }

    #[test]
    fn scenario_payloads_round_trip() {
        let run = ScenarioRun {
            id: "scenario_run_startup".to_string(),
            scenario_name: "startup-readiness".to_string(),
            scenario_family: "startup-readiness".to_string(),
            scenario_version: "v1".to_string(),
            tool_surface: "cli".to_string(),
            runtime_root: Some("/tmp/runtime-root".to_string()),
            commit_sha: Some("29e4808".to_string()),
            branch_name: Some("main".to_string()),
            platform: "darwin".to_string(),
            started_at: "2026-03-12T00:00:00Z".to_string(),
            finished_at: Some("2026-03-12T00:00:30Z".to_string()),
            status: "passed".to_string(),
            summary_path: Some("/tmp/summary.md".to_string()),
        };
        let step = ScenarioStep {
            id: "scenario_step_health".to_string(),
            run_id: run.id.clone(),
            ordinal: 1,
            step_name: "health".to_string(),
            step_kind: "command".to_string(),
            command_line: Some("pengu-mesh health".to_string()),
            started_at: "2026-03-12T00:00:01Z".to_string(),
            finished_at: Some("2026-03-12T00:00:02Z".to_string()),
            status: "passed".to_string(),
            error_code: None,
        };
        let assertion = ScenarioAssertion {
            id: "scenario_assertion_health_ok".to_string(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            assertion_name: "health ok".to_string(),
            expected_value: Some("true".to_string()),
            actual_value: Some("true".to_string()),
            status: "passed".to_string(),
            failure_category: None,
            notes: Some("health returned ok".to_string()),
        };
        let sample = LatencySample {
            id: "scenario_latency_health".to_string(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            metric_name: "health".to_string(),
            sample_ms: 12.5.into(),
            capture_method: Some("wall_clock".to_string()),
        };
        let fingerprint = EnvironmentFingerprint {
            id: "scenario_env".to_string(),
            run_id: run.id.clone(),
            platform: "darwin".to_string(),
            arch: "arm64".to_string(),
            os_version: Some("Darwin 25.0.0".to_string()),
            rust_version: Some("rustc 1.94.0".to_string()),
            cargo_version: Some("cargo 1.94.0".to_string()),
            chrome_channel: Some("chrome-dev".to_string()),
            chrome_version: Some("136.0.0.0".to_string()),
        };
        let payload = ScenarioRunDetailPayload {
            run: run.clone(),
            steps: vec![step.clone()],
            assertions: vec![assertion.clone()],
            latency_samples: vec![sample.clone()],
            environment_fingerprint: Some(fingerprint.clone()),
        };
        let list_payload = ScenarioListPayload {
            requested_family: Some("startup-readiness".to_string()),
            requested_limit: 10,
            runs: vec![run.clone()],
        };
        let summary_payload = ScenarioSummaryPayload {
            requested_family: Some("startup-readiness".to_string()),
            requested_limit: 10,
            total_runs: 1,
            families: vec![ScenarioFamilySummary {
                scenario_family: "startup-readiness".to_string(),
                runs: 1,
                statuses: vec![ScenarioStatusCount {
                    status: "passed".to_string(),
                    runs: 1,
                }],
                assertion_total: 1,
                assertion_failures: 0,
                latency_sample_count: 1,
                latency_min_ms: Some(12.5.into()),
                latency_median_ms: Some(12.5.into()),
                latency_max_ms: Some(12.5.into()),
                latest_run_id: Some(run.id.clone()),
                latest_status: Some("passed".to_string()),
                latest_started_at: Some("2026-03-12T00:00:00Z".to_string()),
                latest_commit_sha: Some("29e4808".to_string()),
            }],
        };
        let scenario_evidence = ScenarioEvidenceStatus {
            state: DiagnoseState::Ready,
            summary: "1 scenario families have latest passing evidence across 1 runs".to_string(),
            total_runs: 1,
            passing_families: 1,
            degraded_families: 0,
            families: summary_payload.families.clone(),
        };
        let gate_payload = ScenarioGatePayload {
            requested_family: Some("startup-readiness".to_string()),
            requested_limit: 10,
            policy: ScenarioGatePolicy {
                thresholds: vec![ScenarioLatencyThreshold {
                    name: "health-fast".to_string(),
                    metric: "health".to_string(),
                    max_ms: 1000,
                    p50_ms: Some(100),
                    p95_ms: None,
                    p99_ms: None,
                }],
                ..ScenarioGatePolicy::default()
            },
            passed: true,
            summary: summary_payload.clone(),
            checks: vec![ScenarioGateCheck {
                name: "minimum_runs".to_string(),
                passed: true,
                expected: "at least 1 scenario run(s)".to_string(),
                actual: "1".to_string(),
                detail: Some("family=startup-readiness".to_string()),
            }],
            thresholds: vec![ScenarioGateThresholdResult {
                threshold_name: "health-fast".to_string(),
                metric: "health".to_string(),
                passed: true,
                samples_evaluated: 1,
                violations: Vec::new(),
            }],
            recovery: Vec::new(),
        };

        assert_eq!(
            serde_json::from_value::<ScenarioRun>(serde_json::to_value(&run).expect("run json"))
                .expect("run round trip"),
            run
        );
        assert_eq!(
            serde_json::from_value::<ScenarioStep>(serde_json::to_value(&step).expect("step json"))
                .expect("step round trip"),
            step
        );
        assert_eq!(
            serde_json::from_value::<ScenarioAssertion>(
                serde_json::to_value(&assertion).expect("assertion json")
            )
            .expect("assertion round trip"),
            assertion
        );
        assert_eq!(
            serde_json::from_value::<LatencySample>(
                serde_json::to_value(&sample).expect("sample json")
            )
            .expect("sample round trip"),
            sample
        );
        assert_eq!(
            serde_json::from_value::<EnvironmentFingerprint>(
                serde_json::to_value(&fingerprint).expect("fingerprint json")
            )
            .expect("fingerprint round trip"),
            fingerprint
        );
        assert_eq!(
            serde_json::from_value::<ScenarioRunDetailPayload>(
                serde_json::to_value(&payload).expect("detail payload json")
            )
            .expect("detail payload round trip"),
            payload
        );
        assert_eq!(
            serde_json::from_value::<ScenarioListPayload>(
                serde_json::to_value(&list_payload).expect("list payload json")
            )
            .expect("list payload round trip"),
            list_payload
        );
        assert_eq!(
            serde_json::from_value::<ScenarioSummaryPayload>(
                serde_json::to_value(&summary_payload).expect("summary payload json")
            )
            .expect("summary payload round trip"),
            summary_payload
        );
        assert_eq!(
            serde_json::from_value::<ScenarioEvidenceStatus>(
                serde_json::to_value(&scenario_evidence).expect("scenario evidence json")
            )
            .expect("scenario evidence round trip"),
            scenario_evidence
        );
        assert_eq!(
            serde_json::from_value::<ScenarioGatePayload>(
                serde_json::to_value(&gate_payload).expect("gate payload json")
            )
            .expect("gate payload round trip"),
            gate_payload
        );
    }

    #[test]
    fn task_state_serde_round_trip() {
        for (variant, expected) in [
            (TaskState::Pending, "\"pending\""),
            (TaskState::Running, "\"running\""),
            (TaskState::Completed, "\"completed\""),
            (TaskState::Failed, "\"failed\""),
            (TaskState::Cancelled, "\"cancelled\""),
        ] {
            let serialized = serde_json::to_string(&variant).expect("serialize task state");
            assert_eq!(serialized, expected);
            let deserialized: TaskState =
                serde_json::from_str(&serialized).expect("deserialize task state");
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn task_priority_serde_round_trip() {
        for (variant, expected) in [
            (TaskPriority::Low, "\"low\""),
            (TaskPriority::Normal, "\"normal\""),
            (TaskPriority::High, "\"high\""),
            (TaskPriority::Critical, "\"critical\""),
        ] {
            let serialized = serde_json::to_string(&variant).expect("serialize task priority");
            assert_eq!(serialized, expected);
            let deserialized: TaskPriority =
                serde_json::from_str(&serialized).expect("deserialize task priority");
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn task_descriptor_serde_round_trip() {
        let descriptor = TaskDescriptor {
            id: "tsk_00000001".to_string(),
            agent_id: "agent_alpha".to_string(),
            action: "navigate".to_string(),
            state: TaskState::Running,
            priority: TaskPriority::High,
            params: Some(json!({"url": "https://example.com"})),
            created_at: "2026-03-12T00:00:00Z".to_string(),
            started_at: Some("2026-03-12T00:00:01Z".to_string()),
            completed_at: None,
            latency_ms: None,
        };

        let encoded = serde_json::to_value(&descriptor).expect("serialize descriptor");
        assert_eq!(encoded["state"], "running");
        assert_eq!(encoded["priority"], "high");
        assert_eq!(encoded["params"]["url"], "https://example.com");

        let decoded: TaskDescriptor =
            serde_json::from_value(encoded).expect("deserialize descriptor");
        assert_eq!(decoded, descriptor);
    }

    #[test]
    fn task_result_serde_round_trip() {
        let result = TaskResult {
            task_id: "tsk_00000001".to_string(),
            success: true,
            output: Some(json!({"status": "ok"})),
            error: None,
        };

        let encoded = serde_json::to_value(&result).expect("serialize result");
        assert_eq!(encoded["success"], true);
        assert_eq!(encoded["output"]["status"], "ok");
        assert!(encoded["error"].is_null());

        let decoded: TaskResult = serde_json::from_value(encoded).expect("deserialize result");
        assert_eq!(decoded, result);

        // Also test the failure case
        let failure = TaskResult {
            task_id: "tsk_00000002".to_string(),
            success: false,
            output: None,
            error: Some("timeout exceeded".to_string()),
        };
        let failure_rt: TaskResult =
            serde_json::from_value(serde_json::to_value(&failure).expect("serialize failure"))
                .expect("deserialize failure");
        assert_eq!(failure_rt, failure);
    }

    #[test]
    fn ownership_scope_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&OwnershipScope::Instance).expect("serialize"),
            "\"instance\""
        );
        assert_eq!(
            serde_json::to_string(&OwnershipScope::Tab).expect("serialize"),
            "\"tab\""
        );
        assert_eq!(
            serde_json::to_string(&OwnershipScope::Artifact).expect("serialize"),
            "\"artifact\""
        );
        assert_eq!(
            serde_json::to_string(&OwnershipScope::Runtime).expect("serialize"),
            "\"runtime\""
        );
    }

    #[test]
    fn token_kind_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&TokenKind::Local).expect("serialize"),
            "\"local\""
        );
        assert_eq!(
            serde_json::to_string(&TokenKind::SharedSecret).expect("serialize"),
            "\"shared_secret\""
        );
        assert_eq!(
            serde_json::to_string(&TokenKind::Certificate).expect("serialize"),
            "\"certificate\""
        );
    }

    #[test]
    fn ownership_token_round_trips() {
        let token = OwnershipToken {
            value: "tok_abc123".to_string(),
            kind: TokenKind::SharedSecret,
        };
        let encoded = serde_json::to_value(&token).expect("serialize token");
        assert_eq!(encoded["kind"], "shared_secret");
        let decoded: OwnershipToken = serde_json::from_value(encoded).expect("deserialize token");
        assert_eq!(decoded, token);
    }

    #[test]
    fn authenticated_holder_round_trips() {
        let holder = AuthenticatedHolder {
            holder_id: "agent_alpha".to_string(),
            token: OwnershipToken {
                value: "tok_abc123".to_string(),
                kind: TokenKind::Local,
            },
            scopes: vec![OwnershipScope::Instance, OwnershipScope::Tab],
            issued_at: "2026-03-12T00:00:00Z".to_string(),
            expires_at: Some("2026-03-12T01:00:00Z".to_string()),
        };
        let encoded = serde_json::to_value(&holder).expect("serialize holder");
        assert_eq!(encoded["holder_id"], "agent_alpha");
        assert_eq!(encoded["token"]["kind"], "local");
        assert_eq!(encoded["scopes"][0], "instance");
        assert_eq!(encoded["scopes"][1], "tab");
        assert_eq!(encoded["expires_at"], "2026-03-12T01:00:00Z");

        let decoded: AuthenticatedHolder =
            serde_json::from_value(encoded).expect("deserialize holder");
        assert_eq!(decoded, holder);
    }

    #[test]
    fn authenticated_holder_round_trips_without_expiry() {
        let holder = AuthenticatedHolder {
            holder_id: "agent_beta".to_string(),
            token: OwnershipToken {
                value: "cert_xyz".to_string(),
                kind: TokenKind::Certificate,
            },
            scopes: vec![OwnershipScope::Runtime],
            issued_at: "2026-03-12T00:00:00Z".to_string(),
            expires_at: None,
        };
        let encoded = serde_json::to_value(&holder).expect("serialize holder");
        assert_eq!(encoded["expires_at"], serde_json::Value::Null);

        let decoded: AuthenticatedHolder =
            serde_json::from_value(encoded).expect("deserialize holder");
        assert_eq!(decoded, holder);
    }

    #[test]
    fn ownership_denial_payload_round_trips() {
        let payload = OwnershipDenialPayload {
            operation: "lease_acquire".to_string(),
            attempted: OwnershipDenialAttempt {
                holder_id: "agent_alpha".to_string(),
                scope: OwnershipScope::Instance,
            },
            reason: "token expired".to_string(),
            recovery: vec!["re-authenticate the holder before retrying the operation".to_string()],
            retry_likely: false,
        };
        let encoded = serde_json::to_value(&payload).expect("serialize ownership denial");
        assert_eq!(encoded["operation"], "lease_acquire");
        assert_eq!(encoded["attempted"]["holder_id"], "agent_alpha");
        assert_eq!(encoded["attempted"]["scope"], "instance");
        assert_eq!(encoded["retry_likely"], false);

        let decoded: OwnershipDenialPayload =
            serde_json::from_value(encoded).expect("deserialize ownership denial");
        assert_eq!(decoded, payload);
    }
}
