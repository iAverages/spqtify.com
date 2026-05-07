use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumEmbedJsonData {
    pub props: Props,
    pub page: String,
    pub query: Query,
    pub build_id: String,
    pub asset_prefix: String,
    pub is_fallback: bool,
    pub is_experimental_compile: bool,
    pub gssp: bool,
    pub script_loader: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Props {
    pub page_props: PageProps,
    #[serde(rename = "__N_SSP")]
    pub n_ssp: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageProps {
    pub state: State,
    pub config: Config,
    #[serde(rename = "_sentryTraceData")]
    pub sentry_trace_data: String,
    #[serde(rename = "_sentryBaggage")]
    pub sentry_baggage: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub data: Data,
    pub settings: Settings,
    pub machine_state: MachineState,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub entity: Entity,
    #[serde(rename = "embeded_entity_uri")]
    pub embeded_entity_uri: String,
    pub default_audio_file_object: DefaultAudioFileObject,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    #[serde(rename = "type")]
    pub type_field: String,
    pub name: String,
    pub uri: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub is_pre_release: bool,
    pub release_date: Value,
    pub duration: i64,
    pub is_playable: bool,
    pub playability_reason: String,
    pub is_explicit: bool,
    pub has_video: bool,
    pub related_entity_uri: String,
    pub track_list: Vec<TrackList>,
    pub visual_identity: VisualIdentity,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackList {
    pub uri: String,
    pub uid: String,
    pub title: String,
    pub subtitle: String,
    pub is_explicit: bool,
    pub is_nineteen_plus: bool,
    pub content_ratings: ContentRatings,
    pub duration: i64,
    pub is_playable: bool,
    pub playability_reason: String,
    pub audio_preview: AudioPreview,
    pub entity_type: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRatings {
    pub labels: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPreview {
    pub format: String,
    pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualIdentity {
    pub background_base: BackgroundBase,
    pub background_tinted_base: BackgroundTintedBase,
    pub text_base: TextBase,
    pub text_bright_accent: TextBrightAccent,
    pub text_subdued: TextSubdued,
    pub image: Vec<Image>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundBase {
    pub alpha: i64,
    pub blue: i64,
    pub green: i64,
    pub red: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTintedBase {
    pub alpha: i64,
    pub blue: i64,
    pub green: i64,
    pub red: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBase {
    pub alpha: i64,
    pub blue: i64,
    pub green: i64,
    pub red: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBrightAccent {
    pub alpha: i64,
    pub blue: i64,
    pub green: i64,
    pub red: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSubdued {
    pub alpha: i64,
    pub blue: i64,
    pub green: i64,
    pub red: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub url: String,
    pub max_height: i64,
    pub max_width: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAudioFileObject {
    pub passthrough: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub rtl: bool,
    pub session: Session,
    pub entity_context: String,
    pub client_id: String,
    pub is_mobile: bool,
    pub is_safari: bool,
    #[serde(rename = "isIOS")]
    pub is_ios: bool,
    pub is_tablet: bool,
    pub is_dark_mode: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub access_token: String,
    pub access_token_expiration_timestamp_ms: i64,
    pub is_anonymous: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineState {
    pub initialized: bool,
    pub show_overflow_menu: bool,
    pub playback_mode: String,
    pub current_preview_track_index: i64,
    pub platform_supports_encrypted_content: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub correlation_id: String,
    pub strings: Strings,
    pub locale: String,
    pub client_id: String,
    pub restriction_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Strings {
    pub en: En,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct En {
    pub translation: Translation,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub id: String,
}
