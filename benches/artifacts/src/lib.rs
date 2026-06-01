use image::{DynamicImage, Rgba, RgbaImage};
use pengu_mesh_artifacts::ArtifactStore;
use pengu_mesh_shared::{ArtifactKind, NormalizedRegion};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

pub fn write_sample_artifact() -> usize {
    let tempdir = tempdir().expect("tempdir");
    let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
    let handle = store
        .write_bytes(
            ArtifactKind::Screenshot,
            Some("run_demo"),
            "inst_demo",
            "tab_demo",
            &[0_u8; 4096],
        )
        .expect("write artifact");
    handle.bytes
}

pub fn crop_sample_artifact() -> usize {
    let tempdir = tempdir().expect("tempdir");
    let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
    let png = sample_png_bytes();
    let source = store
        .write_bytes(
            ArtifactKind::Screenshot,
            Some("run_demo"),
            "inst_demo",
            "tab_demo",
            &png,
        )
        .expect("write source artifact");
    let handle = store
        .crop_artifact(
            &source,
            Some("run_demo"),
            &NormalizedRegion {
                x_min: 100,
                y_min: 100,
                x_max: 900,
                y_max: 900,
            },
            None,
        )
        .expect("crop artifact");
    handle.bytes
}

pub fn crop_grid_sample_artifact() -> usize {
    let tempdir = tempdir().expect("tempdir");
    let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
    let png = sample_png_bytes();
    let source = store
        .write_bytes(
            ArtifactKind::Screenshot,
            Some("run_demo"),
            "inst_demo",
            "tab_demo",
            &png,
        )
        .expect("write source artifact");
    let regions = ArtifactStore::batch_grid_regions(3, 3, 20).expect("grid regions");
    let handles = store
        .crop_artifact_many(&source, Some("run_demo"), &regions, None)
        .expect("crop artifact grid");
    handles.iter().map(|handle| handle.bytes).sum()
}

pub fn write_recording_archive_sample() -> usize {
    let tempdir = tempdir().expect("tempdir");
    let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
    let png = sample_png_bytes();
    let frames = (0..6)
        .map(|index| (format!("frames/frame-{index:04}.png"), png.clone()))
        .collect::<Vec<_>>();
    let handle = store
        .write_recording_archive(
            Some("run_demo"),
            "inst_demo",
            "tab_demo",
            r#"{"schema_version":1,"frame_count":6}"#,
            &frames,
        )
        .expect("write recording archive");
    handle.bytes
}

pub fn materialize_sample_artifact() -> usize {
    let tempdir = tempdir().expect("tempdir");
    let source = tempdir.path().join("source.bin");
    let destination = tempdir.path().join("destination.bin");
    fs::write(&source, vec![7_u8; 256 * 1024]).expect("write source");
    copy_with_sha256(&source, &destination).expect("materialize artifact");
    fs::metadata(&destination)
        .expect("materialized artifact metadata")
        .len() as usize
}

pub fn checksum_sample_artifact() -> usize {
    let tempdir = tempdir().expect("tempdir");
    let path = tempdir.path().join("artifact.bin");
    fs::write(&path, vec![9_u8; 256 * 1024]).expect("write sample");
    sha256_path(&path).expect("checksum").len()
}

fn sample_png_bytes() -> Vec<u8> {
    let mut image = RgbaImage::new(64, 64);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([255, 128, 0, 255]);
    }
    let mut buffer = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut buffer, image::ImageFormat::Png)
        .expect("encode png");
    buffer.into_inner()
}

fn copy_with_sha256(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<String> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::File::create(destination)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut input, &mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_path(path: &std::path::Path) -> std::io::Result<String> {
    let mut input = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut input, &mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
