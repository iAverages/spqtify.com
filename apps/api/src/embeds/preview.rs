use axum::extract::{Path, Query, State};
use axum::http::Uri;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html as AxumHtml, IntoResponse, Redirect, Response};
use bytes::Bytes;
use serde::Deserialize;

use crate::AppState;
use crate::embeds::preview_generation::{CacheStatus, PreloadedPreviewInput, normalize_track_id};
use crate::embeds::spotify_metadata::{SpotifyCollectionKind, SpotifyCollectionTrackMetadata};

#[derive(Deserialize)]
pub struct CollectionTrackQuery {
    track: Option<String>,
}

#[axum::debug_handler]
pub async fn get_album_page(
    Path(collection_id): Path<String>,
    Query(query): Query<CollectionTrackQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    get_collection_page(
        SpotifyCollectionKind::Album,
        collection_id,
        query,
        state,
        headers,
    )
    .await
}

#[axum::debug_handler]
pub async fn get_playlist_page(
    Path(collection_id): Path<String>,
    Query(query): Query<CollectionTrackQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    get_collection_page(
        SpotifyCollectionKind::Playlist,
        collection_id,
        query,
        state,
        headers,
    )
    .await
}

async fn get_collection_page(
    collection_kind: SpotifyCollectionKind,
    collection_id: String,
    query: CollectionTrackQuery,
    state: AppState,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    tracing::info!(
        collection_kind = collection_kind.path_segment(),
        collection_id = collection_id.as_str(),
        requested_track = query.track.as_deref().unwrap_or(""),
        "collection page request received"
    );
    if !is_discord_request(&headers) {
        tracing::info!(
            collection_kind = collection_kind.path_segment(),
            collection_id = collection_id.as_str(),
            "collection page redirecting to spotify"
        );
        let redirect_url = spotify_collection_url(collection_kind, &collection_id);
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let collection_data = state
        .spotify_metadata
        .get_collection_track_metadata(collection_kind, &collection_id, query.track.as_deref())
        .await
        .map_err(map_preview_error)?;

    let og = build_collection_og_image(&state, &collection_data, collection_kind, &collection_id)
        .await
        .map_err(map_preview_error)?;

    let title = if collection_data.collection_name.trim().is_empty() {
        collection_data.track.song_name.clone()
    } else {
        format!(
            "{} - {}",
            collection_data.track.song_name, collection_data.collection_name
        )
    };

    let collection_video_id = build_collection_video_id(
        collection_kind,
        &collection_id,
        collection_data.selected_track_index,
        &collection_data.track.media_id,
    );

    state
        .preview_generation
        .ensure_generated(PreloadedPreviewInput {
            track_id: collection_video_id,
            preview_url: collection_data.track.preview_url.clone(),
            og_bytes: og.image_bytes,
        })
        .await
        .map_err(map_preview_error)?;

    let canonical_path = format!("/{}/{}", collection_kind.path_segment(), collection_id);
    let collection_image_url = format!(
        "{}/api/generate/image/{}/{}?track={}",
        state.app_url.trim_end_matches('/'),
        collection_kind.path_segment(),
        collection_id,
        collection_data.selected_track_index + 1
    );
    let collection_video_url = format!(
        "{}/api/generate/video/{}/{}.mp4?track={}",
        state.app_url.trim_end_matches('/'),
        collection_kind.path_segment(),
        collection_id,
        collection_data.selected_track_index + 1
    );
    let block = build_preview_meta_page(
        &title,
        &canonical_path,
        &collection_video_url,
        &og.theme_color,
        &state.app_url,
        &collection_image_url,
    );

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
pub async fn get_track_page(
    Path(track_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    tracing::info!(track_id = track_id.as_str(), "track page request received");
    if !is_discord_request(&headers) {
        tracing::info!(
            track_id = track_id.as_str(),
            "track page redirecting to spotify"
        );
        let redirect_url = format!("https://open.spotify.com/track/{track_id}");
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let spotify_data = state
        .spotify_metadata
        .get_track_metadata(&track_id)
        .await
        .map_err(map_preview_error)?;

    let og = state
        .image_client
        .generate_track_og(&spotify_data)
        .await
        .map_err(map_preview_error)?;

    state
        .preview_generation
        .ensure_generated(PreloadedPreviewInput {
            track_id: spotify_data.media_id.clone(),
            preview_url: spotify_data.preview_url.clone(),
            og_bytes: og.image_bytes,
        })
        .await
        .map_err(map_preview_error)?;

    let canonical_path = format!("/track/{track_id}");
    let image_url = format!(
        "{}/api/generate/image/{}",
        state.app_url.trim_end_matches('/'),
        spotify_data.media_id
    );
    let video_url = format!(
        "{}/api/generate/video/{}.mp4",
        state.app_url.trim_end_matches('/'),
        spotify_data.media_id
    );
    let block = build_preview_meta_page(
        &spotify_data.song_name,
        &canonical_path,
        &video_url,
        &og.theme_color,
        &state.app_url,
        &image_url,
    );

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
pub async fn get_episode_page(
    Path(episode_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    tracing::info!(
        episode_id = episode_id.as_str(),
        "episode page request received"
    );
    if !is_discord_request(&headers) {
        tracing::info!(
            episode_id = episode_id.as_str(),
            "episode page redirecting to spotify"
        );
        let redirect_url = format!("https://open.spotify.com/episode/{episode_id}");
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let spotify_data = state
        .spotify_metadata
        .get_episode_metadata(&episode_id)
        .await
        .map_err(map_preview_error)?;

    let og = state
        .image_client
        .generate_track_og(&spotify_data)
        .await
        .map_err(map_preview_error)?;

    state
        .preview_generation
        .ensure_generated(PreloadedPreviewInput {
            track_id: spotify_data.media_id.clone(),
            preview_url: spotify_data.preview_url.clone(),
            og_bytes: og.image_bytes,
        })
        .await
        .map_err(map_preview_error)?;

    let canonical_path = format!("/episode/{episode_id}");
    let title = if spotify_data.artist_names.is_empty() {
        spotify_data.song_name.clone()
    } else {
        format!(
            "{} - {}",
            spotify_data.song_name,
            spotify_data.artist_text()
        )
    };
    let image_url = format!(
        "{}/api/generate/image/{}",
        state.app_url.trim_end_matches('/'),
        spotify_data.media_id
    );
    let video_url = format!(
        "{}/api/generate/video/{}.mp4",
        state.app_url.trim_end_matches('/'),
        spotify_data.media_id
    );
    let block = build_preview_meta_page(
        &title,
        &canonical_path,
        &video_url,
        &og.theme_color,
        &state.app_url,
        &image_url,
    );

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
pub async fn get_fallback_redirect(uri: Uri) -> impl IntoResponse {
    tracing::info!(
        path = uri.path(),
        query = uri.query().unwrap_or(""),
        "fallback redirect requested"
    );
    Redirect::temporary(&build_passthrough_redirect(
        &uri,
        "",
        "https://open.spotify.com",
    ))
}

#[axum::debug_handler]
pub async fn get_generated_image(
    Path(track_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    tracing::info!(
        track_id = track_id.as_str(),
        "generated image request received"
    );
    let spotify_data = state
        .spotify_metadata
        .get_track_or_episode_metadata(&track_id)
        .await
        .map_err(map_preview_error)?;

    let og = state
        .image_client
        .generate_track_og(&spotify_data)
        .await
        .map_err(map_preview_error)?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    headers.insert("X-Basecolor", og.theme_color.parse().unwrap());

    Ok((StatusCode::OK, headers, og.image_bytes).into_response())
}

#[axum::debug_handler]
pub async fn get_generated_album_image(
    Path(collection_id): Path<String>,
    Query(query): Query<CollectionTrackQuery>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, String)> {
    get_generated_collection_image(SpotifyCollectionKind::Album, collection_id, query, state).await
}

#[axum::debug_handler]
pub async fn get_generated_playlist_image(
    Path(collection_id): Path<String>,
    Query(query): Query<CollectionTrackQuery>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, String)> {
    get_generated_collection_image(SpotifyCollectionKind::Playlist, collection_id, query, state)
        .await
}

async fn get_generated_collection_image(
    collection_kind: SpotifyCollectionKind,
    collection_id: String,
    query: CollectionTrackQuery,
    state: AppState,
) -> Result<Response, (StatusCode, String)> {
    tracing::info!(
        collection_kind = collection_kind.path_segment(),
        collection_id = collection_id.as_str(),
        requested_track = query.track.as_deref().unwrap_or(""),
        "generated collection image request received"
    );
    let collection_data = state
        .spotify_metadata
        .get_collection_track_metadata(collection_kind, &collection_id, query.track.as_deref())
        .await
        .map_err(map_preview_error)?;

    let og = build_collection_og_image(&state, &collection_data, collection_kind, &collection_id)
        .await
        .map_err(map_preview_error)?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    headers.insert("X-Basecolor", og.theme_color.parse().unwrap());

    Ok((StatusCode::OK, headers, og.image_bytes).into_response())
}

#[axum::debug_handler]
pub async fn get_preview_video(
    Path(track_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    tracing::info!(
        track_id = track_id.as_str(),
        "preview video request received"
    );
    let served = state
        .preview_generation
        .ensure_and_serve(&track_id)
        .await
        .map_err(map_preview_error)?;
    tracing::debug!(
        track_id = track_id.as_str(),
        cache_status = cache_status_label(&served.cache_status),
        "preview served"
    );
    Ok(build_video_response(served.video_bytes, served.mime))
}

#[axum::debug_handler]
pub async fn get_preview_album_video(
    Path(collection_id): Path<String>,
    Query(query): Query<CollectionTrackQuery>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, String)> {
    get_preview_collection_video(SpotifyCollectionKind::Album, collection_id, query, state).await
}

#[axum::debug_handler]
pub async fn get_preview_playlist_video(
    Path(collection_id): Path<String>,
    Query(query): Query<CollectionTrackQuery>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, String)> {
    get_preview_collection_video(SpotifyCollectionKind::Playlist, collection_id, query, state).await
}

async fn get_preview_collection_video(
    collection_kind: SpotifyCollectionKind,
    collection_id: String,
    query: CollectionTrackQuery,
    state: AppState,
) -> Result<Response, (StatusCode, String)> {
    let collection_id = normalize_track_id(&collection_id).map_err(map_preview_error)?;
    tracing::info!(
        collection_kind = collection_kind.path_segment(),
        collection_id = collection_id.as_str(),
        requested_track = query.track.as_deref().unwrap_or(""),
        "collection preview video request received"
    );

    let collection_data = state
        .spotify_metadata
        .get_collection_track_metadata(collection_kind, &collection_id, query.track.as_deref())
        .await
        .map_err(map_preview_error)?;

    let og = build_collection_og_image(&state, &collection_data, collection_kind, &collection_id)
        .await
        .map_err(map_preview_error)?;

    let collection_video_id = build_collection_video_id(
        collection_kind,
        &collection_id,
        collection_data.selected_track_index,
        &collection_data.track.media_id,
    );

    state
        .preview_generation
        .ensure_generated(PreloadedPreviewInput {
            track_id: collection_video_id.clone(),
            preview_url: collection_data.track.preview_url,
            og_bytes: og.image_bytes,
        })
        .await
        .map_err(map_preview_error)?;

    let served = state
        .preview_generation
        .ensure_and_serve(&collection_video_id)
        .await
        .map_err(map_preview_error)?;

    Ok(build_video_response(served.video_bytes, served.mime))
}

fn cache_status_label(cache_status: &CacheStatus) -> &'static str {
    match cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Hydrated => "hydrated",
        CacheStatus::Rendered => "rendered",
    }
}

fn build_video_response(video_bytes: Bytes, mime: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    (StatusCode::OK, headers, video_bytes).into_response()
}

pub fn build_passthrough_redirect(uri: &Uri, route_prefix: &str, target_base: &str) -> String {
    let path = uri
        .path()
        .strip_prefix(route_prefix)
        .unwrap_or(uri.path())
        .trim_start_matches('/');

    let mut redirect_url = if path.is_empty() {
        format!("{}/", target_base.trim_end_matches('/'))
    } else {
        format!("{}/{}", target_base.trim_end_matches('/'), path)
    };

    if let Some(query) = uri.query() {
        redirect_url.push('?');
        redirect_url.push_str(query);
    }

    redirect_url
}

fn is_discord_request(headers: &HeaderMap) -> bool {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|ua| ua.to_ascii_lowercase().contains("discord"))
        .unwrap_or(false)
}

fn map_preview_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    tracing::error!("preview pipeline error: {error}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "failed to generate preview".to_string(),
    )
}

fn build_collection_video_id(
    collection_kind: SpotifyCollectionKind,
    collection_id: &str,
    selected_track_index: usize,
    media_track_id: &str,
) -> String {
    format!(
        "{}-{collection_id}-{selected_track_index}-{media_track_id}",
        collection_kind.path_segment()
    )
}

fn spotify_collection_url(collection_kind: SpotifyCollectionKind, collection_id: &str) -> String {
    format!(
        "https://open.spotify.com/{}/{collection_id}",
        collection_kind.path_segment()
    )
}

async fn build_collection_og_image(
    state: &AppState,
    collection_data: &SpotifyCollectionTrackMetadata,
    collection_kind: SpotifyCollectionKind,
    collection_id: &str,
) -> Result<crate::embeds::image_client::GeneratedOgImage, crate::embeds::image_client::OgImageError>
{
    if collection_data
        .track_names
        .iter()
        .all(|name| name.trim().is_empty())
    {
        return state
            .image_client
            .generate_track_og(&collection_data.track)
            .await;
    }

    match state
        .image_client
        .generate_collection_og(collection_kind, collection_data)
        .await
    {
        Ok(image) => Ok(image),
        Err(error) => {
            tracing::warn!(
                "falling back to single-track image for {} {}: {}",
                collection_kind.path_segment(),
                collection_id,
                error
            );
            state
                .image_client
                .generate_track_og(&collection_data.track)
                .await
        }
    }
}

fn build_preview_meta_page(
    title: &str,
    canonical_path: &str,
    video_url: &str,
    theme_color: &str,
    app_url: &str,
    image_url: &str,
) -> String {
    let app_url = app_url.trim_end_matches('/');
    format!(
        concat!(
            "<!doctype html>",
            "<html lang=\"en\">",
            "<head>",
            "<title>{title}</title>",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<meta property=\"og:title\" content=\"{title}\">",
            "<meta property=\"og:url\" content=\"{app_url}{canonical_path}\">",
            "<meta property=\"theme-color\" content=\"{theme_color}\">",
            "<meta property=\"og:image\" content=\"{image_url}\">",
            "<meta property=\"og:type\" content=\"video\">",
            "<meta property=\"og:video\" content=\"{video_url}\">",
            "<meta property=\"og:video:type\" content=\"video/mp4\">",
            "<meta property=\"og:video:height\" content=\"300\">",
            "<meta property=\"og:video:width\" content=\"800\">",
            "<meta property=\"og:video:secure_url\" content=\"{video_url}\">",
            "<meta name=\"twitter:card\" content=\"summary_large_image\">",
            "<meta name=\"twitter:title\" content=\"{title}\">",
            "<meta name=\"twitter:image\" content=\"{image_url}\">",
            "</head>",
            "<body></body>",
            "</html>"
        ),
        title = title,
        canonical_path = canonical_path,
        video_url = video_url,
        theme_color = theme_color,
        app_url = app_url,
        image_url = image_url,
    )
}

#[cfg(test)]
mod tests {
    use super::{build_collection_video_id, build_passthrough_redirect, spotify_collection_url};
    use crate::embeds::spotify_metadata::SpotifyCollectionKind;
    use axum::http::Uri;

    #[test]
    fn passthrough_redirect_keeps_wildcard_and_query() {
        let uri: Uri = "/prerelease/track/abc123?si=xyz&foo=bar".parse().unwrap();

        let redirect = build_passthrough_redirect(&uri, "/prerelease", "https://open.spotify.com/");

        assert_eq!(
            redirect,
            "https://open.spotify.com/track/abc123?si=xyz&foo=bar"
        );
    }

    #[test]
    fn passthrough_redirect_handles_base_route() {
        let uri: Uri = "/prerelease?foo=bar".parse().unwrap();

        let redirect = build_passthrough_redirect(&uri, "/prerelease", "https://open.spotify.com");

        assert_eq!(redirect, "https://open.spotify.com/?foo=bar");
    }

    #[test]
    fn passthrough_redirect_handles_empty_prefix() {
        let uri: Uri = "/track/abc123?si=xyz".parse().unwrap();

        let redirect = build_passthrough_redirect(&uri, "", "https://open.spotify.com");

        assert_eq!(redirect, "https://open.spotify.com/track/abc123?si=xyz");
    }

    #[test]
    fn playlist_urls_point_to_spotify_playlists() {
        assert_eq!(
            spotify_collection_url(SpotifyCollectionKind::Playlist, "playlist-id"),
            "https://open.spotify.com/playlist/playlist-id"
        );
    }

    #[test]
    fn collection_video_ids_are_namespaced_by_kind() {
        assert_eq!(
            build_collection_video_id(SpotifyCollectionKind::Album, "id", 1, "track"),
            "album-id-1-track"
        );
        assert_eq!(
            build_collection_video_id(SpotifyCollectionKind::Playlist, "id", 1, "track"),
            "playlist-id-1-track"
        );
    }
}
