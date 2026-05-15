use image::DynamicImage;
use image::imageops;

pub fn apply_orientation(image: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        1 => image,
        2 => DynamicImage::ImageRgba8(imageops::flip_horizontal(&image)),
        3 => DynamicImage::ImageRgba8(imageops::rotate180(&image)),
        4 => DynamicImage::ImageRgba8(imageops::flip_vertical(&image)),
        5 => {
            let rotated = imageops::rotate90(&image);
            DynamicImage::ImageRgba8(imageops::flip_horizontal(&rotated))
        }
        6 => DynamicImage::ImageRgba8(imageops::rotate90(&image)),
        7 => {
            let rotated = imageops::rotate270(&image);
            DynamicImage::ImageRgba8(imageops::flip_horizontal(&rotated))
        }
        8 => DynamicImage::ImageRgba8(imageops::rotate270(&image)),
        _ => image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orientation_1_no_change() {
        let img = image::DynamicImage::new_rgb8(10, 20);
        let result = apply_orientation(img.clone(), 1);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }

    #[test]
    fn test_orientation_3_rotate_180() {
        let img = image::DynamicImage::new_rgb8(10, 20);
        let result = apply_orientation(img, 3);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }

    #[test]
    fn test_orientation_6_rotate_90_cw() {
        let img = image::DynamicImage::new_rgb8(100, 50);
        let result = apply_orientation(img, 6);
        assert_eq!(result.width(), 50);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_orientation_8_rotate_90_ccw() {
        let img = image::DynamicImage::new_rgb8(100, 50);
        let result = apply_orientation(img, 8);
        assert_eq!(result.width(), 50);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_orientation_invalid_returns_original() {
        let img = image::DynamicImage::new_rgb8(10, 20);
        let result = apply_orientation(img.clone(), 99);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }
}
