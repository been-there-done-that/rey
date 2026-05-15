use types::crypto::{Header24, Key256};
use crate::error::ThumbnailError;

const MAX_DIMENSION: u32 = 720;
const MAX_SIZE_BYTES: usize = 100 * 1024;
const INITIAL_QUALITY: u8 = 85;
const MIN_QUALITY: u8 = 10;
const QUALITY_STEP: u8 = 10;

pub fn generate_thumbnail(
    source: &[u8],
    mime_type: &str,
    file_key: &Key256,
) -> Result<(Header24, Vec<u8>), ThumbnailError> {
    if mime_type.starts_with("video/") {
        return Err(ThumbnailError::UnsupportedFormat);
    }

    let img = rey_image::decode_image(source, mime_type)
        .map_err(|_| ThumbnailError::UnsupportedFormat)?;

    let exif = rey_image::extract_exif(source);
    let orientation = exif.orientation.unwrap_or(1);

    let img = rey_image::apply_orientation(img, orientation);
    let img = rey_image::resize_max_dimension(img, MAX_DIMENSION);

    let mut quality = INITIAL_QUALITY;
    let mut jpeg_bytes = rey_image::encode_jpeg(&img, quality);

    while jpeg_bytes.len() > MAX_SIZE_BYTES && quality > MIN_QUALITY {
        quality = quality.saturating_sub(QUALITY_STEP);
        if quality < MIN_QUALITY {
            quality = MIN_QUALITY;
        }
        jpeg_bytes = rey_image::encode_jpeg(&img, quality);
    }

    let (header, ciphertext) = crypto::stream_encrypt(&jpeg_bytes, file_key);

    Ok((header, ciphertext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;

    #[test]
    fn thumbnail_from_jpeg_is_within_specs() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(1920, 1080);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg).unwrap();
        let key = generate_key();
        let (_, ciphertext) = generate_thumbnail(&buf, "image/jpeg", &key).unwrap();
        assert!(ciphertext.len() <= MAX_SIZE_BYTES + 16);
    }

    #[test]
    fn video_mime_type_returns_unsupported() {
        let source = b"fake video data";
        let key = generate_key();
        let result = generate_thumbnail(source, "video/mp4", &key);
        assert!(matches!(result, Err(ThumbnailError::UnsupportedFormat)));
    }

    #[test]
    fn unsupported_format_returns_error() {
        let source = b"not an image";
        let key = generate_key();
        let result = generate_thumbnail(source, "image/bmp", &key);
        assert!(matches!(result, Err(ThumbnailError::UnsupportedFormat)));
    }

    #[test]
    fn small_image_no_resize_needed() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(100, 100);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg).unwrap();
        let key = generate_key();
        let (_, ciphertext) = generate_thumbnail(&buf, "image/jpeg", &key).unwrap();
        assert!(!ciphertext.is_empty());
    }
}
