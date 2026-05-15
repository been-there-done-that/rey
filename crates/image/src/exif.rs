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

fn extract_ascii(field: &exif::Field) -> Option<String> {
    match &field.value {
        exif::Value::Ascii(vecs) => vecs
            .first()
            .map(|bytes| String::from_utf8_lossy(bytes).trim().to_string()),
        _ => None,
    }
}

fn extract_rational(field: &exif::Field, index: usize) -> Option<f64> {
    match &field.value {
        exif::Value::Rational(vec) => vec.get(index).map(|r| r.to_f64()),
        _ => None,
    }
}

fn extract_gps_dms(field: &exif::Field) -> Option<f64> {
    let d = extract_rational(field, 0)?;
    let m = extract_rational(field, 1)?;
    let s = extract_rational(field, 2)?;
    Some(d + m / 60.0 + s / 3600.0)
}

pub fn extract_exif(source: &[u8]) -> ExifData {
    let mut data = ExifData::empty();

    let reader = Reader::new();
    let mut cursor = Cursor::new(source);
    let exif = match reader.read_from_container(&mut cursor) {
        Ok(e) => e,
        Err(_) => return data,
    };

    if let Some(orientation) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        if let Some(val) = orientation.value.get_uint(0) {
            data.orientation = Some(val as u16);
        }
    }

    if let Some(lat) = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY) {
        if let Some(lat_ref) = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY) {
            if let (Some(decimal), Some(ref_str)) = (extract_gps_dms(lat), extract_ascii(lat_ref)) {
                let sign = if ref_str.starts_with('S') { -1.0 } else { 1.0 };
                data.latitude = Some(decimal * sign);
            }
        }
    }

    if let Some(lon) = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY) {
        if let Some(lon_ref) = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY) {
            if let (Some(decimal), Some(ref_str)) = (extract_gps_dms(lon), extract_ascii(lon_ref)) {
                let sign = if ref_str.starts_with('W') { -1.0 } else { 1.0 };
                data.longitude = Some(decimal * sign);
            }
        }
    }

    if let Some(dt) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        if let Some(dt_str) = extract_ascii(dt) {
            if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y:%m:%d %H:%M:%S")
            {
                data.taken_at = Some(parsed.and_utc().timestamp_millis());
            }
        }
    }

    if let Some(make) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
        data.device_make = extract_ascii(make);
    }

    if let Some(model) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        data.device_model = extract_ascii(model);
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_exif_no_exif_returns_empty() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(10, 10);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let data = extract_exif(&buf);
        assert!(data.latitude.is_none());
        assert!(data.longitude.is_none());
        assert!(data.taken_at.is_none());
        assert!(data.device_make.is_none());
        assert!(data.device_model.is_none());
        assert!(data.orientation.is_none());
    }

    #[test]
    fn test_extract_exif_empty_bytes_returns_empty() {
        let data = extract_exif(&[]);
        assert!(data.latitude.is_none());
        assert!(data.longitude.is_none());
        assert!(data.taken_at.is_none());
        assert!(data.device_make.is_none());
        assert!(data.device_model.is_none());
        assert!(data.orientation.is_none());
    }

    #[test]
    fn test_extract_exif_invalid_bytes_returns_empty() {
        let data = extract_exif(b"not a valid image at all");
        assert!(data.latitude.is_none());
        assert!(data.longitude.is_none());
        assert!(data.taken_at.is_none());
        assert!(data.device_make.is_none());
        assert!(data.device_model.is_none());
        assert!(data.orientation.is_none());
    }

    #[test]
    fn test_extract_exif_jpeg_without_exif() {
        let mut buf = Vec::new();
        let img = image::DynamicImage::new_rgb8(100, 100);
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .unwrap();
        let data = extract_exif(&buf);
        assert!(data.orientation.is_none());
        assert!(data.latitude.is_none());
        assert!(data.taken_at.is_none());
    }

    #[test]
    fn test_exif_data_empty_constructor() {
        let data = ExifData::empty();
        assert!(data.latitude.is_none());
        assert!(data.longitude.is_none());
        assert!(data.taken_at.is_none());
        assert!(data.device_make.is_none());
        assert!(data.device_model.is_none());
        assert!(data.orientation.is_none());
    }
}
