use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use photo_core::ScanConfig;

/// Camera RAW extensions (lowercase). `.tif` is included for pairing only.
pub const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "ari", "arw", "bay", "braw", "crw", "cr2", "cr3", "cap", "data", "dcs", "dcr", "dng",
    "drf", "eip", "erf", "fff", "gpr", "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef",
    "nrw", "obm", "orf", "pef", "ptx", "pxn", "r3d", "raf", "raw", "rwl", "rw2", "rwz", "sr2",
    "srf", "srw", "tif", "x3f",
];

pub fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

pub fn is_raw_extension(ext: &str) -> bool {
    RAW_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

pub fn is_tiff_family(ext: &str) -> bool {
    matches!(ext.to_ascii_lowercase().as_str(), "tif" | "tiff")
}

/// RAW files that should never be scanned on their own (all RAW extensions except TIFF).
pub fn is_standalone_raw_path(path: &Path) -> bool {
    extension_lower(path)
        .map(|ext| is_raw_extension(&ext) && !is_tiff_family(&ext))
        .unwrap_or(false)
}

fn file_key(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
    let ext = extension_lower(path)?;
    Some(format!("{stem}.{ext}"))
}

/// Build a per-directory lookup of lowercase `stem.ext` -> path.
pub fn index_directory_files(paths: &[PathBuf]) -> HashMap<PathBuf, HashMap<String, PathBuf>> {
    let mut by_dir: HashMap<PathBuf, HashMap<String, PathBuf>> = HashMap::new();

    for path in paths {
        let Some(parent) = path.parent() else {
            continue;
        };
        let Some(key) = file_key(path) else {
            continue;
        };
        by_dir
            .entry(parent.to_path_buf())
            .or_default()
            .insert(key, path.clone());
    }

    by_dir
}

/// Find a same-basename RAW companion for `image_path` in the same directory.
pub fn find_companion_raw(
    image_path: &Path,
    dir_index: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    let stem = image_path.file_stem()?.to_str()?.to_ascii_lowercase();

    for ext in RAW_EXTENSIONS {
        let key = format!("{stem}.{ext}");
        if let Some(candidate) = dir_index.get(&key) {
            if candidate != image_path {
                return Some(candidate.clone());
            }
        }
    }

    None
}

pub struct ScanEntryPlan {
    pub entries: Vec<PathBuf>,
    pub companion_raw_by_image: HashMap<PathBuf, PathBuf>,
}

/// Decide which files to scan and which RAW companions to associate.
pub fn plan_scan_entries(all_files: &[PathBuf], config: &ScanConfig) -> ScanEntryPlan {
    let scannable: Vec<PathBuf> = all_files
        .iter()
        .filter(|path| crate::hash::is_supported_image(path))
        .filter(|path| {
            if config.include_raw {
                true
            } else {
                !is_standalone_raw_path(path)
            }
        })
        .cloned()
        .collect();

    if !config.include_raw {
        return ScanEntryPlan {
            entries: scannable,
            companion_raw_by_image: HashMap::new(),
        };
    }

    let dir_indexes = index_directory_files(all_files);
    let mut companion_raw_by_image = HashMap::new();
    let mut companion_raw_paths = HashSet::new();

    for image_path in &scannable {
        let Some(parent) = image_path.parent() else {
            continue;
        };
        let dir_index = dir_indexes.get(parent);
        let Some(dir_index) = dir_index else {
            continue;
        };

        if let Some(raw_path) = find_companion_raw(image_path, dir_index) {
            companion_raw_by_image.insert(image_path.clone(), raw_path.clone());
            companion_raw_paths.insert(raw_path);
        }
    }

    let entries = scannable
        .into_iter()
        .filter(|path| !companion_raw_paths.contains(path))
        .collect();

    ScanEntryPlan {
        entries,
        companion_raw_by_image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_raw_excludes_arw_but_not_tif() {
        assert!(is_standalone_raw_path(Path::new("/p/DSC06404.ARW")));
        assert!(is_standalone_raw_path(Path::new("/p/DSC06404.arw")));
        assert!(!is_standalone_raw_path(Path::new("/p/scan.tif")));
        assert!(!is_standalone_raw_path(Path::new("/p/scan.tiff")));
        assert!(!is_standalone_raw_path(Path::new("/p/photo.jpg")));
    }

    #[test]
    fn pairs_jpg_with_arw_and_hides_raw_from_entries() {
        let jpg = PathBuf::from("/photos/DSC06404.JPG");
        let arw = PathBuf::from("/photos/DSC06404.ARW");
        let other = PathBuf::from("/photos/DSC06405.JPG");
        let all = vec![jpg.clone(), arw.clone(), other.clone()];

        let config = ScanConfig {
            include_raw: true,
            ..ScanConfig::default()
        };
        let plan = plan_scan_entries(&all, &config);

        assert_eq!(plan.entries.len(), 2);
        assert!(plan.entries.contains(&jpg));
        assert!(plan.entries.contains(&other));
        assert!(!plan.entries.contains(&arw));
        assert_eq!(plan.companion_raw_by_image.get(&jpg), Some(&arw));
    }

    #[test]
    fn tif_is_companion_when_jpg_exists() {
        let jpg = PathBuf::from("/photos/DSC06404.jpg");
        let tif = PathBuf::from("/photos/DSC06404.tif");
        let config = ScanConfig {
            include_raw: true,
            ..ScanConfig::default()
        };
        let plan = plan_scan_entries(&[jpg.clone(), tif.clone()], &config);

        assert_eq!(plan.entries, vec![jpg.clone()]);
        assert_eq!(plan.companion_raw_by_image.get(&jpg), Some(&tif));
    }

    #[test]
    fn standalone_tif_is_scanned_without_jpg() {
        let tif = PathBuf::from("/photos/DSC06404.tif");
        let config = ScanConfig {
            include_raw: true,
            ..ScanConfig::default()
        };
        let plan = plan_scan_entries(&[tif.clone()], &config);

        assert_eq!(plan.entries, vec![tif]);
        assert!(plan.companion_raw_by_image.is_empty());
    }

    #[test]
    fn include_raw_false_ignores_arw_entirely() {
        let jpg = PathBuf::from("/photos/DSC06404.JPG");
        let arw = PathBuf::from("/photos/DSC06404.ARW");
        let config = ScanConfig::default();
        let plan = plan_scan_entries(&[jpg.clone(), arw], &config);

        assert_eq!(plan.entries, vec![jpg]);
        assert!(plan.companion_raw_by_image.is_empty());
    }
}
