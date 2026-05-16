use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub use crate::clipboard_db::ClipboardContent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub content_type: String,
    pub content: ClipboardContent,
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
            println!("Clipboard check - text: len={}, hash={}, last={}", text.len(), hash, last);
            if hash != last {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let item = ClipboardItem {
                    content_type: "text".to_string(),
                    content: ClipboardContent::Text(text.clone()),
                    timestamp: now,
                };

                println!("Emitting TEXT clipboard event: len={}", text.len());
                let result = app_handle.emit("clipboard-change", &item);
                println!("Emit result: {:?}", result);
                last_text_hash.store(hash, Ordering::SeqCst);

                if let Some(service) = app_handle.try_state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>() {
                    let content = ClipboardContent::Text(text);
                    if let Err(e) = service.on_clipboard_change(content) {
                        eprintln!("Failed to save clipboard to DB: {}", e);
                    }
                }
            }
        } else {
            println!("Clipboard check - no text available");
        }

        if let Ok(image) = app_handle.clipboard().read_image() {
            let rgba = image.rgba().to_vec();
            let width = image.width();
            let height = image.height();
            let hash = Self::hash_bytes(&rgba);
            let last = last_image_hash.load(Ordering::SeqCst);
            println!("Clipboard check - image: {}x{}, hash={}, last={}", width, height, hash, last);
            if hash != last {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let item = ClipboardItem {
                    content_type: "image".to_string(),
                    content: ClipboardContent::Image {
                        width,
                        height,
                        rgba: rgba.clone(),
                    },
                    timestamp: now,
                };

                println!("Emitting IMAGE clipboard event: {}x{}", width, height);
                let result = app_handle.emit("clipboard-change", &item);
                println!("Emit result: {:?}", result);
                last_image_hash.store(hash, Ordering::SeqCst);

                if let Some(service) = app_handle.try_state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>() {
                    let content = ClipboardContent::Image { width, height, rgba };
                    if let Err(e) = service.on_clipboard_change(content) {
                        eprintln!("Failed to save clipboard to DB: {}", e);
                    }
                }
            }
        }
    }

    fn hash_string(s: &str) -> u64 {
        let mut hash: u64 = 0;
        for (i, byte) in s.bytes().enumerate() {
            hash = hash.wrapping_add((byte as u64).wrapping_mul(i as u64 + 1));
        }
        hash
    }

    fn hash_bytes(bytes: &[u8]) -> u64 {
        let mut hash: u64 = 0;
        for (i, &byte) in bytes.iter().enumerate() {
            hash = hash.wrapping_add((byte as u64).wrapping_mul(i as u64 + 1));
        }
        hash
    }
}
