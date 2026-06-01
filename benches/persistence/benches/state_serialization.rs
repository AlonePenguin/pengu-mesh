use pengu_mesh_bench_persistence::{
    event_tail_payload_size, manifest_only_replay_manifest_size, portable_replay_manifest_size,
    state_payload_size,
};

fn main() {
    divan::main();
}

#[divan::bench]
fn runtime_state_serialization() -> usize {
    state_payload_size()
}

#[divan::bench]
fn event_tail_payload_serialization() -> usize {
    event_tail_payload_size()
}

#[divan::bench]
fn manifest_only_replay_manifest_serialization() -> usize {
    manifest_only_replay_manifest_size()
}

#[divan::bench]
fn portable_replay_manifest_serialization() -> usize {
    portable_replay_manifest_size()
}
