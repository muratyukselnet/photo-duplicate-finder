use std::collections::{HashMap, HashSet};

use photo_core::{DuplicateKind, ExifData, ScanConfig};
use serde_json;

use crate::hash::hamming_distance;

#[derive(Debug, Clone)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub blake3: Option<String>,
    pub dhash: Option<u64>,
    pub phash: Option<u64>,
    pub exif: Option<ExifData>,
}

pub struct DuplicateCluster {
    pub kind: DuplicateKind,
    pub confidence: f32,
    pub file_ids: Vec<i64>,
}

pub fn cluster_duplicates(files: Vec<FileRecord>, config: &ScanConfig) -> Vec<DuplicateCluster> {
    let mut clusters = Vec::new();
    let mut used = HashSet::new();

    if config.exact_hash {
        clusters.extend(cluster_exact(&files, &mut used));
    }

    if config.visual_similar {
        clusters.extend(cluster_visual(&files, config.phash_threshold, &mut used));
    }

    if config.burst_detection {
        clusters.extend(cluster_burst(&files, config.burst_window_secs, &mut used));
    }

    clusters
}

fn cluster_exact(files: &[FileRecord], used: &mut HashSet<i64>) -> Vec<DuplicateCluster> {
    let mut by_hash: HashMap<String, Vec<i64>> = HashMap::new();

    for file in files {
        if used.contains(&file.id) {
            continue;
        }
        if let Some(hash) = &file.blake3 {
            by_hash.entry(hash.clone()).or_default().push(file.id);
        }
    }

    let mut clusters = Vec::new();
    for ids in by_hash.into_values() {
        if ids.len() < 2 {
            continue;
        }
        for id in &ids {
            used.insert(*id);
        }
        clusters.push(DuplicateCluster {
            kind: DuplicateKind::Exact,
            confidence: 1.0,
            file_ids: ids,
        });
    }

    clusters
}

fn cluster_visual(
    files: &[FileRecord],
    threshold: u8,
    used: &mut HashSet<i64>,
) -> Vec<DuplicateCluster> {
    let candidates: Vec<&FileRecord> = files
        .iter()
        .filter(|f| !used.contains(&f.id) && f.phash.is_some())
        .collect();

    let mut buckets: HashMap<u16, Vec<usize>> = HashMap::new();
    for (idx, file) in candidates.iter().enumerate() {
        let phash = file.phash.unwrap();
        let prefix = (phash >> 48) as u16;
        buckets.entry(prefix).or_default().push(idx);
    }

    let mut uf = UnionFind::new(candidates.len());
    let threshold = threshold as u32;

    for indices in buckets.values() {
        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                let a = candidates[indices[i]];
                let b = candidates[indices[j]];

                if !size_dimension_compatible(a, b) {
                    continue;
                }

                let dist = hamming_distance(a.phash.unwrap(), b.phash.unwrap());
                if dist <= threshold {
                    uf.union(indices[i], indices[j]);
                }
            }
        }
    }

    let groups = uf.groups();
    let mut clusters = Vec::new();

    for member_indices in groups {
        if member_indices.len() < 2 {
            continue;
        }

        let ids: Vec<i64> = member_indices
            .iter()
            .map(|&idx| candidates[idx].id)
            .collect();

        for id in &ids {
            used.insert(*id);
        }

        let confidence = visual_confidence(&candidates, &member_indices, threshold);
        clusters.push(DuplicateCluster {
            kind: DuplicateKind::Visual,
            confidence,
            file_ids: ids,
        });
    }

    clusters
}

fn cluster_burst(
    files: &[FileRecord],
    window_secs: u32,
    used: &mut HashSet<i64>,
) -> Vec<DuplicateCluster> {
    let mut dated: Vec<&FileRecord> = files
        .iter()
        .filter(|f| !used.contains(&f.id))
        .filter(|f| f.exif.as_ref().and_then(|e| e.date_taken).is_some())
        .collect();

    dated.sort_by_key(|f| f.exif.as_ref().and_then(|e| e.date_taken));

    let window = chrono::Duration::seconds(window_secs as i64);
    let mut clusters = Vec::new();
    let mut i = 0;

    while i < dated.len() {
        let mut group = vec![dated[i].id];
        let base = dated[i].clone();
        let base_time = base.exif.as_ref().and_then(|e| e.date_taken).unwrap();
        let base_camera = camera_key(&base);

        let mut j = i + 1;
        while j < dated.len() {
            let next = dated[j];
            let next_time = next.exif.as_ref().and_then(|e| e.date_taken).unwrap();
            if next_time - base_time > window {
                break;
            }
            if camera_key(next) == base_camera {
                group.push(next.id);
            }
            j += 1;
        }

        if group.len() >= 2 {
            for id in &group {
                used.insert(*id);
            }
            clusters.push(DuplicateCluster {
                kind: DuplicateKind::Burst,
                confidence: 0.75,
                file_ids: group,
            });
        }

        i = j.max(i + 1);
    }

    clusters
}

fn size_dimension_compatible(a: &FileRecord, b: &FileRecord) -> bool {
    if a.size == b.size {
        return true;
    }
    match (a.width, a.height, b.width, b.height) {
        (Some(w1), Some(h1), Some(w2), Some(h2)) => w1 == w2 && h1 == h2,
        _ => false,
    }
}

fn visual_confidence(candidates: &[&FileRecord], indices: &[usize], threshold: u32) -> f32 {
    let mut min_dist = threshold;
    for i in 0..indices.len() {
        for j in (i + 1)..indices.len() {
            let a = candidates[indices[i]].phash.unwrap();
            let b = candidates[indices[j]].phash.unwrap();
            min_dist = min_dist.min(hamming_distance(a, b));
        }
    }
    1.0 - (min_dist as f32 / (threshold.max(1) as f32))
}

fn camera_key(file: &FileRecord) -> Option<String> {
    file.exif.as_ref().map(|e| {
        format!(
            "{}|{}",
            e.camera_make.as_deref().unwrap_or(""),
            e.camera_model.as_deref().unwrap_or("")
        )
    })
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[rb] = ra;
        }
    }

    fn groups(&mut self) -> Vec<Vec<usize>> {
        let mut map: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..self.parent.len() {
            let root = self.find(i);
            map.entry(root).or_default().push(i);
        }
        map.into_values().collect()
    }
}

pub fn file_record_from_row(
    id: i64,
    path: String,
    size: u64,
    width: Option<u32>,
    height: Option<u32>,
    blake3: Option<String>,
    dhash: Option<u64>,
    phash: Option<u64>,
    exif_json: Option<String>,
) -> FileRecord {
    let exif = exif_json.and_then(|j| serde_json::from_str::<ExifData>(&j).ok());
    FileRecord {
        id,
        path,
        size,
        width,
        height,
        blake3,
        dhash,
        phash,
        exif,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_find_merges_transitive_groups() {
        let mut uf = UnionFind::new(4);
        uf.union(0, 1);
        uf.union(1, 2);
        let groups = uf.groups();
        let large = groups.iter().find(|g| g.len() == 3).expect("group of 3");
        assert!(large.contains(&0));
        assert!(large.contains(&2));
    }

    #[test]
    fn hamming_clustering_respects_threshold() {
        let files = vec![
            FileRecord {
                id: 1,
                path: "a.jpg".into(),
                size: 100,
                width: Some(100),
                height: Some(100),
                blake3: None,
                dhash: Some(0),
                phash: Some(0b1111),
                exif: None,
            },
            FileRecord {
                id: 2,
                path: "b.jpg".into(),
                size: 100,
                width: Some(100),
                height: Some(100),
                blake3: None,
                dhash: Some(0),
                phash: Some(0b1110),
                exif: None,
            },
        ];

        let config = ScanConfig {
            exact_hash: false,
            visual_similar: true,
            burst_detection: false,
            filename_ranking: false,
            phash_threshold: 2,
            burst_window_secs: 2,
            include_raw: false,
        };

        let clusters = cluster_duplicates(files, &config);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].kind, DuplicateKind::Visual);
    }
}
