use std::fs::File;
use std::io::Read;
use std::path::Path;

use image::ImageReader;
use image_hasher::{HashAlg, HasherConfig};
use photo_core::AppError;

const MMAP_THRESHOLD: u64 = 64 * 1024;

pub fn blake3_file(path: &Path) -> Result<String, AppError> {
    let metadata = std::fs::metadata(path).map_err(AppError::Io)?;
    let size = metadata.len();

    if size > MMAP_THRESHOLD {
        let file = File::open(path).map_err(AppError::Io)?;
        let mmap = unsafe { memmap2::Mmap::map(&file).map_err(AppError::Io)? };
        Ok(blake3::hash(&mmap).to_hex().to_string())
    } else {
        let mut file = File::open(path).map_err(AppError::Io)?;
        let mut buffer = Vec::with_capacity(size as usize);
        file.read_to_end(&mut buffer).map_err(AppError::Io)?;
        Ok(blake3::hash(&buffer).to_hex().to_string())
    }
}

pub fn perceptual_hashes(path: &Path) -> Result<(u64, u64), AppError> {
    let img = ImageReader::open(path)
        .map_err(|e| AppError::Image(e.to_string()))?
        .decode()
        .map_err(|e| AppError::Image(e.to_string()))?;

    let dhasher = HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher();
    let phasher = HasherConfig::new()
        .hash_alg(HashAlg::Median)
        .preproc_dct()
        .to_hasher();

    let dhash = dhasher.hash_image(&img);
    let phash = phasher.hash_image(&img);

    Ok((hash_to_u64(&dhash), hash_to_u64(&phash)))
}

pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

fn hash_to_u64(hash: &image_hasher::ImageHash) -> u64 {
    let bytes = hash.as_bytes();
    let mut value = 0u64;
    for (i, byte) in bytes.iter().take(8).enumerate() {
        value |= (*byte as u64) << (i * 8);
    }
    value
}

pub fn image_dimensions(path: &Path) -> Result<(Option<u32>, Option<u32>), AppError> {
    let reader = ImageReader::open(path).map_err(|e| AppError::Image(e.to_string()))?;
    let dimensions = reader
        .into_dimensions()
        .map_err(|e| AppError::Image(e.to_string()))?;
    Ok((Some(dimensions.0), Some(dimensions.1)))
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "tiff" | "tif" | "heic" | "heif"
            )
        })
        .unwrap_or(false)
}
