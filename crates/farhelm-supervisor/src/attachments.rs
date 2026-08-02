//! Where a session's uploaded attachments live, and what they are called
//! (PLAN_M4.md item 4).
//!
//! SPEC.md's contract is short and this module is what makes each half of
//! it true on disk: uploads land in a per-session directory under the
//! supervisor's own state directory — never in the working directory,
//! because dropping untracked files into a workspace is exactly the
//! implicit mutation this system promises not to make — and they are
//! removed when their session is deleted.
//!
//! The transfer machinery itself (channels, credit, stall detection,
//! commit) lives in `service.rs`; the write atomicity lives in
//! `crate::files`. What is here is everything that is a property of the
//! STORAGE rather than of the protocol: the layout, the naming rules, and
//! the lifecycle operations (a session's directory at delete, and the
//! startup reconciliation that cleans up after a crash).
//!
//! # Layout, and why it has reserved directories
//!
//! ```text
//! attachments/
//!   .quarantine/            <- directories a delete detached, awaiting removal
//!   <session-id>/           <- published attachments, USER-CHOSEN names
//!     .staging/             <- in-flight uploads, never anything else
//! ```
//!
//! The two dotted directories exist because every other name under
//! `<session-id>/` comes from the user. A sweep that recognized debris by
//! NAME would eventually delete somebody's `report.tmp-backup`; a sweep
//! that recognizes it by LOCATION cannot, because no upload ever publishes
//! into `.staging/` and nothing but a delete ever moves a directory into
//! `.quarantine/`. Session ids are UUIDs, so neither reserved name can
//! collide with a session directory, and [`publish_name`] refuses to hand
//! back a reserved name for the same reason.
//!
//! # Naming: recognizable, but shell-safe
//!
//! The published path is inserted into a terminal as input — that is the
//! entire point of an attachment — so the filename component has to
//! survive a shell without quoting. It is reduced to ASCII alphanumerics
//! plus `.`, `_`, and `-`, with everything else (spaces included) mapped
//! to `_`. The original name is kept recognizable rather than replaced by
//! a hash because agents read paths, and `screenshot.png` beats a digest.
//!
//! A name is never a reason to REFUSE an upload: SPEC.md rejects
//! directories, never a file for what it is called. A proposal that
//! reduces to nothing usable — empty, a bare path separator, the reserved
//! components `.` and `..`, or one of this module's own reserved directory
//! names — gets a generated name instead ([`GENERATED_NAME_PREFIX`]),
//! which is the same shape the client already generates for a pasted
//! screenshot.
//!
//! # Collisions: both uploads publish, under distinct names
//!
//! Two uploads of `screenshot.png` — including two in flight at the same
//! instant — must both land, under distinct paths, with neither silently
//! replacing the other. So this module supplies a SEQUENCE of candidate
//! names ([`name_candidates`]) and `files::StagedStream::publish_no_clobber`
//! walks it with `link`, letting the kernel's own `EEXIST` settle the
//! race. Nothing here ever tests a name for existence: any such test
//! would be a check-then-create the other uploader can interleave with.
//! The sequence ends in generated names rather than running out, because
//! "your file could not be named" is not one of the refusals SPEC.md
//! allows.

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// The characters a published attachment name may contain. Everything
/// else — spaces, quotes, `$`, `*`, newlines, any non-ASCII — is mapped to
/// `_`, because the path is inserted as terminal input and a name a shell
/// would split, glob, or expand breaks exactly the flow attachments exist
/// for.
fn is_shell_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// The in-flight directory inside a session's attachments directory. No
/// upload ever publishes into it, which is what makes "everything in here
/// is debris" a safe thing for the startup reconciliation to believe.
const STAGING_DIR: &str = ".staging";

/// Where a delete parks a session's attachments directory between
/// detaching it (an atomic rename) and removing it. See
/// [`quarantine_session_dir`] for why the two are separate steps.
const QUARANTINE_DIR: &str = ".quarantine";

/// Cap on a published attachment's own filename.
///
/// A filesystem limit, not a policy one: Linux caps a single path
/// COMPONENT at 255 bytes, and the staged temp file's name is the
/// destination name plus a `.` prefix and a `.tmp-<uuid>` suffix (42
/// bytes), while collision resolution can add a few more. 128 leaves room
/// for both with margin, and is far beyond any filename a human or a
/// screenshot tool produces — an over-long proposal is truncated (keeping
/// its extension, see [`sanitize`]) rather than refused, since a name is
/// never grounds for refusing an upload.
const MAX_NAME_BYTES: usize = 128;

/// Longest extension carried through a truncation. Past this, whatever
/// trails the last `.` is not an extension in any useful sense, and
/// preserving it would eat the part of the name a human would recognize.
const MAX_KEPT_EXTENSION_BYTES: usize = 16;

/// How many `<stem>-<n><.ext>` variants [`name_candidates`] offers before
/// it stops trying to keep the user's name and switches to generated ones.
///
/// Bounded rather than unbounded because each candidate is a real `link`
/// syscall: a directory that somehow accumulated a thousand
/// `screenshot.png` variants should stop paying for the walk rather than
/// spend unbounded time in the kernel on every subsequent upload. Falling
/// back to a generated name (rather than failing) is what keeps the
/// never-refuse-for-a-name contract intact past this point.
const MAX_NUMBERED_CANDIDATES: usize = 1000;

/// How many generated names the candidate sequence ends with.
///
/// One would do — a v4 UUID colliding with an existing file is not a
/// thing that happens — so this is pure paranoia about a future generator
/// with less entropy, and it costs nothing unless the impossible occurs.
const GENERATED_CANDIDATES: usize = 4;

/// The prefix of a generated attachment name — what a proposal that
/// sanitizes to nothing becomes. Public so tests (and any future client-
/// side naming) can recognize a generated name without matching the whole
/// shape.
pub const GENERATED_NAME_PREFIX: &str = "attachment-";

/// The attachments root inside a state directory: one subdirectory per
/// session, as SPEC_impl.md's layout names it, plus this module's
/// reserved [`QUARANTINE_DIR`].
pub fn attachments_root(state_dir: &Path) -> PathBuf {
    state_dir.join("attachments")
}

/// Where one session's PUBLISHED attachments live.
///
/// Keyed by session id rather than by anything derived from it: the
/// directory has to be findable by delete (which knows only the id) and
/// has to survive a supervisor restart, and a session id is already the
/// one durable name this system agrees on.
pub fn session_dir(state_dir: &Path, session_id: &str) -> PathBuf {
    attachments_root(state_dir).join(session_id)
}

/// Where one session's IN-FLIGHT uploads stage. See the module docs for
/// why this is a directory rather than a filename convention.
pub fn staging_dir(state_dir: &Path, session_id: &str) -> PathBuf {
    session_dir(state_dir, session_id).join(STAGING_DIR)
}

/// Create both of a session's attachment directories, owner-only.
///
/// The staging directory is created eagerly with the published one: an
/// upload needs both to exist before its first byte, and creating them
/// together means no code path can stage into a directory that a partial
/// earlier failure left missing.
pub async fn ensure_session_dirs(state_dir: &Path, session_id: &str) -> std::io::Result<()> {
    // `ensure_private_dir` is recursive, so this one call also creates the
    // attachments root and the session directory above it.
    crate::ensure_private_dir(&staging_dir(state_dir, session_id)).await
}

/// Reduce a client's PROPOSED filename to the shell-safe basename it will
/// publish under, or `None` when nothing usable survives.
///
/// Three reductions, in order, each with a reason:
///
/// 1. **Basename only.** Everything through the last `/` is dropped, so a
///    proposal of `../../.ssh/authorized_keys` proposes `authorized_keys`
///    and nothing else. Directory traversal is not refused, it is simply
///    not expressible — the published name has no path structure to
///    traverse with.
/// 2. **Shell-safe characters.** Per [`is_shell_safe`], one `_` per
///    rejected character (never a silent deletion, so distinct names stay
///    distinct).
/// 3. **Bounded length.** Over [`MAX_NAME_BYTES`], the STEM is truncated
///    and a short extension is kept — `.png` is the part a later reader
///    (and the agent being handed the path) actually acts on.
///
/// Returns `None` exactly when nothing usable survives: an empty
/// proposal, one that is all separators, one whose basename is `.` or
/// `..` — the two names that are directory entries every directory
/// already has, and so cannot be created — or one that collides with a
/// reserved directory name, which would put a user's file where the
/// startup reconciliation deletes debris. The caller answers `None` with
/// [`generated_name`]; a refusal is never the answer, per SPEC.md.
fn sanitize(proposed: &str) -> Option<String> {
    let basename = proposed.rsplit('/').next().unwrap_or("");
    let safe: String = basename
        .chars()
        .map(|c| if is_shell_safe(c) { c } else { '_' })
        .collect();
    // Sanitizing maps rather than deletes, so a non-empty basename always
    // yields a non-empty name — but `.`, `..`, and the reserved directory
    // names all survive the mapping intact, which is why the check runs on
    // the RESULT rather than on the input.
    if safe.is_empty() || safe == "." || safe == ".." || safe == STAGING_DIR {
        return None;
    }
    Some(truncate_name(safe))
}

/// Cut an over-long sanitized name down to [`MAX_NAME_BYTES`], keeping a
/// short trailing extension.
///
/// Every character is ASCII by the time this runs (sanitizing guarantees
/// it), so byte and character positions coincide and truncation cannot
/// split anything.
fn truncate_name(name: String) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name;
    }
    // The same stem/extension split publication uses, so a truncated name
    // and its collision variants cannot disagree about where the
    // extension starts.
    let (_, extension) = split_extension(&name);
    let extension = if extension.len() <= MAX_KEPT_EXTENSION_BYTES {
        extension
    } else {
        // Not an extension in any useful sense; keeping it would eat the
        // part of the name a human would recognize.
        ""
    };
    format!("{}{}", &name[..MAX_NAME_BYTES - extension.len()], extension)
}

/// A generated attachment name, for a proposal that sanitized to nothing
/// and for the tail of the candidate sequence.
///
/// Deliberately carries no extension: the supervisor knows the byte count
/// and nothing else about the content, and inventing `.png` for bytes
/// that might be anything would be a worse lie than a name with no
/// extension at all. The client generates its own (typed) names for
/// pasted images before the proposal ever reaches here; this is the
/// fallback for a proposal that carried no usable name whatsoever.
fn generated_name() -> String {
    format!("{GENERATED_NAME_PREFIX}{}", uuid::Uuid::new_v4())
}

/// The name an upload publishes under, given whatever the client
/// proposed: [`sanitize`]'s result, or a generated name when nothing
/// survived.
///
/// Called ONCE per transfer, at admission, and the result is what the
/// transfer carries from then on — through staging, its diagnostics, and
/// its publication. Calling it twice for one upload would mint a
/// different generated name the second time, so the log and the file on
/// disk would name different things.
pub fn publish_name(proposed: &str) -> String {
    sanitize(proposed).unwrap_or_else(generated_name)
}

/// The candidate names a publication tries, in order: the name itself,
/// then `<stem>-1<.ext>`, `<stem>-2<.ext>`, and finally a few generated
/// names.
///
/// The numeric suffix goes before the extension, not after the whole
/// name, so a collision-resolved `screenshot-1.png` is still a PNG to
/// everything that reads paths — the agent this file is being handed to
/// included.
///
/// The generated tail is what keeps the never-refuse contract:
/// [`MAX_NUMBERED_CANDIDATES`] bounds how hard the walk tries to keep the
/// user's name, and past that an upload still publishes — just under a
/// name nobody chose, which beats losing the file.
pub fn name_candidates(name: &str) -> impl Iterator<Item = String> + use<> {
    let (stem, extension) = split_extension(name);
    let stem = stem.to_string();
    let extension = extension.to_string();
    let numbered = {
        let stem = stem.clone();
        let extension = extension.clone();
        (1..=MAX_NUMBERED_CANDIDATES).map(move |n| format!("{stem}-{n}{extension}"))
    };
    std::iter::once(format!("{stem}{extension}"))
        .chain(numbered)
        .chain((0..GENERATED_CANDIDATES).map(|_| generated_name()))
}

/// Split a name into its stem and its extension (including the dot, or
/// empty). A leading dot is part of the stem: `.bashrc` has no extension,
/// and treating it as one would collision-resolve to `-1.bashrc`.
///
/// Tolerates the empty name rather than indexing past it: callers reach
/// this through [`publish_name`], which never produces one, but a
/// panicking helper one refactor away from a caller that does is not a
/// contract worth relying on.
fn split_extension(name: &str) -> (&str, &str) {
    if name.is_empty() {
        return ("", "");
    }
    match name[1..].rfind('.') {
        Some(i) => name.split_at(i + 1),
        None => (name, ""),
    }
}

/// Detach one session's attachments directory from its session by
/// renaming it into [`QUARANTINE_DIR`], returning where it went (or
/// `None` when the session never received an attachment).
///
/// ## Why a rename rather than a removal
///
/// Delete has to leave one of two states behind at every instant,
/// including across a crash: the session and its attachments both exist,
/// or neither does. Removing the directory before the row is committed
/// loses a live session's files on a crash; removing it after leaves the
/// files with no row on a crash. A rename splits the difference — it is
/// atomic and fast, it happens BEFORE the row delete (so it can still
/// fail the delete closed, with the row retained for a retry), and what
/// it leaves in the crash window is debris in a reserved directory that
/// the startup reconciliation removes, not a user's file and not a
/// surviving session missing its attachments.
///
/// Fail-closed for the reason the launch artifacts and the alt-screen
/// snapshot are: these are the user's own files, delete is the last
/// moment anything comes back for them, and SPEC.md's "removed when their
/// session is deleted" is not a best-effort promise.
pub async fn quarantine_session_dir(
    state_dir: &Path,
    session_id: &str,
) -> Result<Option<PathBuf>, String> {
    let dir = session_dir(state_dir, session_id);
    let quarantine = attachments_root(state_dir).join(QUARANTINE_DIR);
    // The destination name is unique per delete so two deletes of
    // successive sessions sharing an id (impossible today, cheap to be
    // right about) cannot collide, and so a leftover from a previous crash
    // never blocks this rename.
    let parked = quarantine.join(format!("{session_id}-{}", uuid::Uuid::new_v4()));
    match tokio::fs::metadata(&dir).await {
        // The ordinary case: this session never received an attachment.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(format!(
                "checking this session's attachments ({}): {e}",
                dir.display()
            ));
        }
        Ok(_) => {}
    }
    crate::ensure_private_dir(&quarantine).await.map_err(|e| {
        format!(
            "preparing the attachment quarantine ({}): {e}",
            quarantine.display()
        )
    })?;
    tokio::fs::rename(&dir, &parked).await.map_err(|e| {
        format!(
            "detaching this session's attachments ({}): {e}",
            dir.display()
        )
    })?;
    Ok(Some(parked))
}

/// Remove a directory [`quarantine_session_dir`] parked, after the
/// session's row is gone.
///
/// Best-effort and log-only, unlike the quarantining step: by this point
/// the session no longer exists, so there is nothing to retain for a
/// retry and nothing a caller could usefully do about a failure. What is
/// left behind is debris in a reserved directory, which the next startup
/// reconciles.
pub async fn discard_quarantined(parked: &Path) {
    if let Err(e) = tokio::fs::remove_dir_all(parked).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        warn!(path = %parked.display(), error = %e,
            "could not remove a deleted session's quarantined attachments; the next startup \
             will reconcile them");
    }
}

/// Reconcile the attachments tree against the sessions that actually
/// exist — the startup half of every lifecycle rule in this module.
///
/// Three sources of debris, none of them reachable by a running
/// supervisor's own cleanup paths:
///
/// - **Staging files** from a transfer that a hard crash (`kill -9`, an
///   OOM kill, power loss) ended between staging and finishing, or whose
///   removal genuinely failed. Everything under a `.staging/` directory
///   is one of these by construction, so no name has to be interpreted.
/// - **Quarantined directories** from a delete that crashed between the
///   rename and the removal (see [`quarantine_session_dir`]).
/// - **Whole session directories** whose session no longer exists —
///   either a delete that crashed after committing the row removal, or a
///   database restored from under this tree. `known_sessions` is the
///   authority; a directory not named in it belongs to nothing.
///
/// Best-effort and log-only: startup must not fail over debris, and a
/// leftover file is wasted bytes rather than a correctness problem for
/// anything the caller is about to do. PUBLISHED attachments of KNOWN
/// sessions are never touched — a sweep that took those would be data
/// loss dressed up as tidiness.
pub async fn reconcile_at_startup(
    state_dir: &Path,
    known_sessions: &std::collections::HashSet<String>,
) {
    let root = attachments_root(state_dir);
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        // No attachments directory at all: nothing on this state dir has
        // ever been uploaded. The ordinary case, not worth a log line.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            warn!(error = %e,
                "could not reconcile the attachments directory; debris may remain");
            return;
        }
    };
    loop {
        match entries.next_entry().await {
            Ok(None) => break,
            Ok(Some(entry)) => {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == QUARANTINE_DIR {
                    discard_quarantine_root(&entry.path()).await;
                } else if known_sessions.contains(&name) {
                    sweep_staging(&entry.path().join(STAGING_DIR)).await;
                } else {
                    // A session directory with no session. Removing it is
                    // the completion of a delete that did not survive to
                    // finish, which is why it takes the whole tree rather
                    // than just its staging area.
                    match tokio::fs::remove_dir_all(entry.path()).await {
                        Ok(()) => debug!(path = %entry.path().display(),
                            "removed the attachments of a session that no longer exists"),
                        Err(e) => warn!(path = %entry.path().display(), error = %e,
                            "could not remove the attachments of a session that no longer exists"),
                    }
                }
            }
            Err(e) => {
                warn!(error = %e,
                    "attachment reconciliation aborted early; debris may remain");
                break;
            }
        }
    }
}

/// Remove everything a delete parked in the quarantine directory but
/// never got to discard.
async fn discard_quarantine_root(quarantine: &Path) {
    let mut entries = match tokio::fs::read_dir(quarantine).await {
        Ok(entries) => entries,
        Err(e) => {
            warn!(path = %quarantine.display(), error = %e,
                "could not read the attachment quarantine; debris may remain");
            return;
        }
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        discard_quarantined(&entry.path()).await;
    }
}

/// Empty one session's staging directory. Everything in it is an
/// interrupted transfer's staging file — nothing else is ever written
/// there — so this needs no name test at all.
async fn sweep_staging(staging: &Path) {
    let mut entries = match tokio::fs::read_dir(staging).await {
        Ok(entries) => entries,
        // A session that has never received an upload has no staging
        // directory yet; that is not debris and not a problem.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            warn!(path = %staging.display(), error = %e,
                "could not sweep an attachment staging directory; debris may remain");
            return;
        }
    };
    loop {
        match entries.next_entry().await {
            Ok(None) => break,
            Ok(Some(entry)) => match tokio::fs::remove_file(entry.path()).await {
                Ok(()) => debug!(path = %entry.path().display(),
                    "removed an interrupted upload's staging file"),
                Err(e) => warn!(path = %entry.path().display(), error = %e,
                    "could not remove an interrupted upload's staging file"),
            },
            Err(e) => {
                warn!(path = %staging.display(), error = %e,
                    "staging sweep aborted early for this session; debris may remain");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The sanitizing rules, as a table of the cases that actually
    /// motivate them: a shell-hostile name publishes under a safe one, a
    /// path proposal keeps only its basename (so traversal is not
    /// expressible rather than merely refused), and a name that is
    /// already safe is left exactly as it was — the recognizability half
    /// of the contract, which an over-eager sanitizer would quietly break.
    #[test]
    fn sanitizing_makes_names_shell_safe_without_rewriting_safe_ones() {
        for (proposed, expected) in [
            ("screenshot.png", "screenshot.png"),
            ("Screen Shot 2026-08-02.png", "Screen_Shot_2026-08-02.png"),
            ("rm -rf $HOME; echo.txt", "rm_-rf__HOME__echo.txt"),
            ("a'b\"c`d.txt", "a_b_c_d.txt"),
            ("../../.ssh/authorized_keys", "authorized_keys"),
            ("/etc/passwd", "passwd"),
            ("naïve-∑.txt", "na_ve-_.txt"),
            (".bashrc", ".bashrc"),
            // A published name that LOOKS like staging debris is
            // perfectly legal, and must survive both sanitizing and every
            // sweep — the reason staging lives in its own directory.
            ("report.tmp-backup", "report.tmp-backup"),
        ] {
            assert_eq!(publish_name(proposed), expected, "proposed: {proposed:?}");
        }
    }

    /// The names that sanitize to NOTHING take a generated name rather
    /// than a refusal — SPEC.md rejects directories, never a file for
    /// what it is called.
    ///
    /// `.` and `..` are the interesting members: they survive the
    /// character mapping unchanged and are the two names no directory
    /// entry can ever be created under, so a sanitizer that only checked
    /// for emptiness would produce a name that cannot be published at
    /// all. `.staging` is the same shape one layer up: publishable, but
    /// it would put a user's file exactly where the startup
    /// reconciliation deletes debris.
    #[test]
    fn unusable_proposals_take_a_generated_name_instead_of_a_refusal() {
        for proposed in ["", ".", "..", "/", "dir/..", "some/path/", ".staging"] {
            let name = publish_name(proposed);
            assert!(
                name.starts_with(GENERATED_NAME_PREFIX),
                "proposal {proposed:?} must fall back to a generated name, got {name:?}"
            );
            assert!(
                name.chars().all(is_shell_safe),
                "a generated name must itself be shell-safe, got {name:?}"
            );
        }
        assert_ne!(
            publish_name(""),
            publish_name(""),
            "generated names must be unique, or two nameless uploads would collide by \
             construction on every publish"
        );
    }

    /// An over-long proposal is truncated, not refused — and keeps its
    /// extension WITH its dot, because `report.png` and `reportpng` are
    /// not the same thing to anything that opens the file. The bound is a
    /// filesystem limit (a path component is 255 bytes, and the staging
    /// name adds ~42 more), so exceeding it would fail the upload at
    /// `open` rather than merely look untidy.
    #[test]
    fn an_over_long_name_is_truncated_with_its_extension_kept() {
        let name = publish_name(&format!("{}.png", "a".repeat(500)));
        assert!(
            name.len() <= MAX_NAME_BYTES,
            "truncated name is {} bytes, over the {MAX_NAME_BYTES}-byte cap",
            name.len()
        );
        assert!(
            name.ends_with(".png"),
            "the extension must survive with its dot: {name:?}"
        );
        assert!(name.starts_with("aaaa"), "the stem must survive: {name:?}");

        // An "extension" too long to be one is not worth preserving at
        // the stem's expense.
        let junk = publish_name(&format!("{}.{}", "a".repeat(200), "b".repeat(200)));
        assert!(junk.len() <= MAX_NAME_BYTES);
        assert!(junk.starts_with("aaaa"), "the stem must survive: {junk:?}");
    }

    /// Collision candidates put the numeric suffix before the extension,
    /// so a resolved name is still a PNG to whatever opens it, and start
    /// with the unsuffixed name so an uncontended upload publishes under
    /// exactly the name it proposed.
    ///
    /// The tail matters as much as the head: after the bounded numbered
    /// walk the sequence continues with GENERATED names rather than
    /// ending, because an upload that cannot be named is still an upload
    /// SPEC.md does not let us refuse.
    #[test]
    fn collision_candidates_suffix_the_stem_then_fall_back_to_generated_names() {
        let candidates: Vec<String> = name_candidates("screenshot.png").take(3).collect();
        assert_eq!(
            candidates,
            ["screenshot.png", "screenshot-1.png", "screenshot-2.png"]
        );

        let extensionless: Vec<String> = name_candidates("notes").take(2).collect();
        assert_eq!(extensionless, ["notes", "notes-1"]);

        // A leading dot is a stem, not an extension: `-1.bashrc` would be
        // a different FILE TYPE to every tool that reads names.
        let dotfile: Vec<String> = name_candidates(".bashrc").take(2).collect();
        assert_eq!(dotfile, [".bashrc", ".bashrc-1"]);

        let all: Vec<String> = name_candidates("x").collect();
        assert_eq!(
            all.len(),
            MAX_NUMBERED_CANDIDATES + 1 + GENERATED_CANDIDATES
        );
        for generated in &all[MAX_NUMBERED_CANDIDATES + 1..] {
            assert!(
                generated.starts_with(GENERATED_NAME_PREFIX),
                "the sequence must end in generated names, got {generated:?}"
            );
        }

        // Never reached through `publish_name`, which cannot produce an
        // empty name — but a helper that panics on one is a trap for the
        // next caller.
        assert_eq!(name_candidates("").next(), Some(String::new()));
    }

    /// Startup reconciliation empties staging directories and leaves
    /// published attachments alone — including one whose NAME looks
    /// exactly like a staging file, which is the case that makes the
    /// directory split load-bearing rather than stylistic.
    #[tokio::test]
    async fn reconciliation_empties_staging_and_keeps_published_files() {
        let state = tempfile::tempdir().unwrap();
        let session = "9f8e7d6c-0000-4000-8000-000000000000";
        ensure_session_dirs(state.path(), session).await.unwrap();
        let published = session_dir(state.path(), session).join("screenshot.png");
        let decoy = session_dir(state.path(), session).join("report.tmp-backup");
        let staged = staging_dir(state.path(), session).join(".screenshot.png.tmp-1234");
        tokio::fs::write(&published, b"real attachment")
            .await
            .unwrap();
        tokio::fs::write(&decoy, b"also a real attachment")
            .await
            .unwrap();
        tokio::fs::write(&staged, b"half an upload").await.unwrap();

        let known = HashSet::from([session.to_string()]);
        reconcile_at_startup(state.path(), &known).await;

        assert!(
            !staged.exists(),
            "an interrupted upload's staging file must be swept"
        );
        assert_eq!(
            tokio::fs::read(&published).await.unwrap(),
            b"real attachment"
        );
        assert_eq!(
            tokio::fs::read(&decoy).await.unwrap(),
            b"also a real attachment",
            "a published file whose NAME resembles staging debris must survive"
        );
        assert!(
            staging_dir(state.path(), session).exists(),
            "the staging directory itself must survive, ready for the next upload"
        );
    }

    /// Reconciliation removes what belongs to nothing: a session
    /// directory with no session (a delete that crashed after committing
    /// its row removal) and anything a delete parked in quarantine.
    ///
    /// Both are cases where the alternative — leaving them — means a
    /// deleted session's files living on indefinitely, which is exactly
    /// what SPEC.md's "removed when their session is deleted" forbids.
    #[tokio::test]
    async fn reconciliation_removes_orphans_and_quarantined_directories() {
        let state = tempfile::tempdir().unwrap();
        ensure_session_dirs(state.path(), "live-session")
            .await
            .unwrap();
        ensure_session_dirs(state.path(), "deleted-session")
            .await
            .unwrap();
        let kept = session_dir(state.path(), "live-session").join("keep.png");
        let orphan = session_dir(state.path(), "deleted-session").join("gone.png");
        tokio::fs::write(&kept, b"keep").await.unwrap();
        tokio::fs::write(&orphan, b"gone").await.unwrap();

        let parked = quarantine_session_dir(state.path(), "deleted-session")
            .await
            .unwrap()
            .expect("a session with attachments must be quarantined");
        assert!(parked.exists(), "quarantining must not remove anything yet");

        let known = HashSet::from(["live-session".to_string()]);
        reconcile_at_startup(state.path(), &known).await;

        assert!(
            !parked.exists(),
            "quarantined attachments must be discarded"
        );
        assert_eq!(tokio::fs::read(&kept).await.unwrap(), b"keep");
    }

    /// Quarantining is a rename, not a removal: the files still exist
    /// (under the reserved directory) until the caller discards them, and
    /// a session that never received an attachment quarantines nothing
    /// rather than failing its own delete.
    #[tokio::test]
    async fn quarantining_moves_the_directory_and_tolerates_absence() {
        let state = tempfile::tempdir().unwrap();
        ensure_session_dirs(state.path(), "session-a")
            .await
            .unwrap();
        let file = session_dir(state.path(), "session-a").join("shot.png");
        tokio::fs::write(&file, b"bytes").await.unwrap();

        let parked = quarantine_session_dir(state.path(), "session-a")
            .await
            .unwrap()
            .expect("a session with attachments must be quarantined");
        assert!(!session_dir(state.path(), "session-a").exists());
        assert_eq!(
            tokio::fs::read(parked.join("shot.png")).await.unwrap(),
            b"bytes",
            "quarantining must preserve the files until they are discarded"
        );

        discard_quarantined(&parked).await;
        assert!(!parked.exists());

        assert_eq!(
            quarantine_session_dir(state.path(), "never-existed")
                .await
                .unwrap(),
            None,
            "a session with no attachments must quarantine nothing, not fail"
        );
    }
}
