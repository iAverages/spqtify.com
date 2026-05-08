use axum::extract::{Path, Query, State};
use axum::http::Uri;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html as AxumHtml, IntoResponse, Redirect, Response};
use bytes::Bytes;
use serde::Deserialize;

use crate::AppState;
use crate::embeds::preview_generation::PreloadedPreviewInput;

#[derive(Deserialize)]
pub struct AlbumTrackQuery {
    track: Option<String>,
}

#[axum::debug_handler]
pub async fn get_album_page(
    Path(album_id): Path<String>,
    Query(query): Query<AlbumTrackQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !is_discord_request(&headers) {
        let redirect_url = format!("https://open.spotify.com/album/{album_id}");
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let album_data = state
        .spotify_metadata
        .get_album_track_metadata(&album_id, query.track.as_deref())
        .await
        .map_err(map_preview_error)?;

    let og = state
        .image_client
        .generate_track_og(&album_data.track)
        .await
        .map_err(map_preview_error)?;

    let title = if album_data.album_name.trim().is_empty() {
        album_data.track.song_name.clone()
    } else {
        format!("{} - {}", album_data.track.song_name, album_data.album_name)
    };

    state
        .preview_generation
        .ensure_generated(PreloadedPreviewInput {
            track_id: album_data.track.media_id.clone(),
            preview_url: album_data.track.preview_url.clone(),
            og_bytes: og.image_bytes,
        })
        .await
        .map_err(map_preview_error)?;

    let canonical_path = format!("/album/{album_id}");
    let album_image_url = format!(
        "{}/api/generate/image/album/{}?track={}",
        state.app_url.trim_end_matches('/'),
        album_id,
        album_data.selected_track_index + 1
    );
    let block = build_preview_meta_page(
        &title,
        &canonical_path,
        &album_data.track.media_id,
        &og.theme_color,
        &state.app_url,
        &album_image_url,
    );

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
pub async fn get_track_page(
    Path(track_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !is_discord_request(&headers) {
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
    let block = build_preview_meta_page(
        &spotify_data.song_name,
        &canonical_path,
        &spotify_data.media_id,
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
    if !is_discord_request(&headers) {
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
    let block = build_preview_meta_page(
        &title,
        &canonical_path,
        &spotify_data.media_id,
        &og.theme_color,
        &state.app_url,
        &image_url,
    );

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
pub async fn get_fallback_redirect(uri: Uri) -> impl IntoResponse {
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
    Path(album_id): Path<String>,
    Query(query): Query<AlbumTrackQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let album_data = state
        .spotify_metadata
        .get_album_track_metadata(&album_id, query.track.as_deref())
        .await
        .map_err(map_preview_error)?;

    let og = if album_data
        .track_names
        .iter()
        .all(|name| name.trim().is_empty())
    {
        state
            .image_client
            .generate_track_og(&album_data.track)
            .await
            .map_err(map_preview_error)?
    } else {
        match state.image_client.generate_album_og(&album_data).await {
            Ok(image) => image,
            Err(error) => {
                tracing::warn!(
                    "falling back to single-track album image for {}: {}",
                    album_id,
                    error
                );
                state
                    .image_client
                    .generate_track_og(&album_data.track)
                    .await
                    .map_err(map_preview_error)?
            }
        }
    };

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
    let served = state
        .preview_generation
        .ensure_and_serve(&track_id)
        .await
        .map_err(map_preview_error)?;
    tracing::debug!("preview served via {:?}", served.cache_status);
    Ok(build_video_response(served.video_bytes, served.mime))
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

fn build_preview_meta_page(
    title: &str,
    canonical_path: &str,
    media_track_id: &str,
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
            "<meta property=\"og:video\" content=\"{app_url}/api/generate/video/{media_track_id}.mp4\">",
            "<meta property=\"og:video:type\" content=\"video/mp4\">",
            "<meta property=\"og:video:height\" content=\"300\">",
            "<meta property=\"og:video:width\" content=\"800\">",
            "<meta property=\"og:video:secure_url\" content=\"{app_url}/api/generate/video/{media_track_id}.mp4\">",
            "<meta name=\"twitter:card\" content=\"summary_large_image\">",
            "<meta name=\"twitter:title\" content=\"{title}\">",
            "<meta name=\"twitter:image\" content=\"{image_url}\">",
            "</head>",
            "<body></body>",
            "</html>"
        ),
        title = title,
        canonical_path = canonical_path,
        media_track_id = media_track_id,
        theme_color = theme_color,
        app_url = app_url,
        image_url = image_url,
    )
}

#[cfg(test)]
mod tests {
    use super::build_passthrough_redirect;
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
}
