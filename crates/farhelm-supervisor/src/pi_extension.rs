//! Private materialization of the static Pi conversation reporter.
//!
//! The artifact lives under Farhelm's state rather than Pi's configuration or
//! project directories. A fixed versioned path is trusted only after its exact
//! bytes have been checked through the same bounded, no-follow regular-file
//! reader used for conversation records.

use anyhow::Context as _;
use std::path::{Path, PathBuf};

pub(crate) const SOURCE: &[u8] = include_bytes!("../assets/pi-conversation-v1.ts");

/// Publish the extension once, or verify the exact safe artifact already there.
pub(crate) async fn materialize(state_dir: &Path) -> anyhow::Result<PathBuf> {
    let directory = state_dir.join("integrations").join("pi");
    crate::ensure_private_dir(&directory).await?;
    let path = directory.join("farhelm-conversation-v1.ts");
    match crate::files::write_private_file(&path, SOURCE).await {
        Ok(()) => return Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("publishing the Pi extension"),
    }
    let existing = crate::agent_kind::read_bounded_regular_file(&path)
        .await
        .context("verifying the existing Pi extension")?
        .ok_or_else(|| anyhow::anyhow!("the existing Pi extension disappeared"))?;
    if existing.as_bytes() != SOURCE {
        anyhow::bail!("the existing Pi extension does not contain Farhelm's expected bytes");
    }
    Ok(path)
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
        assert_eq!(std::fs::read(&first).expect("artifact bytes"), SOURCE);
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
            .join("integrations/pi/farhelm-conversation-v1.ts");
        crate::ensure_private_dir(path.parent().unwrap())
            .await
            .expect("artifact directory");
        std::fs::write(&path, b"not Farhelm's extension").expect("collision");
        let error = materialize(state.path())
            .await
            .expect_err("mismatched collision must be refused");
        assert!(format!("{error:#}").contains("does not contain Farhelm's expected bytes"));
    }
}
