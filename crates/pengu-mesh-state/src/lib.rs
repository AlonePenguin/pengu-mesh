use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, types::Type};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use time::OffsetDateTime;

use pengu_mesh_shared::{
    ArtifactHandle, ArtifactProvenance, AttachContinuityStatus, BrowserInstance, BrowserTab,
    CaptureRun, EnvironmentFingerprint, EventLevel, IdKind, LatencySample, LeaseMode, LeaseRecord,
    LeaseResourceKind, ManagedProfile, NormalizedRegion, RunStatus, RuntimeEvent, RuntimePaths,
    ScenarioAssertion, ScenarioRun, ScenarioStep, StableId, TaskPriority, TaskRecord, TaskState,
    utc_timestamp,
};

#[derive(Debug, Clone, Serialize)]
pub struct StatePlan {
    pub primary_store: &'static str,
    pub sqlite_status: &'static str,
    pub write_model: &'static str,
    pub event_log: &'static str,
}

impl Default for StatePlan {
    fn default() -> Self {
        Self {
            primary_store: "sqlite",
            sqlite_status: "active",
            write_model: "single-process-connection-per-operation",
            event_log: "sqlite-append-only-events",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateStore {
    paths: RuntimePaths,
    read_only: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeIdentity {
    pub entrypoint: String,
    pub operator_id: String,
    pub created_at: String,
    pub updated_at: String,
}

pub const METRIC_TASKS_SUBMITTED: &str = "tasks_submitted";
pub const METRIC_TASKS_COMPLETED: &str = "tasks_completed";
pub const METRIC_TASKS_FAILED: &str = "tasks_failed";
pub const METRIC_TASKS_CANCELLED: &str = "tasks_cancelled";

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerMetricRow {
    pub id: i64,
    pub metric_name: String,
    pub agent_id: Option<String>,
    pub counter: i64,
    pub last_updated: String,
}

impl StateStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let artifact_dir = root.join("artifacts");
        let profile_dir = root.join("profiles");
        let replay_dir = root.join("replay");
        fs::create_dir_all(&artifact_dir)
            .with_context(|| format!("create artifact dir {}", artifact_dir.display()))?;
        fs::create_dir_all(&profile_dir)
            .with_context(|| format!("create profile dir {}", profile_dir.display()))?;
        fs::create_dir_all(&replay_dir)
            .with_context(|| format!("create replay dir {}", replay_dir.display()))?;
        let db_path = root.join("runtime.sqlite3");
        let store = Self {
            paths: RuntimePaths {
                root_dir: root.display().to_string(),
                state_db_path: db_path.display().to_string(),
                profile_dir: profile_dir.display().to_string(),
                artifact_dir: artifact_dir.display().to_string(),
                replay_dir: replay_dir.display().to_string(),
            },
            read_only: false,
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn inspect_existing(root: impl Into<PathBuf>) -> Result<Option<Self>> {
        let root = root.into();
        let db_path = root.join("runtime.sqlite3");
        if !db_path.exists() {
            return Ok(None);
        }
        Ok(Some(Self {
            paths: RuntimePaths {
                root_dir: root.display().to_string(),
                state_db_path: db_path.display().to_string(),
                profile_dir: root.join("profiles").display().to_string(),
                artifact_dir: root.join("artifacts").display().to_string(),
                replay_dir: root.join("replay").display().to_string(),
            },
            read_only: true,
        }))
    }

    pub fn paths(&self) -> &RuntimePaths {
        &self.paths
    }

    pub fn state_db_path(&self) -> &Path {
        Path::new(&self.paths.state_db_path)
    }

    pub fn profile_root(&self) -> &Path {
        Path::new(&self.paths.profile_dir)
    }

    pub fn artifact_root(&self) -> &Path {
        Path::new(&self.paths.artifact_dir)
    }

    fn connection(&self) -> Result<Connection> {
        let conn = if self.read_only {
            Connection::open_with_flags(
                self.state_db_path(),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
        } else {
            Connection::open(self.state_db_path())
        }
        .with_context(|| format!("open {}", self.state_db_path().display()))?;
        if !self.read_only {
            conn.pragma_update(None, "journal_mode", "WAL")
                .context("enable WAL mode")?;
        }
        Ok(conn)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                channel TEXT NOT NULL,
                path TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                channel TEXT NOT NULL,
                mode TEXT NOT NULL,
                status TEXT NOT NULL,
                debug_http_url TEXT NOT NULL,
                browser_ws_url TEXT,
                profile_id TEXT,
                profile_path TEXT,
                pid INTEGER,
                last_error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tabs (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                websocket_url TEXT NOT NULL,
                active INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                run_id TEXT,
                instance_id TEXT NOT NULL,
                tab_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                path TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                bytes INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                source_artifact_id TEXT,
                crop_region TEXT,
                page_index INTEGER,
                checksum_sha256 TEXT
            );
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                entrypoint TEXT NOT NULL,
                detail TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT
            );
            CREATE TABLE IF NOT EXISTS runtime_identities (
                entrypoint TEXT PRIMARY KEY,
                operator_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT NOT NULL UNIQUE,
                run_id TEXT NOT NULL,
                category TEXT NOT NULL,
                action TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                instance_id TEXT,
                tab_id TEXT,
                artifact_id TEXT,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS leases (
                id TEXT PRIMARY KEY,
                resource_kind TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                holder_id TEXT NOT NULL,
                holder_label TEXT,
                mode TEXT NOT NULL,
                granted_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                last_heartbeat_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS attach_continuity (
                scope TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scenario_runs (
                id TEXT PRIMARY KEY,
                scenario_name TEXT NOT NULL,
                scenario_family TEXT NOT NULL,
                scenario_version TEXT NOT NULL,
                tool_surface TEXT NOT NULL,
                runtime_root TEXT,
                commit_sha TEXT,
                branch_name TEXT,
                platform TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                status TEXT NOT NULL,
                summary_path TEXT
            );
            CREATE TABLE IF NOT EXISTS scenario_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES scenario_runs(id),
                ordinal INTEGER NOT NULL,
                step_name TEXT NOT NULL,
                step_kind TEXT NOT NULL,
                command_line TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                status TEXT NOT NULL,
                error_code TEXT
            );
            CREATE TABLE IF NOT EXISTS scenario_assertions (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES scenario_runs(id),
                step_id TEXT REFERENCES scenario_steps(id),
                assertion_name TEXT NOT NULL,
                expected_value TEXT,
                actual_value TEXT,
                status TEXT NOT NULL,
                failure_category TEXT,
                notes TEXT
            );
            CREATE TABLE IF NOT EXISTS latency_samples (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES scenario_runs(id),
                step_id TEXT REFERENCES scenario_steps(id),
                metric_name TEXT NOT NULL,
                sample_ms REAL NOT NULL,
                capture_method TEXT
            );
            CREATE TABLE IF NOT EXISTS environment_fingerprints (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES scenario_runs(id),
                platform TEXT NOT NULL,
                arch TEXT NOT NULL,
                os_version TEXT,
                rust_version TEXT,
                cargo_version TEXT,
                chrome_channel TEXT,
                chrome_version TEXT
            );
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                action TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'pending',
                priority TEXT NOT NULL DEFAULT 'normal',
                params_json TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                latency_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_events_run_sequence
                ON events (run_id, sequence DESC);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_leases_identity
                ON leases (resource_kind, resource_id, holder_id, mode);
            CREATE INDEX IF NOT EXISTS idx_leases_resource_expiry
                ON leases (resource_kind, resource_id, expires_at DESC);
            CREATE INDEX IF NOT EXISTS idx_scenario_runs_family_started
                ON scenario_runs (scenario_family, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_scenario_steps_run_ordinal
                ON scenario_steps (run_id, ordinal ASC);
            CREATE INDEX IF NOT EXISTS idx_scenario_assertions_run_step
                ON scenario_assertions (run_id, step_id, assertion_name);
            CREATE INDEX IF NOT EXISTS idx_latency_samples_run_step
                ON latency_samples (run_id, step_id, metric_name);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_environment_fingerprints_run_id
                ON environment_fingerprints (run_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_agent_state
                ON tasks (agent_id, state, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_tasks_state_created
                ON tasks (state, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_tasks_created_at
                ON tasks (created_at DESC);
            CREATE TABLE IF NOT EXISTS scheduler_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                metric_name TEXT NOT NULL,
                agent_id TEXT,
                counter INTEGER NOT NULL DEFAULT 0,
                last_updated TEXT NOT NULL,
                UNIQUE(metric_name, agent_id)
            );
            ",
        )
        .context("initialize sqlite schema")?;
        ensure_column(&conn, "artifacts", "run_id", "TEXT")?;
        ensure_column(&conn, "artifacts", "source_artifact_id", "TEXT")?;
        ensure_column(&conn, "artifacts", "crop_region", "TEXT")?;
        ensure_column(&conn, "artifacts", "page_index", "INTEGER")?;
        ensure_column(&conn, "artifacts", "checksum_sha256", "TEXT")?;
        conn.execute(
            "
            CREATE INDEX IF NOT EXISTS idx_artifacts_run_created
            ON artifacts (run_id, created_at DESC)
            ",
            [],
        )
        .context("create artifact run index")?;
        Ok(())
    }

    pub fn upsert_profile(&self, profile: &ManagedProfile) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO profiles (id, name, channel, path)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                channel = excluded.channel,
                path = excluded.path
            ",
            params![
                profile.id,
                profile.name,
                profile.channel.as_str(),
                profile.path
            ],
        )
        .context("upsert profile")?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<ManagedProfile>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare("SELECT id, name, channel, path FROM profiles ORDER BY name")
            .context("prepare profile query")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ManagedProfile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    channel: parse_channel(row.get::<_, String>(2)?),
                    path: row.get(3)?,
                })
            })
            .context("query profiles")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect profiles")
    }

    pub fn upsert_instance(&self, instance: &BrowserInstance) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO instances (
                id, name, channel, mode, status, debug_http_url, browser_ws_url,
                profile_id, profile_path, pid, last_error, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                channel = excluded.channel,
                mode = excluded.mode,
                status = excluded.status,
                debug_http_url = excluded.debug_http_url,
                browser_ws_url = excluded.browser_ws_url,
                profile_id = excluded.profile_id,
                profile_path = excluded.profile_path,
                pid = excluded.pid,
                last_error = excluded.last_error,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at
            ",
            params![
                instance.id,
                instance.name,
                instance.channel.as_str(),
                serde_json::to_string(&instance.mode).expect("mode json"),
                serde_json::to_string(&instance.status).expect("status json"),
                instance.debug_http_url,
                instance.browser_ws_url,
                instance.profile_id,
                instance.profile_path,
                instance.pid.map(i64::from),
                instance.last_error,
                instance.created_at,
                instance.updated_at,
            ],
        )
        .context("upsert instance")?;
        Ok(())
    }

    pub fn list_instances(&self) -> Result<Vec<BrowserInstance>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT id, name, channel, mode, status, debug_http_url, browser_ws_url,
                       profile_id, profile_path, pid, last_error, created_at, updated_at
                FROM instances
                ORDER BY updated_at DESC
                ",
            )
            .context("prepare instance query")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BrowserInstance {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    channel: parse_channel(row.get::<_, String>(2)?),
                    mode: serde_json::from_str(&row.get::<_, String>(3)?).expect("mode"),
                    status: serde_json::from_str(&row.get::<_, String>(4)?).expect("status"),
                    debug_http_url: row.get(5)?,
                    browser_ws_url: row.get(6)?,
                    profile_id: row.get(7)?,
                    profile_path: row.get(8)?,
                    pid: row.get::<_, Option<i64>>(9)?.map(|pid| pid as u32),
                    last_error: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .context("query instances")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect instances")
    }

    pub fn get_instance(&self, instance_id: &str) -> Result<Option<BrowserInstance>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, name, channel, mode, status, debug_http_url, browser_ws_url,
                   profile_id, profile_path, pid, last_error, created_at, updated_at
            FROM instances
            WHERE id = ?1
            ",
            [instance_id],
            |row| {
                Ok(BrowserInstance {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    channel: parse_channel(row.get::<_, String>(2)?),
                    mode: serde_json::from_str(&row.get::<_, String>(3)?).expect("mode"),
                    status: serde_json::from_str(&row.get::<_, String>(4)?).expect("status"),
                    debug_http_url: row.get(5)?,
                    browser_ws_url: row.get(6)?,
                    profile_id: row.get(7)?,
                    profile_path: row.get(8)?,
                    pid: row.get::<_, Option<i64>>(9)?.map(|pid| pid as u32),
                    last_error: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            },
        )
        .optional()
        .context("get instance")
    }

    pub fn upsert_attach_continuity(&self, status: &AttachContinuityStatus) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO attach_continuity (scope, payload)
            VALUES ('global', ?1)
            ON CONFLICT(scope) DO UPDATE SET
                payload = excluded.payload
            ",
            [serde_json::to_string(status).context("serialize attach continuity")?],
        )
        .context("upsert attach continuity")?;
        Ok(())
    }

    pub fn get_attach_continuity(&self) -> Result<Option<AttachContinuityStatus>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT payload FROM attach_continuity WHERE scope = 'global'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("get attach continuity")?
        .map(|payload| {
            serde_json::from_str(&payload).context("deserialize attach continuity payload")
        })
        .transpose()
    }

    pub fn replace_tabs(&self, instance_id: &str, tabs: &[BrowserTab]) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction().context("begin tab transaction")?;
        tx.execute("DELETE FROM tabs WHERE instance_id = ?1", [instance_id])
            .context("delete old tabs")?;
        for tab in tabs {
            tx.execute(
                "
                INSERT INTO tabs (
                    id, instance_id, target_id, title, url, websocket_url, active,
                    created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
                params![
                    tab.id,
                    tab.instance_id,
                    tab.target_id,
                    tab.title,
                    tab.url,
                    tab.websocket_url,
                    if tab.active { 1 } else { 0 },
                    tab.created_at,
                    tab.updated_at
                ],
            )
            .context("insert tab")?;
        }
        tx.commit().context("commit tab transaction")?;
        Ok(())
    }

    pub fn list_tabs(&self, instance_id: Option<&str>) -> Result<Vec<BrowserTab>> {
        let conn = self.connection()?;
        let sql = if instance_id.is_some() {
            "
            SELECT id, instance_id, target_id, title, url, websocket_url, active,
                   created_at, updated_at
            FROM tabs
            WHERE instance_id = ?1
            ORDER BY active DESC, updated_at DESC
            "
        } else {
            "
            SELECT id, instance_id, target_id, title, url, websocket_url, active,
                   created_at, updated_at
            FROM tabs
            ORDER BY active DESC, updated_at DESC
            "
        };
        let mut stmt = conn.prepare(sql).context("prepare tab query")?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(BrowserTab {
                id: row.get(0)?,
                instance_id: row.get(1)?,
                target_id: row.get(2)?,
                title: row.get(3)?,
                url: row.get(4)?,
                websocket_url: row.get(5)?,
                active: row.get::<_, i64>(6)? == 1,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        };
        let rows = if let Some(instance_id) = instance_id {
            stmt.query_map([instance_id], mapper)
                .context("query tabs")?
        } else {
            stmt.query_map([], mapper).context("query tabs")?
        };
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect tabs")
    }

    pub fn get_tab(&self, tab_id: &str) -> Result<Option<BrowserTab>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, instance_id, target_id, title, url, websocket_url, active,
                   created_at, updated_at
            FROM tabs
            WHERE id = ?1
            ",
            [tab_id],
            |row| {
                Ok(BrowserTab {
                    id: row.get(0)?,
                    instance_id: row.get(1)?,
                    target_id: row.get(2)?,
                    title: row.get(3)?,
                    url: row.get(4)?,
                    websocket_url: row.get(5)?,
                    active: row.get::<_, i64>(6)? == 1,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .optional()
        .context("get tab")
    }

    pub fn upsert_artifact(&self, artifact: &ArtifactHandle) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO artifacts (
                id, run_id, instance_id, tab_id, kind, path, mime_type, bytes, created_at,
                source_artifact_id, crop_region, page_index, checksum_sha256
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                run_id = excluded.run_id,
                instance_id = excluded.instance_id,
                tab_id = excluded.tab_id,
                kind = excluded.kind,
                path = excluded.path,
                mime_type = excluded.mime_type,
                bytes = excluded.bytes,
                created_at = excluded.created_at,
                source_artifact_id = excluded.source_artifact_id,
                crop_region = excluded.crop_region,
                page_index = excluded.page_index,
                checksum_sha256 = excluded.checksum_sha256
            ",
            params![
                artifact.id,
                artifact.run_id,
                artifact.instance_id,
                artifact.tab_id,
                serde_json::to_string(&artifact.kind).expect("kind"),
                artifact.path,
                artifact.mime_type,
                artifact.bytes as i64,
                artifact.created_at,
                artifact.provenance.source_artifact_id,
                artifact
                    .provenance
                    .crop_region
                    .as_ref()
                    .map(|region| serde_json::to_string(region).expect("crop region")),
                artifact.provenance.page_index.map(i64::from),
                artifact.checksum_sha256,
            ],
        )
        .context("upsert artifact")?;
        Ok(())
    }

    pub fn get_artifact(&self, artifact_id: &str) -> Result<Option<ArtifactHandle>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, run_id, instance_id, tab_id, kind, path, mime_type, bytes, created_at,
                   source_artifact_id, crop_region, page_index, checksum_sha256
            FROM artifacts
            WHERE id = ?1
            ",
            [artifact_id],
            map_artifact_row,
        )
        .optional()
        .context("get artifact")
    }

    pub fn create_run(&self, entrypoint: &str, detail: &str) -> Result<CaptureRun> {
        let run = CaptureRun {
            id: StableId::new(
                IdKind::Run,
                format!("{entrypoint}_{}_{}", std::process::id(), unique_suffix()),
            )
            .into_string(),
            entrypoint: entrypoint.to_string(),
            detail: detail.to_string(),
            status: RunStatus::Active,
            started_at: utc_timestamp(),
            ended_at: None,
        };
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO runs (id, entrypoint, detail, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                run.id,
                run.entrypoint,
                run.detail,
                serde_json::to_string(&run.status).expect("run status"),
                run.started_at,
                run.ended_at,
            ],
        )
        .context("insert run")?;
        Ok(run)
    }

    pub fn latest_active_run(&self, entrypoint: &str) -> Result<Option<CaptureRun>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, entrypoint, detail, status, started_at, ended_at
            FROM runs
            WHERE entrypoint = ?1
              AND status = ?2
            ORDER BY started_at DESC
            LIMIT ?3
            ",
            params![
                entrypoint,
                serde_json::to_string(&RunStatus::Active).expect("run status"),
                1_i64
            ],
            |row| {
                Ok(CaptureRun {
                    id: row.get(0)?,
                    entrypoint: row.get(1)?,
                    detail: row.get(2)?,
                    status: serde_json::from_str(&row.get::<_, String>(3)?).expect("run status"),
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                })
            },
        )
        .optional()
        .context("latest active run")
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<CaptureRun>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, entrypoint, detail, status, started_at, ended_at
            FROM runs
            WHERE id = ?1
            ",
            [run_id],
            |row| {
                Ok(CaptureRun {
                    id: row.get(0)?,
                    entrypoint: row.get(1)?,
                    detail: row.get(2)?,
                    status: serde_json::from_str(&row.get::<_, String>(3)?).expect("run status"),
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                })
            },
        )
        .optional()
        .context("get run")
    }

    pub fn list_runs(&self, limit: usize) -> Result<Vec<CaptureRun>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT id, entrypoint, detail, status, started_at, ended_at
                FROM runs
                ORDER BY started_at DESC
                LIMIT ?1
                ",
            )
            .context("prepare run query")?;
        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(CaptureRun {
                    id: row.get(0)?,
                    entrypoint: row.get(1)?,
                    detail: row.get(2)?,
                    status: serde_json::from_str(&row.get::<_, String>(3)?).expect("run status"),
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                })
            })
            .context("query runs")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect runs")
    }

    pub fn complete_run(&self, run_id: &str, detail: Option<&str>) -> Result<Option<CaptureRun>> {
        let Some(mut run) = self.get_run(run_id)? else {
            return Ok(None);
        };
        if run.status == RunStatus::Completed {
            return Ok(Some(run));
        }
        if let Some(detail) = detail {
            run.detail = detail.to_string();
        }
        run.status = RunStatus::Completed;
        run.ended_at = Some(utc_timestamp());
        let conn = self.connection()?;
        conn.execute(
            "
            UPDATE runs
            SET detail = ?2, status = ?3, ended_at = ?4
            WHERE id = ?1
            ",
            params![
                run.id,
                run.detail,
                serde_json::to_string(&run.status).expect("run status"),
                run.ended_at,
            ],
        )
        .context("complete run")?;
        Ok(Some(run))
    }

    pub fn insert_scenario_run(&self, run: &ScenarioRun) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO scenario_runs (
                id, scenario_name, scenario_family, scenario_version, tool_surface,
                runtime_root, commit_sha, branch_name, platform, started_at,
                finished_at, status, summary_path
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ",
            params![
                run.id,
                run.scenario_name,
                run.scenario_family,
                run.scenario_version,
                run.tool_surface,
                run.runtime_root,
                run.commit_sha,
                run.branch_name,
                run.platform,
                run.started_at,
                run.finished_at,
                run.status,
                run.summary_path,
            ],
        )
        .context("insert scenario run")?;
        Ok(())
    }

    pub fn get_scenario_run(&self, run_id: &str) -> Result<Option<ScenarioRun>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, scenario_name, scenario_family, scenario_version, tool_surface,
                   runtime_root, commit_sha, branch_name, platform, started_at,
                   finished_at, status, summary_path
            FROM scenario_runs
            WHERE id = ?1
            ",
            [run_id],
            map_scenario_run_row,
        )
        .optional()
        .context("get scenario run")
    }

    pub fn list_scenario_runs(
        &self,
        family: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScenarioRun>> {
        let conn = self.connection()?;
        if let Some(family) = family {
            let mut stmt = conn
                .prepare(
                    "
                SELECT id, scenario_name, scenario_family, scenario_version, tool_surface,
                       runtime_root, commit_sha, branch_name, platform, started_at,
                       finished_at, status, summary_path
                FROM scenario_runs
                WHERE scenario_family = ?1
                ORDER BY started_at DESC
                LIMIT ?2
                ",
                )
                .context("prepare scenario run query")?;
            let rows = stmt
                .query_map(params![family, limit as i64], map_scenario_run_row)
                .context("query scenario runs")?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .context("collect scenario runs")
        } else {
            let mut stmt = conn
                .prepare(
                    "
                SELECT id, scenario_name, scenario_family, scenario_version, tool_surface,
                       runtime_root, commit_sha, branch_name, platform, started_at,
                       finished_at, status, summary_path
                FROM scenario_runs
                ORDER BY started_at DESC
                LIMIT ?1
                ",
                )
                .context("prepare scenario run query")?;
            let rows = stmt
                .query_map([limit as i64], map_scenario_run_row)
                .context("query scenario runs")?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .context("collect scenario runs")
        }
    }

    pub fn insert_scenario_step(&self, step: &ScenarioStep) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO scenario_steps (
                id, run_id, ordinal, step_name, step_kind, command_line,
                started_at, finished_at, status, error_code
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                step.id,
                step.run_id,
                step.ordinal,
                step.step_name,
                step.step_kind,
                step.command_line,
                step.started_at,
                step.finished_at,
                step.status,
                step.error_code,
            ],
        )
        .context("insert scenario step")?;
        Ok(())
    }

    pub fn list_scenario_steps(&self, run_id: &str) -> Result<Vec<ScenarioStep>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT id, run_id, ordinal, step_name, step_kind, command_line,
                       started_at, finished_at, status, error_code
                FROM scenario_steps
                WHERE run_id = ?1
                ORDER BY ordinal ASC, started_at ASC, id ASC
                ",
            )
            .context("prepare scenario step query")?;
        let rows = stmt
            .query_map([run_id], map_scenario_step_row)
            .context("query scenario steps")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect scenario steps")
    }

    pub fn get_scenario_step(&self, step_id: &str) -> Result<Option<ScenarioStep>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, run_id, ordinal, step_name, step_kind, command_line,
                   started_at, finished_at, status, error_code
            FROM scenario_steps
            WHERE id = ?1
            ",
            [step_id],
            map_scenario_step_row,
        )
        .optional()
        .context("get scenario step")
    }

    pub fn next_scenario_step_ordinal(&self, run_id: &str) -> Result<i64> {
        let conn = self.connection()?;
        let next_ordinal = conn
            .query_row(
                "
                SELECT COALESCE(MAX(ordinal), 0) + 1
                FROM scenario_steps
                WHERE run_id = ?1
                ",
                [run_id],
                |row| row.get::<_, i64>(0),
            )
            .context("select next scenario step ordinal")?;
        Ok(next_ordinal)
    }

    pub fn create_scenario_step(
        &self,
        step_id: &str,
        run_id: &str,
        step_name: &str,
        step_kind: &str,
        command_line: Option<&str>,
        started_at: &str,
    ) -> Result<ScenarioStep> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("begin scenario step transaction")?;
        let ordinal = tx
            .query_row(
                "
                SELECT COALESCE(MAX(ordinal), 0) + 1
                FROM scenario_steps
                WHERE run_id = ?1
                ",
                [run_id],
                |row| row.get::<_, i64>(0),
            )
            .context("select next scenario step ordinal in transaction")?;

        let step = ScenarioStep {
            id: step_id.to_string(),
            run_id: run_id.to_string(),
            ordinal,
            step_name: step_name.to_string(),
            step_kind: step_kind.to_string(),
            command_line: command_line.map(str::to_string),
            started_at: started_at.to_string(),
            finished_at: None,
            status: "running".to_string(),
            error_code: None,
        };

        tx.execute(
            "
            INSERT INTO scenario_steps (
                id, run_id, ordinal, step_name, step_kind, command_line,
                started_at, finished_at, status, error_code
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                step.id,
                step.run_id,
                step.ordinal,
                step.step_name,
                step.step_kind,
                step.command_line,
                step.started_at,
                step.finished_at,
                step.status,
                step.error_code,
            ],
        )
        .context("insert scenario step in transaction")?;
        tx.commit().context("commit scenario step transaction")?;
        Ok(step)
    }

    pub fn insert_scenario_assertion(&self, assertion: &ScenarioAssertion) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO scenario_assertions (
                id, run_id, step_id, assertion_name, expected_value, actual_value,
                status, failure_category, notes
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                assertion.id,
                assertion.run_id,
                assertion.step_id,
                assertion.assertion_name,
                assertion.expected_value,
                assertion.actual_value,
                assertion.status,
                assertion.failure_category,
                assertion.notes,
            ],
        )
        .context("insert scenario assertion")?;
        Ok(())
    }

    pub fn list_scenario_assertions(&self, run_id: &str) -> Result<Vec<ScenarioAssertion>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT
                    scenario_assertions.id,
                    scenario_assertions.run_id,
                    scenario_assertions.step_id,
                    scenario_assertions.assertion_name,
                    scenario_assertions.expected_value,
                    scenario_assertions.actual_value,
                    scenario_assertions.status,
                    scenario_assertions.failure_category,
                    scenario_assertions.notes
                FROM scenario_assertions
                LEFT JOIN scenario_steps ON scenario_steps.id = scenario_assertions.step_id
                WHERE scenario_assertions.run_id = ?1
                ORDER BY
                    COALESCE(scenario_steps.ordinal, 2147483647) ASC,
                    scenario_assertions.rowid ASC
                ",
            )
            .context("prepare scenario assertion query")?;
        let rows = stmt
            .query_map([run_id], map_scenario_assertion_row)
            .context("query scenario assertions")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect scenario assertions")
    }

    pub fn insert_latency_sample(&self, sample: &LatencySample) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO latency_samples (
                id, run_id, step_id, metric_name, sample_ms, capture_method
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                sample.id,
                sample.run_id,
                sample.step_id,
                sample.metric_name,
                sample.sample_ms.into_inner(),
                sample.capture_method,
            ],
        )
        .context("insert latency sample")?;
        Ok(())
    }

    pub fn list_latency_samples(&self, run_id: &str) -> Result<Vec<LatencySample>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT
                    latency_samples.id,
                    latency_samples.run_id,
                    latency_samples.step_id,
                    latency_samples.metric_name,
                    latency_samples.sample_ms,
                    latency_samples.capture_method
                FROM latency_samples
                LEFT JOIN scenario_steps ON scenario_steps.id = latency_samples.step_id
                WHERE latency_samples.run_id = ?1
                ORDER BY
                    COALESCE(scenario_steps.ordinal, 2147483647) ASC,
                    latency_samples.rowid ASC
                ",
            )
            .context("prepare latency sample query")?;
        let rows = stmt
            .query_map([run_id], map_latency_sample_row)
            .context("query latency samples")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect latency samples")
    }

    pub fn insert_environment_fingerprint(
        &self,
        fingerprint: &EnvironmentFingerprint,
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO environment_fingerprints (
                id, run_id, platform, arch, os_version, rust_version,
                cargo_version, chrome_channel, chrome_version
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                fingerprint.id,
                fingerprint.run_id,
                fingerprint.platform,
                fingerprint.arch,
                fingerprint.os_version,
                fingerprint.rust_version,
                fingerprint.cargo_version,
                fingerprint.chrome_channel,
                fingerprint.chrome_version,
            ],
        )
        .context("insert environment fingerprint")?;
        Ok(())
    }

    pub fn get_environment_fingerprint(
        &self,
        run_id: &str,
    ) -> Result<Option<EnvironmentFingerprint>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, run_id, platform, arch, os_version, rust_version,
                   cargo_version, chrome_channel, chrome_version
            FROM environment_fingerprints
            WHERE run_id = ?1
            ",
            [run_id],
            map_environment_fingerprint_row,
        )
        .optional()
        .context("get environment fingerprint")
    }

    pub fn update_scenario_run(&self, run: &ScenarioRun) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            UPDATE scenario_runs
            SET scenario_name = ?2,
                scenario_family = ?3,
                scenario_version = ?4,
                tool_surface = ?5,
                runtime_root = ?6,
                commit_sha = ?7,
                branch_name = ?8,
                platform = ?9,
                started_at = ?10,
                finished_at = ?11,
                status = ?12,
                summary_path = ?13
            WHERE id = ?1
            ",
            params![
                run.id,
                run.scenario_name,
                run.scenario_family,
                run.scenario_version,
                run.tool_surface,
                run.runtime_root,
                run.commit_sha,
                run.branch_name,
                run.platform,
                run.started_at,
                run.finished_at,
                run.status,
                run.summary_path,
            ],
        )
        .context("update scenario run")?;
        Ok(())
    }

    pub fn update_scenario_step(&self, step: &ScenarioStep) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            UPDATE scenario_steps
            SET run_id = ?2,
                ordinal = ?3,
                step_name = ?4,
                step_kind = ?5,
                command_line = ?6,
                started_at = ?7,
                finished_at = ?8,
                status = ?9,
                error_code = ?10
            WHERE id = ?1
            ",
            params![
                step.id,
                step.run_id,
                step.ordinal,
                step.step_name,
                step.step_kind,
                step.command_line,
                step.started_at,
                step.finished_at,
                step.status,
                step.error_code,
            ],
        )
        .context("update scenario step")?;
        Ok(())
    }

    pub fn upsert_lease(&self, lease: &LeaseRecord) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO leases (
                id, resource_kind, resource_id, holder_id, holder_label, mode,
                granted_at, expires_at, last_heartbeat_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(resource_kind, resource_id, holder_id, mode) DO UPDATE SET
                id = excluded.id,
                holder_label = excluded.holder_label,
                granted_at = excluded.granted_at,
                expires_at = excluded.expires_at,
                last_heartbeat_at = excluded.last_heartbeat_at
            ",
            params![
                lease.id,
                lease.resource_kind.as_str(),
                lease.resource_id,
                lease.holder_id,
                lease.holder_label,
                lease.mode.as_str(),
                lease.granted_at,
                lease.expires_at,
                lease.last_heartbeat_at,
            ],
        )
        .context("upsert lease")?;
        Ok(())
    }

    pub fn acquire_lease(&self, lease: &LeaseRecord, now: &str) -> Result<bool> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("begin lease acquire transaction")?;
        tx.execute("DELETE FROM leases WHERE expires_at <= ?1", [now])
            .context("prune expired leases before acquire")?;
        let renewed = tx
            .query_row(
                "
                SELECT 1
                FROM leases
                WHERE resource_kind = ?1
                  AND resource_id = ?2
                  AND holder_id = ?3
                  AND mode = ?4
                  AND expires_at > ?5
                LIMIT 1
                ",
                params![
                    lease.resource_kind.as_str(),
                    lease.resource_id,
                    lease.holder_id,
                    lease.mode.as_str(),
                    now,
                ],
                |_| Ok(true),
            )
            .optional()
            .context("check lease renewal candidate")?
            .unwrap_or(false);
        if lease.mode == LeaseMode::Writer
            && let Some(active_writer) = tx
                .query_row(
                    "
                    SELECT id, resource_kind, resource_id, holder_id, holder_label, mode,
                           granted_at, expires_at, last_heartbeat_at
                    FROM leases
                    WHERE resource_kind = ?1
                      AND resource_id = ?2
                      AND mode = ?3
                      AND expires_at > ?4
                    ORDER BY granted_at ASC
                    LIMIT 1
                    ",
                    params![
                        lease.resource_kind.as_str(),
                        lease.resource_id,
                        LeaseMode::Writer.as_str(),
                        now,
                    ],
                    map_lease_row,
                )
                .optional()
                .context("get writer lease in acquire transaction")?
        {
            anyhow::ensure!(
                active_writer.holder_id == lease.holder_id,
                "writer lease for {} is held by {} until {}",
                lease.resource_id,
                active_writer.holder_id,
                active_writer.expires_at
            );
        }
        tx.execute(
            "
            INSERT INTO leases (
                id, resource_kind, resource_id, holder_id, holder_label, mode,
                granted_at, expires_at, last_heartbeat_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(resource_kind, resource_id, holder_id, mode) DO UPDATE SET
                id = excluded.id,
                holder_label = excluded.holder_label,
                granted_at = excluded.granted_at,
                expires_at = excluded.expires_at,
                last_heartbeat_at = excluded.last_heartbeat_at
            ",
            params![
                lease.id,
                lease.resource_kind.as_str(),
                lease.resource_id,
                lease.holder_id,
                lease.holder_label,
                lease.mode.as_str(),
                lease.granted_at,
                lease.expires_at,
                lease.last_heartbeat_at,
            ],
        )
        .context("upsert lease in acquire transaction")?;
        tx.commit().context("commit lease acquire transaction")?;
        Ok(renewed)
    }

    pub fn prune_expired_leases(&self, now: &str) -> Result<usize> {
        let conn = self.connection()?;
        let deleted = conn
            .execute("DELETE FROM leases WHERE expires_at <= ?1", [now])
            .context("delete expired leases")?;
        Ok(deleted)
    }

    pub fn get_or_create_runtime_identity(
        &self,
        entrypoint: &str,
    ) -> Result<(RuntimeIdentity, bool)> {
        let conn = self.connection()?;
        if let Some(identity) = conn
            .query_row(
                "
                SELECT entrypoint, operator_id, created_at, updated_at
                FROM runtime_identities
                WHERE entrypoint = ?1
                ",
                [entrypoint],
                |row| {
                    Ok(RuntimeIdentity {
                        entrypoint: row.get(0)?,
                        operator_id: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
            .context("get runtime identity")?
        {
            let updated_at = utc_timestamp();
            conn.execute(
                "
                UPDATE runtime_identities
                SET updated_at = ?2
                WHERE entrypoint = ?1
                ",
                params![entrypoint, updated_at],
            )
            .context("touch runtime identity")?;
            return Ok((
                RuntimeIdentity {
                    updated_at,
                    ..identity
                },
                true,
            ));
        }
        let now = utc_timestamp();
        let identity = RuntimeIdentity {
            entrypoint: entrypoint.to_string(),
            operator_id: StableId::new(IdKind::Lease, format!("operator_{entrypoint}"))
                .into_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        conn.execute(
            "
            INSERT INTO runtime_identities (entrypoint, operator_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                identity.entrypoint,
                identity.operator_id,
                identity.created_at,
                identity.updated_at
            ],
        )
        .context("insert runtime identity")?;
        Ok((identity, false))
    }

    pub fn list_leases(&self, resource_id: Option<&str>, now: &str) -> Result<Vec<LeaseRecord>> {
        let conn = self.connection()?;
        let sql = if resource_id.is_some() {
            "
            SELECT id, resource_kind, resource_id, holder_id, holder_label, mode,
                   granted_at, expires_at, last_heartbeat_at
            FROM leases
            WHERE resource_kind = ?1 AND resource_id = ?2 AND expires_at > ?3
            ORDER BY resource_id ASC, mode ASC, granted_at ASC
            "
        } else {
            "
            SELECT id, resource_kind, resource_id, holder_id, holder_label, mode,
                   granted_at, expires_at, last_heartbeat_at
            FROM leases
            WHERE resource_kind = ?1 AND expires_at > ?2
            ORDER BY resource_id ASC, mode ASC, granted_at ASC
            "
        };
        let mut stmt = conn.prepare(sql).context("prepare lease query")?;
        let rows = if let Some(resource_id) = resource_id {
            stmt.query_map(
                params![LeaseResourceKind::Instance.as_str(), resource_id, now],
                map_lease_row,
            )
            .context("query leases")?
        } else {
            stmt.query_map(
                params![LeaseResourceKind::Instance.as_str(), now],
                map_lease_row,
            )
            .context("query leases")?
        };
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect leases")
    }

    pub fn list_active_leases_for_holder(
        &self,
        holder_id: &str,
        now: &str,
    ) -> Result<Vec<LeaseRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT id, resource_kind, resource_id, holder_id, holder_label, mode,
                       granted_at, expires_at, last_heartbeat_at
                FROM leases
                WHERE holder_id = ?1
                  AND expires_at > ?2
                ORDER BY resource_id ASC, mode ASC, granted_at ASC
                ",
            )
            .context("prepare holder lease query")?;
        let rows = stmt
            .query_map(params![holder_id, now], map_lease_row)
            .context("query holder leases")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect holder leases")
    }

    pub fn get_writer_lease(&self, resource_id: &str, now: &str) -> Result<Option<LeaseRecord>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, resource_kind, resource_id, holder_id, holder_label, mode,
                   granted_at, expires_at, last_heartbeat_at
            FROM leases
            WHERE resource_kind = ?1
              AND resource_id = ?2
              AND mode = ?3
              AND expires_at > ?4
            ORDER BY granted_at ASC
            LIMIT 1
            ",
            params![
                LeaseResourceKind::Instance.as_str(),
                resource_id,
                LeaseMode::Writer.as_str(),
                now,
            ],
            map_lease_row,
        )
        .optional()
        .context("get writer lease")
    }

    pub fn delete_leases(
        &self,
        resource_id: &str,
        holder_id: &str,
        mode: Option<LeaseMode>,
    ) -> Result<usize> {
        let conn = self.connection()?;
        let deleted = if let Some(mode) = mode {
            conn.execute(
                "
                DELETE FROM leases
                WHERE resource_kind = ?1
                  AND resource_id = ?2
                  AND holder_id = ?3
                  AND mode = ?4
                ",
                params![
                    LeaseResourceKind::Instance.as_str(),
                    resource_id,
                    holder_id,
                    mode.as_str(),
                ],
            )
        } else {
            conn.execute(
                "
                DELETE FROM leases
                WHERE resource_kind = ?1
                  AND resource_id = ?2
                  AND holder_id = ?3
                ",
                params![LeaseResourceKind::Instance.as_str(), resource_id, holder_id,],
            )
        }
        .context("delete leases")?;
        Ok(deleted)
    }

    pub fn transfer_writer_lease(
        &self,
        resource_id: &str,
        from_holder_id: &str,
        replacement: &LeaseRecord,
        now: &str,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .context("begin lease transfer transaction")?;
        tx.execute("DELETE FROM leases WHERE expires_at <= ?1", [now])
            .context("prune expired leases before transfer")?;
        let deleted = tx
            .execute(
                "
                DELETE FROM leases
                WHERE resource_kind = ?1
                  AND resource_id = ?2
                  AND holder_id = ?3
                  AND mode = ?4
                ",
                params![
                    LeaseResourceKind::Instance.as_str(),
                    resource_id,
                    from_holder_id,
                    LeaseMode::Writer.as_str(),
                ],
            )
            .context("delete transferred writer lease")?;
        anyhow::ensure!(
            deleted == 1,
            "writer lease transfer requires an active writer lease for {from_holder_id}"
        );
        tx.execute(
            "
            INSERT INTO leases (
                id, resource_kind, resource_id, holder_id, holder_label, mode,
                granted_at, expires_at, last_heartbeat_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(resource_kind, resource_id, holder_id, mode) DO UPDATE SET
                id = excluded.id,
                holder_label = excluded.holder_label,
                granted_at = excluded.granted_at,
                expires_at = excluded.expires_at,
                last_heartbeat_at = excluded.last_heartbeat_at
            ",
            params![
                replacement.id,
                replacement.resource_kind.as_str(),
                replacement.resource_id,
                replacement.holder_id,
                replacement.holder_label,
                replacement.mode.as_str(),
                replacement.granted_at,
                replacement.expires_at,
                replacement.last_heartbeat_at,
            ],
        )
        .context("insert replacement writer lease")?;
        tx.commit().context("commit lease transfer transaction")?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_event(
        &self,
        run_id: &str,
        category: &str,
        action: &str,
        level: EventLevel,
        message: &str,
        instance_id: Option<&str>,
        tab_id: Option<&str>,
        artifact_id: Option<&str>,
        data: Value,
    ) -> Result<RuntimeEvent> {
        let timestamp = utc_timestamp();
        let id = StableId::new(
            IdKind::Event,
            format!("{run_id}_{category}_{action}_{}", unique_suffix()),
        )
        .into_string();
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO events (
                id, run_id, category, action, level, message,
                instance_id, tab_id, artifact_id, data, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                id,
                run_id,
                category,
                action,
                serde_json::to_string(&level).expect("event level"),
                message,
                instance_id,
                tab_id,
                artifact_id,
                serde_json::to_string(&data).expect("event data"),
                timestamp,
            ],
        )
        .context("append event")?;
        let sequence = conn.last_insert_rowid() as u64;
        Ok(RuntimeEvent {
            schema_version: 1,
            id,
            run_id: run_id.to_string(),
            sequence,
            category: category.to_string(),
            action: action.to_string(),
            level,
            message: message.to_string(),
            instance_id: instance_id.map(str::to_string),
            tab_id: tab_id.map(str::to_string),
            artifact_id: artifact_id.map(str::to_string),
            data,
            timestamp,
        })
    }

    pub fn tail_events(&self, run_id: Option<&str>, limit: usize) -> Result<Vec<RuntimeEvent>> {
        let conn = self.connection()?;
        let sql = if run_id.is_some() {
            "
            SELECT sequence, id, run_id, category, action, level, message,
                   instance_id, tab_id, artifact_id, data, created_at
            FROM events
            WHERE run_id = ?1
            ORDER BY sequence DESC
            LIMIT ?2
            "
        } else {
            "
            SELECT sequence, id, run_id, category, action, level, message,
                   instance_id, tab_id, artifact_id, data, created_at
            FROM events
            ORDER BY sequence DESC
            LIMIT ?1
            "
        };
        let mut stmt = conn.prepare(sql).context("prepare event query")?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(RuntimeEvent {
                schema_version: 1,
                sequence: row.get::<_, i64>(0)? as u64,
                id: row.get(1)?,
                run_id: row.get(2)?,
                category: row.get(3)?,
                action: row.get(4)?,
                level: serde_json::from_str(&row.get::<_, String>(5)?).expect("event level"),
                message: row.get(6)?,
                instance_id: row.get(7)?,
                tab_id: row.get(8)?,
                artifact_id: row.get(9)?,
                data: serde_json::from_str(&row.get::<_, String>(10)?).expect("event data"),
                timestamp: row.get(11)?,
            })
        };
        let rows = if let Some(run_id) = run_id {
            stmt.query_map(params![run_id, limit as i64], mapper)
                .context("query events")?
        } else {
            stmt.query_map([limit as i64], mapper)
                .context("query events")?
        };
        let mut events = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("collect events")?;
        events.reverse();
        Ok(events)
    }

    pub fn list_events_for_run(&self, run_id: &str) -> Result<Vec<RuntimeEvent>> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT sequence, id, run_id, category, action, level, message,
                       instance_id, tab_id, artifact_id, data, created_at
                FROM events
                WHERE run_id = ?1
                ORDER BY sequence ASC
                ",
            )
            .context("prepare replay event query")?;
        let rows = stmt
            .query_map([run_id], |row| {
                Ok(RuntimeEvent {
                    schema_version: 1,
                    sequence: row.get::<_, i64>(0)? as u64,
                    id: row.get(1)?,
                    run_id: row.get(2)?,
                    category: row.get(3)?,
                    action: row.get(4)?,
                    level: serde_json::from_str(&row.get::<_, String>(5)?).expect("event level"),
                    message: row.get(6)?,
                    instance_id: row.get(7)?,
                    tab_id: row.get(8)?,
                    artifact_id: row.get(9)?,
                    data: serde_json::from_str(&row.get::<_, String>(10)?).expect("event data"),
                    timestamp: row.get(11)?,
                })
            })
            .context("query replay events")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("collect replay events")
    }

    pub fn list_artifacts_for_run(&self, run_id: &str) -> Result<Vec<ArtifactHandle>> {
        self.list_artifacts(None, Some(run_id))
    }

    pub fn list_artifacts(
        &self,
        instance_id: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<Vec<ArtifactHandle>> {
        let conn = self.connection()?;
        match (instance_id, run_id) {
            (Some(instance_id), Some(run_id)) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, run_id, instance_id, tab_id, kind, path, mime_type, bytes,
                               created_at, source_artifact_id, crop_region, page_index,
                               checksum_sha256
                        FROM artifacts
                        WHERE instance_id = ?1 AND run_id = ?2
                        ORDER BY created_at DESC
                        ",
                    )
                    .context("prepare artifact inventory query")?;
                let rows = stmt
                    .query_map(params![instance_id, run_id], map_artifact_row)
                    .context("query artifact inventory")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect artifact inventory")
            }
            (Some(instance_id), None) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, run_id, instance_id, tab_id, kind, path, mime_type, bytes,
                               created_at, source_artifact_id, crop_region, page_index,
                               checksum_sha256
                        FROM artifacts
                        WHERE instance_id = ?1
                        ORDER BY created_at DESC
                        ",
                    )
                    .context("prepare artifact inventory query")?;
                let rows = stmt
                    .query_map([instance_id], map_artifact_row)
                    .context("query artifact inventory")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect artifact inventory")
            }
            (None, Some(run_id)) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, run_id, instance_id, tab_id, kind, path, mime_type, bytes,
                               created_at, source_artifact_id, crop_region, page_index,
                               checksum_sha256
                        FROM artifacts
                        WHERE run_id = ?1
                        ORDER BY created_at DESC
                        ",
                    )
                    .context("prepare artifact inventory query")?;
                let rows = stmt
                    .query_map([run_id], map_artifact_row)
                    .context("query artifact inventory")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect artifact inventory")
            }
            (None, None) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, run_id, instance_id, tab_id, kind, path, mime_type, bytes,
                               created_at, source_artifact_id, crop_region, page_index,
                               checksum_sha256
                        FROM artifacts
                        ORDER BY created_at DESC
                        ",
                    )
                    .context("prepare artifact inventory query")?;
                let rows = stmt
                    .query_map([], map_artifact_row)
                    .context("query artifact inventory")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect artifact inventory")
            }
        }
    }

    pub fn insert_task(&self, task: &TaskRecord) -> Result<()> {
        let conn = self.connection()?;
        let params_json = task
            .params
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context("serialize task params")?;
        let latency_ms = task
            .latency_ms
            .map(i64::try_from)
            .transpose()
            .context("convert task latency to sqlite integer")?;
        conn.execute(
            "
            INSERT INTO tasks (
                id, agent_id, action, state, priority, params_json,
                created_at, started_at, completed_at, latency_ms
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                task.id.as_str(),
                task.agent_id.as_str(),
                task.action.as_str(),
                task.state.as_str(),
                task.priority.as_str(),
                params_json,
                task.created_at.as_str(),
                task.started_at.as_deref(),
                task.completed_at.as_deref(),
                latency_ms
            ],
        )
        .context("insert task")?;
        Ok(())
    }

    pub fn update_task_state(
        &self,
        id: &str,
        state: TaskState,
        started_at: Option<&str>,
        completed_at: Option<&str>,
        latency_ms: Option<u64>,
    ) -> Result<()> {
        let conn = self.connection()?;
        let latency_ms = latency_ms
            .map(i64::try_from)
            .transpose()
            .context("convert task latency to sqlite integer")?;
        let updated = conn
            .execute(
                "
            UPDATE tasks
            SET state = ?2, started_at = COALESCE(?3, started_at),
                completed_at = COALESCE(?4, completed_at),
                latency_ms = COALESCE(?5, latency_ms)
            WHERE id = ?1
            ",
                params![id, state.as_str(), started_at, completed_at, latency_ms],
            )
            .context("update task state")?;
        anyhow::ensure!(updated == 1, "update task state: task {id} not found");
        Ok(())
    }

    pub fn query_tasks(
        &self,
        agent_id: Option<&str>,
        state: Option<TaskState>,
        limit: usize,
    ) -> Result<Vec<TaskRecord>> {
        let conn = self.connection()?;
        let state = state.as_ref().map(TaskState::as_str);
        match (agent_id, state) {
            (Some(agent), Some(st)) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, agent_id, action, state, priority, params_json,
                               created_at, started_at, completed_at, latency_ms
                        FROM tasks
                        WHERE agent_id = ?1 AND state = ?2
                        ORDER BY created_at DESC
                        LIMIT ?3
                        ",
                    )
                    .context("prepare task query")?;
                let rows = stmt
                    .query_map(params![agent, st, limit as i64], map_task_row)
                    .context("query tasks")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect tasks")
            }
            (Some(agent), None) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, agent_id, action, state, priority, params_json,
                               created_at, started_at, completed_at, latency_ms
                        FROM tasks
                        WHERE agent_id = ?1
                        ORDER BY created_at DESC
                        LIMIT ?2
                        ",
                    )
                    .context("prepare task query")?;
                let rows = stmt
                    .query_map(params![agent, limit as i64], map_task_row)
                    .context("query tasks")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect tasks")
            }
            (None, Some(st)) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, agent_id, action, state, priority, params_json,
                               created_at, started_at, completed_at, latency_ms
                        FROM tasks
                        WHERE state = ?1
                        ORDER BY created_at DESC
                        LIMIT ?2
                        ",
                    )
                    .context("prepare task query")?;
                let rows = stmt
                    .query_map(params![st, limit as i64], map_task_row)
                    .context("query tasks")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect tasks")
            }
            (None, None) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, agent_id, action, state, priority, params_json,
                               created_at, started_at, completed_at, latency_ms
                        FROM tasks
                        ORDER BY created_at DESC
                        LIMIT ?1
                        ",
                    )
                    .context("prepare task query")?;
                let rows = stmt
                    .query_map([limit as i64], map_task_row)
                    .context("query tasks")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect tasks")
            }
        }
    }

    pub fn query_task(&self, id: &str) -> Result<Option<TaskRecord>> {
        let conn = self.connection()?;
        conn.query_row(
            "
            SELECT id, agent_id, action, state, priority, params_json,
                   created_at, started_at, completed_at, latency_ms
            FROM tasks
            WHERE id = ?1
            ",
            [id],
            map_task_row,
        )
        .optional()
        .context("query task")
    }

    pub fn latest_timestamp(&self) -> String {
        utc_timestamp()
    }

    pub fn increment_scheduler_metric(
        &self,
        metric_name: &str,
        agent_id: Option<&str>,
        timestamp: &str,
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "
            INSERT INTO scheduler_metrics (metric_name, agent_id, counter, last_updated)
            VALUES (?1, ?2, 1, ?3)
            ON CONFLICT(metric_name, agent_id) DO UPDATE SET
                counter = counter + 1,
                last_updated = excluded.last_updated
            ",
            params![metric_name, agent_id, timestamp],
        )
        .context("increment scheduler metric")?;
        Ok(())
    }

    pub fn query_scheduler_metrics(
        &self,
        metric_name: Option<&str>,
        agent_id: Option<&str>,
    ) -> Result<Vec<SchedulerMetricRow>> {
        let conn = self.connection()?;
        match (metric_name, agent_id) {
            (Some(metric_name), Some(agent_id)) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, metric_name, agent_id, counter, last_updated
                        FROM scheduler_metrics
                        WHERE metric_name = ?1 AND agent_id = ?2
                        ORDER BY metric_name, agent_id
                        ",
                    )
                    .context("prepare scheduler metrics query")?;
                let rows = stmt
                    .query_map(params![metric_name, agent_id], map_scheduler_metric_row)
                    .context("query scheduler metrics")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect scheduler metrics")
            }
            (Some(metric_name), None) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, metric_name, agent_id, counter, last_updated
                        FROM scheduler_metrics
                        WHERE metric_name = ?1
                        ORDER BY metric_name, agent_id
                        ",
                    )
                    .context("prepare scheduler metrics query")?;
                let rows = stmt
                    .query_map(params![metric_name], map_scheduler_metric_row)
                    .context("query scheduler metrics")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect scheduler metrics")
            }
            (None, Some(agent_id)) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, metric_name, agent_id, counter, last_updated
                        FROM scheduler_metrics
                        WHERE agent_id = ?1
                        ORDER BY metric_name, agent_id
                        ",
                    )
                    .context("prepare scheduler metrics query")?;
                let rows = stmt
                    .query_map(params![agent_id], map_scheduler_metric_row)
                    .context("query scheduler metrics")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect scheduler metrics")
            }
            (None, None) => {
                let mut stmt = conn
                    .prepare(
                        "
                        SELECT id, metric_name, agent_id, counter, last_updated
                        FROM scheduler_metrics
                        ORDER BY metric_name, agent_id
                        ",
                    )
                    .context("prepare scheduler metrics query")?;
                let rows = stmt
                    .query_map([], map_scheduler_metric_row)
                    .context("query scheduler metrics")?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .context("collect scheduler metrics")
            }
        }
    }

    pub fn reset_scheduler_metrics(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM scheduler_metrics", [])
            .context("reset scheduler metrics")?;
        Ok(())
    }
}

fn map_artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactHandle> {
    let crop_region = row
        .get::<_, Option<String>>(10)?
        .map(|value| serde_json::from_str::<NormalizedRegion>(&value).expect("crop region"));
    Ok(ArtifactHandle {
        id: row.get(0)?,
        run_id: row.get(1)?,
        instance_id: row.get(2)?,
        tab_id: row.get(3)?,
        kind: serde_json::from_str(&row.get::<_, String>(4)?).expect("artifact kind"),
        path: row.get(5)?,
        mime_type: row.get(6)?,
        bytes: row.get::<_, i64>(7)? as usize,
        created_at: row.get(8)?,
        checksum_sha256: row.get(12)?,
        provenance: ArtifactProvenance {
            source_artifact_id: row.get(9)?,
            crop_region,
            page_index: row.get::<_, Option<i64>>(11)?.map(|value| value as u32),
        },
    })
}

fn map_scenario_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScenarioRun> {
    Ok(ScenarioRun {
        id: row.get(0)?,
        scenario_name: row.get(1)?,
        scenario_family: row.get(2)?,
        scenario_version: row.get(3)?,
        tool_surface: row.get(4)?,
        runtime_root: row.get(5)?,
        commit_sha: row.get(6)?,
        branch_name: row.get(7)?,
        platform: row.get(8)?,
        started_at: row.get(9)?,
        finished_at: row.get(10)?,
        status: row.get(11)?,
        summary_path: row.get(12)?,
    })
}

fn map_scenario_step_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScenarioStep> {
    Ok(ScenarioStep {
        id: row.get(0)?,
        run_id: row.get(1)?,
        ordinal: row.get(2)?,
        step_name: row.get(3)?,
        step_kind: row.get(4)?,
        command_line: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        status: row.get(8)?,
        error_code: row.get(9)?,
    })
}

fn map_scenario_assertion_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScenarioAssertion> {
    Ok(ScenarioAssertion {
        id: row.get(0)?,
        run_id: row.get(1)?,
        step_id: row.get(2)?,
        assertion_name: row.get(3)?,
        expected_value: row.get(4)?,
        actual_value: row.get(5)?,
        status: row.get(6)?,
        failure_category: row.get(7)?,
        notes: row.get(8)?,
    })
}

fn map_latency_sample_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LatencySample> {
    Ok(LatencySample {
        id: row.get(0)?,
        run_id: row.get(1)?,
        step_id: row.get(2)?,
        metric_name: row.get(3)?,
        sample_ms: row.get::<_, f64>(4)?.into(),
        capture_method: row.get(5)?,
    })
}

fn map_environment_fingerprint_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<EnvironmentFingerprint> {
    Ok(EnvironmentFingerprint {
        id: row.get(0)?,
        run_id: row.get(1)?,
        platform: row.get(2)?,
        arch: row.get(3)?,
        os_version: row.get(4)?,
        rust_version: row.get(5)?,
        cargo_version: row.get(6)?,
        chrome_channel: row.get(7)?,
        chrome_version: row.get(8)?,
    })
}

fn map_lease_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LeaseRecord> {
    Ok(LeaseRecord {
        id: row.get(0)?,
        resource_kind: parse_lease_resource_kind(row.get::<_, String>(1)?),
        resource_id: row.get(2)?,
        holder_id: row.get(3)?,
        holder_label: row.get(4)?,
        mode: parse_lease_mode(row.get::<_, String>(5)?),
        granted_at: row.get(6)?,
        expires_at: row.get(7)?,
        last_heartbeat_at: row.get(8)?,
    })
}

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecord> {
    Ok(TaskRecord {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        action: row.get(2)?,
        state: parse_task_state(row.get(3)?)?,
        priority: parse_task_priority(row.get(4)?)?,
        params: parse_task_params(row.get(5)?)?,
        created_at: row.get(6)?,
        started_at: row.get(7)?,
        completed_at: row.get(8)?,
        latency_ms: parse_optional_latency(row.get(9)?)?,
    })
}

fn map_scheduler_metric_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SchedulerMetricRow> {
    Ok(SchedulerMetricRow {
        id: row.get(0)?,
        metric_name: row.get(1)?,
        agent_id: row.get(2)?,
        counter: row.get(3)?,
        last_updated: row.get(4)?,
    })
}

fn parse_channel(value: String) -> pengu_mesh_shared::BrowserChannel {
    match value.as_str() {
        "chrome-dev" => pengu_mesh_shared::BrowserChannel::ChromeDev,
        "chromium" => pengu_mesh_shared::BrowserChannel::Chromium,
        _ => pengu_mesh_shared::BrowserChannel::Chrome,
    }
}

fn parse_lease_resource_kind(value: String) -> LeaseResourceKind {
    serde_json::from_str(&value).unwrap_or(match value.as_str() {
        "instance" => LeaseResourceKind::Instance,
        _ => LeaseResourceKind::Instance,
    })
}

fn parse_lease_mode(value: String) -> LeaseMode {
    serde_json::from_str(&value).unwrap_or(match value.as_str() {
        "observer" => LeaseMode::Observer,
        _ => LeaseMode::Writer,
    })
}

fn parse_task_state(value: String) -> rusqlite::Result<TaskState> {
    serde_json::from_value(Value::String(value))
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(error)))
}

fn parse_task_priority(value: String) -> rusqlite::Result<TaskPriority> {
    serde_json::from_value(Value::String(value))
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error)))
}

fn parse_task_params(value: Option<String>) -> rusqlite::Result<Option<Value>> {
    value
        .map(|params| {
            serde_json::from_str(&params).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

fn parse_optional_latency(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value
        .map(|latency| {
            u64::try_from(latency).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(9, Type::Integer, Box::new(error))
            })
        })
        .transpose()
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&pragma)
        .with_context(|| format!("prepare table info for {table}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .with_context(|| format!("query table info for {table}"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("collect table info for {table}"))?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    if let Err(error) = conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    ) {
        if error.to_string().contains("duplicate column name") {
            return Ok(());
        }
        return Err(error).with_context(|| format!("add {column} column to {table}"));
    }
    Ok(())
}

fn unique_suffix() -> i128 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let time_component = OffsetDateTime::now_utc().unix_timestamp_nanos();
    let counter_component = i128::from(COUNTER.fetch_add(1, Ordering::Relaxed));
    time_component * 1_000 + counter_component
}

#[cfg(test)]
mod tests {
    use super::StateStore;
    use pengu_mesh_shared::{
        ArtifactHandle, ArtifactKind, ArtifactProvenance, AttachContinuityStatus,
        AttachResolutionKind, BrowserChannel, BrowserInstance, BrowserTab, EnvironmentFingerprint,
        EventLevel, InstanceMode, InstanceStatus, LatencySample, LeaseMode, LeaseRecord,
        LeaseResourceKind, ManagedProfile, ScenarioAssertion, ScenarioRun, ScenarioStep,
        TaskPriority, TaskRecord, TaskState,
    };
    use serde_json::json;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    #[test]
    fn persists_profiles_instances_tabs_and_artifacts() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        store
            .upsert_profile(&ManagedProfile {
                id: "prof_dev".into(),
                name: "Chrome Dev".into(),
                channel: BrowserChannel::ChromeDev,
                path: tempdir.path().join("profile").display().to_string(),
            })
            .expect("profile");

        let instance = BrowserInstance {
            id: "inst_demo".into(),
            name: "demo".into(),
            channel: BrowserChannel::ChromeDev,
            mode: InstanceMode::Managed,
            status: InstanceStatus::Running,
            debug_http_url: "http://127.0.0.1:9222".into(),
            browser_ws_url: Some("ws://127.0.0.1:9222/devtools/browser/demo".into()),
            profile_id: Some("prof_dev".into()),
            profile_path: Some(tempdir.path().join("profile").display().to_string()),
            pid: Some(1234),
            last_error: None,
            created_at: "2026-03-11T12:00:00Z".into(),
            updated_at: "2026-03-11T12:00:00Z".into(),
        };
        store.upsert_instance(&instance).expect("instance");

        store
            .replace_tabs(
                "inst_demo",
                &[BrowserTab {
                    id: "tab_demo".into(),
                    instance_id: "inst_demo".into(),
                    target_id: "TARGET-1".into(),
                    title: "Example".into(),
                    url: "https://example.com".into(),
                    websocket_url: "ws://127.0.0.1:9222/devtools/page/TARGET-1".into(),
                    active: true,
                    created_at: "2026-03-11T12:00:00Z".into(),
                    updated_at: "2026-03-11T12:00:00Z".into(),
                }],
            )
            .expect("tabs");

        let run = store
            .create_run("pengu-mesh", "capture recording active")
            .expect("run");

        store
            .upsert_artifact(&ArtifactHandle {
                id: "artifact_demo".into(),
                run_id: Some(run.id.clone()),
                instance_id: "inst_demo".into(),
                tab_id: "tab_demo".into(),
                kind: ArtifactKind::Screenshot,
                path: tempdir.path().join("artifact.png").display().to_string(),
                mime_type: "image/png".into(),
                bytes: 42,
                created_at: "2026-03-11T12:00:00Z".into(),
                checksum_sha256: Some("demo".into()),
                provenance: ArtifactProvenance::primary(),
            })
            .expect("artifact");
        let event = store
            .append_event(
                &run.id,
                "instance",
                "start",
                EventLevel::Info,
                "managed browser started",
                Some("inst_demo"),
                Some("tab_demo"),
                Some("artifact_demo"),
                json!({"channel": "chrome-dev"}),
            )
            .expect("event");
        let completed = store
            .complete_run(&run.id, Some("capture recording stopped"))
            .expect("complete run")
            .expect("completed run");

        assert_eq!(store.list_profiles().expect("profiles").len(), 1);
        assert_eq!(store.list_instances().expect("instances").len(), 1);
        assert_eq!(store.list_tabs(Some("inst_demo")).expect("tabs").len(), 1);
        assert!(store.get_tab("tab_demo").expect("tab").is_some());
        assert!(store.get_instance("inst_demo").expect("instance").is_some());
        assert_eq!(
            store
                .tail_events(Some(&run.id), 10)
                .expect("events")
                .into_iter()
                .map(|event| event.id)
                .collect::<Vec<_>>(),
            vec![event.id]
        );
        assert_eq!(
            store
                .list_artifacts_for_run(&run.id)
                .expect("artifacts")
                .len(),
            1
        );
        assert!(completed.ended_at.is_some());
    }

    #[test]
    fn list_artifacts_returns_empty_vec_for_empty_database_filters() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        assert_eq!(
            store.list_artifacts(None, None).expect("all artifacts"),
            vec![]
        );
        assert_eq!(
            store
                .list_artifacts(Some("inst_demo"), None)
                .expect("instance artifacts"),
            vec![]
        );
        assert_eq!(
            store
                .list_artifacts(None, Some("run_demo"))
                .expect("run artifacts"),
            vec![]
        );
    }

    #[test]
    fn persists_attach_continuity_status() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");
        let status = AttachContinuityStatus {
            outcome: Some(pengu_mesh_shared::AttachContinuityOutcome::ReusedExistingInstance),
            freshness: pengu_mesh_shared::AttachContinuityFreshness::Live,
            last_resolution: Some(AttachResolutionKind::DebugHttpUrl),
            last_instance_id: Some("inst_attach_demo".into()),
            last_debug_http_url: Some("http://127.0.0.1:9222".into()),
            last_requested_cdp_url: Some("ws://127.0.0.1:9222/devtools/browser/requested".into()),
            last_browser_ws_url: Some("ws://127.0.0.1:9222/devtools/browser/live".into()),
            reused_existing_instance: true,
            endpoint_refreshed: true,
            updated_at: Some("2026-03-12T02:40:00Z".into()),
        };

        store
            .upsert_attach_continuity(&status)
            .expect("upsert attach continuity");
        let loaded = store
            .get_attach_continuity()
            .expect("get attach continuity")
            .expect("attach continuity payload");
        assert_eq!(loaded, status);
    }

    #[test]
    fn prunes_and_transfers_instance_leases() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        store
            .upsert_lease(&LeaseRecord {
                id: "lease_writer".into(),
                resource_kind: LeaseResourceKind::Instance,
                resource_id: "inst_demo".into(),
                holder_id: "agent_alpha".into(),
                holder_label: Some("Alpha".into()),
                mode: LeaseMode::Writer,
                granted_at: "2026-03-11T12:00:00Z".into(),
                expires_at: "2026-03-11T12:05:00Z".into(),
                last_heartbeat_at: "2026-03-11T12:00:00Z".into(),
            })
            .expect("writer lease");
        store
            .upsert_lease(&LeaseRecord {
                id: "lease_observer".into(),
                resource_kind: LeaseResourceKind::Instance,
                resource_id: "inst_demo".into(),
                holder_id: "agent_beta".into(),
                holder_label: Some("Beta".into()),
                mode: LeaseMode::Observer,
                granted_at: "2026-03-11T12:00:00Z".into(),
                expires_at: "2026-03-11T12:10:00Z".into(),
                last_heartbeat_at: "2026-03-11T12:00:00Z".into(),
            })
            .expect("observer lease");

        assert_eq!(
            store
                .list_leases(Some("inst_demo"), "2026-03-11T12:01:00Z")
                .expect("leases")
                .len(),
            2
        );
        assert_eq!(
            store
                .prune_expired_leases("2026-03-11T12:06:00Z")
                .expect("prune"),
            1
        );
        assert_eq!(
            store
                .list_leases(Some("inst_demo"), "2026-03-11T12:06:00Z")
                .expect("leases after prune")
                .len(),
            1
        );

        store
            .upsert_lease(&LeaseRecord {
                id: "lease_writer_recreated".into(),
                resource_kind: LeaseResourceKind::Instance,
                resource_id: "inst_demo".into(),
                holder_id: "agent_alpha".into(),
                holder_label: Some("Alpha".into()),
                mode: LeaseMode::Writer,
                granted_at: "2026-03-11T12:06:30Z".into(),
                expires_at: "2026-03-11T12:12:00Z".into(),
                last_heartbeat_at: "2026-03-11T12:06:30Z".into(),
            })
            .expect("writer recreated");
        store
            .transfer_writer_lease(
                "inst_demo",
                "agent_alpha",
                &LeaseRecord {
                    id: "lease_writer_gamma".into(),
                    resource_kind: LeaseResourceKind::Instance,
                    resource_id: "inst_demo".into(),
                    holder_id: "agent_gamma".into(),
                    holder_label: Some("Gamma".into()),
                    mode: LeaseMode::Writer,
                    granted_at: "2026-03-11T12:07:00Z".into(),
                    expires_at: "2026-03-11T12:15:00Z".into(),
                    last_heartbeat_at: "2026-03-11T12:07:00Z".into(),
                },
                "2026-03-11T12:07:00Z",
            )
            .expect("transfer writer");

        let writer = store
            .get_writer_lease("inst_demo", "2026-03-11T12:07:01Z")
            .expect("writer lookup")
            .expect("writer present");
        assert_eq!(writer.holder_id, "agent_gamma");
        assert_eq!(
            store
                .delete_leases("inst_demo", "agent_gamma", Some(LeaseMode::Writer))
                .expect("delete writer"),
            1
        );
    }

    #[test]
    fn serializes_concurrent_writer_acquire_to_single_holder() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");
        let barrier = Arc::new(Barrier::new(3));
        let store_alpha = store.clone();
        let store_beta = store.clone();
        let barrier_alpha = Arc::clone(&barrier);
        let barrier_beta = Arc::clone(&barrier);

        let writer_for = |holder_id: &str| LeaseRecord {
            id: format!("lease_{holder_id}"),
            resource_kind: LeaseResourceKind::Instance,
            resource_id: "inst_demo".into(),
            holder_id: holder_id.into(),
            holder_label: Some(holder_id.into()),
            mode: LeaseMode::Writer,
            granted_at: "2026-03-12T03:00:00Z".into(),
            expires_at: "2026-03-12T03:05:00Z".into(),
            last_heartbeat_at: "2026-03-12T03:00:00Z".into(),
        };

        let alpha = thread::spawn(move || {
            barrier_alpha.wait();
            store_alpha.acquire_lease(&writer_for("agent_alpha"), "2026-03-12T03:00:00Z")
        });
        let beta = thread::spawn(move || {
            barrier_beta.wait();
            store_beta.acquire_lease(&writer_for("agent_beta"), "2026-03-12T03:00:00Z")
        });

        barrier.wait();
        let alpha_result = alpha.join().expect("alpha join");
        let beta_result = beta.join().expect("beta join");
        assert!(
            alpha_result.is_ok() ^ beta_result.is_ok(),
            "exactly one writer acquire should succeed"
        );

        let leases = store
            .list_leases(Some("inst_demo"), "2026-03-12T03:00:01Z")
            .expect("list leases");
        let writers = leases
            .iter()
            .filter(|lease| lease.mode == LeaseMode::Writer)
            .collect::<Vec<_>>();
        assert_eq!(writers.len(), 1);
    }

    #[test]
    fn recovers_latest_active_run_and_runtime_identity() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        let first = store
            .create_run("pengu-mesh-daemon", "capture recording active")
            .expect("first run");
        let second = store
            .create_run("pengu-mesh-daemon", "capture recording active")
            .expect("second run");
        let third = store
            .create_run("pengu-mesh", "capture recording active")
            .expect("third run");
        let completed = store
            .complete_run(&first.id, Some("capture recording stopped"))
            .expect("complete first")
            .expect("completed run");

        let latest = store
            .latest_active_run("pengu-mesh-daemon")
            .expect("latest active daemon run")
            .expect("daemon active run");
        assert_eq!(latest.id, second.id);
        assert!(completed.ended_at.is_some());
        assert_eq!(
            store
                .latest_active_run("pengu-mesh")
                .expect("latest active pengu-mesh run")
                .expect("pengu-mesh run")
                .id,
            third.id
        );

        let (identity_one, reused_one) = store
            .get_or_create_runtime_identity("pengu-mesh-daemon")
            .expect("create identity");
        let (identity_two, reused_two) = store
            .get_or_create_runtime_identity("pengu-mesh-daemon")
            .expect("reuse identity");
        assert!(!reused_one);
        assert!(reused_two);
        assert_eq!(identity_one.operator_id, identity_two.operator_id);
        assert_eq!(identity_one.entrypoint, "pengu-mesh-daemon");
    }

    #[test]
    fn persists_scenario_metrics_round_trip() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");
        let run = ScenarioRun {
            id: "scenario_run_startup".into(),
            scenario_name: "startup-readiness".into(),
            scenario_family: "startup-readiness".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: Some(tempdir.path().join("runtime-root").display().to_string()),
            commit_sha: Some("29e4808".into()),
            branch_name: Some("main".into()),
            platform: "darwin".into(),
            started_at: "2026-03-12T10:00:00Z".into(),
            finished_at: Some("2026-03-12T10:00:30Z".into()),
            status: "passed".into(),
            summary_path: Some(tempdir.path().join("summary.md").display().to_string()),
        };
        let steps = vec![
            ScenarioStep {
                id: "scenario_step_health".into(),
                run_id: run.id.clone(),
                ordinal: 1,
                step_name: "health".into(),
                step_kind: "command".into(),
                command_line: Some("pengu-mesh health".into()),
                started_at: "2026-03-12T10:00:01Z".into(),
                finished_at: Some("2026-03-12T10:00:02Z".into()),
                status: "passed".into(),
                error_code: None,
            },
            ScenarioStep {
                id: "scenario_step_diagnose".into(),
                run_id: run.id.clone(),
                ordinal: 2,
                step_name: "diagnose".into(),
                step_kind: "command".into(),
                command_line: Some("pengu-mesh diagnose".into()),
                started_at: "2026-03-12T10:00:03Z".into(),
                finished_at: Some("2026-03-12T10:00:05Z".into()),
                status: "passed".into(),
                error_code: None,
            },
        ];
        let assertions = vec![
            ScenarioAssertion {
                id: "scenario_assertion_health_ok".into(),
                run_id: run.id.clone(),
                step_id: Some(steps[0].id.clone()),
                assertion_name: "health ok".into(),
                expected_value: Some("true".into()),
                actual_value: Some("true".into()),
                status: "passed".into(),
                failure_category: None,
                notes: Some("health returned ok".into()),
            },
            ScenarioAssertion {
                id: "scenario_assertion_diagnose_state".into(),
                run_id: run.id.clone(),
                step_id: Some(steps[1].id.clone()),
                assertion_name: "diagnose ready or degraded".into(),
                expected_value: Some("ready|degraded".into()),
                actual_value: Some("ready".into()),
                status: "passed".into(),
                failure_category: None,
                notes: None,
            },
        ];
        let samples = vec![
            LatencySample {
                id: "scenario_latency_health".into(),
                run_id: run.id.clone(),
                step_id: Some(steps[0].id.clone()),
                metric_name: "health".into(),
                sample_ms: 12.5.into(),
                capture_method: Some("wall_clock".into()),
            },
            LatencySample {
                id: "scenario_latency_diagnose".into(),
                run_id: run.id.clone(),
                step_id: Some(steps[1].id.clone()),
                metric_name: "diagnose".into(),
                sample_ms: 18.75.into(),
                capture_method: Some("wall_clock".into()),
            },
        ];
        let fingerprint = EnvironmentFingerprint {
            id: "scenario_env_startup".into(),
            run_id: run.id.clone(),
            platform: "darwin".into(),
            arch: "arm64".into(),
            os_version: Some("Darwin 25.0.0".into()),
            rust_version: Some("rustc 1.94.0".into()),
            cargo_version: Some("cargo 1.94.0".into()),
            chrome_channel: Some("chrome-dev".into()),
            chrome_version: Some("136.0.0.0".into()),
        };

        store
            .insert_scenario_run(&run)
            .expect("insert scenario run");
        for step in &steps {
            store
                .insert_scenario_step(step)
                .expect("insert scenario step");
        }
        for assertion in &assertions {
            store
                .insert_scenario_assertion(assertion)
                .expect("insert scenario assertion");
        }
        for sample in &samples {
            store
                .insert_latency_sample(sample)
                .expect("insert latency sample");
        }
        store
            .insert_environment_fingerprint(&fingerprint)
            .expect("insert environment fingerprint");

        assert_eq!(
            store
                .list_scenario_runs(Some("startup-readiness"), 10)
                .expect("scenario runs"),
            vec![run.clone()]
        );
        assert_eq!(
            store.list_scenario_steps(&run.id).expect("scenario steps"),
            steps
        );
        assert_eq!(
            store
                .list_scenario_assertions(&run.id)
                .expect("scenario assertions"),
            assertions
        );
        assert_eq!(
            store
                .list_latency_samples(&run.id)
                .expect("latency samples"),
            samples
        );
        assert_eq!(
            store
                .get_environment_fingerprint(&run.id)
                .expect("environment fingerprint"),
            Some(fingerprint)
        );
    }

    #[test]
    fn filters_and_limits_scenario_runs() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");
        let runs = vec![
            ScenarioRun {
                id: "scenario_run_alpha".into(),
                scenario_name: "alpha".into(),
                scenario_family: "startup-readiness".into(),
                scenario_version: "v1".into(),
                tool_surface: "cli".into(),
                runtime_root: None,
                commit_sha: None,
                branch_name: None,
                platform: "darwin".into(),
                started_at: "2026-03-12T09:00:00Z".into(),
                finished_at: None,
                status: "running".into(),
                summary_path: None,
            },
            ScenarioRun {
                id: "scenario_run_beta".into(),
                scenario_name: "beta".into(),
                scenario_family: "evidence-chain".into(),
                scenario_version: "v1".into(),
                tool_surface: "cli".into(),
                runtime_root: None,
                commit_sha: None,
                branch_name: None,
                platform: "darwin".into(),
                started_at: "2026-03-12T10:00:00Z".into(),
                finished_at: None,
                status: "running".into(),
                summary_path: None,
            },
            ScenarioRun {
                id: "scenario_run_gamma".into(),
                scenario_name: "gamma".into(),
                scenario_family: "startup-readiness".into(),
                scenario_version: "v2".into(),
                tool_surface: "cli".into(),
                runtime_root: None,
                commit_sha: None,
                branch_name: None,
                platform: "darwin".into(),
                started_at: "2026-03-12T11:00:00Z".into(),
                finished_at: None,
                status: "running".into(),
                summary_path: None,
            },
        ];

        for run in &runs {
            store.insert_scenario_run(run).expect("insert scenario run");
        }

        assert_eq!(
            store
                .list_scenario_runs(Some("startup-readiness"), 10)
                .expect("filtered scenario runs"),
            vec![runs[2].clone(), runs[0].clone()]
        );
        assert_eq!(
            store
                .list_scenario_runs(None, 2)
                .expect("limited scenario runs"),
            vec![runs[2].clone(), runs[1].clone()]
        );
    }

    #[test]
    fn preserves_insertion_order_for_assertions_and_latency_within_a_step() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        let run = ScenarioRun {
            id: "scenario_run_ordering".into(),
            scenario_name: "ordering".into(),
            scenario_family: "ordering".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: None,
            commit_sha: None,
            branch_name: None,
            platform: "darwin".into(),
            started_at: "2026-03-12T10:00:00Z".into(),
            finished_at: None,
            status: "running".into(),
            summary_path: None,
        };
        store.insert_scenario_run(&run).expect("scenario run");

        let step = ScenarioStep {
            id: "scenario_step_ordering".into(),
            run_id: run.id.clone(),
            ordinal: 1,
            step_name: "ordering".into(),
            step_kind: "command".into(),
            command_line: None,
            started_at: "2026-03-12T10:00:01Z".into(),
            finished_at: Some("2026-03-12T10:00:02Z".into()),
            status: "passed".into(),
            error_code: None,
        };
        store.insert_scenario_step(&step).expect("scenario step");

        let first_assertion = ScenarioAssertion {
            id: "scenario_assertion_second_name_first_insert".into(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            assertion_name: "z-last".into(),
            expected_value: Some("true".into()),
            actual_value: Some("true".into()),
            status: "passed".into(),
            failure_category: None,
            notes: None,
        };
        let second_assertion = ScenarioAssertion {
            id: "scenario_assertion_first_name_second_insert".into(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            assertion_name: "a-first".into(),
            expected_value: Some("true".into()),
            actual_value: Some("true".into()),
            status: "passed".into(),
            failure_category: None,
            notes: None,
        };
        store
            .insert_scenario_assertion(&first_assertion)
            .expect("first assertion");
        store
            .insert_scenario_assertion(&second_assertion)
            .expect("second assertion");

        let first_sample = LatencySample {
            id: "scenario_latency_second_name_first_insert".into(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            metric_name: "z-last".into(),
            sample_ms: 9.0.into(),
            capture_method: Some("wall_clock".into()),
        };
        let second_sample = LatencySample {
            id: "scenario_latency_first_name_second_insert".into(),
            run_id: run.id.clone(),
            step_id: Some(step.id.clone()),
            metric_name: "a-first".into(),
            sample_ms: 11.0.into(),
            capture_method: Some("wall_clock".into()),
        };
        store
            .insert_latency_sample(&first_sample)
            .expect("first sample");
        store
            .insert_latency_sample(&second_sample)
            .expect("second sample");

        assert_eq!(
            store
                .list_scenario_assertions(&run.id)
                .expect("scenario assertions"),
            vec![first_assertion, second_assertion]
        );
        assert_eq!(
            store
                .list_latency_samples(&run.id)
                .expect("latency samples"),
            vec![first_sample, second_sample]
        );
    }

    #[test]
    fn concurrent_scenario_step_creation_preserves_unique_ordinals() {
        let tempdir = tempdir().expect("tempdir");
        let store = Arc::new(StateStore::new(tempdir.path()).expect("state store"));

        let run = ScenarioRun {
            id: "scenario_run_concurrent_steps".into(),
            scenario_name: "concurrent-steps".into(),
            scenario_family: "concurrency".into(),
            scenario_version: "v1".into(),
            tool_surface: "cli".into(),
            runtime_root: None,
            commit_sha: None,
            branch_name: None,
            platform: "darwin".into(),
            started_at: "2026-03-12T10:00:00Z".into(),
            finished_at: None,
            status: "running".into(),
            summary_path: None,
        };
        store.insert_scenario_run(&run).expect("scenario run");

        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for index in 0..8 {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let run_id = run.id.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                store
                    .create_scenario_step(
                        &format!("scenario_step_concurrent_{index}"),
                        &run_id,
                        &format!("step-{index}"),
                        "command",
                        Some("pengu-mesh health"),
                        "2026-03-12T10:00:01Z",
                    )
                    .expect("create concurrent step")
                    .ordinal
            }));
        }

        let mut ordinals = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread join"))
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        assert_eq!(ordinals, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        let stored_steps = store
            .list_scenario_steps(&run.id)
            .expect("stored scenario steps");
        let stored_ordinals = stored_steps
            .iter()
            .map(|step| step.ordinal)
            .collect::<Vec<_>>();
        assert_eq!(stored_ordinals, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn lists_only_active_leases_for_requested_holder() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");
        let now = OffsetDateTime::now_utc();
        let granted_at = now
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format granted_at");
        let expires_at = (now + time::Duration::minutes(5))
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format expires_at");
        let daemon_writer = LeaseRecord {
            id: "lease_daemon_writer".into(),
            resource_kind: LeaseResourceKind::Instance,
            resource_id: "inst_alpha".into(),
            holder_id: "pengu-mesh-daemon:operator".into(),
            holder_label: Some("daemon".into()),
            mode: LeaseMode::Writer,
            granted_at: granted_at.clone(),
            expires_at: expires_at.clone(),
            last_heartbeat_at: granted_at.clone(),
        };
        let daemon_observer = LeaseRecord {
            id: "lease_daemon_observer".into(),
            resource_kind: LeaseResourceKind::Instance,
            resource_id: "inst_beta".into(),
            holder_id: "pengu-mesh-daemon:operator".into(),
            holder_label: Some("daemon".into()),
            mode: LeaseMode::Observer,
            granted_at: granted_at.clone(),
            expires_at: expires_at.clone(),
            last_heartbeat_at: granted_at.clone(),
        };
        let peer_writer = LeaseRecord {
            id: "lease_peer_writer".into(),
            resource_kind: LeaseResourceKind::Instance,
            resource_id: "inst_gamma".into(),
            holder_id: "peer-agent".into(),
            holder_label: Some("peer".into()),
            mode: LeaseMode::Writer,
            granted_at: granted_at.clone(),
            expires_at,
            last_heartbeat_at: granted_at,
        };

        store
            .upsert_lease(&daemon_writer)
            .expect("upsert daemon writer");
        store
            .upsert_lease(&daemon_observer)
            .expect("upsert daemon observer");
        store
            .upsert_lease(&peer_writer)
            .expect("upsert peer writer");

        let leases = store
            .list_active_leases_for_holder("pengu-mesh-daemon:operator", &daemon_writer.granted_at)
            .expect("holder leases");
        assert_eq!(leases.len(), 2);
        assert!(
            leases
                .iter()
                .all(|lease| lease.holder_id == "pengu-mesh-daemon:operator")
        );
        assert!(leases.iter().any(|lease| lease.resource_id == "inst_alpha"));
        assert!(leases.iter().any(|lease| lease.resource_id == "inst_beta"));
    }

    #[test]
    fn task_insert_query_and_update() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        store
            .insert_task(&TaskRecord {
                id: "task_001".into(),
                agent_id: "agent_alpha".into(),
                action: "screenshot".into(),
                state: TaskState::Pending,
                priority: TaskPriority::High,
                params: Some(json!({
                    "url": "https://example.com"
                })),
                created_at: "2026-03-12T10:00:00Z".into(),
                started_at: None,
                completed_at: None,
                latency_ms: None,
            })
            .expect("insert task");

        store
            .insert_task(&TaskRecord {
                id: "task_002".into(),
                agent_id: "agent_alpha".into(),
                action: "navigate".into(),
                state: TaskState::Pending,
                priority: TaskPriority::Normal,
                params: None,
                created_at: "2026-03-12T10:01:00Z".into(),
                started_at: None,
                completed_at: None,
                latency_ms: None,
            })
            .expect("insert second task");

        store
            .insert_task(&TaskRecord {
                id: "task_003".into(),
                agent_id: "agent_beta".into(),
                action: "click".into(),
                state: TaskState::Pending,
                priority: TaskPriority::Low,
                params: None,
                created_at: "2026-03-12T10:02:00Z".into(),
                started_at: None,
                completed_at: None,
                latency_ms: None,
            })
            .expect("insert third task");

        // query single task
        let task = store
            .query_task("task_001")
            .expect("query task")
            .expect("task exists");
        assert_eq!(task.id, "task_001");
        assert_eq!(task.agent_id, "agent_alpha");
        assert_eq!(task.action, "screenshot");
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.params, Some(json!({ "url": "https://example.com" })));
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());
        assert!(task.latency_ms.is_none());

        // query missing task
        let missing = store.query_task("nonexistent").expect("query missing");
        assert!(missing.is_none());

        // query by agent_id
        let alpha_tasks = store
            .query_tasks(Some("agent_alpha"), None, 10)
            .expect("query agent tasks");
        assert_eq!(alpha_tasks.len(), 2);

        // query by state
        let pending = store
            .query_tasks(None, Some(TaskState::Pending), 10)
            .expect("query pending tasks");
        assert_eq!(pending.len(), 3);

        // query by agent + state
        let alpha_pending = store
            .query_tasks(Some("agent_alpha"), Some(TaskState::Pending), 10)
            .expect("query agent pending");
        assert_eq!(alpha_pending.len(), 2);

        // query all with limit
        let limited = store
            .query_tasks(None, None, 2)
            .expect("query limited tasks");
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].id, "task_003");
        assert_eq!(limited[1].id, "task_002");

        // update task state to running
        store
            .update_task_state(
                "task_001",
                TaskState::Running,
                Some("2026-03-12T10:00:05Z"),
                None,
                None,
            )
            .expect("update to running");

        let running = store
            .query_task("task_001")
            .expect("query running")
            .expect("task exists");
        assert_eq!(running.state, TaskState::Running);
        assert_eq!(running.started_at.as_deref(), Some("2026-03-12T10:00:05Z"));
        assert!(running.completed_at.is_none());

        // update task state to completed
        store
            .update_task_state(
                "task_001",
                TaskState::Completed,
                None,
                Some("2026-03-12T10:00:10Z"),
                Some(5000),
            )
            .expect("update to completed");

        let completed = store
            .query_task("task_001")
            .expect("query completed")
            .expect("task exists");
        assert_eq!(completed.state, TaskState::Completed);
        assert_eq!(
            completed.started_at.as_deref(),
            Some("2026-03-12T10:00:05Z")
        );
        assert_eq!(
            completed.completed_at.as_deref(),
            Some("2026-03-12T10:00:10Z")
        );
        assert_eq!(completed.latency_ms, Some(5000));

        // query by state filters correctly after update
        let still_pending = store
            .query_tasks(None, Some(TaskState::Pending), 10)
            .expect("query pending after update");
        assert_eq!(still_pending.len(), 2);

        let completed_tasks = store
            .query_tasks(None, Some(TaskState::Completed), 10)
            .expect("query completed tasks");
        assert_eq!(completed_tasks.len(), 1);
        assert_eq!(completed_tasks[0].id, "task_001");

        let missing_update =
            store.update_task_state("task_404", TaskState::Running, None, None, None);
        assert!(missing_update.is_err());
    }

    #[test]
    fn increment_and_query_scheduler_metrics() {
        use super::{METRIC_TASKS_COMPLETED, METRIC_TASKS_FAILED, METRIC_TASKS_SUBMITTED};

        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        store
            .increment_scheduler_metric(
                METRIC_TASKS_SUBMITTED,
                Some("agent-1"),
                "2026-03-12T00:00:00Z",
            )
            .expect("increment submitted");
        store
            .increment_scheduler_metric(
                METRIC_TASKS_SUBMITTED,
                Some("agent-1"),
                "2026-03-12T00:01:00Z",
            )
            .expect("increment submitted again");
        store
            .increment_scheduler_metric(
                METRIC_TASKS_COMPLETED,
                Some("agent-1"),
                "2026-03-12T00:02:00Z",
            )
            .expect("increment completed");
        store
            .increment_scheduler_metric(METRIC_TASKS_FAILED, None, "2026-03-12T00:03:00Z")
            .expect("increment failed with no agent");

        // Query all
        let all = store
            .query_scheduler_metrics(None, None)
            .expect("all metrics");
        assert_eq!(all.len(), 3);

        // Query by metric name
        let submitted = store
            .query_scheduler_metrics(Some(METRIC_TASKS_SUBMITTED), None)
            .expect("submitted metrics");
        assert_eq!(submitted.len(), 1);
        assert_eq!(submitted[0].counter, 2);
        assert_eq!(submitted[0].last_updated, "2026-03-12T00:01:00Z");

        // Query by agent
        let agent_metrics = store
            .query_scheduler_metrics(None, Some("agent-1"))
            .expect("agent metrics");
        assert_eq!(agent_metrics.len(), 2);

        // Query by both
        let specific = store
            .query_scheduler_metrics(Some(METRIC_TASKS_COMPLETED), Some("agent-1"))
            .expect("specific metric");
        assert_eq!(specific.len(), 1);
        assert_eq!(specific[0].counter, 1);
    }

    #[test]
    fn reset_scheduler_metrics_clears_all_rows() {
        use super::METRIC_TASKS_SUBMITTED;

        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        store
            .increment_scheduler_metric(
                METRIC_TASKS_SUBMITTED,
                Some("agent-1"),
                "2026-03-12T00:00:00Z",
            )
            .expect("increment");

        let before = store
            .query_scheduler_metrics(None, None)
            .expect("before reset");
        assert_eq!(before.len(), 1);

        store.reset_scheduler_metrics().expect("reset");

        let after = store
            .query_scheduler_metrics(None, None)
            .expect("after reset");
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn inspect_existing_stays_read_only_when_state_is_absent() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::inspect_existing(tempdir.path()).expect("inspect state");
        assert!(store.is_none());
        assert!(!tempdir.path().join("runtime.sqlite3").exists());
        assert!(!tempdir.path().join("artifacts").exists());
        assert!(!tempdir.path().join("profiles").exists());
        assert!(!tempdir.path().join("replay").exists());
    }
}
