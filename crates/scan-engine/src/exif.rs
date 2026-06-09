use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use chrono::{DateTime, NaiveDateTime, Utc};
use exif::{In, Tag, Value};
use photo_core::ExifData;
use serde_json;

pub fn read_exif(path: &Path) -> Option<ExifData> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);
    let exif = exif::Reader::new()
        .read_from_container(&mut bufreader)
        .ok()?;

    let camera_make = get_string(&exif, Tag::Make);
    let camera_model = get_string(&exif, Tag::Model);
    let iso = get_u32(&exif, Tag::PhotographicSensitivity);
    let aperture = get_rational(&exif, Tag::FNumber).map(|v| format!("f/{v:.1}"));
    let shutter_speed = get_rational(&exif, Tag::ExposureTime).map(format_shutter);
    let focal_length = get_rational(&exif, Tag::FocalLength).map(|v| format!("{v:.0}mm"));
    let date_taken = get_datetime(&exif, Tag::DateTimeOriginal)
        .or_else(|| get_datetime(&exif, Tag::DateTime));

    Some(ExifData {
        camera_make,
        camera_model,
        iso,
        aperture,
        shutter_speed,
        focal_length,
        date_taken,
    })
}

pub fn exif_to_json(path: &Path) -> Option<String> {
    read_exif(path).and_then(|e| serde_json::to_string(&e).ok())
}

fn get_string(exif: &exif::Exif, tag: Tag) -> Option<String> {
    match exif.get_field(tag, In::PRIMARY)?.value {
        Value::Ascii(ref vec) => vec.first().map(|bytes| {
            String::from_utf8_lossy(bytes)
                .trim_matches('\0')
                .trim()
                .to_string()
        }),
        _ => None,
    }
}

fn get_u32(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Short(v) => v.first().map(|value| *value as u32),
        Value::Long(v) => v.first().copied(),
        _ => None,
    }
}

fn get_rational(exif: &exif::Exif, tag: Tag) -> Option<f64> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(v) if !v.is_empty() => Some(v[0].to_f64()),
        _ => None,
    }
}

fn get_datetime(exif: &exif::Exif, tag: Tag) -> Option<DateTime<Utc>> {
    let raw = get_string(exif, tag)?;
    NaiveDateTime::parse_from_str(&raw, "%Y:%m:%d %H:%M:%S")
        .ok()
        .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
}

fn format_shutter(value: f64) -> String {
    if value >= 1.0 {
        format!("{value:.1}s")
    } else {
        format!("1/{}", (1.0 / value).round() as u32)
    }
}
