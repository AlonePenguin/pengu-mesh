use pengu_mesh_bench_json::serialize_len;

fn main() {
    divan::main();
}

#[divan::bench]
fn response_envelope_serialization() -> usize {
    serialize_len()
}
