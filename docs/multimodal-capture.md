# Multimodal Capture

The artifact model will cover:

- screenshots
- element screenshots
- PDFs
- derived screenshot crops and deterministic crop grids from screenshots and
  PDFs
- snapshots
- traces
- recordings
- replay bundles

All large artifacts should stream to disk and resolve through lightweight
handles in agent-facing surfaces.

Inspection work should escalate in this order:

- `quick_read`: text and event-tail first
- `faithful_extract`: preserve layout with snapshot/screenshot/PDF capture
- `compositional_inspect`: add richer evidence before higher-cost reasoning,
  favoring deterministic `artifact_crop_grid` batches over whole-page reruns
- `multi_pass_inspect`: derive narrow crops, capture bounded traces or
  recordings when motion/timeline evidence matters, and export portable bundles
  for handoff or repeatable reruns
