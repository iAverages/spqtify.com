use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{Mutex, Notify};
use tokio::task;

use crate::embeds::cache_manager::{VideoCache, VideoCacheError};
use crate::embeds::image_client::{EmbedImageClient, OgImageError};
use crate::embeds::renderer::{FfmpegRenderer, RenderInput, RendererError};
use crate::embeds::spotify_metadata::{SpotifyMetadataClient, SpotifyMetadataError};
use crate::embeds::video_source::{B2VideoSource, VideoSourceError};
use crate::metrics::{FAILED_VIDEO_GENERATIONS, VIDEO_GEN_DURATION};

#[derive(Clone, Debug)]
pub struct ServedPreview {
    pub video_bytes: Bytes,
    pub mime: &'static str,
    pub cache_status: CacheStatus,
}

#[derive(Clone, Debug)]
pub enum CacheStatus {
    Hit,
    Hydrated,
    Rendered,
}

#[derive(Clone, Debug)]
pub struct PreloadedPreviewInput {
    pub track_id: String,
    pub preview_url: String,
    pub og_bytes: Bytes,
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewGenerationError {
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),

    #[error("spotify metadata failed: {0}")]
    Metadata(#[from] SpotifyMetadataError),

    #[error("og image generation failed: {0}")]
    Image(#[from] OgImageError),

    #[error("video cache failed: {0}")]
    Cache(#[from] VideoCacheError),

    #[error("video source failed: {0}")]
    Source(#[from] VideoSourceError),

    #[error("renderer failed: {0}")]
    Renderer(#[from] RendererError),

    #[error("video generation failed")]
    GenerationFailed,
}

pub struct PreviewGeneration {
    cache: Arc<VideoCache>,
    metadata: Arc<SpotifyMetadataClient>,
    image_client: Arc<EmbedImageClient>,
    renderer: Arc<FfmpegRenderer>,
    video_source: Arc<B2VideoSource>,
    generation_tasks: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
}

impl PreviewGeneration {
    pub fn new(
        cache: Arc<VideoCache>,
        metadata: Arc<SpotifyMetadataClient>,
        image_client: Arc<EmbedImageClient>,
        renderer: Arc<FfmpegRenderer>,
        video_source: Arc<B2VideoSource>,
    ) -> Self {
        Self {
            cache,
            metadata,
            image_client,
            renderer,
            video_source,
            generation_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn ensure_and_serve(
        &self,
        raw_track_id: &str,
    ) -> Result<ServedPreview, PreviewGenerationError> {
        let track_id = normalize_track_id(raw_track_id)?;

        if let Some(bytes) = self.cache.get_video_bytes(&track_id).await {
            return Ok(ServedPreview {
                video_bytes: bytes,
                mime: "video/mp4",
                cache_status: CacheStatus::Hit,
            });
        }

        if let Some(bytes) = self.hydrate_from_source(&track_id).await? {
            return Ok(ServedPreview {
                video_bytes: bytes,
                mime: "video/mp4",
                cache_status: CacheStatus::Hydrated,
            });
        }

        let spotify_data = self.metadata.get_track_metadata(&track_id).await?;
        let og = self.image_client.generate_track_og(&spotify_data).await?;

        self.ensure_generated(PreloadedPreviewInput {
            track_id: spotify_data.track_id,
            preview_url: spotify_data.preview_url,
            og_bytes: og.image_bytes,
        })
        .await?;

        let bytes = self
            .cache
            .get_video_bytes(&track_id)
            .await
            .ok_or(PreviewGenerationError::GenerationFailed)?;

        Ok(ServedPreview {
            video_bytes: bytes,
            mime: "video/mp4",
            cache_status: CacheStatus::Rendered,
        })
    }

    pub async fn ensure_generated(
        &self,
        input: PreloadedPreviewInput,
    ) -> Result<(), PreviewGenerationError> {
        let track_id = normalize_track_id(&input.track_id)?;

        if self.cache.has_id(&track_id).await {
            return Ok(());
        }

        if self.hydrate_from_source(&track_id).await?.is_some() {
            return Ok(());
        }

        let mut is_leader = false;
        let notify = {
            let mut tasks = self.generation_tasks.lock().await;
            if let Some(existing_notify) = tasks.get(&track_id) {
                Arc::clone(existing_notify)
            } else {
                let notify = Arc::new(Notify::new());
                tasks.insert(track_id.clone(), Arc::clone(&notify));
                is_leader = true;
                notify
            }
        };

        if !is_leader {
            notify.notified().await;
            if self.cache.has_id(&track_id).await {
                return Ok(());
            }
            if self.hydrate_from_source(&track_id).await?.is_some() {
                return Ok(());
            }
            return Err(PreviewGenerationError::GenerationFailed);
        }

        let result = self
            .render_and_cache(PreloadedPreviewInput {
                track_id: track_id.clone(),
                preview_url: input.preview_url,
                og_bytes: input.og_bytes,
            })
            .await;

        let mut tasks = self.generation_tasks.lock().await;
        if let Some(notify) = tasks.remove(&track_id) {
            notify.notify_waiters();
        }
        drop(tasks);

        result
    }

    async fn render_and_cache(
        &self,
        input: PreloadedPreviewInput,
    ) -> Result<(), PreviewGenerationError> {
        let _timer = VIDEO_GEN_DURATION.start_timer();
        let track_id = input.track_id.clone();

        let rendered_bytes = self
            .renderer
            .render_preview_video(RenderInput {
                track_id: track_id.clone(),
                preview_url: input.preview_url,
                track_og_bytes: input.og_bytes,
            })
            .await
            .inspect_err(|_| FAILED_VIDEO_GENERATIONS.inc())?;

        let _ = self
            .cache
            .cache_video_bytes(track_id.clone(), rendered_bytes.clone())
            .await;

        let video_source = self.video_source.clone();
        let renderer = self.renderer.clone();
        task::spawn(async move {
            if video_source.has_video(&track_id).await.ok() != Some(true) {
                let _ = video_source
                    .upload_video_bytes(&track_id, rendered_bytes)
                    .await;
            }
            renderer.remove_track_output(&track_id).await;
        });

        Ok(())
    }

    async fn hydrate_from_source(
        &self,
        track_id: &str,
    ) -> Result<Option<Bytes>, PreviewGenerationError> {
        match self.video_source.fetch_video_bytes(track_id).await {
            Ok(bytes) => {
                let _ = self
                    .cache
                    .cache_video_bytes(track_id.to_string(), bytes.clone())
                    .await;
                Ok(Some(bytes))
            }
            Err(VideoSourceError::NotFound(_)) => Ok(None),
            Err(error) => Err(PreviewGenerationError::Source(error)),
        }
    }
}

fn normalize_track_id(raw_track_id: &str) -> Result<String, PreviewGenerationError> {
    let value = raw_track_id
        .strip_suffix(".mp4")
        .unwrap_or(raw_track_id)
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(PreviewGenerationError::InvalidInput("track_id is empty"));
    }

    Ok(value)
}
