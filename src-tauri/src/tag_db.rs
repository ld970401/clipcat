use crate::database::{Database, DbError};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTagRequest {
    pub name: String,
    pub color: String,
}

pub struct TagRepository {
    db: Database,
}

impl TagRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

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

    pub fn delete(&self, id: i64) -> Result<(), DbError> {
        let conn = self.db.get_conn()?;
        let deleted = conn.execute("DELETE FROM tag WHERE id = ?1", params![id])?;

        if deleted == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    pub fn get_next_color(&self) -> Result<String, DbError> {
        let conn = self.db.get_conn()?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tag", [], |row| row.get(0))?;

        let colors = [
            "#EF4444", // 红色
            "#F97316", // 橙色
            "#EAB308", // 黄色
            "#22C55E", // 绿色
            "#3B82F6", // 蓝色
            "#8B5CF6", // 紫色
        ];

        let color_index = (count as usize) % colors.len();
        Ok(colors[color_index].to_string())
    }

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
