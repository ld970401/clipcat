use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const DEFAULT_SHORTCUT_SHOW: &str = "CmdOrCtrl+Shift+V";
const DEFAULT_SHORTCUT_HIDE: &str = "Escape";

pub struct ShortcutState_ {
    pub show_shortcut: Shortcut,
    pub hide_shortcut: Shortcut,
}

pub struct ShortcutManager {
    app_handle: AppHandle,
}

impl ShortcutManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn register_shortcuts(&self) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.app_handle.state::<Arc<crate::database::Database>>();
        let show_str = db
            .get_setting("shortcut_show")
            .unwrap_or(None)
            .unwrap_or_else(|| DEFAULT_SHORTCUT_SHOW.to_string());
        let hide_str = db
            .get_setting("shortcut_hide")
            .unwrap_or(None)
            .unwrap_or_else(|| DEFAULT_SHORTCUT_HIDE.to_string());

        let show_shortcut = parse_shortcut(&show_str)
            .ok_or_else(|| format!("Invalid shortcut_show: {}", show_str))?;
        let hide_shortcut = parse_shortcut(&hide_str)
            .ok_or_else(|| format!("Invalid shortcut_hide: {}", hide_str))?;

        self.app_handle.manage(Arc::new(std::sync::RwLock::new(ShortcutState_ {
            show_shortcut,
            hide_shortcut,
        })));

        self.app_handle.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let state = app.state::<Arc<std::sync::RwLock<ShortcutState_>>>();
                        let guard = state.read().unwrap();
                        if shortcut == &guard.show_shortcut {
                            if let Some(win) = app.get_webview_window("main") {
                                show_window(&win);
                            }
                        } else if shortcut == &guard.hide_shortcut {
                            if let Some(win) = app.get_webview_window("main") {
                                hide_window(&win);
                            }
                        }
                    }
                })
                .build(),
        )?;

        let gs = self.app_handle.global_shortcut();
        gs.register(show_shortcut)?;
        gs.register(hide_shortcut)?;

        Ok(())
    }

    pub fn reregister_shortcuts(
        &self,
        show_str: &str,
        hide_str: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let new_show = parse_shortcut(show_str)
            .ok_or_else(|| format!("Invalid shortcut_show: {}", show_str))?;
        let new_hide = parse_shortcut(hide_str)
            .ok_or_else(|| format!("Invalid shortcut_hide: {}", hide_str))?;

        let state = self.app_handle.state::<Arc<std::sync::RwLock<ShortcutState_>>>();
        let guard = state.read().unwrap();

        let gs = self.app_handle.global_shortcut();
        let _ = gs.unregister(guard.show_shortcut);
        let _ = gs.unregister(guard.hide_shortcut);

        drop(guard);

        gs.register(new_show)?;
        gs.register(new_hide)?;

        let mut guard = state.write().unwrap();
        guard.show_shortcut = new_show;
        guard.hide_shortcut = new_hide;

        Ok(())
    }
}

fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("escape") || s.eq_ignore_ascii_case("esc") {
        return Some(Shortcut::new(None, Code::Escape));
    }

    let mut modifiers = Modifiers::empty();
    let mut code = None;

    for part in s.split('+') {
        let part = part.trim();
        if part.eq_ignore_ascii_case("cmd") || part.eq_ignore_ascii_case("cmdorctrl") || part.eq_ignore_ascii_case("super") || part.eq_ignore_ascii_case("command") {
            modifiers |= Modifiers::SUPER;
        } else if part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control") {
            modifiers |= Modifiers::CONTROL;
        } else if part.eq_ignore_ascii_case("alt") || part.eq_ignore_ascii_case("option") {
            modifiers |= Modifiers::ALT;
        } else if part.eq_ignore_ascii_case("shift") {
            modifiers |= Modifiers::SHIFT;
        } else {
            code = parse_code(part);
        }
    }

    code.map(|c| Shortcut::new(if modifiers.is_empty() { None } else { Some(modifiers) }, c))
}

fn parse_code(s: &str) -> Option<Code> {
    match s.to_lowercase().as_str() {
        "a" => Some(Code::KeyA),
        "b" => Some(Code::KeyB),
        "c" => Some(Code::KeyC),
        "d" => Some(Code::KeyD),
        "e" => Some(Code::KeyE),
        "f" => Some(Code::KeyF),
        "g" => Some(Code::KeyG),
        "h" => Some(Code::KeyH),
        "i" => Some(Code::KeyI),
        "j" => Some(Code::KeyJ),
        "k" => Some(Code::KeyK),
        "l" => Some(Code::KeyL),
        "m" => Some(Code::KeyM),
        "n" => Some(Code::KeyN),
        "o" => Some(Code::KeyO),
        "p" => Some(Code::KeyP),
        "q" => Some(Code::KeyQ),
        "r" => Some(Code::KeyR),
        "s" => Some(Code::KeyS),
        "t" => Some(Code::KeyT),
        "u" => Some(Code::KeyU),
        "v" => Some(Code::KeyV),
        "w" => Some(Code::KeyW),
        "x" => Some(Code::KeyX),
        "y" => Some(Code::KeyY),
        "z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0),
        "1" => Some(Code::Digit1),
        "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3),
        "4" => Some(Code::Digit4),
        "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6),
        "7" => Some(Code::Digit7),
        "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        "f1" => Some(Code::F1),
        "f2" => Some(Code::F2),
        "f3" => Some(Code::F3),
        "f4" => Some(Code::F4),
        "f5" => Some(Code::F5),
        "f6" => Some(Code::F6),
        "f7" => Some(Code::F7),
        "f8" => Some(Code::F8),
        "f9" => Some(Code::F9),
        "f10" => Some(Code::F10),
        "f11" => Some(Code::F11),
        "f12" => Some(Code::F12),
        "space" => Some(Code::Space),
        "tab" => Some(Code::Tab),
        "enter" | "return" => Some(Code::Enter),
        "backspace" => Some(Code::Backspace),
        "delete" => Some(Code::Delete),
        "home" => Some(Code::Home),
        "end" => Some(Code::End),
        "pageup" => Some(Code::PageUp),
        "pagedown" => Some(Code::PageDown),
        "up" => Some(Code::ArrowUp),
        "down" => Some(Code::ArrowDown),
        "left" => Some(Code::ArrowLeft),
        "right" => Some(Code::ArrowRight),
        "[" | "bracketleft" => Some(Code::BracketLeft),
        "]" | "bracketright" => Some(Code::BracketRight),
        ";" | "semicolon" => Some(Code::Semicolon),
        "'" | "quote" => Some(Code::Quote),
        "," | "comma" => Some(Code::Comma),
        "." | "period" => Some(Code::Period),
        "/" | "slash" => Some(Code::Slash),
        "\\" | "backslash" => Some(Code::Backslash),
        "-" | "minus" => Some(Code::Minus),
        "=" | "equal" => Some(Code::Equal),
        "`" | "backquote" => Some(Code::Backquote),
        _ => None,
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
        let _ = app.emit("window-showing", ());
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
