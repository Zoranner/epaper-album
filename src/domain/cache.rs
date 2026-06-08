use crate::model::{CachedResource, DisplayState, PlanSnapshot, ResourceIndex};
use std::collections::BTreeSet;

pub const LOW_SPACE_BYTES: u64 = 1_073_741_824;
pub const TARGET_FREE_BYTES: u64 = 3_221_225_472;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheCleanupCandidate {
    pub sha256: String,
    pub byte_size: u64,
    pub last_used_at_unix_secs: u64,
    pub free_bytes_after_delete: u64,
}

pub fn missing_resources(snapshot: &PlanSnapshot, index: &ResourceIndex) -> Vec<String> {
    snapshot
        .plans
        .iter()
        .flat_map(|plan| plan.images.iter())
        .filter(|sha256| !index.contains(sha256))
        .fold(Vec::<String>::new(), |mut missing, sha256| {
            if !missing.iter().any(|known| known == sha256) {
                missing.push(sha256.clone());
            }
            missing
        })
}

pub fn protected_resources(
    snapshot: &PlanSnapshot,
    display_state: Option<&DisplayState>,
) -> BTreeSet<String> {
    let mut protected = snapshot.referenced_resources();

    if let Some(display_state) = display_state {
        if let Some(image_sha256) = &display_state.image_sha256 {
            protected.insert(image_sha256.clone());
        }
    }

    protected
}

pub fn cleanup_candidates(
    index: &ResourceIndex,
    protected: &BTreeSet<String>,
    available_bytes: u64,
    low_space_bytes: u64,
    target_free_bytes: u64,
) -> Vec<CacheCleanupCandidate> {
    if available_bytes >= low_space_bytes {
        return Vec::new();
    }

    let mut candidates: Vec<&CachedResource> = index
        .resources
        .iter()
        .filter(|resource| !protected.contains(&resource.sha256))
        .collect();

    candidates.sort_by(|left, right| {
        left.last_used_at_unix_secs
            .cmp(&right.last_used_at_unix_secs)
            .then_with(|| left.sha256.cmp(&right.sha256))
    });

    let mut free_bytes = available_bytes;
    let mut selected = Vec::new();

    for resource in candidates {
        free_bytes = free_bytes.saturating_add(resource.byte_size);
        selected.push(CacheCleanupCandidate {
            sha256: resource.sha256.clone(),
            byte_size: resource.byte_size,
            last_used_at_unix_secs: resource.last_used_at_unix_secs,
            free_bytes_after_delete: free_bytes,
        });

        if free_bytes >= target_free_bytes {
            break;
        }
    }

    selected
}

pub fn cleanup_candidates_for_plan(
    snapshot: &PlanSnapshot,
    index: &ResourceIndex,
    display_state: Option<&DisplayState>,
    available_bytes: u64,
) -> Vec<CacheCleanupCandidate> {
    let protected = protected_resources(snapshot, display_state);

    cleanup_candidates(
        index,
        &protected,
        available_bytes,
        LOW_SPACE_BYTES,
        TARGET_FREE_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LocalDate, PlanItem};

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn snapshot(images: &[&str]) -> PlanSnapshot {
        PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![PlanItem {
                id: 1,
                start: date("2026-06-06"),
                end: date("2026-06-08"),
                caption: "caption".to_string(),
                images: images.iter().map(|image| image.to_string()).collect(),
            }],
        }
    }

    fn resource(sha256: &str, byte_size: u64, last_used_at_unix_secs: u64) -> CachedResource {
        CachedResource {
            sha256: sha256.to_string(),
            byte_size,
            last_used_at_unix_secs,
        }
    }

    #[test]
    fn reports_missing_resources_without_duplicates() {
        let snapshot = snapshot(&["a", "b", "a", "c"]);
        let index = ResourceIndex {
            resources: vec![resource("a", 10, 1), resource("c", 10, 1)],
        };

        assert_eq!(missing_resources(&snapshot, &index), vec!["b"]);
    }

    #[test]
    fn protects_plan_resources_and_last_displayed_resource() {
        let snapshot = snapshot(&["a"]);
        let display_state = DisplayState {
            image_sha256: Some("last".to_string()),
            ..DisplayState::default()
        };

        let protected = protected_resources(&snapshot, Some(&display_state));

        assert!(protected.contains("a"));
        assert!(protected.contains("last"));
    }

    #[test]
    fn returns_no_cleanup_candidates_when_space_is_above_low_watermark() {
        let index = ResourceIndex {
            resources: vec![resource("old", 100, 1)],
        };

        let candidates = cleanup_candidates(&index, &BTreeSet::new(), 1_000, 500, 2_000);

        assert!(candidates.is_empty());
    }

    #[test]
    fn selects_oldest_unprotected_resources_until_target_free_space() {
        let index = ResourceIndex {
            resources: vec![
                resource("protected", 500, 1),
                resource("new", 700, 30),
                resource("old", 600, 10),
                resource("older", 800, 5),
            ],
        };
        let protected = BTreeSet::from(["protected".to_string()]);

        let candidates = cleanup_candidates(&index, &protected, 100, 500, 1_500);

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
