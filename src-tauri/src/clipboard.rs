/// clipboard.rs — 剪贴板监听管理器（Clipboard Manager）
///
/// 职责：
/// - 轮询系统剪贴板（每 500ms 一次），检测文本和图片变化
/// - 通过 xxhash3_64 哈希值去重，避免重复记录
/// - 检测到新内容时调用 ClipboardService 处理业务逻辑
/// - 通过 Tauri 事件系统将变更通知前端（clipboard-change 事件）
///
/// 去重策略：
/// - 维护 last_text_hash 和 last_image_hash 两个原子变量
/// - 文本和图片的哈希独立跟踪
/// - 检测到文本变化时重置图片哈希（反之亦然），避免文本/图片交替复制时的遗漏
///
/// 数据流：
/// 系统剪贴板 → ClipboardManager.check_clipboard() → ClipboardService.on_clipboard_change()
///           → ClipboardRepository.create() → SQLite
///           → Tauri emit("clipboard-change") → 前端 Vue
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub use crate::clipboard_db::ClipboardContent;
pub use crate::tag_db::Tag;

/// 向前端发送的剪贴板条目数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub content: ClipboardContent,
    pub tags: Vec<Tag>,
    pub timestamp: u64,
}

/// 剪贴板监听管理器
///
/// 在独立线程中轮询剪贴板变化，检测到新内容后通过 ClipboardService 持久化，
/// 并通过 Tauri 事件系统通知前端
pub struct ClipboardManager {
    app_handle: AppHandle,
    /// 上次检测到的文本内容哈希（xxhash3_64），用于文本去重
    last_text_hash: Arc<std::sync::atomic::AtomicU64>,
    /// 上次检测到的图片内容哈希（xxhash3_64），用于图片去重
    last_image_hash: Arc<std::sync::atomic::AtomicU64>,
    /// 监听线程运行标志
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

    /// 启动剪贴板监听线程
    ///
    /// 创建独立线程，每 500ms 调用 check_clipboard 检查剪贴板变化
    /// 若已在运行则直接返回（防止重复启动）
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

    /// 停止剪贴板监听线程
    #[allow(dead_code)]
    pub fn stop_listening(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// 检查剪贴板变化（核心检测逻辑）
    ///
    /// 检测优先级：文本优先于图片
    /// - 先尝试读取文本，若有文本内容则与上次文本哈希比较
    /// - 若文本读取失败（剪贴板中无文本），再尝试读取图片
    /// - 文本变化时会重置图片哈希为 0，反之亦然
    ///
    /// 当检测到新内容时：
    /// 1. 调用 ClipboardService.on_clipboard_change() 进行去重和持久化
    /// 2. 若返回 Some(record)，说明是新记录，通过 emit 发送给前端
    /// 3. 对于图片内容，发送前会剥离 original 数据（仅发送缩略图），减少传输量
    fn check_clipboard(
        app_handle: &AppHandle,
        last_text_hash: &Arc<std::sync::atomic::AtomicU64>,
        last_image_hash: &Arc<std::sync::atomic::AtomicU64>,
    ) {
        // 优先检测文本
        if let Ok(text) = app_handle.clipboard().read_text() {
            let hash = Self::hash_string(&text);
            let last = last_text_hash.load(Ordering::SeqCst);
            if hash != last {
                if let Some(service) = app_handle.try_state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>() {
                    let content = ClipboardContent::Text(text);
                    match service.on_clipboard_change(content) {
                        Ok(Some(record)) => {
                            // 文本变化时重置图片哈希，避免后续图片检测误判
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
                            // 内容重复（与数据库最新记录哈希相同），仅更新内存中的哈希
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

        // 剪贴板无文本，检测图片
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
                            // 图片事件：剥离 original 数据，仅发送缩略图给前端（节省带宽）
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

    /// 使用 xxhash3 计算字符串的 64 位哈希值
    fn hash_string(s: &str) -> u64 {
        xxhash_rust::xxh3::xxh3_64(s.as_bytes())
    }

    /// 使用 xxhash3 计算字节数组的 64 位哈希值
    fn hash_bytes(bytes: &[u8]) -> u64 {
        xxhash_rust::xxh3::xxh3_64(bytes)
    }
}
