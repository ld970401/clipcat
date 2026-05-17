/// image_processor.rs — 图片缩略图生成
///
/// 职责：
/// - 将剪贴板中的 RGBA 原始图片数据缩放为缩略图（最大边 200px）
/// - 使用 Lanczos3 算法进行高质量下采样
/// - 输出为 PNG 格式的字节数据，存入数据库
use image::{ImageBuffer, RgbaImage, ImageFormat};
use std::io::Cursor;

/// 缩略图最大边长（像素），超过此尺寸的图片将等比缩放
const THUMBNAIL_MAX_SIZE: u32 = 200;

/// 图片处理器（无状态工具结构体）
pub struct ImageProcessor;

impl ImageProcessor {
    /// 根据 RGBA 原始数据生成缩略图
    ///
    /// # 参数
    /// - `rgba`: 原始 RGBA 像素数据（每像素 4 字节）
    /// - `width`: 原始图片宽度
    /// - `height`: 原始图片高度
    ///
    /// # 返回
    /// PNG 格式的缩略图字节数据
    ///
    /// # 流程
    /// 1. 将 RGBA 字节重建为 ImageBuffer
    /// 2. 计算缩放后尺寸（保持宽高比，最大边不超过 THUMBNAIL_MAX_SIZE）
    /// 3. 使用 Lanczos3 滤波器进行缩放
    /// 4. 编码为 PNG 输出
    pub fn generate_thumbnail(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
        let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
            .ok_or_else(|| "Failed to create image buffer".to_string())?;

        let (new_width, new_height) = Self::calculate_thumbnail_size(width, height);

        let thumbnail = image::imageops::resize(
            &img,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );

        let mut buf = Cursor::new(Vec::new());
        thumbnail.write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| e.to_string())?;

        Ok(buf.into_inner())
    }

    /// 计算缩略图尺寸，保持宽高比
    ///
    /// - 若原图最大边 <= THUMBNAIL_MAX_SIZE，直接返回原始尺寸（无需缩放）
    /// - 否则按比例缩放，使最大边恰好等于 THUMBNAIL_MAX_SIZE
    pub fn calculate_thumbnail_size(width: u32, height: u32) -> (u32, u32) {
        let max_dim = width.max(height);
        if max_dim <= THUMBNAIL_MAX_SIZE {
            return (width, height);
        }
        let ratio = THUMBNAIL_MAX_SIZE as f32 / max_dim as f32;
        ((width as f32 * ratio) as u32, (height as f32 * ratio) as u32)
    }
}
