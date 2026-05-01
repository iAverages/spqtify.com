mod config;
mod embeds;
mod metrics;
mod utils;

use self::config::{MachinaConfig, get_config};
use self::embeds::cache_manager::CacheManger;
use self::embeds::preview::{B2Video, LocalVideo, get_preview_video};
use self::metrics::{get_prometheus_metrics, metric_setup};
use self::utils::{get_track_output_path, get_video_output_path, upload_to_b2};
use axum::Router;
use axum::http::HeaderValue;
use axum::routing::get;
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use backblaze_b2_client::client::B2Client;
use once_cell::sync::{Lazy, OnceCell};
use reqwest::Method;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs::{self};
use tokio::sync::{Mutex, Notify};
use tokio::task;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    cache_manager: Arc<CacheManger>,
    generation_tasks: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
}

static B2: OnceCell<B2Client> = OnceCell::new();

pub fn get_b2() -> &'static B2Client {
    B2.get().expect("b2 client not set")
}

static MACHINA_CONFIG: Lazy<MachinaConfig> = Lazy::new(|| get_config().expect("config"));

#[tokio::main]
async fn main() {
    let _guard = init_tracing_opentelemetry::tracing_subscriber_ext::init_subscribers().unwrap();
    metric_setup();

    let b2 = B2Client::new(
        MACHINA_CONFIG.b2_application_key_id.clone(),
        MACHINA_CONFIG.b2_application_key.clone(),
    )
    .await
    .expect("b2");

    let _ = B2.set(b2);

    let state = AppState {
        cache_manager: Arc::new(CacheManger::new()),
        generation_tasks: Arc::new(Mutex::new(HashMap::new())),
    };

    task::spawn(cache_upload_existing_tmp(state.cache_manager.clone()));

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(
            MACHINA_CONFIG
                .app_url
                .trim_end_matches('/')
                .parse::<HeaderValue>()
                .unwrap(),
        );

    let app = Router::new()
        .route("/api/generate/video/{trackId}", get(get_preview_video))
        .route("/metrics", get(get_prometheus_metrics))
        .layer(OtelInResponseLayer)
        .layer(OtelAxumLayer::default())
        .layer(cors)
        .with_state(state.clone());

    state.cache_manager.start_cleanup_thread();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    tracing::info!("listening on :3001");
    axum::serve(listener, app).await.unwrap();
}

async fn cache_upload_existing_tmp(cache_manager: Arc<CacheManger>) {
    tracing::debug!(
        "checking {} for existing generated videos",
        MACHINA_CONFIG.video_generator_dir
    );
    let mut entries = fs::read_dir(MACHINA_CONFIG.video_generator_dir.clone())
        .await
        .unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(folder_name) = path.file_name() else {
            continue;
        };

        if let Some(track_id) = folder_name.to_str() {
            tracing::debug!("found track {:?} in tmp, uploading to b2", track_id);
            let local_video = LocalVideo::new(
                track_id.to_string(),
                get_video_output_path(track_id.to_string()),
            )
            .await;

            if let Ok(video) = local_video {
                let _ = cache_manager.cache_video(&video).await;
                // if we error there was no b2 video so we should upload
                if B2Video::new(track_id.to_string()).await.is_err() {
                    upload_to_b2(cache_manager.clone(), track_id.to_string()).await;
                }
            }

            let _ = fs::remove_dir_all(get_track_output_path(track_id.to_string())).await;
            tracing::debug!("deleted tmp directory for {}", track_id);
        }
    }
}
