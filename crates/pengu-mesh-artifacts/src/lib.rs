use anyhow::{Context, Result};
use base64::{Engine, read::DecoderReader};
use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::{BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::{Builder, Header};

use pengu_mesh_shared::{
    ArtifactHandle, ArtifactKind, ArtifactProvenance, IdKind, NormalizedRegion, StableId,
    utc_timestamp,
};

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactDescriptor {
    pub kind: ArtifactKind,
    pub streaming_policy: &'static str,
    pub storage_path_hint: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchCropArtifact {
    pub crop_region: NormalizedRegion,
    pub artifact: ArtifactHandle,
}

pub fn baseline_artifacts() -> Vec<ArtifactDescriptor> {
    vec![
        ArtifactDescriptor {
            kind: ArtifactKind::Screenshot,
            streaming_policy: "stream-to-disk",
            storage_path_hint: "artifacts/screenshots",
        },
        ArtifactDescriptor {
            kind: ArtifactKind::Pdf,
            streaming_policy: "stream-to-disk",
            storage_path_hint: "artifacts/pdfs",
        },
        ArtifactDescriptor {
            kind: ArtifactKind::Trace,
            streaming_policy: "stream-to-disk",
            storage_path_hint: "artifacts/traces",
        },
        ArtifactDescriptor {
            kind: ArtifactKind::Recording,
            streaming_policy: "stream-to-disk",
            storage_path_hint: "artifacts/recordings",
        },
    ]
}

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for descriptor in baseline_artifacts() {
            fs::create_dir_all(root.join(kind_dir(&descriptor.kind)))
                .with_context(|| format!("create artifact dir under {}", root.display()))?;
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_base64(
        &self,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        payload: &str,
    ) -> Result<ArtifactHandle> {
        if should_stream_large_capture(&kind, estimated_base64_decoded_len(payload.len())) {
            return self.write_base64_streamed(
                kind,
                run_id,
                instance_id,
                tab_id,
                payload,
                ArtifactProvenance::primary(),
            );
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .context("decode artifact payload")?;
        self.write_bytes(kind, run_id, instance_id, tab_id, &bytes)
    }

    pub fn write_text(
        &self,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        payload: &str,
    ) -> Result<ArtifactHandle> {
        self.write_bytes(kind, run_id, instance_id, tab_id, payload.as_bytes())
    }

    pub fn write_bytes(
        &self,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        payload: &[u8],
    ) -> Result<ArtifactHandle> {
        self.write_bytes_with_provenance(
            kind,
            run_id,
            instance_id,
            tab_id,
            payload,
            ArtifactProvenance::primary(),
        )
    }

    pub fn write_bytes_with_provenance(
        &self,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        payload: &[u8],
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactHandle> {
        let created_at = utc_timestamp();
        let id = artifact_id(&kind, instance_id, tab_id, &created_at);
        self.write_payload(
            id,
            kind,
            run_id,
            instance_id,
            tab_id,
            payload,
            created_at,
            provenance,
        )
    }

    pub fn crop_artifact(
        &self,
        source: &ArtifactHandle,
        run_id: Option<&str>,
        region: &NormalizedRegion,
        page_index: Option<u32>,
    ) -> Result<ArtifactHandle> {
        Ok(self
            .crop_artifact_many(source, run_id, std::slice::from_ref(region), page_index)?
            .into_iter()
            .next()
            .expect("single crop artifact"))
    }

    pub fn crop_artifact_many(
        &self,
        source: &ArtifactHandle,
        run_id: Option<&str>,
        regions: &[NormalizedRegion],
        page_index: Option<u32>,
    ) -> Result<Vec<ArtifactHandle>> {
        for region in regions {
            region.validate()?;
        }
        let crop_source = match source.kind {
            ArtifactKind::Screenshot => {
                let bytes = fs::read(&source.path)
                    .with_context(|| format!("read screenshot {}", source.path))?;
                CropSource::Image(image::load_from_memory(&bytes).context("decode png payload")?)
            }
            ArtifactKind::Pdf => {
                let rendered = render_pdf_page_png(&source.path, page_index.unwrap_or(0))?;
                CropSource::Image(
                    image::load_from_memory(&rendered).context("decode rendered pdf page")?,
                )
            }
            _ => anyhow::bail!("artifact_crop only supports screenshot and pdf artifacts"),
        };
        regions
            .iter()
            .map(|region| {
                let provenance = ArtifactProvenance {
                    source_artifact_id: Some(source.id.clone()),
                    crop_region: Some(region.clone()),
                    page_index,
                };
                let cropped = crop_source.crop_png(region)?;
                self.write_bytes_with_provenance(
                    ArtifactKind::Screenshot,
                    run_id,
                    &source.instance_id,
                    &source.tab_id,
                    &cropped,
                    provenance,
                )
            })
            .collect()
    }

    pub fn write_recording_archive(
        &self,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        manifest_json: &str,
        frames: &[(String, Vec<u8>)],
    ) -> Result<ArtifactHandle> {
        let mut archive = Vec::new();
        {
            let mut builder = Builder::new(&mut archive);
            append_tar_bytes(&mut builder, "manifest.json", manifest_json.as_bytes())?;
            for (name, payload) in frames {
                append_tar_bytes(&mut builder, name, payload)?;
            }
            builder.finish().context("finish recording archive")?;
        }
        self.write_bytes(
            ArtifactKind::Recording,
            run_id,
            instance_id,
            tab_id,
            &archive,
        )
    }

    pub fn batch_grid_regions(rows: u16, cols: u16, overlap: u16) -> Result<Vec<NormalizedRegion>> {
        anyhow::ensure!(rows >= 1, "rows must be at least 1");
        anyhow::ensure!(cols >= 1, "cols must be at least 1");
        anyhow::ensure!(
            overlap <= 250,
            "overlap must be at most 250 normalized units"
        );
        anyhow::ensure!(
            u32::from(rows) * u32::from(cols) <= 64,
            "grid may contain at most 64 regions"
        );
        let mut regions = Vec::with_capacity(rows as usize * cols as usize);
        for row in 0..rows {
            let y0 = ((u32::from(row) * 1000) / u32::from(rows)) as i32;
            let y1 = (u32::from(row + 1) * 1000).div_ceil(u32::from(rows)) as i32;
            for col in 0..cols {
                let x0 = ((u32::from(col) * 1000) / u32::from(cols)) as i32;
                let x1 = (u32::from(col + 1) * 1000).div_ceil(u32::from(cols)) as i32;
                let region = NormalizedRegion {
                    x_min: (x0 - i32::from(overlap)).clamp(0, 999) as u16,
                    y_min: (y0 - i32::from(overlap)).clamp(0, 999) as u16,
                    x_max: (x1 + i32::from(overlap)).clamp(1, 999) as u16,
                    y_max: (y1 + i32::from(overlap)).clamp(1, 999) as u16,
                };
                region.validate()?;
                regions.push(region);
            }
        }
        Ok(regions)
    }

    #[allow(clippy::too_many_arguments)]
    fn write_payload(
        &self,
        id: String,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        payload: &[u8],
        created_at: String,
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactHandle> {
        let path = self.output_path(&kind, &id)?;
        let checksum_sha256 = if should_stream_large_capture(&kind, payload.len()) {
            write_artifact_chunked(
                &path,
                payload,
                chunked_write_config_for_payload_len(payload.len()),
            )
            .with_context(|| format!("stream {}", path.display()))?
            .sha256
        } else {
            fs::write(&path, payload).with_context(|| format!("write {}", path.display()))?;
            sha256_hex(payload)
        };
        let metadata = fs::metadata(&path).with_context(|| format!("stat {}", path.display()))?;
        Ok(self.build_artifact_handle(
            id,
            kind,
            run_id,
            instance_id,
            tab_id,
            &path,
            metadata.len() as usize,
            created_at,
            checksum_sha256,
            provenance,
        ))
    }

    fn write_base64_streamed(
        &self,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        payload: &str,
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactHandle> {
        let created_at = utc_timestamp();
        let id = artifact_id(&kind, instance_id, tab_id, &created_at);
        let path = self.output_path(&kind, &id)?;
        let checksum_sha256 = write_base64_artifact_chunked(
            &path,
            payload,
            chunked_write_config_for_payload_len(estimated_base64_decoded_len(payload.len())),
        )
        .with_context(|| format!("stream base64 payload to {}", path.display()))?
        .sha256;
        let metadata = fs::metadata(&path).with_context(|| format!("stat {}", path.display()))?;
        Ok(self.build_artifact_handle(
            id,
            kind,
            run_id,
            instance_id,
            tab_id,
            &path,
            metadata.len() as usize,
            created_at,
            checksum_sha256,
            provenance,
        ))
    }

    fn output_path(&self, kind: &ArtifactKind, id: &str) -> Result<PathBuf> {
        let directory = self.root.join(kind_dir(kind));
        fs::create_dir_all(&directory)
            .with_context(|| format!("create artifact directory {}", directory.display()))?;
        Ok(directory.join(format!("{id}.{}", extension(kind))))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_artifact_handle(
        &self,
        id: String,
        kind: ArtifactKind,
        run_id: Option<&str>,
        instance_id: &str,
        tab_id: &str,
        path: &Path,
        bytes: usize,
        created_at: String,
        checksum_sha256: String,
        provenance: ArtifactProvenance,
    ) -> ArtifactHandle {
        ArtifactHandle {
            id,
            run_id: run_id.map(str::to_string),
            instance_id: instance_id.to_string(),
            tab_id: tab_id.to_string(),
            kind: kind.clone(),
            path: path.display().to_string(),
            mime_type: mime_type(&kind).to_string(),
            bytes,
            created_at,
            checksum_sha256: Some(checksum_sha256),
            provenance,
        }
    }
}

// ---------------------------------------------------------------------------
// Chunked capture streaming
// ---------------------------------------------------------------------------

/// Configuration for chunked artifact writes.
#[derive(Debug, Clone)]
struct ChunkedWriteConfig {
    /// Maximum bytes written per chunk (default 64 KB).
    pub chunk_size: usize,
    /// Hard ceiling on the total artifact size (default 100 MB).
    pub max_total_size: usize,
}

impl Default for ChunkedWriteConfig {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024,
            max_total_size: 100 * 1024 * 1024,
        }
    }
}

/// Result returned after a chunked write completes.
#[derive(Debug, Clone, Serialize)]
struct ChunkedWriteResult {
    pub path: String,
    pub total_bytes: usize,
    pub chunks_written: usize,
    pub sha256: String,
}

/// Incrementally writes an artifact to disk in fixed-size chunks while computing
/// a rolling SHA-256 digest.
struct ChunkedWriter {
    writer: BufWriter<fs::File>,
    hasher: Sha256,
    path: PathBuf,
    config: ChunkedWriteConfig,
    total_written: usize,
    chunks_written: usize,
}

impl ChunkedWriter {
    /// Open a new chunked writer at `path`.
    fn new(path: &Path, config: ChunkedWriteConfig) -> std::io::Result<Self> {
        if config.chunk_size == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "chunk_size must be at least 1 byte",
            ));
        }
        let file = fs::File::create(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
            hasher: Sha256::new(),
            path: path.to_path_buf(),
            config,
            total_written: 0,
            chunks_written: 0,
        })
    }

    /// Write up to `chunk_size` bytes from `data`.
    ///
    /// Returns the number of bytes actually written in this call.  Returns an
    /// error if the cumulative written bytes would exceed `max_total_size`.
    fn write_chunk(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        let len = data.len().min(self.config.chunk_size);
        if self.total_written + len > self.config.max_total_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "chunked write would exceed max_total_size ({} bytes)",
                    self.config.max_total_size,
                ),
            ));
        }
        self.writer.write_all(&data[..len])?;
        self.hasher.update(&data[..len]);
        self.total_written += len;
        self.chunks_written += 1;
        Ok(len)
    }

    /// Flush buffers and return the final result.
    fn finish(mut self) -> std::io::Result<ChunkedWriteResult> {
        self.writer.flush()?;
        Ok(ChunkedWriteResult {
            path: self.path.display().to_string(),
            total_bytes: self.total_written,
            chunks_written: self.chunks_written,
            sha256: format!("{:x}", self.hasher.finalize()),
        })
    }
}

/// Convenience helper: write `data` to `path` in chunks, returning the result.
fn write_artifact_chunked(
    path: &Path,
    data: &[u8],
    config: ChunkedWriteConfig,
) -> std::io::Result<ChunkedWriteResult> {
    write_reader_chunked(path, Cursor::new(data), config)
}

fn write_base64_artifact_chunked(
    path: &Path,
    payload: &str,
    config: ChunkedWriteConfig,
) -> std::io::Result<ChunkedWriteResult> {
    let reader = Cursor::new(payload.as_bytes());
    write_reader_chunked(
        path,
        DecoderReader::new(reader, &base64::engine::general_purpose::STANDARD),
        config,
    )
}

fn write_reader_chunked<R: Read>(
    path: &Path,
    mut reader: R,
    config: ChunkedWriteConfig,
) -> std::io::Result<ChunkedWriteResult> {
    let buffer_len = config.chunk_size;
    let mut writer = ChunkedWriter::new(path, config)?;
    let mut buffer = vec![0_u8; buffer_len];
    let result = (|| {
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let written = writer.write_chunk(&buffer[..bytes_read])?;
            if written != bytes_read {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    format!("chunked writer accepted {written} bytes after reading {bytes_read}"),
                ));
            }
        }
        writer.finish()
    })();
    if result.is_err() {
        cleanup_partial_file(path);
    }
    result
}

// ---------------------------------------------------------------------------

enum CropSource {
    Image(DynamicImage),
}

impl CropSource {
    fn crop_png(&self, region: &NormalizedRegion) -> Result<Vec<u8>> {
        match self {
            Self::Image(image) => {
                let (width, height) = image.dimensions();
                let x_min = scale_floor(region.x_min, width);
                let y_min = scale_floor(region.y_min, height);
                let x_max = scale_ceil(region.x_max, width);
                let y_max = scale_ceil(region.y_max, height);
                anyhow::ensure!(x_min < x_max, "crop produced empty width");
                anyhow::ensure!(y_min < y_max, "crop produced empty height");
                let cropped = image.crop_imm(x_min, y_min, x_max - x_min, y_max - y_min);
                encode_png(&cropped)
            }
        }
    }
}

fn encode_png(image: &DynamicImage) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .context("encode png artifact")?;
    Ok(cursor.into_inner())
}

fn scale_floor(value: u16, dimension: u32) -> u32 {
    ((u64::from(value) * u64::from(dimension)) / 1000) as u32
}

fn scale_ceil(value: u16, dimension: u32) -> u32 {
    (u64::from(value) * u64::from(dimension)).div_ceil(1000) as u32
}

fn render_pdf_page_png(path: &str, page_index: u32) -> Result<Vec<u8>> {
    let tmp_root = std::env::temp_dir().join(format!(
        "pengu-mesh-pdf-render-{}",
        utc_timestamp().replace(':', "-")
    ));
    fs::create_dir_all(&tmp_root)
        .with_context(|| format!("create temp render dir {}", tmp_root.display()))?;
    let prefix = tmp_root.join("page");
    let prefix_arg: OsString = prefix.as_os_str().to_os_string();
    let output = Command::new("pdftoppm")
        .arg("-f")
        .arg((page_index + 1).to_string())
        .arg("-l")
        .arg((page_index + 1).to_string())
        .arg("-singlefile")
        .arg("-png")
        .arg(path)
        .arg(&prefix_arg)
        .output()
        .context("run pdftoppm for pdf crop")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let _ = fs::remove_dir_all(&tmp_root);
        anyhow::bail!(
            "pdftoppm failed for {} page {}: {}",
            path,
            page_index,
            stderr
        );
    }
    let rendered_path = tmp_root.join("page.png");
    let bytes = fs::read(&rendered_path)
        .with_context(|| format!("read rendered pdf page {}", rendered_path.display()))?;
    let _ = fs::remove_dir_all(&tmp_root);
    Ok(bytes)
}

fn artifact_id(kind: &ArtifactKind, instance_id: &str, tab_id: &str, created_at: &str) -> String {
    StableId::new(
        IdKind::Artifact,
        format!(
            "{}_{}_{}_{}",
            kind_dir(kind),
            instance_id,
            tab_id,
            created_at
        ),
    )
    .into_string()
}

fn should_stream_large_capture(kind: &ArtifactKind, payload_len: usize) -> bool {
    matches!(kind, ArtifactKind::Screenshot | ArtifactKind::Pdf)
        && payload_len > ChunkedWriteConfig::default().chunk_size
}

fn chunked_write_config_for_payload_len(payload_len: usize) -> ChunkedWriteConfig {
    let config = ChunkedWriteConfig::default();
    ChunkedWriteConfig {
        max_total_size: payload_len.max(config.max_total_size),
        ..config
    }
}

fn estimated_base64_decoded_len(payload_len: usize) -> usize {
    payload_len.div_ceil(4).saturating_mul(3)
}

fn cleanup_partial_file(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
}

fn append_tar_bytes(builder: &mut Builder<&mut Vec<u8>>, path: &str, payload: &[u8]) -> Result<()> {
    let mut header = Header::new_gnu();
    header
        .set_path(path)
        .with_context(|| format!("set tar path {path}"))?;
    header.set_size(payload.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append(&header, payload)
        .with_context(|| format!("append {path} to recording archive"))?;
    Ok(())
}

fn kind_dir(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Screenshot => "screenshots",
        ArtifactKind::Pdf => "pdfs",
        ArtifactKind::Snapshot => "snapshots",
        ArtifactKind::Text => "text",
        ArtifactKind::Trace => "traces",
        ArtifactKind::Recording => "recordings",
    }
}

fn extension(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Screenshot => "png",
        ArtifactKind::Pdf => "pdf",
        ArtifactKind::Snapshot => "json",
        ArtifactKind::Text => "txt",
        ArtifactKind::Trace => "json",
        ArtifactKind::Recording => "tar",
    }
}

fn mime_type(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Screenshot => "image/png",
        ArtifactKind::Pdf => "application/pdf",
        ArtifactKind::Snapshot => "application/json",
        ArtifactKind::Text => "text/plain; charset=utf-8",
        ArtifactKind::Trace => "application/json",
        ArtifactKind::Recording => "application/x-tar",
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtifactStore, baseline_artifacts};
    use base64::Engine;
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
    use pengu_mesh_shared::{ArtifactKind, ArtifactProvenance, NormalizedRegion};
    use std::io::Read;
    use std::path::Path;
    use tar::Archive;
    use tempfile::tempdir;

    #[test]
    fn advertises_stage_one_artifacts() {
        let descriptors = baseline_artifacts();
        assert_eq!(descriptors.len(), 4);
        assert!(
            descriptors
                .iter()
                .any(|item| item.kind == ArtifactKind::Screenshot)
        );
        assert!(
            descriptors
                .iter()
                .any(|item| item.kind == ArtifactKind::Pdf)
        );
        assert!(
            descriptors
                .iter()
                .any(|item| item.kind == ArtifactKind::Trace)
        );
        assert!(
            descriptors
                .iter()
                .any(|item| item.kind == ArtifactKind::Recording)
        );
    }

    #[test]
    fn writes_artifacts_to_disk() {
        let tempdir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
        let handle = store
            .write_bytes(
                ArtifactKind::Screenshot,
                Some("run_demo"),
                "inst_demo",
                "tab_demo",
                b"png-bytes",
            )
            .expect("handle");
        assert!(Path::new(&handle.path).exists());
        assert_eq!(handle.mime_type, "image/png");
        assert_eq!(handle.run_id.as_deref(), Some("run_demo"));
    }

    #[test]
    fn crops_screenshot_artifacts() {
        let tempdir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
        let mut image = RgbaImage::new(10, 10);
        for pixel in image.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");
        let source = store
            .write_bytes(
                ArtifactKind::Screenshot,
                Some("run_demo"),
                "inst_demo",
                "tab_demo",
                &bytes,
            )
            .expect("source artifact");
        let cropped = store
            .crop_artifact(
                &source,
                Some("run_demo"),
                &NormalizedRegion {
                    x_min: 200,
                    y_min: 200,
                    x_max: 800,
                    y_max: 800,
                },
                None,
            )
            .expect("crop artifact");
        assert!(Path::new(&cropped.path).exists());
        assert_eq!(
            cropped.provenance,
            ArtifactProvenance {
                source_artifact_id: Some(source.id.clone()),
                crop_region: Some(NormalizedRegion {
                    x_min: 200,
                    y_min: 200,
                    x_max: 800,
                    y_max: 800,
                }),
                page_index: None,
            }
        );
        let cropped_image = image::open(&cropped.path).expect("open cropped artifact");
        assert_eq!(cropped_image.dimensions(), (6, 6));
    }

    #[test]
    fn crops_pdf_artifacts_via_rendered_page() {
        if !pdftoppm_available() {
            return;
        }
        let tempdir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
        let pdf_path = tempdir.path().join("fixture.pdf");
        std::fs::write(&pdf_path, minimal_pdf_bytes()).expect("write fixture pdf");
        let source = pengu_mesh_shared::ArtifactHandle {
            id: "artifact_pdf".into(),
            run_id: Some("run_demo".into()),
            instance_id: "inst_demo".into(),
            tab_id: "tab_demo".into(),
            kind: ArtifactKind::Pdf,
            path: pdf_path.display().to_string(),
            mime_type: "application/pdf".into(),
            bytes: std::fs::metadata(&pdf_path).expect("pdf metadata").len() as usize,
            created_at: "2026-03-11T12:00:00Z".into(),
            checksum_sha256: None,
            provenance: ArtifactProvenance::primary(),
        };
        let cropped = store
            .crop_artifact(
                &source,
                Some("run_demo"),
                &NormalizedRegion {
                    x_min: 100,
                    y_min: 100,
                    x_max: 900,
                    y_max: 900,
                },
                Some(0),
            )
            .expect("crop pdf artifact");
        assert!(Path::new(&cropped.path).exists());
        assert_eq!(cropped.kind, ArtifactKind::Screenshot);
        assert_eq!(
            cropped.provenance.source_artifact_id.as_deref(),
            Some("artifact_pdf")
        );
        assert_eq!(cropped.provenance.page_index, Some(0));
    }

    #[test]
    fn builds_bounded_grid_regions() {
        let regions = ArtifactStore::batch_grid_regions(2, 3, 25).expect("grid regions");
        assert_eq!(regions.len(), 6);
        assert_eq!(
            regions.first().expect("first region"),
            &NormalizedRegion {
                x_min: 0,
                y_min: 0,
                x_max: 359,
                y_max: 525,
            }
        );
        assert_eq!(
            regions.last().expect("last region"),
            &NormalizedRegion {
                x_min: 641,
                y_min: 475,
                x_max: 999,
                y_max: 999,
            }
        );
    }

    #[test]
    fn rejects_oversized_grid_region_sets() {
        let error = ArtifactStore::batch_grid_regions(9, 8, 0).expect_err("oversized grid");
        assert!(error.to_string().contains("at most 64"));
    }

    #[test]
    fn writes_recording_archive_with_manifest_and_frames() {
        let tempdir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
        let handle = store
            .write_recording_archive(
                Some("run_demo"),
                "inst_demo",
                "tab_demo",
                r#"{"frame_count":2}"#,
                &[
                    ("frames/frame-0000.png".into(), vec![1, 2, 3]),
                    ("frames/frame-0001.png".into(), vec![4, 5, 6]),
                ],
            )
            .expect("recording archive");
        assert_eq!(handle.kind, ArtifactKind::Recording);
        assert_eq!(handle.mime_type, "application/x-tar");

        let mut archive = Archive::new(std::fs::File::open(&handle.path).expect("open tar"));
        let mut names = archive
            .entries()
            .expect("tar entries")
            .map(|entry| {
                let entry = entry.expect("tar entry");
                entry
                    .path()
                    .expect("entry path")
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            vec![
                "frames/frame-0000.png".to_string(),
                "frames/frame-0001.png".to_string(),
                "manifest.json".to_string(),
            ]
        );

        let mut archive = Archive::new(std::fs::File::open(&handle.path).expect("open tar"));
        let mut manifest = String::new();
        for entry in archive.entries().expect("tar entries") {
            let mut entry = entry.expect("tar entry");
            if entry.path().expect("entry path").to_string_lossy() == "manifest.json" {
                entry.read_to_string(&mut manifest).expect("read manifest");
            }
        }
        assert!(manifest.contains("\"frame_count\":2"));
    }

    #[test]
    fn creates_deterministic_batch_grid_regions() {
        let regions = ArtifactStore::batch_grid_regions(2, 2, 25).expect("grid regions");
        assert_eq!(regions.len(), 4);
        assert_eq!(
            regions[0],
            NormalizedRegion {
                x_min: 0,
                y_min: 0,
                x_max: 525,
                y_max: 525,
            }
        );
        assert_eq!(
            regions[3],
            NormalizedRegion {
                x_min: 475,
                y_min: 475,
                x_max: 999,
                y_max: 999,
            }
        );
    }

    #[test]
    fn chunked_write_small_payload_fits_in_one_chunk() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("small.bin");
        let data = vec![0xABu8; 100];
        let result =
            super::write_artifact_chunked(&path, &data, super::ChunkedWriteConfig::default())
                .expect("chunked write");
        assert_eq!(result.total_bytes, 100);
        assert_eq!(result.chunks_written, 1);
        assert_eq!(std::fs::read(&path).expect("read back"), data);
        assert_eq!(result.sha256, super::sha256_hex(&data));
    }

    #[test]
    fn chunked_write_multi_chunk_payload() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("medium.bin");
        let config = super::ChunkedWriteConfig {
            chunk_size: 32,
            max_total_size: 1024,
        };
        let data = vec![0xCDu8; 100];
        let result = super::write_artifact_chunked(&path, &data, config).expect("chunked write");
        assert_eq!(result.total_bytes, 100);
        // 100 / 32 = 3 full chunks + 1 partial = 4
        assert_eq!(result.chunks_written, 4);
        assert_eq!(std::fs::read(&path).expect("read back"), data);
        assert_eq!(result.sha256, super::sha256_hex(&data));
    }

    #[test]
    fn chunked_write_rejects_oversized_payload() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("oversized.bin");
        let config = super::ChunkedWriteConfig {
            chunk_size: 32,
            max_total_size: 50,
        };
        let data = vec![0xFFu8; 100];
        let err = super::write_artifact_chunked(&path, &data, config).expect_err("should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("max_total_size"));
    }

    #[test]
    fn chunked_write_removes_partial_file_on_oversize() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("cleanup.bin");
        let config = super::ChunkedWriteConfig {
            chunk_size: 32,
            max_total_size: 48,
        };
        let data = vec![0xAAu8; 64];
        let err = super::write_artifact_chunked(&path, &data, config).expect_err("should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            !path.exists(),
            "oversized chunked writes must not leave partial files"
        );
    }

    #[test]
    fn chunked_writer_incremental_api() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("incremental.bin");
        let config = super::ChunkedWriteConfig {
            chunk_size: 16,
            max_total_size: 256,
        };
        let mut writer = super::ChunkedWriter::new(&path, config).expect("open writer");
        let chunk_a = [1u8; 16];
        let chunk_b = [2u8; 10];
        assert_eq!(writer.write_chunk(&chunk_a).expect("write a"), 16);
        assert_eq!(writer.write_chunk(&chunk_b).expect("write b"), 10);
        let result = writer.finish().expect("finish");
        assert_eq!(result.total_bytes, 26);
        assert_eq!(result.chunks_written, 2);
        let on_disk = std::fs::read(&path).expect("read back");
        let mut expected = Vec::new();
        expected.extend_from_slice(&chunk_a);
        expected.extend_from_slice(&chunk_b);
        assert_eq!(on_disk, expected);
        assert_eq!(result.sha256, super::sha256_hex(&expected));
    }

    #[test]
    fn streams_large_screenshot_bytes_through_artifact_store() {
        let tempdir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
        let payload = vec![0x5Au8; (super::ChunkedWriteConfig::default().chunk_size * 3) + 17];
        let handle = store
            .write_bytes(
                ArtifactKind::Screenshot,
                Some("run_large"),
                "inst_demo",
                "tab_demo",
                &payload,
            )
            .expect("streamed screenshot");
        assert!(Path::new(&handle.path).exists());
        assert!(handle.path.contains("/screenshots/"));
        assert_eq!(handle.bytes, payload.len());
        let checksum = super::sha256_hex(&payload);
        assert_eq!(handle.checksum_sha256.as_deref(), Some(checksum.as_str()));
        assert_eq!(std::fs::read(&handle.path).expect("read back"), payload);
    }

    #[test]
    fn streams_large_pdf_base64_through_artifact_store() {
        let tempdir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(tempdir.path()).expect("artifact store");
        let mut payload = b"%PDF-1.7\n".to_vec();
        payload.extend(vec![
            0x33u8;
            (super::ChunkedWriteConfig::default().chunk_size * 3)
                + 41
        ]);
        let encoded = base64::engine::general_purpose::STANDARD.encode(&payload);
        let handle = store
            .write_base64(
                ArtifactKind::Pdf,
                Some("run_large"),
                "inst_demo",
                "tab_demo",
                &encoded,
            )
            .expect("streamed pdf");
        assert!(Path::new(&handle.path).exists());
        assert!(handle.path.contains("/pdfs/"));
        assert_eq!(handle.bytes, payload.len());
        let checksum = super::sha256_hex(&payload);
        assert_eq!(handle.checksum_sha256.as_deref(), Some(checksum.as_str()));
        assert_eq!(std::fs::read(&handle.path).expect("read back"), payload);
    }

    fn pdftoppm_available() -> bool {
        std::process::Command::new("pdftoppm")
            .arg("-v")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn minimal_pdf_bytes() -> Vec<u8> {
        let stream = b"0.9 0 0 rg\n0 0 200 200 re\nf\n";
        let objects = [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_string(),
            "2 0 obj\n<< /Type /Pages /Count 1 /Kids [3 0 R] >>\nendobj\n".to_string(),
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << >> >>\nendobj\n".to_string(),
            format!(
                "4 0 obj\n<< /Length {} >>\nstream\n{}endstream\nendobj\n",
                stream.len(),
                String::from_utf8_lossy(stream)
            ),
        ];
        let mut pdf = b"%PDF-1.4\n%\xFF\xFF\xFF\xFF\n".to_vec();
        let mut offsets = Vec::new();
        for object in objects {
            offsets.push(pdf.len());
            pdf.extend_from_slice(object.as_bytes());
        }
        let xref_start = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", offsets.len() + 1).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref_start}\n%%EOF\n")
                .as_bytes(),
        );
        pdf
    }
}
