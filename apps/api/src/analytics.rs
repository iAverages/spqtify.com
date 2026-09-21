use posthog_rs::{Client, Event};
use serde::Serialize;

use crate::embeds::VideoKind;

pub struct Analytics {
    client: Client,
}

impl Analytics {
    pub async fn new(api_key: &str, host: &str) -> Self {
        let client = posthog_rs::client((api_key, host)).await;

        if api_key.trim().is_empty() {
            tracing::info!("posthog analytics disabled (POSTHOG_API_KEY is empty)");
        } else {
            tracing::info!(posthog_host = host, "posthog analytics enabled");
        }

        Self { client }
    }

    pub fn spotify_redirect(&self, media_type: &str, route: &str) {
        let mut event = Event::new_anon("spotify redirect");
        insert_property(&mut event, "media_type", media_type);
        insert_property(&mut event, "route", route);
        self.client.capture(event);
    }

    pub fn spotify_launch_page_served(&self, media_type: &str, route: &str) {
        let mut event = Event::new_anon("spotify launch page served");
        insert_property(&mut event, "media_type", media_type);
        insert_property(&mut event, "route", route);
        self.client.capture(event);
    }

    pub fn video_generated(
        &self,
        video_id: &str,
        video_kind: VideoKind,
        size_bytes: usize,
        duration_ms: u64,
    ) {
        let mut event = self.video_event("video generated", video_id, video_kind);
        insert_property(&mut event, "size_bytes", size_bytes);
        insert_property(&mut event, "duration_ms", duration_ms);
        self.client.capture(event);
    }

    pub fn video_generation_failed(&self, video_id: &str, video_kind: VideoKind, duration_ms: u64) {
        let mut event = self.video_event("video generation failed", video_id, video_kind);
        insert_property(&mut event, "duration_ms", duration_ms);
        insert_property(&mut event, "failure_stage", "render");
        self.client.capture(event);
    }

    pub fn video_cache_hit(&self, video_id: &str, video_kind: VideoKind, size_bytes: usize) {
        let mut event = self.video_event("video cache hit", video_id, video_kind);
        insert_property(&mut event, "size_bytes", size_bytes);
        self.client.capture(event);
    }

    pub fn video_cache_updated(
        &self,
        current_size_bytes: u64,
        max_size_bytes: u64,
        entry_count: usize,
        incoming_size_bytes: u64,
        replaced_existing: bool,
    ) {
        let mut event = Event::new_anon("video cache updated");
        insert_property(&mut event, "current_size_bytes", current_size_bytes);
        insert_property(&mut event, "max_size_bytes", max_size_bytes);
        insert_property(&mut event, "entry_count", entry_count);
        insert_property(&mut event, "incoming_size_bytes", incoming_size_bytes);
        insert_property(&mut event, "replaced_existing", replaced_existing);
        self.client.capture(event);
    }

    pub fn video_cache_evicted(
        &self,
        eviction_count: u64,
        current_size_bytes: u64,
        max_size_bytes: u64,
        entry_count: usize,
    ) {
        let mut event = Event::new_anon("video cache evicted");
        insert_property(&mut event, "eviction_count", eviction_count);
        insert_property(&mut event, "current_size_bytes", current_size_bytes);
        insert_property(&mut event, "max_size_bytes", max_size_bytes);
        insert_property(&mut event, "entry_count", entry_count);
        self.client.capture(event);
    }

    pub fn video_cache_write_skipped(
        &self,
        reason: &str,
        incoming_size_bytes: u64,
        max_size_bytes: u64,
    ) {
        let mut event = Event::new_anon("video cache write skipped");
        insert_property(&mut event, "reason", reason);
        insert_property(&mut event, "incoming_size_bytes", incoming_size_bytes);
        insert_property(&mut event, "max_size_bytes", max_size_bytes);
        self.client.capture(event);
    }

    pub fn video_source_hit(&self, video_id: &str, video_kind: VideoKind, size_bytes: usize) {
        let mut event = self.video_event("video source hit", video_id, video_kind);
        insert_property(&mut event, "size_bytes", size_bytes);
        self.client.capture(event);
    }

    pub fn video_served(
        &self,
        video_id: &str,
        video_kind: VideoKind,
        size_bytes: usize,
        cache_status: &str,
    ) {
        let mut event = self.video_event("video served", video_id, video_kind);
        insert_property(&mut event, "size_bytes", size_bytes);
        insert_property(&mut event, "cache_status", cache_status);
        self.client.capture(event);
    }

    pub async fn shutdown(&self) {
        self.client.shutdown().await;
    }

    fn video_event(&self, event_name: &str, video_id: &str, video_kind: VideoKind) -> Event {
        let mut event = Event::new_anon(event_name);
        insert_property(&mut event, "video_id", video_id);
        insert_property(&mut event, "video_kind", video_kind.as_str());
        event
    }
}

fn insert_property<T: Serialize>(event: &mut Event, key: &str, value: T) {
    if let Err(error) = event.insert_prop(key, value) {
        tracing::warn!(property = key, %error, "failed to add PostHog event property");
    }
}
