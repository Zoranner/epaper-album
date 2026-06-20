use crate::model::Plan;
use std::collections::BTreeSet;

pub const LOW_SPACE_BYTES: u64 = 1_073_741_824;
pub const TARGET_FREE_BYTES: u64 = 3_221_225_472;

pub fn referenced_images(plans: &[Plan]) -> BTreeSet<String> {
    plans.iter().map(|plan| plan.image.clone()).collect()
}

pub fn missing_images(plans: &[Plan], mut exists: impl FnMut(&str) -> bool) -> Vec<&str> {
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();

    for plan in plans {
        if seen.insert(plan.image.as_str()) && !exists(&plan.image) {
            missing.push(plan.image.as_str());
        }
    }

    missing
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheFile {
    pub sha256: String,
    pub byte_size: u64,
    pub modified_at_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheCleanupCandidate {
    pub sha256: String,
    pub byte_size: u64,
    pub modified_at_unix_secs: u64,
    pub free_bytes_after_delete: u64,
}

pub fn cleanup_candidates(
    files: &[CacheFile],
    protected: &BTreeSet<String>,
    available_bytes: u64,
    low_space_bytes: u64,
    target_free_bytes: u64,
) -> Vec<CacheCleanupCandidate> {
    if available_bytes >= low_space_bytes {
        return Vec::new();
    }

    let mut candidates = files
        .iter()
        .filter(|file| !protected.contains(&file.sha256))
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        left.modified_at_unix_secs
            .cmp(&right.modified_at_unix_secs)
            .then_with(|| left.sha256.cmp(&right.sha256))
    });

    let mut free_bytes = available_bytes;
    let mut selected = Vec::new();

    for file in candidates {
        free_bytes = free_bytes.saturating_add(file.byte_size);
        selected.push(CacheCleanupCandidate {
            sha256: file.sha256.clone(),
            byte_size: file.byte_size,
            modified_at_unix_secs: file.modified_at_unix_secs,
            free_bytes_after_delete: free_bytes,
        });

        if free_bytes >= target_free_bytes {
            break;
        }
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(image: &str) -> Plan {
        Plan::fixed(
            crate::model::LocalDate::new(2026, 6, 6).unwrap(),
            "caption",
            image,
        )
    }

    fn file(sha256: &str, byte_size: u64, modified_at_unix_secs: u64) -> CacheFile {
        CacheFile {
            sha256: sha256.to_string(),
            byte_size,
            modified_at_unix_secs,
        }
    }

    #[test]
    fn reports_missing_images_without_duplicates() {
        let plans = vec![plan("a"), plan("b"), plan("a"), plan("c")];

        let missing = missing_images(&plans, |sha256| matches!(sha256, "a" | "c"));

        assert_eq!(missing, vec!["b"]);
    }

    #[test]
    fn collects_referenced_images() {
        let plans = vec![plan("a"), plan("b"), plan("a")];

        assert_eq!(
            referenced_images(&plans),
            BTreeSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn returns_no_cleanup_candidates_when_space_is_above_low_watermark() {
        let files = vec![file("old", 100, 1)];

        let candidates = cleanup_candidates(&files, &BTreeSet::new(), 1_000, 500, 2_000);

        assert!(candidates.is_empty());
    }

    #[test]
    fn selects_oldest_unprotected_files_until_target_free_space() {
        let files = vec![
            file("protected", 500, 1),
            file("new", 700, 30),
            file("old", 600, 10),
            file("older", 800, 5),
        ];
        let protected = BTreeSet::from(["protected".to_string()]);

        let candidates = cleanup_candidates(&files, &protected, 100, 500, 1_500);

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.sha256.as_str())
                .collect::<Vec<_>>(),
            vec!["older", "old"]
        );
        assert_eq!(candidates.last().unwrap().free_bytes_after_delete, 1_500);
    }
}
