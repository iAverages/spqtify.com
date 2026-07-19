use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use tokio::sync::RwLock;

use crate::metrics::{VIDEO_CACHE_CURRENT_BYTES, VIDEO_CACHE_EVICTIONS_TOTAL};

type Cache = Arc<RwLock<CacheState>>;

pub struct VideoCache {
    cache: Cache,
}

impl VideoCache {
    pub fn new(max_size_bytes: u64) -> VideoCache {
        VideoCache {
            cache: Arc::new(RwLock::new(CacheState {
                entries: HashMap::new(),
                current_size_bytes: 0,
                max_size_bytes,
            })),
        }
    }

    pub async fn has_id(&self, video_id: &str) -> bool {
        self.cache.read().await.entries.contains_key(video_id)
    }

    pub async fn get_video_bytes(&self, video_id: &str) -> Option<Bytes> {
        let mut state = self.cache.write().await;
        let cached = state.entries.get_mut(video_id)?;
        cached.last_accessed = Instant::now();
        Some(cached.bytes.clone())
    }

    pub async fn cache_video_bytes(&self, video_id: String, video_bytes: Bytes) {
        let mut state = self.cache.write().await;
        let incoming_size = video_bytes.len() as u64;

        if state.max_size_bytes == 0 || incoming_size > state.max_size_bytes {
            return;
        }

        if let Some(previous) = state.entries.insert(
            video_id.clone(),
            CachedVideoData {
                bytes: video_bytes,
                last_accessed: Instant::now(),
            },
        ) {
            state.current_size_bytes -= previous.bytes.len() as u64;
        }
        state.current_size_bytes += incoming_size;

        let mut evictions = 0_u64;
        if state.current_size_bytes > state.max_size_bytes {
            let mut candidates = state
                .entries
                .iter()
                .filter(|(id, _)| **id != video_id)
                .map(|(id, video)| (id.clone(), video.last_accessed))
                .collect::<Vec<(String, Instant)>>();

            candidates.sort_by_key(|(_, last_accessed)| *last_accessed);

            for (id, _) in candidates {
                if state.current_size_bytes <= state.max_size_bytes {
                    break;
                }

                if let Some(removed) = state.entries.remove(&id) {
                    state.current_size_bytes -= removed.bytes.len() as u64;
                    evictions += 1;
                }
            }
        }

        VIDEO_CACHE_CURRENT_BYTES.set(state.current_size_bytes as i64);
        if evictions > 0 {
            VIDEO_CACHE_EVICTIONS_TOTAL.inc_by(evictions);
        }
    }
}

struct CacheState {
    entries: HashMap<String, CachedVideoData>,
    current_size_bytes: u64,
    max_size_bytes: u64,
}

#[derive(Clone, Debug)]
struct CachedVideoData {
    bytes: Bytes,
    last_accessed: Instant,
}

#[cfg(test)]
mod tests {
    use super::VideoCache;
    use bytes::Bytes;
    use tokio::time::{Duration, sleep};

    fn bytes_with_size(size: usize) -> Bytes {
        Bytes::from(vec![0; size])
    }

    #[tokio::test]
    async fn evicts_least_recently_accessed_when_over_capacity() {
        let cache = VideoCache::new(10);

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(4))
            .await;
        sleep(Duration::from_millis(2)).await;
        cache
            .cache_video_bytes("b".to_string(), bytes_with_size(4))
            .await;
        sleep(Duration::from_millis(2)).await;

        let _ = cache.get_video_bytes("a").await;
        sleep(Duration::from_millis(2)).await;

        cache
            .cache_video_bytes("c".to_string(), bytes_with_size(4))
            .await;

        assert!(cache.has_id("a").await);
        assert!(!cache.has_id("b").await);
        assert!(cache.has_id("c").await);
    }

    #[tokio::test]
    async fn skips_oversized_entries() {
        let cache = VideoCache::new(10);

        cache
            .cache_video_bytes("large".to_string(), bytes_with_size(11))
            .await;

        assert!(!cache.has_id("large").await);
        assert!(cache.get_video_bytes("large").await.is_none());
    }

    #[tokio::test]
    async fn disabled_cache_always_misses_and_writes_noop() {
        let cache = VideoCache::new(0);

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(4))
            .await;

        assert!(!cache.has_id("a").await);
        assert!(cache.get_video_bytes("a").await.is_none());
    }

    #[tokio::test]
    async fn upsert_replaces_and_keeps_new_entry_under_pressure() {
        let cache = VideoCache::new(10);

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(4))
            .await;
        sleep(Duration::from_millis(2)).await;
        cache
            .cache_video_bytes("b".to_string(), bytes_with_size(4))
            .await;
        sleep(Duration::from_millis(2)).await;

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(7))
            .await;

        assert!(cache.has_id("a").await);
        assert!(!cache.has_id("b").await);
        assert_eq!(cache.get_video_bytes("a").await.unwrap().len(), 7);
    }
}
