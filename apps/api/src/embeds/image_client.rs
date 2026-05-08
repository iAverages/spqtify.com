use bytes::Bytes;

use crate::MACHINA_CONFIG;
use crate::embeds::spotify_metadata::SpotifyAlbumTrackMetadata;
use crate::embeds::spotify_metadata::SpotifyPreviewMetadata;

#[derive(Debug, thiserror::Error)]
pub enum OgImageError {
    #[error("failed to fetch generated image")]
    RequestFailed,

    #[error("image service returned non-success status")]
    BadStatus,
}

pub struct GeneratedOgImage {
    pub image_bytes: Bytes,
    pub theme_color: String,
}

pub struct EmbedImageClient;

impl EmbedImageClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate_track_og(
        &self,
        spotify_data: &SpotifyPreviewMetadata,
    ) -> Result<GeneratedOgImage, OgImageError> {
        tracing::debug!(
            media_id = spotify_data.media_id.as_str(),
            "requesting track og image"
        );
        self.send_image_request(
            "/image",
            serde_json::json!({
                "albumArt": spotify_data.album_art_url,
                "songName": spotify_data.song_name,
                "artist": spotify_data.artist_text(),
            }),
        )
        .await
    }

    pub async fn generate_album_og(
        &self,
        album_data: &SpotifyAlbumTrackMetadata,
    ) -> Result<GeneratedOgImage, OgImageError> {
        tracing::debug!(
            media_id = album_data.track.media_id.as_str(),
            selected_track_index = album_data.selected_track_index,
            "requesting album og image"
        );
        self.send_image_request(
            "/image/album",
            serde_json::json!({
                "albumArt": album_data.album_art_url,
                "titleText": album_data.track.song_name,
                "artistText": album_data.track.artist_text(),
                "tracks": album_data.track_names,
                "currentTrackIndex": album_data.selected_track_index,
            }),
        )
        .await
    }

    async fn send_image_request(
        &self,
        endpoint: &str,
        payload: serde_json::Value,
    ) -> Result<GeneratedOgImage, OgImageError> {
        let base_url = MACHINA_CONFIG.embed_image_service_url.trim_end_matches('/');
        tracing::debug!(endpoint = endpoint, "sending og image request");
        let response = reqwest::Client::new()
            .post(format!("{base_url}{endpoint}"))
            .json(&payload)
            .send()
            .await
            .map_err(|_| OgImageError::RequestFailed)?;

        if !response.status().is_success() {
            tracing::warn!(
                endpoint = endpoint,
                status = response.status().as_u16(),
                "og image service returned non-success status"
            );
            return Err(OgImageError::BadStatus);
        }

        let theme_color = response
            .headers()
            .get("X-Basecolor")
            .and_then(|value| value.to_str().ok())
            .unwrap_or(MACHINA_CONFIG.default_base_color.as_str())
            .to_string();
        let image_bytes = response
            .bytes()
            .await
            .map_err(|_| OgImageError::RequestFailed)?;

        tracing::debug!(
            endpoint = endpoint,
            image_size_bytes = image_bytes.len(),
            "og image response received"
        );

        if image_bytes.is_empty() {
            return Err(OgImageError::RequestFailed);
        }

        if theme_color.trim().is_empty() {
            return Err(OgImageError::RequestFailed);
        }

        Ok(GeneratedOgImage {
            image_bytes,
            theme_color,
        })
    }
}
