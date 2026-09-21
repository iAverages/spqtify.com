use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use tokio::sync::RwLock;

use crate::analytics::Analytics;
use crate::embeds::VideoKind;

pub struct VideoCache {
    cache: BoundedCache<CachedVideoData>,
    analytics: Arc<Analytics>,
}

impl VideoCache {
    pub fn new(max_size_bytes: u64, analytics: Arc<Analytics>) -> VideoCache {
        VideoCache {
            cache: BoundedCache::new(max_size_bytes),
            analytics,
        }
    }

    pub async fn has_id(&self, video_id: &str) -> bool {
        self.cache.contains_key(video_id).await
    }

    pub async fn get_video_bytes(&self, video_id: &str) -> Option<(Bytes, VideoKind)> {
        let cached = self.cache.get(video_id).await?;
        Some((cached.bytes, cached.video_kind))
    }

    pub async fn cache_video_bytes(
        &self,
        video_id: String,
        video_bytes: Bytes,
        video_kind: VideoKind,
    ) {
        let incoming_size = video_bytes.len() as u64;
        match self
            .cache
            .insert(
                video_id,
                incoming_size,
                CachedVideoData {
                    bytes: video_bytes,
                    video_kind,
                },
            )
            .await
        {
            CacheInsertResult::Disabled => {
                self.analytics
                    .video_cache_write_skipped("disabled", incoming_size, 0);
            }
            CacheInsertResult::Oversized { max_size_bytes } => {
                self.analytics.video_cache_write_skipped(
                    "oversized",
                    incoming_size,
                    max_size_bytes,
                );
            }
            CacheInsertResult::Stored {
                current_size_bytes,
                max_size_bytes,
                entry_count,
                replaced_existing,
                evictions,
            } => {
                self.analytics.video_cache_updated(
                    current_size_bytes,
                    max_size_bytes,
                    entry_count,
                    incoming_size,
                    replaced_existing,
                );
                if evictions > 0 {
                    self.analytics.video_cache_evicted(
                        evictions,
                        current_size_bytes,
                        max_size_bytes,
                        entry_count,
                    );
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct MetadataCache {
    cache: BoundedCache<Bytes>,
}

impl MetadataCache {
    pub fn new(max_size_bytes: u64) -> Self {
        Self {
            cache: BoundedCache::new(max_size_bytes),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Bytes> {
        self.cache.get(key).await
    }

    pub async fn insert(&self, key: String, bytes: Bytes) {
        let size = bytes.len() as u64;
        let _ = self.cache.insert(key, size, bytes).await;
    }
}

#[derive(Clone)]
struct BoundedCache<T> {
    state: Arc<RwLock<CacheState<T>>>,
}

impl<T: Clone> BoundedCache<T> {
    fn new(max_size_bytes: u64) -> Self {
        Self {
            state: Arc::new(RwLock::new(CacheState {
                entries: HashMap::new(),
                current_size_bytes: 0,
                max_size_bytes,
            })),
        }
    }

    async fn contains_key(&self, key: &str) -> bool {
        self.state.read().await.entries.contains_key(key)
    }

    async fn get(&self, key: &str) -> Option<T> {
        let mut state = self.state.write().await;
        let cached = state.entries.get_mut(key)?;
        cached.last_accessed = Instant::now();
        Some(cached.value.clone())
    }

    async fn insert(&self, key: String, size_bytes: u64, value: T) -> CacheInsertResult {
        let mut state = self.state.write().await;
        if state.max_size_bytes == 0 {
            return CacheInsertResult::Disabled;
        }
        if size_bytes > state.max_size_bytes {
            return CacheInsertResult::Oversized {
                max_size_bytes: state.max_size_bytes,
            };
        }

        let replaced_existing = if let Some(previous) = state.entries.insert(
            key.clone(),
            CachedData {
                value,
                size_bytes,
                last_accessed: Instant::now(),
            },
        ) {
            state.current_size_bytes -= previous.size_bytes;
            true
        } else {
            false
        };
        state.current_size_bytes += size_bytes;

        let mut evictions = 0_u64;
        if state.current_size_bytes > state.max_size_bytes {
            let mut candidates = state
                .entries
                .iter()
                .filter(|(cached_key, _)| **cached_key != key)
                .map(|(cached_key, entry)| (cached_key.clone(), entry.last_accessed))
                .collect::<Vec<(String, Instant)>>();

            candidates.sort_by_key(|(_, last_accessed)| *last_accessed);

            for (id, _) in candidates {
                if state.current_size_bytes <= state.max_size_bytes {
                    break;
                }

                if let Some(removed) = state.entries.remove(&id) {
                    state.current_size_bytes -= removed.size_bytes;
                    evictions += 1;
                }
            }
        }

        let current_size_bytes = state.current_size_bytes;
        let max_size_bytes = state.max_size_bytes;
        let entry_count = state.entries.len();
        CacheInsertResult::Stored {
            current_size_bytes,
            max_size_bytes,
            entry_count,
            replaced_existing,
            evictions,
        }
    }
}

struct CacheState<T> {
    entries: HashMap<String, CachedData<T>>,
    current_size_bytes: u64,
    max_size_bytes: u64,
}

enum CacheInsertResult {
    Disabled,
    Oversized {
        max_size_bytes: u64,
    },
    Stored {
        current_size_bytes: u64,
        max_size_bytes: u64,
        entry_count: usize,
        replaced_existing: bool,
        evictions: u64,
    },
}

#[derive(Clone, Debug)]
struct CachedVideoData {
    bytes: Bytes,
    video_kind: VideoKind,
}

struct CachedData<T> {
    value: T,
    size_bytes: u64,
    last_accessed: Instant,
}

#[cfg(test)]
mod tests {
    use super::{MetadataCache, VideoCache};
    use crate::analytics::Analytics;
    use crate::embeds::VideoKind;
    use bytes::Bytes;
    use std::sync::Arc;
    use tokio::time::{Duration, sleep};

    fn bytes_with_size(size: usize) -> Bytes {
        Bytes::from(vec![0; size])
    }

    async fn test_cache(max_size_bytes: u64) -> VideoCache {
        let analytics = Arc::new(Analytics::new("", "https://us.i.posthog.com").await);
        VideoCache::new(max_size_bytes, analytics)
    }

    #[tokio::test]
    async fn evicts_least_recently_accessed_when_over_capacity() {
        let cache = test_cache(10).await;

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(4), VideoKind::Track)
            .await;
        sleep(Duration::from_millis(2)).await;
        cache
            .cache_video_bytes("b".to_string(), bytes_with_size(4), VideoKind::Track)
            .await;
        sleep(Duration::from_millis(2)).await;

        let _ = cache.get_video_bytes("a").await;
        sleep(Duration::from_millis(2)).await;

        cache
            .cache_video_bytes("c".to_string(), bytes_with_size(4), VideoKind::Track)
            .await;

        assert!(cache.has_id("a").await);
        assert!(!cache.has_id("b").await);
        assert!(cache.has_id("c").await);
    }

    #[tokio::test]
    async fn skips_oversized_entries() {
        let cache = test_cache(10).await;

        cache
            .cache_video_bytes("large".to_string(), bytes_with_size(11), VideoKind::Track)
            .await;

        assert!(!cache.has_id("large").await);
        assert!(cache.get_video_bytes("large").await.is_none());
    }

    #[tokio::test]
    async fn disabled_cache_always_misses_and_writes_noop() {
        let cache = test_cache(0).await;

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(4), VideoKind::Track)
            .await;

        assert!(!cache.has_id("a").await);
        assert!(cache.get_video_bytes("a").await.is_none());
    }

    #[tokio::test]
    async fn upsert_replaces_and_keeps_new_entry_under_pressure() {
        let cache = test_cache(10).await;

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(4), VideoKind::Track)
            .await;
        sleep(Duration::from_millis(2)).await;
        cache
            .cache_video_bytes("b".to_string(), bytes_with_size(4), VideoKind::Track)
            .await;
        sleep(Duration::from_millis(2)).await;

        cache
            .cache_video_bytes("a".to_string(), bytes_with_size(7), VideoKind::Episode)
            .await;

        assert!(cache.has_id("a").await);
        assert!(!cache.has_id("b").await);
        let (bytes, video_kind) = cache.get_video_bytes("a").await.unwrap();
        assert_eq!(bytes.len(), 7);
        assert_eq!(video_kind, VideoKind::Episode);
    }

    #[tokio::test]
    async fn metadata_cache_uses_the_same_lru_eviction() {
        let cache = MetadataCache::new(10);
        cache.insert("a".to_string(), bytes_with_size(4)).await;
        sleep(Duration::from_millis(2)).await;
        cache.insert("b".to_string(), bytes_with_size(4)).await;
        sleep(Duration::from_millis(2)).await;
        let _ = cache.get("a").await;
        sleep(Duration::from_millis(2)).await;

        cache.insert("c".to_string(), bytes_with_size(4)).await;

        assert!(cache.get("a").await.is_some());
        assert!(cache.get("b").await.is_none());
        assert!(cache.get("c").await.is_some());
    }

    #[tokio::test]
    async fn disabled_metadata_cache_always_misses() {
        let cache = MetadataCache::new(0);
        cache.insert("a".to_string(), bytes_with_size(4)).await;

        assert!(cache.get("a").await.is_none());
    }
}
