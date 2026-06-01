use pengu_mesh_shared::{
    ArtifactKind, ArtifactProvenance, BrowserChannel, BrowserInstance, CaptureRun, EventLevel,
    EventTailPayload, InstanceMode, InstanceStatus, ReplayArtifactRecord, ReplayBundleMetadata,
    ReplayExportMode, ReplayManifest, RunStatus, RuntimeEvent, inspection_modes,
};
use serde_json::json;

pub fn state_payload_size() -> usize {
    let payload = vec![BrowserInstance {
        id: "inst_managed_chrome_dev_9222".into(),
        name: "stage1".into(),
        channel: BrowserChannel::ChromeDev,
        mode: InstanceMode::Managed,
        status: InstanceStatus::Running,
        debug_http_url: "http://127.0.0.1:9222".into(),
        browser_ws_url: Some("ws://127.0.0.1:9222/devtools/browser/1".into()),
        profile_id: Some("prof_chrome_dev".into()),
        profile_path: Some("/tmp/pengu-mesh-runtime/profiles/chrome-dev".into()),
        pid: Some(1234),
        last_error: None,
        created_at: "2026-03-11T12:00:00Z".into(),
        updated_at: "2026-03-11T12:00:01Z".into(),
    }];
    serde_json::to_vec(&payload)
        .expect("serialize browser instances")
        .len()
}

pub fn event_tail_payload_size() -> usize {
    let payload = sample_event_tail_payload();
    serde_json::to_vec(&payload)
        .expect("serialize event tail payload")
        .len()
}

pub fn manifest_only_replay_manifest_size() -> usize {
    let payload = sample_replay_manifest(ReplayExportMode::ManifestOnly);
    serde_json::to_vec(&payload)
        .expect("serialize manifest only replay manifest")
        .len()
}

pub fn portable_replay_manifest_size() -> usize {
    let payload = sample_replay_manifest(ReplayExportMode::Portable);
    serde_json::to_vec(&payload)
        .expect("serialize replay manifest")
        .len()
}

fn sample_event_tail_payload() -> EventTailPayload {
    EventTailPayload {
        run: Some(sample_run()),
        requested_limit: 25,
        events: sample_events(),
    }
}

fn sample_replay_manifest(mode: ReplayExportMode) -> ReplayManifest {
    ReplayManifest {
        schema_version: 2,
        exported_at: "2026-03-11T13:20:30Z".into(),
        mode: mode.clone(),
        bundle: ReplayBundleMetadata {
            root_path: match mode {
                ReplayExportMode::ManifestOnly => {
                    "/tmp/pengu-mesh-runtime/replay/run_pengu_mesh_95594_1773234049094139000/manifest_only"
                        .into()
                }
                ReplayExportMode::Portable => {
                    "/tmp/pengu-mesh-runtime/replay/run_pengu_mesh_95594_1773234049094139000/portable".into()
                }
            },
            manifest_path: match mode {
                ReplayExportMode::ManifestOnly => "/tmp/pengu-mesh-runtime/replay/run_pengu_mesh_95594_1773234049094139000/manifest_only/manifest.json".into(),
                ReplayExportMode::Portable => "/tmp/pengu-mesh-runtime/replay/run_pengu_mesh_95594_1773234049094139000/portable/manifest.json".into(),
            },
            artifact_root: match mode {
                ReplayExportMode::ManifestOnly => None,
                ReplayExportMode::Portable => Some(
                    "/tmp/pengu-mesh-runtime/replay/run_pengu_mesh_95594_1773234049094139000/portable/artifacts"
                        .into(),
                ),
            },
            staged_atomically: matches!(mode, ReplayExportMode::Portable),
        },
        run: sample_run(),
        inspection_modes: inspection_modes(),
        events: sample_events(),
        artifacts: sample_artifacts(mode),
    }
}

fn sample_run() -> CaptureRun {
    CaptureRun {
        id: "run_pengu_mesh_95594_1773234049094139000".into(),
        entrypoint: "pengu-mesh-mcp".into(),
        detail: "capture recording active".into(),
        status: RunStatus::Completed,
        started_at: "2026-03-11T13:20:00Z".into(),
        ended_at: Some("2026-03-11T13:20:30Z".into()),
    }
}

fn sample_events() -> Vec<RuntimeEvent> {
    vec![
        RuntimeEvent {
            schema_version: 1,
            id: "event_bootstrap".into(),
            run_id: "run_pengu_mesh_95594_1773234049094139000".into(),
            sequence: 1,
            category: "runtime".into(),
            action: "bootstrap".into(),
            level: EventLevel::Info,
            message: "pengu-mesh-mcp runtime ready".into(),
            instance_id: None,
            tab_id: None,
            artifact_id: None,
            data: json!({
                "entrypoint": "pengu-mesh-mcp",
                "runtime_root": "/tmp/pengu-mesh-runtime"
            }),
            timestamp: "2026-03-11T13:20:00Z".into(),
        },
        RuntimeEvent {
            schema_version: 1,
            id: "event_tab_snapshot".into(),
            run_id: "run_pengu_mesh_95594_1773234049094139000".into(),
            sequence: 2,
            category: "tab".into(),
            action: "snapshot".into(),
            level: EventLevel::Info,
            message: "captured accessibility snapshot".into(),
            instance_id: Some("inst_demo".into()),
            tab_id: Some("tab_demo".into()),
            artifact_id: Some("artifact_snapshot".into()),
            data: json!({
                "artifact_id": "artifact_snapshot",
                "artifact_kind": "snapshot",
                "artifact_path": "/tmp/pengu-mesh-runtime/artifacts/snapshots/artifact_snapshot.json",
                "mime_type": "application/json",
                "bytes": 8192,
                "run_id": "run_pengu_mesh_95594_1773234049094139000",
                "node_count": 128
            }),
            timestamp: "2026-03-11T13:20:10Z".into(),
        },
        RuntimeEvent {
            schema_version: 1,
            id: "event_replay_export".into(),
            run_id: "run_pengu_mesh_95594_1773234049094139000".into(),
            sequence: 3,
            category: "artifact".into(),
            action: "crop_grid".into(),
            level: EventLevel::Info,
            message: "created 4 derived grid crops".into(),
            instance_id: Some("inst_demo".into()),
            tab_id: Some("tab_demo".into()),
            artifact_id: None,
            data: json!({
                "source_artifact_id": "artifact_screenshot",
                "rows": 2,
                "cols": 2,
                "overlap": 25,
                "derived_count": 4,
                "derived_artifact_ids": ["artifact_crop_1", "artifact_crop_2", "artifact_crop_3", "artifact_crop_4"]
            }),
            timestamp: "2026-03-11T13:20:18Z".into(),
        },
        RuntimeEvent {
            schema_version: 1,
            id: "event_trace_capture".into(),
            run_id: "run_pengu_mesh_95594_1773234049094139000".into(),
            sequence: 4,
            category: "tab".into(),
            action: "trace_capture".into(),
            level: EventLevel::Info,
            message: "captured trace artifact".into(),
            instance_id: Some("inst_demo".into()),
            tab_id: Some("tab_demo".into()),
            artifact_id: Some("artifact_trace".into()),
            data: json!({
                "artifact_id": "artifact_trace",
                "artifact_kind": "trace",
                "artifact_path": "/tmp/pengu-mesh-runtime/artifacts/traces/artifact_trace.json",
                "mime_type": "application/json",
                "bytes": 32768,
                "run_id": "run_pengu_mesh_95594_1773234049094139000",
                "duration_ms": 2000,
                "categories": ["devtools.timeline", "disabled-by-default-devtools.screenshot", "toplevel"]
            }),
            timestamp: "2026-03-11T13:20:24Z".into(),
        },
        RuntimeEvent {
            schema_version: 1,
            id: "event_replay_export".into(),
            run_id: "run_pengu_mesh_95594_1773234049094139000".into(),
            sequence: 5,
            category: "replay".into(),
            action: "export".into(),
            level: EventLevel::Info,
            message: "exported replay manifest".into(),
            instance_id: Some("inst_demo".into()),
            tab_id: Some("tab_demo".into()),
            artifact_id: None,
            data: json!({
                "manifest_path": "/tmp/pengu-mesh-runtime/replay/run_pengu_mesh_95594_1773234049094139000/manifest.json",
                "event_count": 3,
                "artifact_count": 2
            }),
            timestamp: "2026-03-11T13:20:30Z".into(),
        },
    ]
}

fn sample_artifacts(mode: ReplayExportMode) -> Vec<ReplayArtifactRecord> {
    vec![
        ReplayArtifactRecord {
            artifact_id: "artifact_snapshot".into(),
            run_id: Some("run_pengu_mesh_95594_1773234049094139000".into()),
            instance_id: "inst_demo".into(),
            tab_id: "tab_demo".into(),
            kind: ArtifactKind::Snapshot,
            path: match mode {
                ReplayExportMode::ManifestOnly => {
                    "/tmp/pengu-mesh-runtime/artifacts/snapshots/artifact_snapshot.json".into()
                }
                ReplayExportMode::Portable => "artifacts/snapshots/artifact_snapshot.json".into(),
            },
            mime_type: "application/json".into(),
            bytes: 8192,
            created_at: "2026-03-11T13:20:10Z".into(),
            materialized: matches!(mode, ReplayExportMode::Portable),
            checksum_sha256: Some(
                "0f5e6f97ea4eb7834c712f7ef8d4533ad9a356d7a8b2e6d889581f4c8a91b3f1".into(),
            ),
            provenance: ArtifactProvenance::primary(),
        },
        ReplayArtifactRecord {
            artifact_id: "artifact_screenshot".into(),
            run_id: Some("run_pengu_mesh_95594_1773234049094139000".into()),
            instance_id: "inst_demo".into(),
            tab_id: "tab_demo".into(),
            kind: ArtifactKind::Screenshot,
            path: match mode {
                ReplayExportMode::ManifestOnly => {
                    "/tmp/pengu-mesh-runtime/artifacts/screenshots/artifact_screenshot.png".into()
                }
                ReplayExportMode::Portable => {
                    "artifacts/screenshots/artifact_screenshot.png".into()
                }
            },
            mime_type: "image/png".into(),
            bytes: 65536,
            created_at: "2026-03-11T13:20:12Z".into(),
            materialized: matches!(mode, ReplayExportMode::Portable),
            checksum_sha256: Some(
                "7f4a3fca6f0ac8f2c658ff5502cb1581f17c94d6b8fe0f7a6ad7d0e31e4d4d88".into(),
            ),
            provenance: ArtifactProvenance::primary(),
        },
        ReplayArtifactRecord {
            artifact_id: "artifact_pdf".into(),
            run_id: Some("run_pengu_mesh_95594_1773234049094139000".into()),
            instance_id: "inst_demo".into(),
            tab_id: "tab_demo".into(),
            kind: ArtifactKind::Pdf,
            path: match mode {
                ReplayExportMode::ManifestOnly => {
                    "/tmp/pengu-mesh-runtime/artifacts/pdfs/artifact_pdf.pdf".into()
                }
                ReplayExportMode::Portable => "artifacts/pdfs/artifact_pdf.pdf".into(),
            },
            mime_type: "application/pdf".into(),
            bytes: 245760,
            created_at: "2026-03-11T13:20:22Z".into(),
            materialized: matches!(mode, ReplayExportMode::Portable),
            checksum_sha256: Some(
                "f599cba69f68c3fefca40eb70f393d34e2b70c5006fdb64423ab4aeb00cf34fb".into(),
            ),
            provenance: ArtifactProvenance {
                source_artifact_id: Some("artifact_screenshot".into()),
                crop_region: Some(pengu_mesh_shared::NormalizedRegion {
                    x_min: 100,
                    y_min: 100,
                    x_max: 900,
                    y_max: 900,
                }),
                page_index: Some(0),
            },
        },
        ReplayArtifactRecord {
            artifact_id: "artifact_trace".into(),
            run_id: Some("run_pengu_mesh_95594_1773234049094139000".into()),
            instance_id: "inst_demo".into(),
            tab_id: "tab_demo".into(),
            kind: ArtifactKind::Trace,
            path: match mode {
                ReplayExportMode::ManifestOnly => {
                    "/tmp/pengu-mesh-runtime/artifacts/traces/artifact_trace.json".into()
                }
                ReplayExportMode::Portable => "artifacts/traces/artifact_trace.json".into(),
            },
            mime_type: "application/json".into(),
            bytes: 32768,
            created_at: "2026-03-11T13:20:24Z".into(),
            materialized: matches!(mode, ReplayExportMode::Portable),
            checksum_sha256: Some(
                "5181139fc733fe4d5d630a7a8462ee0c84a9f7f4bbfe0d1f2a9b0fc5c4cb26e6".into(),
            ),
            provenance: ArtifactProvenance::primary(),
        },
        ReplayArtifactRecord {
            artifact_id: "artifact_recording".into(),
            run_id: Some("run_pengu_mesh_95594_1773234049094139000".into()),
            instance_id: "inst_demo".into(),
            tab_id: "tab_demo".into(),
            kind: ArtifactKind::Recording,
            path: match mode {
                ReplayExportMode::ManifestOnly => {
                    "/tmp/pengu-mesh-runtime/artifacts/recordings/artifact_recording.tar".into()
                }
                ReplayExportMode::Portable => "artifacts/recordings/artifact_recording.tar".into(),
            },
            mime_type: "application/x-tar".into(),
            bytes: 131072,
            created_at: "2026-03-11T13:20:28Z".into(),
            materialized: matches!(mode, ReplayExportMode::Portable),
            checksum_sha256: Some(
                "ef6eb4fca8d0d1fd0c65e6063ab774d34f8495b0e4ecbcb9b380178d8b57090a".into(),
            ),
            provenance: ArtifactProvenance::primary(),
        },
    ]
}
