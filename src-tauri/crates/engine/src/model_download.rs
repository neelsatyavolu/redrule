//! Fetches the on-device models on first run. Everything lands in a temporary path first and is
//! renamed into place only when complete, so an interrupted download is simply retried.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use minutes_core::{Error, Result};
use tokio::io::AsyncWriteExt;

const RELEASES: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
/// Progress is reported at most once per this many bytes.
const PROGRESS_STEP: u64 = 1 << 20;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub stage: String,
}

pub(crate) type Progress = dyn Fn(ModelProgress) + Send + Sync;

/// A downloadable model: a `.tar.bz2` archive holding one directory, or a single file.
pub(crate) struct Model {
    /// Directory or file name under the models folder.
    pub name: &'static str,
    /// Under the sherpa-onnx releases, or a full `https://` URL.
    pub url_path: &'static str,
    /// Files that must exist (relative to `name` for archives) for the model to count as present.
    pub files: &'static [&'static str],
    /// What the model is called in progress and error messages.
    pub noun: &'static str,
}

impl Model {
    pub(crate) fn path(&self, models_dir: &Path) -> PathBuf {
        models_dir.join(self.name)
    }

    fn url(&self) -> String {
        if self.url_path.starts_with("https://") { self.url_path.to_string() } else { format!("{RELEASES}/{}", self.url_path) }
    }

    fn is_archive(&self) -> bool {
        self.url_path.ends_with(".tar.bz2")
    }

    pub(crate) fn is_present(&self, models_dir: &Path) -> bool {
        let path = self.path(models_dir);
        if self.is_archive() { self.files.iter().all(|file| path.join(file).is_file()) } else { path.is_file() }
    }
}

/// Downloads `model` into `models_dir` unless it is already there.
pub(crate) async fn ensure(model: &Model, models_dir: &Path, progress: &Progress) -> Result<()> {
    if model.is_present(models_dir) {
        return Ok(());
    }
    tokio::fs::create_dir_all(models_dir).await?;
    let download = models_dir.join(format!(".{}.download", model.name));
    let result = fetch(model, &download, progress).await;
    let result = match result {
        Ok(()) if model.is_archive() => install_archive(model, &download, models_dir).await,
        Ok(()) => tokio::fs::rename(&download, model.path(models_dir)).await.map_err(Error::from),
        Err(error) => Err(error),
    };
    // A leftover partial file is useless; a failure to delete it is not worth reporting.
    let _ = tokio::fs::remove_file(&download).await;
    result.map_err(|error| Error::message(format!("The {} could not be downloaded. {error}", model.noun)))
}

async fn fetch(model: &Model, destination: &Path, progress: &Progress) -> Result<()> {
    let response = reqwest::get(model.url())
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| Error::message(format!("Check your internet connection and try again. ({error})")))?;
    let total = response.content_length();
    let report = |downloaded| {
        progress(ModelProgress {
            downloaded_bytes: downloaded,
            total_bytes: total,
            stage: format!("Downloading {}", model.noun),
        })
    };

    let mut file = tokio::fs::File::create(destination).await?;
    let mut body = response.bytes_stream();
    let (mut downloaded, mut reported) = (0u64, 0u64);
    report(0);
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|error| Error::message(format!("The download was interrupted. ({error})")))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        if downloaded - reported >= PROGRESS_STEP {
            reported = downloaded;
            report(downloaded);
        }
    }
    file.flush().await?;
    report(downloaded);
    Ok(())
}

/// Unpacks next to the destination, then renames the model directory into place.
async fn install_archive(model: &Model, archive: &Path, models_dir: &Path) -> Result<()> {
    let staging = models_dir.join(format!(".{}.partial", model.name));
    let (archive, staging_dir) = (archive.to_path_buf(), staging.clone());
    let unpacked = tokio::task::spawn_blocking(move || unpack(&archive, &staging_dir))
        .await
        .map_err(|error| Error::message(error.to_string()))
        .and_then(|result| result);

    let result = match unpacked {
        Ok(()) => move_into_place(model, &staging, models_dir).await,
        Err(error) => Err(error),
    };
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

fn unpack(archive: &Path, into: &Path) -> Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    let file = std::fs::File::open(archive)?;
    tar::Archive::new(bzip2::read::BzDecoder::new(file)).unpack(into)?;
    Ok(())
}

async fn move_into_place(model: &Model, staging: &Path, models_dir: &Path) -> Result<()> {
    let unpacked = staging.join(model.name);
    if !model.files.iter().all(|file| unpacked.join(file).is_file()) {
        return Err(Error::message("The downloaded archive is missing model files."));
    }
    let target = model.path(models_dir);
    if target.exists() {
        tokio::fs::remove_dir_all(&target).await?;
    }
    tokio::fs::rename(&unpacked, &target).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{PARAKEET_V3 as SPEECH, PYANNOTE as SEGMENTATION, RESNET34 as EMBEDDING};

    #[test]
    fn archives_count_as_present_only_with_every_file() {
        let dir = tempfile::tempdir().unwrap();
        let model_dir = SPEECH.path(dir.path());
        std::fs::create_dir_all(&model_dir).unwrap();
        for file in &SPEECH.files[..3] {
            std::fs::write(model_dir.join(file), b"x").unwrap();
        }
        assert!(!SPEECH.is_present(dir.path()));
        std::fs::write(model_dir.join("tokens.txt"), b"x").unwrap();
        assert!(SPEECH.is_present(dir.path()));
    }

    #[test]
    fn single_file_models_are_present_as_a_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!EMBEDDING.is_present(dir.path()));
        std::fs::write(EMBEDDING.path(dir.path()), b"x").unwrap();
        assert!(EMBEDDING.is_present(dir.path()));
    }

    #[test]
    fn unpacks_an_archive_into_place() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("model.tar.bz2");
        let mut builder = tar::Builder::new(bzip2::write::BzEncoder::new(
            std::fs::File::create(&archive).unwrap(),
            bzip2::Compression::fast(),
        ));
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, format!("{}/model.onnx", SEGMENTATION.name), &b"x"[..]).unwrap();
        builder.into_inner().unwrap().finish().unwrap();

        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        runtime.block_on(install_archive(&SEGMENTATION, &archive, dir.path())).unwrap();
        assert!(SEGMENTATION.is_present(dir.path()));
        assert!(!dir.path().join(format!(".{}.partial", SEGMENTATION.name)).exists());
    }
}
