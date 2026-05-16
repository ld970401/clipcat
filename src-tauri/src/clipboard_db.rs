use crate::database::{Database, DbError, DbConn};
use crate::tag_db::Tag;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    Image { width: u32, height: u32, rgba: Vec<u8> },
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClipboardRequest {
    pub content_type: String,
    pub content: ClipboardContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateClipboardRequest {
    pub is_pinned: Option<bool>,
    pub tag_ids: Option<Vec<i64>>,
}

pub struct ClipboardRepository {
    db: Database,
}

impl ClipboardRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create(&self, req: &CreateClipboardRequest) -> Result<ClipboardRecord, DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let (content_text, content_image, width, height) = match &req.content {
            ClipboardContent::Text(text) => (Some(text.clone()), None, None, None),
            ClipboardContent::Image { width, height, rgba } => {
                (None, Some(rgba.clone()), Some(*width as i64), Some(*height as i64))
            }
        };

        conn.execute(
            "INSERT INTO clipboard (content_type, content_text, content_image, width, height, is_pinned, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
            params![
                req.content_type,
                content_text,
                content_image,
                width,
                height,
                now as i64
            ],
        )?;

        let id = conn.last_insert_rowid();
        Ok(ClipboardRecord {
            id,
            content_type: req.content_type.clone(),
            content: req.content.clone(),
            is_pinned: false,
            tags: vec![],
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, content_type, content_text, content_image, width, height, is_pinned, created_at, updated_at
             FROM clipboard WHERE id = ?1",
        )?;

        let record = stmt
            .query_row(params![id], |row| {
                Ok(ClipboardRow {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content_text: row.get(2)?,
                    content_image: row.get(3)?,
                    width: row.get(4)?,
                    height: row.get(5)?,
                    is_pinned: row.get::<_, i64>(6)? != 0,
                    created_at: row.get::<_, i64>(7)? as u64,
                    updated_at: row.get::<_, i64>(8)? as u64,
                })
            })
            .optional()?;

        match record {
            Some(row) => {
                let tags = self.get_tags_for_clipboard(&conn, id)?;
                Ok(Some(self.row_to_record(row, tags)?))
            }
            None => Ok(None),
        }
    }

    pub fn list(
        &self,
        page: u32,
        page_size: u32,
        tag_id: Option<i64>,
    ) -> Result<Vec<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;
        let offset = page * page_size;

        let sql: String;

        match tag_id {
            Some(tid) => {
                sql = "SELECT c.id, c.content_type, c.content_text, c.content_image, c.width, c.height, c.is_pinned, c.created_at, c.updated_at
                       FROM clipboard c
                       INNER JOIN clipboard_tags ct ON c.id = ct.clipboard_id
                       WHERE ct.tag_id = ?1
                       ORDER BY c.is_pinned DESC, c.created_at DESC
                       LIMIT ?2 OFFSET ?3".to_string();

                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(params![tid, page_size as i64, offset as i64], |row| {
                    Ok(ClipboardRow {
                        id: row.get(0)?,
                        content_type: row.get(1)?,
                        content_text: row.get(2)?,
                        content_image: row.get(3)?,
                        width: row.get(4)?,
                        height: row.get(5)?,
                        is_pinned: row.get::<_, i64>(6)? != 0,
                        created_at: row.get::<_, i64>(7)? as u64,
                        updated_at: row.get::<_, i64>(8)? as u64,
                    })
                })?;

                let mut records = Vec::new();
                for row in rows {
                    let row = row?;
                    let tags = self.get_tags_for_clipboard(&conn, row.id)?;
                    records.push(self.row_to_record(row, tags)?);
                }
                return Ok(records);
            }
            None => {
                sql = "SELECT id, content_type, content_text, content_image, width, height, is_pinned, created_at, updated_at
                       FROM clipboard
                       ORDER BY is_pinned DESC, created_at DESC
                       LIMIT ?1 OFFSET ?2".to_string();

                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(params![page_size as i64, offset as i64], |row| {
                    Ok(ClipboardRow {
                        id: row.get(0)?,
                        content_type: row.get(1)?,
                        content_text: row.get(2)?,
                        content_image: row.get(3)?,
                        width: row.get(4)?,
                        height: row.get(5)?,
                        is_pinned: row.get::<_, i64>(6)? != 0,
                        created_at: row.get::<_, i64>(7)? as u64,
                        updated_at: row.get::<_, i64>(8)? as u64,
                    })
                })?;

                let mut records = Vec::new();
                for row in rows {
                    let row = row?;
                    let tags = self.get_tags_for_clipboard(&conn, row.id)?;
                    records.push(self.row_to_record(row, tags)?);
                }
                return Ok(records);
            }
        }
    }

    pub fn list_pinned(&self) -> Result<Vec<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, content_type, content_text, content_image, width, height, is_pinned, created_at, updated_at
             FROM clipboard
             WHERE is_pinned = 1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ClipboardRow {
                id: row.get(0)?,
                content_type: row.get(1)?,
                content_text: row.get(2)?,
                content_image: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
                is_pinned: row.get::<_, i64>(6)? != 0,
                created_at: row.get::<_, i64>(7)? as u64,
                updated_at: row.get::<_, i64>(8)? as u64,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            let row = row?;
            let tags = self.get_tags_for_clipboard(&conn, row.id)?;
            records.push(self.row_to_record(row, tags)?);
        }

        Ok(records)
    }

    pub fn search(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;
        let offset = page * page_size;
        let search_pattern = format!("%{}%", keyword);

        let mut stmt = conn.prepare(
            "SELECT id, content_type, content_text, content_image, width, height, is_pinned, created_at, updated_at
             FROM clipboard
             WHERE content_type = 'text' AND content_text LIKE ?1
             ORDER BY is_pinned DESC, created_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;

        let rows = stmt.query_map(
            params![search_pattern, page_size as i64, offset as i64],
            |row| {
                Ok(ClipboardRow {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content_text: row.get(2)?,
                    content_image: row.get(3)?,
                    width: row.get(4)?,
                    height: row.get(5)?,
                    is_pinned: row.get::<_, i64>(6)? != 0,
                    created_at: row.get::<_, i64>(7)? as u64,
                    updated_at: row.get::<_, i64>(8)? as u64,
                })
            },
        )?;

        let mut records = Vec::new();
        for row in rows {
            let row = row?;
            let tags = self.get_tags_for_clipboard(&conn, row.id)?;
            records.push(self.row_to_record(row, tags)?);
        }

        Ok(records)
    }

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

    pub fn update_tags(&self, id: i64, tag_ids: &[i64]) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "DELETE FROM clipboard_tags WHERE clipboard_id = ?1",
            params![id],
        )?;

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

    pub fn delete(&self, id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
        Ok(())
    }

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

    pub fn get_last_image_hash(&self) -> Result<Option<u64>, DbError> {
        let conn = self.db.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT content_image FROM clipboard WHERE content_type = 'image' ORDER BY created_at DESC LIMIT 1",
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

    fn row_to_record(&self, row: ClipboardRow, tags: Vec<Tag>) -> Result<ClipboardRecord, DbError> {
        let content = match row.content_type.as_str() {
            "text" => {
                let text = row.content_text.unwrap_or_default();
                ClipboardContent::Text(text)
            }
            "image" => {
                let rgba = row.content_image.unwrap_or_default();
                let width = row.width.unwrap_or(0) as u32;
                let height = row.height.unwrap_or(0) as u32;
                ClipboardContent::Image { width, height, rgba }
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

    pub fn hash_string(s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    pub fn hash_bytes(bytes: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }
}

struct ClipboardRow {
    id: i64,
    content_type: String,
    content_text: Option<String>,
    content_image: Option<Vec<u8>>,
    width: Option<i64>,
    height: Option<i64>,
    is_pinned: bool,
    created_at: u64,
    updated_at: u64,
}

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
