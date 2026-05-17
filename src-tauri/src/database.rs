use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Not found")]
    NotFound,
}

pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConn = PooledConnection<SqliteConnectionManager>;

pub struct Database {
    pool: Arc<DbPool>,
}

impl Database {
    pub fn new(app_dir: PathBuf) -> Result<Self, DbError> {
        std::fs::create_dir_all(&app_dir)?;
        let db_path = app_dir.join("clipcat.db");

        let manager = SqliteConnectionManager::file(&db_path)
            .with_init(|conn| {
                conn.execute_batch("PRAGMA foreign_keys = ON")?;
                Ok(())
            });
        let pool = Pool::builder().max_size(10).build(manager)?;

        let db = Self {
            pool: Arc::new(pool),
        };

        db.init_schema()?;
        Ok(db)
    }

    pub fn get_conn(&self) -> Result<DbConn, DbError> {
        Ok(self.pool.get()?)
    }

    fn init_schema(&self) -> Result<(), DbError> {
        let conn = self.get_conn()?;

        conn.execute_batch(
            r#"
            -- 创建表（如果不存在）
            -- 剪切板历史记录表
            CREATE TABLE IF NOT EXISTS clipboard (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type    TEXT NOT NULL CHECK (content_type IN ('text', 'image')),
                content_text    TEXT,
                width           INTEGER,
                height          INTEGER,
                is_pinned       INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );

            -- 标签表
            CREATE TABLE IF NOT EXISTS tag (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL UNIQUE,
                color       TEXT NOT NULL DEFAULT '#6B7280',
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            );

            -- 剪切板-标签关联表
            CREATE TABLE IF NOT EXISTS clipboard_tags (
                clipboard_id INTEGER NOT NULL,
                tag_id      INTEGER NOT NULL,
                PRIMARY KEY (clipboard_id, tag_id),
                FOREIGN KEY (clipboard_id) REFERENCES clipboard(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tag(id) ON DELETE CASCADE
            );

            -- 索引
            CREATE INDEX IF NOT EXISTS idx_clipboard_created_at ON clipboard(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_clipboard_is_pinned ON clipboard(is_pinned);
            CREATE INDEX IF NOT EXISTS idx_clipboard_content_type ON clipboard(content_type);

            -- 设置表
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- 图片表（存储缩略图）
            CREATE TABLE IF NOT EXISTS clipboard_images (
                id INTEGER PRIMARY KEY,
                thumbnail BLOB NOT NULL,
                original BLOB,
                file_size INTEGER,
                FOREIGN KEY (id) REFERENCES clipboard(id) ON DELETE CASCADE
            );

            -- 初始化默认设置（使用 INSERT OR IGNORE 避免覆盖已有值）
            INSERT OR IGNORE INTO settings (key, value) VALUES ('save_mode', 'duration');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('retention_duration', '30');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('retention_count', '500');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('cleanup_time', '00:00');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('shortcut_show', 'Cmd+Shift+V');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('shortcut_pin', 'Cmd+Shift+P');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('locale', 'en');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('launch_at_login', 'false');
            "#,
        )?;

        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?")?;
        let mut rows = stmt.query([key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        let conn = self.get_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
            [key, value],
        )?;
        Ok(())
    }

}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
        }
    }
}
