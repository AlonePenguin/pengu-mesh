import { startTransition, useEffect, useState, type ReactNode } from "react";
import { HEALTH_ENDPOINT, fetchHealth } from "./api";
import type {
  BrowserInstance,
  ExecutionChannelAvailability,
  HealthEnvelope,
  HostAccessProbe,
  InspectionModeContract,
  LeaseCoverageEntry,
  LeaseRecord,
} from "./types";

const REFRESH_INTERVAL_MS = 15_000;

export default function App() {
  const [health, setHealth] = useState<HealthEnvelope | null>(null);
  const [requestError, setRequestError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [lastAttemptAt, setLastAttemptAt] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timerId: number | undefined;
    let activeController: AbortController | null = null;

    const scheduleNextPoll = () => {
      timerId = window.setTimeout(() => {
        void refresh("poll");
      }, REFRESH_INTERVAL_MS);
    };

    const refresh = async (mode: "initial" | "manual" | "poll") => {
      activeController?.abort();
      const controller = new AbortController();
      activeController = controller;

      if (mode === "initial") {
        setIsLoading(true);
      } else {
        setIsRefreshing(true);
      }

      setLastAttemptAt(new Date().toISOString());

      try {
        const next = await fetchHealth(controller.signal);
        if (cancelled || controller.signal.aborted) {
          return;
        }

        startTransition(() => {
          setHealth(next);
          setRequestError(null);
        });
      } catch (error) {
        if (cancelled || controller.signal.aborted) {
          return;
        }

        setRequestError(error instanceof Error ? error.message : String(error));
      } finally {
        if (cancelled || controller.signal.aborted) {
          return;
        }

        setIsLoading(false);
        setIsRefreshing(false);

        if (mode !== "manual") {
          scheduleNextPoll();
        }
      }
    };

    void refresh("initial");

    return () => {
      cancelled = true;
      activeController?.abort();
      if (timerId !== undefined) {
        window.clearTimeout(timerId);
      }
    };
  }, []);

  const handleManualRefresh = async () => {
    setIsRefreshing(true);
    setLastAttemptAt(new Date().toISOString());

    try {
      const next = await fetchHealth();
      startTransition(() => {
        setHealth(next);
        setRequestError(null);
      });
    } catch (error) {
      setRequestError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoading(false);
      setIsRefreshing(false);
    }
  };

  if (isLoading && !health) {
    return (
      <div className="app-shell">
        <main className="center-stage">
          <div className="loading-card">
            <p className="eyebrow">Operator Console</p>
            <h1>Connecting to {HEALTH_ENDPOINT}</h1>
            <p className="body-copy">
              Waiting for the local control plane to return a typed health envelope.
            </p>
          </div>
        </main>
      </div>
    );
  }

  if (!health) {
    return (
      <div className="app-shell">
        <main className="center-stage">
          <div className="loading-card error-shell">
            <p className="eyebrow">Operator Console</p>
            <h1>Health endpoint unavailable</h1>
            <p className="body-copy">{requestError ?? `Could not reach ${HEALTH_ENDPOINT}.`}</p>
            <button className="action-button" type="button" onClick={() => void handleManualRefresh()}>
              Retry health request
            </button>
          </div>
        </main>
      </div>
    );
  }

  const payload = health.data;
  const installedBrowsers = payload.installations.filter((item) => item.installed);
  const unavailableServices = payload.host_access.services.filter(
    (item) => item.state !== "granted",
  );
  const availableChannels = payload.host_access.execution_channels.filter(
    (item) => item.available,
  );
  const liveInstances = payload.instances.filter((item) => isLiveInstanceStatus(item.status));
  const writerRequiredCount = payload.lease_coverage.filter(
    (item) => item.disposition === "writer_required",
  ).length;
  const observerRequiredCount = payload.lease_coverage.filter(
    (item) => item.disposition === "observer_required",
  ).length;

  return (
    <div className="app-shell">
      <main className="console">
        <header className="hero">
          <div className="hero-copy">
            <p className="eyebrow">Read-only Operator Console</p>
            <h1>pengu mesh health</h1>
            <p className="body-copy hero-copy-width">
              A Vite and React console scaffold over the real <code>{HEALTH_ENDPOINT}</code> contract.
              Runtime truth still belongs to the CLI, daemon, and HTTP control plane.
            </p>
          </div>
          <div className="hero-actions">
            <button
              className="action-button"
              type="button"
              onClick={() => void handleManualRefresh()}
              disabled={isRefreshing}
            >
              {isRefreshing ? "Refreshing..." : "Refresh"}
            </button>
            <div className="meta-cluster">
              <MetaLabel label="Auto refresh" value={`every ${REFRESH_INTERVAL_MS / 1000}s`} />
              <MetaLabel
                label="Last success"
                value={formatTimestamp(health.timestamp) ?? "Not yet recorded"}
              />
              <MetaLabel
                label="Last attempt"
                value={formatTimestamp(lastAttemptAt) ?? "Unavailable"}
              />
            </div>
          </div>
        </header>

        <section className="summary-grid">
          <MetricCard
            label="Runtime state"
            value={health.ok ? "ready" : humanizeToken(health.code)}
            badge={health.ok ? "ok" : humanizeToken(health.code)}
            tone={health.ok ? "success" : "warning"}
            detail={health.message}
          />
          <MetricCard
            label="Operator"
            value={payload.operator_id}
            badge={payload.daemon ? "daemon" : "local"}
            tone="neutral"
            detail={payload.daemon?.bind_addr ?? "standalone runtime without daemon metadata"}
          />
          <MetricCard
            label="Browser installs"
            value={`${installedBrowsers.length}/${payload.installations.length}`}
            badge={installedBrowsers.length > 0 ? "ready" : "blocked"}
            tone={installedBrowsers.length > 0 ? "success" : "warning"}
            detail={
              installedBrowsers.length > 0
                ? "supported channel discovered"
                : "no supported browser found"
            }
          />
          <MetricCard
            label="Host access"
            value={`${payload.host_access.services.length - unavailableServices.length}/${payload.host_access.services.length}`}
            badge={unavailableServices.length === 0 ? "granted" : "needs review"}
            tone={unavailableServices.length === 0 ? "success" : "warning"}
            detail={payload.host_access.summary}
          />
          <MetricCard
            label="Instances"
            value={`${liveInstances.length}/${payload.instances.length}`}
            badge={liveInstances.length > 0 ? "live" : "idle"}
            tone={liveInstances.length > 0 ? "success" : "neutral"}
            detail="running or attached over total known instances"
          />
          <MetricCard
            label="Lease model"
            value={`${writerRequiredCount} write / ${observerRequiredCount} read`}
            badge="catalog"
            tone="neutral"
            detail={`${payload.lease_coverage.length} operations in the surfaced matrix`}
          />
        </section>

        {requestError && (
          <section className="banner banner-warning">
            <div>
              <p className="banner-title">Showing the last successful health snapshot</p>
              <p className="body-copy">{requestError}</p>
            </div>
            <StatusPill tone="warning">stale</StatusPill>
          </section>
        )}

        <section className="dashboard-grid">
          <Panel title="Runtime posture" subtitle="Product identity and control-plane stance">
            <DefinitionGrid
              items={[
                ["Tier 1 platform", payload.posture.tier_one_platform],
                ["Core language", payload.posture.core_language],
                ["MCP mode", payload.posture.mcp_mode],
                ["Control plane", payload.posture.control_plane],
                ["Dashboard posture", payload.posture.dashboard_status],
              ]}
            />
          </Panel>

          <Panel title="State and capture" subtitle="Persistence and the current recording context">
            <DefinitionGrid
              items={[
                ["Primary store", payload.state.primary_store],
                ["SQLite status", payload.state.sqlite_status],
                ["Write model", payload.state.write_model],
                ["Event log", payload.state.event_log],
                ["Capture run", payload.capture_run.id],
                ["Capture status", payload.capture_run.status],
                ["Entrypoint", payload.capture_run.entrypoint],
                ["Started", formatTimestamp(payload.capture_run.started_at) ?? payload.capture_run.started_at],
              ]}
            />
            <p className="panel-detail">{payload.capture_run.detail}</p>
          </Panel>

          <Panel title="Continuity" subtitle="Daemon and attach continuity state">
            <div className="split-column">
              <Subsection title="Runtime continuity">
                <DefinitionGrid
                  items={[
                    ["Continuity enabled", formatBoolean(payload.continuity.continuity_enabled)],
                    ["Recovered run", formatBoolean(payload.continuity.recovered_run)],
                    ["Reused operator id", formatBoolean(payload.continuity.reused_operator_id)],
                    ["Recovered leases", String(payload.continuity.recovered_lease_count)],
                    ["Recovered instances", String(payload.continuity.recovered_instance_count)],
                    ["Stale instances", String(payload.continuity.stale_instance_count)],
                  ]}
                />
                {payload.continuity.recovered_run_id && (
                  <p className="panel-detail">Recovered run id: {payload.continuity.recovered_run_id}</p>
                )}
                {payload.continuity.stale_instance_ids.length > 0 && (
                  <TagList values={payload.continuity.stale_instance_ids} tone="warning" />
                )}
              </Subsection>

              <Subsection title="Attach continuity">
                <DefinitionGrid
                  items={[
                    [
                      "Outcome",
                      payload.attach_continuity.outcome
                        ? humanizeToken(payload.attach_continuity.outcome)
                        : "none",
                    ],
                    ["Freshness", humanizeToken(payload.attach_continuity.freshness)],
                    [
                      "Last resolution",
                      payload.attach_continuity.last_resolution
                        ? humanizeToken(payload.attach_continuity.last_resolution)
                        : "none",
                    ],
                    [
                      "Reused existing instance",
                      formatBoolean(payload.attach_continuity.reused_existing_instance),
                    ],
                    [
                      "Endpoint refreshed",
                      formatBoolean(payload.attach_continuity.endpoint_refreshed),
                    ],
                    [
                      "Updated",
                      formatTimestamp(payload.attach_continuity.updated_at) ?? "not recorded",
                    ],
                  ]}
                />
                <InlineMetaList
                  items={[
                    ["Instance", payload.attach_continuity.last_instance_id],
                    ["Debug URL", payload.attach_continuity.last_debug_http_url],
                    ["Requested CDP URL", payload.attach_continuity.last_requested_cdp_url],
                    ["Browser WS URL", payload.attach_continuity.last_browser_ws_url],
                  ]}
                />
              </Subsection>
            </div>
          </Panel>

          <Panel title="Host access" subtitle="Machine-level permissions and available execution channels">
            <div className="stack-group">
              <DefinitionGrid
                items={[
                  ["Platform", payload.host_access.platform],
                  ["Targets", payload.host_access.app_targets.join(", ")],
                  [
                    "Recommended services",
                    payload.host_access.recommended_services.join(", ") || "none",
                  ],
                  [
                    "Execution channels",
                    `${availableChannels.length}/${payload.host_access.execution_channels.length} available`,
                  ],
                ]}
              />

              <Subsection title="Permission services">
                <div className="surface-list">
                  {payload.host_access.services.map((service) => (
                    <HostAccessCard key={service.service} service={service} />
                  ))}
                </div>
              </Subsection>

              <Subsection title="Execution channels">
                <div className="surface-list">
                  {payload.host_access.execution_channels.map((channel) => (
                    <ExecutionChannelCard key={channel.channel} channel={channel} />
                  ))}
                </div>
              </Subsection>

              <Subsection title="Assistive overlays">
                <div className="overlay-grid">
                  {payload.host_access.assistive_overlays.map((overlay) => (
                    <article key={overlay.id} className="surface-card">
                      <p className="surface-title">{overlay.title}</p>
                      <p className="body-copy">{overlay.summary}</p>
                      <p className="panel-detail">{overlay.primary_use}</p>
                    </article>
                  ))}
                </div>
              </Subsection>
            </div>
          </Panel>

          <Panel title="Runtime inventory" subtitle="Browsers, profiles, instances, and live leases">
            <div className="stack-group">
              <Subsection title="Installations">
                <div className="surface-list">
                  {payload.installations.map((install) => (
                    <article key={install.channel} className="surface-card">
                      <div className="card-header">
                        <p className="surface-title">{humanizeToken(install.channel)}</p>
                        <StatusPill tone={install.installed ? "success" : "warning"}>
                          {install.installed ? "installed" : "missing"}
                        </StatusPill>
                      </div>
                      <p className="body-copy">{install.app_path}</p>
                      <p className="panel-detail">{install.binary_path}</p>
                    </article>
                  ))}
                </div>
              </Subsection>

              <Subsection title="Managed profiles">
                {payload.profiles.length === 0 ? (
                  <EmptyState message="No managed profiles are currently registered." />
                ) : (
                  <div className="surface-list">
                    {payload.profiles.map((profile) => (
                      <article key={profile.id} className="surface-card">
                        <div className="card-header">
                          <p className="surface-title">{profile.name}</p>
                          <StatusPill tone="neutral">{humanizeToken(profile.channel)}</StatusPill>
                        </div>
                        <p className="panel-detail">{profile.id}</p>
                        <p className="body-copy">{profile.path}</p>
                      </article>
                    ))}
                  </div>
                )}
              </Subsection>

              <Subsection title="Instances">
                <InstanceTable instances={payload.instances} />
              </Subsection>

              <Subsection title="Leases">
                <LeaseTable leases={payload.leases} />
              </Subsection>
            </div>
          </Panel>

          <Panel title="Inspection and evidence" subtitle="Read paths, artifact policies, and replay posture">
            <div className="stack-group">
              <Subsection title="Inspection modes">
                <div className="surface-list">
                  {payload.inspection_modes.map((mode) => (
                    <InspectionModeCard key={mode.mode} mode={mode} />
                  ))}
                </div>
              </Subsection>

              <Subsection title="Artifact policies">
                <div className="surface-list">
                  {payload.artifacts.map((artifact) => (
                    <article key={artifact.kind} className="surface-card">
                      <div className="card-header">
                        <p className="surface-title">{humanizeToken(artifact.kind)}</p>
                        <StatusPill tone="neutral">
                          {humanizeToken(artifact.streaming_policy)}
                        </StatusPill>
                      </div>
                      <p className="panel-detail">{artifact.storage_path_hint}</p>
                    </article>
                  ))}
                </div>
              </Subsection>

              <Subsection title="Runtime paths">
                <DefinitionGrid
                  items={[
                    ["Root dir", payload.paths.root_dir],
                    ["State DB", payload.paths.state_db_path],
                    ["Profiles", payload.paths.profile_dir],
                    ["Artifacts", payload.paths.artifact_dir],
                    ["Replay", payload.paths.replay_dir],
                  ]}
                />
              </Subsection>
            </div>
          </Panel>

          <Panel title="Surface catalog" subtitle="HTTP routes and lease coverage from the live health payload">
            <div className="stack-group">
              <Subsection title="HTTP routes">
                <div className="route-grid">
                  {payload.routes.map((route) => (
                    <article key={route.route} className="route-card">
                      <p className="surface-title">{route.route}</p>
                      <p className="panel-detail">{route.role}</p>
                    </article>
                  ))}
                </div>
              </Subsection>

              <Subsection title="Lease coverage matrix">
                <CoverageTable coverage={payload.lease_coverage} />
              </Subsection>
            </div>
          </Panel>
        </section>
      </main>
    </div>
  );
}

function HostAccessCard({ service }: { service: HostAccessProbe }) {
  return (
    <article className="surface-card">
      <div className="card-header">
        <p className="surface-title">{humanizeToken(service.service)}</p>
        <StatusPill tone={toneForPermissionState(service.state)}>
          {humanizeToken(service.state)}
        </StatusPill>
      </div>
      <p className="body-copy">{service.detail}</p>
      <p className="panel-detail">{service.requestable ? "requestable" : "read-only probe"}</p>
      {service.open_settings_url && (
        <a className="inline-link" href={service.open_settings_url}>
          Open macOS settings target
        </a>
      )}
    </article>
  );
}

function ExecutionChannelCard({ channel }: { channel: ExecutionChannelAvailability }) {
  return (
    <article className="surface-card">
      <div className="card-header">
        <p className="surface-title">{humanizeToken(channel.channel)}</p>
        <StatusPill tone={channel.available ? "success" : "warning"}>
          {channel.available ? "available" : "blocked"}
        </StatusPill>
      </div>
      <p className="panel-detail">Interference: {humanizeToken(channel.interference_level)}</p>
      <p className="body-copy">{channel.detail}</p>
    </article>
  );
}

function InspectionModeCard({ mode }: { mode: InspectionModeContract }) {
  return (
    <article className="surface-card">
      <div className="card-header">
        <p className="surface-title">{humanizeToken(mode.mode)}</p>
        <StatusPill tone="neutral">{humanizeToken(mode.replay_mode)}</StatusPill>
      </div>
      <p className="body-copy">{mode.summary}</p>
      <TagList values={mode.recommended_tools} tone="neutral" />
    </article>
  );
}

function InstanceTable({ instances }: { instances: BrowserInstance[] }) {
  if (instances.length === 0) {
    return <EmptyState message="No browser instances are currently registered." />;
  }

  return (
    <div className="table-shell">
      <table className="data-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Status</th>
            <th>Channel</th>
            <th>Mode</th>
            <th>PID</th>
            <th>Updated</th>
            <th>Debug endpoint</th>
          </tr>
        </thead>
        <tbody>
          {instances.map((instance) => (
            <tr key={instance.id}>
              <td>
                <div className="table-primary">
                  <span>{instance.name}</span>
                  <span className="panel-detail">{instance.id}</span>
                </div>
                {instance.last_error && <p className="table-note">{instance.last_error}</p>}
              </td>
              <td>
                <StatusPill tone={toneForInstanceStatus(instance.status)}>
                  {humanizeToken(instance.status)}
                </StatusPill>
              </td>
              <td>{humanizeToken(instance.channel)}</td>
              <td>{humanizeToken(instance.mode)}</td>
              <td>{instance.pid ?? "n/a"}</td>
              <td>{formatTimestamp(instance.updated_at) ?? instance.updated_at}</td>
              <td className="table-mono">{instance.debug_http_url}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function LeaseTable({ leases }: { leases: LeaseRecord[] }) {
  if (leases.length === 0) {
    return <EmptyState message="No active leases are currently held." />;
  }

  return (
    <div className="table-shell">
      <table className="data-table">
        <thead>
          <tr>
            <th>Resource</th>
            <th>Mode</th>
            <th>Holder</th>
            <th>Granted</th>
            <th>Expires</th>
            <th>Heartbeat</th>
          </tr>
        </thead>
        <tbody>
          {leases.map((lease) => (
            <tr key={lease.id}>
              <td>
                <div className="table-primary">
                  <span>{lease.resource_id}</span>
                  <span className="panel-detail">{humanizeToken(lease.resource_kind)}</span>
                </div>
              </td>
              <td>{humanizeToken(lease.mode)}</td>
              <td>
                <div className="table-primary">
                  <span>{lease.holder_label ?? lease.holder_id}</span>
                  {lease.holder_label && <span className="panel-detail">{lease.holder_id}</span>}
                </div>
              </td>
              <td>{formatTimestamp(lease.granted_at) ?? lease.granted_at}</td>
              <td>{formatTimestamp(lease.expires_at) ?? lease.expires_at}</td>
              <td>{formatTimestamp(lease.last_heartbeat_at) ?? lease.last_heartbeat_at}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function CoverageTable({ coverage }: { coverage: LeaseCoverageEntry[] }) {
  return (
    <div className="table-shell">
      <table className="data-table">
        <thead>
          <tr>
            <th>Operation</th>
            <th>Lease disposition</th>
            <th>HTTP route</th>
            <th>CLI</th>
            <th>Rationale</th>
          </tr>
        </thead>
        <tbody>
          {coverage.map((item) => (
            <tr key={item.operation}>
              <td>{item.operation}</td>
              <td>
                <StatusPill tone={toneForDisposition(item.disposition)}>
                  {humanizeToken(item.disposition)}
                </StatusPill>
              </td>
              <td>
                {item.http_method && item.http_route
                  ? `${item.http_method} ${item.http_route}`
                  : "n/a"}
              </td>
              <td className="table-mono">{item.cli_command ?? item.mcp_tool ?? "n/a"}</td>
              <td>{item.rationale}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Panel({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: ReactNode;
}) {
  return (
    <section className="panel">
      <div className="panel-header">
        <h2>{title}</h2>
        <p className="body-copy">{subtitle}</p>
      </div>
      {children}
    </section>
  );
}

function Subsection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="subsection">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function MetricCard({
  label,
  value,
  badge,
  detail,
  tone,
}: {
  label: string;
  value: string;
  badge: string;
  detail: string;
  tone: Tone;
}) {
  return (
    <article className="metric-card">
      <p className="metric-label">{label}</p>
      <div className="metric-row">
        <strong>{value}</strong>
        <StatusPill tone={tone}>{badge}</StatusPill>
      </div>
      <p className="panel-detail">{detail}</p>
    </article>
  );
}

function MetaLabel({ label, value }: { label: string; value: string }) {
  return (
    <div className="meta-label">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function DefinitionGrid({ items }: { items: [string, string][] }) {
  return (
    <dl className="definition-grid">
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function InlineMetaList({ items }: { items: [string, string | null][] }) {
  const populated = items.filter(([, value]) => value);
  if (populated.length === 0) {
    return null;
  }

  return (
    <div className="inline-meta-list">
      {populated.map(([label, value]) => (
        <div key={label}>
          <span>{label}</span>
          <code>{value}</code>
        </div>
      ))}
    </div>
  );
}

function TagList({ values, tone }: { values: string[]; tone: Tone }) {
  return (
    <div className="tag-list">
      {values.map((value) => (
        <StatusPill key={value} tone={tone}>
          {value}
        </StatusPill>
      ))}
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return <p className="empty-state">{message}</p>;
}

function StatusPill({
  tone,
  children,
}: {
  tone: Tone;
  children: ReactNode;
}) {
  return <span className={`status-pill status-pill-${tone}`}>{children}</span>;
}

type Tone = "success" | "warning" | "critical" | "neutral";

function toneForPermissionState(state: string): Tone {
  switch (state) {
    case "granted":
      return "success";
    case "missing":
      return "critical";
    case "unknown":
      return "warning";
    default:
      return "neutral";
  }
}

function toneForDisposition(disposition: string): Tone {
  switch (disposition) {
    case "writer_required":
      return "critical";
    case "observer_required":
      return "warning";
    default:
      return "neutral";
  }
}

function isLiveInstanceStatus(status: BrowserInstance["status"]): boolean {
  return status === "running" || status === "attached";
}

function toneForInstanceStatus(status: BrowserInstance["status"]): Tone {
  if (status === "running" || status === "attached") {
    return "success";
  }
  if (status === "starting") {
    return "neutral";
  }
  return "warning";
}

function formatBoolean(value: boolean): string {
  return value ? "yes" : "no";
}

function humanizeToken(value: string): string {
  return value
    .split(/[_-]/g)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatTimestamp(value: string | null): string | null {
  if (!value) {
    return null;
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
}
