use crate::embeds::album::embed::AlbumEmbedJsonData;
use crate::embeds::track::embed::TrackEmbedJsonData;
use crate::metrics::{FAILED_VIDEO_GENERATIONS, VIDEO_GEN_DURATION};
use crate::utils::{
    get_audio_output_path, get_b2_video_path, get_og_output_path, get_track_output_path,
    get_video_output_path, upload_to_b2,
};
use crate::{AppState, MACHINA_CONFIG, get_b2};
use anyhow::{Result, anyhow};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html as AxumHtml, IntoResponse, Redirect, Response};
use backblaze_b2_client::definitions::query_params::{
    B2DownloadFileQueryParameters, B2ListFileNamesQueryParameters,
};
use bytes::Bytes;
use futures::stream::StreamExt;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::path::Path as StdPath;
use std::process::Stdio;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::task;
use tracing::instrument;

#[derive(Debug, thiserror::Error)]
pub enum B2VideoError {
    #[error("video with prefix {0} was not found")]
    NotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum LocalVideoError {
    #[error("no video found on path {0}")]
    NotFound(String),
}

pub struct LocalVideo {
    path: String,
    track_id: String,
}

impl LocalVideo {
    pub async fn new(track_id: String, path: String) -> Result<LocalVideo, LocalVideoError> {
        let path_path = std::path::Path::new(&path);
        if !path_path.exists() {
            return Err(LocalVideoError::NotFound(path));
        }

        Ok(LocalVideo { track_id, path })
    }
}

impl StoredVideo for LocalVideo {
    async fn get_file_stream_all(&self) -> Result<Bytes, StoredVideoError> {
        let mut file = File::open(self.path.clone())
            .await
            .map_err(|_| StoredVideoError::StreamError)?;
        let metadata = file
            .metadata()
            .await
            .map_err(|_| StoredVideoError::StreamError)?;
        let file_size = metadata.len() as usize;

        let mut buffer: Vec<u8> = Vec::with_capacity(file_size);

        file.read_to_end(&mut buffer).await.unwrap();

        Ok(Bytes::from(buffer))
    }

    fn get_id(&self) -> String {
        self.track_id.clone()
    }
}

pub struct B2Video {
    pub file_id: String,
    pub track_id: String,
}

impl B2Video {
    pub async fn new(track_id: String) -> Result<B2Video, B2VideoError> {
        let files = get_b2()
            .basic_client()
            .list_file_names(
                B2ListFileNamesQueryParameters::builder()
                    .bucket_id(MACHINA_CONFIG.b2_bucket_id.clone())
                    .prefix(Some(get_b2_video_path(track_id.clone())))
                    .build(),
            )
            .await
            .unwrap();

        if !files.files.is_empty() {
            let file = files.files.first().unwrap();
            tracing::info!("found video for track in b2");
            return Ok(B2Video {
                file_id: file.file_id.clone(),
                track_id,
            });
        }

        Err(B2VideoError::NotFound(track_id.clone()))
    }
}

impl StoredVideo for B2Video {
    fn get_id(&self) -> String {
        self.track_id.clone()
    }

    async fn get_file_stream_all(&self) -> Result<Bytes, StoredVideoError> {
        let video = get_b2()
            .basic_client()
            .download_file_by_id(
                self.file_id.clone(),
                Some(B2DownloadFileQueryParameters::builder().build()),
            )
            .await
            .unwrap();
        let (size, mut stream) = video.file.into_stream();

        let mut buffer: Vec<u8> = Vec::with_capacity(size);

        while let Some(value) = stream.next().await {
            let value = value.map_err(|_| StoredVideoError::StreamError)?;
            buffer.extend_from_slice(value.as_ref());
        }

        Ok(Bytes::from(buffer))
    }
}

pub trait StoredVideo {
    fn get_id(&self) -> String;

    async fn get_file_stream_all(&self) -> Result<Bytes, StoredVideoError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StoredVideoError {
    #[error("an error occured while streaming video data")]
    StreamError,
}

pub async fn serve_cached_video(
    state: AppState,
    track_id: String,
) -> Result<Response, (StatusCode, String)> {
    if let Some(bytes) = state
        .cache_manager
        .get_and_cache_video_bytes(&track_id)
        .await
    {
        Ok(build_response(bytes))
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "an error has occured".to_string(),
        ))
    }
}

struct SpotifyTrackData {
    track_id: String,
    song_name: String,
    artist_names: Vec<String>,
    preview_url: String,
    album_art_url: String,
}

struct SpotifyAlbumData {
    album_name: String,
    track_data: SpotifyTrackData,
}

#[derive(Deserialize)]
pub struct AlbumTrackQuery {
    track: Option<String>,
}

impl SpotifyTrackData {
    fn artist_text(&self) -> String {
        self.artist_names.join(", ")
    }
}

struct PreviewGenerationInputs {
    preview_url: String,
    track_og_bytes: Bytes,
}

fn generation_error() -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error generating video".to_string(),
    )
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

fn build_preview_meta_page(
    title: &str,
    canonical_path: &str,
    media_track_id: &str,
    theme_color: &str,
) -> String {
    let app_url = MACHINA_CONFIG.app_url.trim_end_matches('/');
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
            "<meta property=\"og:image\" content=\"{app_url}/api/generate/image/{media_track_id}\">",
            "<meta property=\"og:type\" content=\"video\">",
            "<meta property=\"og:video\" content=\"{app_url}/api/generate/video/{media_track_id}.mp4\">",
            "<meta property=\"og:video:type\" content=\"video/mp4\">",
            "<meta property=\"og:video:height\" content=\"300\">",
            "<meta property=\"og:video:width\" content=\"800\">",
            "<meta property=\"og:video:secure_url\" content=\"{app_url}/api/generate/video/{media_track_id}.mp4\">",
            "<meta name=\"twitter:card\" content=\"summary_large_image\">",
            "<meta name=\"twitter:title\" content=\"{title}\">",
            "<meta name=\"twitter:image\" content=\"{app_url}/api/generate/image/{media_track_id}\">",
            "</head>",
            "<body></body>",
            "</html>"
        ),
        title = title,
        canonical_path = canonical_path,
        media_track_id = media_track_id,
        theme_color = theme_color,
        app_url = app_url
    )
}

async fn write_bytes_to_output_path(full_path: String, bytes: Bytes) -> Result<File> {
    let path = StdPath::new(&full_path);
    if let Some(parent) = path.parent() {
        tracing::info!("cache folder did not exist, creating...");
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file = File::create(path).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;

    Ok(file)
}

fn extract_spotify_next_data_json(html_content: &str) -> Result<String> {
    let document = Html::parse_document(html_content);
    let selector = Selector::parse("#__NEXT_DATA__").unwrap();
    let element = document
        .select(&selector)
        .next()
        .ok_or(anyhow!("failed to find __NEXT_DATA__"))?;
    Ok(element.text().collect::<Vec<_>>().concat())
}

async fn fetch_spotify_embed_json<T: DeserializeOwned>(url: String) -> Result<T> {
    let response = reqwest::get(url).await?;
    let html_content = response.text().await?;
    let json_text = extract_spotify_next_data_json(&html_content)?;
    Ok(serde_json::from_str(&json_text)?)
}

async fn ensure_preview_video_exists(
    state: AppState,
    track_id: String,
    preloaded_inputs: Option<PreviewGenerationInputs>,
) -> Result<(), (StatusCode, String)> {
    if state.cache_manager.has_id(&track_id).await {
        return Ok(());
    }

    if state
        .cache_manager
        .get_and_cache_video_bytes(&track_id)
        .await
        .is_some()
    {
        return Ok(());
    }

    let mut is_leader = false;
    let notify = {
        let mut tasks = state.generation_tasks.lock().await;
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
        tracing::info!("waiting for leader to complete video");
        notify.notified().await;
        if state.cache_manager.has_id(&track_id).await {
            return Ok(());
        }
        if state
            .cache_manager
            .get_and_cache_video_bytes(&track_id)
            .await
            .is_some()
        {
            return Ok(());
        }
        return Err(generation_error());
    }

    tracing::info!("we are the leader");

    let generation_inputs = if let Some(preloaded_inputs) = preloaded_inputs {
        preloaded_inputs
    } else {
        let spotify_data = get_spotify_track_data(&track_id).await.map_err(|err| {
            tracing::error!("failed to fetch spotify data: {:?}", err);
            generation_error()
        })?;
        let (track_og_bytes, _) =
            get_track_og_from_service(&spotify_data)
                .await
                .map_err(|err| {
                    tracing::error!("failed to fetch og image: {:?}", err);
                    generation_error()
                })?;

        PreviewGenerationInputs {
            preview_url: spotify_data.preview_url,
            track_og_bytes,
        }
    };

    if let Err(err) = generate_preview_video(track_id.clone(), generation_inputs).await {
        tracing::error!("Error generating video: {:?}", err);
        FAILED_VIDEO_GENERATIONS.inc();
        let mut tasks = state.generation_tasks.lock().await;
        if let Some(notify) = tasks.remove(&track_id) {
            notify.notify_waiters();
        }
        return Err(generation_error());
    }

    let local_video = LocalVideo::new(track_id.clone(), get_video_output_path(track_id.clone()))
        .await
        .map_err(|err| {
            tracing::error!("failed to open generated file: {:?}", err);
            generation_error()
        })?;
    let _ = state.cache_manager.cache_video(&local_video).await;

    let cloned_track_id = track_id.clone();
    let cloned_cache_manager = state.cache_manager.clone();
    task::spawn(async move {
        if B2Video::new(cloned_track_id.clone()).await.is_err() {
            upload_to_b2(cloned_cache_manager, cloned_track_id.clone()).await;
        }
        let _ = fs::remove_dir_all(get_track_output_path(cloned_track_id)).await;
    });

    tracing::info!("video complete, notifying waiters");
    let mut tasks = state.generation_tasks.lock().await;
    if let Some(notify) = tasks.remove(&track_id) {
        notify.notify_waiters();
    }

    Ok(())
}

#[instrument(skip_all)]
async fn write_track_og(track_id: String, bytes: Bytes) -> Result<File> {
    let full_path = get_og_output_path(track_id);
    let file = write_bytes_to_output_path(full_path, bytes).await?;
    tracing::info!("finished fetching og");
    Ok(file)
}

#[instrument(skip_all)]
async fn get_spotify_album_data(
    album_id: &str,
    raw_track: Option<&str>,
) -> Result<SpotifyAlbumData> {
    tracing::info!("fetching album data");
    let json: AlbumEmbedJsonData =
        fetch_spotify_embed_json(format!("https://open.spotify.com/embed/album/{}", album_id))
            .await?;
    let entity = json.props.page_props.state.data.entity;
    let album_name = entity.title.trim().to_string();

    let track_index = resolve_requested_track_index(raw_track, entity.track_list.len())
        .ok_or(anyhow!("album has no tracks"))?;
    let track = entity
        .track_list
        .get(track_index)
        .ok_or(anyhow!("failed to select track from album"))?;

    let track_id = track
        .uri
        .rsplit_once(':')
        .map(|(_, tail)| tail)
        .ok_or(anyhow!("track has no uri"))?;
    let track_data = get_spotify_track_data(track_id).await?;

    Ok(SpotifyAlbumData {
        album_name,
        track_data,
    })
}

#[instrument(skip_all)]
async fn get_spotify_track_data(track_id: &str) -> Result<SpotifyTrackData> {
    tracing::info!("fetching spotify data");
    let json: TrackEmbedJsonData =
        fetch_spotify_embed_json(format!("https://open.spotify.com/embed/track/{}", track_id))
            .await?;
    let entity = json.props.page_props.state.data.entity;

    let song_name = entity.title.trim().to_string();
    if song_name.is_empty() {
        return Err(anyhow!("spotify response missing song title"));
    }

    let artist_names = entity
        .artists
        .into_iter()
        .map(|artist| artist.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect::<Vec<String>>();
    if artist_names.is_empty() {
        return Err(anyhow!("spotify response missing artist names"));
    }

    let preview_url = entity.audio_preview.url.trim().to_string();
    if preview_url.is_empty() {
        return Err(anyhow!("spotify response missing preview url"));
    }

    let album_art_url = entity
        .visual_identity
        .image
        .iter()
        .max_by_key(|image| image.max_width)
        .map(|image| image.url.trim().to_string())
        .filter(|url| !url.is_empty())
        .ok_or(anyhow!("spotify response missing album art"))?;

    Ok(SpotifyTrackData {
        track_id: track_id.to_string(),
        song_name,
        artist_names,
        preview_url,
        album_art_url,
    })
}

#[instrument(skip_all)]
async fn get_track_og_from_service(spotify_data: &SpotifyTrackData) -> Result<(Bytes, String)> {
    tracing::info!("fetching og image");
    let base_url = MACHINA_CONFIG.embed_image_service_url.trim_end_matches('/');
    let endpoint = format!("{base_url}/image");
    let response = reqwest::Client::new()
        .post(endpoint)
        .json(&serde_json::json!({
            "albumArt": spotify_data.album_art_url,
            "songName": spotify_data.song_name,
            "artist": spotify_data.artist_text(),
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "og image service failed with {}",
            response.status()
        ));
    }

    let theme_color = response
        .headers()
        .get("X-Basecolor")
        .and_then(|value| value.to_str().ok())
        .unwrap_or(MACHINA_CONFIG.default_base_color.as_str())
        .to_string();
    let bytes = response.bytes().await?;
    Ok((bytes, theme_color))
}

#[instrument(skip_all)]
async fn get_track_preview_audio(track_id: String, preview_url: String) -> Result<File> {
    tracing::info!("fetching preview audio");
    let response = reqwest::get(preview_url).await?;

    let bytes = response.bytes().await?;
    let full_path = get_audio_output_path(track_id);
    let file = write_bytes_to_output_path(full_path, bytes).await?;
    tracing::info!("finished fetching preview audio");
    Ok(file)
}

#[instrument(name = "ffmpeg::generate", skip_all)]
async fn generate_preview_video(track_id: String, inputs: PreviewGenerationInputs) -> Result<File> {
    // HistogramTimer impls drop, and will stop the timer automatically
    let _timer = VIDEO_GEN_DURATION.start_timer();
    tracing::info!("preparing assets for video");

    let full_path = get_track_output_path(track_id.clone());
    let path = StdPath::new(&full_path);
    if let Some(parent) = path.parent() {
        tracing::info!("cache folder did not exist, creating...");
        tokio::fs::create_dir_all(parent).await?;
    }

    let (track_og, preview_audio) = tokio::join!(
        write_track_og(track_id.clone(), inputs.track_og_bytes),
        get_track_preview_audio(track_id.clone(), inputs.preview_url)
    );
    track_og?;
    preview_audio?;

    let status = Command::new("ffmpeg")
        .args([
            "-loop",
            "1",
            "-i",
            "og.png",
            "-i",
            "audio.mp3",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "stillimage",
            "-crf",
            "23",
            "-r",
            "2",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-aac_coder",
            "fast",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-threads",
            "0",
            "-shortest",
            "out.mp4",
        ])
        .current_dir(get_track_output_path(track_id.clone()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow!("ffmpeg failed with status {}", status));
    }
    let out_file = File::open(get_video_output_path(track_id)).await.ok();

    tracing::info!("generated video");
    out_file.ok_or(anyhow!("no file got created"))
}

#[axum::debug_handler]
#[instrument(name = "get_album_page", skip_all)]
pub async fn get_album_page(
    Path(album_id): Path<String>,
    Query(query): Query<AlbumTrackQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let is_discord_request = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|ua| ua.to_ascii_lowercase().contains("discord"))
        .unwrap_or(false);

    if !is_discord_request {
        let redirect_url = format!("https://open.spotify.com/album/{album_id}");
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let album_data = get_spotify_album_data(&album_id, query.track.as_deref())
        .await
        .map_err(|err| {
            tracing::error!("failed to fetch spotify album metadata: {:?}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load track metadata".to_string(),
            )
        })?;
    let spotify_data = album_data.track_data;

    let (og_image_bytes, theme_color) =
        get_track_og_from_service(&spotify_data)
            .await
            .map_err(|err| {
                tracing::error!("failed to fetch og image: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to get image".to_string(),
                )
            })?;

    let title = if album_data.album_name.trim().is_empty() {
        spotify_data.song_name.clone()
    } else {
        format!("{} - {}", spotify_data.song_name, album_data.album_name)
    };
    let selected_track_id = spotify_data.track_id.clone();

    ensure_preview_video_exists(
        state.clone(),
        selected_track_id.clone(),
        Some(PreviewGenerationInputs {
            preview_url: spotify_data.preview_url,
            track_og_bytes: og_image_bytes,
        }),
    )
    .await?;

    let canonical_path = format!("/album/{album_id}");
    let block = build_preview_meta_page(&title, &canonical_path, &selected_track_id, &theme_color);

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
#[instrument(name = "video-preview", skip_all)]
pub async fn get_track_page(
    Path(track_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let is_discord_request = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|ua| ua.to_ascii_lowercase().contains("discord"))
        .unwrap_or(false);

    if !is_discord_request {
        let redirect_url = format!("https://open.spotify.com/track/{track_id}");
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let spotify_data = get_spotify_track_data(&track_id).await.map_err(|err| {
        tracing::error!("failed to fetch spotify metadata: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to load track metadata".to_string(),
        )
    })?;
    let (og_image_bytes, theme_color) =
        get_track_og_from_service(&spotify_data)
            .await
            .map_err(|err| {
                tracing::error!("failed to fetch og image: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to get image".to_string(),
                )
            })?;

    let title = spotify_data.song_name.clone();
    let track_id_for_meta = spotify_data.track_id.clone();

    ensure_preview_video_exists(
        state.clone(),
        track_id_for_meta.clone(),
        Some(PreviewGenerationInputs {
            preview_url: spotify_data.preview_url,
            track_og_bytes: og_image_bytes,
        }),
    )
    .await?;

    let canonical_path = format!("/track/{track_id}");
    let block = build_preview_meta_page(&title, &canonical_path, &track_id_for_meta, &theme_color);

    Ok(AxumHtml(block).into_response())
}

#[axum::debug_handler]
#[instrument(name = "image-preview", skip_all)]
pub async fn get_generated_image(
    Path(track_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let spotify_data = get_spotify_track_data(&track_id).await.map_err(|err| {
        tracing::error!("failed to fetch spotify metadata: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to load track metadata".to_string(),
        )
    })?;

    let (image_bytes, theme_color) =
        get_track_og_from_service(&spotify_data)
            .await
            .map_err(|err| {
                tracing::error!("failed to fetch og image: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to get image".to_string(),
                )
            })?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    headers.insert("X-Basecolor", theme_color.parse().unwrap());

    Ok((StatusCode::OK, headers, image_bytes).into_response())
}

#[axum::debug_handler]
#[instrument(name = "video-preview", skip(state))]
pub async fn get_preview_video(
    Path(raw_track_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let track_id = raw_track_id
        .strip_suffix(".mp4")
        .unwrap_or(&raw_track_id)
        .to_string();

    if let Some(bytes) = state
        .cache_manager
        .get_and_cache_video_bytes(&track_id)
        .await
    {
        return Ok(build_response(bytes));
    }

    ensure_preview_video_exists(state.clone(), track_id.clone(), None).await?;

    serve_cached_video(state, track_id).await
}

fn build_response(video_bytes: Bytes) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    headers.insert(header::CONTENT_TYPE, "video/mp4".parse().unwrap());
    (StatusCode::OK, headers, video_bytes).into_response()
}
