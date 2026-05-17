use crate::clipboard_db::{ClipboardContent, ClipboardRecord, ClipboardRepository, CreateClipboardRequest};
use crate::database::Database;
use crate::tag_db::{CreateTagRequest, Tag, TagRepository};
use std::sync::Arc;

pub struct ClipboardService {
    pub clipboard_repo: Arc<ClipboardRepository>,
    tag_repo: Arc<TagRepository>,
}

impl ClipboardService {
    pub fn new(db: Database) -> Self {
        let tag_repo = TagRepository::new(db.clone());
        if let Err(e) = tag_repo.ensure_default_tag_exists("Clipboard", "#6B7280") {
            eprintln!("Failed to ensure default tag exists: {}", e);
        }
        Self {
            clipboard_repo: Arc::new(ClipboardRepository::new(db.clone())),
            tag_repo: Arc::new(tag_repo),
        }
    }

    pub fn on_clipboard_change(&self, content: ClipboardContent) -> Result<Option<ClipboardRecord>, String> {
        let (hash, content_type) = match &content {
            ClipboardContent::Text(text) => {
                (ClipboardRepository::hash_string(text), "text")
            }
            ClipboardContent::Image { original: Some(rgba), .. } => {
                let hash = ClipboardRepository::hash_bytes(rgba);
                (hash, "image")
            }
            ClipboardContent::Image { original: None, .. } => {
                return Err("Image content missing original data".to_string());
            }
        };

        let last_hash = if content_type == "text" {
            self.clipboard_repo.get_last_text_hash().map_err(|e| e.to_string())?
        } else {
            self.clipboard_repo.get_last_image_hash().map_err(|e| e.to_string())?
        };

        if last_hash == Some(hash) {
            return Ok(None);
        }

        let req = CreateClipboardRequest {
            content_type: content_type.to_string(),
            content,
        };

        let record = self.clipboard_repo.create(&req).map_err(|e| e.to_string())?;

        if let Err(e) = self.clipboard_repo.add_tag_to_record(record.id, 1) {
            eprintln!("Failed to add default tag to record: {}", e);
        }

        if let Ok(Some(record_with_tags)) = self.clipboard_repo.get_by_id(record.id) {
            return Ok(Some(record_with_tags));
        }

        Ok(Some(record))
    }

    pub fn get_history(
        &self,
        page: u32,
        page_size: Option<u32>,
        tag_id: Option<i64>,
    ) -> Result<Vec<ClipboardRecord>, String> {
        let page_size = page_size.unwrap_or(30);
        self.clipboard_repo
            .list(page, page_size, tag_id)
            .map_err(|e| e.to_string())
    }

    pub fn toggle_pin(&self, id: i64) -> Result<bool, String> {
        let record = self
            .clipboard_repo
            .get_by_id(id)
            .map_err(|e| e.to_string())?
            .ok_or("Record not found")?;

        let new_pinned = !record.is_pinned;
        self.clipboard_repo
            .update_pinned(id, new_pinned)
            .map_err(|e| e.to_string())?;

        Ok(new_pinned)
    }

    pub fn update_record_tags(&self, id: i64, tag_ids: Vec<i64>) -> Result<(), String> {
        self.clipboard_repo
            .update_tags(id, &tag_ids)
            .map_err(|e| e.to_string())
    }

    pub fn delete_record(&self, id: i64) -> Result<(), String> {
        self.clipboard_repo.delete(id).map_err(|e| e.to_string())
    }

    pub fn get_record(&self, id: i64) -> Result<Option<ClipboardRecord>, String> {
        self.clipboard_repo.get_by_id(id).map_err(|e| e.to_string())
    }

    pub fn create_tag(&self, name: String, color: Option<String>) -> Result<Tag, String> {
        let color = color.unwrap_or_else(|| {
            self.tag_repo.get_next_color().unwrap_or_else(|_| "#6B7280".to_string())
        });

        let req = CreateTagRequest { name, color };
        self.tag_repo.create(&req).map_err(|e| e.to_string())
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>, String> {
        self.tag_repo.list_all().map_err(|e| e.to_string())
    }

    pub fn delete_tag(&self, id: i64) -> Result<(), String> {
        self.tag_repo.delete(id).map_err(|e| e.to_string())
    }

    pub fn update_tag(&self, id: i64, name: String, color: String) -> Result<(), String> {
        let req = crate::tag_db::UpdateTagRequest { name, color };
        self.tag_repo.update(id, &req).map_err(|e| e.to_string())
    }

    pub fn cleanup_old_records(&self, _max_count: u32, keep_days: u64) -> Result<u32, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let cutoff = now - (keep_days * 24 * 60 * 60 * 1000);

        let deleted = self
            .clipboard_repo
            .delete_older_than(cutoff, true)
            .map_err(|e| e.to_string())?;

        Ok(deleted)
    }

    pub fn cleanup_excess_count(&self, max_count: u32) -> Result<u32, String> {
        let deleted = self
            .clipboard_repo
            .delete_excess_count(max_count, true)
            .map_err(|e| e.to_string())?;

        Ok(deleted)
    }

}
