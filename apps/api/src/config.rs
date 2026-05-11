use dotenvy::dotenv;
use envconfig::Envconfig;

#[derive(Debug, Envconfig, Clone)]
pub struct MachinaConfig {
    #[envconfig(from = "APP_URL")]
    pub app_url: String,
    #[envconfig(from = "B2_BUCKET_ID")]
    pub b2_bucket_id: String,
    #[envconfig(from = "B2_APPLICATION_KEY_ID")]
    pub b2_application_key_id: String,
    #[envconfig(from = "B2_APPLICATION_KEY")]
    pub b2_application_key: String,
    #[envconfig(from = "MACHINA_VIDEO_GENERATION_DIR", default = "/tmp/machina")]
    pub video_generator_dir: String,

    #[envconfig(from = "MACHINA_VIDEO_CACHE_MAX_BYTES", default = "524288000")]
    pub video_cache_max_bytes: u64,

    #[envconfig(from = "SPQTIFY_DEFAULT_BASE_COLOR", default = "#000")]
    pub default_base_color: String,

    #[envconfig(from = "EMBED_IMAGE_SERVICE_URL", default = "http://localhost:3001")]
    pub embed_image_service_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MachinaConfigError {
    #[error("missing required environment variable: {0}")]
    MissingRequired(&'static str),

    #[error("failed to parse environment variable: {0}")]
    ParseError(&'static str),
}

pub fn get_config() -> Result<MachinaConfig, MachinaConfigError> {
    dotenv().ok();

    // TODO: add some more validation to ensure, for example, that the API/APP urls are
    // actually valid urls (and always strip trailing slashs)
    MachinaConfig::init_from_env().map_err(|error| match error {
        envconfig::Error::EnvVarMissing { name } => MachinaConfigError::MissingRequired(name),
        envconfig::Error::ParseError { name } => MachinaConfigError::ParseError(name),
    })
}
