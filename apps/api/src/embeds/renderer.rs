use anyhow::Result;
use bytes::Bytes;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("failed to write renderer input files")]
    WriteFailed,

    #[error("failed to fetch preview audio")]
    AudioFetchFailed,

    #[error("ffmpeg failed to render preview video")]
    FfmpegFailed,

    #[error("rendered video file was not found")]
    OutputMissing,
}

#[derive(Clone)]
pub struct RenderInput {
    pub track_id: String,
    pub preview_url: String,
    pub track_og_bytes: Bytes,
}

pub struct FfmpegRenderer {
    output_root: String,
}

impl FfmpegRenderer {
    pub fn new(output_root: String) -> Self {
        Self { output_root }
    }

    pub fn video_output_path(&self, track_id: &str) -> String {
        format!("{}/{track_id}/out.mp4", self.output_root)
    }

    pub fn track_output_path(&self, track_id: &str) -> String {
        format!("{}/{track_id}", self.output_root)
    }

    pub async fn ensure_output_root_exists(&self) -> Result<(), RendererError> {
        tracing::debug!(
            output_root = self.output_root.as_str(),
            "ensuring renderer output root exists"
        );
        fs::create_dir_all(&self.output_root)
            .await
            .map_err(|_| RendererError::WriteFailed)
    }

    pub async fn render_preview_video(&self, input: RenderInput) -> Result<Bytes, RendererError> {
        let output_dir = self.track_output_path(&input.track_id);
        tracing::info!(
            track_id = input.track_id.as_str(),
            output_dir = output_dir.as_str(),
            "starting ffmpeg preview render"
        );
        fs::create_dir_all(&output_dir)
            .await
            .map_err(|_| RendererError::WriteFailed)?;

        tracing::debug!(track_id = input.track_id.as_str(), "writing og image input");
        self.write_file(Path::new(&output_dir).join("og.png"), input.track_og_bytes)
            .await?;

        tracing::debug!(track_id = input.track_id.as_str(), "fetching preview audio");
        let preview_audio = reqwest::get(input.preview_url)
            .await
            .map_err(|_| RendererError::AudioFetchFailed)?
            .bytes()
            .await
            .map_err(|_| RendererError::AudioFetchFailed)?;

        tracing::debug!(
            track_id = input.track_id.as_str(),
            audio_size_bytes = preview_audio.len(),
            "preview audio fetched"
        );
        self.write_file(Path::new(&output_dir).join("audio.mp3"), preview_audio)
            .await?;

        tracing::debug!(track_id = input.track_id.as_str(), "running ffmpeg process");
        let status = Command::new("ffmpeg")
            .args([
                "-loop",
                "1",
                "-i",
                "og.png",
                "-i",
                "audio.mp3",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "stillimage",
                "-crf",
                "23",
                "-r",
                "2",
                "-c:a",
                "aac",
                "-b:a",
                "96k",
                "-aac_coder",
                "fast",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
                "-threads",
                "0",
                "-shortest",
                "out.mp4",
            ])
            .current_dir(&output_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|_| RendererError::FfmpegFailed)?;

        if !status.success() {
            tracing::warn!(
                track_id = input.track_id.as_str(),
                exit_code = status.code().unwrap_or(-1),
                "ffmpeg preview render failed"
            );
            return Err(RendererError::FfmpegFailed);
        }

        let output_path = self.video_output_path(&input.track_id);
        let video_bytes = fs::read(output_path)
            .await
            .map(Bytes::from)
            .map_err(|_| RendererError::OutputMissing)?;

        tracing::debug!(
            track_id = input.track_id.as_str(),
            video_size_bytes = video_bytes.len(),
            "ffmpeg preview render complete"
        );

        Ok(video_bytes)
    }

    pub async fn remove_track_output(&self, track_id: &str) {
        tracing::debug!(track_id = track_id, "removing renderer output directory");
        let _ = fs::remove_dir_all(self.track_output_path(track_id)).await;
    }

    async fn write_file(&self, path: PathBuf, bytes: Bytes) -> Result<(), RendererError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| RendererError::WriteFailed)?;
        }

        let mut file = fs::File::create(path)
            .await
            .map_err(|_| RendererError::WriteFailed)?;
        file.write_all(&bytes)
            .await
            .map_err(|_| RendererError::WriteFailed)?;
        file.flush().await.map_err(|_| RendererError::WriteFailed)
    }
}
