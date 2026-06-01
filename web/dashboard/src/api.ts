import type {
  ArtifactDescriptor,
  AttachContinuityStatus,
  BrowserInstall,
  BrowserInstance,
  CaptureRun,
  ContinuityStatus,
  DaemonMetadata,
  ExecutionChannelAvailability,
  HealthEnvelope,
  HealthPayload,
  HostAccessProbe,
  HostAccessStatus,
  InspectionModeContract,
  LeaseCoverageEntry,
  LeaseRecord,
  ManagedProfile,
  RouteSurface,
  RuntimePaths,
  RuntimePosture,
  StatePlan,
  AssistiveOverlayDescriptor,
} from "./types";

export const HEALTH_ENDPOINT = "/health";

export async function fetchHealth(signal?: AbortSignal): Promise<HealthEnvelope> {
  const response = await fetch(HEALTH_ENDPOINT, {
    headers: {
      Accept: "application/json",
    },
    signal,
  });

  if (!response.ok) {
    throw new Error(`GET ${HEALTH_ENDPOINT} returned HTTP ${response.status}`);
  }

  const payload = (await response.json()) as unknown;
  return parseHealthEnvelope(payload);
}

function parseHealthEnvelope(value: unknown): HealthEnvelope {
  const record = expectRecord(value, "health response");
  return {
    ok: expectBoolean(record.ok, "health response.ok"),
    code: expectString(record.code, "health response.code"),
    message: expectString(record.message, "health response.message"),
    timestamp: expectString(record.timestamp, "health response.timestamp"),
    data: parseHealthPayload(record.data),
  };
}

function parseHealthPayload(value: unknown): HealthPayload {
  const record = expectRecord(value, "health response.data");
  return {
    posture: parseRuntimePosture(record.posture),
    paths: parseRuntimePaths(record.paths),
    operator_id: expectString(record.operator_id, "health response.data.operator_id"),
    daemon: parseOptionalRecord(record.daemon, parseDaemonMetadata, "health response.data.daemon"),
    continuity: parseContinuityStatus(record.continuity),
    attach_continuity: parseAttachContinuityStatus(record.attach_continuity),
    state: parseStatePlan(record.state),
    capture_run: parseCaptureRun(record.capture_run),
    inspection_modes: expectObjectArray(
      record.inspection_modes,
      "health response.data.inspection_modes",
      parseInspectionModeContract,
    ),
    routes: expectObjectArray(record.routes, "health response.data.routes", parseRouteSurface),
    lease_coverage: expectObjectArray(
      record.lease_coverage,
      "health response.data.lease_coverage",
      parseLeaseCoverageEntry,
    ),
    host_access: parseHostAccessStatus(record.host_access),
    artifacts: expectObjectArray(
      record.artifacts,
      "health response.data.artifacts",
      parseArtifactDescriptor,
    ),
    installations: expectObjectArray(
      record.installations,
      "health response.data.installations",
      parseBrowserInstall,
    ),
    profiles: expectObjectArray(
      record.profiles,
      "health response.data.profiles",
      parseManagedProfile,
    ),
    instances: expectObjectArray(
      record.instances,
      "health response.data.instances",
      parseBrowserInstance,
    ),
    leases: expectObjectArray(record.leases, "health response.data.leases", parseLeaseRecord),
  };
}

function parseRuntimePosture(value: unknown): RuntimePosture {
  const record = expectRecord(value, "health response.data.posture");
  return {
    tier_one_platform: expectString(
      record.tier_one_platform,
      "health response.data.posture.tier_one_platform",
    ),
    core_language: expectString(
      record.core_language,
      "health response.data.posture.core_language",
    ),
    mcp_mode: expectString(record.mcp_mode, "health response.data.posture.mcp_mode"),
    control_plane: expectString(
      record.control_plane,
      "health response.data.posture.control_plane",
    ),
    dashboard_status: expectString(
      record.dashboard_status,
      "health response.data.posture.dashboard_status",
    ),
  };
}

function parseRuntimePaths(value: unknown): RuntimePaths {
  const record = expectRecord(value, "health response.data.paths");
  return {
    root_dir: expectString(record.root_dir, "health response.data.paths.root_dir"),
    state_db_path: expectString(
      record.state_db_path,
      "health response.data.paths.state_db_path",
    ),
    profile_dir: expectString(record.profile_dir, "health response.data.paths.profile_dir"),
    artifact_dir: expectString(
      record.artifact_dir,
      "health response.data.paths.artifact_dir",
    ),
    replay_dir: expectString(record.replay_dir, "health response.data.paths.replay_dir"),
  };
}

function parseDaemonMetadata(value: unknown): DaemonMetadata {
  const record = expectRecord(value, "health response.data.daemon");
  return {
    bind_addr: expectString(record.bind_addr, "health response.data.daemon.bind_addr"),
    pid: expectNumber(record.pid, "health response.data.daemon.pid"),
    entrypoint: expectString(record.entrypoint, "health response.data.daemon.entrypoint"),
    started_at: expectString(record.started_at, "health response.data.daemon.started_at"),
  };
}

function parseContinuityStatus(value: unknown): ContinuityStatus {
  const record = expectRecord(value, "health response.data.continuity");
  return {
    continuity_enabled: expectBoolean(
      record.continuity_enabled,
      "health response.data.continuity.continuity_enabled",
    ),
    recovered_run: expectBoolean(
      record.recovered_run,
      "health response.data.continuity.recovered_run",
    ),
    reused_operator_id: expectBoolean(
      record.reused_operator_id,
      "health response.data.continuity.reused_operator_id",
    ),
    recovered_run_id: expectOptionalString(
      record.recovered_run_id,
      "health response.data.continuity.recovered_run_id",
    ),
    recovered_lease_count: expectNumber(
      record.recovered_lease_count,
      "health response.data.continuity.recovered_lease_count",
    ),
    recovered_instance_count: expectNumber(
      record.recovered_instance_count,
      "health response.data.continuity.recovered_instance_count",
    ),
    stale_instance_count: expectNumber(
      record.stale_instance_count,
      "health response.data.continuity.stale_instance_count",
    ),
    stale_instance_ids: expectStringArray(
      record.stale_instance_ids,
      "health response.data.continuity.stale_instance_ids",
    ),
  };
}

function parseAttachContinuityStatus(value: unknown): AttachContinuityStatus {
  const record = expectRecord(value, "health response.data.attach_continuity");
  return {
    outcome: expectOptionalString(
      record.outcome,
      "health response.data.attach_continuity.outcome",
    ),
    freshness: expectString(record.freshness, "health response.data.attach_continuity.freshness"),
    last_resolution: expectOptionalString(
      record.last_resolution,
      "health response.data.attach_continuity.last_resolution",
    ),
    last_instance_id: expectOptionalString(
      record.last_instance_id,
      "health response.data.attach_continuity.last_instance_id",
    ),
    last_debug_http_url: expectOptionalString(
      record.last_debug_http_url,
      "health response.data.attach_continuity.last_debug_http_url",
    ),
    last_requested_cdp_url: expectOptionalString(
      record.last_requested_cdp_url,
      "health response.data.attach_continuity.last_requested_cdp_url",
    ),
    last_browser_ws_url: expectOptionalString(
      record.last_browser_ws_url,
      "health response.data.attach_continuity.last_browser_ws_url",
    ),
    reused_existing_instance: expectBoolean(
      record.reused_existing_instance,
      "health response.data.attach_continuity.reused_existing_instance",
    ),
    endpoint_refreshed: expectBoolean(
      record.endpoint_refreshed,
      "health response.data.attach_continuity.endpoint_refreshed",
    ),
    updated_at: expectOptionalString(
      record.updated_at,
      "health response.data.attach_continuity.updated_at",
    ),
  };
}

function parseStatePlan(value: unknown): StatePlan {
  const record = expectRecord(value, "health response.data.state");
  return {
    primary_store: expectString(
      record.primary_store,
      "health response.data.state.primary_store",
    ),
    sqlite_status: expectString(
      record.sqlite_status,
      "health response.data.state.sqlite_status",
    ),
    write_model: expectString(record.write_model, "health response.data.state.write_model"),
    event_log: expectString(record.event_log, "health response.data.state.event_log"),
  };
}

function parseCaptureRun(value: unknown): CaptureRun {
  const record = expectRecord(value, "health response.data.capture_run");
  return {
    id: expectString(record.id, "health response.data.capture_run.id"),
    entrypoint: expectString(
      record.entrypoint,
      "health response.data.capture_run.entrypoint",
    ),
    detail: expectString(record.detail, "health response.data.capture_run.detail"),
    status: expectString(record.status, "health response.data.capture_run.status"),
    started_at: expectString(
      record.started_at,
      "health response.data.capture_run.started_at",
    ),
    ended_at: expectOptionalString(
      record.ended_at,
      "health response.data.capture_run.ended_at",
    ),
  };
}

function parseInspectionModeContract(value: unknown): InspectionModeContract {
  const record = expectRecord(value, "health response.data.inspection_modes[]");
  return {
    mode: expectString(record.mode, "health response.data.inspection_modes[].mode"),
    summary: expectString(record.summary, "health response.data.inspection_modes[].summary"),
    recommended_tools: expectStringArray(
      record.recommended_tools,
      "health response.data.inspection_modes[].recommended_tools",
    ),
    replay_mode: expectString(
      record.replay_mode,
      "health response.data.inspection_modes[].replay_mode",
    ),
  };
}

function parseRouteSurface(value: unknown): RouteSurface {
  const record = expectRecord(value, "health response.data.routes[]");
  return {
    route: expectString(record.route, "health response.data.routes[].route"),
    role: expectString(record.role, "health response.data.routes[].role"),
  };
}

function parseLeaseCoverageEntry(value: unknown): LeaseCoverageEntry {
  const record = expectRecord(value, "health response.data.lease_coverage[]");
  return {
    operation: expectString(record.operation, "health response.data.lease_coverage[].operation"),
    cli_command: expectOptionalString(
      record.cli_command,
      "health response.data.lease_coverage[].cli_command",
    ),
    mcp_tool: expectOptionalString(
      record.mcp_tool,
      "health response.data.lease_coverage[].mcp_tool",
    ),
    http_method: expectOptionalString(
      record.http_method,
      "health response.data.lease_coverage[].http_method",
    ),
    http_route: expectOptionalString(
      record.http_route,
      "health response.data.lease_coverage[].http_route",
    ),
    disposition: expectString(
      record.disposition,
      "health response.data.lease_coverage[].disposition",
    ),
    rationale: expectString(
      record.rationale,
      "health response.data.lease_coverage[].rationale",
    ),
  };
}

function parseHostAccessStatus(value: unknown): HostAccessStatus {
  const record = expectRecord(value, "health response.data.host_access");
  return {
    platform: expectString(record.platform, "health response.data.host_access.platform"),
    app_targets: expectStringArray(
      record.app_targets,
      "health response.data.host_access.app_targets",
    ),
    services: expectObjectArray(
      record.services,
      "health response.data.host_access.services",
      parseHostAccessProbe,
    ),
    execution_channels: expectObjectArray(
      record.execution_channels,
      "health response.data.host_access.execution_channels",
      parseExecutionChannelAvailability,
    ),
    assistive_overlays: expectObjectArray(
      record.assistive_overlays,
      "health response.data.host_access.assistive_overlays",
      parseAssistiveOverlayDescriptor,
    ),
    recommended_services: expectStringArray(
      record.recommended_services,
      "health response.data.host_access.recommended_services",
    ),
    summary: expectString(record.summary, "health response.data.host_access.summary"),
  };
}

function parseHostAccessProbe(value: unknown): HostAccessProbe {
  const record = expectRecord(value, "health response.data.host_access.services[]");
  return {
    service: expectString(record.service, "health response.data.host_access.services[].service"),
    state: expectString(record.state, "health response.data.host_access.services[].state"),
    requestable: expectBoolean(
      record.requestable,
      "health response.data.host_access.services[].requestable",
    ),
    open_settings_url: expectOptionalString(
      record.open_settings_url,
      "health response.data.host_access.services[].open_settings_url",
    ),
    detail: expectString(record.detail, "health response.data.host_access.services[].detail"),
  };
}

function parseExecutionChannelAvailability(value: unknown): ExecutionChannelAvailability {
  const record = expectRecord(value, "health response.data.host_access.execution_channels[]");
  return {
    channel: expectString(
      record.channel,
      "health response.data.host_access.execution_channels[].channel",
    ),
    available: expectBoolean(
      record.available,
      "health response.data.host_access.execution_channels[].available",
    ),
    interference_level: expectString(
      record.interference_level,
      "health response.data.host_access.execution_channels[].interference_level",
    ),
    detail: expectString(
      record.detail,
      "health response.data.host_access.execution_channels[].detail",
    ),
  };
}

function parseAssistiveOverlayDescriptor(value: unknown): AssistiveOverlayDescriptor {
  const record = expectRecord(value, "health response.data.host_access.assistive_overlays[]");
  return {
    id: expectString(record.id, "health response.data.host_access.assistive_overlays[].id"),
    title: expectString(
      record.title,
      "health response.data.host_access.assistive_overlays[].title",
    ),
    summary: expectString(
      record.summary,
      "health response.data.host_access.assistive_overlays[].summary",
    ),
    primary_use: expectString(
      record.primary_use,
      "health response.data.host_access.assistive_overlays[].primary_use",
    ),
  };
}

function parseArtifactDescriptor(value: unknown): ArtifactDescriptor {
  const record = expectRecord(value, "health response.data.artifacts[]");
  return {
    kind: expectString(record.kind, "health response.data.artifacts[].kind"),
    streaming_policy: expectString(
      record.streaming_policy,
      "health response.data.artifacts[].streaming_policy",
    ),
    storage_path_hint: expectString(
      record.storage_path_hint,
      "health response.data.artifacts[].storage_path_hint",
    ),
  };
}

function parseBrowserInstall(value: unknown): BrowserInstall {
  const record = expectRecord(value, "health response.data.installations[]");
  return {
    channel: expectString(record.channel, "health response.data.installations[].channel"),
    installed: expectBoolean(record.installed, "health response.data.installations[].installed"),
    app_path: expectString(record.app_path, "health response.data.installations[].app_path"),
    binary_path: expectString(
      record.binary_path,
      "health response.data.installations[].binary_path",
    ),
  };
}

function parseManagedProfile(value: unknown): ManagedProfile {
  const record = expectRecord(value, "health response.data.profiles[]");
  return {
    id: expectString(record.id, "health response.data.profiles[].id"),
    name: expectString(record.name, "health response.data.profiles[].name"),
    channel: expectString(record.channel, "health response.data.profiles[].channel"),
    path: expectString(record.path, "health response.data.profiles[].path"),
  };
}

function parseBrowserInstance(value: unknown): BrowserInstance {
  const record = expectRecord(value, "health response.data.instances[]");
  return {
    id: expectString(record.id, "health response.data.instances[].id"),
    name: expectString(record.name, "health response.data.instances[].name"),
    channel: expectString(record.channel, "health response.data.instances[].channel"),
    mode: expectString(record.mode, "health response.data.instances[].mode"),
    status: expectString(record.status, "health response.data.instances[].status"),
    debug_http_url: expectString(
      record.debug_http_url,
      "health response.data.instances[].debug_http_url",
    ),
    browser_ws_url: expectOptionalString(
      record.browser_ws_url,
      "health response.data.instances[].browser_ws_url",
    ),
    profile_id: expectOptionalString(
      record.profile_id,
      "health response.data.instances[].profile_id",
    ),
    profile_path: expectOptionalString(
      record.profile_path,
      "health response.data.instances[].profile_path",
    ),
    pid: expectOptionalNumber(record.pid, "health response.data.instances[].pid"),
    last_error: expectOptionalString(
      record.last_error,
      "health response.data.instances[].last_error",
    ),
    created_at: expectString(record.created_at, "health response.data.instances[].created_at"),
    updated_at: expectString(record.updated_at, "health response.data.instances[].updated_at"),
  };
}

function parseLeaseRecord(value: unknown): LeaseRecord {
  const record = expectRecord(value, "health response.data.leases[]");
  return {
    id: expectString(record.id, "health response.data.leases[].id"),
    resource_kind: expectString(
      record.resource_kind,
      "health response.data.leases[].resource_kind",
    ),
    resource_id: expectString(record.resource_id, "health response.data.leases[].resource_id"),
    holder_id: expectString(record.holder_id, "health response.data.leases[].holder_id"),
    holder_label: expectOptionalString(
      record.holder_label,
      "health response.data.leases[].holder_label",
    ),
    mode: expectString(record.mode, "health response.data.leases[].mode"),
    granted_at: expectString(record.granted_at, "health response.data.leases[].granted_at"),
    expires_at: expectString(record.expires_at, "health response.data.leases[].expires_at"),
    last_heartbeat_at: expectString(
      record.last_heartbeat_at,
      "health response.data.leases[].last_heartbeat_at",
    ),
  };
}

function expectObjectArray<T>(
  value: unknown,
  label: string,
  parser: (entry: unknown) => T,
): T[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid ${label}: expected an array`);
  }

  return value.map((entry) => parser(entry));
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`Invalid ${label}: expected an object`);
  }

  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new Error(`Invalid ${label}: expected a string`);
  }

  return value;
}

function expectOptionalString(value: unknown, label: string): string | null {
  if (value === null || value === undefined) {
    return null;
  }

  return expectString(value, label);
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== "number" || Number.isNaN(value)) {
    throw new Error(`Invalid ${label}: expected a number`);
  }

  return value;
}

function expectOptionalNumber(value: unknown, label: string): number | null {
  if (value === null || value === undefined) {
    return null;
  }

  return expectNumber(value, label);
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`Invalid ${label}: expected a boolean`);
  }

  return value;
}

function expectStringArray(value: unknown, label: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid ${label}: expected an array`);
  }

  return value.map((entry, index) => expectString(entry, `${label}[${index}]`));
}

function parseOptionalRecord<T>(
  value: unknown,
  parser: (entry: unknown) => T,
  _label: string,
): T | null {
  if (value === null || value === undefined) {
    return null;
  }

  return parser(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
