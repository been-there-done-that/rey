use image::DynamicImage;
use crate::error::ImageError;

pub fn decode_image(source: &[u8], mime_type: &str) -> Result<DynamicImage, ImageError> {
    let format = match mime_type {
        "image/jpeg" | "image/jpg" => image::ImageFormat::Jpeg,
        "image/png" => image::ImageFormat::Png,
        "image/webp" => image::ImageFormat::WebP,
        "image/heic" | "image/heif" => return Err(ImageError::UnsupportedFormat),
        _ => image::guess_format(source).map_err(|_| ImageError::UnsupportedFormat)?,
    };

    let cursor = std::io::Cursor::new(source);
    let reader = image::ImageReader::with_format(cursor, format);
    reader.decode().map_err(|e| ImageError::DecodeError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_jpeg() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(10, 10);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg).unwrap();
        let decoded = decode_image(&buf, "image/jpeg").unwrap();
        assert_eq!(decoded.width(), 10);
        assert_eq!(decoded.height(), 10);
    }

    #[test]
    fn test_decode_png() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(20, 15);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        let decoded = decode_image(&buf, "image/png").unwrap();
        assert_eq!(decoded.width(), 20);
        assert_eq!(decoded.height(), 15);
    }

    #[test]
    fn test_decode_webp() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(8, 8);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::WebP).unwrap();
        let decoded = decode_image(&buf, "image/webp").unwrap();
        assert_eq!(decoded.width(), 8);
        assert_eq!(decoded.height(), 8);
    }

    #[test]
    fn test_decode_heic_unsupported() {
        let data = b"fake heic data";
        let result = decode_image(data, "image/heic");
        assert!(matches!(result, Err(ImageError::UnsupportedFormat)));
    }

    #[test]
    fn test_decode_unknown_format() {
        let data = b"not an image";
        let result = decode_image(data, "image/bmp");
        assert!(matches!(result, Err(ImageError::UnsupportedFormat)));
    }
}
