# Live Web

This family proves that pengu mesh can leave the data-URL lab, fetch a real
public page over the network, and still produce durable browser artifacts plus
scenario metrics that stay truthful under live conditions.

`run.sh` launches managed headless Chrome Dev under an isolated runtime root,
navigates to `https://example.com`, and records latencies and assertions for:

- browser startup against the local managed channel
- live URL navigation with real DNS/TLS-backed page fetch
- snapshot, screenshot, and text capture on remote content
- artifact inventory and checksum verification for every captured artifact
- clean browser shutdown after evidence capture

Artifacts and `summary.md` stay inside the provided output directory. The named
scenario run, steps, assertions, and latency samples are persisted in the
runtime SQLite database under the isolated runtime root.
