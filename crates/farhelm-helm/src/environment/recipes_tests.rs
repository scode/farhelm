//! Shared recipe-state compatibility and resource-retention contracts.

use super::*;

/// Missing Podman state must load as empty, and Tensorlake-only writes retain
/// their historical fields without introducing a new provider wrapper.
#[test]
fn old_tensorlake_state_loads_without_a_podman_map() {
    let legacy = serde_json::json!({
        "snapshot": {"id": "built", "built_at": 7, "disk_gb": 20},
        "pending_create_at": 9,
        "builder_sandbox_id": "builder",
        "retired_snapshots": ["retired"]
    });
    let state: RecipeState = serde_json::from_value(legacy).unwrap();
    assert!(state.podman.is_empty());
    assert!(state.needs_cleanup());
    assert_eq!(
        state.snapshot.as_ref().unwrap().base,
        RecipeBase::ManagedUbuntu
    );
    let encoded = serde_json::to_value(&state).unwrap();
    assert!(encoded.get("podman").is_none());
    assert_eq!(
        encoded["snapshot"]["base"],
        serde_json::json!({"kind":"tensorlake_ubuntu"})
    );
    assert_eq!(encoded["pending_create_at"], 9);
    assert_eq!(encoded["builder_sandbox_id"], "builder");
    assert_eq!(
        serde_json::from_value::<RecipeState>(encoded).unwrap(),
        state
    );
    assert_eq!(
        serde_json::from_str::<RecipeState>("{}").unwrap(),
        RecipeState::default()
    );
}

/// A write for either provider must retain every other place's owned IDs. Host
/// IDs are numeric map keys on disk, keeping a place independent of its label.
#[test]
fn podman_places_round_trip_independently_of_tensorlake_state() {
    let json = serde_json::json!({
        "snapshot": {"id": "tensorlake-built"},
        "podman": {
            "7": {
                "image": {
                    "id": "sha256:built", "built_at": 10,
                    "base": {"kind":"container_image", "reference":"example.test/team/dev:latest", "user":"dev"},
                    "packages": ["git"]
                },
                "prepared": {
                    "id": "sha256:prepared", "prepared_at": 11,
                    "farhelm_version":"test-version", "source_image":"sha256:built",
                    "facts": {"home":"/home/dev", "farhelm_path":"/home/dev/bin/farhelm", "state_dir":"/home/dev/state", "farhelm_version":"test-version"}
                },
                "retired_images": ["sha256:old"]
            },
            "8": {"builder_container_id":"builder-eight", "build_started_at":12}
        }
    });
    let mut state: RecipeState = serde_json::from_value(json).unwrap();
    let places = state.podman.clone();
    assert_eq!(places.len(), 2);
    assert_eq!(
        places[&7].prepared.as_ref().unwrap().source_image,
        "sha256:built"
    );
    assert!(state.needs_cleanup());
    state.pending_create_at = Some(13);
    let encoded = serde_json::to_value(&state).unwrap();
    assert_eq!(
        encoded["podman"]["8"]["builder_container_id"],
        "builder-eight"
    );
    let decoded: RecipeState = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.podman, places);
    assert_eq!(decoded.snapshot.unwrap().id, "tensorlake-built");
    assert_eq!(decoded.pending_create_at, Some(13));
}

/// Forgetting a recipe must never lose an image, a container or interrupted
/// work. Empty place records themselves hold no cleanup obligation.
#[test]
fn every_owned_podman_resource_blocks_recipe_removal() {
    assert!(!RecipeState::default().needs_cleanup());
    for resource in [
        serde_json::json!({"image":{"id":"built","built_at":10,"base":{"kind":"tensorlake_ubuntu"},"packages":[]}}),
        serde_json::json!({"prepared":{"id":"prepared","prepared_at":11,"farhelm_version":"test","source_image":"built","facts":{"home":"/home/dev","farhelm_path":"/home/dev/bin/farhelm","state_dir":"/home/dev/state","farhelm_version":"test"}}}),
        serde_json::json!({"builder_container_id":"builder"}),
        serde_json::json!({"preparer_container_id":"preparer"}),
        serde_json::json!({"build_started_at":10}),
        serde_json::json!({"prepare_started_at":11}),
        serde_json::json!({"retired_images":["retired"]}),
    ] {
        let mut state = RecipeState::default();
        state
            .podman
            .insert(7, serde_json::from_value(resource).unwrap());
        assert!(state.needs_cleanup());
    }
    let mut empty = RecipeState::default();
    empty.podman.insert(7, PodmanRecipeState::default());
    assert!(!empty.needs_cleanup());
}
