use pengu_mesh_bench_cdp::parse_target_count;

fn main() {
    divan::main();
}

#[divan::bench]
fn devtools_target_parse() -> usize {
    parse_target_count()
}
