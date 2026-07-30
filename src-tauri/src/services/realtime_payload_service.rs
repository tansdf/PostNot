use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub const REALTIME_INLINE_PAYLOAD_LIMIT: usize = 256 * 1024;
pub const REALTIME_PAYLOAD_PREVIEW_LIMIT: usize = 4 * 1024;
pub const REALTIME_PAYLOAD_READ_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum RealtimePayload {
    Inline {
        text: String,
        #[serde(rename = "sizeBytes")]
        size_bytes: u64,
        encoding: RealtimePayloadEncoding,
        truncated: bool,
    },
    File {
        #[serde(rename = "handleId")]
        handle_id: String,
        #[serde(rename = "previewText")]
        preview_text: String,
        #[serde(rename = "sizeBytes")]
        size_bytes: u64,
        encoding: RealtimePayloadEncoding,
        truncated: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimePayloadEncoding {
    Utf8,
    Base64,
}

impl RealtimePayload {
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Inline { size_bytes, .. } | Self::File { size_bytes, .. } => *size_bytes,
        }
    }

    pub fn handle_id(&self) -> Option<&str> {
        match self {
            Self::Inline { .. } => None,
            Self::File { handle_id, .. } => Some(handle_id),
        }
    }
}

#[derive(Clone)]
pub struct RealtimePayloadStore {
    inner: Arc<RealtimePayloadStoreInner>,
}

struct RealtimePayloadStoreInner {
    root: PathBuf,
    handles: Mutex<HashMap<String, StoredPayload>>,
}

#[derive(Clone)]
struct StoredPayload {
    path: PathBuf,
    encoding: RealtimePayloadEncoding,
    retain_count: usize,
}

impl RealtimePayloadStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            inner: Arc::new(RealtimePayloadStoreInner {
                root,
                handles: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn reset(&self) -> AppResult<()> {
        if fs::try_exists(&self.inner.root).await? {
            fs::remove_dir_all(&self.inner.root).await?;
        }
        fs::create_dir_all(&self.inner.root).await?;
        self.inner
            .handles
            .lock()
            .map_err(|_| payload_state_error())?
            .clear();
        Ok(())
    }

    pub async fn store_text(&self, text: String) -> AppResult<RealtimePayload> {
        let bytes = text.as_bytes();
        if bytes.len() <= REALTIME_INLINE_PAYLOAD_LIMIT {
            let size_bytes = bytes.len() as u64;
            return Ok(RealtimePayload::Inline {
                text,
                size_bytes,
                encoding: RealtimePayloadEncoding::Utf8,
                truncated: false,
            });
        }

        self.store_file(
            bytes,
            utf8_preview(bytes),
            RealtimePayloadEncoding::Utf8,
            true,
        )
        .await
    }

    pub async fn store_binary(&self, bytes: &[u8]) -> AppResult<RealtimePayload> {
        if bytes.len() <= REALTIME_INLINE_PAYLOAD_LIMIT {
            return Ok(RealtimePayload::Inline {
                text: BASE64.encode(bytes),
                size_bytes: bytes.len() as u64,
                encoding: RealtimePayloadEncoding::Base64,
                truncated: false,
            });
        }
        self.store_file(
            bytes,
            BASE64.encode(&bytes[..bytes.len().min(REALTIME_PAYLOAD_PREVIEW_LIMIT)]),
            RealtimePayloadEncoding::Base64,
            bytes.len() > REALTIME_PAYLOAD_PREVIEW_LIMIT,
        )
        .await
    }

    async fn store_file(
        &self,
        bytes: &[u8],
        preview_text: String,
        encoding: RealtimePayloadEncoding,
        truncated: bool,
    ) -> AppResult<RealtimePayload> {
        fs::create_dir_all(&self.inner.root).await?;
        let handle_id = Uuid::new_v4().to_string();
        let path = self.inner.root.join(format!("{handle_id}.payload"));
        fs::write(&path, bytes).await?;
        self.inner
            .handles
            .lock()
            .map_err(|_| payload_state_error())?
            .insert(
                handle_id.clone(),
                StoredPayload {
                    path,
                    encoding,
                    retain_count: 1,
                },
            );
        Ok(RealtimePayload::File {
            handle_id,
            preview_text,
            size_bytes: bytes.len() as u64,
            encoding,
            truncated,
        })
    }

    pub async fn read(&self, handle_id: &str) -> AppResult<String> {
        let stored = self.lookup(handle_id)?;
        let size = fs::metadata(&stored.path).await?.len();
        if size > REALTIME_PAYLOAD_READ_LIMIT {
            return Err(AppError::Message(format!(
                "Realtime payload is {size} bytes and is too large to read into the UI. Use Save As to retain the complete payload."
            )));
        }
        let bytes = fs::read(stored.path).await?;
        Ok(match stored.encoding {
            RealtimePayloadEncoding::Utf8 => String::from_utf8(bytes).map_err(|_| {
                AppError::Message(
                    "The stored realtime text payload is not valid UTF-8.".to_string(),
                )
            })?,
            RealtimePayloadEncoding::Base64 => BASE64.encode(bytes),
        })
    }

    pub(crate) fn export_source(
        &self,
        handle_id: &str,
    ) -> AppResult<(PathBuf, RealtimePayloadEncoding)> {
        let stored = self.lookup(handle_id)?;
        Ok((stored.path, stored.encoding))
    }

    pub async fn copy_to(&self, handle_id: &str, destination: &Path) -> AppResult<()> {
        let stored = self.lookup(handle_id)?;
        fs::copy(stored.path, destination).await?;
        Ok(())
    }

    pub async fn release(&self, handle_id: &str) -> AppResult<()> {
        let stored = {
            let mut handles = self
                .inner
                .handles
                .lock()
                .map_err(|_| payload_state_error())?;
            match handles.get_mut(handle_id) {
                Some(stored) if stored.retain_count > 1 => {
                    stored.retain_count -= 1;
                    None
                }
                Some(_) => handles.remove(handle_id),
                None => None,
            }
        };
        if let Some(stored) = stored {
            match fs::remove_file(stored.path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub(crate) fn retain(&self, handle_id: &str) -> AppResult<()> {
        let mut handles = self
            .inner
            .handles
            .lock()
            .map_err(|_| payload_state_error())?;
        let stored = handles
            .get_mut(handle_id)
            .ok_or_else(|| AppError::Message("Realtime payload not found.".to_string()))?;
        stored.retain_count = stored.retain_count.saturating_add(1);
        Ok(())
    }

    fn lookup(&self, handle_id: &str) -> AppResult<StoredPayload> {
        self.inner
            .handles
            .lock()
            .map_err(|_| payload_state_error())?
            .get(handle_id)
            .cloned()
            .ok_or_else(|| AppError::Message("Realtime payload not found.".to_string()))
    }
}

fn utf8_preview(bytes: &[u8]) -> String {
    let mut end = bytes.len().min(REALTIME_PAYLOAD_PREVIEW_LIMIT);
    while end > 0 && std::str::from_utf8(&bytes[..end]).is_err() {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn payload_state_error() -> AppError {
    AppError::Message("Failed to access realtime payload state.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn large_text_and_binary_are_file_backed_and_releasable() {
        let root = std::env::temp_dir().join(format!("postnot-realtime-test-{}", Uuid::new_v4()));
        let store = RealtimePayloadStore::new(root.clone());
        store.reset().await.expect("reset");

        let text = "x".repeat(REALTIME_INLINE_PAYLOAD_LIMIT + 1);
        let payload = store.store_text(text.clone()).await.expect("store text");
        let handle = payload.handle_id().expect("file handle").to_string();
        assert_eq!(store.read(&handle).await.expect("read"), text);
        store.release(&handle).await.expect("release");
        assert!(store.read(&handle).await.is_err());

        let small_binary = vec![0, 1, 2, 255];
        let payload = store
            .store_binary(&small_binary)
            .await
            .expect("store binary");
        assert!(matches!(payload, RealtimePayload::Inline { .. }));

        let binary = vec![0; REALTIME_INLINE_PAYLOAD_LIMIT + 1];
        let payload = store.store_binary(&binary).await.expect("store binary");
        let handle = payload.handle_id().expect("file handle");
        assert_eq!(
            store.read(handle).await.expect("read"),
            BASE64.encode(binary)
        );

        let _ = fs::remove_dir_all(root).await;
    }
}
