/// paste.rs — Tauri 命令处理层
///
/// 职责：
/// - 定义所有 `#[tauri::command]` 函数，供前端通过 invoke 调用
/// - 剪贴板粘贴/复制操作（含平台相关的粘贴模拟）
/// - 剪贴板历史的 CRUD 命令
/// - 设置的读写命令
/// - 标签的 CRUD 命令
///
/// 平台差异：
/// - macOS: 使用 AppleScript 模拟 Cmd+V 粘贴（需辅助功能权限）
/// - 其他平台: 使用 enigo 库模拟 Ctrl+V 粘贴
#[cfg(not(target_os = "macos"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::clipboard::{ClipboardContent, ClipboardItem};

/// macOS 平台：通过 AppleScript 模拟 Cmd+V 粘贴操作
///
/// 需要辅助功能权限（Accessibility Permission），否则会报 -1743 错误
/// 错误时会自动打开系统偏好设置的辅助功能面板，提示用户授权
#[cfg(target_os = "macos")]
fn simulate_paste() -> Result<(), String> {
    use std::process::Command;

    let script = r#"
        tell application "System Events"
            keystroke "v" using command down
        end tell
    "#;

    let output = Command::new("osascript")
        .args(["-e", script])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("AppleScript error: {}", stderr);
                if stderr.contains("-1743") || stderr.contains("未获得授权") {
                    let msg = "开发阶段需要给【终端】添加辅助功能权限。\n\n请执行以下步骤：\n1. 系统偏好设置 > 安全性与隐私 > 隐私 > 辅助功能\n2. 点击左下角解锁按钮\n3. 点击 + 按钮添加应用\n4. 选择【应用程序】>【实用工具】>【终端.app】（或 iTerm）\n5. 重新运行 pnpm dev\n\n提示：也可以使用 tccutil reset accessibility 重置权限后重新授权";
                    eprintln!("{}", msg);
                    Command::new("bash")
                        .args(["-c", "open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'"])
                        .spawn()
                        .ok();
                    Err(msg.to_string())
                } else {
                    Err(format!("AppleScript failed: {}", stderr))
                }
            }
        }
        Err(e) => Err(format!("Failed to execute AppleScript: {}", e)),
    }
}

/// 非 macOS 平台：通过 enigo 库模拟 Ctrl+V 粘贴操作
#[cfg(not(target_os = "macos"))]
fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create input controller: {:?}", e))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| format!("Failed to press ctrl: {:?}", e))?;

    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Failed to type v: {:?}", e))?;

    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| format!("Failed to release ctrl: {:?}", e))?;

    Ok(())
}

/// 将 ClipboardItem 的内容写入系统剪贴板
///
/// - 文本：直接写入剪贴板
/// - 图片：从 RGBA 数据重建 Image 对象后写入剪贴板
///   会校验图片尺寸和数据长度的匹配关系
fn write_to_clipboard(app: &AppHandle, item: &ClipboardItem) -> Result<(), String> {
    match &item.content {
        ClipboardContent::Text(text) => {
            if text.is_empty() {
                return Err("Text content is empty".to_string());
            }
            app.clipboard()
                .write_text(text)
                .map_err(|e| format!("Failed to write text: {}", e))
        }
        ClipboardContent::Image { width, height, thumbnail: _, original } => {
            let rgba = original.as_ref().ok_or("Image content missing original data")?;
            if *width == 0 || *height == 0 {
                eprintln!("Invalid image dimensions: {}x{}", width, height);
                return Err(format!("Invalid image dimensions: {}x{}", width, height));
            }
            // 校验 RGBA 数据长度 = width * height * 4（每像素 4 字节）
            let expected_len = (*width as usize) * (*height as usize) * 4;
            if rgba.len() != expected_len {
                eprintln!("Image data size mismatch: expected {} bytes, got {}", expected_len, rgba.len());
                return Err(format!("Image data size mismatch: expected {} bytes, got {}", expected_len, rgba.len()));
            }
            let image = tauri::image::Image::new(rgba.as_slice(), *width, *height);
            app.clipboard()
                .write_image(&image)
                .map_err(|e| format!("Failed to write image: {}", e))
        }
    }
}

/// 隐藏主面板（快速隐藏，不带动画）
///
/// 直接将窗口移到屏幕下方并隐藏，用于粘贴操作后快速收起面板
fn hide_panel(app: &AppHandle) {
    let win = match app.get_webview_window("main") {
        Some(w) => w,
        None => {
            eprintln!("hide_panel: window not found");
            return;
        }
    };

    let state = app.state::<crate::AppState>();
    if !state.window_visible.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    state.window_visible.store(false, std::sync::atomic::Ordering::SeqCst);

    // 快速下移窗口位置（非动画），然后隐藏
    if let Ok(pos) = win.outer_position() {
        let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            pos.x,
            pos.y + super::RISE_OFFSET,
        )));
    }

    let _ = win.hide();
}

/// 粘贴条目到当前活动应用
///
/// 核心粘贴流程：
/// 1. 若 move_to_top 为 true，先复制记录到顶部再删除原记录
/// 2. 将内容写入系统剪贴板
/// 3. 延迟 50ms 后模拟粘贴按键（Cmd+V / Ctrl+V）
/// 4. 隐藏面板
/// 5. 返回记录数据（图片类型会剥离 original 以节省带宽）
#[tauri::command]
pub async fn paste_item(app: AppHandle, id: i64, move_to_top: Option<bool>) -> Result<Option<crate::clipboard_db::ClipboardRecord>, String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();

    // move_to_top: 复制新记录到顶部 + 删除原记录
    let record = if move_to_top.unwrap_or(false) {
        let repo = &service.clipboard_repo;
        let new_record = repo.duplicate_and_move_to_top(id).map_err(|e| e.to_string())?;
        repo.delete(id).map_err(|e| e.to_string())?;
        new_record
    } else {
        service.get_record(id)?.ok_or("Record not found")?
    };

    let item = ClipboardItem {
        id: record.id,
        content_type: record.content_type.clone(),
        content: record.content.clone(),
        tags: record.tags.clone(),
        timestamp: record.created_at,
    };

    // 将内容写入系统剪贴板
    write_to_clipboard(&app, &item)?;

    // 延迟后模拟粘贴（等待剪贴板写入完成）
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Err(e) = simulate_paste() {
            eprintln!("Failed to simulate paste: {}", e);
        }
    });
    hide_panel(&app);

    // 返回结果时剥离图片原始数据
    if move_to_top.unwrap_or(false) {
        let mut result = record;
        if let crate::clipboard_db::ClipboardContent::Image { width, height, thumbnail, original: _ } = &result.content {
            result.content = crate::clipboard_db::ClipboardContent::Image {
                width: *width,
                height: *height,
                thumbnail: thumbnail.clone(),
                original: None,
            };
        }
        Ok(Some(result))
    } else {
        let mut result = record;
        if let crate::clipboard_db::ClipboardContent::Image { width, height, thumbnail, original: _ } = &result.content {
            result.content = crate::clipboard_db::ClipboardContent::Image {
                width: *width,
                height: *height,
                thumbnail: thumbnail.clone(),
                original: None,
            };
        }
        Ok(Some(result))
    }
}

/// 复制条目到系统剪贴板（不模拟粘贴，不隐藏面板）
#[tauri::command]
pub async fn copy_to_clipboard(app: AppHandle, id: i64) -> Result<(), String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    let record = service.get_record(id)?.ok_or("Record not found")?;

    let item = ClipboardItem {
        id: record.id,
        content_type: record.content_type.clone(),
        content: record.content.clone(),
        tags: record.tags.clone(),
        timestamp: record.created_at,
    };

    write_to_clipboard(&app, &item)?;
    Ok(())
}

/// 获取剪贴板历史列表
///
/// - `page`: 页码（从 0 开始）
/// - `page_size`: 每页条数
/// - `tag_id`: 可选标签过滤
#[tauri::command]
pub async fn get_clipboard_history(
    app: AppHandle,
    page: u32,
    page_size: Option<u32>,
    tag_id: Option<i64>,
) -> Result<Vec<crate::clipboard_db::ClipboardRecord>, String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    service.get_history(page, page_size, tag_id)
}

/// 切换条目置顶状态
///
/// 返回切换后的新状态（true = 已置顶）
#[tauri::command]
pub async fn toggle_pin(
    app: AppHandle,
    id: i64,
) -> Result<bool, String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    service.toggle_pin(id)
}

/// 删除剪贴板条目
#[tauri::command]
pub async fn delete_clipboard_item(
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    service.delete_record(id)
}

/// 应用设置数据结构
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    pub save_mode: String,
    pub retention_duration: i64,
    pub retention_count: i64,
    pub cleanup_time: String,
    pub shortcut_show: String,
    pub shortcut_hide: String,
    pub locale: String,
    pub launch_at_login: bool,
}

/// 获取应用设置
///
/// 从数据库读取所有设置项，缺失项使用默认值
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let db = app.state::<std::sync::Arc<crate::database::Database>>();
    let save_mode = db.get_setting("save_mode").map_err(|e| e.to_string())?.unwrap_or_else(|| "duration".to_string());
    let retention_duration = db.get_setting("retention_duration").map_err(|e| e.to_string())?.unwrap_or_else(|| "30".to_string()).parse().unwrap_or(30);
    let retention_count = db.get_setting("retention_count").map_err(|e| e.to_string())?.unwrap_or_else(|| "500".to_string()).parse().unwrap_or(500);
    let cleanup_time = db.get_setting("cleanup_time").map_err(|e| e.to_string())?.unwrap_or_else(|| "00:00".to_string());
    let shortcut_show = db.get_setting("shortcut_show").map_err(|e| e.to_string())?.unwrap_or_else(|| "CmdOrCtrl+Shift+V".to_string());
    let shortcut_hide = db.get_setting("shortcut_hide").map_err(|e| e.to_string())?.unwrap_or_else(|| "Escape".to_string());
    let locale = db.get_setting("locale").map_err(|e| e.to_string())?.unwrap_or_else(|| "en".to_string());
    let launch_at_login = db.get_setting("launch_at_login").map_err(|e| e.to_string())?.unwrap_or_else(|| "false".to_string()).parse().unwrap_or(false);

    Ok(AppSettings {
        save_mode,
        retention_duration,
        retention_count,
        cleanup_time,
        shortcut_show,
        shortcut_hide,
        locale,
        launch_at_login,
    })
}

/// 保存应用设置
///
/// 流程：
/// 1. 将所有设置项写入数据库
/// 2. 更新开机自启状态（通过 tauri-plugin-autostart）
/// 3. 重新注册快捷键（快捷键可能已变更）
/// 4. 发射 settings-changed 事件通知前端
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let db = app.state::<std::sync::Arc<crate::database::Database>>();
    db.set_setting("save_mode", &settings.save_mode).map_err(|e| e.to_string())?;
    db.set_setting("retention_duration", &settings.retention_duration.to_string()).map_err(|e| e.to_string())?;
    db.set_setting("retention_count", &settings.retention_count.to_string()).map_err(|e| e.to_string())?;
    db.set_setting("cleanup_time", &settings.cleanup_time).map_err(|e| e.to_string())?;
    db.set_setting("shortcut_show", &settings.shortcut_show).map_err(|e| e.to_string())?;
    db.set_setting("shortcut_hide", &settings.shortcut_hide).map_err(|e| e.to_string())?;
    db.set_setting("locale", &settings.locale).map_err(|e| e.to_string())?;
    db.set_setting("launch_at_login", &settings.launch_at_login.to_string()).map_err(|e| e.to_string())?;

    // 更新开机自启状态
    {
        use tauri_plugin_autostart::ManagerExt;
        let autostart_manager = app.autolaunch();
        if settings.launch_at_login {
            let _ = autostart_manager.enable();
        } else {
            let _ = autostart_manager.disable();
        }
    }

    // 快捷键变更后需重新注册
    {
        let shortcut_manager = crate::shortcuts::ShortcutManager::new(app.clone());
        if let Err(e) = shortcut_manager.reregister_shortcuts(&settings.shortcut_show, &settings.shortcut_hide) {
            eprintln!("Failed to re-register shortcuts: {}", e);
        }
    }

    app.emit("settings-changed", &settings).map_err(|e| e.to_string())?;

    Ok(())
}

/// 标签数据传输对象（用于 Tauri 命令返回，简化字段）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TagData {
    pub id: i64,
    pub name: String,
    pub color: String,
}

/// 创建标签
#[tauri::command]
pub async fn create_tag(app: AppHandle, name: String, color: Option<String>) -> Result<TagData, String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    let tag = service.create_tag(name, color)?;
    Ok(TagData {
        id: tag.id,
        name: tag.name,
        color: tag.color,
    })
}

/// 更新标签
#[tauri::command]
pub async fn update_tag(app: AppHandle, id: i64, name: String, color: String) -> Result<(), String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    service.update_tag(id, name, color)
}

/// 删除标签
#[tauri::command]
pub async fn delete_tag(app: AppHandle, id: i64) -> Result<(), String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    service.delete_tag(id)
}

/// 获取所有标签
#[tauri::command]
pub async fn get_all_tags(app: AppHandle) -> Result<Vec<TagData>, String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    let tags = service.list_tags()?;
    Ok(tags.into_iter().map(|t| TagData {
        id: t.id,
        name: t.name,
        color: t.color,
    }).collect())
}

/// 更新记录的标签关联
#[tauri::command]
pub async fn update_record_tags(app: AppHandle, id: i64, tag_ids: Vec<i64>) -> Result<(), String> {
    let service = app.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
    service.update_record_tags(id, tag_ids)
}
