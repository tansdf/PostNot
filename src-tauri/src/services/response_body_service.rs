use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub const INLINE_BODY_LIMIT: usize = 1024 * 1024;
pub const BODY_PREVIEW_LIMIT: usize = 4096;
const SEARCH_BUFFER_SIZE: usize = 64 * 1024;
const SEARCH_MATCH_LIMIT: usize = 100_000;
const DISPLAY_ROW_BYTES: usize = 64 * 1024;
const ROW_INDEX_STRIDE: u64 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsePresentation {
    Text,
    Json,
    Image,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredResponseBody {
    pub handle_id: String,
    pub preview_text: String,
    pub size_bytes: u64,
    pub content_type: Option<String>,
    pub charset: Option<String>,
    pub presentation: ResponsePresentation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBodyRow {
    pub key: String,
    pub row_index: u64,
    pub source_line: u64,
    pub segment_index: u32,
    pub text: String,
    pub continues: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBodyWindow {
    pub start_row: u64,
    pub total_rows: u64,
    pub rows: Vec<ResponseBodyRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSearchMatch {
    pub byte_offset: u64,
    pub byte_length: u64,
    pub row_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSearchResult {
    pub total_matches: u64,
    pub capped: bool,
    pub matches: Vec<ResponseSearchMatch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSearchProgress {
    pub search_id: String,
    pub scanned_bytes: u64,
    pub total_bytes: u64,
    pub total_matches: u64,
    pub first_match: Option<ResponseSearchMatch>,
    pub finished: bool,
}

pub type ResponseSearchProgressSink = Arc<dyn Fn(ResponseSearchProgress) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBodyJobProgress {
    pub job_id: String,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub finished: bool,
}

pub type ResponseBodyJobProgressSink = Arc<dyn Fn(ResponseBodyJobProgress) + Send + Sync>;

#[derive(Clone)]
struct BodyEntry {
    path: PathBuf,
    leases: usize,
    delete_on_release: bool,
    row_index: Option<Arc<RowIndex>>,
    charset: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RowLocation {
    row_index: u64,
    offset: u64,
    source_line: u64,
    segment_index: u32,
}

pub(crate) struct RowIndex {
    anchors: Vec<RowLocation>,
    total_rows: u64,
}

#[derive(Clone, Copy)]
struct DisplayRowState {
    row_index: u64,
    bytes_in_row: usize,
    utf8_remaining: u8,
}

impl DisplayRowState {
    fn advance(&mut self, byte: u8) {
        if byte == b'\n' {
            self.row_index = self.row_index.saturating_add(1);
            self.bytes_in_row = 0;
            self.utf8_remaining = 0;
        } else {
            self.utf8_remaining = next_utf8_remaining(self.utf8_remaining, byte);
            self.bytes_in_row += 1;
            if self.bytes_in_row >= DISPLAY_ROW_BYTES && self.utf8_remaining == 0 {
                self.row_index = self.row_index.saturating_add(1);
                self.bytes_in_row = 0;
            }
        }
    }
}

pub struct ResponseRowIndexBuilder {
    anchors: Vec<RowLocation>,
    absolute: u64,
    row_start: u64,
    total_rows: u64,
    source_line: u64,
    segment_index: u32,
    utf8_remaining: u8,
}

impl ResponseRowIndexBuilder {
    pub fn new() -> Self {
        Self {
            anchors: Vec::new(),
            absolute: 0,
            row_start: 0,
            total_rows: 0,
            source_line: 0,
            segment_index: 0,
            utf8_remaining: 0,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        if self.total_rows == 0 && !bytes.is_empty() {
            self.total_rows = 1;
            self.anchors.push(RowLocation {
                row_index: 0,
                offset: 0,
                source_line: 0,
                segment_index: 0,
            });
        }
        for byte in bytes {
            self.absolute += 1;
            self.utf8_remaining = next_utf8_remaining(self.utf8_remaining, *byte);
            if *byte == b'\n' {
                self.source_line += 1;
                self.segment_index = 0;
                self.row_start = self.absolute;
                self.push_row_anchor();
            } else if self.absolute.saturating_sub(self.row_start) >= DISPLAY_ROW_BYTES as u64
                && self.utf8_remaining == 0
            {
                self.segment_index = self.segment_index.saturating_add(1);
                self.row_start = self.absolute;
                self.push_row_anchor();
            }
        }
    }

    fn push_row_anchor(&mut self) {
        self.total_rows = self.total_rows.saturating_add(1);
        let row_index = self.total_rows - 1;
        if row_index % ROW_INDEX_STRIDE == 0 {
            self.anchors.push(RowLocation {
                row_index,
                offset: self.absolute,
                source_line: self.source_line,
                segment_index: self.segment_index,
            });
        }
    }

    pub(crate) fn finish(mut self) -> RowIndex {
        if self.absolute > 0 && self.row_start == self.absolute {
            self.total_rows = self.total_rows.saturating_sub(1);
            if self
                .anchors
                .last()
                .map(|anchor| anchor.offset == self.absolute)
                .unwrap_or(false)
            {
                self.anchors.pop();
            }
        }
        RowIndex {
            anchors: self.anchors,
            total_rows: self.total_rows,
        }
    }
}

#[derive(Clone)]
pub struct ResponseBodyStore {
    root: PathBuf,
    entries: Arc<Mutex<HashMap<String, BodyEntry>>>,
    search_jobs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    format_jobs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ResponseBodyStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            entries: Arc::new(Mutex::new(HashMap::new())),
            search_jobs: Arc::new(Mutex::new(HashMap::new())),
            format_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn store_bytes(
        &self,
        bytes: &[u8],
        content_type: Option<&str>,
    ) -> AppResult<StoredResponseBody> {
        tokio::fs::create_dir_all(&self.root).await?;
        let handle_id = Uuid::new_v4().to_string();
        let path = self.root.join(format!("{handle_id}.body"));
        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(bytes).await?;
        file.flush().await?;
        drop(file);

        self.insert_entry(
            handle_id.clone(),
            path,
            content_type.and_then(parse_charset),
        );
        Ok(describe_body(
            handle_id,
            bytes,
            content_type.map(str::to_string),
        ))
    }

    pub fn register_existing(
        &self,
        path: PathBuf,
        content_type: Option<String>,
        preview: &[u8],
        size_bytes: u64,
    ) -> AppResult<StoredResponseBody> {
        let handle_id = Uuid::new_v4().to_string();
        let charset = content_type.as_deref().and_then(parse_charset);
        self.insert_entry(handle_id.clone(), path, charset);
        if let Some(entry) = self
            .entries
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .get_mut(&handle_id)
        {
            entry.delete_on_release = false;
            entry.row_index = read_row_index(&entry.path).ok().map(Arc::new);
        }
        let mut descriptor = describe_body(handle_id, preview, content_type);
        descriptor.size_bytes = size_bytes;
        Ok(descriptor)
    }

    pub fn register_temporary(
        &self,
        handle_id: String,
        path: PathBuf,
        content_type: Option<String>,
        preview: &[u8],
        size_bytes: u64,
    ) -> AppResult<StoredResponseBody> {
        let charset = content_type.as_deref().and_then(parse_charset);
        self.insert_entry(handle_id.clone(), path, charset);
        let mut descriptor = describe_body(handle_id, preview, content_type);
        descriptor.size_bytes = size_bytes;
        Ok(descriptor)
    }

    pub(crate) fn register_temporary_with_index(
        &self,
        handle_id: String,
        path: PathBuf,
        content_type: Option<String>,
        preview: &[u8],
        size_bytes: u64,
        row_index: RowIndex,
    ) -> AppResult<StoredResponseBody> {
        write_row_index(&path, &row_index)?;
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        entries.insert(
            handle_id.clone(),
            BodyEntry {
                path,
                leases: 1,
                delete_on_release: true,
                row_index: Some(Arc::new(row_index)),
                charset: content_type.as_deref().and_then(parse_charset),
            },
        );
        drop(entries);
        let mut descriptor = describe_body(handle_id, preview, content_type);
        descriptor.size_bytes = size_bytes;
        Ok(descriptor)
    }

    pub fn retain(&self, handle_id: &str) -> AppResult<()> {
        let mut entries = self.lock_entries()?;
        let entry = entries
            .get_mut(handle_id)
            .ok_or_else(|| AppError::Message("Response body is no longer available.".into()))?;
        entry.leases = entry.leases.saturating_add(1);
        Ok(())
    }

    pub fn release(&self, handle_id: &str) -> AppResult<()> {
        let path_to_delete = {
            let mut entries = self.lock_entries()?;
            let Some(entry) = entries.get_mut(handle_id) else {
                return Ok(());
            };
            entry.leases = entry.leases.saturating_sub(1);
            if entry.leases == 0 {
                let removed = entries.remove(handle_id).expect("entry exists");
                let still_leased = entries
                    .values()
                    .any(|entry| entry.path == removed.path && entry.leases > 0);
                (removed.delete_on_release && !still_leased).then_some(removed.path)
            } else {
                None
            }
        };

        if let Some(path) = path_to_delete {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let _ = std::fs::remove_file(index_path(&path));
            let _ = std::fs::remove_file(display_path(&path));
        }
        Ok(())
    }

    pub fn mark_history_owned(&self, handle_id: &str, path: PathBuf) -> AppResult<()> {
        let mut entries = self.lock_entries()?;
        let entry = entries
            .get_mut(handle_id)
            .ok_or_else(|| AppError::Message("Response body is no longer available.".into()))?;
        let previous_index = index_path(&entry.path);
        let next_index = index_path(&path);
        if previous_index.exists() {
            let _ = std::fs::rename(previous_index, next_index);
        }
        entry.path = path;
        entry.delete_on_release = false;
        Ok(())
    }

    pub fn delete_path_when_released(&self, path: &Path) -> AppResult<bool> {
        let mut entries = self.lock_entries()?;
        let mut leased = false;
        for entry in entries.values_mut().filter(|entry| entry.path == path) {
            entry.delete_on_release = true;
            leased |= entry.leases > 0;
        }
        if !leased {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let _ = std::fs::remove_file(index_path(path));
            let _ = std::fs::remove_file(display_path(path));
        }
        Ok(leased)
    }

    pub fn path_for(&self, handle_id: &str) -> AppResult<PathBuf> {
        self.lock_entries()?
            .get(handle_id)
            .map(|entry| entry.path.clone())
            .ok_or_else(|| AppError::Message("Response body is no longer available.".into()))
    }

    async fn search_path_for(&self, handle_id: &str) -> AppResult<PathBuf> {
        let (path, charset) = {
            let entries = self.lock_entries()?;
            let entry = entries
                .get(handle_id)
                .ok_or_else(|| AppError::Message("Response body is no longer available.".into()))?;
            (entry.path.clone(), entry.charset.clone())
        };
        let encoding = charset
            .as_deref()
            .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
            .unwrap_or(encoding_rs::UTF_8);
        if encoding == encoding_rs::UTF_8 {
            return Ok(path);
        }
        let display = display_path(&path);
        if display.exists() {
            return Ok(display);
        }
        let source = path.clone();
        let destination = display.clone();
        tokio::task::spawn_blocking(move || {
            transcode_display_file(&source, &destination, encoding)
        })
        .await
        .map_err(|error| AppError::Message(format!("Response decoding task failed: {error}")))??;
        Ok(display)
    }

    pub async fn read_all_text(&self, handle_id: &str) -> AppResult<String> {
        let (path, charset) = {
            let entries = self.lock_entries()?;
            let entry = entries
                .get(handle_id)
                .ok_or_else(|| AppError::Message("Response body is no longer available.".into()))?;
            (entry.path.clone(), entry.charset.clone())
        };
        let bytes = tokio::fs::read(path).await?;
        Ok(decode_text(&bytes, charset.as_deref()))
    }

    pub async fn read_window(
        &self,
        handle_id: &str,
        start_row: u64,
        row_count: u64,
        max_bytes: usize,
    ) -> AppResult<ResponseBodyWindow> {
        let (path, charset) = {
            let entries = self.lock_entries()?;
            let entry = entries
                .get(handle_id)
                .ok_or_else(|| AppError::Message("Response body is no longer available.".into()))?;
            (entry.path.clone(), entry.charset.clone())
        };
        let index = self.ensure_row_index(handle_id, &path).await?;
        let total_rows = index.total_rows;
        if start_row >= total_rows {
            return Ok(ResponseBodyWindow {
                start_row,
                total_rows,
                rows: Vec::new(),
            });
        }
        let anchor = index
            .anchors
            .iter()
            .rev()
            .find(|anchor| anchor.row_index <= start_row)
            .cloned()
            .unwrap_or(RowLocation {
                row_index: 0,
                offset: 0,
                source_line: 0,
                segment_index: 0,
            });
        let mut file = tokio::fs::File::open(&path).await?;
        file.seek(std::io::SeekFrom::Start(anchor.offset)).await?;
        let mut reader = tokio::io::BufReader::with_capacity(SEARCH_BUFFER_SIZE, file);
        let mut current_row = anchor.row_index;
        let mut source_line = anchor.source_line;
        let mut segment_index = anchor.segment_index;
        let mut used = 0usize;
        let mut selected = Vec::new();
        while current_row < total_rows && selected.len() < row_count as usize {
            let row_source_line = source_line;
            let row_segment = segment_index;
            let Some((mut bytes, continues)) =
                read_display_row(&mut reader, &mut source_line, &mut segment_index).await?
            else {
                break;
            };
            while matches!(bytes.last(), Some(b'\n') | Some(b'\r')) {
                bytes.pop();
            }
            if current_row >= start_row {
                if used > 0 && used.saturating_add(bytes.len()) > max_bytes {
                    break;
                }
                used = used.saturating_add(bytes.len());
                selected.push(ResponseBodyRow {
                    key: format!("{}:{}", row_source_line, row_segment),
                    row_index: current_row,
                    source_line: row_source_line,
                    segment_index: row_segment,
                    text: decode_text(&bytes, charset.as_deref()),
                    continues,
                });
            }
            current_row += 1;
        }

        Ok(ResponseBodyWindow {
            start_row,
            total_rows,
            rows: selected,
        })
    }

    pub async fn search(
        &self,
        handle_id: &str,
        query: &str,
        case_sensitive: bool,
    ) -> AppResult<ResponseSearchResult> {
        self.search_internal(handle_id, query, case_sensitive, None, None, None)
            .await
    }

    pub async fn search_with_id(
        &self,
        search_id: &str,
        handle_id: &str,
        query: &str,
        case_sensitive: bool,
    ) -> AppResult<ResponseSearchResult> {
        let cancelled = Arc::new(AtomicBool::new(false));
        self.search_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .insert(search_id.to_string(), Arc::clone(&cancelled));
        let result = self
            .search_internal(
                handle_id,
                query,
                case_sensitive,
                Some(&cancelled),
                None,
                Some(search_id),
            )
            .await;
        self.search_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .remove(search_id);
        result
    }

    pub async fn search_with_progress(
        &self,
        search_id: &str,
        handle_id: &str,
        query: &str,
        case_sensitive: bool,
        progress: ResponseSearchProgressSink,
    ) -> AppResult<ResponseSearchResult> {
        let cancelled = Arc::new(AtomicBool::new(false));
        self.search_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .insert(search_id.to_string(), Arc::clone(&cancelled));
        let result = self
            .search_internal(
                handle_id,
                query,
                case_sensitive,
                Some(&cancelled),
                Some(&progress),
                Some(search_id),
            )
            .await;
        self.search_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .remove(search_id);
        result
    }

    pub fn cancel_search(&self, search_id: &str) {
        if let Some(cancelled) = self
            .search_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .get(search_id)
        {
            cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub async fn find_directional_match(
        &self,
        handle_id: &str,
        query: &str,
        case_sensitive: bool,
        from_offset: u64,
        forward: bool,
        wrap: bool,
    ) -> AppResult<Option<ResponseSearchMatch>> {
        let path = self.search_path_for(handle_id).await?;
        let needle = normalized_search_bytes(query.as_bytes(), case_sensitive);
        if needle.is_empty() {
            return Ok(None);
        }
        let found =
            scan_directional_match(&path, &needle, case_sensitive, Some(from_offset), forward)
                .await?;
        if found.is_some() || !wrap {
            return Ok(found);
        }
        scan_directional_match(&path, &needle, case_sensitive, None, forward).await
    }

    async fn search_internal(
        &self,
        handle_id: &str,
        query: &str,
        case_sensitive: bool,
        cancelled: Option<&AtomicBool>,
        progress: Option<&ResponseSearchProgressSink>,
        search_id: Option<&str>,
    ) -> AppResult<ResponseSearchResult> {
        if query.is_empty() {
            return Ok(ResponseSearchResult {
                total_matches: 0,
                capped: false,
                matches: Vec::new(),
            });
        }

        let path = self.search_path_for(handle_id).await?;
        let mut file = tokio::fs::File::open(&path).await?;
        let total_bytes = file.metadata().await?.len();
        let needle = normalized_search_bytes(query.as_bytes(), case_sensitive);
        let overlap_size = needle.len().saturating_sub(1);
        let mut buffer = vec![0u8; SEARCH_BUFFER_SIZE];
        let mut overlap = Vec::new();
        let mut file_offset = 0u64;
        let mut total_matches = 0u64;
        let mut matches = Vec::new();
        let mut base_state = DisplayRowState {
            row_index: 0,
            bytes_in_row: 0,
            utf8_remaining: 0,
        };
        let mut last_progress = std::time::Instant::now();
        let mut first_match_sent = false;

        loop {
            if cancelled
                .map(|flag| flag.load(Ordering::Relaxed))
                .unwrap_or(false)
            {
                return Err(AppError::Cancelled);
            }
            let read = file.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            let mut haystack = Vec::with_capacity(overlap.len() + read);
            haystack.extend_from_slice(&overlap);
            haystack.extend_from_slice(&buffer[..read]);
            let normalized = normalized_search_bytes(&haystack, case_sensitive);
            let base_offset = file_offset.saturating_sub(overlap.len() as u64);
            let mut states = Vec::with_capacity(haystack.len() + 1);
            let mut state = base_state;
            for byte in &haystack {
                states.push(state);
                state.advance(*byte);
            }
            states.push(state);

            for offset in find_overlapping(&normalized, &needle) {
                let absolute = base_offset + offset as u64;
                if file_offset > 0 && absolute.saturating_add(needle.len() as u64) <= file_offset {
                    continue;
                }
                total_matches += 1;
                if matches.len() < SEARCH_MATCH_LIMIT {
                    matches.push(ResponseSearchMatch {
                        byte_offset: absolute,
                        byte_length: needle.len() as u64,
                        row_index: states[offset].row_index,
                    });
                }
            }

            if let (Some(progress), Some(search_id)) = (progress, search_id) {
                let first_match = if !first_match_sent {
                    matches.first().cloned()
                } else {
                    None
                };
                if first_match.is_some()
                    || last_progress.elapsed() >= std::time::Duration::from_millis(100)
                {
                    progress(ResponseSearchProgress {
                        search_id: search_id.to_string(),
                        scanned_bytes: file_offset.saturating_add(read as u64),
                        total_bytes,
                        total_matches,
                        first_match: first_match.clone(),
                        finished: false,
                    });
                    first_match_sent |= first_match.is_some();
                    last_progress = std::time::Instant::now();
                }
            }

            overlap.clear();
            let retained = overlap_size.min(haystack.len());
            base_state = states[haystack.len() - retained];
            overlap.extend_from_slice(&haystack[haystack.len() - retained..]);
            file_offset += read as u64;
        }

        if let (Some(progress), Some(search_id)) = (progress, search_id) {
            progress(ResponseSearchProgress {
                search_id: search_id.to_string(),
                scanned_bytes: total_bytes,
                total_bytes,
                total_matches,
                first_match: None,
                finished: true,
            });
        }

        Ok(ResponseSearchResult {
            total_matches,
            capped: total_matches as usize > SEARCH_MATCH_LIMIT,
            matches,
        })
    }

    pub async fn read_hex_window(
        &self,
        handle_id: &str,
        start_row: u64,
        row_count: u64,
    ) -> AppResult<ResponseBodyWindow> {
        const BYTES_PER_ROW: u64 = 16;
        let path = self.path_for(handle_id)?;
        let mut file = tokio::fs::File::open(path).await?;
        let size = file.metadata().await?.len();
        let total_rows = size.saturating_add(BYTES_PER_ROW - 1) / BYTES_PER_ROW;
        let mut rows = Vec::new();
        file.seek(std::io::SeekFrom::Start(
            start_row.saturating_mul(BYTES_PER_ROW),
        ))
        .await?;
        for row_index in start_row..(start_row + row_count).min(total_rows) {
            let mut bytes = [0u8; BYTES_PER_ROW as usize];
            let read = file.read(&mut bytes).await?;
            if read == 0 {
                break;
            }
            let hex = bytes[..read]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            let ascii: String = bytes[..read]
                .iter()
                .map(|byte| {
                    if byte.is_ascii_graphic() || *byte == b' ' {
                        *byte as char
                    } else {
                        '.'
                    }
                })
                .collect();
            rows.push(ResponseBodyRow {
                key: format!("hex:{row_index}"),
                row_index,
                source_line: row_index,
                segment_index: 0,
                text: format!("{:08x}  {:<47}  {}", row_index * BYTES_PER_ROW, hex, ascii),
                continues: false,
            });
        }
        Ok(ResponseBodyWindow {
            start_row,
            total_rows,
            rows,
        })
    }

    pub async fn copy_to(&self, handle_id: &str, destination: &Path) -> AppResult<()> {
        tokio::fs::copy(self.path_for(handle_id)?, destination).await?;
        Ok(())
    }

    pub async fn format_json(&self, handle_id: &str) -> AppResult<StoredResponseBody> {
        self.format_json_internal(handle_id, None, None, None).await
    }

    pub async fn format_json_with_id(
        &self,
        job_id: &str,
        handle_id: &str,
        progress: Option<ResponseBodyJobProgressSink>,
    ) -> AppResult<StoredResponseBody> {
        let cancelled = Arc::new(AtomicBool::new(false));
        self.format_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .insert(job_id.to_string(), Arc::clone(&cancelled));
        let result = self
            .format_json_internal(handle_id, Some(&cancelled), progress.as_ref(), Some(job_id))
            .await;
        self.format_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .remove(job_id);
        result
    }

    pub fn cancel_job(&self, job_id: &str) {
        if let Some(cancelled) = self
            .format_jobs
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .get(job_id)
        {
            cancelled.store(true, Ordering::Relaxed);
        }
    }

    async fn format_json_internal(
        &self,
        handle_id: &str,
        cancelled: Option<&AtomicBool>,
        progress: Option<&ResponseBodyJobProgressSink>,
        job_id: Option<&str>,
    ) -> AppResult<StoredResponseBody> {
        tokio::fs::create_dir_all(&self.root).await?;
        let source_path = self.path_for(handle_id)?;
        let mut source = tokio::fs::File::open(source_path).await?;
        let total_bytes = source.metadata().await?.len();
        let formatted_id = Uuid::new_v4().to_string();
        let formatted_path = self.root.join(format!("{formatted_id}.formatted.body"));
        let mut destination = tokio::fs::File::create(&formatted_path).await?;
        let mut buffer = vec![0u8; SEARCH_BUFFER_SIZE];
        let mut preview = Vec::with_capacity(BODY_PREVIEW_LIMIT);
        let mut in_string = false;
        let mut escaped = false;
        let mut depth = 0usize;
        let mut containers = Vec::new();
        let mut written = 0u64;
        let mut row_index = ResponseRowIndexBuilder::new();
        let mut processed_bytes = 0u64;
        let mut last_progress = std::time::Instant::now();

        loop {
            if cancelled
                .map(|flag| flag.load(Ordering::Relaxed))
                .unwrap_or(false)
            {
                drop(destination);
                let _ = tokio::fs::remove_file(&formatted_path).await;
                return Err(AppError::Cancelled);
            }
            let read = source.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            processed_bytes += read as u64;
            for byte in &buffer[..read] {
                if in_string {
                    write_format_bytes(
                        &mut destination,
                        &[*byte],
                        &mut preview,
                        &mut written,
                        &mut row_index,
                    )
                    .await?;
                    if escaped {
                        escaped = false;
                    } else if *byte == b'\\' {
                        escaped = true;
                    } else if *byte == b'"' {
                        in_string = false;
                    }
                    continue;
                }

                match *byte {
                    b'"' => {
                        in_string = true;
                        write_format_bytes(
                            &mut destination,
                            b"\"",
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                    }
                    b'{' | b'[' => {
                        containers.push(*byte);
                        depth += 1;
                        write_format_bytes(
                            &mut destination,
                            &[*byte],
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                        write_indent(
                            &mut destination,
                            depth,
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                    }
                    b'}' | b']' => {
                        let expected = if *byte == b'}' { b'{' } else { b'[' };
                        if containers.pop() != Some(expected) {
                            drop(destination);
                            let _ = tokio::fs::remove_file(&formatted_path).await;
                            return Err(AppError::Message(
                                "Response body is not valid JSON.".into(),
                            ));
                        }
                        depth = depth.saturating_sub(1);
                        write_indent(
                            &mut destination,
                            depth,
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                        write_format_bytes(
                            &mut destination,
                            &[*byte],
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                    }
                    b',' => {
                        write_format_bytes(
                            &mut destination,
                            b",",
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                        write_indent(
                            &mut destination,
                            depth,
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                    }
                    b':' => {
                        write_format_bytes(
                            &mut destination,
                            b": ",
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?;
                    }
                    byte if byte.is_ascii_whitespace() => {}
                    _ => {
                        write_format_bytes(
                            &mut destination,
                            &[*byte],
                            &mut preview,
                            &mut written,
                            &mut row_index,
                        )
                        .await?
                    }
                }
            }
            if let (Some(progress), Some(job_id)) = (progress, job_id) {
                if last_progress.elapsed() >= std::time::Duration::from_millis(100) {
                    progress(ResponseBodyJobProgress {
                        job_id: job_id.to_string(),
                        processed_bytes,
                        total_bytes,
                        finished: false,
                    });
                    last_progress = std::time::Instant::now();
                }
            }
        }
        if in_string || !containers.is_empty() {
            drop(destination);
            let _ = tokio::fs::remove_file(&formatted_path).await;
            return Err(AppError::Message("Response body is not valid JSON.".into()));
        }
        destination.flush().await?;
        if let (Some(progress), Some(job_id)) = (progress, job_id) {
            progress(ResponseBodyJobProgress {
                job_id: job_id.to_string(),
                processed_bytes: total_bytes,
                total_bytes,
                finished: true,
            });
        }
        drop(destination);
        self.register_temporary_with_index(
            formatted_id,
            formatted_path,
            Some("application/json; charset=utf-8".into()),
            &preview,
            written,
            row_index.finish(),
        )
    }

    pub async fn read_preview(path: &Path) -> AppResult<Vec<u8>> {
        let mut file = tokio::fs::File::open(path).await?;
        let mut preview = vec![0u8; BODY_PREVIEW_LIMIT];
        let read = file.read(&mut preview).await?;
        preview.truncate(read);
        Ok(preview)
    }

    pub async fn file_size(path: &Path) -> AppResult<u64> {
        Ok(tokio::fs::metadata(path).await?.len())
    }

    pub async fn reconcile(&self, referenced: &[PathBuf]) -> AppResult<()> {
        tokio::fs::create_dir_all(&self.root).await?;
        let mut referenced: std::collections::HashSet<PathBuf> =
            referenced.iter().cloned().collect();
        for path in referenced.clone() {
            referenced.insert(index_path(&path));
        }
        let mut entries = tokio::fs::read_dir(&self.root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if entry.file_type().await?.is_file() && !referenced.contains(&path) {
                match tokio::fs::remove_file(path).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(())
    }

    pub async fn move_file(source: &Path, destination: &Path) -> AppResult<()> {
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        match tokio::fs::rename(source, destination).await {
            Ok(()) => Ok(()),
            Err(_) => {
                tokio::fs::copy(source, destination).await?;
                tokio::fs::remove_file(source).await?;
                Ok(())
            }
        }
    }

    fn insert_entry(&self, handle_id: String, path: PathBuf, charset: Option<String>) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        entries.insert(
            handle_id,
            BodyEntry {
                path,
                leases: 1,
                delete_on_release: true,
                row_index: None,
                charset,
            },
        );
    }

    fn lock_entries(&self) -> AppResult<std::sync::MutexGuard<'_, HashMap<String, BodyEntry>>> {
        self.entries
            .lock()
            .map_err(|_| AppError::Message("Failed to access response body state.".into()))
    }

    async fn ensure_row_index(&self, handle_id: &str, path: &Path) -> AppResult<Arc<RowIndex>> {
        if let Some(index) = self
            .lock_entries()?
            .get(handle_id)
            .and_then(|entry| entry.row_index.clone())
        {
            return Ok(index);
        }

        let mut file = tokio::fs::File::open(path).await?;
        let mut builder = ResponseRowIndexBuilder::new();
        let mut buffer = vec![0u8; SEARCH_BUFFER_SIZE];
        loop {
            let read = file.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            builder.push(&buffer[..read]);
        }
        let index = Arc::new(builder.finish());
        write_row_index(path, &index)?;
        if let Some(entry) = self.lock_entries()?.get_mut(handle_id) {
            entry.row_index = Some(Arc::clone(&index));
        }
        Ok(index)
    }
}

async fn scan_directional_match(
    path: &Path,
    needle: &[u8],
    case_sensitive: bool,
    from_offset: Option<u64>,
    forward: bool,
) -> AppResult<Option<ResponseSearchMatch>> {
    let mut file = tokio::fs::File::open(path).await?;
    let overlap_size = needle.len().saturating_sub(1);
    let mut buffer = vec![0u8; SEARCH_BUFFER_SIZE];
    let mut overlap = Vec::new();
    let mut file_offset = 0u64;
    let mut base_state = DisplayRowState {
        row_index: 0,
        bytes_in_row: 0,
        utf8_remaining: 0,
    };
    let mut candidate = None;
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let mut haystack = Vec::with_capacity(overlap.len() + read);
        haystack.extend_from_slice(&overlap);
        haystack.extend_from_slice(&buffer[..read]);
        let normalized = normalized_search_bytes(&haystack, case_sensitive);
        let base_offset = file_offset.saturating_sub(overlap.len() as u64);
        let mut states = Vec::with_capacity(haystack.len() + 1);
        let mut state = base_state;
        for byte in &haystack {
            states.push(state);
            state.advance(*byte);
        }
        states.push(state);
        for offset in find_overlapping(&normalized, needle) {
            let absolute = base_offset + offset as u64;
            if file_offset > 0 && absolute.saturating_add(needle.len() as u64) <= file_offset {
                continue;
            }
            let qualifies = from_offset
                .map(|from| {
                    if forward {
                        absolute > from
                    } else {
                        absolute < from
                    }
                })
                .unwrap_or(true);
            if !qualifies {
                continue;
            }
            let found = ResponseSearchMatch {
                byte_offset: absolute,
                byte_length: needle.len() as u64,
                row_index: states[offset].row_index,
            };
            if forward {
                return Ok(Some(found));
            }
            candidate = Some(found);
        }
        let retained = overlap_size.min(haystack.len());
        base_state = states[haystack.len() - retained];
        overlap.clear();
        overlap.extend_from_slice(&haystack[haystack.len() - retained..]);
        file_offset += read as u64;
    }
    Ok(candidate)
}

fn index_path(body_path: &Path) -> PathBuf {
    body_path.with_extension("idx")
}

fn display_path(body_path: &Path) -> PathBuf {
    body_path.with_extension("utf8")
}

fn transcode_display_file(
    source: &Path,
    destination: &Path,
    encoding: &'static encoding_rs::Encoding,
) -> AppResult<()> {
    if destination.exists() {
        return Ok(());
    }
    let temporary = destination.with_extension(format!("utf8-{}.tmp", Uuid::new_v4()));
    let result = (|| -> AppResult<()> {
        let mut input = std::io::BufReader::new(std::fs::File::open(source)?);
        let mut output = std::io::BufWriter::new(std::fs::File::create(&temporary)?);
        let mut decoder = encoding.new_decoder_without_bom_handling();
        let mut buffer = vec![0u8; SEARCH_BUFFER_SIZE];
        loop {
            let read = std::io::Read::read(&mut input, &mut buffer)?;
            let last = read == 0;
            let mut consumed = 0;
            loop {
                let mut decoded = String::with_capacity(SEARCH_BUFFER_SIZE * 3);
                let (status, used, _) =
                    decoder.decode_to_string(&buffer[consumed..read], &mut decoded, last);
                std::io::Write::write_all(&mut output, decoded.as_bytes())?;
                consumed += used;
                if status == encoding_rs::CoderResult::InputEmpty {
                    break;
                }
            }
            if last {
                std::io::Write::flush(&mut output)?;
                break;
            }
        }
        match std::fs::rename(&temporary, destination) {
            Ok(()) => Ok(()),
            Err(_) if destination.exists() => Ok(()),
            Err(error) => Err(error.into()),
        }
    })();
    if result.is_err() || destination.exists() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn write_row_index(body_path: &Path, index: &RowIndex) -> AppResult<()> {
    let mut bytes = Vec::with_capacity(8 + index.anchors.len().saturating_mul(28));
    bytes.extend_from_slice(&index.total_rows.to_le_bytes());
    for row in &index.anchors {
        bytes.extend_from_slice(&row.row_index.to_le_bytes());
        bytes.extend_from_slice(&row.offset.to_le_bytes());
        bytes.extend_from_slice(&row.source_line.to_le_bytes());
        bytes.extend_from_slice(&row.segment_index.to_le_bytes());
    }
    std::fs::write(index_path(body_path), bytes)?;
    Ok(())
}

fn read_row_index(body_path: &Path) -> AppResult<RowIndex> {
    let bytes = std::fs::read(index_path(body_path))?;
    if bytes.len() < 8 || (bytes.len() - 8) % 28 != 0 {
        return Err(AppError::Message(
            "Stored response row index is invalid.".into(),
        ));
    }
    let total_rows = u64::from_le_bytes(bytes[0..8].try_into().expect("fixed row count bytes"));
    let anchors = bytes[8..]
        .chunks_exact(28)
        .map(|chunk| RowLocation {
            row_index: u64::from_le_bytes(chunk[0..8].try_into().expect("fixed row bytes")),
            offset: u64::from_le_bytes(chunk[8..16].try_into().expect("fixed offset bytes")),
            source_line: u64::from_le_bytes(chunk[16..24].try_into().expect("fixed line bytes")),
            segment_index: u32::from_le_bytes(
                chunk[24..28].try_into().expect("fixed segment bytes"),
            ),
        })
        .collect();
    Ok(RowIndex {
        anchors,
        total_rows,
    })
}

async fn write_indent(
    destination: &mut tokio::fs::File,
    depth: usize,
    preview: &mut Vec<u8>,
    written: &mut u64,
    row_index: &mut ResponseRowIndexBuilder,
) -> AppResult<()> {
    write_format_bytes(destination, b"\n", preview, written, row_index).await?;
    let spaces = vec![b' '; depth.saturating_mul(2)];
    write_format_bytes(destination, &spaces, preview, written, row_index).await
}

async fn read_display_row<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    source_line: &mut u64,
    segment_index: &mut u32,
) -> AppResult<Option<(Vec<u8>, bool)>> {
    let mut row = Vec::with_capacity(DISPLAY_ROW_BYTES.min(4096));
    let mut utf8_remaining = 0u8;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return if row.is_empty() {
                Ok(None)
            } else {
                Ok(Some((row, false)))
            };
        }
        let mut consumed = 0usize;
        let mut completed = None;
        for byte in available {
            consumed += 1;
            if *byte == b'\n' {
                completed = Some(false);
                break;
            }
            utf8_remaining = next_utf8_remaining(utf8_remaining, *byte);
            if row.len() + consumed >= DISPLAY_ROW_BYTES && utf8_remaining == 0 {
                completed = Some(true);
                break;
            }
        }
        let content_length = if completed == Some(false) {
            consumed - 1
        } else {
            consumed
        };
        row.extend_from_slice(&available[..content_length]);
        reader.consume(consumed);
        if let Some(continues) = completed {
            if continues {
                *segment_index = segment_index.saturating_add(1);
            } else {
                *source_line = source_line.saturating_add(1);
                *segment_index = 0;
            }
            return Ok(Some((row, continues)));
        }
    }
}

fn next_utf8_remaining(current: u8, byte: u8) -> u8 {
    if current > 0 {
        return if byte & 0b1100_0000 == 0b1000_0000 {
            current - 1
        } else {
            0
        };
    }
    match byte {
        0xc2..=0xdf => 1,
        0xe0..=0xef => 2,
        0xf0..=0xf4 => 3,
        _ => 0,
    }
}

async fn write_format_bytes(
    destination: &mut tokio::fs::File,
    bytes: &[u8],
    preview: &mut Vec<u8>,
    written: &mut u64,
    row_index: &mut ResponseRowIndexBuilder,
) -> AppResult<()> {
    destination.write_all(bytes).await?;
    row_index.push(bytes);
    if preview.len() < BODY_PREVIEW_LIMIT {
        let remaining = BODY_PREVIEW_LIMIT - preview.len();
        preview.extend_from_slice(&bytes[..bytes.len().min(remaining)]);
    }
    *written = written.saturating_add(bytes.len() as u64);
    Ok(())
}

pub fn describe_inline(bytes: &[u8], content_type: Option<String>) -> StoredResponseBody {
    describe_body(String::new(), bytes, content_type)
}

fn describe_body(
    handle_id: String,
    bytes: &[u8],
    content_type: Option<String>,
) -> StoredResponseBody {
    let charset = content_type.as_deref().and_then(parse_charset);
    let presentation = classify_presentation(content_type.as_deref(), bytes);
    StoredResponseBody {
        handle_id,
        preview_text: String::from_utf8_lossy(&bytes[..bytes.len().min(BODY_PREVIEW_LIMIT)])
            .into_owned(),
        size_bytes: bytes.len() as u64,
        content_type,
        charset,
        presentation,
    }
}

fn parse_charset(value: &str) -> Option<String> {
    value.split(';').skip(1).find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches('"').to_string())
    })
}

pub(crate) fn decode_text(bytes: &[u8], charset: Option<&str>) -> String {
    let encoding = charset
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (decoded, _) = encoding.decode_without_bom_handling(bytes);
    decoded.into_owned()
}

fn classify_presentation(content_type: Option<&str>, preview: &[u8]) -> ResponsePresentation {
    let media_type = content_type
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if media_type == "application/json" || media_type.ends_with("+json") || looks_like_json(preview)
    {
        ResponsePresentation::Json
    } else if media_type.starts_with("image/") {
        ResponsePresentation::Image
    } else if media_type.starts_with("text/")
        || media_type.contains("xml")
        || media_type.contains("javascript")
        || std::str::from_utf8(preview).is_ok()
    {
        ResponsePresentation::Text
    } else {
        ResponsePresentation::Binary
    }
}

fn looks_like_json(bytes: &[u8]) -> bool {
    matches!(
        bytes
            .iter()
            .copied()
            .find(|byte| !byte.is_ascii_whitespace()),
        Some(b'{') | Some(b'[')
    )
}

fn normalized_search_bytes(bytes: &[u8], case_sensitive: bool) -> Vec<u8> {
    if case_sensitive {
        bytes.to_vec()
    } else {
        let mut normalized = bytes.to_vec();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index].is_ascii() {
                normalized[index] = bytes[index].to_ascii_lowercase();
                index += 1;
                continue;
            }
            let width = utf8_char_width(bytes[index]);
            if width == 0 || index + width > bytes.len() {
                index += 1;
                continue;
            }
            if let Ok(value) = std::str::from_utf8(&bytes[index..index + width]) {
                if let Some(character) = value.chars().next() {
                    let lowered = character.to_lowercase().collect::<String>();
                    if lowered.len() == width {
                        normalized[index..index + width].copy_from_slice(lowered.as_bytes());
                    }
                }
            }
            index += width;
        }
        normalized
    }
}

fn utf8_char_width(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

fn find_overlapping(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let finder = memchr::memmem::Finder::new(needle);
    let mut matches = Vec::new();
    let mut cursor = 0usize;
    while cursor + needle.len() <= haystack.len() {
        let Some(relative) = finder.find(&haystack[cursor..]) else {
            break;
        };
        let offset = cursor + relative;
        matches.push(offset);
        cursor = offset + 1;
    }
    matches
}
