use serde_json::{Value, json};

use pengu_mesh_shared::OperationOutcome;

pub fn sample_outcome() -> OperationOutcome<Value> {
    OperationOutcome::success(
        "stage1 json envelope",
        json!({
            "instance_id": "inst_managed_chrome_dev_9222",
            "tab_id": "tab_demo",
            "url": "file:///tmp/stage1-smoke.html",
            "title": "Stage 1 Smoke",
            "nodes": [
                {"ref":"e0","role":"link","name":"More info"},
                {"ref":"e1","role":"button","name":"Ship it"}
            ],
            "artifacts": {
                "screenshot": "/tmp/pengu-mesh-runtime/artifacts/screenshots/shot.png",
                "pdf": "/tmp/pengu-mesh-runtime/artifacts/pdfs/shot.pdf"
            }
        }),
    )
}

pub fn serialize_len() -> usize {
    serde_json::to_vec(&sample_outcome())
        .expect("serialize outcome")
        .len()
}
