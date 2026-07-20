use anyhow::{Result, anyhow};
use scraper::{Html, Selector};
use serde::Deserialize;
use serde::de::DeserializeOwned;

const SPOTIFY_EMBED_TRACK_LIMIT: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum SpotifyMetadataError {
    #[error("failed to fetch spotify metadata")]
    RequestFailed,

    #[error("spotify metadata missing required fields")]
    MissingData,
}

#[derive(Clone, Debug)]
pub struct SpotifyPreviewMetadata {
    pub media_id: String,
    pub song_name: String,
    pub artist_names: Vec<String>,
    pub preview_url: String,
    pub album_art_url: String,
}

impl SpotifyPreviewMetadata {
    pub fn artist_text(&self) -> String {
        self.artist_names.join(", ")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotifyCollectionKind {
    Album,
    Playlist,
}

impl SpotifyCollectionKind {
    pub fn path_segment(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::Playlist => "playlist",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpotifyCollectionTrackMetadata {
    pub collection_name: String,
    pub collection_creator: String,
    pub artwork_url: String,
    pub selected_track_index: usize,
    pub track_names: Vec<String>,
    pub track: SpotifyPreviewMetadata,
}

impl SpotifyCollectionTrackMetadata {
    pub fn track_count_is_capped(&self) -> bool {
        self.track_names.len() == SPOTIFY_EMBED_TRACK_LIMIT
    }
}

pub struct SpotifyMetadataClient;

impl SpotifyMetadataClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_track_metadata(
        &self,
        track_id: &str,
    ) -> Result<SpotifyPreviewMetadata, SpotifyMetadataError> {
        tracing::debug!(track_id = track_id, "fetching spotify track metadata");
        let json: TrackRoot = self
            .fetch_spotify_embed_json(format!("https://open.spotify.com/embed/track/{track_id}"))
            .await
            .map_err(|_| SpotifyMetadataError::RequestFailed)?;

        let metadata = normalize_track_metadata(track_id, json)
            .map_err(|_| SpotifyMetadataError::MissingData)?;
        tracing::debug!(track_id = track_id, "spotify track metadata fetched");
        Ok(metadata)
    }

    pub async fn get_episode_metadata(
        &self,
        episode_id: &str,
    ) -> Result<SpotifyPreviewMetadata, SpotifyMetadataError> {
        tracing::debug!(episode_id = episode_id, "fetching spotify episode metadata");
        let json: EpisodeRoot = self
            .fetch_spotify_embed_json(format!(
                "https://open.spotify.com/embed/episode/{episode_id}"
            ))
            .await
            .map_err(|_| SpotifyMetadataError::RequestFailed)?;

        let metadata = normalize_episode_metadata(episode_id, json)
            .map_err(|_| SpotifyMetadataError::MissingData)?;
        tracing::debug!(episode_id = episode_id, "spotify episode metadata fetched");
        Ok(metadata)
    }

    pub async fn get_track_or_episode_metadata(
        &self,
        media_id: &str,
    ) -> Result<SpotifyPreviewMetadata, SpotifyMetadataError> {
        tracing::debug!(media_id = media_id, "resolving spotify media metadata");
        if let Ok(track) = self.get_track_metadata(media_id).await {
            tracing::debug!(media_id = media_id, "resolved spotify media as track");
            return Ok(track);
        }

        tracing::debug!(
            media_id = media_id,
            "falling back to spotify episode metadata"
        );
        self.get_episode_metadata(media_id).await
    }

    pub async fn get_collection_track_metadata(
        &self,
        collection_kind: SpotifyCollectionKind,
        collection_id: &str,
        raw_track: Option<&str>,
    ) -> Result<SpotifyCollectionTrackMetadata, SpotifyMetadataError> {
        tracing::debug!(
            collection_kind = collection_kind.path_segment(),
            collection_id = collection_id,
            requested_track = raw_track.unwrap_or(""),
            "fetching spotify collection metadata"
        );
        let json: CollectionRoot = self
            .fetch_spotify_embed_json(format!(
                "https://open.spotify.com/embed/{}/{collection_id}",
                collection_kind.path_segment()
            ))
            .await
            .map_err(|_| SpotifyMetadataError::RequestFailed)?;

        let metadata = normalize_collection_metadata(raw_track, json)
            .map_err(|_| SpotifyMetadataError::MissingData)?;

        tracing::debug!(
            collection_kind = collection_kind.path_segment(),
            collection_id = collection_id,
            selected_track_index = metadata.selected_track_index,
            "spotify collection metadata fetched"
        );
        Ok(metadata)
    }

    async fn fetch_spotify_embed_json<T: DeserializeOwned>(&self, url: String) -> Result<T> {
        tracing::debug!(url = url.as_str(), "requesting spotify embed page");
        let response = reqwest::get(url).await?;
        tracing::debug!(
            status = response.status().as_u16(),
            "spotify embed response received"
        );
        let html_content = response.text().await?;
        tracing::debug!(html_len = html_content.len(), "spotify embed html loaded");
        let json_text = extract_spotify_next_data_json(&html_content)?;
        tracing::debug!(json_len = json_text.len(), "spotify embed json extracted");
        serde_json::from_str(&json_text).map_err(Into::into)
    }
}

fn normalize_collection_metadata(
    raw_track: Option<&str>,
    root: CollectionRoot,
) -> Result<SpotifyCollectionTrackMetadata> {
    let entity = root.props.page_props.state.data.entity;
    let track_index = resolve_requested_track_index(raw_track, entity.track_list.len())
        .ok_or_else(|| anyhow!("missing tracks"))?;

    let artwork_url = entity
        .visual_identity
        .image
        .iter()
        .max_by_key(|image| image.max_width)
        .map(|image| image.url.trim().to_string())
        .filter(|url| !url.is_empty())
        .ok_or_else(|| anyhow!("missing artwork"))?;

    let track_names = entity
        .track_list
        .iter()
        .map(|track| track.title.trim().to_string())
        .collect::<Vec<_>>();

    if track_names.iter().all(|name| name.is_empty()) {
        return Err(anyhow!("missing track names"));
    }

    let track = entity
        .track_list
        .get(track_index)
        .ok_or_else(|| anyhow!("missing selected track"))?;

    let track_id = track
        .uri
        .rsplit_once(':')
        .map(|(_, tail)| tail)
        .ok_or_else(|| anyhow!("missing track id"))?;

    let song_name = track.title.trim().to_string();
    if song_name.is_empty() {
        return Err(anyhow!("missing song title"));
    }

    let artist_names = parse_artist_names(&track.subtitle)
        .into_iter()
        .map(|artist| artist.trim().to_string())
        .filter(|artist| !artist.is_empty())
        .collect::<Vec<_>>();
    if artist_names.is_empty() {
        return Err(anyhow!("missing artist names"));
    }

    let preview_url = track.audio_preview.url.trim().to_string();
    if preview_url.is_empty() {
        return Err(anyhow!("missing preview url"));
    }

    let track_metadata = SpotifyPreviewMetadata {
        media_id: track_id.to_string(),
        song_name,
        artist_names,
        preview_url,
        album_art_url: artwork_url.clone(),
    };

    Ok(SpotifyCollectionTrackMetadata {
        collection_name: entity.title.trim().to_string(),
        collection_creator: entity.subtitle.trim().to_string(),
        artwork_url,
        selected_track_index: track_index,
        track_names,
        track: track_metadata,
    })
}

fn resolve_requested_track_index(raw_track: Option<&str>, track_count: usize) -> Option<usize> {
    if track_count == 0 {
        return None;
    }

    let requested_index = raw_track
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.saturating_sub(1))
        .unwrap_or(0);

    Some(requested_index.min(track_count - 1))
}

fn normalize_track_metadata(track_id: &str, root: TrackRoot) -> Result<SpotifyPreviewMetadata> {
    let entity = root.props.page_props.state.data.entity;
    let song_name = entity.title.trim().to_string();
    if song_name.is_empty() {
        return Err(anyhow!("missing song title"));
    }

    let artist_names = entity
        .artists
        .into_iter()
        .map(|artist| artist.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if artist_names.is_empty() {
        return Err(anyhow!("missing artist names"));
    }

    let preview_url = entity.audio_preview.url.trim().to_string();
    if preview_url.is_empty() {
        return Err(anyhow!("missing preview url"));
    }

    let album_art_url = entity
        .visual_identity
        .image
        .iter()
        .max_by_key(|image| image.max_width)
        .map(|image| image.url.trim().to_string())
        .filter(|url| !url.is_empty())
        .ok_or(anyhow!("missing album art"))?;

    Ok(SpotifyPreviewMetadata {
        media_id: track_id.to_string(),
        song_name,
        artist_names,
        preview_url,
        album_art_url,
    })
}

fn normalize_episode_metadata(
    episode_id: &str,
    root: EpisodeRoot,
) -> Result<SpotifyPreviewMetadata> {
    let entity = root.props.page_props.state.data.entity;

    let title = entity.title.trim().to_string();
    if title.is_empty() {
        return Err(anyhow!("missing episode title"));
    }

    let show_name = entity.subtitle.trim().to_string();
    if show_name.is_empty() {
        return Err(anyhow!("missing show name"));
    }

    let preview_url = entity.audio_preview.url.trim().to_string();
    if preview_url.is_empty() {
        return Err(anyhow!("missing preview url"));
    }

    let album_art_url = entity
        .visual_identity
        .image
        .iter()
        .max_by_key(|image| image.max_width)
        .map(|image| image.url.trim().to_string())
        .filter(|url| !url.is_empty())
        .ok_or(anyhow!("missing cover art"))?;

    let normalized_id =
        parse_media_id_from_uri(&entity.uri).unwrap_or_else(|| episode_id.to_string());

    Ok(SpotifyPreviewMetadata {
        media_id: normalized_id,
        song_name: title,
        artist_names: vec![show_name],
        preview_url,
        album_art_url,
    })
}

fn parse_media_id_from_uri(uri: &str) -> Option<String> {
    uri.rsplit_once(':')
        .map(|(_, tail)| tail.trim().to_string())
        .filter(|id| !id.is_empty())
}

fn parse_artist_names(subtitle: &str) -> Vec<String> {
    subtitle
        .replace('\u{a0}', " ")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn extract_spotify_next_data_json(html_content: &str) -> Result<String> {
    let document = Html::parse_document(html_content);
    let selector = Selector::parse("#__NEXT_DATA__").map_err(|_| anyhow!("selector"))?;
    let element = document
        .select(&selector)
        .next()
        .ok_or(anyhow!("failed to find __NEXT_DATA__"))?;
    Ok(element.text().collect::<Vec<_>>().concat())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackRoot {
    props: TrackProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackProps {
    page_props: TrackPageProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackPageProps {
    state: TrackState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackState {
    data: TrackData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackData {
    entity: TrackEntity,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackEntity {
    title: String,
    artists: Vec<TrackArtist>,
    audio_preview: TrackAudioPreview,
    visual_identity: TrackVisualIdentity,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackArtist {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackAudioPreview {
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackVisualIdentity {
    image: Vec<TrackImage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackImage {
    url: String,
    max_width: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionRoot {
    props: CollectionProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionProps {
    page_props: CollectionPageProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionPageProps {
    state: CollectionState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionState {
    data: CollectionData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionData {
    entity: CollectionEntity,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionEntity {
    title: String,
    subtitle: String,
    track_list: Vec<CollectionTrack>,
    visual_identity: TrackVisualIdentity,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollectionTrack {
    uri: String,
    title: String,
    subtitle: String,
    audio_preview: TrackAudioPreview,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeRoot {
    props: EpisodeProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeProps {
    page_props: EpisodePageProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodePageProps {
    state: EpisodeState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeState {
    data: EpisodeData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeData {
    entity: EpisodeEntity,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeEntity {
    uri: String,
    title: String,
    subtitle: String,
    audio_preview: TrackAudioPreview,
    visual_identity: TrackVisualIdentity,
}

#[cfg(test)]
mod tests {
    use super::{
        CollectionRoot, normalize_collection_metadata, parse_artist_names,
        resolve_requested_track_index,
    };
    use serde_json::json;

    #[test]
    fn resolve_requested_track_index_defaults_to_first_track() {
        assert_eq!(resolve_requested_track_index(None, 4), Some(0));
        assert_eq!(resolve_requested_track_index(Some(""), 4), Some(0));
        assert_eq!(resolve_requested_track_index(Some("0"), 4), Some(0));
    }

    #[test]
    fn resolve_requested_track_index_clamps_to_valid_bounds() {
        assert_eq!(resolve_requested_track_index(Some("2"), 4), Some(1));
        assert_eq!(resolve_requested_track_index(Some("99"), 4), Some(3));
    }

    #[test]
    fn resolve_requested_track_index_returns_none_when_no_tracks() {
        assert_eq!(resolve_requested_track_index(Some("1"), 0), None);
    }

    #[test]
    fn parse_artist_names_splits_comma_separated_subtitle() {
        let artists = parse_artist_names("Pitbull,\u{a0}Christina Aguilera");
        assert_eq!(artists, vec!["Pitbull", "Christina Aguilera"]);
    }

    #[test]
    fn playlist_metadata_uses_selected_track_audio_preview() {
        let root: CollectionRoot = serde_json::from_value(json!({
            "props": {
                "pageProps": {
                    "state": {
                        "data": {
                            "entity": {
                                "title": "Public playlist",
                                "subtitle": "Playlist curator",
                                "trackList": [
                                    {
                                        "uri": "spotify:track:first",
                                        "title": "First song",
                                        "subtitle": "First artist",
                                        "audioPreview": { "url": "https://preview/first.mp3" }
                                    },
                                    {
                                        "uri": "spotify:track:second",
                                        "title": "Second song",
                                        "subtitle": "Second artist",
                                        "audioPreview": { "url": "https://preview/second.mp3" }
                                    }
                                ],
                                "visualIdentity": {
                                    "image": [
                                        { "url": "https://image/small.jpg", "maxWidth": 64 },
                                        { "url": "https://image/large.jpg", "maxWidth": 640 }
                                    ]
                                }
                            }
                        }
                    }
                }
            }
        }))
        .unwrap();

        let metadata = normalize_collection_metadata(Some("2"), root).unwrap();

        assert_eq!(metadata.track.preview_url, "https://preview/second.mp3");
        assert_eq!(metadata.track.media_id, "second");
        assert_eq!(metadata.artwork_url, "https://image/large.jpg");
        assert_eq!(metadata.collection_name, "Public playlist");
        assert_eq!(metadata.collection_creator, "Playlist curator");
        assert!(!metadata.track_count_is_capped());
    }
}
