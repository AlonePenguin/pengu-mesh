export interface OperationOutcome<T> {
  ok: boolean;
  code: string;
  message: string;
  timestamp: string;
  data: T;
}

export interface RuntimePosture {
  tier_one_platform: string;
  core_language: string;
  mcp_mode: string;
  control_plane: string;
  dashboard_status: string;
}

export interface RuntimePaths {
  root_dir: string;
  state_db_path: string;
  profile_dir: string;
  artifact_dir: string;
  replay_dir: string;
}

export interface DaemonMetadata {
  bind_addr: string;
  pid: number;
  entrypoint: string;
  started_at: string;
}

export interface ContinuityStatus {
  continuity_enabled: boolean;
  recovered_run: boolean;
  reused_operator_id: boolean;
  recovered_run_id: string | null;
  recovered_lease_count: number;
  recovered_instance_count: number;
  stale_instance_count: number;
  stale_instance_ids: string[];
}

export interface AttachContinuityStatus {
  outcome: string | null;
  freshness: string;
  last_resolution: string | null;
  last_instance_id: string | null;
  last_debug_http_url: string | null;
  last_requested_cdp_url: string | null;
  last_browser_ws_url: string | null;
  reused_existing_instance: boolean;
  endpoint_refreshed: boolean;
  updated_at: string | null;
}

export interface StatePlan {
  primary_store: string;
  sqlite_status: string;
  write_model: string;
  event_log: string;
}

export interface CaptureRun {
  id: string;
  entrypoint: string;
  detail: string;
  status: string;
  started_at: string;
  ended_at: string | null;
}

export interface InspectionModeContract {
  mode: string;
  summary: string;
  recommended_tools: string[];
  replay_mode: string;
}

export interface CapabilityGatePolicy {
  allow_safe: boolean;
  allow_elevated: boolean;
  allow_dangerous: boolean;
  explicit_grants: string[];
}

export interface CapabilityDecision {
  decision: "allowed" | "denied" | "requires_grant";
  reason?: string;
  capability?: string;
}

export interface CapabilityPostureEntry {
  name: string;
  risk_tier: string;
  description: string;
  requires_explicit_grant: boolean;
  decision: CapabilityDecision;
}

export interface CapabilityPosture {
  policy: CapabilityGatePolicy;
  total: number;
  safe: number;
  elevated: number;
  dangerous: number;
  allowed: number;
  denied: number;
  requires_grant: number;
  capabilities: CapabilityPostureEntry[];
}

export interface RouteSurface {
  route: string;
  role: string;
}

export interface LeaseCoverageEntry {
  operation: string;
  cli_command: string | null;
  mcp_tool: string | null;
  http_method: string | null;
  http_route: string | null;
  disposition: string;
  rationale: string;
}

export interface HostAccessProbe {
  service: string;
  state: string;
  requestable: boolean;
  open_settings_url: string | null;
  detail: string;
}

export interface ExecutionChannelAvailability {
  channel: string;
  available: boolean;
  interference_level: string;
  detail: string;
}

export interface AssistiveOverlayDescriptor {
  id: string;
  title: string;
  summary: string;
  primary_use: string;
}

export interface HostAccessStatus {
  platform: string;
  app_targets: string[];
  services: HostAccessProbe[];
  execution_channels: ExecutionChannelAvailability[];
  assistive_overlays: AssistiveOverlayDescriptor[];
  recommended_services: string[];
  summary: string;
}

export interface ArtifactDescriptor {
  kind: string;
  streaming_policy: string;
  storage_path_hint: string;
}

export interface BrowserInstall {
  channel: string;
  installed: boolean;
  app_path: string;
  binary_path: string;
}

export interface ManagedProfile {
  id: string;
  name: string;
  channel: string;
  path: string;
}

export interface BrowserInstance {
  id: string;
  name: string;
  channel: string;
  mode: string;
  status: string;
  debug_http_url: string;
  browser_ws_url: string | null;
  profile_id: string | null;
  profile_path: string | null;
  pid: number | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface LeaseRecord {
  id: string;
  resource_kind: string;
  resource_id: string;
  holder_id: string;
  holder_label: string | null;
  mode: string;
  granted_at: string;
  expires_at: string;
  last_heartbeat_at: string;
}

export interface HealthPayload {
  posture: RuntimePosture;
  paths: RuntimePaths;
  operator_id: string;
  daemon: DaemonMetadata | null;
  continuity: ContinuityStatus;
  attach_continuity: AttachContinuityStatus;
  state: StatePlan;
  capture_run: CaptureRun;
  inspection_modes: InspectionModeContract[];
  capability_posture: CapabilityPosture;
  routes: RouteSurface[];
  lease_coverage: LeaseCoverageEntry[];
  host_access: HostAccessStatus;
  artifacts: ArtifactDescriptor[];
  installations: BrowserInstall[];
  profiles: ManagedProfile[];
  instances: BrowserInstance[];
  leases: LeaseRecord[];
}

export type HealthEnvelope = OperationOutcome<HealthPayload>;
