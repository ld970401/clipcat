use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub struct ShortcutManager {
    _app_handle: AppHandle,
}

impl ShortcutManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { _app_handle: app_handle }
    }

    #[cfg(target_os = "macos")]
    pub fn register_shortcuts(&self) -> Result<(), Box<dyn std::error::Error>> {
        let ctrl_1 = Shortcut::new(Some(Modifiers::CONTROL), Code::Digit1);
        let escape = Shortcut::new(None, Code::Escape);

        self._app_handle.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if shortcut == &ctrl_1 {
                            if let Some(win) = app.get_webview_window("main") {
                                show_window(&win);
                            }
                        } else if shortcut == &escape {
                            if let Some(win) = app.get_webview_window("main") {
                                hide_window(&win);
                            }
                        }
                    }
                })
                .build(),
        )?;

        self._app_handle.global_shortcut().register(ctrl_1)?;
        self._app_handle.global_shortcut().register(escape)?;

        println!("Global shortcuts registered: Ctrl+1 and Escape");
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn register_shortcuts(&self) -> Result<(), Box<dyn std::error::Error>> {
        let ctrl_f1 = Shortcut::new(Some(Modifiers::CONTROL), Code::F1);
        let escape = Shortcut::new(None, Code::Escape);

        self._app_handle.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if shortcut == &ctrl_f1 {
                            if let Some(win) = app.get_webview_window("main") {
                                show_window(&win);
                            }
                        } else if shortcut == &escape {
                            if let Some(win) = app.get_webview_window("main") {
                                hide_window(&win);
                            }
                        }
                    }
                })
                .build(),
        )?;

        self._app_handle.global_shortcut().register(ctrl_f1)?;
        self._app_handle.global_shortcut().register(escape)?;

        Ok(())
    }
}

fn show_window(window: &WebviewWindow) {
    let app = window.app_handle();
    let state = app.state::<super::AppState>();
    if !state.window_visible.load(std::sync::atomic::Ordering::SeqCst) {
        state.window_visible.store(true, std::sync::atomic::Ordering::SeqCst);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        state.last_show_time.store(now, std::sync::atomic::Ordering::SeqCst);

        let _ = window.show();
        let target_x = state.target_x.load(std::sync::atomic::Ordering::SeqCst);
        let target_y = state.target_y.load(std::sync::atomic::Ordering::SeqCst);
        let window_height = state.window_height.load(std::sync::atomic::Ordering::SeqCst);
        let window_width = state.window_width.load(std::sync::atomic::Ordering::SeqCst);

        let win = window.clone();
        tauri::async_runtime::spawn(async move {
            super::animate_window_rise(win, target_x, target_y, window_height, window_width).await;
        });
    }
}

fn hide_window(window: &WebviewWindow) {
    let app = window.app_handle();
    let state = app.state::<super::AppState>();

    if state.window_visible.load(std::sync::atomic::Ordering::SeqCst) {
        state.window_visible.store(false, std::sync::atomic::Ordering::SeqCst);
        let target_x = state.target_x.load(std::sync::atomic::Ordering::SeqCst);
        let target_y = state.target_y.load(std::sync::atomic::Ordering::SeqCst);

        let win = window.clone();
        tauri::async_runtime::spawn(async move {
            super::animate_window_fall(win, target_x, target_y).await;
        });
    }
}
