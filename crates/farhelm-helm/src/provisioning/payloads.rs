//! Host-architecture-specific payload sources own release embedding and
//! materialization so the provisioning executor does not.

use super::plan::{PayloadArch, PayloadKind};
use anyhow::{Context as _, bail};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Supplies host-architecture-specific artifacts without coupling item 6 to
/// item 8's release embedding.
pub trait PayloadSource: Send + Sync {
    fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<PathBuf>;
}

/// Development helms intentionally carry no cross-compiled install payloads.
pub(super) struct NoPayloads;

impl PayloadSource for NoPayloads {
    fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
        bail!("this build carries no provisioning payloads")
    }
}

include!(concat!(env!("OUT_DIR"), "/embedded_payloads.rs"));

/// Release payloads compiled into the helm and materialized below helm state.
///
/// Keeping materialization here avoids teaching provisioning about release
/// layout or byte blobs. Development builds generate an empty table and keep
/// using [`NoPayloads`], so an ordinary cargo build never depends on foreign
/// target artifacts. The directory is app-owned rather than system-temporary:
/// a tmp cleaner must not be able to remove provisioning support from a helm
/// that is still running.
struct EmbeddedPayloads {
    root: PathBuf,
    state: std::sync::Mutex<EmbeddedPayloadState>,
}

/// Process-local cache for the generation this helm binary embeds.
struct EmbeddedPayloadState {
    prepared: bool,
    paths: std::collections::HashMap<(PayloadKind, PayloadArch), PathBuf>,
}

impl EmbeddedPayloads {
    /// Create a lazy source without touching the filesystem at helm startup.
    ///
    /// Only a confirmed plan needing a payload pays the write and fsync cost.
    /// Files stay in app-owned state for reuse; the next access removes names
    /// that no longer belong to the embedded generation.
    fn load(helm_state_dir: &Path) -> anyhow::Result<Option<Self>> {
        if EMBEDDED_PAYLOADS.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            root: helm_state_dir.join("embedded-payloads"),
            state: std::sync::Mutex::new(EmbeddedPayloadState {
                prepared: false,
                paths: std::collections::HashMap::new(),
            }),
        }))
    }

    /// Prepare the private cache and discard files from older manifests.
    fn prepare_root(&self) -> anyhow::Result<()> {
        let root = &self.root;
        std::fs::create_dir_all(root)
            .context("creating the app-owned embedded payload directory")?;
        #[cfg(unix)]
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
        for entry in std::fs::read_dir(root).context("reading the embedded payload directory")? {
            let entry = entry.context("reading an embedded payload directory entry")?;
            if EMBEDDED_PAYLOADS
                .iter()
                .any(|payload| entry.file_name() == std::ffi::OsStr::new(payload.filename))
            {
                continue;
            }
            let file_type = entry.file_type().with_context(|| {
                format!(
                    "inspecting stale embedded payload {}",
                    entry.path().display()
                )
            })?;
            if file_type.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
}

impl PayloadSource for EmbeddedPayloads {
    fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<PathBuf> {
        let embedded = EMBEDDED_PAYLOADS
            .iter()
            .find(|embedded| embedded.kind == payload && embedded.arch == arch)
            .with_context(|| {
                format!("this build carries no {payload:?} provisioning payload for {arch:?}")
            })?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("embedded payload cache lock was poisoned"))?;
        if !state.prepared {
            self.prepare_root()?;
            state.prepared = true;
        }
        if let Some(path) = state.paths.get(&(payload, arch)) {
            return Ok(path.clone());
        }

        let path = self.root.join(embedded.filename);
        let mut staged = tempfile::NamedTempFile::new_in(&self.root)
            .with_context(|| format!("staging embedded payload {}", embedded.filename))?;
        staged
            .write_all(embedded.bytes)
            .with_context(|| format!("materializing embedded payload {}", embedded.filename))?;
        staged.as_file().sync_all()?;
        #[cfg(unix)]
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o700))?;
        staged
            .persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("installing embedded payload {}", embedded.filename))?;
        std::fs::File::open(&self.root)?.sync_all()?;
        state.paths.insert((payload, arch), path.clone());
        Ok(path)
    }
}

/// Select release payloads when this build embeds them, preserving the
/// explicit no-payload source used by ordinary development builds.
pub(super) fn production_payloads(helm_state_dir: &Path) -> anyhow::Result<Arc<dyn PayloadSource>> {
    Ok(match EmbeddedPayloads::load(helm_state_dir)? {
        Some(payloads) => Arc::new(payloads),
        None => Arc::new(NoPayloads),
    })
}
