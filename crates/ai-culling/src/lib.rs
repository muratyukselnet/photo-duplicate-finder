use photo_core::{AiMember, AiVerdict, DuplicateGroupDetail, DuplicateKind};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub preset: Preset,
    pub tier1_enabled: bool,
    pub protected_folders: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            preset: Preset::Balanced,
            tier1_enabled: true,
            protected_folders: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Conservative,
    Balanced,
    Aggressive,
}

pub fn settings_path() -> std::path::PathBuf {
    scan_engine::paths::app_data_dir().join("ai-settings.json")
}
pub fn load_settings() -> Result<Settings, String> {
    match std::fs::read(settings_path()) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid review settings: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn save_settings(settings: &Settings) -> Result<(), String> {
    scan_engine::paths::ensure_app_dirs().map_err(|e| e.to_string())?;
    let path = settings_path();
    let temp = path.with_extension("json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}

/// Content and extractor identity survive rescans and file moves. Never cache a
/// decode if the source changed while the helper was reading it.
pub fn cached_features<F>(
    db: &scan_engine::db::Database,
    path: &Path,
    hash: &str,
    version: &str,
    analyze: F,
) -> Result<Option<serde_json::Value>, String>
where
    F: FnOnce(&Path) -> Result<serde_json::Value, photo_core::AppError>,
{
    if let Some(value) = db
        .get_ai_features(hash, version)
        .map_err(|e| e.to_string())?
        .and_then(|v| serde_json::from_str(&v).ok())
    {
        return Ok(Some(value));
    }
    let Ok(value) = analyze(path) else {
        return Ok(None);
    };
    if scan_engine::hash::blake3_file(path).ok().as_deref() != Some(hash) {
        return Ok(None);
    }
    db.put_ai_features(hash, version, &value.to_string())
        .map_err(|e| e.to_string())?;
    Ok(Some(value))
}

// Results carry the file hashes observed during review. Protection is deliberately
// absent: it is re-read whenever a result is shown, including for identical files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResult {
    pub hashes: HashMap<i64, String>,
    pub bucket: String,
    pub verdict: AiVerdict,
    pub decisions: HashMap<i64, AiMember>,
    #[serde(default)]
    pub features: HashMap<i64, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct CachedVerdict {
    pub bucket: String,
    pub verdict: AiVerdict,
    pub decisions: HashMap<String, AiMember>,
}
impl CachedVerdict {
    pub fn from_result(result: &GroupResult) -> Option<Self> {
        if result
            .hashes
            .values()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != result.hashes.len()
        {
            return None;
        }
        let decisions = result
            .decisions
            .iter()
            .filter_map(|(id, value)| {
                result
                    .hashes
                    .get(id)
                    .map(|hash| (hash.clone(), value.clone()))
            })
            .collect();
        Some(Self {
            bucket: result.bucket.clone(),
            verdict: result.verdict.clone(),
            decisions,
        })
    }
    pub fn materialize(self, hashes: HashMap<i64, String>) -> GroupResult {
        let decisions = hashes
            .iter()
            .filter_map(|(id, hash)| self.decisions.get(hash).map(|v| (*id, v.clone())))
            .collect();
        GroupResult {
            hashes,
            bucket: self.bucket,
            verdict: self.verdict,
            decisions,
            features: HashMap::new(),
        }
    }
}

pub fn protection(member: &photo_core::FileMember, settings: &Settings) -> Vec<String> {
    let path = Path::new(&member.path);
    let mut flags = Vec::new();
    if path
        .ancestors()
        .skip(1)
        .filter_map(|p| p.file_name())
        .any(|name| {
            settings.protected_folders.iter().any(|f| {
                !f.trim().is_empty() && name.to_string_lossy().eq_ignore_ascii_case(f.trim())
            })
        })
    {
        flags.push("Protected folder".into());
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if stem
        .split(|c: char| !c.is_alphanumeric())
        .any(|part| part == "edit" || part == "edited")
    {
        flags.push("Edited filename".into());
    }
    if member.companion_raw_path.is_some() {
        flags.push("RAW companion".into());
    }
    if let Some(dir) = path.parent() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            if entries.flatten().any(|entry| {
                let p = entry.path();
                let same_stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().eq_ignore_ascii_case(&stem))
                    .unwrap_or(false);
                let raw = p
                    .extension()
                    .map(|s| {
                        matches!(
                            s.to_string_lossy().to_ascii_lowercase().as_str(),
                            "arw"
                                | "cr2"
                                | "cr3"
                                | "dng"
                                | "nef"
                                | "nrw"
                                | "orf"
                                | "raf"
                                | "rw2"
                                | "pef"
                                | "raw"
                                | "srw"
                                | "rwl"
                                | "3fr"
                                | "iiq"
                                | "mos"
                                | "mrw"
                                | "x3f"
                        )
                    })
                    .unwrap_or(false);
                same_stem && raw
            }) {
                flags.push("RAW companion".into());
            }
        }
    }
    let mut candidates = vec![
        path.with_extension("xmp"),
        std::path::PathBuf::from(format!("{}.xmp", member.path)),
    ];
    candidates.dedup();
    for sidecar in candidates {
        if sidecar.exists() {
            flags.push("XMP sidecar".into());
            match std::fs::read(&sidecar) {
                Ok(data) => xmp_flags(&data, &mut flags),
                Err(_) => flags.push("Unreadable XMP sidecar".into()),
            }
        }
    }
    if let Some(raw) = &member.companion_raw_path {
        if Path::new(raw).with_extension("xmp").exists()
            || Path::new(&format!("{raw}.xmp")).exists()
        {
            flags.push("RAW XMP sidecar".into());
        }
    }
    // Search embedded metadata as bytes, so binary JPEG/TIFF remains readable.
    match std::fs::read(path) {
        Ok(data) => xmp_flags(&data, &mut flags),
        Err(_) => flags.push("Unreadable photo".into()),
    }
    flags.sort();
    flags.dedup();
    flags
}
fn xmp_flags(data: &[u8], flags: &mut Vec<String>) {
    let text = String::from_utf8_lossy(data);
    let rating = regex::Regex::new(
        r#"(?i)(?:[\w.-]+:rating\s*=\s*[\"']\s*([1-9][0-9]*)|<[\w.-]+:rating[^>]*>\s*([1-9][0-9]*))"#,
    )
    .unwrap();
    let label = regex::Regex::new(
        r#"(?i)(?:[\w.-]+:label\s*=\s*[\"']\s*[^\"'\s]+|<[\w.-]+:label[^>]*>\s*[^<\s]+)"#,
    )
    .unwrap();
    let pick = regex::Regex::new(r#"(?i)(?:(?:[\w.-]+:)?pick\s*=\s*[\"']\s*(?:1|true|yes)|<(?:[\w.-]+:)?pick[^>]*>\s*(?:1|true|yes))"#).unwrap();
    if rating.is_match(&text) {
        flags.push("XMP rating".into());
    }
    if label.is_match(&text) {
        flags.push("XMP label".into());
    }
    if pick.is_match(&text) {
        flags.push("XMP pick".into());
    }
    let lower = text.to_lowercase();
    if lower.contains("crs:")
        || lower.contains("xmpmm:history")
        || lower.contains("<stEvt:")
        || lower.contains("<stevt:")
    {
        flags.push("XMP edits".into());
    }
}

fn number(s: &str) -> Option<f64> {
    let s = s.trim().trim_start_matches("f/").trim_end_matches("mm");
    s.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0)
}
pub fn has_variants(group: &DuplicateGroupDetail) -> bool {
    for (i, a) in group.members.iter().enumerate() {
        for b in &group.members[i + 1..] {
            if let (Some(x), Some(y)) = (&a.exif, &b.exif) {
                if let (Some(x), Some(y)) = (
                    x.focal_length.as_deref().and_then(number),
                    y.focal_length.as_deref().and_then(number),
                ) {
                    if x.max(y) / x.min(y) > 1.15 {
                        return true;
                    }
                }
                if let (Some(x), Some(y)) = (
                    x.aperture.as_deref().and_then(number),
                    y.aperture.as_deref().and_then(number),
                ) {
                    if x.max(y) / x.min(y) >= 2f64.sqrt() {
                        return true;
                    }
                }
            }
            if let (Some(aw), Some(ah), Some(bw), Some(bh)) = (a.width, a.height, b.width, b.height)
            {
                if (aw > ah && bw < bh) || (aw < ah && bw > bh) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn group_key(hashes: &[String]) -> String {
    let mut hashes = hashes.to_vec();
    hashes.sort();
    let mut hasher = blake3::Hasher::new();
    for hash in hashes {
        hasher.update(&(hash.len() as u64).to_le_bytes());
        hasher.update(hash.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}
pub fn decide(group: &DuplicateGroupDetail, hashes: HashMap<i64, String>) -> GroupResult {
    let all_hashed = hashes.len() == group.members.len() && !hashes.is_empty();
    let exact = all_hashed
        && hashes.values().all(|h| Some(h) == hashes.values().next())
        && group.kind == DuplicateKind::Exact;
    let variants = !exact && has_variants(group);
    let (bucket, group_type, reason, tier) = if exact {
        (
            "ready_to_confirm",
            "exact",
            "These files have identical content. Review the suggested keepers.",
            "rules",
        )
    } else if variants {
        (
            "variants_kept",
            "intentional_variants",
            "Camera settings or orientation differ; all variants are kept.",
            "rules",
        )
    } else {
        (
            "needs_you",
            "uncertain",
            "No reliable technical comparison is available; choose the keepers yourself.",
            "rules",
        )
    };
    let mut decisions = HashMap::new();
    if exact {
        // Prefer a RAW-backed copy, then older and shorter path, with stable tie-breaking.
        let best = group
            .members
            .iter()
            .min_by_key(|m| {
                (
                    m.companion_raw_path.is_none(),
                    m.created_at,
                    m.path.len(),
                    m.path.clone(),
                )
            })
            .map(|m| m.file_id);
        for m in &group.members {
            decisions.insert(
                m.file_id,
                AiMember {
                    decision: if Some(m.file_id) == best {
                        "keep"
                    } else {
                        "discard"
                    }
                    .into(),
                    score: None,
                    flags: vec![],
                },
            );
        }
    } else if variants {
        for m in &group.members {
            decisions.insert(
                m.file_id,
                AiMember {
                    decision: "keep".into(),
                    score: None,
                    flags: vec![],
                },
            );
        }
    }
    GroupResult {
        hashes,
        bucket: bucket.into(),
        verdict: AiVerdict {
            group_type: group_type.into(),
            reason: reason.into(),
            deciding_tier: tier.into(),
        },
        decisions,
        features: HashMap::new(),
    }
}

/// Variant detection only adds keepers, so uncertain mixed groups preserve every intent.
fn feature_variants(
    group: &DuplicateGroupDetail,
    features: &HashMap<i64, serde_json::Value>,
) -> bool {
    let value = |v: &serde_json::Value, key: &str| {
        v.get(key)
            .and_then(|n| n.as_f64())
            .filter(|n| n.is_finite())
    };
    for (i, a) in group.members.iter().enumerate() {
        for b in &group.members[i + 1..] {
            let (Some(x), Some(y)) = (features.get(&a.file_id), features.get(&b.file_id)) else {
                continue;
            };
            if let (Some(xm), Some(ym)) = (x.get("metadata"), y.get("metadata")) {
                for (key, threshold) in [("focal_length", 1.15), ("aperture", 2f64.sqrt())] {
                    if let (Some(a), Some(b)) = (value(xm, key), value(ym, key)) {
                        if a > 0.0 && b > 0.0 && a.max(b) / a.min(b) >= threshold {
                            return true;
                        }
                    }
                }
                for key in ["orientation", "lens", "subject_area"] {
                    if let (Some(a), Some(b)) = (xm.get(key), ym.get(key)) {
                        if !a.is_null() && !b.is_null() && a != b {
                            return true;
                        }
                    }
                }
                if let (Some(a), Some(b)) = (value(xm, "exposure_bias"), value(ym, "exposure_bias"))
                {
                    if (a - b).abs() >= 1.0 {
                        return true;
                    }
                }
            }
            let (Some(xs), Some(ys), Some(xb), Some(yb)) = (
                value(x, "subject_sharpness"),
                value(y, "subject_sharpness"),
                value(x, "background_sharpness"),
                value(y, "background_sharpness"),
            ) else {
                continue;
            };
            if x.get("subject_source").is_some()
                && x.get("subject_source") == y.get("subject_source")
                && xs > 0.0
                && ys > 0.0
                && xb > 0.0
                && yb > 0.0
                && ((xs > ys * 3.0 && yb > xb * 3.0) || (ys > xs * 3.0 && xb > yb * 3.0))
            {
                return true;
            }
        }
    }
    false
}

/// Only resolve an exceptionally obvious two-frame technical failure. Sharpness
/// values are texture-sensitive, so matching face boxes, capture settings and
/// an independent severe exposure failure are all required.
pub fn apply_local_analysis(
    result: &mut GroupResult,
    group: &DuplicateGroupDetail,
    features: &HashMap<i64, serde_json::Value>,
    preset: Preset,
) {
    if result.bucket != "needs_you" {
        return;
    }
    if feature_variants(group, features) {
        result.bucket = "variants_kept".into();
        result.verdict = AiVerdict { group_type: "intentional_variants".into(), reason: "Different camera settings or sharp regions may reflect different intent; all variants are kept.".into(), deciding_tier: "tier1".into() };
        for m in &group.members {
            result.decisions.insert(
                m.file_id,
                AiMember {
                    decision: "keep".into(),
                    score: None,
                    flags: vec![],
                },
            );
        }
        return;
    }
    if !matches!(group.kind, DuplicateKind::Visual | DuplicateKind::Burst)
        || group.members.len() != 2
    {
        return;
    }
    let a = &group.members[0];
    let b = &group.members[1];
    let (Some(ae), Some(be)) = (&a.exif, &b.exif) else {
        return;
    };
    if ae.focal_length.is_none()
        || ae.aperture.is_none()
        || ae.focal_length != be.focal_length
        || ae.aperture != be.aperture
    {
        return;
    }
    let (Some(x), Some(y)) = (features.get(&a.file_id), features.get(&b.file_id)) else {
        return;
    };
    if x.get("subject_source").and_then(|v| v.as_str()) != Some("face")
        || y.get("subject_source").and_then(|v| v.as_str()) != Some("face")
    {
        return;
    }
    let (Some(xf), Some(yf)) = (
        x.get("faces").and_then(|v| v.as_array()),
        y.get("faces").and_then(|v| v.as_array()),
    ) else {
        return;
    };
    if xf.len() != 1 || yf.len() != 1 {
        return;
    }
    for key in ["x", "y", "width", "height"] {
        let (Some(xv), Some(yv)) = (
            xf[0].get(key).and_then(|v| v.as_f64()),
            yf[0].get(key).and_then(|v| v.as_f64()),
        ) else {
            return;
        };
        if !xv.is_finite() || !yv.is_finite() || (xv - yv).abs() > 0.03 {
            return;
        }
    }
    let metric = |v: &serde_json::Value, name: &str| {
        v.get(name)
            .and_then(|v| v.as_f64())
            .filter(|x| x.is_finite() && *x >= 0.0)
    };
    let (Some(xs), Some(ys), Some(xb), Some(yb), Some(xh), Some(yh), Some(xd), Some(yd)) = (
        metric(x, "subject_sharpness"),
        metric(y, "subject_sharpness"),
        metric(x, "background_sharpness"),
        metric(y, "background_sharpness"),
        metric(x, "highlight_clipping"),
        metric(y, "highlight_clipping"),
        metric(x, "shadow_clipping"),
        metric(y, "shadow_clipping"),
    ) else {
        return;
    };
    // A reversal between subject and background can indicate different focus intent.
    if xs > 0.0
        && ys > 0.0
        && xb > 0.0
        && yb > 0.0
        && ((xs > ys * 3.0 && yb > xb * 3.0) || (ys > xs * 3.0 && xb > yb * 3.0))
    {
        result.bucket = "variants_kept".into();
        result.verdict = AiVerdict {
            group_type: "intentional_variants".into(),
            reason:
                "Different sharp regions may reflect different focus intent; both photos are kept."
                    .into(),
            deciding_tier: "tier1".into(),
        };
        for m in &group.members {
            result.decisions.insert(
                m.file_id,
                AiMember {
                    decision: "keep".into(),
                    score: None,
                    flags: vec![],
                },
            );
        }
        return;
    }
    let (ratio, clipping) = match preset {
        Preset::Conservative => (0.05, 0.70),
        Preset::Balanced => (0.10, 0.55),
        Preset::Aggressive => (0.20, 0.40),
    };
    let bad = |subject: f64,
               background: f64,
               clip: f64,
               good_subject: f64,
               good_background: f64,
               good_clip: f64| {
        good_subject > 1.0
            && good_background > 1.0
            && subject < good_subject * ratio
            && background < good_background * ratio
            && clip >= clipping
            && good_clip <= 0.03
    };
    let (discard, keep, clip_flag) = if bad(xs, xb, xh.max(xd), ys, yb, yh.max(yd)) {
        (
            a.file_id,
            b.file_id,
            if xh >= xd {
                "overexposed"
            } else {
                "underexposed"
            },
        )
    } else if bad(ys, yb, yh.max(yd), xs, xb, xh.max(xd)) {
        (
            b.file_id,
            a.file_id,
            if yh >= yd {
                "overexposed"
            } else {
                "underexposed"
            },
        )
    } else {
        return;
    };
    result.bucket = "ready_to_confirm".into();
    result.verdict = AiVerdict { group_type: "near_identical".into(), reason: "One matching frame has extreme loss of detail and exposure clipping. Check the suggestion.".into(), deciding_tier: "tier1".into() };
    result.decisions.insert(
        keep,
        AiMember {
            decision: "keep".into(),
            score: None,
            flags: vec![],
        },
    );
    result.decisions.insert(
        discard,
        AiMember {
            decision: "discard".into(),
            score: None,
            flags: vec!["low_detail".into(), clip_flag.into()],
        },
    );
}
pub fn apply_result(
    group: &mut DuplicateGroupDetail,
    result: Option<GroupResult>,
    settings: &Settings,
) {
    for member in &mut group.members {
        member.protected = protection(member, settings);
    }
    let Some(mut result) = result else {
        return;
    };
    // Never show stale preselection if any file is missing or its content changed.
    if group.members.len() != result.hashes.len()
        || group.members.iter().any(|m| {
            scan_engine::hash::blake3_file(Path::new(&m.path))
                .ok()
                .as_ref()
                != result.hashes.get(&m.file_id)
        })
    {
        return;
    }
    let features = result.features.clone();
    if settings.tier1_enabled {
        apply_local_analysis(&mut result, group, &features, settings.preset);
    }
    group.ai_bucket = Some(result.bucket);
    group.ai_verdict = Some(result.verdict);
    for member in &mut group.members {
        member.ai = result.decisions.get(&member.file_id).cloned();
        if !member.protected.is_empty() {
            if let Some(ai) = member.ai.as_mut() {
                ai.decision = "keep".into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_core::{ExifData, FileMember, ReviewStatus};
    use serde_json::json;

    fn member(id: i64, path: &Path) -> FileMember {
        FileMember {
            file_id: id,
            path: path.to_string_lossy().into_owned(),
            file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
            size: 1,
            width: Some(1200),
            height: Some(800),
            created_at: None,
            modified_at: None,
            exif: Some(ExifData {
                camera_make: None,
                camera_model: None,
                iso: None,
                aperture: Some("f/2.8".into()),
                shutter_speed: None,
                focal_length: Some("50mm".into()),
                date_taken: None,
            }),
            is_keeper: None,
            thumbnail_key: "x".into(),
            companion_raw_path: None,
            companion_raw_size: None,
            ai: None,
            protected: vec![],
        }
    }
    fn group(a: FileMember, b: FileMember, kind: DuplicateKind) -> DuplicateGroupDetail {
        DuplicateGroupDetail {
            id: 1,
            kind,
            confidence: 1.0,
            review_status: ReviewStatus::Pending,
            members: vec![a, b],
            bytes_recoverable: 0,
            ai_bucket: None,
            ai_verdict: None,
        }
    }
    fn features(subject: f64, background: f64, clip: f64) -> serde_json::Value {
        json!({"subject_source":"face","faces":[{"x":0.2,"y":0.2,"width":0.2,"height":0.2}],"subject_sharpness":subject,"background_sharpness":background,"highlight_clipping":clip,"shadow_clipping":0.0})
    }
    #[test]
    fn content_key_is_order_independent_and_counts_repeated_hashes() {
        let (a, b) = ("a".to_string(), "b".to_string());
        assert_eq!(
            group_key(&[a.clone(), b.clone()]),
            group_key(&[b.clone(), a.clone()])
        );
        assert_ne!(group_key(&[a.clone(), a]), group_key(&[b]));
    }
    #[test]
    fn protection_is_current_per_path_and_stale_content_invalidates() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jpg");
        let q = dir.path().join("b.jpg");
        std::fs::write(&p, b"same").unwrap();
        std::fs::write(&q, b"same").unwrap();
        let mut g = group(member(1, &p), member(2, &q), DuplicateKind::Exact);
        let hash = scan_engine::hash::blake3_file(&p).unwrap();
        let result = decide(&g, HashMap::from([(1, hash.clone()), (2, hash)]));
        std::fs::write(q.with_extension("xmp"), b"malformed").unwrap();
        apply_result(&mut g, Some(result.clone()), &Settings::default());
        assert_eq!(g.members[1].ai.as_ref().unwrap().decision, "keep");
        assert!(!g.members[1].protected.is_empty());
        std::fs::write(&q, b"changed").unwrap();
        let mut changed = group(member(1, &p), member(2, &q), DuplicateKind::Exact);
        apply_result(&mut changed, Some(result), &Settings::default());
        assert!(changed.ai_bucket.is_none());
    }
    #[test]
    fn embedded_alias_and_adjacent_raw_are_protected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("test-Edit-2.jpg");
        std::fs::write(&p, br#"<rdf:Description r:Rating='4'/>"#).unwrap();
        std::fs::write(dir.path().join("test-Edit-2.CR3"), b"raw").unwrap();
        let flags = protection(&member(1, &p), &Settings::default());
        assert!(flags.contains(&"XMP rating".to_string()));
        assert!(flags.contains(&"Edited filename".to_string()));
        assert!(flags.contains(&"RAW companion".to_string()));
    }
    #[test]
    fn focal_change_preserves_every_variant() {
        let mut a = member(1, Path::new("a.jpg"));
        let b = member(2, Path::new("b.jpg"));
        a.exif.as_mut().unwrap().focal_length = Some("35mm".into());
        let g = group(a, b, DuplicateKind::Burst);
        let r = decide(&g, HashMap::from([(1, "a".into()), (2, "b".into())]));
        assert_eq!(r.bucket, "variants_kept");
        assert!(r.decisions.values().all(|v| v.decision == "keep"));
    }
    #[test]
    fn tier1_requires_independent_extreme_evidence_and_honors_preset() {
        let g = group(
            member(1, Path::new("a.jpg")),
            member(2, Path::new("b.jpg")),
            DuplicateKind::Visual,
        );
        let original = decide(&g, HashMap::from([(1, "a".into()), (2, "b".into())]));
        let f = HashMap::from([
            (1, features(10.0, 10.0, 0.6)),
            (2, features(100.0, 100.0, 0.01)),
        ]);
        let mut conservative = original.clone();
        apply_local_analysis(&mut conservative, &g, &f, Preset::Conservative);
        assert_eq!(conservative.bucket, "needs_you");
        let mut balanced = original.clone();
        apply_local_analysis(&mut balanced, &g, &f, Preset::Balanced);
        assert_eq!(balanced.bucket, "needs_you"); // exactly the boundary is not enough
        let mut aggressive = original.clone();
        apply_local_analysis(&mut aggressive, &g, &f, Preset::Aggressive);
        assert_eq!(aggressive.bucket, "ready_to_confirm");
        let mut missing = original;
        apply_local_analysis(&mut missing, &g, &HashMap::new(), Preset::Aggressive);
        assert_eq!(missing.bucket, "needs_you");
    }
    #[test]
    fn opposed_sharp_regions_keep_both() {
        let g = group(
            member(1, Path::new("a.jpg")),
            member(2, Path::new("b.jpg")),
            DuplicateKind::Visual,
        );
        let mut r = decide(&g, HashMap::from([(1, "a".into()), (2, "b".into())]));
        let f = HashMap::from([
            (1, features(100.0, 1.0, 0.0)),
            (2, features(1.0, 100.0, 0.0)),
        ]);
        apply_local_analysis(&mut r, &g, &f, Preset::Aggressive);
        assert_eq!(r.bucket, "variants_kept");
        assert!(r.decisions.values().all(|v| v.decision == "keep"));
    }
    #[test]
    fn moved_content_reuses_features_and_changed_decode_is_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.jpg");
        std::fs::write(&path, b"photo").unwrap();
        let hash = scan_engine::hash::blake3_file(&path).unwrap();
        let db = scan_engine::db::Database::open_in_memory().unwrap();
        assert!(
            cached_features(&db, &path, &hash, "v1", |_| Ok(json!({"sharpness":42})))
                .unwrap()
                .is_some()
        );
        let moved = dir.path().join("moved.jpg");
        std::fs::rename(&path, &moved).unwrap();
        let cached = cached_features(&db, &moved, &hash, "v1", |_| {
            panic!("unchanged content must not be analyzed twice")
        })
        .unwrap();
        assert_eq!(cached.unwrap()["sharpness"], 42);
        let changed = cached_features(&db, &moved, &hash, "v2", |p| {
            std::fs::write(p, b"changed").unwrap();
            Ok(json!({"sharpness":1}))
        })
        .unwrap();
        assert!(changed.is_none());
        assert!(db.get_ai_features(&hash, "v2").unwrap().is_none());
        db.clear_ai_cache().unwrap();
        assert!(db.get_ai_features(&hash, "v1").unwrap().is_none());
    }

    #[test]
    fn mixed_burst_focus_variants_keep_every_member_without_exif() {
        let mut g = group(
            member(1, Path::new("a.jpg")),
            member(2, Path::new("b.jpg")),
            DuplicateKind::Burst,
        );
        g.members.push(member(3, Path::new("c.jpg")));
        for m in &mut g.members {
            m.exif = None;
        }
        let mut r = decide(
            &g,
            HashMap::from([(1, "a".into()), (2, "b".into()), (3, "c".into())]),
        );
        let f = HashMap::from([
            (1, features(100.0, 1.0, 0.0)),
            (2, features(1.0, 100.0, 0.0)),
        ]);
        apply_local_analysis(&mut r, &g, &f, Preset::Conservative);
        assert_eq!(r.bucket, "variants_kept");
        assert_eq!(r.decisions.len(), 3);
        assert!(r.decisions.values().all(|d| d.decision == "keep"));
    }

    #[test]
    fn imageio_metadata_preserves_intent_without_rust_exif() {
        let mut g = group(
            member(1, Path::new("a.heic")),
            member(2, Path::new("b.heic")),
            DuplicateKind::Burst,
        );
        for m in &mut g.members {
            m.exif = None;
        }
        let mut r = decide(&g, HashMap::from([(1, "a".into()), (2, "b".into())]));
        let f = HashMap::from([
            (1, json!({"metadata":{"focal_length":35}})),
            (2, json!({"metadata":{"focal_length":85}})),
        ]);
        apply_local_analysis(&mut r, &g, &f, Preset::Balanced);
        assert_eq!(r.bucket, "variants_kept");
    }
}
