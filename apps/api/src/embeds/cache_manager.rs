use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::RwLock;
use tokio::task;
use tokio::time::sleep;

#[derive(Debug, thiserror::Error)]
pub enum VideoCacheError {
    #[error("video with id {0} already exists in the cache")]
    AlreadyCached(String),
}

type Cache = Arc<RwLock<HashMap<String, CachedVideoData>>>;

pub struct VideoCache {
    cache: Cache,
}

impl VideoCache {
    pub fn new() -> VideoCache {
        VideoCache {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_cleanup_thread(&self) {
        let cloned_cache = self.cache.clone();
        task::spawn(async move {
            loop {
                VideoCache::cleanup(cloned_cache.clone()).await;
                sleep(Duration::from_mins(10)).await;
            }
        });
    }

    pub async fn has_id(&self, video_id: &str) -> bool {
        self.cache.read().await.get(video_id).is_some()
    }

    pub async fn get_video_bytes(&self, video_id: &str) -> Option<Bytes> {
        let reader = self.cache.read().await;
        let local_cache = reader.get(video_id)?;
        let bytes = local_cache.bytes.clone();
        drop(reader);

        let mut writer = self.cache.write().await;
        writer.insert(
            video_id.to_string(),
            CachedVideoData {
                bytes: bytes.clone(),
                last_accessed: Instant::now(),
            },
        );
        Some(bytes)
    }

    pub async fn cache_video_bytes(
        &self,
        video_id: String,
        video_bytes: Bytes,
    ) -> Result<(), VideoCacheError> {
        if self.has_id(&video_id).await {
            return Err(VideoCacheError::AlreadyCached(video_id));
        }

        let mut cache_write = self.cache.write().await;
        cache_write.insert(
            video_id,
            CachedVideoData {
                bytes: video_bytes,
                last_accessed: Instant::now(),
            },
        );
        Ok(())
    }

    async fn cleanup(cache: Cache) -> bool {
        let reader = cache.read().await;
        let to_remove = reader
            .iter()
            .filter(|(_, video)| {
                Instant::now().duration_since(video.last_accessed) > Duration::from_secs(5 * 60)
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<String>>();
        drop(reader);

        if to_remove.is_empty() {
            return false;
        }

        let mut cache_write = cache.write().await;
        for id in to_remove {
            cache_write.remove(&id);
        }
        cache_write.shrink_to_fit();
        true
    }
}

#[derive(Clone, Debug)]
struct CachedVideoData {
    bytes: Bytes,
    last_accessed: Instant,
}
