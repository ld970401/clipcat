/// clipboard_db.rs — 剪切板记录数据仓库（Clipboard Repository）
///
/// 职责：
/// - 剪切板记录的 CRUD 操作（创建、查询、置顶、删除）
/// - 图片缩略图和原始数据的存储与批量查询
/// - 记录与标签的关联管理
/// - 复制并置顶（duplicate_and_move_to_top，事务性操作）
/// - 按时间/数量清理历史记录
/// - 基于哈希的内容去重查询
///
/// 核心数据流：
/// ClipboardManager → ClipboardService → ClipboardRepository → SQLite
use std::collections::HashMap;

use crate::database::{Database, DbError, DbConn};
use crate::tag_db::Tag;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// 剪切板内容枚举，区分文本和图片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    /// 文本内容
    Text(String),
    /// 图片内容
    /// - `width`/`height`: 图片尺寸
    /// - `thumbnail`: 缩略图 PNG 数据（前端展示用）
    /// - `original`: 原始 RGBA 像素数据（用于写回剪贴板），向前端发送时置为 None 以节省带宽
    Image {
        width: u32,
        height: u32,
        thumbnail: Option<Vec<u8>>,
        original: Option<Vec<u8>>,
    },
}

/// 剪切板记录完整模型，对应 clipboard 表 + 关联数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardRecord {
    pub id: i64,
    pub content_type: String,
    pub content: ClipboardContent,
    pub is_pinned: bool,
    pub tags: Vec<Tag>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// 创建剪切板记录请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClipboardRequest {
    pub content_type: String,
    pub content: ClipboardContent,
}

/// 更新剪切板记录请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateClipboardRequest {
    pub is_pinned: Option<bool>,
    pub tag_ids: Option<Vec<i64>>,
}

/// 剪切板数据仓库，封装所有剪切板记录相关的数据库操作
pub struct ClipboardRepository {
    db: Database,
}

impl ClipboardRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 创建剪切板记录
    ///
    /// 根据内容类型分别处理：
    /// - **文本**: 直接插入 clipboard 表的 content_text 字段
    /// - **图片**: 插入 clipboard 表的宽高信息，同时调用 ImageProcessor 生成缩略图，
    ///   将缩略图（PNG）和原始数据（RGBA）存入 clipboard_images 表
    pub fn create(&self, req: &CreateClipboardRequest) -> Result<ClipboardRecord, DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        match &req.content {
            ClipboardContent::Text(text) => {
                conn.execute(
                    "INSERT INTO clipboard (content_type, content_text, is_pinned, created_at, updated_at)
                     VALUES ('text', ?1, 0, ?2, ?2)",
                    params![text, now as i64],
                )?;

                let id = conn.last_insert_rowid();
                Ok(ClipboardRecord {
                    id,
                    content_type: "text".to_string(),
                    content: ClipboardContent::Text(text.clone()),
                    is_pinned: false,
                    tags: vec![],
                    created_at: now,
                    updated_at: now,
                })
            }
            ClipboardContent::Image { width, height, thumbnail: _, original: _ } => {
                conn.execute(
                    "INSERT INTO clipboard (content_type, width, height, is_pinned, created_at, updated_at)
                     VALUES ('image', ?1, ?2, 0, ?3, ?3)",
                    params![*width as i64, *height as i64, now as i64],
                )?;

                let id = conn.last_insert_rowid();

                // 提取原始 RGBA 数据，用于生成缩略图和存储
                let original = if let ClipboardContent::Image { original: Some(o), .. } = &req.content {
                    o.clone()
                } else {
                    Vec::new()
                };

                if !original.is_empty() {
                    // 生成缩略图，失败时回退使用原始数据
                    let thumbnail = crate::image_processor::ImageProcessor::generate_thumbnail(
                        &original, *width, *height
                    ).unwrap_or_else(|e| {
                        eprintln!("Thumbnail generation failed: {}", e);
                        original.clone()
                    });

                    // 将缩略图和原始图片数据存入 clipboard_images 表
                    conn.execute(
                        "INSERT INTO clipboard_images (id, thumbnail, original, file_size) VALUES (?1, ?2, ?3, ?4)",
                        params![id, thumbnail.clone(), original.clone(), original.len() as i64],
                    )?;

                    Ok(ClipboardRecord {
                        id,
                        content_type: "image".to_string(),
                        content: ClipboardContent::Image {
                            width: *width,
                            height: *height,
                            thumbnail: Some(thumbnail),
                            original: Some(original),
                        },
                        is_pinned: false,
                        tags: vec![],
                        created_at: now,
                        updated_at: now,
                    })
                } else {
                    // 无原始图片数据的情况（仅存元信息）
                    Ok(ClipboardRecord {
                        id,
                        content_type: "image".to_string(),
                        content: ClipboardContent::Image {
                            width: *width,
                            height: *height,
                            thumbnail: None,
                            original: None,
                        },
                        is_pinned: false,
                        tags: vec![],
                        created_at: now,
                        updated_at: now,
                    })
                }
            }
        }
    }

    /// 按 ID 查询单条记录（含标签和图片数据）
    pub fn get_by_id(&self, id: i64) -> Result<Option<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, content_type, content_text, width, height, is_pinned, created_at, updated_at
             FROM clipboard WHERE id = ?1",
        )?;

        let record = stmt
            .query_row(params![id], |row| {
                Ok(ClipboardRow {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content_text: row.get(2)?,
                    width: row.get(3)?,
                    height: row.get(4)?,
                    is_pinned: row.get::<_, i64>(5)? != 0,
                    created_at: row.get::<_, i64>(6)? as u64,
                    updated_at: row.get::<_, i64>(7)? as u64,
                })
            })
            .optional()?;

        match record {
            Some(row) => {
                let tags = self.get_tags_for_clipboard(&conn, id)?;

                // 图片类型需要额外查询 clipboard_images 表
                let (thumbnail, original) = if row.content_type == "image" {
                    let mut img_stmt = conn.prepare("SELECT thumbnail, original FROM clipboard_images WHERE id = ?1")?;
                    let img_result = img_stmt.query_row(params![id], |row| {
                        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
                    }).optional()?;
                    img_result.unwrap_or((Vec::new(), None))
                } else {
                    (Vec::new(), None)
                };

                let thumbnail = if thumbnail.is_empty() { None } else { Some(thumbnail) };
                let original_for_record = original;

                Ok(Some(self.row_to_record(row, tags, thumbnail, original_for_record)?))
            }
            None => Ok(None),
        }
    }

    /// 分页查询剪切板历史列表
    ///
    /// - `page`: 页码（从 0 开始）
    /// - `page_size`: 每页条数
    /// - `tag_id`: 可选标签过滤，若指定则只返回该标签下的记录
    ///
    /// 排序规则：置顶记录优先（is_pinned DESC），然后按创建时间倒序
    ///
    /// 性能优化：使用批量查询获取标签和缩略图，避免 N+1 查询问题
    /// 列表查询不加载原始图片数据（original），仅加载缩略图用于前端展示
    pub fn list(
        &self,
        page: u32,
        page_size: u32,
        tag_id: Option<i64>,
    ) -> Result<Vec<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;
        let offset = page * page_size;

        // 根据是否有标签过滤，构建不同的 SQL 查询
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match tag_id {
            Some(tid) => (
                "SELECT c.id, c.content_type, c.content_text, c.width, c.height, c.is_pinned, c.created_at, c.updated_at
                 FROM clipboard c
                 INNER JOIN clipboard_tags ct ON c.id = ct.clipboard_id
                 WHERE ct.tag_id = ?1
                 ORDER BY c.is_pinned DESC, c.created_at DESC
                 LIMIT ?2 OFFSET ?3".to_string(),
                vec![Box::new(tid), Box::new(page_size as i64), Box::new(offset as i64)],
            ),
            None => (
                "SELECT id, content_type, content_text, width, height, is_pinned, created_at, updated_at
                 FROM clipboard
                 ORDER BY is_pinned DESC, created_at DESC
                 LIMIT ?1 OFFSET ?2".to_string(),
                vec![Box::new(page_size as i64), Box::new(offset as i64)],
            ),
        };

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(ClipboardRow {
                id: row.get(0)?,
                content_type: row.get(1)?,
                content_text: row.get(2)?,
                width: row.get(3)?,
                height: row.get(4)?,
                is_pinned: row.get::<_, i64>(5)? != 0,
                created_at: row.get::<_, i64>(6)? as u64,
                updated_at: row.get::<_, i64>(7)? as u64,
            })
        })?;

        let mut rows_vec: Vec<ClipboardRow> = Vec::new();
        for row in rows {
            rows_vec.push(row?);
        }

        // 批量查询标签和缩略图，避免 N+1 问题
        let ids: Vec<i64> = rows_vec.iter().map(|r| r.id).collect();
        let tags_map = self.batch_get_tags_for_clipboards(&conn, &ids)?;
        let thumbnails_map = self.batch_get_thumbnails(&conn, &ids)?;

        let mut records = Vec::new();
        for row in rows_vec {
            let tags: Vec<Tag> = tags_map.get(&row.id).cloned().unwrap_or_default();
            let thumbnail = thumbnails_map.get(&row.id).cloned();
            records.push(self.row_to_record(row, tags, thumbnail, None)?);
        }

        Ok(records)
    }

    /// 切换记录的置顶状态
    pub fn update_pinned(&self, id: i64, is_pinned: bool) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "UPDATE clipboard SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3",
            params![is_pinned as i64, now, id],
        )?;

        Ok(())
    }

    /// 更新记录的标签关联（全量替换）
    ///
    /// 先删除该记录的所有标签关联，再重新插入指定的标签 ID 列表
    pub fn update_tags(&self, id: i64, tag_ids: &[i64]) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // 先删除旧的标签关联
        conn.execute(
            "DELETE FROM clipboard_tags WHERE clipboard_id = ?1",
            params![id],
        )?;

        // 逐个插入新标签关联
        for tag_id in tag_ids {
            conn.execute(
                "INSERT INTO clipboard_tags (clipboard_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )?;
        }

        conn.execute(
            "UPDATE clipboard SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;

        Ok(())
    }

    /// 为记录添加单个标签关联
    ///
    /// 使用 INSERT OR IGNORE 避免重复插入
    pub fn add_tag_to_record(&self, clipboard_id: i64, tag_id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO clipboard_tags (clipboard_id, tag_id) VALUES (?1, ?2)",
            params![clipboard_id, tag_id],
        )?;
        Ok(())
    }

    /// 删除记录
    ///
    /// clipboard_images 和 clipboard_tags 的关联记录由外键 ON DELETE CASCADE 自动删除
    pub fn delete(&self, id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// 复制并置顶：创建原记录的副本（新时间戳），原记录被删除
    ///
    /// 整个操作在事务中执行，保证原子性：
    /// 1. 读取原记录数据
    /// 2. 插入新记录（created_at 为当前时间，is_pinned 为 false）
    /// 3. 复制图片数据到新记录
    /// 4. 复制标签关联到新记录
    /// 5. 提交事务（失败则回滚）
    ///
    /// 注意：调用方需在事务外另行删除原记录（paste.rs 中的 paste_item 命令）
    pub fn duplicate_and_move_to_top(&self, id: i64) -> Result<ClipboardRecord, DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        conn.execute_batch("BEGIN")?;

        let result = (|| -> Result<ClipboardRecord, DbError> {
            // 1. 读取原记录
            let mut stmt = conn.prepare(
                "SELECT id, content_type, content_text, width, height, is_pinned FROM clipboard WHERE id = ?1",
            )?;

            let record = stmt.query_row(params![id], |row| {
                Ok(ClipboardRow {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content_text: row.get(2)?,
                    width: row.get(3)?,
                    height: row.get(4)?,
                    is_pinned: row.get::<_, i64>(5)? != 0,
                    created_at: now,
                    updated_at: now,
                })
            })?;

            // 2. 插入新记录
            match record.content_type.as_str() {
                "text" => {
                    conn.execute(
                        "INSERT INTO clipboard (content_type, content_text, is_pinned, created_at, updated_at)
                         VALUES ('text', ?1, 0, ?2, ?2)",
                        params![record.content_text, now as i64],
                    )?;
                }
                "image" => {
                    conn.execute(
                        "INSERT INTO clipboard (content_type, width, height, is_pinned, created_at, updated_at)
                         VALUES ('image', ?1, ?2, 0, ?3, ?3)",
                        params![record.width.unwrap_or(0) as i64, record.height.unwrap_or(0) as i64, now as i64],
                    )?;
                }
                _ => return Err(DbError::NotFound),
            }

            let new_id = conn.last_insert_rowid();

            // 3. 复制图片数据
            let mut img_stmt = conn.prepare("SELECT thumbnail, original FROM clipboard_images WHERE id = ?1")?;
            let img_result = img_stmt.query_row(params![id], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
            }).optional()?;

            if let Some((thumbnail, original)) = img_result {
                conn.execute(
                    "INSERT INTO clipboard_images (id, thumbnail, original, file_size) VALUES (?1, ?2, ?3, ?4)",
                    params![new_id, thumbnail, original, original.len() as i64],
                )?;
            }

            // 4. 复制标签关联
            let mut tag_stmt = conn.prepare("SELECT tag_id FROM clipboard_tags WHERE clipboard_id = ?1")?;
            let tag_ids: Vec<i64> = tag_stmt.query_map(params![id], |row| {
                row.get::<_, i64>(0)
            })?.filter_map(|r| r.ok()).collect();

            for tag_id in tag_ids {
                conn.execute(
                    "INSERT OR IGNORE INTO clipboard_tags (clipboard_id, tag_id) VALUES (?1, ?2)",
                    params![new_id, tag_id],
                )?;
            }

            // 5. 查询新记录的标签和图片数据，构建完整的返回结果
            let tags = self.get_tags_for_clipboard(&conn, new_id)?;
            let (thumbnail, original) = if record.content_type == "image" {
                let mut img_stmt = conn.prepare("SELECT thumbnail, original FROM clipboard_images WHERE id = ?1")?;
                let img_result = img_stmt.query_row(params![new_id], |row| {
                    Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
                }).optional()?;
                img_result.unwrap_or((Vec::new(), None))
            } else {
                (Vec::new(), None)
            };

            let thumbnail = if thumbnail.is_empty() { None } else { Some(thumbnail) };
            let original_for_record = original;

            let new_record = ClipboardRow {
                id: new_id,
                content_type: record.content_type,
                content_text: record.content_text,
                width: record.width,
                height: record.height,
                is_pinned: false,
                created_at: now,
                updated_at: now,
            };

            Ok(self.row_to_record(new_record, tags, thumbnail, original_for_record)?)
        })();

        match result {
            Ok(r) => {
                conn.execute_batch("COMMIT")?;
                Ok(r)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// 删除早于指定时间戳的记录
    ///
    /// - `timestamp`: 截止时间戳（毫秒）
    /// - `keep_pinned`: 是否保留置顶记录
    ///
    /// 返回被删除的记录数
    pub fn delete_older_than(
        &self,
        timestamp: u64,
        keep_pinned: bool,
    ) -> Result<u32, DbError> {
        let conn = self.db.get_conn()?;

        let sql = if keep_pinned {
            "DELETE FROM clipboard WHERE created_at < ?1 AND is_pinned = 0"
        } else {
            "DELETE FROM clipboard WHERE created_at < ?1"
        };

        let deleted = conn.execute(sql, params![timestamp as i64])?;
        Ok(deleted as u32)
    }

    /// 删除超出数量限制的记录（保留最新的 max_count 条）
    ///
    /// - `max_count`: 最大保留数量
    /// - `keep_pinned`: 是否保留置顶记录不受数量限制
    ///
    /// 实现方式：按创建时间降序排列，使用 OFFSET 跳过保留的记录，删除其余记录
    /// 返回被删除的记录数
    pub fn delete_excess_count(&self, max_count: u32, keep_pinned: bool) -> Result<u32, DbError> {
        let conn = self.db.get_conn()?;

        let sql = if keep_pinned {
            "DELETE FROM clipboard WHERE id IN (
                SELECT id FROM clipboard WHERE is_pinned = 0
                ORDER BY created_at DESC
                LIMIT -1 OFFSET ?1
            )"
        } else {
            "DELETE FROM clipboard WHERE id IN (
                SELECT id FROM clipboard
                ORDER BY created_at DESC
                LIMIT -1 OFFSET ?1
            )"
        };

        let deleted = conn.execute(sql, params![max_count as i64])?;
        Ok(deleted as u32)
    }

    /// 获取最新文本记录的哈希值
    ///
    /// 用于去重判断：与当前剪贴板文本哈希比较，若相同则说明是重复内容
    pub fn get_last_text_hash(&self) -> Result<Option<u64>, DbError> {
        let conn = self.db.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT content_text FROM clipboard WHERE content_type = 'text' ORDER BY created_at DESC LIMIT 1",
        )?;

        let result = stmt
            .query_row([], |row| {
                let text: Option<String> = row.get(0)?;
                Ok(text)
            })
            .optional()?;

        match result {
            Some(Some(text)) => Ok(Some(ClipboardRepository::hash_string(&text))),
            _ => Ok(None),
        }
    }

    /// 获取最新图片记录的哈希值
    ///
    /// 基于原始 RGBA 数据计算哈希，用于图片去重判断
    pub fn get_last_image_hash(&self) -> Result<Option<u64>, DbError> {
        let conn = self.db.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT ci.original FROM clipboard_images ci
             INNER JOIN clipboard c ON ci.id = c.id
             WHERE c.content_type = 'image'
             ORDER BY c.created_at DESC LIMIT 1",
        )?;

        let result = stmt
            .query_row([], |row| {
                let image: Option<Vec<u8>> = row.get(0)?;
                Ok(image)
            })
            .optional()?;

        match result {
            Some(Some(rgba)) => Ok(Some(ClipboardRepository::hash_bytes(&rgba))),
            _ => Ok(None),
        }
    }

    /// 查询单条记录关联的所有标签（内部辅助方法）
    fn get_tags_for_clipboard(&self, conn: &DbConn, clipboard_id: i64) -> Result<Vec<Tag>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, t.color, t.created_at, t.updated_at
             FROM tag t
             INNER JOIN clipboard_tags ct ON t.id = ct.tag_id
             WHERE ct.clipboard_id = ?1",
        )?;

        let rows = stmt.query_map(params![clipboard_id], |row| {
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

    /// 批量查询多条记录的标签（避免 N+1 查询）
    ///
    /// 返回 HashMap<clipboard_id, Vec<Tag>>，未找到标签的记录不会出现在 Map 中
    fn batch_get_tags_for_clipboards(&self, conn: &DbConn, clipboard_ids: &[i64]) -> Result<HashMap<i64, Vec<Tag>>, DbError> {
        if clipboard_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // 动态构建 IN 子句的占位符
        let placeholders: Vec<String> = clipboard_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT ct.clipboard_id, t.id, t.name, t.color, t.created_at, t.updated_at
             FROM clipboard_tags ct
             INNER JOIN tag t ON ct.tag_id = t.id
             WHERE ct.clipboard_id IN ({})",
            placeholders.join(",")
        );

        let mut stmt = conn.prepare(&sql)?;
        let ids_ref: Vec<i64> = clipboard_ids.to_vec();
        let params: Vec<&dyn rusqlite::ToSql> = ids_ref.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                Tag {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                    updated_at: row.get::<_, i64>(5)? as u64,
                },
            ))
        })?;

        let mut tags_map: HashMap<i64, Vec<Tag>> = HashMap::new();
        for row_result in rows {
            let (clipboard_id, tag) = row_result?;
            tags_map.entry(clipboard_id).or_insert_with(Vec::new).push(tag);
        }

        Ok(tags_map)
    }

    /// 批量查询多条记录的缩略图（避免 N+1 查询）
    ///
    /// 返回 HashMap<id, thumbnail_bytes>，仅包含有缩略图的图片记录
    fn batch_get_thumbnails(&self, conn: &DbConn, clipboard_ids: &[i64]) -> Result<HashMap<i64, Vec<u8>>, DbError> {
        if clipboard_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = clipboard_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT id, thumbnail FROM clipboard_images WHERE id IN ({})",
            placeholders.join(",")
        );

        let mut stmt = conn.prepare(&sql)?;
        let ids_ref: Vec<i64> = clipboard_ids.to_vec();
        let params: Vec<&dyn rusqlite::ToSql> = ids_ref.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;

        let mut thumbnails_map: HashMap<i64, Vec<u8>> = HashMap::new();
        for row_result in rows {
            let (id, thumbnail) = row_result?;
            thumbnails_map.insert(id, thumbnail);
        }

        Ok(thumbnails_map)
    }

    /// 将数据库行数据（ClipboardRow）转换为业务模型（ClipboardRecord）
    ///
    /// 根据内容类型构建对应的 ClipboardContent：
    /// - "text" → ClipboardContent::Text
    /// - "image" → ClipboardContent::Image（包含缩略图和原始数据）
    fn row_to_record(&self, row: ClipboardRow, tags: Vec<Tag>, thumbnail: Option<Vec<u8>>, original: Option<Vec<u8>>) -> Result<ClipboardRecord, DbError> {
        let content = match row.content_type.as_str() {
            "text" => {
                let text = row.content_text.unwrap_or_default();
                ClipboardContent::Text(text)
            }
            "image" => {
                let width = row.width.unwrap_or(0) as u32;
                let height = row.height.unwrap_or(0) as u32;
                ClipboardContent::Image {
                    width,
                    height,
                    thumbnail,
                    original,
                }
            }
            _ => return Err(DbError::NotFound),
        };

        Ok(ClipboardRecord {
            id: row.id,
            content_type: row.content_type,
            content,
            is_pinned: row.is_pinned,
            tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// 使用 xxhash3 对字符串计算 64 位哈希（用于文本去重）
    pub fn hash_string(s: &str) -> u64 {
        xxhash_rust::xxh3::xxh3_64(s.as_bytes())
    }

    /// 使用 xxhash3 对字节数组计算 64 位哈希（用于图片去重）
    pub fn hash_bytes(bytes: &[u8]) -> u64 {
        xxhash_rust::xxh3::xxh3_64(bytes)
    }
}

/// 数据库行数据中间结构，用于从 clipboard 表读取原始行
struct ClipboardRow {
    id: i64,
    content_type: String,
    content_text: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    is_pinned: bool,
    created_at: u64,
    updated_at: u64,
}

/// rusqlite 查询结果的 Optional 扩展
///
/// 将 QueryReturnedNoRows 错误转换为 Ok(None)，
/// 其他错误原样传递。用于单行查询的"存在性"判断。
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
