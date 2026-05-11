use axum::response::IntoResponse;
use lazy_static::lazy_static;
use prometheus::Encoder;
use prometheus::Histogram;
use prometheus::IntCounter;
use prometheus::IntGauge;
use prometheus::TextEncoder;
use prometheus::{register_histogram, register_int_counter, register_int_gauge};
use reqwest::StatusCode;
use tracing::instrument;

lazy_static! {
    pub static ref FAILED_VIDEO_GENERATIONS: IntCounter = register_int_counter!(
        "failed_video_generations",
        "Number of videos that failed to generate",
    )
    .unwrap();
}

lazy_static! {
    pub static ref VIDEO_GEN_DURATION: Histogram = register_histogram!(
        "video_generation_duration_seconds",
        "Duration of video generation in seconds",
        vec![
            0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5,
            9.0, 9.5, 10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0, 14.5, 15.0
        ]
    )
    .unwrap();
}

lazy_static! {
    pub static ref VIDEO_CACHE_CURRENT_BYTES: IntGauge = register_int_gauge!(
        "video_cache_current_bytes",
        "Current number of bytes held in local video cache",
    )
    .unwrap();
}

lazy_static! {
    pub static ref VIDEO_CACHE_EVICTIONS_TOTAL: IntCounter = register_int_counter!(
        "video_cache_evictions_total",
        "Total number of local video cache evictions",
    )
    .unwrap();
}

pub fn metric_setup() {
    FAILED_VIDEO_GENERATIONS.reset();
    VIDEO_CACHE_CURRENT_BYTES.set(0);
    VIDEO_CACHE_EVICTIONS_TOTAL.reset();
}

#[axum::debug_handler]
#[instrument(name = "metrics", skip_all)]
pub async fn get_prometheus_metrics() -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|err| {
            tracing::error!("failed to generate metrics for prometheus: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to generate metrics".to_string(),
            )
        })?;

    Ok(([("Content-Type", encoder.format_type())], buffer).into_response())
}
