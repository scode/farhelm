//! Host-architecture-specific payload sources own release retrieval and
//! materialization so the provisioning executor does not.
//!
//! Three sources exist: [`NoPayloads`] for an ordinary developer build
//! (D13), [`DirectoryPayloads`] for an operator-staged directory
//! (`--payload-dir`), and [`ReleasePayloadSource`] — a verified download
//! over HTTP, which is what a release build uses by default (D2).
//! [`production_payloads`] is the one place that turns a
//! [`PayloadSelection`] plus D13's release-build fact into the
//! `Arc<dyn PayloadSource>` the rest of provisioning drives.

use super::assets;
use super::plan::{PayloadArch, PayloadKind};
use super::release_payloads::{self, MINISIGN_PUBKEY, ReleasePayloadSource};
use anyhow::{Context as _, bail};
use async_trait::async_trait;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Supplies host-architecture-specific artifacts without coupling
/// provisioning to how they are obtained.
///
/// `async` (D16): the download source performs network I/O and awaits a
/// per-asset lock, and every source — including the two here that touch
/// neither — implements the same signature so `prepare_payloads` never has
/// to know which kind of source it is holding.
///
/// `Debug` is a supertrait so that a source, once erased behind
/// `Arc<dyn PayloadSource>`, can still say which one it is: the wiring in
/// [`production_payloads`] is a policy decision (D13) worth logging and
/// worth testing, and the erased handle carries no other way to ask.
/// Implementations should render their identity, not their contents.
#[async_trait]
pub trait PayloadSource: std::fmt::Debug + Send + Sync {
    async fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<PathBuf>;
}

/// Which payload source `run_with_ready` wires up, resolved from `HelmArgs`
/// before it reaches [`production_payloads`].
///
/// `Directory` outranks `Release` (D18: an explicit, local `--payload-dir`
/// is unambiguous, so there is never a reason to let a network source win
/// when both are given) — enforced once, in `HelmArgs::payload_selection`,
/// so nothing downstream of it has to re-decide the precedence.
pub enum PayloadSelection {
    /// Neither flag was given: `production_payloads` picks the default for
    /// this build (D13 — a release-shaped build downloads, a developer
    /// build carries nothing).
    Default,
    /// `--payload-dir`: read published release files staged in this
    /// directory, verifying nothing (D3, operator-trusted).
    Directory(PathBuf),
    /// `--release-base-url`: download from this base URL instead of the
    /// default GitHub release matching this build's version. Selectable on
    /// ANY build (not just a release-shaped one) so tests and air-gapped
    /// mirrors can point at another server.
    Release { base_url: url::Url },
}

/// Development helms intentionally carry no cross-compiled install
/// payloads (D13): a build without an embedded web UI is not release-shaped,
/// so "add host" has nothing to offer beyond the paths this message names.
#[derive(Debug)]
pub(super) struct NoPayloads;

#[async_trait]
impl PayloadSource for NoPayloads {
    async fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
        bail!(
            "this farhelm was built from source and carries no provisioning payloads; pass \
             --payload-dir <dir> holding the release files, or install a release build (see \
             README, \"Install\")"
        )
    }
}

/// Payloads staged by an operator into a plain directory instead of
/// downloaded — for air-gapped installs, mirrors, and tests that would
/// rather not reach GitHub.
///
/// Reads exactly the asset names a GitHub release publishes ([`assets`] is
/// the single source of truth for those names), so the same directory an
/// operator copies release assets into unmodified — or a test populates
/// with tiny fixtures — works with no renaming. Verifies nothing: files in
/// an explicitly selected local directory are treated as operator-trusted
/// and are not verified (D3).
#[derive(Debug)]
pub(super) struct DirectoryPayloads {
    dir: PathBuf,
}

impl DirectoryPayloads {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The private, per-call materialization cache — always below the
    /// operator-supplied directory. See [`PayloadSource::path`]'s impl for
    /// why every call gets its OWN uniquely named output here rather than a
    /// name shared across calls.
    fn extracted_dir(&self) -> PathBuf {
        self.dir.join(".extracted")
    }

    /// A fresh, collision-proof destination for one materialization of
    /// `asset` — `.extracted/<asset>.<random>.bin`.
    ///
    /// F2 (review round 2, DECISION: per-call private snapshot): an earlier
    /// version always published to the SAME pathname
    /// (`.extracted/<asset>.bin`), atomically renamed into place per call.
    /// Atomic rename stops a reader from ever observing a half-written
    /// file, but it does not stop one caller's freshly renamed output from
    /// being overwritten by a DIFFERENT caller's in-flight materialization
    /// before the first caller's own `prepare_payloads` has reopened the
    /// path it was just handed — the provisioning service runs up to four
    /// hosts at once, all sharing one `DirectoryPayloads`. Concretely: run A
    /// starts extracting from the payload directory's current bytes; an
    /// operator refreshes the staged release; run B starts and finishes
    /// extracting the new bytes, publishing them; run A then finishes and
    /// renames ITS (now stale) output over run B's, and B's own
    /// `prepare_payloads` opens the stale generation A actually produced.
    /// A unique filename per call removes the shared name entirely: two
    /// materializations can run concurrently and each caller's path stays
    /// exclusively theirs from the moment `path()` returns it.
    fn unique_extracted_path(&self, asset: &str) -> PathBuf {
        self.extracted_dir()
            .join(format!("{asset}.{}.bin", uuid::Uuid::new_v4()))
    }
}

#[async_trait]
impl PayloadSource for DirectoryPayloads {
    /// Re-materializes `asset` into a brand-new, uniquely named file under
    /// `.extracted/` on EVERY call (F2, review round 1 and 2) rather than
    /// trusting — or sharing — whatever an earlier call already produced.
    /// Not caching at all is deliberate: an "add host" run is a rare,
    /// operator-initiated action touching at most a couple of payloads, so
    /// redoing cheap extraction/copy work every time costs nothing worth
    /// optimizing away, and it is what lets a same-named replacement asset
    /// take effect immediately (round 1) without teaching this source the
    /// content-addressed invalidation machinery [`ReleasePayloadSource`]
    /// needs for its own, genuinely expensive, cache — that source pays a
    /// network round trip a cache hit is worth avoiding, and this one does
    /// not.
    ///
    /// Because every call's output is private, nothing here ever expires it
    /// on the caller's behalf — see [`prune_stale_generations`], run
    /// best-effort after each success, for the only cleanup this directory
    /// gets.
    async fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<PathBuf> {
        let asset = match payload {
            PayloadKind::Farhelm => assets::archive_name(assets::farhelm_archive_for(arch)),
            PayloadKind::Tmux => assets::tmux_name(arch).to_string(),
        };
        let source = self.dir.join(&asset);
        require_regular_file(&source, "the published release asset")?;
        let extracted_dir = self.extracted_dir();
        ensure_private_extracted_dir(&extracted_dir)?;
        let dest = self.unique_extracted_path(&asset);
        // Both branches perform blocking file I/O proportional to a whole
        // binary's size, so both run through `spawn_blocking` rather than
        // stalling whichever tokio worker thread is driving this request.
        match payload {
            PayloadKind::Farhelm => {
                let member = assets::farhelm_archive_for(arch).member;
                let dest_task = dest.clone();
                tokio::task::spawn_blocking(move || {
                    extract_single_member(&source, member, &dest_task)
                })
                .await
                .context("the archive extraction task panicked")??;
            }
            PayloadKind::Tmux => {
                let dest_task = dest.clone();
                tokio::task::spawn_blocking(move || copy_executable(&source, &dest_task))
                    .await
                    .context("the payload copy task panicked")??;
            }
        }
        // Best-effort only: pruning is entirely an optimization (keeping
        // `.extracted/` from growing forever), never load-bearing for
        // correctness, so a panic or error inside it must not fail the
        // provisioning run that just succeeded.
        let prune_dir = extracted_dir.clone();
        let prune_asset = asset.clone();
        let _ = tokio::task::spawn_blocking(move || {
            prune_stale_generations(&prune_dir, &prune_asset, Duration::from_secs(3600))
        })
        .await;
        Ok(dest)
    }
}

/// Remove `.extracted/<asset>.*.bin` snapshots older than `max_age`,
/// swallowing every error along the way.
///
/// The per-call private-snapshot policy (F2, review round 2) means every
/// successful call leaves its own file behind forever unless something
/// prunes it, so this runs after each success rather than as a separate
/// background sweep — an idle helm therefore racks up nothing beyond what a
/// live provisioning run actually produced. The age floor is deliberately
/// conservative: a concurrent run's own fresh snapshot — anywhere between
/// this function's caller returning it and `prepare_payloads` actually
/// reopening it — must never be a pruning candidate, and an hour is far
/// longer than that handoff could plausibly take. Every failure (a read_dir
/// that cannot be opened, a metadata call that fails, a remove that loses a
/// race with something else) is swallowed rather than propagated: failing
/// to prune must never fail provisioning, and the next successful call
/// tries again over whatever is left.
fn prune_stale_generations(extracted_dir: &Path, asset: &str, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(extracted_dir) else {
        return;
    };
    let prefix = format!("{asset}.");
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".bin") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age >= max_age {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Create (or reuse) `.extracted` with Unix mode 0700, regardless of the
/// helm process's umask, and refuse anything that is not plainly ours: a
/// pre-existing symlink or non-directory object at that path.
///
/// F6 (review round 2, security): the materialized binary this directory
/// holds is what `prepare_payloads` opens BY PATHNAME and installs onto the
/// remote host, executed there as the SSH user. `create_dir_all`'s
/// permissions follow the calling process's umask — under a permissive one
/// (`000`), `.extracted` could end up world-writable even though the
/// operator's own `--payload-dir` is otherwise protected. Another local
/// user able to write into `.extracted` could then substitute their own
/// binary for a legitimate materialization during the window between this
/// source publishing a path and `prepare_payloads` reopening it, regardless
/// of how carefully the WRITE into that directory is made atomic — the
/// atomicity protects the bytes, not who else can reach the directory
/// holding them.
///
/// IDEMPOTENT under concurrent first use (F2, review round 3): the
/// provisioning service runs up to four host installs at once, all sharing
/// one `DirectoryPayloads`, so two calls can both observe `.extracted`
/// absent and both attempt to create it. Losing that race is not an error —
/// `DirBuilder::create` reporting `AlreadyExists` means some other call
/// already produced the exact directory this one wanted, so the loser just
/// re-inspects it under the same symlink/non-dir/mode rules an
/// already-existing cache gets on every other call, rather than turning a
/// harmless race into a provisioning failure.
fn ensure_private_extracted_dir(extracted_dir: &Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(extracted_dir) {
        Ok(metadata) => secure_existing_extracted_dir(extracted_dir, &metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match create_extracted_dir(extracted_dir) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata = std::fs::symlink_metadata(extracted_dir).with_context(|| {
                        format!(
                            "inspecting {} after losing the race to create it",
                            extracted_dir.display()
                        )
                    })?;
                    secure_existing_extracted_dir(extracted_dir, &metadata)
                }
                Err(error) => {
                    Err(error).with_context(|| format!("creating {}", extracted_dir.display()))
                }
            }
        }
        Err(error) => Err(error).with_context(|| format!("inspecting {}", extracted_dir.display())),
    }
}

/// The raw, non-idempotent creation attempt [`ensure_private_extracted_dir`]
/// wraps — 0700 from the moment it exists, independent of umask, so there
/// is never a window where the directory is briefly world-accessible.
fn create_extracted_dir(extracted_dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::DirBuilder::new().mode(0o700).create(extracted_dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(extracted_dir)
    }
}

/// Apply [`ensure_private_extracted_dir`]'s acceptance rules to an entry
/// that is already known to exist at `extracted_dir` — shared by the
/// "already existed" and "lost the create race to a concurrent call" paths
/// so both are held to exactly the same standard: reject a symlink (would
/// make `remove_dir_all`-style cleanup elsewhere follow it) or any
/// non-directory object, then (re)assert mode 0700 on Unix regardless of
/// which caller's `symlink_metadata` observed it or under what umask it was
/// originally created.
fn secure_existing_extracted_dir(
    extracted_dir: &Path,
    metadata: &std::fs::Metadata,
) -> anyhow::Result<()> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        bail!(
            "{} is a symlink; refusing to use it as the extraction cache",
            extracted_dir.display()
        );
    }
    if !file_type.is_dir() {
        bail!(
            "{} exists but is not a directory; refusing to use it as the extraction cache",
            extracted_dir.display()
        );
    }
    #[cfg(unix)]
    std::fs::set_permissions(extracted_dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("securing {}", extracted_dir.display()))?;
    Ok(())
}

/// Distinguish "nothing at this path" from "something here that is not a
/// plain file" from "the filesystem could not even be asked" (F5, review
/// round 1) — the three outcomes `Path::is_file()` collapses into a single
/// `false`, which used to report a permission failure or other I/O error on
/// an EXISTING asset exactly as if the asset were simply absent, sending an
/// operator chasing the wrong fix.
///
/// `what` names the role of `path` in the resulting messages (e.g. "the
/// published release asset") so a caller's error reads naturally. Follows
/// symlinks, matching `Path::is_file()`'s own behavior, so an operator
/// staging assets via symlinks from a shared release cache is unaffected —
/// only the ERROR HANDLING changes, not what counts as present.
///
/// Used here only for the staged SOURCE asset. There is no second call site
/// for "the cache destination" the way an earlier draft of this fix had: F2
/// (above) removed the only place that ever inspected the destination
/// before writing it, so there is nothing left to misreport there.
fn require_regular_file(path: &Path, what: &str) -> anyhow::Result<()> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => bail!(
            "{} exists but is not a regular file; {what} must be a plain file",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => bail!(
            "{} does not exist; place {what} there, or point --payload-dir at a directory \
             that holds it",
            path.display()
        ),
        Err(error) => Err(error).with_context(|| format!("inspecting {}", path.display())),
    }
}

/// Extract the single REGULAR-FILE member whose basename is `member` from
/// the `.tar.gz` at `archive` into `dest`, setting mode 0755.
///
/// Shared by [`DirectoryPayloads`] and [`ReleasePayloadSource`] —
/// both extract one binary out of a dist-produced archive that also nests
/// it under a `<package>-<target>/` directory entry (VALIDATE, plan Step
/// 5), so both need this exact "match by basename, refuse unless there is
/// exactly one" rule rather than an exact in-archive path.
///
/// ONE OPEN, ONE PASS (F4, review round 1): an earlier version opened the
/// archive twice — a counting pass, then a separate extraction pass — which
/// meant the two opens were not guaranteed to see the same bytes. An
/// operator replacing the staged archive between them could make the count
/// pass observe one generation while the extraction pass observed another:
/// either the promised match had vanished (an `expect` panic) or the wrong
/// generation's first match was extracted without ever having been counted.
/// Streaming the first REGULAR match to a temp file WHILE STILL COUNTING —
/// persisting it only once the final count is confirmed to be exactly one —
/// closes that particular seam: the archive is opened exactly once, so
/// renaming a DIFFERENT file over `archive`'s pathname mid-function cannot
/// happen (this already-open descriptor keeps referring to whatever inode
/// it opened, whatever a later `rename(2)` does to the name). It does NOT
/// make the opened inode itself immutable (F3, review round 2): a tool that
/// truncates and rewrites the SAME file in place while this function is
/// mid-read can still hand it a stream assembled across two generations of
/// bytes. That narrower guarantee — protection from pathname replacement,
/// not from in-place rewrites of one inode — is what the single open
/// actually buys; no snapshot-and-compare step defends against the latter
/// here, and none is added for it.
///
/// ONLY A REGULAR FILE QUALIFIES (F3, review round 1): a same-named
/// directory, symlink, or hard link is not silently excluded from the count
/// and does not quietly become staged content either — its mere presence
/// makes the archive itself malformed, refused with its own diagnostic.
/// `dist` never legitimately produces such an entry; accepting one would
/// either install non-executable garbage as the payload (if it were the
/// only "match") or corrupt the exactly-one count against a real match
/// sitting elsewhere in the archive.
///
/// Synchronous — `flate2` and `tar` offer no async API — so every caller
/// MUST run this through `spawn_blocking`.
pub(super) fn extract_single_member(
    archive: &Path,
    member: &str,
    dest: &Path,
) -> anyhow::Result<()> {
    let file =
        std::fs::File::open(archive).with_context(|| format!("opening {}", archive.display()))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let asset_name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<archive>");
    let extracted_dir = dest
        .parent()
        .expect("extraction destinations always have a parent");

    let mut matches = 0usize;
    let mut staged: Option<tempfile::NamedTempFile> = None;
    for entry in tar
        .entries()
        .with_context(|| format!("reading {}", archive.display()))?
    {
        let mut entry =
            entry.with_context(|| format!("reading an entry in {}", archive.display()))?;
        let entry_path = entry.path().context("reading a tar entry's path")?;
        if entry_path.file_name().and_then(|name| name.to_str()) != Some(member) {
            continue;
        }
        if !entry.header().entry_type().is_file() {
            bail!(
                "{asset_name} contains a {:?} entry named {member}, not a regular file; \
                 expected exactly one regular file",
                entry.header().entry_type()
            );
        }
        matches += 1;
        if staged.is_none() {
            let mut candidate = tempfile::NamedTempFile::new_in(extracted_dir)
                .with_context(|| format!("staging extracted {member}"))?;
            std::io::copy(&mut entry, candidate.as_file_mut())
                .with_context(|| format!("extracting {member} from {asset_name}"))?;
            staged = Some(candidate);
        }
    }
    if matches != 1 {
        // Dropping `staged` here — if the loop staged a first match before
        // finding a second, or found none at all — deletes its temp file
        // through `NamedTempFile`'s own `Drop`, not anything this function
        // does by hand. Nothing is ever persisted to `dest` on this path.
        bail!("{asset_name} contains {matches} members named {member}; expected exactly one");
    }
    let staged = staged.expect("matches == 1 implies the loop staged exactly one candidate");
    staged.as_file().sync_all()?;
    #[cfg(unix)]
    staged
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    staged
        .persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("installing extracted {member} at {}", dest.display()))?;
    Ok(())
}

/// Copy `source` to `dest` verbatim with mode 0755 — for the one payload
/// kind D5/D14 ship unarchived (tmux). Staged under `dest`'s own directory
/// and renamed into place, matching [`extract_single_member`]'s atomicity so
/// a reader can never observe a partially written binary.
///
/// Shared with the download source for the same reason
/// [`extract_single_member`] is: both sources put every payload kind through
/// one cache shape, so tmux takes this path where a farhelm archive takes
/// the extractor. Synchronous — callers MUST run it through
/// `spawn_blocking`.
pub(super) fn copy_executable(source: &Path, dest: &Path) -> anyhow::Result<()> {
    let mut staged = tempfile::NamedTempFile::new_in(
        dest.parent()
            .expect("extraction destinations always have a parent"),
    )
    .with_context(|| format!("staging {}", dest.display()))?;
    let mut input =
        std::fs::File::open(source).with_context(|| format!("opening {}", source.display()))?;
    std::io::copy(&mut input, staged.as_file_mut())
        .with_context(|| format!("copying {}", source.display()))?;
    staged.as_file().sync_all()?;
    #[cfg(unix)]
    staged
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    staged
        .persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("installing {}", dest.display()))?;
    Ok(())
}

/// Collapse `.`/`..` path components using pure text manipulation — no
/// filesystem access, no symlink resolution — clamping any `..` that would
/// otherwise walk above the filesystem root exactly the way real path
/// resolution does.
///
/// This is deliberately NOT `Path::canonicalize()`: it exists to catch a
/// relationship canonicalization can miss (F1, review round 2). An
/// operator-selected PATHNAME can sit inside the legacy `embedded-payloads`
/// tree even when a symlink somewhere along that pathname resolves outward
/// to an entirely different, populated directory — canonicalizing both
/// sides would follow the symlink and conclude the two paths are unrelated,
/// when the pathname the operator actually typed says otherwise. Two
/// directories that are unrelated once fully resolved can still be "the
/// same tree" from the text of the path an operator gave `--payload-dir`,
/// and it is that text startup cleanup must never delete part of.
///
/// The root-clamping matters for the same reason (F1, review round 3): the
/// kernel treats `/..` as `/`, not as an escape to a nonexistent
/// grandparent, so a normalizer that instead PRESERVED a leading `..` past
/// the root would disagree with where a pathname like `/../<legacy path>`
/// actually resolves — letting it slip past the alias guard below as
/// though it named something unrelated to `<legacy path>`.
///
/// Only meaningful on an ABSOLUTE input; callers must make `path` absolute
/// first (see [`make_absolute`]) so every leading `..` this function sees
/// is either clamped at a real root or cancels a preceding real segment,
/// never left dangling at the front of a still-relative result.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                // Already at the root: `..` goes nowhere, so drop it
                // instead of appending a literal `..` past `/`.
                Some(Component::RootDir) => {}
                _ => normalized.push(component.as_os_str()),
            },
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Make `path` absolute by prepending `cwd` when it is not already —
/// pure text, like [`normalize_lexical`]: no symlink resolution, no
/// filesystem access.
///
/// `--payload-dir` is an ordinary clap argument, so an operator can spell
/// it relative to wherever they happened to invoke `farhelm helm run`
/// from. [`selected_directory_aliases_legacy_cache`]'s lexical comparison
/// only means anything once both sides are expressed in the same
/// coordinate space — the legacy cache path is always already absolute
/// (built from `helm_state_dir`), so a relative selection has to be
/// anchored the same way before the two are compared as text.
fn make_absolute(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// Whether `selected` (an operator-chosen `--payload-dir`, resolved against
/// `cwd` — see [`make_absolute`]) aliases the legacy `embedded-payloads`
/// cache, checked two ways (F1, review round 2):
///
/// (a) LEXICALLY — `selected` equals or sits beneath `legacy` as plain text,
///     once both are made absolute and `.`/`..` are collapsed — catches a
///     selected pathname inside the legacy tree even when a symlink
///     somewhere along it resolves outward.
/// (b) CANONICALLY — the two paths' RESOLVED targets coincide or nest —
///     catches the opposite case, a selected pathname that sits somewhere
///     else entirely but whose target is the legacy cache via a symlink
///     alias.
///
/// Either relationship alone is reason enough to preserve the cache; ONLY
/// when NEITHER holds does cleanup proceed. Round 1 checked only (b), which
/// is what let (a)'s case slip through: canonicalizing an outward-pointing
/// symlink resolves it away from the legacy tree entirely, so the
/// canonical-only check saw two unrelated directories and let the
/// unconditional `remove_dir_all` run.
///
/// Both checks canonicalize the ALREADY-ABSOLUTE form of each path, never
/// the caller-supplied `legacy`/`selected` directly (F1, review round 3):
/// `Path::canonicalize` resolves a relative input against the REAL process
/// cwd, which is exactly the mismatch this function exists to close if
/// `selected` is relative and `cwd` is something else (a test's stand-in
/// cwd, for instance).
///
/// The canonical half returns `false` when either side cannot be resolved
/// because it does not exist yet: an operator pointing `--payload-dir` at a
/// directory this run would create cannot already be aliasing a legacy
/// cache that predates it. Any OTHER canonicalization failure — on EITHER
/// input — propagates rather than being treated as "not aliased" (F1,
/// review round 3 fixes a fail-open here: round 2 swallowed every legacy-side
/// canonicalization error, permission failures included, as "not an alias")
/// — this function must never let genuine uncertainty about either path
/// resolve toward deleting something.
fn selected_directory_aliases_legacy_cache(
    cwd: &Path,
    legacy: &Path,
    selected: &Path,
) -> anyhow::Result<bool> {
    let legacy_absolute = make_absolute(cwd, legacy);
    let selected_absolute = make_absolute(cwd, selected);
    let legacy_lexical = normalize_lexical(&legacy_absolute);
    let selected_lexical = normalize_lexical(&selected_absolute);
    if selected_lexical == legacy_lexical || selected_lexical.starts_with(&legacy_lexical) {
        return Ok(true);
    }

    let legacy_canonical = match legacy_absolute.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("resolving legacy cache {}", legacy.display()));
        }
    };
    match selected_absolute.canonicalize() {
        Ok(selected_canonical) => Ok(selected_canonical == legacy_canonical
            || selected_canonical.starts_with(&legacy_canonical)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("resolving --payload-dir {}", selected.display()))
        }
    }
}

/// Remove a leftover `<state_dir>/embedded-payloads/` cache from a build
/// that predates D2, if this state directory carries one AND it is safe to
/// do so without risking data that is not ours.
///
/// F1 (review round 1, BLOCKING; round 2 widens the guard): `--payload-dir`
/// accepts an arbitrary filesystem path with nothing stopping an operator
/// migrating from a pre-D2 install from pointing it at
/// `<state_dir>/embedded-payloads` — it is, after all, already sitting
/// right there with binaries in it — or at a path or symlink that aliases
/// it either lexically or once resolved. Deleting that path unconditionally
/// before even looking at the selection would destroy the operator's
/// explicitly staged release files and leave `DirectoryPayloads` pointed at
/// an unreachable path, turning a migration courtesy into data loss on the
/// exact directory the operator just told Farhelm to use.
///
/// Two guards run before any deletion is even considered, in order:
///
/// 1. The legacy entry itself must be an ordinary directory, inspected with
///    `symlink_metadata` (never `metadata`, which would follow a symlink
///    there and inspect its TARGET instead of the entry itself). Absent
///    (`NotFound`) means nothing to clean. Present but a symlink, or
///    present but not a directory at all — a dangling symlink included,
///    since `symlink_metadata` reports the link itself regardless of
///    whether its target exists — means this function cannot be certain
///    `remove_dir_all` would remove only what it thinks it would (a symlink
///    passed to `remove_dir_all` deletes whatever it points at, not the
///    link), so it leaves the entry alone and warns instead. Any OTHER
///    metadata failure propagates rather than being treated as "safe to
///    delete" — see this function's caller-facing contract: uncertainty
///    never resolves toward deletion.
/// 2. Only once the legacy entry is confirmed to be a plain directory does
///    the operator's selection get checked for aliasing
///    ([`selected_directory_aliases_legacy_cache`], resolved against `cwd`
///    so a RELATIVE `--payload-dir` — clap accepts one unchanged — is
///    compared in the same coordinate space as the always-absolute legacy
///    path). If it aliases, cleanup is skipped ENTIRELY (nothing is
///    deleted, not even files that would not have collided) and a `warn!`
///    tells the operator to move `--payload-dir` elsewhere so the retired
///    cache can be cleaned up on a later run.
///
/// Every source since D2 materializes below a name of its own instead
/// ([`DirectoryPayloads`]'s `.extracted/` sits inside the operator's own
/// `--payload-dir`, not helm state), so a directory still called
/// `embedded-payloads` under helm state — an ordinary directory the
/// operator did NOT just select — can only be dead weight left by the
/// retired `EmbeddedPayloads` source, safe to remove.
fn remove_leftover_embedded_payloads(
    cwd: &Path,
    helm_state_dir: &Path,
    selected_directory: Option<&Path>,
) -> anyhow::Result<()> {
    let leftover = helm_state_dir.join("embedded-payloads");
    let legacy_type = match std::fs::symlink_metadata(&leftover) {
        Ok(metadata) => metadata.file_type(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("inspecting {}", leftover.display()));
        }
    };
    if !legacy_type.is_dir() {
        warn!(
            path = %leftover.display(),
            "a leftover pre-download provisioning payload cache exists but is not a plain \
             directory (a symlink or other object, dangling or not); leaving it in place rather \
             than removing something that might not be ours to remove"
        );
        return Ok(());
    }

    if let Some(selected) = selected_directory
        && selected_directory_aliases_legacy_cache(cwd, &leftover, selected)?
    {
        warn!(
            payload_dir = %selected.display(),
            legacy_cache = %leftover.display(),
            "--payload-dir points at (or inside) the retired embedded-payloads cache; leaving \
             it in place rather than deleting staged release files — move --payload-dir to a \
             different directory so this leftover cache can be cleaned up"
        );
        return Ok(());
    }
    match std::fs::remove_dir_all(&leftover) {
        Ok(()) => {
            info!(
                path = %leftover.display(),
                "removed a leftover pre-download provisioning payload cache"
            );
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("removing leftover {}", leftover.display()))
        }
    }
}

/// Where a release-shaped build looks for its payloads when nothing on the
/// command line says otherwise: the GitHub release tagged with this build's
/// own version (D2).
///
/// Built from `CARGO_PKG_VERSION` rather than written out, so a version bump
/// cannot leave the default pointing at the previous release's assets — the
/// failure that would silently provision hosts with a mismatched binary.
fn default_release_base_url() -> anyhow::Result<url::Url> {
    let version = env!("CARGO_PKG_VERSION");
    url::Url::parse(&format!(
        "https://github.com/scode/farhelm/releases/download/v{version}/"
    ))
    .context("building the default release download URL")
}

/// Every setting the release download client is built with.
///
/// The timeouts are the shape of the workload (plan Step 3): a connect
/// either happens quickly or is not going to, so 15 s is generous; a body
/// read that makes no progress for 60 s is stalled rather than slow. There
/// is deliberately NO overall deadline — a release archive is tens of
/// megabytes and a genuinely slow link must be allowed to finish, since the
/// alternative is an "add host" that fails at the same point every time.
///
/// `retry(never())` is load-bearing, not tidiness. reqwest 0.12 retries safe
/// protocol nacks on its own, up to twice per `send()`, which would make the
/// download source's documented "exactly one connect retry" a statement
/// about only one of two retry layers. Disabling reqwest's leaves
/// `ReleasePayloadSource::get` the single place attempts are decided.
/// Deliberately UNTESTED: reqwest only retries at a protocol layer an
/// in-process fixture server cannot reach into, so a test would have to
/// assert on reqwest's internals rather than on farhelm's behaviour. The
/// scripted transport covers farhelm's own retry; this line is a settled
/// decision recorded here rather than a claim a test defends.
///
/// A BUILDER rather than a finished client so tests can add `no_proxy()` —
/// reqwest honours the ambient proxy variables, so a loopback fixture URL is
/// not by itself a guarantee that no socket leaves the machine — without
/// forking the settings and letting them drift from production.
pub(super) fn release_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(60))
        .retry(reqwest::retry::never())
        .redirect(release_redirect_policy())
        .user_agent(concat!("farhelm/", env!("CARGO_PKG_VERSION")))
}

/// Redirects must be followed — GitHub answers every release asset URL with
/// one, pointing at its object store — but not unconditionally.
///
/// Two bounds, and deliberately only two:
///
/// - At most five hops. reqwest's default is ten; one hop is the real case,
///   and a chain longer than five is a loop or a game, not a CDN.
/// - No https → http DOWNGRADE. A redirect is chosen by the server, so
///   without this an operator who carefully typed an `https://` base URL
///   could be walked onto a plaintext connection where anything on the path
///   can see and rewrite the response. The signature check would still refuse
///   the result, but the request has already been made by then, and "we
///   refused the answer" is not the same as "we did not ask".
///
/// What is NOT blocked: loopback, private, and link-local destinations. That
/// is a considered choice rather than an oversight — `--release-base-url`
/// exists precisely so a helm can be pointed at an internal mirror or a test
/// fixture on `127.0.0.1`, and a policy that refused those would break the
/// supported case in order to narrow an unsupported one. The SSRF surface
/// that remains is a GET whose response can never become a payload: it must
/// still hash to an entry in a manifest signed for this version.
fn release_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        // `previous()[0]` is the URL farhelm actually asked for, so the
        // scheme comparison is against the operator's own base URL rather
        // than against whatever the previous hop happened to be.
        match release_redirect_decision(
            attempt.previous().first(),
            attempt.url(),
            attempt.previous().len(),
        ) {
            RedirectDecision::Follow => attempt.follow(),
            RedirectDecision::TooManyHops => {
                attempt.error("too many redirects for a release asset")
            }
            RedirectDecision::Downgrade => attempt
                .error("refusing a release redirect that downgrades https to a plaintext scheme"),
        }
    })
}

/// What [`release_redirect_policy`] does with one hop.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RedirectDecision {
    Follow,
    TooManyHops,
    Downgrade,
}

/// The redirect policy as a pure decision, separated from reqwest's types so
/// it can be tested at all: `reqwest::redirect::Attempt` has no public
/// constructor, so a test cannot drive `Policy::redirect` directly. The
/// end-to-end loopback redirect test covers the wiring; this covers the rule.
pub(super) fn release_redirect_decision(
    origin: Option<&url::Url>,
    next: &url::Url,
    hops: usize,
) -> RedirectDecision {
    if hops >= 5 {
        return RedirectDecision::TooManyHops;
    }
    let asked_for_https = origin.is_some_and(|url| url.scheme() == "https");
    if asked_for_https && next.scheme() != "https" {
        return RedirectDecision::Downgrade;
    }
    RedirectDecision::Follow
}

/// The HTTP client every production download goes through.
pub(super) fn release_client() -> anyhow::Result<reqwest::Client> {
    release_client_builder()
        .build()
        .context("building the release download HTTP client")
}

/// Select the payload source production wiring builds, from a
/// [`PayloadSelection`] and D13's release-build fact.
///
/// This function IS the policy (D13): a release-shaped build downloads by
/// default and a developer build refuses by default, while
/// `--release-base-url` selects a download source on either kind of build
/// so tests, mirrors, and prerelease checks have a way in that does not
/// require faking a release build.
///
/// `cwd` (F1, review round 3) is the directory a relative `--payload-dir`
/// is spelled against — production wiring passes the real
/// `std::env::current_dir()`; tests pass a tempdir so a relative selection
/// can be exercised without ever mutating this process's actual working
/// directory. It exists solely for [`remove_leftover_embedded_payloads`]'s
/// alias guard; nothing else this function does depends on it.
pub(super) fn production_payloads(
    selection: PayloadSelection,
    helm_state_dir: &Path,
    release_build: bool,
    cwd: &Path,
) -> anyhow::Result<Arc<dyn PayloadSource>> {
    production_payloads_with_key(
        selection,
        helm_state_dir,
        release_build,
        cwd,
        // The ONE production call site release_payloads::VERSION's
        // docstring points at: everything downstream of `ReleasePayloadSource`
        // learns the expected version from this argument, never from reading
        // a constant of its own, so this is the only line a future refactor
        // could get wrong and stop shipping `CARGO_PKG_VERSION`.
        release_payloads::VERSION,
        MINISIGN_PUBKEY,
        release_client()?,
    )
}

/// [`production_payloads`] with the trust anchor and HTTP client named
/// explicitly.
///
/// The split exists so the end-to-end provisioning test can drive this
/// EXACT policy — the same arms, cache root and leftover cleanup — against a
/// loopback release signed with a throwaway key. Two things have to be
/// injected for that to be honest:
///
/// - `pubkey`, because the production secret key lives only as a repository
///   secret, so no test can produce a signature [`MINISIGN_PUBKEY`] accepts.
///   That the shipped binary passes the real constant is covered separately,
///   by the key's own oracle test.
/// - `client`, because reqwest honours the ambient proxy variables: a
///   loopback fixture URL alone does not guarantee the socket stays on this
///   machine, and no test in this repository may mutate the environment to
///   arrange that. Tests pass `release_client_builder().no_proxy()`;
///   production passes [`release_client`], and everything else about the
///   client is shared so the two cannot drift.
///
/// `cwd` means exactly what it does on [`production_payloads`]; it is
/// threaded through unchanged.
///
/// `version` is likewise named explicitly rather than read from
/// [`release_payloads::VERSION`] internally, for the same reason `pubkey`
/// and `client` are: the end-to-end provisioning test drives real fixtures
/// signed for `release_payloads::test_support::FIXTURE_VERSION`, a
/// deliberately different value from whatever the workspace version
/// happens to be (see that constant's docstring for why re-signing the
/// fixtures on every release bump is not acceptable). Production always
/// passes [`release_payloads::VERSION`].
pub(super) fn production_payloads_with_key(
    selection: PayloadSelection,
    helm_state_dir: &Path,
    release_build: bool,
    cwd: &Path,
    version: impl Into<String>,
    pubkey: &'static str,
    client: reqwest::Client,
) -> anyhow::Result<Arc<dyn PayloadSource>> {
    // Only `Directory` can collide with the retired embedded-payloads cache,
    // and the guard needs the exact directory to compare against — see
    // [`remove_leftover_embedded_payloads`] for why deleting staged release
    // files out from under an operator is the failure it exists to prevent.
    let selected_directory = match &selection {
        PayloadSelection::Directory(dir) => Some(dir.as_path()),
        PayloadSelection::Default | PayloadSelection::Release { .. } => None,
    };
    remove_leftover_embedded_payloads(cwd, helm_state_dir, selected_directory)?;
    // The two selections that never download are answered first; what
    // remains differs only in WHICH base URL to read, so the release source
    // is constructed exactly once and a future change to its constructor has
    // one call site to keep right.
    let base_url = match selection {
        PayloadSelection::Directory(dir) => return Ok(Arc::new(DirectoryPayloads::new(dir))),
        PayloadSelection::Default if !release_build => return Ok(Arc::new(NoPayloads)),
        PayloadSelection::Release { base_url } => base_url,
        PayloadSelection::Default => default_release_base_url()?,
    };
    // All release sources share one cache parent; `ReleasePayloadSource`
    // then keys a directory below it by version and base URL, so a fixture
    // or mirror URL can never read what a real release wrote.
    Ok(Arc::new(ReleasePayloadSource::new(
        base_url,
        helm_state_dir.join("payloads"),
        version,
        pubkey,
        client,
    )))
}
