/// tag_db.rs — 标签数据仓库（Tag Repository）
///
/// 职责：
/// - 标签的 CRUD 操作（创建、查询、更新、删除）
/// - 6 色轮转自动分配颜色（红、橙、黄、绿、蓝、紫）
/// - 确保默认标签存在（id=1，名为 "Clipboard"）
use crate::database::{Database, DbError};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// 标签数据模型，对应 tag 表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// 创建标签请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: String,
}

/// 更新标签请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTagRequest {
    pub name: String,
    pub color: String,
}

/// 标签数据仓库，封装所有标签相关的数据库操作
pub struct TagRepository {
    db: Database,
}

impl TagRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 创建标签
    ///
    /// 插入新标签记录，created_at 和 updated_at 均设为当前时间戳（毫秒）
    pub fn create(&self, req: &CreateTagRequest) -> Result<Tag, DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT INTO tag (name, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![req.name, req.color, now],
        )?;

        let id = conn.last_insert_rowid();
        Ok(Tag {
            id,
            name: req.name.clone(),
            color: req.color.clone(),
            created_at: now as u64,
            updated_at: now as u64,
        })
    }

    /// 按 ID 查询标签
    ///
    /// 返回 None 表示标签不存在
    pub fn get_by_id(&self, id: i64) -> Result<Option<Tag>, DbError> {
        let conn = self.db.get_conn()?;

        let result = conn.query_row(
            "SELECT id, name, color, created_at, updated_at FROM tag WHERE id = ?1",
            params![id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    created_at: row.get::<_, i64>(3)? as u64,
                    updated_at: row.get::<_, i64>(4)? as u64,
                })
            },
        );

        match result {
            Ok(tag) => Ok(Some(tag)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 查询所有标签，按名称排序
    pub fn list_all(&self) -> Result<Vec<Tag>, DbError> {
        let conn = self.db.get_conn()?;

        let mut stmt = conn.prepare("SELECT id, name, color, created_at, updated_at FROM tag ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get::<_, i64>(3)? as u64,
                updated_at: row.get::<_, i64>(4)? as u64,
            })
        })?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }

        Ok(tags)
    }

    /// 更新标签（名称和颜色）
    ///
    /// 若标签不存在返回 DbError::NotFound
    pub fn update(&self, id: i64, req: &UpdateTagRequest) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let updated = conn.execute(
            "UPDATE tag SET name = ?1, color = ?2, updated_at = ?3 WHERE id = ?4",
            params![req.name, req.color, now, id],
        )?;

        if updated == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    /// 删除标签
    ///
    /// 若标签不存在返回 DbError::NotFound
    /// 关联的 clipboard_tags 记录会被外键 ON DELETE CASCADE 自动删除
    pub fn delete(&self, id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        let deleted = conn.execute("DELETE FROM tag WHERE id = ?1", params![id])?;

        if deleted == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    /// 获取下一个自动分配的颜色
    ///
    /// 根据 tag 表中的记录数量，按 6 色轮转返回对应颜色：
    /// 红(#EF4444) → 橙(#F97316) → 黄(#EAB308) → 绿(#22C55E) → 蓝(#3B82F6) → 紫(#8B5CF6)
    pub fn get_next_color(&self) -> Result<String, DbError> {
        let conn = self.db.get_conn()?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tag", [], |row| row.get(0))?;

        let colors = [
            "#EF4444",
            "#F97316",
            "#EAB308",
            "#22C55E",
            "#3B82F6",
            "#8B5CF6",
        ];

        let color_index = (count as usize) % colors.len();
        Ok(colors[color_index].to_string())
    }

    /// 确保默认标签存在
    ///
    /// 检查 id=1 的标签是否存在，若不存在则创建：
    /// - id 固定为 1（默认标签）
    /// - 名称和颜色由参数指定（通常为 "Clipboard" / "#6B7280"）
    ///
    /// 该标签在 ClipboardService 初始化时调用，用于为每条新记录自动分配默认标签
    pub fn ensure_default_tag_exists(&self, name: &str, color: &str) -> Result<Tag, DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        if let Some(tag) = self.get_by_id(1)? {
            return Ok(tag);
        }

        conn.execute(
            "INSERT INTO tag (id, name, color, created_at, updated_at) VALUES (1, ?1, ?2, ?3, ?3)",
            params![name, color, now],
        )?;

        Ok(Tag {
            id: 1,
            name: name.to_string(),
            color: color.to_string(),
            created_at: now as u64,
            updated_at: now as u64,
        })
    }
}
