#![allow(deprecated)]

//! lib.rs — 应用程序核心入口与窗口管理
//!
//! 职责：
//! - Tauri 应用初始化（插件注册、状态管理、命令注册）
//! - 主窗口的 NSPanel 配置（macOS 专属，浮于 Dock 之上）
//! - 面板上升/下降动画（cubic 缓动）
//! - 窗口焦点丢失时自动隐藏面板
//! - 定时清理任务（按时间或数量清理历史记录）
//! - 设置窗口的打开/关闭
//! - macOS Dock 点击事件处理（Reopen）
//!
//! 窗口行为：
//! - 主面板默认隐藏在屏幕底部（偏移 RISE_OFFSET 像素）
//! - 快捷键触发时，面板从底部滑入（上升动画）
//! - 焦点丢失后，面板滑出屏幕底部（下降动画）并隐藏
//! - 上升动画使用 ease-out 缓动 (1 - (1-t)³)
//! - 下降动画使用 ease-in 缓动 (t³)

mod clipboard;
mod clipboard_db;
mod clipboard_service;
mod database;
mod image_processor;
mod paste;
mod shortcuts;
mod tag_db;

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow, WindowEvent};
use tokio::time::sleep;

#[cfg(target_os = "macos")]
use tauri_nspanel::builder::{CollectionBehavior, PanelLevel, StyleMask};
#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, WebviewWindowExt};

#[cfg(target_os = "windows")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(target_os = "windows")]
use tauri::menu::{Menu, MenuItem};

// macOS NSPanel 宏定义
//
// 配置 NSPanel 的行为：
// - is_floating_panel: 浮动面板，不被其他窗口遮挡
// - can_become_key_window: 可接收键盘焦点
// - can_become_main_window: 不可成为主窗口（避免影响其他应用的主窗口状态）
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(ClipcatPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: false
        }
    })
}

/// 动画步数（越大越平滑）
const ANIMATION_STEPS: u32 = 20;
/// 每步动画间隔（毫秒）
const ANIMATION_STEP_DURATION_MS: u64 = 10;
/// 窗口高度占屏幕高度的比例（40%）
const WINDOW_HEIGHT_RATIO: f64 = 0.40;
/// 面板隐藏时的垂直偏移量（像素），窗口初始位置在目标位置下方此距离
pub const RISE_OFFSET: i32 = 800;
/// Dock 高度补偿（当前设为 0，预留接口）
const DOCK_HEIGHT: i32 = 0;

/// 应用全局状态
///
/// 使用原子类型实现无锁并发访问，所有字段均可在线程间安全共享
struct AppState {
    /// 主面板窗口是否可见
    window_visible: Arc<AtomicBool>,
    /// 面板目标 X 坐标（屏幕居中）
    target_x: Arc<AtomicI32>,
    /// 面板目标 Y 坐标（屏幕底部）
    target_y: Arc<AtomicI32>,
    /// 面板高度
    window_height: Arc<AtomicU32>,
    /// 面板宽度
    window_width: Arc<AtomicU32>,
    /// 上次面板显示的时间戳（毫秒），用于防止焦点丢失事件与显示事件冲突
    last_show_time: Arc<std::sync::atomic::AtomicU64>,
}

/// 面板上升动画（从屏幕底部滑入）
///
/// 使用 ease-out 缓动函数：ease_progress = 1 - (1 - t)³
/// 效果：开始快、结束慢，模拟自然减速
///
/// # 参数
/// - `window`: Tauri 窗口引用
/// - `target_x`: 目标 X 坐标
/// - `target_y`: 目标 Y 坐标（屏幕底部）
/// - `window_height`: 窗口高度
/// - `window_width`: 窗口宽度
async fn animate_window_rise(window: WebviewWindow, target_x: i32, target_y: i32, window_height: u32, window_width: u32) {
    // 起始位置在目标位置下方 RISE_OFFSET 像素
    let start_y = target_y + RISE_OFFSET;

    if let Err(e) = window.set_size(Size::Physical(PhysicalSize::new(window_width, window_height))) {
        eprintln!("Failed to set window size: {}", e);
        return;
    }

    if let Err(e) = window.set_position(Position::Physical(PhysicalPosition::new(target_x, start_y))) {
        eprintln!("Failed to set initial position: {}", e);
        return;
    }

    // 逐步移动窗口，使用 cubic ease-out 缓动
    for i in 1..=ANIMATION_STEPS {
        let progress = i as f64 / ANIMATION_STEPS as f64;
        let ease_progress = 1.0 - (1.0 - progress).powi(3);
        let current_y = start_y + ((target_y - start_y) as f64 * ease_progress) as i32;

        if let Err(e) = window.set_position(Position::Physical(PhysicalPosition::new(target_x, current_y))) {
            eprintln!("Failed to set position: {}", e);
            return;
        }

        sleep(Duration::from_millis(ANIMATION_STEP_DURATION_MS)).await;
    }
}

/// 面板下降动画（从屏幕底部滑出）
///
/// 使用 ease-in 缓动函数：ease_progress = t³
/// 效果：开始慢、结束快，模拟自然加速
///
/// 动画完成后调用 window.hide() 隐藏窗口
async fn animate_window_fall(window: WebviewWindow, target_x: i32, start_y: i32) {
    // 终止位置在起始位置下方 RISE_OFFSET 像素
    let end_y = start_y + RISE_OFFSET;

    for i in 1..=ANIMATION_STEPS {
        let progress = i as f64 / ANIMATION_STEPS as f64;
        let ease_progress = progress * progress * progress;
        let current_y = start_y + ((end_y - start_y) as f64 * ease_progress) as i32;

        if let Err(e) = window.set_position(Position::Physical(PhysicalPosition::new(target_x, current_y))) {
            eprintln!("Failed to set position: {}", e);
            return;
        }

        sleep(Duration::from_millis(ANIMATION_STEP_DURATION_MS)).await;
    }

    let _ = window.hide();
}

/// macOS 应用运行事件处理
///
/// 处理 Dock 图标点击事件（Reopen）：
/// - 当没有可见窗口时点击 Dock 图标，显示主面板
/// - 同样触发上升动画
#[cfg(target_os = "macos")]
fn on_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    {
        if let tauri::RunEvent::Reopen { has_visible_windows: false, .. } = event {
            let state = app.state::<AppState>();
            if !state.window_visible.load(Ordering::SeqCst) {
                state.window_visible.store(true, Ordering::SeqCst);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                state.last_show_time.store(now, Ordering::SeqCst);
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = app.emit("window-showing", ());
                    let target_x = state.target_x.load(Ordering::SeqCst);
                    let target_y = state.target_y.load(Ordering::SeqCst);
                    let window_height = state.window_height.load(Ordering::SeqCst);
                    let window_width = state.window_width.load(Ordering::SeqCst);
                    tauri::async_runtime::spawn(async move {
                        animate_window_rise(win, target_x, target_y, window_height, window_width).await;
                    });
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, event);
}

/// 应用程序主入口
///
/// 初始化流程：
/// 1. 创建全局状态（AppState）
/// 2. 注册 Tauri 插件（opener、clipboard-manager、nspanel、autostart）
/// 3. 注册 Tauri 命令（所有前端可调用的函数）
/// 4. setup 阶段：
///    a. 初始化数据库和 ClipboardService
///    b. 注册全局快捷键
///    c. 启动剪贴板监听
///    d. 启动定时清理任务
///    e. 计算窗口尺寸和位置
///    f. 配置 NSPanel（macOS）/ 系统托盘（Windows）
///    g. 注册窗口焦点丢失回调
/// 5. 构建并运行应用
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState {
        window_visible: Arc::new(AtomicBool::new(false)),
        target_x: Arc::new(AtomicI32::new(0)),
        target_y: Arc::new(AtomicI32::new(0)),
        window_height: Arc::new(AtomicU32::new(0)),
        window_width: Arc::new(AtomicU32::new(0)),
        last_show_time: Arc::new(std::sync::atomic::AtomicU64::new(0)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            paste::paste_item,
            paste::copy_to_clipboard,
            paste::get_clipboard_history,
            paste::toggle_pin,
            paste::delete_clipboard_item,
            paste::get_settings,
            paste::save_settings,
            paste::create_tag,
            paste::update_tag,
            paste::delete_tag,
            paste::get_all_tags,
            paste::update_record_tags,
            open_settings_window,
            close_settings_window,
        ])
        .setup(|app| {
            // 注册 NSPanel 插件（仅 macOS）
            #[cfg(target_os = "macos")]
            app.handle().plugin(tauri_nspanel::init())?;

            // 注册开机自启插件（使用 macOS LaunchAgent 方式）
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;

            // 初始化数据库（路径：~/Library/Application Support/com.winter.clipcat/clipcat.db）
            let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let db = database::Database::new(app_dir).map_err(|e| e.to_string())?;
            let db_for_service = db.clone();
            app.manage(std::sync::Arc::new(db));

            // 初始化剪贴板业务服务
            let clipboard_service = clipboard_service::ClipboardService::new(db_for_service);
            app.manage(std::sync::Arc::new(clipboard_service));

            // 初始化并注册全局快捷键
            let shortcut_manager = shortcuts::ShortcutManager::new(app.handle().clone());
            if let Err(e) = shortcut_manager.register_shortcuts() {
                eprintln!("Failed to register shortcuts: {}", e);
            }

            // 启动剪贴板监听（每 500ms 轮询一次）
            let clipboard_manager = clipboard::ClipboardManager::new(app.handle().clone());
            clipboard_manager.start_listening();

            // 启动定时清理任务（每 60 秒检查一次）
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut last_cleanup_date: Option<chrono::NaiveDate> = None;
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        let db = app_handle.state::<std::sync::Arc<crate::database::Database>>();
                        let cleanup_time_str = db.get_setting("cleanup_time").unwrap_or(None).unwrap_or_else(|| "00:00".to_string());
                        let now = chrono::Local::now();
                        let today = now.date_naive();

                        // 每天只执行一次清理
                        let should_run = match last_cleanup_date {
                            Some(d) if d == today => false,
                            _ => true,
                        };

                        if should_run {
                            let cleanup_time = parse_cleanup_time(&cleanup_time_str);
                            if let Some(ct) = cleanup_time {
                                let target = today.and_hms_opt(ct.0, ct.1, 0).unwrap_or_else(|| today.and_hms_opt(0, 0, 0).unwrap());
                                let now_naive = now.time();
                                let target_time = target.time();
                                // 当前时间已过清理时间点，执行清理
                                if now_naive >= target_time {
                                    let service = app_handle.state::<std::sync::Arc<crate::clipboard_service::ClipboardService>>();
                                    let save_mode = db.get_setting("save_mode").unwrap_or(None).unwrap_or_else(|| "duration".to_string());
                                    if save_mode == "duration" {
                                        // 按天数清理：删除超过保留天数的记录
                                        let days: u64 = db.get_setting("retention_duration").unwrap_or(None).unwrap_or_else(|| "30".to_string()).parse().unwrap_or(30);
                                        if let Err(e) = service.cleanup_old_records(0, days) {
                                            eprintln!("Cleanup task failed: {}", e);
                                        }
                                    } else if save_mode == "count" {
                                        // 按数量清理：删除超出保留数量的记录
                                        let count: u32 = db.get_setting("retention_count").unwrap_or(None).unwrap_or_else(|| "500".to_string()).parse().unwrap_or(500);
                                        if let Err(e) = service.cleanup_excess_count(count) {
                                            eprintln!("Cleanup task failed: {}", e);
                                        }
                                    }
                                    last_cleanup_date = Some(today);
                                }
                            }
                        }
                    }
                });
            }

            // 计算主窗口尺寸和位置
            let window = app.get_webview_window("main").ok_or("Failed to get main window")?;

            let monitor = window.current_monitor()?.ok_or("Failed to get monitor")?;
            let monitor_size = monitor.size();
            let monitor_position = monitor.position();

            // 窗口高度 = 屏幕高度 × 40%，宽度 = 屏幕宽度
            let window_height = (monitor_size.height as f64 * WINDOW_HEIGHT_RATIO) as u32;
            let window_width = monitor_size.width;

            // 水平居中，垂直方向在屏幕底部（扣除 Dock 高度）
            let target_x = monitor_position.x + (monitor_size.width as i32 - window_width as i32) / 2;
            let target_y = monitor_position.y + (monitor_size.height as i32 - window_height as i32 - DOCK_HEIGHT);

            // 设置窗口初始位置（隐藏在屏幕下方 RISE_OFFSET 像素处）
            window.set_size(Size::Physical(PhysicalSize::new(window_width, window_height)))?;
            window.set_position(Position::Physical(PhysicalPosition::new(target_x, target_y + RISE_OFFSET)))?;

            // 保存窗口位置到全局状态
            let state = app.state::<AppState>();
            state.target_x.store(target_x, Ordering::SeqCst);
            state.target_y.store(target_y, Ordering::SeqCst);
            state.window_height.store(window_height, Ordering::SeqCst);
            state.window_width.store(window_width, Ordering::SeqCst);

            // macOS 专属：配置 NSPanel 属性
            #[cfg(target_os = "macos")]
            {
                let panel = window.to_panel::<ClipcatPanel>().map_err(|_| "Failed to convert to panel")?;

                // 设置面板层级为 Dock 级别（浮于 Dock 上方）
                panel.set_level(PanelLevel::Dock.value());

                // 设置样式：可调整大小 + 非激活面板（不抢夺焦点）
                panel.set_style_mask(
                    StyleMask::empty()
                        .resizable()
                        .nonactivating_panel()
                        .into(),
                );

                // 设置集合行为：固定位置 + 跟随活动空间 + 全屏辅助
                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .stationary()
                        .move_to_active_space()
                        .full_screen_auxiliary()
                        .into(),
                );
            }

            // Windows 专属：系统托盘 + 右键菜单
            #[cfg(target_os = "windows")]
            {
                window.set_always_on_top(true)?;

                let show_item = MenuItem::with_id(app, "show", "Show Clipcat", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
                let app_handle = app.handle().clone();

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .menu_on_left_click(false)
                    .on_menu_event(move |app, event| {
                        match event.id.as_ref() {
                            "show" => {
                                if let Some(win) = app.get_webview_window("main") {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                            "quit" => {
                                app.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(move |tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(win) = app.get_webview_window("main") {
                                if win.is_visible().unwrap_or(false) {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                    })
                    .build(app_handle)?;
            }

            // 注册窗口焦点丢失回调
            // 当面板失去焦点时，自动触发下降动画隐藏面板
            // 增加 100ms 防抖：避免焦点事件与显示事件冲突
            let last_show_time = app.state::<AppState>().last_show_time.clone();
            let target_x_clone = target_x;
            let target_y_clone = target_y;
            let window_clone = window.clone();
            let window_visible = app.state::<AppState>().window_visible.clone();

            window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let show_time = last_show_time.load(Ordering::SeqCst);
                    let time_since_show = now.saturating_sub(show_time);
                    // 仅在面板可见且距离上次显示超过 100ms 时才隐藏（防抖）
                    if window_visible.load(Ordering::SeqCst) && time_since_show > 100 {
                        window_visible.store(false, Ordering::SeqCst);
                        let _ = window_clone.app_handle().emit("window-hiding", ());
                        let win = window_clone.clone();
                        let tx = target_x_clone;
                        let ty = target_y_clone;
                        tauri::async_runtime::spawn(async move {
                            animate_window_fall(win, tx, ty).await;
                        });
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(on_run_event);
}

/// 打开设置窗口
///
/// - 先隐藏主面板
/// - 创建独立设置窗口（420×500，居中，不可调整大小）
/// - 窗口标题根据当前语言环境显示"首选项"或"Preferences"
/// - 加载 /settings 路由（对应 settings.html 入口）
#[tauri::command]
async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.hide();
    }

    let title = {
        let db = app.state::<std::sync::Arc<crate::database::Database>>();
        let locale = db.get_setting("locale").map_err(|e| e.to_string())?.unwrap_or_else(|| "en".to_string());
        if locale == "zh" { "首选项" } else { "Preferences" }
    };

    let settings_window = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("/settings".into()),
    )
    .title(title)
    .inner_size(420.0, 500.0)
    .resizable(false)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    let _ = settings_window.set_focus();

    Ok(())
}

/// 关闭设置窗口
#[tauri::command]
async fn close_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(settings_win) = app.get_webview_window("settings") {
        let _ = settings_win.close();
    }
    Ok(())
}

/// 解析清理时间字符串（格式 "HH:MM"）
///
/// 返回 (小时, 分钟) 元组，校验小时 < 24 且分钟 < 60
fn parse_cleanup_time(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    if hour < 24 && minute < 60 {
        Some((hour, minute))
    } else {
        None
    }
}
