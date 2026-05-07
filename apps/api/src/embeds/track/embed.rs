use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEmbedJsonData {
    pub props: Props,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Props {
    pub page_props: PageProps,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageProps {
    pub state: State,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub data: Data,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub entity: Entity,
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
    pub artists: Vec<Artist>,
    pub release_date: ReleaseDate,
    pub duration: i64,
    pub is_playable: bool,
    pub playability_reason: String,
    pub is_explicit: bool,
    pub is_nineteen_plus: bool,
    pub audio_preview: AudioPreview,
    pub has_video: bool,
    pub related_entity_uri: String,
    pub visual_identity: VisualIdentity,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub name: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseDate {
    pub iso_string: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPreview {
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
