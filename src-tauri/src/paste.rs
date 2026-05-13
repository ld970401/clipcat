use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::clipboard::{ClipboardContent, ClipboardItem};

fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create input controller: {:?}", e))?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| format!("Failed to press modifier key: {:?}", e))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Failed to type v: {:?}", e))?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| format!("Failed to release modifier key: {:?}", e))?;

    Ok(())
}

fn write_to_clipboard(app: &AppHandle, item: &ClipboardItem) -> Result<(), String> {
    match &item.content {
        ClipboardContent::Text(text) => {
            println!("Writing text to clipboard: {} chars", text.len());
            app.clipboard()
                .write_text(text)
                .map_err(|e| format!("Failed to write text: {}", e))
        }
        ClipboardContent::Image { width, height, rgba } => {
            println!("Writing image to clipboard: {}x{}", width, height);
            let image = tauri::image::Image::new(rgba, *width, *height);
            app.clipboard()
                .write_image(&image)
                .map_err(|e| format!("Failed to write image: {}", e))
        }
    }
}

fn hide_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let state = app.state::<crate::AppState>();
        if state.window_visible.load(std::sync::atomic::Ordering::SeqCst) {
            state.window_visible.store(false, std::sync::atomic::Ordering::SeqCst);
            let target_x = state.target_x.load(std::sync::atomic::Ordering::SeqCst);
            let target_y = state.target_y.load(std::sync::atomic::Ordering::SeqCst);
            let win = win.clone();
            tauri::async_runtime::spawn(async move {
                crate::animate_window_fall(win, target_x, target_y).await;
            });
        }
    }
}

#[tauri::command]
pub async fn paste_item(app: AppHandle, item: ClipboardItem) -> Result<(), String> {
    println!("paste_item called: content_type={}", item.content_type);

    write_to_clipboard(&app, &item)?;

    hide_panel(&app);

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    simulate_paste()?;

    println!("paste_item completed");
    Ok(())
}
