use std::collections::HashMap;

use crate::database::{Database, DbError, DbConn};
use crate::tag_db::Tag;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    Image {
        width: u32,
        height: u32,
        thumbnail: Option<Vec<u8>>,
        original: Option<Vec<u8>>,
    },
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

                let original = if let ClipboardContent::Image { original: Some(o), .. } = &req.content {
                    o.clone()
                } else {
                    Vec::new()
                };

                if !original.is_empty() {
                    let thumbnail = crate::image_processor::ImageProcessor::generate_thumbnail(
                        &original, *width, *height
                    ).unwrap_or_else(|e| {
                        eprintln!("Thumbnail generation failed: {}", e);
                        original.clone()
                    });

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

    pub fn list(
        &self,
        page: u32,
        page_size: u32,
        tag_id: Option<i64>,
    ) -> Result<Vec<ClipboardRecord>, DbError> {
        let conn = self.db.get_conn()?;
        let offset = page * page_size;

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

    pub fn add_tag_to_record(&self, clipboard_id: i64, tag_id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO clipboard_tags (clipboard_id, tag_id) VALUES (?1, ?2)",
            params![clipboard_id, tag_id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn duplicate_and_move_to_top(&self, id: i64) -> Result<ClipboardRecord, DbError> {
        let conn = self.db.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        conn.execute_batch("BEGIN")?;

        let result = (|| -> Result<ClipboardRecord, DbError> {
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

    fn batch_get_tags_for_clipboards(&self, conn: &DbConn, clipboard_ids: &[i64]) -> Result<HashMap<i64, Vec<Tag>>, DbError> {
        if clipboard_ids.is_empty() {
            return Ok(HashMap::new());
        }

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

    pub fn hash_string(s: &str) -> u64 {
        xxhash_rust::xxh3::xxh3_64(s.as_bytes())
    }

    pub fn hash_bytes(bytes: &[u8]) -> u64 {
        xxhash_rust::xxh3::xxh3_64(bytes)
    }
}

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
