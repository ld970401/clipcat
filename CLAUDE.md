# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ClipboardCat (clipcat) is a macOS-focused clipboard manager built with Tauri 2 + Vue 3. It appears as a floating panel from the bottom of the screen (like Paste app), monitors clipboard changes, and displays history as cards. It also has a separate settings window.

## Build & Dev Commands

```bash
pnpm tauri dev      # Run in dev mode (starts Vite + Rust backend)
pnpm tauri build    # Build production app
pnpm dev            # Vite dev server only (frontend, no Rust backend)
pnpm build          # Vite build only (frontend)
```

No test framework is configured. No linting is configured.

## Architecture

### Two-Process Architecture (Tauri standard)
- **Frontend** (Vue 3 + Vite): `src/` — SPA for main panel and settings window
- **Backend** (Rust): `src-tauri/src/` — native window management, clipboard, database

### Multi-Window Frontend
Vite builds two entry points via `rollupOptions.input`:
- `index.html` → main clipboard panel (`src/main.js` → `App.vue`)
- `settings.html` → settings window (`src/settings.js` → `SettingsApp.vue`)

Settings window is created dynamically via `open_settings_window` Tauri command, loading `/settings` route.

### Rust Backend Module Structure

| Module | Responsibility |
|--------|---------------|
| `lib.rs` | App setup, window management, NSPanel configuration, panel rise/fall animations, focus handling, cleanup background task |
| `clipboard.rs` | `ClipboardManager` — polls clipboard every 500ms, deduplicates via xxhash, emits `clipboard-change` events to frontend, saves to DB via `ClipboardService` |
| `clipboard_db.rs` | `ClipboardRepository` — SQLite CRUD for clipboard records, image thumbnail batch loading, `duplicate_and_move_to_top` (with transaction) |
| `clipboard_service.rs` | `ClipboardService` — business logic layer between clipboard listener and DB, deduplication by comparing hash of latest DB record, auto-assigns default tag (id=1) |
| `tag_db.rs` | `TagRepository` — SQLite CRUD for tags, 6-color rotation for new tags, `ensure_default_tag_exists` creates "Clipboard" tag at id=1 |
| `database.rs` | `Database` — r2d2 connection pool (max 10), schema init, key-value settings table |
| `paste.rs` | Tauri commands: `paste_item`, `copy_to_clipboard`, settings CRUD, tag CRUD — also houses all `#[tauri::command]` functions registered in `lib.rs` |
| `shortcuts.rs` | `ShortcutManager` — global shortcut registration via `tauri-plugin-global-shortcut`, reads shortcuts from DB settings, supports `reregister_shortcuts` |
| `image_processor.rs` | `ImageProcessor` — thumbnail generation (max 200px, Lanczos3, PNG output) |

### Key Data Flow

1. `ClipboardManager` polls clipboard → deduplicates via xxhash3_64 → calls `ClipboardService::on_clipboard_change`
2. Service compares hash with latest DB record → if different, creates record + assigns default tag
3. Service returns `ClipboardRecord` → `ClipboardManager` emits `clipboard-change` event to frontend
4. Image events strip `original` data before emitting (only send thumbnail to frontend)
5. Frontend receives event → updates reactive `allItems` array

### Database Schema (SQLite at `~/Library/Application Support/com.winter.clipcat/clipcat.db`)
- `clipboard` — records with content_type, content_text, width/height, is_pinned, timestamps
- `clipboard_images` — thumbnails (PNG blob) and originals (RGBA blob) linked by FK
- `clipboard_tags` — many-to-many join table
- `tag` — tags with name, hex color, timestamps
- `settings` — key-value store for app preferences

### macOS-Specific Details
- Window uses `tauri-nspanel` (v2.1 branch) for NSPanel with `PanelLevel::Dock` — floats above Dock
- Window: transparent, no decorations, always-on-top, 40% screen height, positioned at screen bottom
- Rise/fall animations: cubic easing, 20 steps × 10ms
- Paste simulation: AppleScript `keystroke "v" using command down` (requires Accessibility permission)
- `cocoa`/`objc` crates are declared but unused (previously caused panics with native styling)

### Tauri Events (frontend ↔ backend)
- `clipboard-change` — backend emits when new clipboard content detected
- `window-showing` — backend emits when panel rises (frontend currently does not listen)
- `window-hiding` — backend emits when panel falls
- `settings-changed` — backend emits after settings saved

### Frontend i18n
Uses `vue-i18n` with locale files in `src/locales/` (`en.json`, `zh.json`). Locale is stored in DB settings.

## Known Issues (from OPTIMIZATION.md)

Critical: temporary ID mismatch (frontend generates `Date.now() + Math.random()` but backend uses DB IDs), `PRAGMA foreign_keys` is enabled in connection pool init, image URL memory leaks in `allItems` management, `duplicate_and_move_to_top` delete is separate from transaction.
