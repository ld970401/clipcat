use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub use crate::clipboard_db::ClipboardContent;
pub use crate::tag_db::Tag;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub content: ClipboardContent,
    pub tags: Vec<Tag>,
    pub timestamp: u64,
}

pub struct ClipboardManager {
    app_handle: AppHandle,
    last_text_hash: Arc<std::sync::atomic::AtomicU64>,
    last_image_hash: Arc<std::sync::atomic::AtomicU64>,
    running: Arc<AtomicBool>,
}

impl ClipboardManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            last_text_hash: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_image_hash: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_listening(&self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(true, Ordering::SeqCst);
        let app_handle = self.app_handle.clone();
        let last_text_hash = Arc::clone(&self.last_text_hash);
        let last_image_hash = Arc::clone(&self.last_image_hash);
        let running = Arc::clone(&self.running);

        std::thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                Self::check_clipboard(&app_handle, &last_text_hash, &last_image_hash);
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    }

    #[allow(dead_code)]
    pub fn stop_listening(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn check_clipboard(
        app_handle: &AppHandle,
        last_text_hash: &Arc<std::sync::atomic::AtomicU64>,
        last_image_hash: &Arc<std::sync::atomic::AtomicU64>,
    ) {
        if let Ok(text) = app_handle.clipboard().read_text() {
            let hash = Self::hash_string(&text);
            let last = last_text_hash.load(Ordering::SeqCst);
            if hash != last {
                if let Some(service) = app_handle.try_state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>() {
                    let content = ClipboardContent::Text(text);
                    match service.on_clipboard_change(content) {
                        Ok(Some(record)) => {
                            last_text_hash.store(hash, Ordering::SeqCst);
                            last_image_hash.store(0, Ordering::SeqCst);
                            let item = ClipboardItem {
                                id: record.id,
                                content_type: record.content_type.clone(),
                                content: record.content,
                                tags: record.tags,
                                timestamp: record.created_at,
                            };
                            if let Err(e) = app_handle.emit("clipboard-change", &item) {
                                eprintln!("Failed to emit clipboard event: {}", e);
                            }
                        }
                        Ok(None) => {
                            last_text_hash.store(hash, Ordering::SeqCst);
                        }
                        Err(e) => {
                            eprintln!("Failed to save clipboard to DB: {}", e);
                        }
                    }
                }
            }
            return;
        }

        if let Ok(image) = app_handle.clipboard().read_image() {
            let rgba = image.rgba().to_vec();
            let width = image.width();
            let height = image.height();
            let hash = Self::hash_bytes(&rgba);
            let last = last_image_hash.load(Ordering::SeqCst);
            if hash != last {
                if let Some(service) = app_handle.try_state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>() {
                    let content = ClipboardContent::Image { width, height, thumbnail: None, original: Some(rgba) };
                    match service.on_clipboard_change(content) {
                        Ok(Some(record)) => {
                            last_image_hash.store(hash, Ordering::SeqCst);
                            let emit_content = match &record.content {
                                ClipboardContent::Image { width, height, thumbnail, original: _ } => {
                                    ClipboardContent::Image {
                                        width: *width,
                                        height: *height,
                                        thumbnail: thumbnail.clone(),
                                        original: None,
                                    }
                                }
                                other => other.clone(),
                            };
                            let item = ClipboardItem {
                                id: record.id,
                                content_type: record.content_type.clone(),
                                content: emit_content,
                                tags: record.tags,
                                timestamp: record.created_at,
                            };
                            if let Err(e) = app_handle.emit("clipboard-change", &item) {
                                eprintln!("Failed to emit clipboard event: {}", e);
                            }
                        }
                        Ok(None) => {
                            last_image_hash.store(hash, Ordering::SeqCst);
                        }
                        Err(e) => {
                            eprintln!("Failed to save clipboard to DB: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn hash_string(s: &str) -> u64 {
        xxhash_rust::xxh3::xxh3_64(s.as_bytes())
    }

    fn hash_bytes(bytes: &[u8]) -> u64 {
        xxhash_rust::xxh3::xxh3_64(bytes)
    }
}
