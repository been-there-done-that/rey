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

    fn make_jpeg_with_exif(fields: Vec<exif::Field>) -> Vec<u8> {
        let mut exif_bytes = Cursor::new(Vec::new());
        let mut writer = exif::experimental::Writer::new();
        for f in &fields {
            writer.push_field(f);
        }
        writer.write(&mut exif_bytes, true).unwrap();
        let exif_bytes = exif_bytes.into_inner();

        let mut jpeg = Vec::new();
        jpeg.extend_from_slice(&[0xFF, 0xD8]); // SOI
        let app1_len = (2 + 6 + exif_bytes.len()) as u16;
        jpeg.extend_from_slice(&[0xFF, 0xE1]); // APP1
        jpeg.extend_from_slice(&app1_len.to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&exif_bytes);
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI
        jpeg
    }

    #[test]
    fn test_extract_exif_reads_orientation() {
        let fields = vec![exif::Field {
            tag: exif::Tag::Orientation,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Short(vec![6]),
        }];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert_eq!(data.orientation, Some(6));
    }

    #[test]
    fn test_extract_exif_reads_make_and_model() {
        let fields = vec![
            exif::Field {
                tag: exif::Tag::Make,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"Canon".to_vec()]),
            },
            exif::Field {
                tag: exif::Tag::Model,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"EOS R5".to_vec()]),
            },
        ];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert_eq!(data.device_make, Some("Canon".to_string()));
        assert_eq!(data.device_model, Some("EOS R5".to_string()));
    }

    #[test]
    fn test_extract_exif_reads_gps_latitude() {
        let fields = vec![
            exif::Field {
                tag: exif::Tag::GPSLatitude,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Rational(vec![
                    exif::Rational { num: 35, denom: 1 },
                    exif::Rational { num: 30, denom: 1 },
                    exif::Rational { num: 0, denom: 1 },
                ]),
            },
            exif::Field {
                tag: exif::Tag::GPSLatitudeRef,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"N".to_vec()]),
            },
        ];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert_eq!(data.latitude, Some(35.5));
    }

    #[test]
    fn test_extract_exif_reads_gps_latitude_south() {
        let fields = vec![
            exif::Field {
                tag: exif::Tag::GPSLatitude,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Rational(vec![
                    exif::Rational { num: 33, denom: 1 },
                    exif::Rational { num: 51, denom: 1 },
                    exif::Rational { num: 0, denom: 1 },
                ]),
            },
            exif::Field {
                tag: exif::Tag::GPSLatitudeRef,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"S".to_vec()]),
            },
        ];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert_eq!(data.latitude, Some(-33.85));
    }

    #[test]
    fn test_extract_exif_reads_gps_longitude() {
        let fields = vec![
            exif::Field {
                tag: exif::Tag::GPSLongitude,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Rational(vec![
                    exif::Rational { num: 139, denom: 1 },
                    exif::Rational { num: 45, denom: 1 },
                    exif::Rational { num: 0, denom: 1 },
                ]),
            },
            exif::Field {
                tag: exif::Tag::GPSLongitudeRef,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"E".to_vec()]),
            },
        ];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert_eq!(data.longitude, Some(139.75));
    }

    #[test]
    fn test_extract_exif_reads_gps_longitude_west() {
        let fields = vec![
            exif::Field {
                tag: exif::Tag::GPSLongitude,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Rational(vec![
                    exif::Rational { num: 74, denom: 1 },
                    exif::Rational { num: 0, denom: 1 },
                    exif::Rational { num: 30, denom: 1 },
                ]),
            },
            exif::Field {
                tag: exif::Tag::GPSLongitudeRef,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"W".to_vec()]),
            },
        ];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert!((data.longitude.unwrap() - -74.00833333333333).abs() < 1e-10);
    }

    #[test]
    fn test_extract_exif_reads_datetime_original() {
        let fields = vec![exif::Field {
            tag: exif::Tag::DateTimeOriginal,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![b"2024:01:15 10:30:00".to_vec()]),
        }];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        let expected = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(10, 30, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        assert_eq!(data.taken_at, Some(expected));
    }

    #[test]
    fn test_extract_exif_reads_all_fields_together() {
        let fields = vec![
            exif::Field {
                tag: exif::Tag::Orientation,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Short(vec![3]),
            },
            exif::Field {
                tag: exif::Tag::Make,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"Nikon".to_vec()]),
            },
            exif::Field {
                tag: exif::Tag::Model,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"Z8".to_vec()]),
            },
            exif::Field {
                tag: exif::Tag::GPSLatitude,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Rational(vec![
                    exif::Rational { num: 48, denom: 1 },
                    exif::Rational { num: 51, denom: 1 },
                    exif::Rational { num: 24, denom: 1 },
                ]),
            },
            exif::Field {
                tag: exif::Tag::GPSLatitudeRef,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"N".to_vec()]),
            },
            exif::Field {
                tag: exif::Tag::GPSLongitude,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Rational(vec![
                    exif::Rational { num: 2, denom: 1 },
                    exif::Rational { num: 20, denom: 1 },
                    exif::Rational { num: 50, denom: 1 },
                ]),
            },
            exif::Field {
                tag: exif::Tag::GPSLongitudeRef,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"E".to_vec()]),
            },
            exif::Field {
                tag: exif::Tag::DateTimeOriginal,
                ifd_num: exif::In::PRIMARY,
                value: exif::Value::Ascii(vec![b"2025:06:20 14:22:10".to_vec()]),
            },
        ];
        let jpeg = make_jpeg_with_exif(fields);
        let data = extract_exif(&jpeg);
        assert_eq!(data.orientation, Some(3));
        assert_eq!(data.device_make, Some("Nikon".to_string()));
        assert_eq!(data.device_model, Some("Z8".to_string()));
        assert_eq!(data.latitude, Some(48.0 + 51.0 / 60.0 + 24.0 / 3600.0));
        assert_eq!(data.longitude, Some(2.0 + 20.0 / 60.0 + 50.0 / 3600.0));
        let expected_ts = chrono::NaiveDate::from_ymd_opt(2025, 6, 20)
            .unwrap()
            .and_hms_opt(14, 22, 10)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        assert_eq!(data.taken_at, Some(expected_ts));
    }

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
        img.write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Jpeg,
        )
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
