use image::{ImageBuffer, RgbaImage, ImageFormat};
use std::io::Cursor;

const THUMBNAIL_MAX_SIZE: u32 = 200;

pub struct ImageProcessor;

impl ImageProcessor {
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

    pub fn calculate_thumbnail_size(width: u32, height: u32) -> (u32, u32) {
        let max_dim = width.max(height);
        if max_dim <= THUMBNAIL_MAX_SIZE {
            return (width, height);
        }
        let ratio = THUMBNAIL_MAX_SIZE as f32 / max_dim as f32;
        ((width as f32 * ratio) as u32, (height as f32 * ratio) as u32)
    }
}