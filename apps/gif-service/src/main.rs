use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use bytes::Bytes;
use futures::StreamExt;
use reqwest::{Client, Url, redirect::Policy};
use std::env;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tower_http::trace::TraceLayer;

const DEFAULT_UPSTREAM: &str = "https://video.twimg.com/";
const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const DEFAULT_MAX_INPUT_BYTES: usize = 50 * 1024 * 1024;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy)]
enum OutputFormat {
    Webp,
    Gif,
}

#[derive(Clone)]
struct AppState {
    client: Client,
    upstream: Url,
    conversions: Arc<Semaphore>,
    max_input_bytes: usize,
    max_output_bytes: usize,
    convert_timeout: Duration,
}

#[derive(Debug)]
enum ServiceError {
    UpstreamNotFound,
    Upstream,
    UpstreamTimeout,
    TooLarge,
    Busy,
    InvalidVideo,
    Internal,
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        tracing::warn!(error = ?self, "gif-service request failed");
        let (status, message) = match self {
            Self::UpstreamNotFound => (StatusCode::NOT_FOUND, "source video not found"),
            Self::Upstream => (StatusCode::BAD_GATEWAY, "failed to fetch source video"),
            Self::UpstreamTimeout => (StatusCode::GATEWAY_TIMEOUT, "source video timed out"),
            Self::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "video is too large"),
            Self::Busy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "conversion capacity exhausted",
            ),
            Self::InvalidVideo => (StatusCode::UNPROCESSABLE_ENTITY, "failed to convert video"),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "conversion service failed",
            ),
        };
        let mut response = (status, message).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if status == StatusCode::SERVICE_UNAVAILABLE {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        response
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let upstream = parse_upstream(
        &env::var("GIF_SERVICE_UPSTREAM_BASE_URL").unwrap_or_else(|_| DEFAULT_UPSTREAM.into()),
    )
    .expect("valid GIF_SERVICE_UPSTREAM_BASE_URL");
    let fetch_timeout = env_value("GIF_SERVICE_FETCH_TIMEOUT_SECONDS", 30);
    let state = AppState {
        client: Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(fetch_timeout))
            .build()
            .expect("HTTP client"),
        upstream,
        conversions: Arc::new(Semaphore::new(env_value("GIF_SERVICE_MAX_CONVERSIONS", 2))),
        max_input_bytes: env_value("GIF_SERVICE_MAX_INPUT_BYTES", DEFAULT_MAX_INPUT_BYTES),
        max_output_bytes: env_value("GIF_SERVICE_MAX_OUTPUT_BYTES", DEFAULT_MAX_OUTPUT_BYTES),
        convert_timeout: Duration::from_secs(env_value("GIF_SERVICE_CONVERT_TIMEOUT_SECONDS", 60)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/tweet_video/{filename}", get(convert))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002")
        .await
        .expect("listen on port 3002");
    tracing::info!("listening on :3002");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve gif-service");
}

async fn health() -> Response {
    let mut response = StatusCode::OK.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn convert(
    Path(filename): Path<String>,
    State(state): State<AppState>,
) -> Result<Response, ServiceError> {
    let output_format = if filename.ends_with(".gif") {
        OutputFormat::Gif
    } else {
        OutputFormat::Webp
    };
    let upstream_filename = upstream_filename(&filename);
    let _permit = state
        .conversions
        .clone()
        .try_acquire_owned()
        .map_err(|_| ServiceError::Busy)?;
    let source = fetch_video(&state, &upstream_filename).await?;
    let image = transcode(
        source,
        output_format,
        state.max_output_bytes,
        state.convert_timeout,
    )
    .await?;
    Ok(image_response(image, output_format))
}

fn upstream_filename(filename: &str) -> String {
    filename
        .strip_suffix(".webp")
        .or_else(|| filename.strip_suffix(".gif"))
        .map_or_else(|| filename.to_owned(), |name| format!("{name}.mp4"))
}

fn parse_upstream(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.cannot_be_a_base() {
        return Err("upstream must be an HTTP(S) base URL".into());
    }
    Ok(url)
}

fn source_url(upstream: &Url, filename: &str) -> Result<Url, ServiceError> {
    let mut url = upstream.clone();
    url.path_segments_mut()
        .map_err(|_| ServiceError::Internal)?
        .pop_if_empty()
        .push("tweet_video")
        .push(filename);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

async fn fetch_video(state: &AppState, filename: &str) -> Result<Bytes, ServiceError> {
    let response = state
        .client
        .get(source_url(&state.upstream, filename)?)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                ServiceError::UpstreamTimeout
            } else {
                ServiceError::Upstream
            }
        })?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(ServiceError::UpstreamNotFound);
    }
    if !response.status().is_success() {
        return Err(ServiceError::Upstream);
    }
    let content_length = response.content_length();
    if content_length.is_some_and(|length| length > state.max_input_bytes as u64) {
        return Err(ServiceError::TooLarge);
    }

    let mut bytes = Vec::with_capacity(content_length.unwrap_or_default() as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ServiceError::Upstream)?;
        if bytes.len() + chunk.len() > state.max_input_bytes {
            return Err(ServiceError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes.into())
}

async fn transcode(
    input: Bytes,
    output_format: OutputFormat,
    max_output_bytes: usize,
    duration: Duration,
) -> Result<Bytes, ServiceError> {
    let mut command = Command::new("ffmpeg");
    command.args(ffmpeg_args());
    match output_format {
        OutputFormat::Webp => {
            command.args(["-c:v", "libwebp_anim", "-loop", "0", "-f", "webp", "pipe:1"])
        }
        OutputFormat::Gif => command.args(["-c:v", "gif", "-loop", "0", "-f", "gif", "pipe:1"]),
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| ServiceError::Internal)?;
    let mut stdin = child.stdin.take().ok_or(ServiceError::Internal)?;
    let stdout = child.stdout.take().ok_or(ServiceError::Internal)?;

    let conversion = async move {
        let write = async move {
            stdin
                .write_all(&input)
                .await
                .map_err(|_| ServiceError::InvalidVideo)?;
            stdin
                .shutdown()
                .await
                .map_err(|_| ServiceError::InvalidVideo)
        };
        let read = read_limited(stdout, max_output_bytes);
        let wait = async { child.wait().await.map_err(|_| ServiceError::Internal) };
        let (_, output, status) = tokio::try_join!(write, read, wait)?;
        if !status.success() || !is_image(&output, output_format) {
            return Err(ServiceError::InvalidVideo);
        }
        Ok(Bytes::from(output))
    };

    timeout(duration, conversion)
        .await
        .map_err(|_| ServiceError::InvalidVideo)?
}

async fn read_limited(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, ServiceError> {
    let mut output = Vec::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|_| ServiceError::Internal)?;
        if read == 0 {
            return Ok(output);
        }
        if output.len() + read > limit {
            return Err(ServiceError::TooLarge);
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

fn ffmpeg_args() -> [&'static str; 12] {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        "pipe:0",
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-fps_mode",
        "passthrough",
    ]
}

fn is_image(bytes: &[u8], output_format: OutputFormat) -> bool {
    match output_format {
        OutputFormat::Webp => {
            bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
        }
        OutputFormat::Gif => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
    }
}

fn image_response(bytes: Bytes, output_format: OutputFormat) -> Response {
    let content_type = match output_format {
        OutputFormat::Webp => "image/webp",
        OutputFormat::Gif => "image/gif",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, CACHE_CONTROL)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from(bytes))
        .expect("valid response headers")
}

fn env_value<T>(name: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    env::var(name)
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|error| panic!("invalid {name}: {error}"))
        })
        .unwrap_or(default)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install termination handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_mp4_for_image_url() {
        assert_eq!(
            upstream_filename("HTNiiJrbAAA-Ya5.webp"),
            "HTNiiJrbAAA-Ya5.mp4"
        );
        assert_eq!(
            upstream_filename("HTNiiJrbAAA-Ya5.gif"),
            "HTNiiJrbAAA-Ya5.mp4"
        );
        assert_eq!(
            upstream_filename("HTNiiJrbAAA-Ya5.mp4"),
            "HTNiiJrbAAA-Ya5.mp4"
        );
    }

    #[test]
    fn webp_response_has_webp_content_type() {
        let response = image_response(Bytes::from_static(b"RIFF\0\0\0\0WEBP"), OutputFormat::Webp);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/webp");
        assert!(is_image(b"RIFF\0\0\0\0WEBP", OutputFormat::Webp));
    }

    #[test]
    fn gif_response_has_gif_content_type() {
        let response = image_response(Bytes::from_static(b"GIF89a"), OutputFormat::Gif);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/gif");
        assert!(is_image(b"GIF89a", OutputFormat::Gif));
    }

    #[tokio::test]
    async fn limits_output_reads() {
        assert!(matches!(
            read_limited(&b"12345"[..], 4).await,
            Err(ServiceError::TooLarge)
        ));
    }
}
