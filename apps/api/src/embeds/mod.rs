pub mod cache_manager;
pub mod image_client;
pub mod preview;
pub mod preview_generation;
pub mod renderer;
pub mod spotify_metadata;
pub mod video_source;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoKind {
    Track,
    Episode,
    Album,
    Playlist,
}

impl VideoKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Episode => "episode",
            Self::Album => "album",
            Self::Playlist => "playlist",
        }
    }
}
