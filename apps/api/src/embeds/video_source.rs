use std::io::Cursor;

use backblaze_b2_client::definitions::query_params::{
    B2DownloadFileQueryParameters, B2ListFileNamesQueryParameters,
};
use bytes::Bytes;
use futures::stream::StreamExt;

use crate::{MACHINA_CONFIG, get_b2};

#[derive(Debug, thiserror::Error)]
pub enum VideoSourceError {
    #[error("video with id {0} was not found")]
    NotFound(String),

    #[error("failed to read video bytes from source")]
    ReadFailed,

    #[error("failed to upload video bytes to source")]
    UploadFailed,
}

pub struct B2VideoSource;

impl B2VideoSource {
    pub fn new() -> Self {
        Self
    }

    pub async fn has_video(&self, track_id: &str) -> Result<bool, VideoSourceError> {
        let files = get_b2()
            .basic_client()
            .list_file_names(
                B2ListFileNamesQueryParameters::builder()
                    .bucket_id(MACHINA_CONFIG.b2_bucket_id.clone())
                    .prefix(Some(get_b2_video_path(track_id)))
                    .build(),
            )
            .await
            .map_err(|_| VideoSourceError::ReadFailed)?;

        Ok(!files.files.is_empty())
    }

    pub async fn fetch_video_bytes(&self, track_id: &str) -> Result<Bytes, VideoSourceError> {
        let files = get_b2()
            .basic_client()
            .list_file_names(
                B2ListFileNamesQueryParameters::builder()
                    .bucket_id(MACHINA_CONFIG.b2_bucket_id.clone())
                    .prefix(Some(get_b2_video_path(track_id)))
                    .build(),
            )
            .await
            .map_err(|_| VideoSourceError::ReadFailed)?;

        let Some(file) = files.files.first() else {
            return Err(VideoSourceError::NotFound(track_id.to_string()));
        };

        let video = get_b2()
            .basic_client()
            .download_file_by_id(
                file.file_id.clone(),
                Some(B2DownloadFileQueryParameters::builder().build()),
            )
            .await
            .map_err(|_| VideoSourceError::ReadFailed)?;
        let (size, mut stream) = video.file.into_stream();
        let mut buffer: Vec<u8> = Vec::with_capacity(size);

        while let Some(value) = stream.next().await {
            let chunk = value.map_err(|_| VideoSourceError::ReadFailed)?;
            buffer.extend_from_slice(chunk.as_ref());
        }

        Ok(Bytes::from(buffer))
    }

    pub async fn upload_video_bytes(
        &self,
        track_id: &str,
        video_bytes: Bytes,
    ) -> Result<(), VideoSourceError> {
        let length = video_bytes.len() as u64;
        let wrapped = Cursor::new(video_bytes);

        let upload = get_b2()
            .create_upload(
                wrapped,
                get_b2_video_path(track_id),
                MACHINA_CONFIG.b2_bucket_id.clone(),
                None,
                length,
                None,
            )
            .await;

        upload
            .start()
            .await
            .map_err(|_| VideoSourceError::UploadFailed)?;
        Ok(())
    }
}

fn get_b2_video_path(track_id: &str) -> String {
    format!("generated/{track_id}/out.mp4")
}
