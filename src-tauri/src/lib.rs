#![allow(deprecated)]

mod clipboard;
mod paste;
mod shortcuts;

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow, WindowEvent};
use tauri_nspanel::builder::{CollectionBehavior, PanelLevel, StyleMask};
use tauri_nspanel::{tauri_panel, WebviewWindowExt};
use tokio::time::sleep;

tauri_panel! {
    panel!(ClipcatPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: false
        }
    })
}

const ANIMATION_STEPS: u32 = 20;
const ANIMATION_STEP_DURATION_MS: u64 = 10;
const WINDOW_HEIGHT_RATIO: f64 = 0.40;
const RISE_OFFSET: i32 = 800;
const DOCK_HEIGHT: i32 = 0;

struct AppState {
    window_visible: Arc<AtomicBool>,
    target_x: Arc<AtomicI32>,
    target_y: Arc<AtomicI32>,
    window_height: Arc<AtomicU32>,
    window_width: Arc<AtomicU32>,
    last_show_time: Arc<std::sync::atomic::AtomicU64>,
}

async fn animate_window_rise(window: WebviewWindow, target_x: i32, target_y: i32, window_height: u32, window_width: u32) {
    let start_y = target_y + RISE_OFFSET;

    if let Err(e) = window.set_size(Size::Physical(PhysicalSize::new(window_width, window_height))) {
        eprintln!("Failed to set window size: {}", e);
        return;
    }

    if let Err(e) = window.set_position(Position::Physical(PhysicalPosition::new(target_x, start_y))) {
        eprintln!("Failed to set initial position: {}", e);
        return;
    }

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

async fn animate_window_fall(window: WebviewWindow, target_x: i32, start_y: i32) {
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
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![paste::paste_item])
        .setup(|app| {
            let shortcut_manager = shortcuts::ShortcutManager::new(app.handle().clone());
            if let Err(e) = shortcut_manager.register_shortcuts() {
                eprintln!("Failed to register shortcuts: {}", e);
            }

            let clipboard_manager = clipboard::ClipboardManager::new(app.handle().clone());
            clipboard_manager.start_listening();

            let window = app.get_webview_window("main").ok_or("Failed to get main window")?;

            let monitor = window.current_monitor()?.ok_or("Failed to get monitor")?;
            let monitor_size = monitor.size();
            let monitor_position = monitor.position();

            let window_height = (monitor_size.height as f64 * WINDOW_HEIGHT_RATIO) as u32;
            let window_width = monitor_size.width;

            let target_x = monitor_position.x + (monitor_size.width as i32 - window_width as i32) / 2;
            let target_y = monitor_position.y + (monitor_size.height as i32 - window_height as i32 - DOCK_HEIGHT);

            window.set_size(Size::Physical(PhysicalSize::new(window_width, window_height)))?;
            window.set_position(Position::Physical(PhysicalPosition::new(target_x, target_y + RISE_OFFSET)))?;

            let state = app.state::<AppState>();
            state.target_x.store(target_x, Ordering::SeqCst);
            state.target_y.store(target_y, Ordering::SeqCst);
            state.window_height.store(window_height, Ordering::SeqCst);
            state.window_width.store(window_width, Ordering::SeqCst);

            #[cfg(target_os = "macos")]
            {
                let panel = window.to_panel::<ClipcatPanel>().map_err(|_| "Failed to convert to panel")?;

                panel.set_level(PanelLevel::Dock.value());

                panel.set_style_mask(
                    StyleMask::empty()
                        .resizable()
                        .nonactivating_panel()
                        .into(),
                );

                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .stationary()
                        .move_to_active_space()
                        .full_screen_auxiliary()
                        .into(),
                );

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
                        if window_visible.load(Ordering::SeqCst) && time_since_show > 100 {
                            window_visible.store(false, Ordering::SeqCst);
                            let win = window_clone.clone();
                            let tx = target_x_clone;
                            let ty = target_y_clone;
                            tauri::async_runtime::spawn(async move {
                                animate_window_fall(win, tx, ty).await;
                            });
                        }
                    }
                });
            }

            #[cfg(not(target_os = "macos"))]
            {
                let window_clone = window.clone();
                let target_x_clone = target_x;
                let target_y_clone = target_y;
                let window_visible = app.state::<AppState>().window_visible.clone();
                let last_show_time = Arc::clone(&app.state::<AppState>().last_show_time);

                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let show_time = last_show_time.load(Ordering::SeqCst);
                        let time_since_show = now.saturating_sub(show_time);
                        if window_visible.load(Ordering::SeqCst) && time_since_show > 100 {
                            window_visible.store(false, Ordering::SeqCst);
                            let win = window_clone.clone();
                            let tx = target_x_clone;
                            let ty = target_y_clone;
                            tauri::async_runtime::spawn(async move {
                                animate_window_fall(win, tx, ty).await;
                            });
                        }
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(on_run_event);
}