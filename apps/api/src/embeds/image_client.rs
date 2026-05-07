use bytes::Bytes;

use crate::MACHINA_CONFIG;
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
        let base_url = MACHINA_CONFIG.embed_image_service_url.trim_end_matches('/');
        let response = reqwest::Client::new()
            .post(format!("{base_url}/image"))
            .json(&serde_json::json!({
                "albumArt": spotify_data.album_art_url,
                "songName": spotify_data.song_name,
                "artist": spotify_data.artist_text(),
            }))
            .send()
            .await
            .map_err(|_| OgImageError::RequestFailed)?;

        if !response.status().is_success() {
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
