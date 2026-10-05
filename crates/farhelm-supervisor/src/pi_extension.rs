//! Private materialization of the static vendor conversation reporters.
//!
//! Each vendor's artifact lives under Farhelm's state rather than the vendor's
//! configuration or project directories. A fixed versioned path is trusted
//! only after its exact bytes have been checked through the same bounded,
//! no-follow regular-file reader used for conversation records.

use anyhow::Context as _;
use std::path::{Path, PathBuf};

/// One vendor's extension artifact: where it is published and what its exact
/// bytes are. The artifact is compiled into this binary, so the bytes on disk
/// are never "whatever is there" — a mismatched file is a collision to refuse.
pub(crate) struct VendorAsset {
    /// The `integrations/` subdirectory this vendor's artifact publishes to.
    pub directory: &'static str,
    /// The artifact's file name inside that directory.
    pub file_name: &'static str,
    /// The exact bytes the published file must contain.
    pub source: &'static [u8],
}

/// Pi's conversation reporter, loaded with `-e` and pointed at the reporter
/// through `FARHELM_PI_REPORTER_EXE`.
///
/// The embedded reporter deliberately keeps its published two-second child
/// timer. Changing those source bytes would require a new asset identity, so
/// keep the published value. The hook it spawns only records its ancestry
/// and writes a report file, which fits well inside that timer whether or
/// not a supervisor is running; Pi's end-of-turn reports heal the rare
/// report that does not.
///
/// `v2` because the bytes changed after `v1` had shipped (#811, first in
/// v0.13.0) and [`materialize_asset`] refuses a published file whose bytes
/// differ: every host that had already published `v1` went on launching Pi
/// without the extension. A new name publishes beside the old file instead.
/// `published_assets_are_pinned_to_their_names` below keeps that from
/// happening again.
pub(crate) const PI_ASSET: VendorAsset = VendorAsset {
    directory: "pi",
    file_name: "farhelm-conversation-v2.ts",
    source: include_bytes!("../assets/pi-conversation-v1.ts"),
};

/// OMP's conversation reporter, loaded with `-e` and pointed at the reporter
/// through `FARHELM_OMP_REPORTER_EXE`. Same publication contract as Pi's;
/// the assets are never shared because the vendors' event surfaces differ.
///
/// Its published two-second child timer is intentional for the same reason as
/// Pi's, but OMP also refuses a report whose launch named a different asset
/// identity. Changing the bytes would therefore make every running OMP
/// session lose conversation tracking until relaunched; an end-of-turn report
/// on the next turn heals one cut short by the timer.
///
/// The file name is versioned past OMP's own gateless `v1`: the gated asset must
/// materialize beside — never over — the gateless `v1` bytes an old launch
/// may still be running, so post-upgrade launches capture immediately
/// while old launches fail closed by launch provenance instead of by
/// having their file pulled out from under them.
pub(crate) const OMP_ASSET: VendorAsset = VendorAsset {
    directory: "omp",
    file_name: "farhelm-conversation-v2.ts",
    source: include_bytes!("../assets/omp-conversation-v1.ts"),
};

/// The reporter extension a kind's launches load, if it has one: each
/// locator-reporting kind needs its OWN vendor's artifact, because the
/// reporters carry different event surfaces and different `vendor`
/// payloads, so one vendor's asset can never serve the other.
///
/// Exhaustive so a new kind decides whether it loads one.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) fn reporter_asset(kind: farhelm_proto::AgentKind) -> Option<VendorAsset> {
    use farhelm_proto::AgentKind;
    match kind {
        AgentKind::Pi => Some(PI_ASSET),
        AgentKind::Omp => Some(OMP_ASSET),
        AgentKind::Claude
        | AgentKind::Codex
        | AgentKind::Goose
        | AgentKind::Grok
        | AgentKind::Generic => None,
    }
}

/// Publish one vendor's extension once, or verify the exact safe artifact
/// already there.
pub(crate) async fn materialize_asset(
    state_dir: &Path,
    asset: &VendorAsset,
) -> anyhow::Result<PathBuf> {
    let directory = state_dir.join("integrations").join(asset.directory);
    crate::ensure_private_dir(&directory).await?;
    let path = directory.join(asset.file_name);
    match crate::files::write_private_file(&path, asset.source).await {
        Ok(()) => return Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("publishing the {} extension", asset.directory));
        }
    }
    let existing = crate::agent_kind::read_bounded_regular_file(&path)
        .await
        .with_context(|| format!("verifying the existing {} extension", asset.directory))?
        .ok_or_else(|| anyhow::anyhow!("the existing {} extension disappeared", asset.directory))?;
    if existing.as_bytes() != asset.source {
        anyhow::bail!(
            "the existing {} extension does not contain Farhelm's expected bytes",
            asset.directory
        );
    }
    Ok(path)
}

/// Publish Pi's extension: [`materialize_asset`] with [`PI_ASSET`]. Kept as
/// the Pi tests' own entry point so they keep exercising exactly the shape a
/// real Pi launch publishes.
#[cfg(test)]
pub(crate) async fn materialize(state_dir: &Path) -> anyhow::Result<PathBuf> {
    materialize_asset(state_dir, &PI_ASSET).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The owned artifact is private and an exact second materialization
    /// reuses it, which is the ordinary create-then-restart lifecycle.
    #[farhelm_testtrace::test]
    async fn exact_artifact_is_private_and_reusable() {
        use std::os::unix::fs::PermissionsExt as _;

        let state = farhelm_teststate::tempdir().expect("state directory");
        let first = materialize(state.path()).await.expect("first publish");
        let second = materialize(state.path()).await.expect("exact reuse");
        assert_eq!(first, second);
        assert_eq!(
            std::fs::read(&first).expect("artifact bytes"),
            PI_ASSET.source
        );
        assert_eq!(
            std::fs::metadata(first)
                .expect("artifact metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    /// A path collision is never trusted by name alone; differing bytes
    /// disable injection instead of executing a file another process placed.
    #[farhelm_testtrace::test]
    async fn mismatched_existing_artifact_is_refused() {
        let state = farhelm_teststate::tempdir().expect("state directory");
        let path = state
            .path()
            .join("integrations/pi/farhelm-conversation-v2.ts");
        crate::ensure_private_dir(path.parent().unwrap())
            .await
            .expect("artifact directory");
        std::fs::write(&path, b"not Farhelm's extension").expect("collision");
        let error = materialize(state.path())
            .await
            .expect_err("mismatched collision must be refused");
        assert!(format!("{error:#}").contains("does not contain Farhelm's expected bytes"));
    }

    /// OMP's artifact materializes under its own directory under the same
    /// exact-bytes contract: private, reused on a second publish, and
    /// refused on a collision — without disturbing Pi's published copy.
    /// The versioned file name keeps the gated asset beside the gateless
    /// `v1` bytes rather than over them.
    #[farhelm_testtrace::test]
    async fn omp_artifact_materializes_privately_and_refuses_collisions() {
        let state = farhelm_teststate::tempdir().expect("state directory");
        let first = materialize_asset(state.path(), &OMP_ASSET)
            .await
            .expect("first publish");
        assert_eq!(
            first,
            state
                .path()
                .join("integrations/omp/farhelm-conversation-v2.ts")
        );
        let second = materialize_asset(state.path(), &OMP_ASSET)
            .await
            .expect("exact reuse");
        assert_eq!(first, second);
        assert_eq!(
            std::fs::read(&first).expect("artifact bytes"),
            OMP_ASSET.source
        );
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&first)
                .expect("artifact metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        // The vendors' artifacts live in disjoint directories, so neither
        // publication ever sees the other's file.
        let pi_path = materialize(state.path()).await.expect("Pi publish");
        assert_ne!(pi_path, first);

        let collision = state
            .path()
            .join("integrations/omp/farhelm-conversation-v2.ts");
        std::fs::write(&collision, b"not Farhelm's extension").expect("collision");
        let error = materialize_asset(state.path(), &OMP_ASSET)
            .await
            .expect_err("mismatched collision must be refused");
        assert!(format!("{error:#}").contains("does not contain Farhelm's expected bytes"));
    }

    /// The gated asset publishes BESIDE the gateless `v1` bytes, never over
    /// them: a pre-existing `v1` file (an old launch's loaded artifact) is
    /// left byte-identical while the versioned publish lands separately.
    ///
    /// Why this test matters: the old-asset fail-closed rule depends on the
    /// old file surviving the upgrade untouched — old launches keep running
    /// the bytes they loaded and fail closed by launch provenance, not by
    /// having their artifact replaced under them.
    #[farhelm_testtrace::test]
    async fn omp_gated_asset_publishes_beside_the_old_artifact() {
        let state = farhelm_teststate::tempdir().expect("state directory");
        let old = state
            .path()
            .join("integrations/omp/farhelm-conversation-v1.ts");
        crate::ensure_private_dir(old.parent().unwrap())
            .await
            .expect("artifact directory");
        std::fs::write(&old, b"gateless old bytes").expect("old artifact");
        let gated = materialize_asset(state.path(), &OMP_ASSET)
            .await
            .expect("gated publish");
        assert_eq!(
            std::fs::read(&old).expect("old artifact bytes"),
            b"gateless old bytes",
            "the old launch's artifact is untouched by the upgrade publish"
        );
        assert_eq!(
            std::fs::read(&gated).expect("gated artifact bytes"),
            OMP_ASSET.source
        );
    }

    /// Every published asset's bytes are pinned to its file name: changing
    /// an asset's source without giving it a new name fails here.
    ///
    /// Why it matters: a host keeps the file it published, and
    /// [`materialize_asset`] refuses one whose bytes differ, so an asset
    /// changed under its old name stops loading on every host that published
    /// the old bytes, silently, with no conversation capture and no Resume.
    /// That happened to Pi's `v1`. Spec: a change to an asset's bytes comes
    /// with a new `file_name` and a new row here, never a new hash under an
    /// old name.
    #[farhelm_testtrace::test]
    fn published_assets_are_pinned_to_their_names() {
        use sha2::{Digest as _, Sha256};
        let pinned = [
            (
                "pi",
                "farhelm-conversation-v2.ts",
                "a1d4f427a8e418b1b548671aafa4ae3aad8a6fb49793a12015c3aae118afc045",
            ),
            (
                "omp",
                "farhelm-conversation-v2.ts",
                "bfa0a909db4e488c85c12db0418e4e7ca6f953b56e5344a9bd1f474f49e52a12",
            ),
        ];
        // Every asset a launch can load, from the per-kind map a new
        // harness's asset is added to, so a new asset cannot skip this test.
        let assets: Vec<VendorAsset> = farhelm_proto::AgentKind::ALL
            .iter()
            .copied()
            .filter_map(reporter_asset)
            .collect();
        assert_eq!(
            assets.len(),
            pinned.len(),
            "every published asset needs a pinned row"
        );
        for asset in &assets {
            let (_, file_name, sha256) = pinned
                .iter()
                .find(|row| row.0 == asset.directory)
                .unwrap_or_else(|| panic!("no pinned row for the {} asset", asset.directory));
            let actual: String = Sha256::digest(asset.source)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(
                (asset.file_name, actual.as_str()),
                (*file_name, *sha256),
                "the {} asset's bytes changed under a published name; publish them under a new file name",
                asset.directory
            );
        }
    }
}
