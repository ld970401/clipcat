# Clipcat 项目优化分析报告

> 审查日期：2026-05-17

---

## 🔴 Bug / 需要修复

### 1. `clipboard-change` 事件中 `item.id` 使用临时 ID

- **位置**: `src/App.vue:85`
- **问题**: `id: Date.now() + Math.random()`，这个临时 ID 和数据库中的真实 ID 不同。当用户双击这个条目粘贴时，`invoke("paste_item", { id: item.id })` 传的是临时 ID，后端 `paste_item` 期望 `i64` 类型的数据库 ID，会查不到记录导致失败。
- **修复方案**: 后端 `clipboard-change` 事件应携带数据库真实 ID。修改 `clipboard.rs` 中 emit 的 `ClipboardItem` 结构体，增加 `id` 字段；前端直接使用后端返回的 `id`。

### 2. `pasteItem` 中 `move_to_top=false` 时前端不更新数据

- **位置**: `src/App.vue:275`
- **问题**: `if (result)` 判断，当 `move_to_top=false`（已是首条）时后端返回 `Ok(None)`，前端不更新 `allItems`。但此时剪贴板监控可能触发 `clipboard-change`，用临时 ID 添加了一个重复条目。
- **修复方案**: `pasteItem` 无论 `result` 是否为 `null`，都应正常完成。后端 `move_to_top=false` 时也应返回记录信息（或前端直接不等待结果更新本地数据）。

### 3. `database.rs` 中残留调试日志

- **位置**: `src-tauri/src/database.rs:55`
- **问题**: 仍有 `eprintln!("DEBUG: Database initialized...")`。
- **修复方案**: 删除该调试日志。

### 4. `paste.rs` 中残留 `println`

- **位置**: `src-tauri/src/paste.rs:73` 和 `src-tauri/src/paste.rs:86`
- **问题**: 仍有 `println!("Writing text/image to clipboard...")`。
- **修复方案**: 删除或改为条件编译的日志。

### 5. `clipboard-change` 事件缺少 `id` 和 `tags` 字段

- **位置**: `src-tauri/src/clipboard.rs`
- **问题**: emit 的 `ClipboardItem` 只有 `content_type`、`content`、`timestamp`，没有 `id` 和 `tags`。前端收到后自行生成临时 ID，且 `tags` 为空数组，导致新条目在标签过滤时可能不显示。
- **修复方案**: 修改 `ClipboardItem` 结构体增加 `id` 和 `tags` 字段，后端在 `on_clipboard_change` 返回 `Ok(Some(record))` 时将 `record.id` 和 `record.tags` 一并发出。

---

## 🟡 逻辑问题 / 潜在风险

### 6. 剪贴板监控线程每 500ms 同时读文本和图片

- **位置**: `src-tauri/src/clipboard.rs:48`
- **问题**: 每次轮询都读文本和图片。当剪贴板是文本时，`read_image` 也会被调用（可能返回错误或空），造成不必要的开销。
- **修复方案**: 根据剪贴板当前类型选择性读取，或先尝试读文本，成功则跳过图片读取。

### 7. 自定义 hash 算法碰撞率高

- **位置**: `src-tauri/src/clipboard.rs:135-147`
- **问题**: 使用简单的加权求和 hash，碰撞概率较高。对于图片的 RGBA 数据（可能几 MB），逐字节计算也很慢。
- **修复方案**: 使用 `ahash` 或 `xxhash` 等高质量 hash 库，性能和碰撞率都更优。

### 8. `duplicate_and_move_to_top` 不是原子操作

- **位置**: `src-tauri/src/clipboard_db.rs:457`
- **问题**: 先 insert 新记录再 delete 旧记录，如果中间出错会留下重复数据。
- **修复方案**: 用事务包裹整个操作。

### 9. `delete` 方法没有级联删除图片数据

- **位置**: `src-tauri/src/clipboard_db.rs:417`
- **问题**: 只 `DELETE FROM clipboard WHERE id = ?1`，虽然 schema 中有 `ON DELETE CASCADE`，但 SQLite 的外键约束默认是关闭的，需要 `PRAGMA foreign_keys = ON` 才生效。否则 `clipboard_images` 和 `clipboard_tags` 中的关联数据会残留。
- **修复方案**: 在数据库连接初始化时执行 `PRAGMA foreign_keys = ON`，或在 `delete` 方法中手动删除关联表数据。

### 10. `maxItems = 50` 硬编码

- **位置**: `src/App.vue:17`
- **问题**: 前端限制 50 条，但后端数据库可能存了更多。用户切换标签或搜索时，只能看到最近 50 条中的匹配项，可能遗漏。
- **修复方案**: 将 `maxItems` 改为可配置值，或根据设置中的 `retention_count` 动态调整。

### 11. 图片 URL 内存泄漏

- **位置**: `src/App.vue`
- **问题**: `imageUrls` 使用 `URL.createObjectURL` 创建，但在很多场景下没有调用 `URL.revokeObjectURL` 释放：
  - `deleteItem` 时没有清理
  - `pasteItem` 移除旧条目时没有清理
  - `loadHistory` 重新加载时没有清理旧的 URL
- **修复方案**: 在所有移除条目的场景中，先 `URL.revokeObjectURL` 释放对应 URL。

---

## 🟢 可优化 / 待完成

### 12. `window-showing` 事件没有刷新数据

- **位置**: `src-tauri/src/shortcuts.rs:65`
- **问题**: 发送了 `window-showing` 事件，但前端没有监听这个事件来刷新数据。如果面板隐藏期间有新的剪贴板变化（通过 `clipboard-change` 事件已经添加到 `allItems`），面板打开时数据是最新的。但如果后端在面板隐藏期间做了清理操作，前端数据就会过时。
- **建议**: 前端监听 `window-showing` 事件，可选地执行一次轻量级数据同步（如只同步 ID 列表，删除已不存在的条目）。

### 13. 设置中的快捷键不可修改

- **位置**: `src/SettingsApp.vue`
- **问题**: 快捷键是 `readonly` 的，用户无法自定义。后端 `shortcuts.rs` 中快捷键也是硬编码的 `Ctrl+1` / `Escape`，没有读取设置。
- **建议**: 实现快捷键录制功能，后端动态注册/注销快捷键。

### 14. 设置中的"数据保留"功能未实现

- **位置**: `save_mode`、`retention_duration`、`retention_count`、`cleanup_time`
- **问题**: 这些设置存在数据库中，但没有任何定时清理逻辑。`cleanup_old_records` 方法存在但从未被调用。
- **建议**: 实现定时清理任务，根据 `cleanup_time` 每天执行一次清理。

### 15. 设置中的"开机启动"功能未实现

- **位置**: `launch_at_login`
- **问题**: 设置存了但后端没有实现开机启动逻辑。
- **建议**: 使用 `tauri-plugin-autostart` 或 macOS 的 `SMAppService` API。

### 16. `SettingsModal.vue` 已废弃

- **位置**: `src/components/SettingsModal.vue`
- **问题**: 这个组件已不再使用（设置改用独立窗口 `SettingsApp.vue`），可以删除。

### 17. `get_clipboard_image` 命令未使用

- **位置**: `src-tauri/src/paste.rs:213`
- **问题**: 命令已注册但前端从未调用。

### 18. `get_pinned_items` / `search_clipboard` 命令未使用

- **问题**: 这两个命令已注册但前端从未调用。

### 19. `enigo` 依赖在 macOS 上未使用

- **位置**: `src-tauri/src/paste.rs:1`、`Cargo.toml`
- **问题**: macOS 使用 AppleScript 模拟粘贴，`enigo` 只在非 macOS 平台使用，但 Cargo.toml 中无条件引入。
- **建议**: 使用 `[target.'cfg(not(target_os = "macos"))'.dependencies]` 条件引入。

### 20. 前端 `copyItem` 使用 `navigator.clipboard.writeText`

- **位置**: `src/App.vue:301`
- **问题**: 使用浏览器 API 复制文本，这会触发剪贴板监控添加重复条目。
- **建议**: 调用后端命令来写入剪贴板（和 `paste_item` 类似，但不模拟粘贴），或在前端标记此次写入为"自身操作"以跳过 `clipboard-change` 处理。

### 21. 右键菜单"标签"子菜单文案有误

- **位置**: `src/components/ClipboardCard.vue`
- **问题**: 右键菜单的"标签"功能用的是 `contextMenu.pin` 的翻译 key，但实际功能是给条目分配标签，不是"置顶"。
- **建议**: 增加 `contextMenu.tags` 翻译 key，替换 `contextMenu.pin`。

### 22. `clipboard_db.rs` 中 `list` 方法代码重复

- **位置**: `src-tauri/src/clipboard_db.rs`
- **问题**: `tag_id` 为 `Some` 和 `None` 两个分支有大量重复代码。
- **建议**: 合并为一个方法，根据 `tag_id` 动态构建 SQL。

### 23. 前端没有键盘快捷键支持

- **问题**: 面板内没有键盘导航（上下选择、Enter 粘贴、Delete 删除等），只能鼠标操作。
- **建议**: 添加键盘事件监听，支持方向键选择、Enter 粘贴、Delete 删除等。

---

## 📋 优先级总览

| 优先级 | 编号 | 问题 | 影响 |
|--------|------|------|------|
| P0 | #1 | 临时 ID 导致粘贴失败 | 核心功能不可用 |
| P0 | #5 | clipboard-change 缺少 id/tags | 数据不一致 |
| P0 | #9 | 外键级联未生效 | 数据残留 |
| P1 | #2 | move_to_top=false 不更新前端 | 数据不同步 |
| P1 | #11 | 图片 URL 内存泄漏 | 内存泄漏 |
| P1 | #20 | copyItem 触发重复添加 | 用户体验 |
| P2 | #7 | hash 碰撞 | 偶发去重失败 |
| P2 | #8 | 非原子操作 | 极端情况数据不一致 |
| P2 | #14 | 数据保留功能未实现 | 功能缺失 |
| P2 | #15 | 开机启动未实现 | 功能缺失 |
| P3 | #3 | 残留调试日志 | 代码整洁 |
| P3 | #4 | 残留 println | 代码整洁 |
| P3 | #6 | 剪贴板轮询效率 | 性能 |
| P3 | #10 | maxItems 硬编码 | 灵活性 |
| P3 | #12 | window-showing 未监听 | 数据时效性 |
| P3 | #13 | 快捷键不可修改 | 功能缺失 |
| P3 | #16 | SettingsModal 废弃 | 代码整洁 |
| P3 | #17 | 未使用命令 | 代码整洁 |
| P3 | #18 | 未使用命令 | 代码整洁 |
| P3 | #19 | enigo 无条件引入 | 包体积 |
| P3 | #21 | 右键菜单文案错误 | 国际化 |
| P3 | #22 | list 方法代码重复 | 代码质量 |
| P3 | #23 | 无键盘快捷键 | 可访问性 |
