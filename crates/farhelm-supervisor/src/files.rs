//! State-file write atomicity policy (PLAN_M3.md item 5).
//!
//! Every path the supervisor or the launch shim writes directly (never
//! through SQLite, which owns its own durability) is enumerated here and
//! assigned one of three tiers. The tier answers a single question a
//! reader must be able to rely on at any point in time, including right
//! after a crash or a host reboot: **what is at this path — complete old
//! content, complete new content, or nothing at all?** A reader must never
//! be able to observe a truncated mixture of the two. Which failure
//! classes a tier defends against, and at what cost, is what distinguishes
//! the tiers below; SQLite's own durability is SQLite's, and is out of
//! scope here.
//!
//! # Tiers
//!
//! **Durability-bearing** ([`write_durable_sync`]). Full sequence: write a
//! fresh 0600 temp file, `fsync` it, `rename` it over the destination
//! (REPLACING whatever was there), `fsync` the parent directory. The
//! directory fsync matters because `rename`'s atomicity is a metadata-only
//! guarantee — without it, a crash right after `rename` can still lose the
//! directory-entry update on some filesystems/mount options, silently
//! reverting the destination to its pre-rename content (or absence) after
//! reboot. This tier exists for exactly one file class today: the launch
//! shim's exec-failure sentinel (`launch/<id>.status`, `crate::launch`
//! module). Losing it flips a session's later classification from
//! **error** to **exited** — the difference between "your invocation is
//! broken" and "your agent ran and finished" (PLAN_M3.md item 3) — so it
//! must survive not just a supervisor crash but a full host reboot with
//! the same fidelity SQLite gives the rest of the session's metadata.
//!
//! **Best-effort atomic** ([`overwrite_private_file_sync`]). Write a fresh 0600
//! temp file, `fsync` it, `rename` it over the destination; no fsync of
//! the parent directory. Today's one file class: the generated tmux config
//! (`tmux.rs`'s `TmuxDriver::ensure_server`). "Losable, never torn": the
//! file fsync means a torn/truncated INODE can never survive a power loss
//! (unlike a bare `rename` with no file fsync at all, which can still
//! persist a partially-flushed inode depending on write-back timing), but
//! the missing DIRECTORY fsync means the whole update — the directory
//! entry now pointing at the new inode — can still be lost outright,
//! reverting the destination to whatever it pointed at before. That is an
//! acceptable loss for this file class: a lost tmux-config update is
//! rebuilt from `TmuxDriver::config_body` on the very next
//! `ensure_server` call regardless. Paying for a directory fsync on every
//! server start to protect trivially regenerable state would be spending
//! durability budget where nothing durability-sensitive is at stake.
//!
//! **Atomic-publication-before-launch** ([`write_private_file`]). Per-
//! launch spec files (`launch/<id>.json`, `crate::launch::LaunchSpec`)
//! are their OWN tier, not a shorthand for either of the above, despite
//! having the shortest life of anything here: PLAN_M3.md item 5 calls
//! this out explicitly, because the shim learns the sentinel's OWN path
//! from the spec (`crate::launch::status_path_for_spec`) — a torn or
//! missing spec is not "regenerate and move on" the way a lost tmux
//! config is, it silently converts a would-be **error** classification
//! into a plain, wrong **exited**. No fsync at all (a supervisor crash
//! between publishing the spec and launching the tmux window that reads
//! it means the window was never created, so nothing will ever read a
//! torn spec regardless of what made it to disk), but the destination
//! must NEVER show a partial file: publication happens by writing a temp
//! file then hard-`link`-ing it into place, which — unlike `rename` —
//! FAILS if the destination already exists, because unlike the other two
//! tiers this one must REFUSE a name collision rather than silently
//! replace it (spec names are fresh per-launch UUIDs; a collision means
//! something is impersonating the supervisor, not retrying a write).
//!
//! **Out of band, deliberately**: `supervisor.lock`. It is opened with
//! `OpenOptions::create(true)` and never written to at all — its entire
//! job is to exist as an `flock` target proving supervisor exclusivity
//! (`service.rs`'s `Supervisor::serve`), so "torn or complete" is not a
//! meaningful question for it (it carries no content whose completeness
//! could vary); the only property it needs is that `flock` itself works,
//! which none of this module's machinery is about.
//!
//! # The fault-injection seam
//!
//! [`FaultSeam`] MEDIATES the four operations a staged write performs —
//! it does not merely observe them after the fact. `write_staged` calls
//! `seam.write(..)`, `seam.fsync_file(..)`, `seam.rename(..)` or
//! `seam.link(..)`, and `seam.fsync_dir(..)` instead of calling
//! `File::write_all`/`sync_all`/`std::fs::rename`/`std::fs::hard_link`
//! directly; [`RealFs`]'s default method bodies ARE those real syscalls,
//! so production code (which always uses `RealFs`) performs exactly what
//! it always did. A test seam can wrap `RealFs` and simply RECORD each
//! call before delegating (proving the true operation sequence, not a
//! parallel marker trail that could silently drift from what actually
//! ran), or override a method to fail WITHOUT delegating (faithfully
//! simulating a crash that prevents that specific operation from ever
//! happening, rather than one that lets it complete and merely reports
//! failure afterward).
//!
//! This distinction is the whole reason for mediating rather than
//! observing: a callback fired immediately after a hard-coded
//! `file.sync_all()` call can prove a marker fired in the right order,
//! but a regression that deletes the `sync_all()` line while leaving the
//! callback in place would sail through such a test undetected — the
//! marker and the syscall it claims to describe would have silently come
//! unglued. With the seam owning the call itself, there is only one call
//! to remove, and removing it is exactly what a wrapping/recording test
//! seam is watching for.
//!
//! # Streaming, for content that must never be held in memory
//!
//! [`StagedStream`] is the same policy with the bytes arriving over time
//! instead of all at once (PLAN_M4.md item 4: attachment uploads). It
//! stages into the identical `.<name>.tmp-<uuid>` file, drives the
//! identical [`FaultSeam`] stages — write (once per chunk), flush, publish,
//! cleanup — and differs in exactly two respects, both forced by what it
//! is for:
//!
//! - **Nothing is ever materialized.** The whole-file helpers take a
//!   `&[u8]` and therefore require the caller to already hold the content;
//!   an upload has no size cap (SPEC_impl.md), so its content must never
//!   be a value anything holds. A chunk is written and forgotten.
//! - **Staging and publication are separate DIRECTORIES.** The whole-file
//!   tiers stage beside their destination, which is safe only because
//!   their directories hold names this code chose (UUIDs, which cannot
//!   contain `.tmp-`). An attachment's published names come from the user,
//!   so no naming convention can separate debris from content — the
//!   separation has to be positional. See [`StagedStream::create`].
//! - **Publication is NO-CLOBBER, and retries under a different name.**
//!   The whole-file tiers either replace (`rename`) or refuse (`link`) on
//!   collision. An attachment must do neither: two concurrent uploads that
//!   both proposed `screenshot.png` must BOTH publish, under distinct
//!   paths, never one silently replacing the other and never one failing.
//!   So publication is `link` (atomic, fails `EEXIST`) walked over a
//!   sequence of candidate names — the collision is resolved by the
//!   filesystem's own atomicity rather than by a check-then-create the
//!   other uploader could interleave with.
//!
//! Durability sits at the best-effort-atomic tier: the temp file is
//! fsync'd before publication (so a torn file is never observable at the
//! published path) and the directory is not (a crash mid-upload simply
//! loses the upload, which the client already sees as a failed transfer).
//!
//! # Orphaned temp files
//!
//! Every helper here cleans up its own staged temp file when a write,
//! fsync, or publish step fails (`remove_temp_after_failure`) — but
//! only once this call has established that it actually OWNS that temp
//! file (its own `create_new` succeeded); an `open` failure from a name
//! COLLISION belongs to whatever invocation actually created that file,
//! and must be returned untouched rather than deleting content this call
//! never wrote. Even so, cleanup only runs if the process is still alive
//! to run it — a harder crash (OOM kill, `kill -9`, power loss) between
//! staging the temp file and either publishing or reaching the failure
//! path skips it entirely. Every temp file this module creates shares one
//! naming convention — `.<destination-file-name>.tmp-<uuid>` — so a
//! single backstop-sweep pattern ([`is_staged_temp_name`]) covers every
//! tier: `service.rs`'s narrowed launch-dir sweep and its
//! tmux-config-temp sweep both key off it. `crate::attachments`
//! deliberately does NOT: its directories hold
//! user-chosen filenames, so it separates staging from publication
//! positionally instead (see [`StagedStream::create`]) and sweeps a
//! reserved directory rather than a name pattern.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Mediates the four filesystem operations a staged write performs,
/// rather than merely being notified after each one — see the module
/// docs' "fault-injection seam" section for why that distinction is
/// load-bearing, not stylistic. Every method's default body IS the real
/// syscall; [`RealFs`] uses every default and is what production code
/// always passes. A method takes exactly what it needs to perform the
/// operation for real, so a wrapping test seam can trivially delegate to
/// the same default after recording its own observation.
///
/// Deliberately NOT `Send + Sync`: nothing in this crate needs a
/// `FaultSeam` trait object to cross a thread boundary. `write_staged`
/// is synchronous end-to-end and never holds a seam reference across an
/// `.await`; the async wrapper ([`write_private_file`]) constructs a
/// fresh [`RealFs`] INSIDE its `spawn_blocking` closure rather than
/// moving a `dyn FaultSeam` into it,
/// so only the concrete, trivially-`Send` `RealFs` ever crosses that
/// boundary. Test seams are therefore free to use plain interior
/// mutability (`RefCell`) instead of a `Mutex`.
pub trait FaultSeam {
    /// Write `bytes` to the staged temp file.
    fn write(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }

    /// `fsync` the staged temp file. Called for the durability-bearing and
    /// best-effort-atomic tiers, never for atomic-publication (see the
    /// module docs for why that tier owes no fsync at all).
    fn fsync_file(&self, file: &File) -> io::Result<()> {
        file.sync_all()
    }

    /// Publish by RENAMING the temp file over the destination — durability-
    /// bearing and best-effort-atomic tiers, both of which REPLACE
    /// whatever was previously at the destination.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    /// Publish by hard-LINKING the temp file to the destination name —
    /// the atomic-publication tier, which must REFUSE a destination that
    /// already exists (`link` fails with `EEXIST`) rather than silently
    /// replace it.
    fn link(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::hard_link(from, to)
    }

    /// `fsync` the destination's parent directory, forcing the `rename`'s
    /// directory-entry update itself to disk. Called only for the
    /// durability-bearing tier.
    ///
    /// `File::sync_all` on a directory descriptor is the standard Linux
    /// mechanism for this (this crate's only supported platform today —
    /// `lib.rs`'s own module docs are unconditionally unix-only). macOS
    /// requires the nonstandard `F_FULLFSYNC` fcntl for durability
    /// guarantees this strong; SPEC_impl.md already defers the macOS
    /// process-sweep gap to the future Mac-supervisor milestone, and this
    /// directory-fsync mechanism belongs on that same list, not this one.
    fn fsync_dir(&self, dir: &File) -> io::Result<()> {
        dir.sync_all()
    }

    /// Remove a staged temp file this call owns, on the failure path (or,
    /// for [`StagedStream`], on the abort path — an abandoned upload's
    /// temp is debris the moment the transfer gives up, not at the next
    /// startup sweep).
    ///
    /// Mediated like the other four so a test can prove the CLEANUP half
    /// of the contract as well as the publish half: that a failed write is
    /// followed by a removal at all, and that a removal which itself fails
    /// is folded into the returned error rather than silently dropped.
    /// Without a seam here the second half is unreachable from a test
    /// (making `remove_file` fail for real means engineering a permission
    /// or mount condition around a temp directory).
    fn remove_temp(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

/// The real filesystem: every production call site's seam. Zero-sized and
/// `Copy` because it carries no state — every method is the trait's
/// default, i.e. the real syscall.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFs;

impl FaultSeam for RealFs {}

/// Which write-atomicity tier [`write_staged`] is performing: which
/// windows the seam exercises, and how the temp file gets published. See
/// the module doc's tier table for the policy each corresponds to and why.
enum Tier {
    Durable,
    BestEffort,
    AtomicPublication,
}

impl Tier {
    /// Whether this tier's temp file gets `fsync`'d before publication.
    /// `AtomicPublication` (launch specs) is the one tier that does not:
    /// see the module docs for why it needs no fsync at all.
    fn fsyncs_file(&self) -> bool {
        matches!(self, Tier::Durable | Tier::BestEffort)
    }

    /// Whether the destination's parent directory gets `fsync`'d after
    /// publication. Only `Durable` pays this cost; see the module docs.
    fn fsyncs_dir(&self) -> bool {
        matches!(self, Tier::Durable)
    }
}

/// A destination path's temp-staging name, shared by every tier so
/// [`is_staged_temp_name`] recognizes debris from any of them.
fn temp_path_for(path: &Path, dir: &Path) -> PathBuf {
    let temp_name = format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("write"),
        uuid::Uuid::new_v4()
    );
    dir.join(temp_name)
}

/// Whether a directory entry's name LOOKS LIKE this module's temp-staging
/// convention. This is a substring check (`.tmp-` appearing anywhere), not
/// a full parse of the `.<name>.tmp-<uuid>` shape — deliberately: every
/// backstop sweep that uses this only needs to tell "definitely one of our
/// staged temp files" apart from "a real destination name" (a session id,
/// a launch id — both plain UUIDs, which by construction never contain
/// the literal substring `.tmp-`), and a substring check is enough for
/// that without committing to (and having to keep in sync with) the exact
/// UUID-formatting details of `temp_path_for`.
pub fn is_staged_temp_name(name: &str) -> bool {
    name.contains(".tmp-")
}

/// Resolve `path`'s parent as a directory to stage a temp file in and
/// later `fsync`, normalizing the one surprising case `Path::parent`
/// itself does not: `Path::new("a-bare-filename").parent()` returns
/// `Some("")` (an EMPTY path), not `None` — only a path with no parent
/// AT ALL (`/`, or an already-empty path) returns `None`. An empty parent
/// is not a usable directory to `File::open` for the directory fsync (it
/// is not "no parent", it IS the current working directory, just spelled
/// as `""` rather than `"."`), so it is normalized to `"."` here rather
/// than left to fail opening `""` — which would happen AFTER the rename
/// already published the new content, turning a successful publish into
/// a reported failure for no reason.
fn parent_dir(path: &Path) -> io::Result<PathBuf> {
    match path.parent() {
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} has no parent directory to stage a temp file in",
                path.display()
            ),
        )),
        Some(p) if p.as_os_str().is_empty() => Ok(PathBuf::from(".")),
        Some(p) => Ok(p.to_path_buf()),
    }
}

/// Remove a staged temp file after its OWN write, fsync, or publish
/// already failed, folding a cleanup failure into the RETURNED error
/// rather than silently discarding it.
///
/// Callers must only reach this once they have established OWNERSHIP of
/// `temp_path` (their own `create_new` open succeeded) — never on an
/// `open` failure itself, which can mean a name COLLISION with some other
/// invocation's in-flight temp file, whose content this call never wrote
/// and has no right to delete.
///
/// A `remove_file` that itself fails here (permissions, a concurrent
/// removal racing this one) means the temp file may still be sitting on
/// disk — exactly the debris this function exists to prevent — so the
/// caller needs to learn that too, not just the original failure.
/// `original`'s `kind()` is preserved on the combined error (rather than
/// the cleanup error's own kind) because the write/publish failure is the
/// causally primary one; the cleanup failure is context layered on top.
fn remove_temp_after_failure(
    temp_path: &Path,
    seam: &dyn FaultSeam,
    original: io::Error,
) -> io::Error {
    match seam.remove_temp(temp_path) {
        Ok(()) => original,
        Err(cleanup_error) => io::Error::new(
            original.kind(),
            format!(
                "{original}; could not remove staged temp file {}: {cleanup_error}",
                temp_path.display()
            ),
        ),
    }
}

/// Create the 0600 temp file a staged write (whole-file or streaming)
/// works through, and defend its mode against the process umask.
///
/// Shared by [`write_staged`] and [`StagedStream::create`] so the two
/// cannot drift on the property that matters most here: the file is
/// created `create_new` (never opening whatever a name collision would
/// have pointed at) AND `fchmod`'d afterwards, because
/// `OpenOptions::mode` is masked DOWN by the umask, never up. A
/// restrictive umask does not stop the CREATING process from writing
/// (its permission check already happened at `open`) but would leave the
/// published file unreadable to whoever opens it fresh later — a
/// restarted supervisor reading a sentinel, the shim reading a spec, or
/// an agent reading the attachment a user just dropped. `fchmod`, unlike
/// the creating `open`, is never subject to umask.
///
/// An `open` failure is returned RAW, with no cleanup attempt: it means a
/// collision with some OTHER invocation's in-flight temp file, whose
/// content this call never wrote and has no right to delete (see
/// [`remove_temp_after_failure`]).
fn open_staged_temp(temp_path: &Path, seam: &dyn FaultSeam) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true).mode(0o600);
    let file = opts.open(temp_path)?;
    if let Err(e) = file.set_permissions(std::fs::Permissions::from_mode(0o600)) {
        return Err(remove_temp_after_failure(temp_path, seam, e));
    }
    Ok(file)
}

/// The shared engine behind all three tiers: stage a fresh 0600 temp
/// file, write `bytes`, optionally `fsync` it, publish it at `path` (by
/// rename or by link, per `tier`), and optionally `fsync` the parent
/// directory — every real operation performed BY `seam` (see [`FaultSeam`]
/// for why), so a test seam observes the true sequence and can inject a
/// failure IN an operation rather than merely after it.
fn write_staged(path: &Path, bytes: &[u8], seam: &dyn FaultSeam, tier: Tier) -> io::Result<()> {
    let dir = parent_dir(path)?;
    let temp_path = temp_path_for(path, &dir);

    let mut file = open_staged_temp(&temp_path, seam)?;

    let staged: io::Result<()> = (|| {
        seam.write(&mut file, bytes)?;
        if tier.fsyncs_file() {
            seam.fsync_file(&file)?;
        }
        Ok(())
    })();
    if let Err(e) = staged {
        return Err(remove_temp_after_failure(&temp_path, seam, e));
    }

    match tier {
        Tier::AtomicPublication => {
            if let Err(e) = seam.link(&temp_path, path) {
                return Err(remove_temp_after_failure(&temp_path, seam, e));
            }
            // The destination already holds the complete content via the
            // hard link, independent of this name's fate: removing the
            // temp name is cosmetic cleanup, not part of the atomicity
            // contract, so its own failure is debris for the launch-dir
            // sweep to catch later, not a caller-visible error.
            let _ = fs::remove_file(&temp_path);
        }
        Tier::Durable | Tier::BestEffort => {
            if let Err(e) = seam.rename(&temp_path, path) {
                return Err(remove_temp_after_failure(&temp_path, seam, e));
            }
        }
    }

    if tier.fsyncs_dir() {
        let dir_handle = File::open(&dir)?;
        seam.fsync_dir(&dir_handle)?;
    }
    Ok(())
}

/// Durability-bearing write: temp file, `fsync` the file, REPLACING
/// rename, `fsync` the parent directory. See the module docs for which
/// file class this is for (today: the launch shim's exec-failure
/// sentinel) and why it alone pays for both fsyncs.
///
/// Synchronous and callable with no tokio runtime present — load-bearing,
/// not incidental: the launch shim (`launch::exec_launch_spec`) runs
/// after `exec`-ing has already failed, deliberately WITHOUT a tokio
/// runtime (`main.rs` never builds one for `internal launch`, to avoid
/// paying for a runtime the successful-`exec` path replaces entirely), so
/// this function must work as plain, blocking I/O. There is no async
/// wrapper: nothing on the supervisor side writes a durability-bearing
/// file today (only the shim does), so one would have no caller — add it
/// if and when one exists, per the same reasoning that removed the
/// asymmetric one this module used to carry.
pub fn write_durable_sync(path: &Path, bytes: &[u8], seam: &dyn FaultSeam) -> io::Result<()> {
    write_staged(path, bytes, seam, Tier::Durable)
}

/// Best-effort atomic write: temp file, `fsync` the file, REPLACING
/// rename; no directory fsync. See the module docs for this tier's
/// "losable, never torn" contract and why no directory fsync is owed.
///
/// Takes the seam explicitly so tests can inject a failure or observe
/// ordering through [`FaultSeam`]; production passes [`RealFs`].
pub fn overwrite_private_file_sync(
    path: &Path,
    bytes: &[u8],
    seam: &dyn FaultSeam,
) -> io::Result<()> {
    write_staged(path, bytes, seam, Tier::BestEffort)
}

/// Atomic-publication-before-launch write: temp file (no fsync), publish
/// by hard-LINKING it to the destination — which fails if the destination
/// already exists, rather than replacing it. See the module docs for why
/// this tier is distinct from both the others despite specs' short life.
///
/// The 0600 mode is defended twice: once at `open` (masked by whatever
/// umask is active) and once by `fchmod` through the open descriptor
/// (immune to umask) — see `write_staged`'s inline docs. Both matter
/// here specifically because these files carry agent command lines users
/// do put credentials into.
pub fn write_private_file_sync(path: &Path, bytes: &[u8], seam: &dyn FaultSeam) -> io::Result<()> {
    write_staged(path, bytes, seam, Tier::AtomicPublication)
}

/// Async wrapper around [`write_private_file_sync`] for the production
/// launch-spec write (`service.rs`'s `create_session`). Offloaded to
/// `spawn_blocking` because the link and unlink are blocking syscalls
/// that must not run on an async worker thread; a fresh [`RealFs`] is
/// constructed INSIDE the closure so only that trivially-`Send`
/// zero-sized type, not a `dyn FaultSeam`, crosses the thread boundary
/// (see [`FaultSeam`]'s own docs on why it need not be `Send + Sync`).
/// The best-effort tier has no async twin: its one caller (the tmux
/// config, `tmux.rs`) writes from a synchronous context.
pub async fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = path.to_path_buf();
    let bytes = bytes.to_vec();
    tokio::task::spawn_blocking(move || write_private_file_sync(&path, &bytes, &RealFs))
        .await
        .unwrap_or_else(|join_err| Err(io::Error::other(join_err)))
}

/// A staged write whose content arrives over time and is never held in
/// memory — the attachment-upload tier (PLAN_M4.md item 4; see the module
/// docs' streaming section for how it differs from the whole-file tiers
/// and why).
///
/// The lifecycle is explicit on purpose: [`create`](Self::create) stages
/// the temp file, [`write_chunk`](Self::write_chunk) appends, and exactly
/// one of [`publish_no_clobber`](Self::publish_no_clobber) or
/// [`abandon`](Self::abandon) ends it. Both consume `self`, so a
/// half-finished stream is not a value a caller can accidentally keep
/// using, and both leave nothing staged behind.
///
/// ## The temp file's fate is not left to the caller's diligence
///
/// Every failing method ATTEMPTS to clean up its own temp file before
/// returning, and `Drop` attempts the same for a stream that ends any
/// OTHER way — a panic, a cancelled async task, a caller that simply drops
/// it. That matters more here than for the whole-file tiers: an upload's
/// stream is owned by a long-lived task that a client disconnect, a
/// session delete, or a stall timeout can end at any moment, and "the
/// staging file goes away when the transfer stops" has to hold on every
/// one of those paths rather than on the handful somebody remembered to
/// write cleanup code for.
///
/// "Attempts" is the honest word: a removal can itself fail (a read-only
/// directory, a filesystem error), and no caller can be made to succeed at
/// it. A failure is always REPORTED — folded into the returned error, or
/// returned by `abandon` — never swallowed, and the startup sweep of the
/// staging directory (`crate::attachments::sweep_staging`) is the backstop
/// both for that and for the case no in-process cleanup can cover at all,
/// a hard crash (`crate::attachments::reconcile_at_startup`). What never
/// varies is the other half: nothing partial is ever published.
pub struct StagedStream {
    /// `None` once the stream has been finished (published or abandoned)
    /// or once a failure closed it, which is also what makes the
    /// post-failure state inert rather than half-usable.
    file: Option<File>,
    temp_path: PathBuf,
    /// Where publication candidates are resolved — a DIFFERENT directory
    /// from the one holding `temp_path`, and that separation is the whole
    /// point (see `create`).
    publish_dir: PathBuf,
    /// Whether the temp file has already been dealt with — the flag
    /// `Drop` consults so a published or abandoned stream's destructor
    /// does not chase a path that is gone (or, worse, one some later
    /// staging call has since reused).
    finished: bool,
}

impl StagedStream {
    /// Stage a temp file in `staging_dir`, to be published later into
    /// `publish_dir`.
    ///
    /// ## Two directories, not one, and why that is structural
    ///
    /// Staged and published files live in DISJOINT namespaces. The
    /// whole-file tiers can afford to stage beside their destination
    /// because their sweeps run over directories whose legitimate names
    /// are UUIDs, so `.tmp-` cannot collide with anything real. An
    /// attachment's published names come from the USER — `report.tmp-old`
    /// is a perfectly ordinary filename — so any sweep that recognized
    /// debris by NAME would eventually delete somebody's file. Putting
    /// staging in its own directory makes "is this debris?" a question
    /// about WHERE a file is rather than what it is called, which no
    /// filename can lie about.
    ///
    /// `name` shapes only the temp file's name (the shared
    /// `.<name>.tmp-<uuid>` convention, kept so a stray staging file is
    /// still recognizable to a human reading a directory listing); the
    /// published name is chosen later, at publication, from the candidate
    /// sequence — collision resolution cannot be decided here, because a
    /// name free at `create` time can be taken by the time the bytes are
    /// all in.
    pub fn create(
        staging_dir: &Path,
        publish_dir: &Path,
        name: &str,
        seam: &dyn FaultSeam,
    ) -> io::Result<StagedStream> {
        let temp_path = temp_path_for(Path::new(name), staging_dir);
        let file = open_staged_temp(&temp_path, seam)?;
        Ok(StagedStream {
            file: Some(file),
            temp_path,
            publish_dir: publish_dir.to_path_buf(),
            finished: false,
        })
    }

    /// Where this stream is staging, for a caller that needs to prove the
    /// temp file's existence (or absence) independently.
    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    /// Append one chunk, seam-mediated exactly like the whole-file tiers'
    /// single `write`.
    ///
    /// A failure here is terminal for the stream: the temp file is
    /// removed and this stream goes inert, because a partial write means
    /// the staged content no longer corresponds to anything the sender
    /// sent, and continuing to append to it would build a file that is
    /// silently missing a hole in its middle.
    pub fn write_chunk(&mut self, seam: &dyn FaultSeam, bytes: &[u8]) -> io::Result<()> {
        let Some(file) = self.file.as_mut() else {
            return Err(io::Error::other(
                "this staged stream was already finished or failed",
            ));
        };
        match seam.write(file, bytes) {
            Ok(()) => Ok(()),
            Err(e) => Err(self.fail(seam, e)),
        }
    }

    /// Flush the staged content and publish it under the FIRST candidate
    /// name that is not already taken, returning the published path.
    ///
    /// Publication is no-clobber by construction: each attempt is a
    /// `link`, which the kernel fails with `EEXIST` rather than replacing
    /// an existing file, so two concurrent streams racing the same
    /// candidate sequence cannot both land on one name and cannot
    /// overwrite each other — the loser simply walks on to the next
    /// candidate. This is why the caller supplies a SEQUENCE rather than
    /// one name and a "does it exist?" check of its own: any such check
    /// would be a TOCTOU the other uploader can interleave with.
    ///
    /// `candidates` running out is a genuine failure (nothing published,
    /// temp cleaned), not a silent overwrite: the caller's sequence is
    /// what decides how hard to try.
    pub fn publish_no_clobber<I>(
        mut self,
        seam: &dyn FaultSeam,
        candidates: I,
    ) -> io::Result<PathBuf>
    where
        I: IntoIterator<Item = String>,
    {
        let Some(file) = self.file.take() else {
            return Err(io::Error::other(
                "this staged stream was already finished or failed",
            ));
        };
        if let Err(e) = seam.fsync_file(&file) {
            return Err(self.fail(seam, e));
        }
        // The descriptor's job is done once the content is durable; the
        // publication below works purely on names.
        drop(file);

        for candidate in candidates {
            let destination = self.publish_dir.join(&candidate);
            match seam.link(&self.temp_path, &destination) {
                Ok(()) => {
                    self.finished = true;
                    // The destination already holds the complete content
                    // through the hard link, so this name's removal is
                    // cosmetic — its failure is debris for the startup
                    // sweep, never a reason to report a publish that
                    // demonstrably happened as a failure.
                    let _ = seam.remove_temp(&self.temp_path);
                    return Ok(destination);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(self.fail(seam, e)),
            }
        }
        Err(self.fail(
            seam,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "every candidate name for this upload is already taken in {}",
                    self.publish_dir.display()
                ),
            ),
        ))
    }

    /// End the stream without publishing anything: the abort, stall, and
    /// channel-loss path. Reports a cleanup failure rather than hiding it,
    /// because a temp file that outlives its transfer is exactly the
    /// debris the lifecycle exists to prevent.
    pub fn abandon(mut self, seam: &dyn FaultSeam) -> io::Result<()> {
        self.file = None;
        self.finished = true;
        match seam.remove_temp(&self.temp_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Shared failure path: close the stream, remove its temp file, and
    /// return `original` with any cleanup failure folded into it — the
    /// streaming counterpart of [`remove_temp_after_failure`], which it
    /// delegates to so both share one wording and one ownership rule.
    fn fail(&mut self, seam: &dyn FaultSeam, original: io::Error) -> io::Error {
        self.file = None;
        self.finished = true;
        remove_temp_after_failure(&self.temp_path, seam, original)
    }
}

impl Drop for StagedStream {
    /// The catch-all for a stream that ended without `publish_no_clobber`
    /// or `abandon` — a panic, or an async task cancelled mid-transfer.
    ///
    /// Deliberately NOT seam-mediated and deliberately silent: `Drop` has
    /// nowhere to report to, and the seam exists to let tests fail an
    /// operation, which here could only mean deliberately leaving debris.
    /// Every path that CAN report goes through `abandon` or `fail`
    /// instead.
    fn drop(&mut self) {
        if !self.finished {
            let _ = fs::remove_file(&self.temp_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;

    /// A test [`FaultSeam`] that WRAPS the real operations — delegating to
    /// [`RealFs`]'s own default bodies rather than reimplementing them —
    /// while recording every call it mediates, in order, and optionally
    /// substituting a failure for one named window WITHOUT performing the
    /// real operation at all (a truer simulation of "this crashed before
    /// the syscall could run" than failing it after the fact). Plain
    /// `RefCell`, not a `Mutex`: see [`FaultSeam`]'s docs on why nothing
    /// here needs to be thread-safe.
    #[derive(Default)]
    struct TestSeam {
        fail_at: Option<&'static str>,
        log: RefCell<Vec<&'static str>>,
        /// Whether `remove_temp` is recorded in `log`.
        ///
        /// Off by default so the ordering assertions that predate the
        /// cleanup stage keep reading as the sequence they are about
        /// ("write, fsync_file, rename"), and on only for the tests that
        /// are specifically about cleanup. Recording it unconditionally
        /// would make every existing expectation carry an entry that has
        /// nothing to do with what it pins.
        record_cleanup: bool,
    }

    impl TestSeam {
        fn failing_at(window: &'static str) -> Self {
            Self {
                fail_at: Some(window),
                log: RefCell::new(Vec::new()),
                record_cleanup: false,
            }
        }

        /// A seam that also records the cleanup stage — see
        /// [`TestSeam::record_cleanup`] for why that is opt-in.
        fn recording_cleanup() -> Self {
            Self {
                fail_at: None,
                log: RefCell::new(Vec::new()),
                record_cleanup: true,
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.log.borrow().clone()
        }

        /// Record `window`, then report whether the real operation should
        /// still run: `false` exactly when this window is the one
        /// configured to fail, in which case the caller must return an
        /// error WITHOUT performing the real syscall.
        fn should_perform(&self, window: &'static str) -> bool {
            self.log.borrow_mut().push(window);
            self.fail_at != Some(window)
        }
    }

    impl FaultSeam for TestSeam {
        fn write(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
            if !self.should_perform("write") {
                return Err(io::Error::other("injected failure at write"));
            }
            RealFs.write(file, bytes)
        }
        fn fsync_file(&self, file: &File) -> io::Result<()> {
            if !self.should_perform("fsync_file") {
                return Err(io::Error::other("injected failure at fsync_file"));
            }
            RealFs.fsync_file(file)
        }
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            if !self.should_perform("rename") {
                return Err(io::Error::other("injected failure at rename"));
            }
            RealFs.rename(from, to)
        }
        fn link(&self, from: &Path, to: &Path) -> io::Result<()> {
            if !self.should_perform("link") {
                return Err(io::Error::other("injected failure at link"));
            }
            RealFs.link(from, to)
        }
        fn fsync_dir(&self, dir: &File) -> io::Result<()> {
            if !self.should_perform("fsync_dir") {
                return Err(io::Error::other("injected failure at fsync_dir"));
            }
            RealFs.fsync_dir(dir)
        }
        fn remove_temp(&self, path: &Path) -> io::Result<()> {
            // The failure check runs whether or not this seam records the
            // stage: a test that injects a cleanup failure is asking for
            // the failure, not for the log entry.
            let perform = if self.record_cleanup {
                self.should_perform("remove_temp")
            } else {
                self.fail_at != Some("remove_temp")
            };
            if !perform {
                return Err(io::Error::other("injected failure at remove_temp"));
            }
            RealFs.remove_temp(path)
        }
    }

    /// Pins the durability-bearing tier's exact operation order — file
    /// fsync strictly before rename, directory fsync strictly after — by
    /// observing calls the seam itself MEDIATES (see the module docs and
    /// [`FaultSeam`] for why that is stronger than a marker fired beside
    /// a hard-coded syscall): a regression that reordered these calls, or
    /// dropped one of them entirely while leaving the others in place,
    /// changes what this log records and fails the assertion either way.
    #[farhelm_testtrace::test]
    fn durable_write_syncs_file_before_rename_and_dir_after() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        let seam = TestSeam::default();

        write_durable_sync(&path, b"exec_failed", &seam).unwrap();

        assert_eq!(
            seam.calls(),
            vec!["write", "fsync_file", "rename", "fsync_dir"],
            "durability-bearing writes must fsync the file before renaming it into place, \
             then fsync the parent directory only after the rename"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"exec_failed");
    }

    /// A failure injected AT the write itself (the real `write_all` never
    /// runs — see `TestSeam::should_perform`) must leave the destination
    /// exactly as it was before the call: absent, here. This is the
    /// window a crash between opening the temp file and writing to it
    /// occupies in real life.
    #[farhelm_testtrace::test]
    fn durable_write_failure_at_write_leaves_destination_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        let seam = TestSeam::failing_at("write");

        write_durable_sync(&path, b"exec_failed", &seam).unwrap_err();

        assert!(
            !path.exists(),
            "a failure at write must never publish anything"
        );
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// A failure injected AT the file fsync — content is written but the
    /// syscall proving it durable never completes — must behave
    /// identically: nothing at the destination, no orphaned temp file.
    #[farhelm_testtrace::test]
    fn durable_write_failure_at_file_fsync_leaves_destination_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        let seam = TestSeam::failing_at("fsync_file");

        write_durable_sync(&path, b"exec_failed", &seam).unwrap_err();

        assert!(!path.exists());
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// A failure injected AT rename (the rename call itself never runs)
    /// must leave a PRE-EXISTING destination's OLD content completely
    /// intact — the temp file is debris (cleaned up), and nothing was
    /// ever published.
    #[farhelm_testtrace::test]
    fn durable_write_failure_at_rename_leaves_old_content_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        std::fs::write(&path, b"exec_failed argv0=old errno=2").unwrap();
        let seam = TestSeam::failing_at("rename");

        write_durable_sync(&path, b"exec_failed argv0=new errno=8", &seam).unwrap_err();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"exec_failed argv0=old errno=2",
            "a failure at rename must leave the OLD content completely intact"
        );
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// A failure injected AT the directory fsync — the rename has ALREADY
    /// run for real by this point (only `should_perform("fsync_dir")`
    /// returns false) — must NOT retroactively un-publish content the
    /// rename already committed: this is the one window where "the call
    /// reports an error" and "the write actually succeeded" are
    /// simultaneously true, matching a real crash between `rename` and
    /// the directory `fsync`.
    #[farhelm_testtrace::test]
    fn durable_write_failure_at_dir_fsync_still_publishes_complete_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        let seam = TestSeam::failing_at("fsync_dir");

        write_durable_sync(&path, b"exec_failed", &seam).unwrap_err();

        assert_eq!(std::fs::read(&path).unwrap(), b"exec_failed");
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// The durability-bearing temp file must be created 0600 regardless
    /// of any pre-existing mode at the destination — the same "mode set
    /// at CREATE time, never by chmod" property [`overwrite_private_file_sync`]
    /// already relies on, pinned separately here because this tier's
    /// temp file is additionally fsync'd before it is ever renamed.
    #[farhelm_testtrace::test]
    fn durable_write_result_is_0600() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        write_durable_sync(&path, b"exec_failed", &TestSeam::default()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "durable sentinel file must be 0600, got {mode:o}"
        );
    }

    /// Best-effort atomic writes must publish via the same temp-then-
    /// rename mechanics as the durability-bearing tier (content ends up
    /// exactly right, replacing longer content with shorter cleanly) and
    /// — item 19's sharpening — DO fsync the file before rename, but must
    /// still skip the directory fsync entirely (that omission is this
    /// tier's whole cost savings).
    #[farhelm_testtrace::test]
    fn best_effort_write_fsyncs_file_but_never_the_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");
        let seam = TestSeam::default();

        overwrite_private_file_sync(&path, b"a much longer first payload", &seam).unwrap();
        overwrite_private_file_sync(&path, b"short", &seam).unwrap();

        assert_eq!(
            seam.calls(),
            vec![
                "write",
                "fsync_file",
                "rename",
                "write",
                "fsync_file",
                "rename"
            ],
            "best-effort atomic writes must fsync the file before rename (item 19) but must \
             never fsync the directory — a reappearing directory fsync here would silently \
             erase this tier's cost savings"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"short",
            "the replaced file must contain exactly the new content, not the old content \
             truncated-and-overwritten with leftover trailing bytes"
        );
    }

    /// A failure injected AT the write (before any fsync) must leave a
    /// fresh destination untouched — the best-effort tier's analogue of
    /// the durability-bearing test above, proving the shared
    /// [`write_staged`] engine behaves the same way here.
    #[farhelm_testtrace::test]
    fn best_effort_write_failure_at_write_leaves_destination_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");
        let seam = TestSeam::failing_at("write");

        overwrite_private_file_sync(&path, b"content", &seam).unwrap_err();

        assert!(!path.exists());
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// The post-rename window specifically (item 10: previously only the
    /// pre-rename windows were exercised for this tier): a failure
    /// injected AT rename, against a PRE-EXISTING destination, must leave
    /// that OLD content completely intact — proving "never torn" holds
    /// even when there is something real to tear.
    #[farhelm_testtrace::test]
    fn best_effort_write_failure_at_rename_leaves_old_content_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");
        std::fs::write(&path, b"the previous snapshot").unwrap();
        let seam = TestSeam::failing_at("rename");

        overwrite_private_file_sync(&path, b"a new, different-length snapshot", &seam).unwrap_err();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"the previous snapshot",
            "a failure at rename must leave the OLD snapshot completely intact, not a mix"
        );
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// A `rename` failure that is a REAL I/O error (not injected) must
    /// still clean up the staged temp file. A DIRECTORY at the
    /// destination reliably makes `rename` fail here: POSIX `rename(2)`
    /// refuses to replace a directory with a non-directory regardless of
    /// permissions.
    #[farhelm_testtrace::test]
    fn best_effort_write_removes_the_temp_file_when_rename_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");
        std::fs::create_dir(&path).unwrap();

        overwrite_private_file_sync(&path, b"content", &TestSeam::default())
            .expect_err("rename onto a directory must fail");

        assert_temp_files_cleaned_up(tmp.path());
    }

    /// `path` being a SYMLINK must not make this function write through
    /// it: `rename` replaces whatever directory entry `path` names
    /// (symlink or not) with the temp file, rather than ever opening
    /// `path` itself and following it. Pins both halves: the destination
    /// ends up a plain, 0600 regular file with the new content, and
    /// whatever the symlink used to point at is completely untouched.
    #[farhelm_testtrace::test]
    fn best_effort_write_replaces_a_symlink_without_following_it() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let path = tmp.path().join("snapshot");
        std::fs::write(&target, b"target content").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        overwrite_private_file_sync(&path, b"replacement", &TestSeam::default()).unwrap();

        assert!(
            !std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the destination must become a regular file, not remain a symlink"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"target content",
            "the symlink's OLD target must be left completely untouched"
        );
    }

    /// A pre-existing file at the destination with a too-wide mode,
    /// planted the way a bug or an older build might leave one, must not
    /// keep that mode forever just because its content happened to get
    /// replaced — `OpenOptions::mode` only applies at file CREATION, so a
    /// `truncate`-in-place implementation would leave this hole open; the
    /// rename-based replacement (a fresh 0600 temp file swapped in) fixes
    /// it structurally rather than by remembering to `chmod` afterward.
    #[farhelm_testtrace::test]
    fn best_effort_write_repairs_a_pre_existing_wide_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");
        std::fs::write(&path, b"planted by something else").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        overwrite_private_file_sync(&path, b"replaced", &TestSeam::default()).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "a pre-existing wide-mode file must be replaced with a 0600 one, got {mode:o}"
        );
    }

    /// The launch-spec write path: owner-only, and refusing to overwrite
    /// (spec names are fresh UUIDs; a collision is an impersonation
    /// attempt, not a retry) — now via hard-link publication rather than
    /// `create_new` at the final path directly (item 9), so a collision
    /// is detected by the PUBLISH step (`link` failing `EEXIST`), not by
    /// the temp file's own `create_new`.
    #[farhelm_testtrace::test]
    fn write_private_file_is_0600_and_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("spec.json");

        write_private_file_sync(&path, b"secret", &TestSeam::default()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "private file must be 0600, got {mode:o}");
        assert_eq!(std::fs::read(&path).unwrap(), b"secret");

        write_private_file_sync(&path, b"clobber", &TestSeam::default())
            .expect_err("existing file must not be overwritten");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"secret",
            "a refused collision must leave the original content untouched"
        );
    }

    /// A failure injected AT the link-publish step (simulating a crash
    /// between the temp write and the link, or a genuine collision) must
    /// never leave a partial file visible at the destination — the launch
    /// shim must never be able to observe a torn spec (item 9).
    #[farhelm_testtrace::test]
    fn write_private_file_failure_at_link_leaves_destination_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("spec.json");
        let seam = TestSeam::failing_at("link");

        write_private_file_sync(&path, b"secret", &seam).unwrap_err();

        assert!(
            !path.exists(),
            "a failure at link must never publish a partial spec at the destination"
        );
        assert_temp_files_cleaned_up(tmp.path());
    }

    /// The atomic-publication tier's temp file is never fsync'd — the one
    /// respect in which it is cheaper than even the best-effort tier
    /// (see the module docs for why: nothing ever reads a spec across a
    /// crash boundary, only within this process's own launch sequence).
    #[farhelm_testtrace::test]
    fn write_private_file_never_syncs_anything() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("spec.json");
        let seam = TestSeam::default();

        write_private_file_sync(&path, b"secret", &seam).unwrap();

        assert_eq!(seam.calls(), vec!["write", "link"]);
    }

    /// The async wrapper ([`write_private_file`]) exists to run blocking
    /// I/O off the async runtime's worker threads via `spawn_blocking` —
    /// this pins that it still produces the exact same on-disk result as
    /// its `_sync` twin, rather than merely trusting `spawn_blocking`'s
    /// plumbing by inspection.
    #[farhelm_testtrace::test]
    async fn write_private_file_matches_its_sync_counterpart() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("spec.json");
        write_private_file(&path, b"secret").await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"secret");
    }

    /// `is_staged_temp_name` is what every backstop sweep keys off to
    /// distinguish debris from legitimate files; a real destination name
    /// (a session id, a launch id — both plain UUIDs) must never match
    /// it, or a sweep would delete live state instead of leftover temp
    /// files. This only pins the substring contract the function's own
    /// docs describe — it is not a full parse of the `.tmp-<uuid>` shape.
    #[farhelm_testtrace::test]
    fn staged_temp_name_pattern_matches_only_temp_files() {
        assert!(is_staged_temp_name(".sentinel.tmp-abc123"));
        assert!(!is_staged_temp_name("sentinel"));
        assert!(!is_staged_temp_name("550e8400-e29b-41d4-a716-446655440000"));
    }

    /// `Path::new("bare-name").parent()` returns `Some("")`, an EMPTY
    /// path, not `None` — only a path with no parent component at all
    /// (`/`) returns `None`. This pins `parent_dir`'s normalization of
    /// that empty case to `"."` directly, independent of any filesystem
    /// side effect, rather than only indirectly through a write that
    /// would need to touch the real current working directory (unsafe to
    /// do from a test that may run concurrently with others).
    #[farhelm_testtrace::test]
    fn parent_dir_normalizes_a_bare_filenames_empty_parent_to_dot() {
        assert_eq!(parent_dir(Path::new("bare-name")).unwrap(), Path::new("."));
        assert_eq!(
            parent_dir(Path::new("/state/file")).unwrap(),
            Path::new("/state")
        );
        assert!(parent_dir(Path::new("/")).is_err());
    }

    /// Item 20: `OpenOptions::mode` is masked DOWN by the process umask —
    /// a hostile one (0o777 clears every permission bit) can leave a
    /// freshly-created temp file completely unreadable even by its own
    /// owner. That does not stop THIS write (the `open` call's own
    /// permission check already ran), but it would make the PUBLISHED
    /// file unreadable to whoever opens it fresh later — a restarted
    /// supervisor reading a sentinel, or the shim reading a spec.
    /// `write_staged`'s `fchmod` after `open` exists specifically to
    /// defeat this, since `fchmod` is never subject to umask.
    ///
    /// Exercised in a genuinely SEPARATE OS process — NEVER by mutating
    /// this test's own umask, which (unlike an ordinary variable) is
    /// process-GLOBAL state shared by every test running concurrently in
    /// this same test binary, so mutating it here would race all of them
    /// (the same class of violation the repo-wide "no environment
    /// mutation in the running test process" rule exists to prevent, one
    /// level lower). This test instead re-execs its own compiled test
    /// binary, filtered to just itself by name, through a shell that sets
    /// the hostile umask first; the re-exec'd CHILD recognizes itself via
    /// an environment variable set only on that child's own `Command`
    /// (never on this, the running test's, environment) and, once
    /// recognized, performs the real write and exits with a bare status
    /// code instead of running the harness's normal pass/fail path.
    #[farhelm_testtrace::test]
    fn write_durable_sync_defeats_a_hostile_creation_time_umask() {
        const CHILD_MARKER: &str = "FARHELM_FILES_TEST_UMASK_CHILD_PATH";
        if let Ok(path) = std::env::var(CHILD_MARKER) {
            // We ARE the re-exec'd child, already running under the
            // hostile umask the parent's shell set before `exec`-ing this
            // same binary. `std::process::exit` here bypasses the
            // harness's own reporting entirely — by this point we are
            // standing in for a tiny standalone helper program, not
            // "running the test" in the ordinary sense.
            let outcome = write_durable_sync(Path::new(&path), b"exec_failed", &RealFs);
            std::process::exit(if outcome.is_ok() { 0 } else { 1 });
        }

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sentinel");
        let exe = std::env::current_exe().unwrap();
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "umask 0777 && exec {} {} --exact",
                shell_words::quote(&exe.to_string_lossy()),
                shell_words::quote(
                    "files::tests::write_durable_sync_defeats_a_hostile_creation_time_umask"
                ),
            ))
            .env(CHILD_MARKER, &path)
            // This stand-in exits before capture completion, and its hostile
            // umask would make a newly allocated trace slot unreadable. The
            // parent wrapper owns the test verdict and retained diagnostics.
            .env_remove("FARHELM_TEST_TRACE_DIR")
            .status()
            .expect("spawning the hostile-umask child process");
        assert!(
            status.success(),
            "the hostile-umask child process itself must report success: {status:?}"
        );

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "fchmod must defeat a hostile creation-time umask, got {mode:o}"
        );
    }

    /// The streaming tier's happy path, pinned as an operation SEQUENCE
    /// rather than an end state: one seam-mediated `write` per chunk (a
    /// chunk is never coalesced into a buffer the caller then writes
    /// once), then exactly one flush, then the publish — and, as with the
    /// best-effort tier it inherits its durability from, never a directory
    /// fsync.
    ///
    /// The end state alone would be satisfied by an implementation that
    /// accumulated every chunk in memory and wrote once at commit, which
    /// is precisely what an upload with no size cap must never do.
    #[farhelm_testtrace::test]
    fn staged_stream_writes_each_chunk_then_flushes_once_before_publishing() {
        let tmp = tempfile::tempdir().unwrap();
        let seam = TestSeam::default();

        let mut stream = StagedStream::create(
            &staging_under(tmp.path()),
            tmp.path(),
            "screenshot.png",
            &seam,
        )
        .unwrap();
        stream.write_chunk(&seam, b"first-").unwrap();
        stream.write_chunk(&seam, b"second").unwrap();
        let published = stream
            .publish_no_clobber(&seam, ["screenshot.png".to_string()])
            .unwrap();

        assert_eq!(
            seam.calls(),
            vec!["write", "write", "fsync_file", "link"],
            "a streaming upload must write each chunk as it arrives, flush once, and publish \
             by link — never buffer, and never fsync the directory"
        );
        assert_eq!(published, tmp.path().join("screenshot.png"));
        assert_eq!(std::fs::read(&published).unwrap(), b"first-second");
        let mode = std::fs::metadata(&published).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "a published attachment must be 0600, got {mode:o}"
        );
        assert_staging_empty(tmp.path());
    }

    /// Publication must NEVER replace an existing file, and must keep
    /// walking the caller's candidate names until one lands.
    ///
    /// This is the property two concurrent uploads of the same filename
    /// depend on (PLAN_M4.md item 4): the collision is settled by `link`'s
    /// own `EEXIST`, so there is no window between "is this name free?"
    /// and "claim it" for the other uploader to interleave with. Pinned
    /// with a pre-existing file whose content is checked afterwards,
    /// because a `rename`-based publication would pass every other
    /// assertion here while silently destroying it.
    #[farhelm_testtrace::test]
    fn staged_stream_publication_walks_past_taken_names_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        let taken = tmp.path().join("shot.png");
        std::fs::write(&taken, b"somebody else's attachment").unwrap();
        let seam = TestSeam::default();

        let mut stream =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                .unwrap();
        stream.write_chunk(&seam, b"mine").unwrap();
        let published = stream
            .publish_no_clobber(&seam, ["shot.png".to_string(), "shot-1.png".to_string()])
            .unwrap();

        assert_eq!(
            published,
            tmp.path().join("shot-1.png"),
            "a taken name must be skipped, not overwritten"
        );
        assert_eq!(
            std::fs::read(&taken).unwrap(),
            b"somebody else's attachment"
        );
        assert_eq!(std::fs::read(&published).unwrap(), b"mine");
    }

    /// Running out of candidate names is an honest failure with nothing
    /// published and no temp left behind — never a fallback that
    /// overwrites the last candidate, which would turn a naming problem
    /// into silent data loss.
    #[farhelm_testtrace::test]
    fn staged_stream_publication_that_runs_out_of_names_publishes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("taken"), b"original").unwrap();
        let seam = TestSeam::default();

        let mut stream =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "taken", &seam).unwrap();
        stream.write_chunk(&seam, b"replacement").unwrap();
        stream
            .publish_no_clobber(&seam, ["taken".to_string()])
            .expect_err("an exhausted candidate list must fail");

        assert_eq!(
            std::fs::read(tmp.path().join("taken")).unwrap(),
            b"original"
        );
        assert_staging_empty(tmp.path());
    }

    /// A failure injected AT a chunk write (the real `write_all` never
    /// runs) publishes nothing and leaves no temp behind — the streaming
    /// analogue of the whole-file tiers' write-failure tests, and the
    /// window a disk filling up mid-upload occupies in real life.
    #[farhelm_testtrace::test]
    fn staged_stream_failure_at_a_chunk_write_publishes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let seam = TestSeam::failing_at("write");

        let mut stream =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                .unwrap();
        stream
            .write_chunk(&seam, b"never lands")
            .expect_err("an injected write failure must be reported");
        stream
            .publish_no_clobber(&seam, ["shot.png".to_string()])
            .expect_err("a stream whose write failed must not be publishable");

        assert!(!tmp.path().join("shot.png").exists());
        assert_staging_empty(tmp.path());
    }

    /// The torn-file window itself: a failure at the flush, and a failure
    /// at the publish, must both leave NOTHING at the destination.
    ///
    /// Both are crash windows a real upload occupies — content written but
    /// not yet durable, and durable but not yet named — and the promise
    /// this tier makes is that neither is ever observable as a partial
    /// file at the published path.
    #[farhelm_testtrace::test]
    fn staged_stream_failures_around_publication_never_expose_a_partial_file() {
        for window in ["fsync_file", "link"] {
            let tmp = tempfile::tempdir().unwrap();
            let seam = TestSeam::failing_at(window);

            let mut stream =
                StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                    .unwrap();
            stream.write_chunk(&seam, b"half an upload").unwrap();
            let published = stream.publish_no_clobber(&seam, ["shot.png".to_string()]);
            assert!(
                published.is_err(),
                "a failure at {window} must be reported, got {published:?}"
            );

            assert!(
                !tmp.path().join("shot.png").exists(),
                "a failure at {window} published a file"
            );
            assert_staging_empty(tmp.path());
        }
    }

    /// Abandoning a transfer removes its temp file immediately, and so
    /// does simply DROPPING one — the paths a client abort, a session
    /// delete, a stall timeout, and a cancelled task actually take.
    ///
    /// The drop half is the one worth pinning: it is the only cleanup that
    /// no call site has to remember, and a refactor that removed it would
    /// leave every cancelled upload's temp file on disk until the next
    /// startup sweep, with nothing else failing.
    #[farhelm_testtrace::test]
    fn staged_stream_cleans_up_on_abandon_and_on_a_bare_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let seam = TestSeam::recording_cleanup();

        let mut abandoned =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                .unwrap();
        abandoned.write_chunk(&seam, b"partial").unwrap();
        let abandoned_temp = abandoned.temp_path().to_path_buf();
        assert!(abandoned_temp.exists());
        abandoned.abandon(&seam).unwrap();
        assert!(
            !abandoned_temp.exists(),
            "abandon must remove the temp file"
        );
        assert_eq!(seam.calls(), vec!["write", "remove_temp"]);

        let mut dropped =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                .unwrap();
        dropped.write_chunk(&seam, b"partial").unwrap();
        let dropped_temp = dropped.temp_path().to_path_buf();
        drop(dropped);
        assert!(
            !dropped_temp.exists(),
            "dropping an unfinished stream must remove its temp file"
        );
        assert_staging_empty(tmp.path());
    }

    /// A cleanup that ITSELF fails must be reported alongside the failure
    /// that triggered it, not swallowed: the temp file is then still on
    /// disk, which is exactly the debris the caller needs to know about
    /// (and the startup sweep's whole reason for existing).
    #[farhelm_testtrace::test]
    fn staged_stream_folds_a_failed_cleanup_into_the_original_error() {
        let tmp = tempfile::tempdir().unwrap();
        let seam = TestSeam {
            fail_at: Some("remove_temp"),
            log: RefCell::new(Vec::new()),
            record_cleanup: false,
        };

        let mut stream =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                .unwrap();
        let temp = stream.temp_path().to_path_buf();
        // A publish onto a name that is already taken, with only that one
        // candidate: the publish fails, which is what drives cleanup.
        std::fs::write(tmp.path().join("shot.png"), b"taken").unwrap();
        stream.write_chunk(&seam, b"content").unwrap();
        let error = stream
            .publish_no_clobber(&seam, ["shot.png".to_string()])
            .expect_err("an exhausted candidate list must fail");

        let rendered = error.to_string();
        assert!(
            rendered.contains("already taken") && rendered.contains("could not remove"),
            "both the original failure and the cleanup failure must be visible, got: {rendered}"
        );
        // Left on disk on purpose by the injected failure — the state the
        // startup sweep exists to clean up after a crash.
        assert!(temp.exists());
        std::fs::remove_file(&temp).unwrap();
    }

    /// The streaming tier must not quietly buffer what it streams.
    ///
    /// PLAN_M4.md's testing decisions call for exactly this: attachments
    /// have no size cap, so the whole no-cap policy rests on a large file
    /// costing TIME and not MEMORY. Every other assertion in this file is
    /// about content and ordering, all of which a buffering implementation
    /// satisfies perfectly right up until a big upload exhausts the host.
    ///
    /// Measured as resident-set growth across a file two orders of
    /// magnitude larger than the chunk: a buffering implementation lands
    /// at ~64 MiB over baseline, a streaming one at roughly zero (the
    /// bytes leave the process through `write`, and written page cache is
    /// not this process's RSS).
    ///
    /// Run in a SEPARATE OS process, for the same reason the hostile-umask
    /// test is: RSS is a property of the whole process, and libtest runs
    /// this file's tests concurrently — a neighbour allocating a few MiB
    /// mid-measurement is indistinguishable here from the buffering this
    /// exists to catch, which would make the test either flaky or (with a
    /// threshold raised to tolerate the noise) blind. The child re-execs
    /// this same binary filtered to this test, recognizes itself by an
    /// environment variable set ONLY on that child's own `Command` (never
    /// on the running test's environment — repo rule), takes the
    /// measurement alone, and exits with a bare status code the parent
    /// turns back into a legible failure.
    #[farhelm_testtrace::test]
    fn staged_stream_does_not_grow_memory_with_the_size_of_the_upload() {
        const CHILD_MARKER: &str = "FARHELM_FILES_TEST_STREAM_RSS_CHILD";
        const CHUNK: usize = 256 * 1024;
        const CHUNKS: usize = 256; // 64 MiB total.
        const ALLOWED_GROWTH: u64 = 16 * 1024 * 1024;

        /// Resident bytes of this process, from `/proc/self/statm` (field
        /// 2 is resident pages) — the same probe the supervisor's
        /// stalled-viewer memory test uses.
        fn rss_bytes() -> u64 {
            let statm = std::fs::read_to_string("/proc/self/statm")
                .expect("this crate is Linux-only; /proc/self/statm must be readable");
            let pages: u64 = statm
                .split_whitespace()
                .nth(1)
                .and_then(|field| field.parse().ok())
                .expect("statm's second field is the resident page count");
            pages * 4096
        }

        if std::env::var(CHILD_MARKER).is_ok() {
            // We ARE the re-exec'd child: this process exists only to
            // stream one file, with nothing else running inside it.
            let tmp = tempfile::tempdir().unwrap();
            let seam = RealFs;
            let chunk = vec![0x61u8; CHUNK];
            let mut stream =
                StagedStream::create(&staging_under(tmp.path()), tmp.path(), "big.bin", &seam)
                    .unwrap();

            let baseline = rss_bytes();
            let mut peak = baseline;
            for _ in 0..CHUNKS {
                stream.write_chunk(&seam, &chunk).unwrap();
                peak = peak.max(rss_bytes());
            }
            let published = stream
                .publish_no_clobber(&seam, ["big.bin".to_string()])
                .unwrap();
            peak = peak.max(rss_bytes());

            let complete = std::fs::metadata(&published).unwrap().len() == (CHUNK * CHUNKS) as u64;
            let growth = peak.saturating_sub(baseline);
            std::process::exit(match (complete, growth < ALLOWED_GROWTH) {
                (true, true) => 0,
                (false, _) => 2,
                (_, false) => 3,
            });
        }

        let exe = std::env::current_exe().unwrap();
        let status = std::process::Command::new(exe)
            .arg("files::tests::staged_stream_does_not_grow_memory_with_the_size_of_the_upload")
            .arg("--exact")
            .env(CHILD_MARKER, "1")
            // The measurement process exits directly instead of completing a
            // test capture. Keep persistence with the parent that interprets
            // its exit status, rather than leaving a partial child slot.
            .env_remove("FARHELM_TEST_TRACE_DIR")
            .status()
            .expect("spawning the measurement child process");
        match status.code() {
            Some(0) => {}
            Some(2) => panic!("the whole payload did not reach the published file"),
            Some(3) => panic!(
                "streaming {} MiB grew the child process by more than {ALLOWED_GROWTH} bytes — \
                 the staged stream is buffering the upload instead of writing it through",
                (CHUNK * CHUNKS) / (1024 * 1024)
            ),
            other => panic!("the measurement child process failed: {other:?}"),
        }
    }

    /// An `abandon` whose removal fails REPORTS the failure rather than
    /// swallowing it — the honest half of this tier's cleanup contract.
    ///
    /// Cleanup is an attempt, not a guarantee: a read-only directory or a
    /// filesystem error can defeat it, and nothing in this module can make
    /// a removal succeed. What it can do is never lie about it, so its
    /// caller can log the truth (and, in the supervisor's case, retry)
    /// rather than report a cleanup that did not happen. The staging file
    /// left behind is exactly what the startup reconciliation exists for.
    #[farhelm_testtrace::test]
    fn staged_stream_abandon_reports_a_cleanup_it_could_not_perform() {
        let tmp = tempfile::tempdir().unwrap();
        let seam = TestSeam::failing_at("remove_temp");

        let mut stream =
            StagedStream::create(&staging_under(tmp.path()), tmp.path(), "shot.png", &seam)
                .unwrap();
        stream.write_chunk(&seam, b"partial").unwrap();
        let temp = stream.temp_path().to_path_buf();

        let error = stream
            .abandon(&seam)
            .expect_err("a removal that failed must be reported");
        assert!(
            error.to_string().contains("injected failure"),
            "the reported error must be the real cause, got: {error}"
        );
        assert!(
            temp.exists(),
            "the staging file is genuinely still there — which is what makes reporting it \
             load-bearing"
        );
        std::fs::remove_file(&temp).unwrap();
    }

    /// The staging directory the streaming tests hand [`StagedStream`],
    /// created on demand under a test's own temp directory.
    ///
    /// A helper rather than a fixture field because it makes every call
    /// site say the thing that matters at a glance: staging and
    /// publication are DIFFERENT directories here, which is the property
    /// the no-name-based-sweeping design rests on.
    fn staging_under(dir: &Path) -> PathBuf {
        let staging = dir.join(".staging");
        std::fs::create_dir_all(&staging).expect("staging dir");
        staging
    }

    /// Shared assertion for the streaming tier: nothing may be left in the
    /// staging directory once a transfer has ended, however it ended.
    fn assert_staging_empty(dir: &Path) {
        let leftover: Vec<_> = std::fs::read_dir(staging_under(dir))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name())
            .collect();
        assert!(
            leftover.is_empty(),
            "a finished transfer must leave nothing staged, found: {leftover:?}"
        );
    }

    /// Shared assertion: after any failure path in this module, the only
    /// things left in `dir` must be whatever the test itself planted —
    /// never a leftover `.tmp-*` staging file.
    fn assert_temp_files_cleaned_up(dir: &Path) {
        let leftover: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| is_staged_temp_name(&entry.file_name().to_string_lossy()))
            .collect();
        assert!(
            leftover.is_empty(),
            "a failed write must not leave a staged temp file behind, found: {leftover:?}"
        );
    }
}
