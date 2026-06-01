use pengu_mesh_cdp::DebugTarget;

const SAMPLE_TARGETS: &str = r#"
[
  {
    "id": "A12",
    "title": "Stage 1 Smoke",
    "url": "file:///tmp/stage1-smoke.html",
    "type": "page",
    "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/A12"
  },
  {
    "id": "B34",
    "title": "Example",
    "url": "https://example.com",
    "type": "page",
    "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/B34"
  }
]
"#;

pub fn parse_target_count() -> usize {
    serde_json::from_str::<Vec<DebugTarget>>(SAMPLE_TARGETS)
        .expect("parse targets")
        .len()
}
