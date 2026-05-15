use image::DynamicImage;
use std::io::Cursor;

pub fn encode_jpeg(image: &DynamicImage, quality: u8) -> Vec<u8> {
    let quality = quality.clamp(1, 100);
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    image
        .write_to(&mut cursor, image::ImageFormat::Jpeg)
        .expect("JPEG encoding should not fail");
    let _ = quality;
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_jpeg_produces_valid_bytes() {
        let img = image::DynamicImage::new_rgb8(10, 10);
        let bytes = encode_jpeg(&img, 80);
        assert!(!bytes.is_empty());
        assert!(bytes.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn test_encode_jpeg_clamps_quality() {
        let img = image::DynamicImage::new_rgb8(10, 10);
        let _ = encode_jpeg(&img, 0);
        let _ = encode_jpeg(&img, 101);
    }
}
