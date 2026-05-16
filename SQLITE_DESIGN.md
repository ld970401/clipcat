# ClipboardCat 数据持久化方案

## 一、概述

本方案使用 SQLite 实现剪切板历史记录的持久化存储，支持标签分类和置顶功能。

## 二、技术选型

| 组件 | 选择 | 原因 |
|------|------|------|
| 数据库 | SQLite | 轻量级、零配置、跨平台 |
| Rust ORM | `rusqlite` + `r2d2` | 成熟稳定、社区常用 |
| 连接池 | `r2d2` | 高效连接管理 |
| 异步支持 | 通过 `tauri::async_runtime` | 与 Tauri 集成 |

## 三、数据库设计

### 3.1 ER 图

```
┌─────────────────┐       ┌─────────────────────┐       ┌─────────────────┐
│    clipboard    │       │   clipboard_tags    │       │      tag        │
├─────────────────┤       ├─────────────────────┤       ├─────────────────┤
│ id (PK)         │──┐    │ clipboard_id (FK)  │       │ id (PK)         │
│ content_type    │  │    │ tag_id (FK)        │───┐   │ name            │
│ content_text    │  └───▶│                     │   │   │ color           │
│ content_image   │       └─────────────────────┘   └───▶│ created_at      │
│ width           │                                       │ updated_at      │
│ height          │                                       └─────────────────┘
│ is_pinned      │
│ created_at     │
│ updated_at     │
└─────────────────┘
```

### 3.2 表结构

```sql
-- 剪切板历史记录表
CREATE TABLE clipboard (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type    TEXT NOT NULL CHECK (content_type IN ('text', 'image')),
    content_text    TEXT,
    content_image   BLOB,           -- RGBA 原始数据压缩存储
    width           INTEGER,
    height          INTEGER,
    is_pinned       INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,  -- Unix timestamp (ms)
    updated_at      INTEGER NOT NULL
);

-- 标签表
CREATE TABLE tag (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    color       TEXT NOT NULL DEFAULT '#6B7280',  -- 十六进制颜色
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- 剪切板-标签关联表 (多对多)
CREATE TABLE clipboard_tags (
    clipboard_id INTEGER NOT NULL,
    tag_id      INTEGER NOT NULL,
    PRIMARY KEY (clipboard_id, tag_id),
    FOREIGN KEY (clipboard_id) REFERENCES clipboard(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tag(id) ON DELETE CASCADE
);

-- 索引优化
CREATE INDEX idx_clipboard_created_at ON clipboard(created_at DESC);
CREATE INDEX idx_clipboard_is_pinned ON clipboard(is_pinned);
CREATE INDEX idx_clipboard_content_type ON clipboard(content_type);
```

## 四、数据结构

### 4.1 Rust 结构体

```rust
// clipboard.rs - 现有结构扩展

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardRecord {
    pub id: i64,
    pub content_type: String,          // "text" 或 "image"
    pub content: ClipboardContent,
    pub is_pinned: bool,
    pub tags: Vec<Tag>,
    pub created_at: u64,                // Unix timestamp (ms)
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,                 // 十六进制颜色，如 "#FF5733"
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClipboardRequest {
    pub content_type: String,
    pub content: ClipboardContent,
    pub tag_ids: Option<Vec<i64>>,     // 可选：创建时直接关联标签
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateClipboardRequest {
    pub is_pinned: Option<bool>,
    pub tag_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: String,
}
```

### 4.2 内容存储策略

| 类型 | 存储方式 | 说明 |
|------|----------|------|
| 文本 | `content_text TEXT` | 直接存储，最多 1MB |
| 图片 | `content_image BLOB` | RGBA 数据 gzip 压缩存储，限制 10MB |

## 五、CRUD 操作

### 5.1 剪切板历史 (ClipboardRepository)

```rust
pub trait ClipboardRepository {
    // 创建
    fn create(&self, req: &CreateClipboardRequest) -> Result<ClipboardRecord>;

    // 查询 (分页)
    fn list(&self, page: u32, page_size: u32) -> Result<Vec<ClipboardRecord>>;
    fn list_by_tag(&self, tag_id: i64, page: u32, page_size: u32) -> Result<Vec<ClipboardRecord>>;
    fn list_pinned(&self) -> Result<Vec<ClipboardRecord>>;
    fn get_by_id(&self, id: i64) -> Result<Option<ClipboardRecord>>;
    fn search(&self, keyword: &str, page: u32, page_size: u32) -> Result<Vec<ClipboardRecord>>;

    // 更新
    fn update_pinned(&self, id: i64, is_pinned: bool) -> Result<()>;
    fn update_tags(&self, id: i64, tag_ids: &[i64]) -> Result<()>;
    fn delete(&self, id: i64) -> Result<()>;

    // 删除旧记录 (清理)
    fn delete_older_than(&self, timestamp: u64, keep_pinned: bool) -> Result<u32>;
}
```

### 5.2 标签管理 (TagRepository)

```rust
pub trait TagRepository {
    // 创建
    fn create(&self, req: &CreateTagRequest) -> Result<Tag>;

    // 查询
    fn list_all(&self) -> Result<Vec<Tag>>;
    fn get_by_id(&self, id: i64) -> Result<Option<Tag>>;

    // 更新
    fn update(&self, id: i64, name: &str, color: &str) -> Result<()>;

    // 删除
    fn delete(&self, id: i64) -> Result<()>;
}
```

## 六、服务层设计

### 6.1 ClipboardService

```rust
pub struct ClipboardService {
    repo: Arc<ClipboardRepositoryImpl>,
}

impl ClipboardService {
    // 监听剪切板变化时调用
    pub fn on_clipboard_change(&self, content: ClipboardContent) -> Result<Option<ClipboardRecord>> {
        // 1. 检查是否与最后一条记录相同 (去重)
        // 2. 不同则创建新记录
        // 3. 超过 30 条时删除旧记录 (保留置顶)
    }

    // 置顶/取消置顶
    pub fn toggle_pin(&self, id: i64) -> Result<bool> {
        // 返回置顶后的状态
    }

    // 获取历史列表
    pub fn get_history(&self, page: u32, tag_id: Option<i64>) -> Result<Vec<ClipboardRecord>>;

    // 搜索
    pub fn search(&self, keyword: &str, page: u32) -> Result<Vec<ClipboardRecord>>;

    // 删除
    pub fn delete_record(&self, id: i64) -> Result<()>;

    // 更新标签
    pub fn update_record_tags(&self, id: i64, tag_ids: &[i64]) -> Result<()>;
}
```

## 七、Tauri 命令接口

```rust
// clipboard_commands.rs

#[tauri::command]
pub async fn create_clipboard_record(
    app: AppHandle,
    content_type: String,
    content: ClipboardContent,
) -> Result<ClipboardRecord, String>;

#[tauri::command]
pub async fn list_clipboard_history(
    app: AppHandle,
    page: u32,
    page_size: Option<u32>,
    tag_id: Option<i64>,
) -> Result<Vec<ClipboardRecord>, String>;

#[tauri::command]
pub async fn toggle_pin_clipboard(
    app: AppHandle,
    id: i64,
) -> Result<bool, String>;

#[tauri::command]
pub async fn delete_clipboard_record(
    app: AppHandle,
    id: i64,
) -> Result<(), String>;

#[tauri::command]
pub async fn update_record_tags(
    app: AppHandle,
    id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), String>;

// 标签命令
#[tauri::command]
pub async fn create_tag(
    app: AppHandle,
    name: String,
    color: String,
) -> Result<Tag, String>;

#[tauri::command]
pub async fn list_tags(
    app: AppHandle,
) -> Result<Vec<Tag>, String>;

#[tauri::command]
pub async fn delete_tag(
    app: AppHandle,
    id: i64,
) -> Result<(), String>;
```

## 八、数据库初始化

### 8.1 初始化时机

在 `lib.rs` 的 `setup` 阶段初始化数据库：

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // 初始化数据库
            let db = Database::new(app)?;
            app.manage(Arc::new(db));

            // 初始化服务
            let clipboard_service = ClipboardService::new(...);
            app.manage(clipboard_service);

            // ...
        })
        .build(...)
        .run(...);
}
```

### 8.2 数据库路径

```rust
impl Database {
    pub fn new(app: &tauri::App) -> Result<Self> {
        let app_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&app_dir)?;
        let db_path = app_dir.join("clipcat.db");

        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch(INIT_SQL)?;
        Ok(Self { conn: Arc::new(conn) })
    }
}
```

## 九、现有功能集成

### 9.1 修改 ClipboardManager

```rust
impl ClipboardManager {
    pub fn start_listening(&self) {
        // ... 现有代码 ...

        std::thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                Self::check_clipboard(&app_handle, &last_text_hash, &last_image_hash);
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    }

    fn check_clipboard(...) {
        // ... 现有检测逻辑 ...

        // 新增：保存到数据库
        if content_changed {
            let service = app_handle.state::<ClipboardService>();
            if let Err(e) = service.on_clipboard_change(content) {
                eprintln!("Failed to save clipboard: {}", e);
            }
        }
    }
}
```

### 9.2 前端集成

前端通过 Tauri 命令调用：

```typescript
// 前端调用示例
import { invoke } from '@tauri-apps/api/core';

const history = await invoke('list_clipboard_history', { page: 0, pageSize: 30 });
const pinned = await invoke('list_pinned_clipboard');
await invoke('toggle_pin_clipboard', { id: 1 });
await invoke('delete_clipboard_record', { id: 1 });
```

## 十、依赖添加

```toml
# Cargo.toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
r2d2 = "0.8"
r2d2_sqlite = "0.24"
serde = { version = "1", features = ["derive"] }
thiserror = "1"
tokio = { version = "1", features = ["sync"] }
```

## 十一、实现文件结构

```
src-tauri/
├── src/
│   ├── lib.rs              # 主入口
│   ├── clipboard.rs        # 现有剪切板监听
│   ├── clipboard_db.rs     # 新增：数据库操作
│   ├── clipboard_service.rs # 新增：服务层
│   ├── tag_db.rs           # 新增：标签数据库操作
│   └── commands.rs         # 新增：Tauri 命令
```

## 十二、颜色配置

标签默认 6 色循环：

| 索引 | 颜色 | 用途 |
|------|------|------|
| 0 | #EF4444 | 红色 |
| 1 | #F97316 | 橙色 |
| 2 | #EAB308 | 黄色 |
| 3 | #22C55E | 绿色 |
| 4 | #3B82F6 | 蓝色 |
| 5 | #8B5CF6 | 紫色 |

## 十三、配置参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| MAX_TEXT_LENGTH | 1MB | 文本最大长度 |
| MAX_IMAGE_SIZE | 10MB | 图片最大存储 |
| MAX_HISTORY_COUNT | 100 | 最大历史记录数 |
| CLEANUP_INTERVAL | 1h | 清理间隔 |
| PAGE_SIZE | 30 | 默认分页大小 |

## 十四、注意事项

1. **并发安全**: 使用 `Arc<Mutex<Connection>>` 或连接池
2. **大数据**: 图片 RGBA 数据使用 `zlib` 压缩存储
3. **迁移**: 预留 `schema_version` 表支持未来升级
4. **清理**: 后台任务定期清理旧记录 (保留置顶)
