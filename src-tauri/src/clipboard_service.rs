/// clipboard_service.rs — 剪切板业务逻辑层（Service Layer）
///
/// 职责：
/// - 作为 ClipboardManager（监听层）与 ClipboardRepository（数据层）之间的中间层
/// - 剪贴板变更时的去重判断（对比最新记录的哈希值）
/// - 新记录自动分配默认标签（id=1）
/// - 历史记录查询、置顶切换、标签管理、记录清理等业务操作
///
/// 依赖关系：
/// ClipboardManager → ClipboardService → ClipboardRepository / TagRepository
use crate::clipboard_db::{ClipboardContent, ClipboardRecord, ClipboardRepository, CreateClipboardRequest};
use crate::database::Database;
use crate::tag_db::{CreateTagRequest, Tag, TagRepository};
use std::sync::Arc;

/// 剪切板业务服务，组合 ClipboardRepository 和 TagRepository
pub struct ClipboardService {
    /// 剪切板数据仓库（pub 暴露给 paste.rs 中 paste_item 命令直接使用）
    pub clipboard_repo: Arc<ClipboardRepository>,
    /// 标签数据仓库
    tag_repo: Arc<TagRepository>,
}

impl ClipboardService {
    /// 创建 ClipboardService 实例
    ///
    /// 初始化时会确保默认标签（id=1, "Clipboard"）存在
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

    /// 剪贴板内容变更回调
    ///
    /// 核心去重逻辑：
    /// 1. 计算新内容的哈希值
    /// 2. 查询数据库中最新记录的哈希值
    /// 3. 若哈希相同则判定为重复内容，返回 Ok(None)
    /// 4. 若哈希不同则创建新记录，并自动分配默认标签（id=1）
    ///
    /// # 返回
    /// - `Ok(Some(record))`: 新记录已创建
    /// - `Ok(None)`: 内容重复，未创建记录
    /// - `Err(...)`: 操作失败
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

        // 与数据库中最新记录的哈希比较，实现去重
        let last_hash = if content_type == "text" {
            self.clipboard_repo.get_last_text_hash().map_err(|e| e.to_string())?
        } else {
            self.clipboard_repo.get_last_image_hash().map_err(|e| e.to_string())?
        };

        if last_hash == Some(hash) {
            return Ok(None);
        }

        // 创建新记录
        let req = CreateClipboardRequest {
            content_type: content_type.to_string(),
            content,
        };

        let record = self.clipboard_repo.create(&req).map_err(|e| e.to_string())?;

        // 自动分配默认标签（id=1）
        if let Err(e) = self.clipboard_repo.add_tag_to_record(record.id, 1) {
            eprintln!("Failed to add default tag to record: {}", e);
        }

        // 重新查询以获取完整的标签信息
        if let Ok(Some(record_with_tags)) = self.clipboard_repo.get_by_id(record.id) {
            return Ok(Some(record_with_tags));
        }

        Ok(Some(record))
    }

    /// 分页查询剪切板历史
    ///
    /// - `page`: 页码（从 0 开始）
    /// - `page_size`: 每页条数，默认 30
    /// - `tag_id`: 可选标签过滤
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

    /// 切换记录置顶状态
    ///
    /// 返回切换后的新状态（true = 已置顶）
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

    /// 更新记录的标签关联（全量替换）
    pub fn update_record_tags(&self, id: i64, tag_ids: Vec<i64>) -> Result<(), String> {
        self.clipboard_repo
            .update_tags(id, &tag_ids)
            .map_err(|e| e.to_string())
    }

    /// 删除记录
    pub fn delete_record(&self, id: i64) -> Result<(), String> {
        self.clipboard_repo.delete(id).map_err(|e| e.to_string())
    }

    /// 查询单条记录
    pub fn get_record(&self, id: i64) -> Result<Option<ClipboardRecord>, String> {
        self.clipboard_repo.get_by_id(id).map_err(|e| e.to_string())
    }

    /// 创建标签
    ///
    /// 若未指定颜色，自动从 6 色轮转中分配下一个颜色
    pub fn create_tag(&self, name: String, color: Option<String>) -> Result<Tag, String> {
        let color = color.unwrap_or_else(|| {
            self.tag_repo.get_next_color().unwrap_or_else(|_| "#6B7280".to_string())
        });

        let req = CreateTagRequest { name, color };
        self.tag_repo.create(&req).map_err(|e| e.to_string())
    }

    /// 查询所有标签
    pub fn list_tags(&self) -> Result<Vec<Tag>, String> {
        self.tag_repo.list_all().map_err(|e| e.to_string())
    }

    /// 删除标签
    pub fn delete_tag(&self, id: i64) -> Result<(), String> {
        self.tag_repo.delete(id).map_err(|e| e.to_string())
    }

    /// 更新标签名称和颜色
    pub fn update_tag(&self, id: i64, name: String, color: String) -> Result<(), String> {
        let req = crate::tag_db::UpdateTagRequest { name, color };
        self.tag_repo.update(id, &req).map_err(|e| e.to_string())
    }

    /// 按保留天数清理旧记录
    ///
    /// 计算截止时间戳 = 当前时间 - keep_days 天，删除早于该时间且未置顶的记录
    /// 返回被删除的记录数
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

    /// 按保留数量清理多余记录
    ///
    /// 保留最新的 max_count 条记录（置顶记录不受限制），删除其余记录
    /// 返回被删除的记录数
    pub fn cleanup_excess_count(&self, max_count: u32) -> Result<u32, String> {
        let deleted = self
            .clipboard_repo
            .delete_excess_count(max_count, true)
            .map_err(|e| e.to_string())?;

        Ok(deleted)
    }

}
