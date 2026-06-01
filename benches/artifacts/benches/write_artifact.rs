use pengu_mesh_bench_artifacts::{
    checksum_sample_artifact, crop_grid_sample_artifact, crop_sample_artifact,
    materialize_sample_artifact, write_recording_archive_sample, write_sample_artifact,
};

fn main() {
    divan::main();
}

#[divan::bench]
fn artifact_write_path() -> usize {
    write_sample_artifact()
}

#[divan::bench]
fn artifact_crop_path() -> usize {
    crop_sample_artifact()
}

#[divan::bench]
fn artifact_crop_grid_path() -> usize {
    crop_grid_sample_artifact()
}

#[divan::bench]
fn artifact_recording_archive_path() -> usize {
    write_recording_archive_sample()
}

#[divan::bench]
fn artifact_materialize_path() -> usize {
    materialize_sample_artifact()
}

#[divan::bench]
fn artifact_checksum_path() -> usize {
    checksum_sample_artifact()
}
