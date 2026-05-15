use image::DynamicImage;

pub fn resize_max_dimension(image: DynamicImage, max_px: u32) -> DynamicImage {
    let (w, h) = (image.width(), image.height());
    if w <= max_px && h <= max_px {
        return image;
    }

    let (new_w, new_h) = if w >= h {
        (max_px, (h as f64 * max_px as f64 / w as f64) as u32)
    } else {
        ((w as f64 * max_px as f64 / h as f64) as u32, max_px)
    };

    image.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_landscape_preserves_aspect_ratio() {
        let img = image::DynamicImage::new_rgb8(1920, 1080);
        let resized = resize_max_dimension(img, 720);
        assert!(resized.width() <= 720);
        assert!(resized.height() <= 720);
        assert_eq!(resized.width(), 720);
    }

    #[test]
    fn test_resize_portrait_preserves_aspect_ratio() {
        let img = image::DynamicImage::new_rgb8(1080, 1920);
        let resized = resize_max_dimension(img, 720);
        assert!(resized.width() <= 720);
        assert!(resized.height() <= 720);
        assert_eq!(resized.height(), 720);
    }

    #[test]
    fn test_resize_no_op_when_small_enough() {
        let img = image::DynamicImage::new_rgb8(100, 100);
        let resized = resize_max_dimension(img, 720);
        assert_eq!(resized.width(), 100);
        assert_eq!(resized.height(), 100);
    }

    #[test]
    fn test_resize_square() {
        let img = image::DynamicImage::new_rgb8(1000, 1000);
        let resized = resize_max_dimension(img, 720);
        assert_eq!(resized.width(), 720);
        assert_eq!(resized.height(), 720);
    }
}
