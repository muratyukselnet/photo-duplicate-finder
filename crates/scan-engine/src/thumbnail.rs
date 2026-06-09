use std::fs;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{ImageReader, RgbaImage};
use photo_core::AppError;

use crate::paths;

const THUMB_MAX_EDGE: u32 = 256;

pub fn ensure_thumbnail(source: &Path, cache_key: &str) -> Result<PathBuf, AppError> {
    let dest = paths::thumbnail_cache_dir().join(format!("{cache_key}.webp"));
    if dest.exists() {
        return Ok(dest);
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(AppError::Io)?;
    }

    let img = ImageReader::open(source)
        .map_err(|e| AppError::Image(e.to_string()))?
        .decode()
        .map_err(|e| AppError::Image(e.to_string()))?;

    let thumb = resize_max_edge(&img.to_rgba8(), img.width(), img.height());
    thumb
        .save_with_format(&dest, image::ImageFormat::WebP)
        .map_err(|e| AppError::Image(e.to_string()))?;

    Ok(dest)
}

fn resize_max_edge(rgba: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (nw, nh) = if width >= height {
        let nw = THUMB_MAX_EDGE;
        let nh = ((height as f64 / width as f64) * nw as f64).round() as u32;
        (nw, nh.max(1))
    } else {
        let nh = THUMB_MAX_EDGE;
        let nw = ((width as f64 / height as f64) * nh as f64).round() as u32;
        (nw.max(1), nh)
    };

    image::imageops::resize(rgba, nw, nh, FilterType::Triangle)
}
