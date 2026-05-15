# Task 6: Implement `crates/image` — Layer 1 Image Decoding and EXIF

## Wave
2 (Layer 1 — Pure Libraries)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete

## Can Run In Parallel With
- Task 5 (crypto crate) — no dependencies between image and crypto

## Design References
- STRUCTURE.md §2.3: image — Image Decode/Encode
- design.md §2.3: Compilation Guarantees (image depends only on types, NO crypto)
- SPEC.md §3.1: Thumbnail Generation (image resize specs)

## Requirements
25.3, 26, 26.1–26.4, 10.1, 10.2, 10.5

## Objective
Implement image decode/encode, resize, EXIF extraction, and orientation correction. No crypto. No I/O beyond reading bytes from slices.

## Cargo.toml
```toml
[package]
name = "image"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
types = { workspace = true }
common = { workspace = true }
image = { workspace = true }  # image-rs crate
kamadak-exif = { workspace = true }
thiserror = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod decode;
pub mod encode;
pub mod resize;
pub mod exif;
pub mod orientation;
pub mod error;

pub use decode::decode_image;
pub use encode::encode_jpeg;
pub use resize::resize_max_dimension;
pub use exif::extract_exif;
pub use orientation::apply_orientation;
pub use error::ImageError;
```

### `src/error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum ImageError {
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("failed to decode image: {0}")]
    DecodeError(String),
    #[error("failed to parse EXIF: {0}")]
    ExifError(String),
}
```

### `src/decode.rs`
Implement `decode_image(source: &[u8], mime_type: &str) -> Result<DynamicImage, ImageError>`:
- Support JPEG, PNG, WebP formats via `image::ImageReader`
- For HEIC: return `ImageError::UnsupportedFormat` (HEIC requires additional codec)
- Use `image::guess_format(source)` as fallback when mime_type is unknown
- Return `ImageError::UnsupportedFormat` for unknown types

```rust
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
```

### `src/encode.rs`
Implement `encode_jpeg(image: &DynamicImage, quality: u8) -> Vec<u8>`:
- Use `image::ImageFormat::Jpeg` encoder
- quality: 1–100
- Return raw JPEG bytes

```rust
use image::DynamicImage;
use std::io::Cursor;

pub fn encode_jpeg(image: &DynamicImage, quality: u8) -> Vec<u8> {
    let quality = quality.clamp(1, 100);
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    image
        .write_to(&mut cursor, image::ImageFormat::Jpeg)
        .expect("JPEG encoding should not fail");
    buf
}
```

### `src/resize.rs`
Implement `resize_max_dimension(image: DynamicImage, max_px: u32) -> DynamicImage`:
- If both width and height ≤ max_px, return original
- Otherwise, scale to fit within max_px while preserving aspect ratio
- Use `image::imageops::resize` with `image::imageops::FilterType::Lanczos3`

```rust
use image::DynamicImage;

pub fn resize_max_dimension(image: DynamicImage, max_px: u32) -> DynamicImage {
    let (w, h) = (image.width(), image.height());
    if w <= max_px && h <= max_px {
        return image;
    }

    let ratio = (w as f64 / h as f64).min(h as f64 / w as f64);
    let (new_w, new_h) = if w >= h {
        (max_px, (h as f64 * max_px as f64 / w as f64) as u32)
    } else {
        ((w as f64 * max_px as f64 / h as f64) as u32, max_px)
    };

    image.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
}
```

### `src/exif.rs`
Implement `extract_exif(source: &[u8]) -> ExifData`:
- `ExifData` struct: `latitude: Option<f64>`, `longitude: Option<f64>`, `taken_at: Option<i64>` (Unix ms), `device_make: Option<String>`, `device_model: Option<String>`, `orientation: Option<u16>`
- Use `kamadak-exif` to parse EXIF from JPEG/PNG/TIFF
- GPS: convert DMS (degrees/minutes/seconds) to decimal degrees
- taken_at: parse DateTimeOriginal to Unix milliseconds
- Return partial result with `None` fields when EXIF is absent or malformed — NO error
- Missing EXIF is normal, not an error condition

```rust
use exif::Reader;
use std::io::Cursor;

pub struct ExifData {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub taken_at: Option<i64>,
    pub device_make: Option<String>,
    pub device_model: Option<String>,
    pub orientation: Option<u16>,
}

impl ExifData {
    pub fn empty() -> Self {
        Self {
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            orientation: None,
        }
    }
}

fn dms_to_decimal(degrees: f64, minutes: f64, seconds: f64, ref_char: &str) -> f64 {
    let decimal = degrees + minutes / 60.0 + seconds / 3600.0;
    if ref_char == "S" || ref_char == "W" {
        -decimal
    } else {
        decimal
    }
}

pub fn extract_exif(source: &[u8]) -> ExifData {
    let mut data = ExifData::empty();

    let reader = Reader::new();
    let mut cursor = Cursor::new(source);
    let exif = match reader.read_from_container(&mut cursor) {
        Ok(e) => e,
        Err(_) => return data,
    };

    // Orientation (tag 0x0112)
    if let Some(orientation) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        if let Ok(val) = orientation.value.get_uint(0) {
            data.orientation = Some(val as u16);
        }
    }

    // GPS Latitude
    if let Some(lat) = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY) {
        if let Some(lat_ref) = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY) {
            let ref_str = lat_ref.value.display_as(exif::DisplayHint::Ascii).to_string();
            let d = lat.value.get_rational(0).map(|r| r.to_f64()).unwrap_or(0.0);
            let m = lat.value.get_rational(1).map(|r| r.to_f64()).unwrap_or(0.0);
            let s = lat.value.get_rational(2).map(|r| r.to_f64()).unwrap_or(0.0);
            data.latitude = Some(dms_to_decimal(d, m, s, &ref_str));
        }
    }

    // GPS Longitude
    if let Some(lon) = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY) {
        if let Some(lon_ref) = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY) {
            let ref_str = lon_ref.value.display_as(exif::DisplayHint::Ascii).to_string();
            let d = lon.value.get_rational(0).map(|r| r.to_f64()).unwrap_or(0.0);
            let m = lon.value.get_rational(1).map(|r| r.to_f64()).unwrap_or(0.0);
            let s = lon.value.get_rational(2).map(|r| r.to_f64()).unwrap_or(0.0);
            data.longitude = Some(dms_to_decimal(d, m, s, &ref_str));
        }
    }

    // DateTimeOriginal
    if let Some(dt) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        let dt_str = dt.value.display_as(exif::DisplayHint::Ascii).to_string();
        if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y:%m:%d %H:%M:%S") {
            data.taken_at = Some(parsed.and_utc().timestamp_millis());
        }
    }

    // Device Make
    if let Some(make) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
        data.device_make = Some(make.value.display_as(exif::DisplayHint::Ascii).to_string());
    }

    // Device Model
    if let Some(model) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        data.device_model = Some(model.value.display_as(exif::DisplayHint::Ascii).to_string());
    }

    data
}
```

### `src/orientation.rs`
Implement `apply_orientation(image: DynamicImage, orientation: u16) -> DynamicImage`:
- Apply all 8 EXIF orientation corrections:
  - 1: no change
  - 2: flip horizontal
  - 3: rotate 180°
  - 4: flip vertical
  - 5: rotate 90° CW + flip horizontal
  - 6: rotate 90° CW
  - 7: rotate 90° CCW + flip horizontal
  - 8: rotate 90° CCW
- Use `image::imageops` operations: `flip_horizontal`, `flip_vertical`, `rotate90`, `rotate180`, `rotate270`

```rust
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
```

## Tests (Task 6.7 — marked with *)
Write unit tests with fixture images:
- JPEG decode succeeds on a valid JPEG fixture
- PNG decode succeeds on a valid PNG fixture
- WebP decode succeeds on a valid WebP fixture
- EXIF GPS extraction from fixture with known coordinates
- EXIF orientation correction for all 8 orientations (use test images with known orientation tags)
- Resize to max 720px preserves aspect ratio (verify width ≤ 720 and height ≤ 720)
- Missing EXIF returns `ExifData` with all `None` fields — no error
- Unsupported format (e.g., BMP) returns `ImageError::UnsupportedFormat`

## Verification Steps
- [ ] `cargo check -p image` succeeds
- [ ] `cargo test -p image` passes
- [ ] NO `crypto` dependency in `Cargo.toml`
- [ ] `cargo tree -p image` shows no crypto-related crates
- [ ] EXIF extraction returns partial results without error when EXIF is missing
- [ ] All 8 orientation corrections produce correct output
- [ ] Resize preserves aspect ratio (verify with known-dimension test images)

## Notes
- The `image` crate (image-rs) is a heavy dependency but provides comprehensive format support.
- HEIC/HEIF support requires `libheif` — defer to v2. Return `UnsupportedFormat` for now.
- EXIF parsing: `kamadak-exif` is a pure-Rust EXIF parser that works with image-rs.
- GPS coordinates in EXIF are stored as rational numbers (degrees/minutes/seconds) — convert to decimal.
- The `orientation` tag is EXIF tag 0x0112.
