//! A slot's clone of the records repo: creating it, bringing it up to date, and pushing records from it.
//!
//! Each slot has one ordinary (non-bare) clone at `<clones_dir>/<slot>/`, and the records' directories in its working
//! tree are where the slot's agent reads and writes. Local edits are the working tree's differences from the clone's
//! `HEAD`: modified, added, deleted, untracked and ignored files, plus anything staged. Four rules carry the design:
//!
//! 1. **Bringing a clone up to date touches only what the remote changed.** Files the remote changed since `HEAD` are
//!    updated; every other local edit stays as it is.
//! 2. **An overlap is resolved by who owns the record.** When a remote-changed file also has a local edit:
//!    - If the working tree already holds exactly the remote's version (same content and mode, or absent in both), it
//!      is no conflict and the clone simply takes the remote state. This is how a push heals that the server
//!      accepted while this client saw a failure (a dropped status report, a timeout, a killed process, a crash
//!      between the push and the ref update): the next call finds its own files already on the remote.
//!    - If every commit that changed the path between `HEAD` and the remote head carries this slot's own stamp
//!      (author `records <slot>`), the local file wins: it stays as it is and only the index takes the remote's
//!      entry, with nothing set aside and nothing refused. A slot cannot conflict with itself: its own pushed
//!      history is an ancestor of its live file. This is the realistic sequence after a push the remote accepted
//!      but the client saw fail: the flow stops, appends a note to its log, and resumes, so the live file is the
//!      pushed version plus more, and the next push carries all of it. Ownership is read from the stamp alone, so it
//!      relies on slot names being unique across every host that shares the repository: two hosts each running an
//!      `exec-1` would take each other's changes for their own and overwrite them.
//!    - Otherwise, during `push`, an overlap inside one of the records being pushed refuses the push (exit 1) and
//!      changes nothing: it would mean two writers on one record.
//!    - Any other overlap (any record during `path`, records not being pushed during `push`) is won by the remote.
//!      The local file is first copied to `.git/records-set-aside/<UTC timestamp>/<path>` and one stderr line names
//!      it. A record has one writer at a time, so a local edit that collides with another slot's change can only be
//!      a stale leftover from a claim this slot no longer holds (a plan it released, blocked or gave back, which
//!      another slot then claimed). Whatever a slot did not push is lost to the next holder by design; setting it
//!      aside keeps that loss from being silent, and keeps a stale leftover from wedging the slot's own pushes.
//! 3. **A push carries exactly the named directories.** The commit is built in a scratch index from the remote head's
//!    tree with the named directories replaced by what the working tree holds right now, so a push needs neither a
//!    clean tree nor a rebase. Other records' local edits stay uncommitted, and an edit that lands after the snapshot
//!    stays a local edit for the next push.
//! 4. **Never force.** When another slot pushed first, the push fetches, brings the clone up to date, rebuilds the
//!    commit on the new head and retries, a bounded number of times. Slots write disjoint directories, so the rebuilt
//!    commit is the same change on a newer base.
//!
//! The remote's `main` must only move forward. A clone whose `HEAD` is not an ancestor of the fetched head (history
//! rewritten, or the remote replaced) is refused rather than reset.
//!
//! Concurrency: every `path` and `push` from one slot holds an exclusive lock on `.git/records.lock` for its whole
//! run, so a slot's own calls serialize. Different slots have different clones and meet only at the remote. The lock
//! file descriptor is close-on-exec (Rust's default), so git children never inherit it.
//!
//! Interrupted operations heal through the first two cases of rule 2 rather than through transactions. The likeliest
//! case is a push the server accepted while the client recorded nothing (any failure after the commit was sent): the
//! remote holds the commit, the clone's `HEAD` and index still name the old base, and the working tree holds the
//! pushed files, which therefore match the remote and are taken as is by the next call; if the agent has written to
//! them since, the commit is its own and the newer local files win. A crash after a sync has
//! updated the index and working tree but before it moved `HEAD` leaves the same shape and heals the same way. A
//! crash after a push moved `HEAD` but before it installed the new index leaves the pushed files looking staged
//! against `HEAD` while their content already matches it; that is harmless, and the next push carrying those
//! records installs a fresh index.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Failure;
use super::git::Git;

/// The branch every slot pushes to.
const MAIN: &str = "refs/heads/main";
/// Where `fetch` leaves the remote's `main`.
const REMOTE_MAIN: &str = "refs/remotes/origin/main";
/// How many times a push is built and sent before giving up on a remote that keeps moving or stays busy.
///
/// Artifacts serializes pushes to one repo: with six slots pushing at once (measured 2026-11-26: all 30 pushes landed,
/// in about twelve seconds), most attempts lost the race (`stale ref`) or were turned away as busy, so a handful of
/// attempts is not enough when every slot pushes before the same kind of step. Twelve attempts with the backoff below
/// span about half a minute.
const MAX_PUSH_ATTEMPTS: usize = 12;
/// The first retry waits about this long; each later one doubles it, up to `MAX_BACKOFF_MS`, plus random jitter so
/// slots that collided once do not collide again in lockstep.
const FIRST_BACKOFF_MS: u64 = 150;
const MAX_BACKOFF_MS: u64 = 4000;
/// The committer email on every records commit: a placeholder, so commits carry the slot name and no personal data.
const AUTHOR_EMAIL: &str = "records@farhelm.invalid";
/// The lock file inside `.git` that serializes one slot's calls.
const LOCK_FILE: &str = "records.lock";
/// The scratch index a push builds its tree in. It becomes the clone's index when the push lands.
const PUSH_INDEX: &str = "records-push.index";
/// Where overlapping local edits are copied before the remote's version replaces them (rule 2 in the module docs).
const SET_ASIDE_DIR: &str = "records-set-aside";
/// Prefix of the directory under `.git` that holds this slot's own re-created files while a sync's merge runs.
const HOLD_DIR: &str = "records-hold";

/// What `push` did.
#[derive(Debug, PartialEq, Eq)]
pub enum PushOutcome {
    /// The named directories already match the remote head; nothing was committed or sent.
    NothingToPush,
    /// This commit is now the remote's `main` and the clone's `HEAD`.
    Pushed(String),
}

/// An open, locked slot clone. The lock is released when this is dropped.
pub struct SlotClone<'g> {
    git: &'g Git,
    root: PathBuf,
    /// The commit author that marks a remote change as this slot's own (`records <slot>`).
    author: String,
    _lock: File,
}

impl<'g> SlotClone<'g> {
    /// Opens the slot's clone, creating an empty one first if it does not exist, and takes its lock.
    ///
    /// Creation fetches nothing: the first `sync` does, so a new clone and an old one go through the same path. An
    /// existing clone must fetch from `remote` and have `main` checked out; a clone pointing elsewhere is refused
    /// rather than repointed, since its history may belong to another repo.
    pub fn open(
        git: &'g Git,
        clones_dir: &Path,
        slot: &str,
        remote: &str,
    ) -> Result<Self, Failure> {
        let root = clones_dir.join(slot);
        if !root.exists() {
            create(git, clones_dir, &root, slot, remote)?;
        }
        let git_dir = root.join(".git");
        if !git_dir.is_dir() {
            return Err(Failure::failed(format!(
                "{} exists but is not a records clone (it has no .git directory)",
                root.display()
            )));
        }
        let lock_path = git_dir.join(LOCK_FILE);
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|e| Failure::failed(format!("cannot open {}: {e}", lock_path.display())))?;
        lock.lock()
            .map_err(|e| Failure::failed(format!("cannot lock {}: {e}", lock_path.display())))?;
        let clone = Self {
            git,
            root,
            author: format!("records {slot}"),
            _lock: lock,
        };
        clone.check_shape(remote)?;
        Ok(clone)
    }

    /// The clone's working tree root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Refuses a clone that fetches from another remote or has another branch checked out.
    fn check_shape(&self, remote: &str) -> Result<(), Failure> {
        let url = self
            .git
            .at(&self.root)
            .args(["config", "--get", "remote.origin.url"])
            .output()?
            .text();
        if url != remote {
            // Neither URL is quoted: an Artifacts URL carries the account id, and failure messages end up in logs.
            return Err(Failure::failed(format!(
                "the clone {} fetches from a different remote than the config names; move the clone aside to start \
                 a fresh one",
                self.root.display()
            )));
        }
        let head = self
            .git
            .at(&self.root)
            .args(["symbolic-ref", "-q", "HEAD"])
            .output()?
            .text();
        if head != MAIN {
            return Err(Failure::failed(format!(
                "the clone {} has `{head}` checked out instead of {MAIN}",
                self.root.display()
            )));
        }
        Ok(())
    }

    /// Fetches the remote and brings the working tree up to date with its `main` (rules 1 and 2 in the module docs).
    ///
    /// `pushing` names the record directories a push is about to carry (empty for `path`): an overlap inside them
    /// refuses instead of being set aside.
    pub fn sync(&self, pushing: &[String]) -> Result<(), Failure> {
        self.git
            .at(&self.root)
            .args(["fetch", "--quiet", "--prune", "--no-tags", "origin"])
            .run()
            .map_err(|f| f.context("fetching the records remote"))?;
        let old = self.resolve(MAIN)?;
        let new = self.resolve(REMOTE_MAIN)?;
        let new = match (&old, new) {
            (None, None) => return Ok(()),
            (Some(_), None) => {
                return Err(Failure::failed(format!(
                    "the remote has no main branch but the clone {} has commits; refusing to continue",
                    self.root.display()
                )));
            }
            (_, Some(new)) if old.as_deref() == Some(new.as_str()) => return Ok(()),
            (_, Some(new)) => new,
        };
        if let Some(old) = &old
            && !self.is_ancestor(old, &new)?
        {
            return Err(Failure::failed(format!(
                "the remote's main ({new}) does not contain the clone's HEAD ({old}): its history was rewritten; \
                 refusing to update {}",
                self.root.display()
            )));
        }
        let changed = self.changed_paths(old.as_deref(), &new)?;
        let local = self.local_edits()?;
        // Rule 2, first two cases: a path whose working tree already equals the remote's version, or whose remote
        // changes are all this slot's own pushes, is not a conflict.
        let mut already_remote = Vec::new();
        let mut own = Vec::new();
        let mut conflicts = Vec::new();
        for (remote_path, local_path) in colliding_pairs(&changed, &local) {
            if remote_path != local_path {
                conflicts.push((remote_path, local_path));
            } else if self.matches(&new, &remote_path)? {
                already_remote.push(remote_path);
            } else if self.only_own_changes(old.as_deref(), &new, &remote_path)? {
                own.push(remote_path);
            } else {
                conflicts.push((remote_path, local_path));
            }
        }
        // Rule 2, second case: two writers on a record being pushed. Checked before anything changes.
        let mut owned: Vec<String> = conflicts
            .iter()
            .filter(|(r, l)| inside_any(r, pushing) || inside_any(l, pushing))
            .map(|(r, _)| r.clone())
            .collect();
        owned.sort();
        owned.dedup();
        if !owned.is_empty() {
            return Err(Failure::failed(format!(
                "the remote changed files in the records being pushed that also have local edits in {}, so nothing \
                 was updated or pushed: {}",
                self.root.display(),
                owned.join(", ")
            )));
        }
        // Rule 2, third case: the remote wins, after the local edits are copied aside.
        let mut stale: Vec<String> = conflicts.into_iter().map(|(_, l)| l).collect();
        stale.sort();
        stale.dedup();
        if !stale.is_empty() {
            self.set_aside(&stale, old.as_deref())?;
        }
        if !already_remote.is_empty() {
            self.stage_working_tree(&already_remote)?;
        }
        // An own path the new tree lacks but the working tree has (the agent re-created a file its own accepted push
        // deleted) would look untracked to the merge, which refuses to remove it. It is held under .git while the
        // merge runs and put back afterwards, whatever the merge's outcome.
        let mut hold = Vec::new();
        for path in &own {
            let absent_in_new = self.stage_remote_entry(&new, path)?;
            if absent_in_new && fs::symlink_metadata(self.root.join(path)).is_ok() {
                hold.push(path.clone());
            }
        }
        let held = self.hold(&hold)?;
        let merged = self.merge(old.as_deref(), &new);
        let restored = self.release(&held);
        merged.and(restored)
    }

    /// The two-tree merge from `old` (or the empty tree) to `new`, then `main` moved to `new`.
    ///
    /// The merge moves the index and working tree, touching only paths that differ between the trees and keeping
    /// every other local change. By the time `sync` calls this, every changed path either has no local edit (set-aside
    /// paths were restored to the old tree) or is staged at exactly the new tree's entry, which the merge accepts as
    /// already done without touching the working tree (so this slot's own newer files stay).
    fn merge(&self, old: Option<&str>, new: &str) -> Result<(), Failure> {
        let old_tree = match old {
            Some(old) => old.to_string(),
            None => self.empty_tree()?,
        };
        self.git
            .at(&self.root)
            .args(["read-tree", "-m", "-u", &old_tree, new])
            .run()?;
        self.git
            .at(&self.root)
            .args([
                "update-ref",
                "-m",
                "records: sync",
                MAIN,
                new,
                old.unwrap_or_default(),
            ])
            .run()?;
        Ok(())
    }

    /// Moves each of `paths` out of the working tree into a fresh directory under `.git`, returning where each went.
    /// On a failure partway, what was already moved is put back before the error returns.
    fn hold(&self, paths: &[String]) -> Result<Vec<(PathBuf, PathBuf)>, Failure> {
        let mut held = Vec::new();
        if paths.is_empty() {
            return Ok(held);
        }
        let dir = self
            .root
            .join(".git")
            .join(format!("{HOLD_DIR}-{}", std::process::id()));
        let result = (|| {
            fs::create_dir_all(&dir)
                .map_err(|e| Failure::failed(format!("cannot create {}: {e}", dir.display())))?;
            for (i, path) in paths.iter().enumerate() {
                let from = self.root.join(path);
                let to = dir.join(i.to_string());
                fs::rename(&from, &to).map_err(|e| {
                    Failure::failed(format!("cannot move {} aside: {e}", from.display()))
                })?;
                held.push((from, to));
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(held),
            Err(failure) => {
                let _ = self.release(&held);
                Err(failure)
            }
        }
    }

    /// Puts files moved by `hold` back where they were, recreating parent directories the merge may have removed.
    fn release(&self, held: &[(PathBuf, PathBuf)]) -> Result<(), Failure> {
        let mut failures = Vec::new();
        for (original, holding) in held {
            let moved = original
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| fs::rename(holding, original));
            if let Err(e) = moved {
                failures.push(format!(
                    "{} is kept at {} ({e})",
                    original.display(),
                    holding.display()
                ));
            }
        }
        if let Some((_, holding)) = held.first()
            && failures.is_empty()
            && let Some(dir) = holding.parent()
        {
            let _ = fs::remove_dir(dir);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(Failure::failed(format!(
                "could not put local files back after the update: {}",
                failures.join("; ")
            )))
        }
    }

    /// Commits exactly `directories` (relative to the root) as they are in the working tree, on top of the remote
    /// head, and pushes the commit to `main` without force (rules 2 and 3 in the module docs).
    pub fn push(
        &self,
        directories: &[String],
        message: &str,
        slot: &str,
    ) -> Result<PushOutcome, Failure> {
        let git_dir = self.root.join(".git");
        let index = git_dir.join(PUSH_INDEX);
        let mut last_rejection = String::new();
        for attempt in 0..MAX_PUSH_ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(backoff(attempt));
            }
            self.sync(directories)?;
            let base = self.resolve(MAIN)?;
            remove_if_present(&index)?;
            let tree = self.snapshot_tree(base.as_deref(), directories, &index);
            let tree = match tree {
                Ok(tree) => tree,
                Err(failure) => {
                    let _ = fs::remove_file(&index);
                    return Err(failure);
                }
            };
            let base_tree = match &base {
                Some(base) => self
                    .git
                    .at(&self.root)
                    .args(["rev-parse", &format!("{base}^{{tree}}")])
                    .run()?
                    .text(),
                None => self.empty_tree()?,
            };
            if tree == base_tree {
                remove_if_present(&index)?;
                return Ok(PushOutcome::NothingToPush);
            }
            let commit = self.commit(&tree, base.as_deref(), message, slot)?;
            match self.send(&commit)? {
                Sent::Accepted => {
                    let expected_old = base.unwrap_or_default();
                    self.git
                        .at(&self.root)
                        .args([
                            "update-ref",
                            "-m",
                            "records: push",
                            MAIN,
                            &commit,
                            &expected_old,
                        ])
                        .run()?;
                    // The scratch index matches the new HEAD exactly, and its entries for the pushed directories carry
                    // the stat data from the snapshot, so any file changed after the snapshot shows as modified.
                    fs::rename(&index, git_dir.join("index")).map_err(|e| {
                        Failure::failed(format!("cannot install the clone's index: {e}"))
                    })?;
                    return Ok(PushOutcome::Pushed(commit));
                }
                Sent::Raced(reason) => {
                    remove_if_present(&index)?;
                    last_rejection = reason;
                }
            }
        }
        Err(Failure::failed(format!(
            "the push was rejected {MAX_PUSH_ATTEMPTS} times because the remote kept moving or was busy \
             ({last_rejection}); nothing was forced"
        )))
    }

    /// Builds the tree for a push in the scratch index `index` and returns its id: the base tree with each of
    /// `directories` replaced by the working tree's files there (respecting the repo's own `.gitignore`).
    fn snapshot_tree(
        &self,
        base: Option<&str>,
        directories: &[String],
        index: &Path,
    ) -> Result<String, Failure> {
        let at = || self.git.at(&self.root).env("GIT_INDEX_FILE", index);
        match base {
            Some(base) => at().args(["read-tree", base]).run()?,
            None => at().args(["read-tree", "--empty"]).run()?,
        };
        // Dropping the directories from the index and re-adding whatever files exist there now handles additions,
        // modifications and deletions alike, and works for an empty or missing directory, where `git add -A` would
        // fail on a pathspec that matches nothing.
        at().args(["rm", "-r", "-q", "--cached", "--ignore-unmatch", "--"])
            .args(directories)
            .run()?;
        let files = at()
            .args(["ls-files", "-z", "--others", "--exclude-standard", "--"])
            .args(directories)
            .run()?
            .stdout;
        if !files.is_empty() {
            // --remove makes a file deleted since ls-files listed it drop out instead of failing the push.
            at().args(["update-index", "--add", "--remove", "-z", "--stdin"])
                .stdin(files)
                .run()?;
        }
        Ok(at().args(["write-tree"]).run()?.text())
    }

    /// Writes the commit object: author and committer `records <slot>`, the message followed by a `Slot:` trailer.
    fn commit(
        &self,
        tree: &str,
        parent: Option<&str>,
        message: &str,
        slot: &str,
    ) -> Result<String, Failure> {
        let name = format!("records {slot}");
        let body = format!("{}\n\nSlot: {slot}\n", message.trim_end());
        let mut call = self
            .git
            .at(&self.root)
            .args(["commit-tree", "--no-gpg-sign", tree]);
        if let Some(parent) = parent {
            call = call.args(["-p", parent]);
        }
        let output = call
            .args(["-F", "-"])
            .env("GIT_AUTHOR_NAME", &name)
            .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
            .env("GIT_COMMITTER_NAME", &name)
            .env("GIT_COMMITTER_EMAIL", AUTHOR_EMAIL)
            .stdin(body.into_bytes())
            .run()?;
        Ok(output.text())
    }

    /// Pushes `commit` to the remote's `main` without force and classifies the result.
    fn send(&self, commit: &str) -> Result<Sent, Failure> {
        let call = self.git.at(&self.root).args([
            "push",
            "--porcelain",
            "origin",
            &format!("{commit}:{MAIN}"),
        ]);
        let described = call.describe();
        let output = call.output()?;
        if output.status.success() {
            return Ok(Sent::Accepted);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        match raced_rejection(&stdout).or_else(|| remote_busy(&stderr)) {
            Some(reason) => Ok(Sent::Raced(reason)),
            None => Err(Failure::failed(format!(
                "{described} failed ({}): {} {}",
                output.status,
                stdout.trim(),
                stderr.trim()
            ))),
        }
    }

    /// The commit a ref points at, or `None` when the ref does not exist (an unborn `main`, an empty remote).
    fn resolve(&self, reference: &str) -> Result<Option<String>, Failure> {
        let call = self.git.at(&self.root).args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ]);
        let output = call.output()?;
        if output.status.success() {
            Ok(Some(output.text()))
        } else if output.status.code() == Some(1) && output.stdout.is_empty() {
            Ok(None)
        } else {
            Err(Failure::failed(format!(
                "cannot resolve {reference}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }

    fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, Failure> {
        let output = self
            .git
            .at(&self.root)
            .args(["merge-base", "--is-ancestor", ancestor, descendant])
            .output()?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Failure::failed(format!(
                "git merge-base failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))),
        }
    }

    /// The empty tree's id in this repo's hash format.
    fn empty_tree(&self) -> Result<String, Failure> {
        Ok(self
            .git
            .at(&self.root)
            .args(["hash-object", "-t", "tree", "--stdin"])
            .stdin(Vec::new())
            .run()?
            .text())
    }

    /// Paths that differ between `old` (or the empty tree) and `new`, relative to the root.
    fn changed_paths(&self, old: Option<&str>, new: &str) -> Result<Vec<String>, Failure> {
        let output = match old {
            Some(old) => self
                .git
                .at(&self.root)
                .args([
                    "diff",
                    "--name-only",
                    "-z",
                    "--no-renames",
                    "--no-ext-diff",
                    old,
                    new,
                    "--",
                ])
                .run()?,
            None => self
                .git
                .at(&self.root)
                .args(["ls-tree", "-r", "-z", "--name-only", new])
                .run()?,
        };
        Ok(split_nul(&output.stdout))
    }

    /// Every path with a local edit (rule 1), including untracked and ignored files: an ignored file is still a file
    /// the agent wrote, and the sync must not overwrite it.
    fn local_edits(&self) -> Result<Vec<String>, Failure> {
        let output = self
            .git
            .at(&self.root)
            .args([
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--no-renames",
            ])
            .run()?;
        // Each entry is "XY <path>"; --no-renames rules out the two-path rename form. A wholly ignored directory is
        // reported once with a trailing `/`, which is dropped so it compares like any other path.
        Ok(split_nul(&output.stdout)
            .into_iter()
            .filter_map(|entry| entry.get(3..).map(|p| p.trim_end_matches('/').to_string()))
            .collect())
    }

    /// Whether the working tree at `path` already holds `commit`'s version: the same blob and mode, or absent in both.
    fn matches(&self, commit: &str, path: &str) -> Result<bool, Failure> {
        let listing = self
            .git
            .at(&self.root)
            .args(["ls-tree", "-z", commit, "--", path])
            .run()?
            .stdout;
        // "<mode> <type> <id>\t<path>"; only a blob entry can match a file.
        let remote = split_nul(&listing).into_iter().next().and_then(|entry| {
            let (meta, _) = entry.split_once('\t')?;
            let mut fields = meta.split(' ');
            let (mode, kind, id) = (fields.next()?, fields.next()?, fields.next()?);
            (kind == "blob").then(|| (mode.to_string(), id.to_string()))
        });
        let full = self.root.join(path);
        let local = match fs::symlink_metadata(&full) {
            // A path under what is now a file (`notes` replaced by a file, say) is just as absent.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                None
            }
            Err(e) => {
                return Err(Failure::failed(format!(
                    "cannot inspect {}: {e}",
                    full.display()
                )));
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = fs::read_link(&full).map_err(|e| {
                    Failure::failed(format!("cannot read the link {}: {e}", full.display()))
                })?;
                let id = self
                    .git
                    .at(&self.root)
                    .args(["hash-object", "--stdin"])
                    .stdin(target.into_os_string().into_encoded_bytes())
                    .run()?
                    .text();
                Some(("120000".to_string(), id))
            }
            Ok(meta) if meta.is_file() => {
                use std::os::unix::fs::PermissionsExt;
                let mode = if meta.permissions().mode() & 0o111 != 0 {
                    "100755"
                } else {
                    "100644"
                };
                let id = self
                    .git
                    .at(&self.root)
                    .args(["hash-object", "--", path])
                    .run()?
                    .text();
                Some((mode.to_string(), id))
            }
            // A directory (or anything else) where the remote has a file, or where it has nothing: not a match.
            Ok(_) => return Ok(false),
        };
        Ok(local == remote)
    }

    /// Whether every commit in `old..new` (all of `new`'s history when `old` is unborn) that touched `path` was
    /// written by this slot. At least one such commit exists, since the caller only asks about changed paths.
    fn only_own_changes(&self, old: Option<&str>, new: &str, path: &str) -> Result<bool, Failure> {
        let range = match old {
            Some(old) => format!("{old}..{new}"),
            None => new.to_string(),
        };
        let authors = self
            .git
            .at(&self.root)
            .args(["log", "--format=%an", &range, "--", path])
            .run()?
            .text();
        Ok(!authors.is_empty() && authors.lines().all(|author| author == self.author))
    }

    /// Puts `commit`'s entry for `path` (or its absence) into the index without touching the working tree, so the
    /// two-tree merge in `sync` accepts the path as already moved and the local file stays a local edit on top.
    /// Returns whether `commit` lacks the path.
    fn stage_remote_entry(&self, commit: &str, path: &str) -> Result<bool, Failure> {
        let listing = self
            .git
            .at(&self.root)
            .args(["ls-tree", "-z", commit, "--", path])
            .run()?
            .stdout;
        let entry = split_nul(&listing).into_iter().next().and_then(|entry| {
            let (meta, _) = entry.split_once('\t')?;
            let mut fields = meta.split(' ');
            let (mode, _kind, id) = (fields.next()?, fields.next()?, fields.next()?);
            Some(format!("{mode},{id},{path}"))
        });
        let call = self.git.at(&self.root).args(["update-index"]);
        let absent = entry.is_none();
        match entry {
            Some(info) => call.args(["--add", "--cacheinfo", &info]).run()?,
            None => call.args(["--force-remove", "--", path]).run()?,
        };
        Ok(absent)
    }

    /// Stages the working tree's state of `paths` (present files added, absent ones removed) so the index already
    /// holds the remote's entries, which the two-tree merge in `sync` then accepts without touching those files.
    fn stage_working_tree(&self, paths: &[String]) -> Result<(), Failure> {
        let (present, absent): (Vec<&String>, Vec<&String>) = paths
            .iter()
            .partition(|p| fs::symlink_metadata(self.root.join(p)).is_ok());
        for (flag, list) in [("--add", present), ("--force-remove", absent)] {
            if list.is_empty() {
                continue;
            }
            let input: Vec<u8> = list
                .iter()
                .flat_map(|p| p.as_bytes().iter().copied().chain([0]))
                .collect();
            self.git
                .at(&self.root)
                .args(["update-index", flag, "-z", "--stdin"])
                .stdin(input)
                .run()?;
        }
        Ok(())
    }

    /// Copies each of `paths` (local edits that lose to a remote change) into a fresh timestamped directory under
    /// `.git/records-set-aside/`, reports each on stderr, then restores those paths in the index and working tree to
    /// `old` so the two-tree merge can bring in the remote's version.
    fn set_aside(&self, paths: &[String], old: Option<&str>) -> Result<(), Failure> {
        let base = self.root.join(".git").join(SET_ASIDE_DIR);
        let stamp = utc_stamp(SystemTime::now());
        let mut dir = base.join(&stamp);
        let mut n = 1;
        while dir.exists() {
            n += 1;
            dir = base.join(format!("{stamp}-{n}"));
        }
        for path in paths {
            let source = self.root.join(path);
            match fs::symlink_metadata(&source) {
                Ok(_) => {
                    let saved = dir.join(path);
                    copy_recursively(&source, &saved)?;
                    eprintln!(
                        "records: set aside local edits to {path} (changed on the remote by another slot): {}",
                        saved.display()
                    );
                }
                // A local deletion has nothing to copy; the remote's version simply comes back.
                Err(_) => eprintln!(
                    "records: dropped the local deletion of {path} (changed on the remote by another slot)"
                ),
            }
        }
        // Back to the old tree: drop the paths from the index and the working tree, then check out whatever the old
        // tree had there.
        let nul_list = |list: &[String]| -> Vec<u8> {
            list.iter()
                .flat_map(|p| p.as_bytes().iter().copied().chain([0]))
                .collect()
        };
        self.git
            .at(&self.root)
            .args(["rm", "-r", "-q", "--cached", "--ignore-unmatch", "--"])
            .args(paths)
            .run()?;
        for path in paths {
            let full = self.root.join(path);
            let removed = match fs::symlink_metadata(&full) {
                Ok(meta) if meta.is_dir() => fs::remove_dir_all(&full),
                Ok(_) => fs::remove_file(&full),
                Err(_) => Ok(()),
            };
            removed
                .map_err(|e| Failure::failed(format!("cannot remove {}: {e}", full.display())))?;
        }
        if let Some(old) = old {
            let tracked = split_nul(
                &self
                    .git
                    .at(&self.root)
                    .args(["ls-tree", "-r", "-z", "--name-only", old, "--"])
                    .args(paths)
                    .run()?
                    .stdout,
            );
            if !tracked.is_empty() {
                self.git
                    .at(&self.root)
                    .args([
                        "checkout",
                        old,
                        "--pathspec-from-file=-",
                        "--pathspec-file-nul",
                    ])
                    .stdin(nul_list(&tracked))
                    .run()?;
            }
        }
        Ok(())
    }
}

/// How a push attempt ended, short of an outright failure.
enum Sent {
    Accepted,
    /// Rejected because the remote moved since the fetch; the reason is git's, for the final error message.
    Raced(String),
}

/// Creates an empty clone for `slot` atomically: it is set up under a temporary name and renamed into place, so a
/// half-created clone never sits at the slot's path, and two first calls racing each other end up sharing one clone.
fn create(
    git: &Git,
    clones_dir: &Path,
    root: &Path,
    slot: &str,
    remote: &str,
) -> Result<(), Failure> {
    fs::create_dir_all(clones_dir)
        .map_err(|e| Failure::failed(format!("cannot create {}: {e}", clones_dir.display())))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let temporary = clones_dir.join(format!(".{slot}.init-{}-{nanos}", std::process::id()));
    let result = (|| {
        git.at(clones_dir)
            .args(["init", "-q", "--initial-branch=main"])
            .args([&temporary])
            .run()?;
        git.at(&temporary)
            .args(["config", "remote.origin.url", remote])
            .run()?;
        git.at(&temporary)
            .args([
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ])
            .run()?;
        match fs::rename(&temporary, root) {
            Ok(()) => Ok(()),
            // Another call from this slot created the clone first; use that one.
            Err(_) if root.join(".git").is_dir() => {
                let _ = fs::remove_dir_all(&temporary);
                Ok(())
            }
            Err(e) => Err(Failure::failed(format!(
                "cannot move the new clone to {}: {e}",
                root.display()
            ))),
        }
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

/// Removes a file, treating "already gone" as success.
fn remove_if_present(path: &Path) -> Result<(), Failure> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Failure::failed(format!(
            "cannot remove {}: {e}",
            path.display()
        ))),
    }
}

fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// Whether `inner` is `outer` itself or lies inside it, by whole path components (`plans/ab` is not inside `plans/a`).
fn within(inner: &str, outer: &str) -> bool {
    inner == outer
        || inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `path` lies in one of the record directories `dirs`.
fn inside_any(path: &str, dirs: &[String]) -> bool {
    dirs.iter().any(|dir| within(path, dir))
}

/// Every (remote-changed path, local-edit path) pair that collides: the same path, or one inside the other (a remote
/// file where the agent has a directory, or the reverse), since updating either would clobber the other.
fn colliding_pairs(changed: &[String], local: &[String]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = changed
        .iter()
        .flat_map(|c| {
            local
                .iter()
                .filter(move |l| within(c, l) || within(l, c))
                .map(move |l| (c.clone(), l.clone()))
        })
        .collect();
    pairs.sort();
    pairs.dedup();
    pairs
}

/// Copies a file, symlink or directory tree to `to`, creating parent directories.
fn copy_recursively(from: &Path, to: &Path) -> Result<(), Failure> {
    let fail = |e: std::io::Error| {
        Failure::failed(format!(
            "cannot set aside {} as {}: {e}",
            from.display(),
            to.display()
        ))
    };
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let meta = fs::symlink_metadata(from).map_err(fail)?;
    if meta.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(from).map_err(fail)?, to).map_err(fail)?;
    } else if meta.is_dir() {
        fs::create_dir_all(to).map_err(fail)?;
        for entry in fs::read_dir(from).map_err(fail)? {
            let entry = entry.map_err(fail)?;
            copy_recursively(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        fs::copy(from, to).map_err(fail)?;
    }
    Ok(())
}

/// `YYYYMMDDTHHMMSSZ` in UTC, for set-aside directory names that sort by time.
fn utc_stamp(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's days_from_civil, inverted).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Whether a failed `git push --porcelain` was rejected only because the remote moved since the fetch, which a
/// rebuilt commit on the new head fixes. Returns git's reason when so.
///
/// Two forms count: git's own `[rejected]` (it saw from the ref advertisement that the push is not a fast-forward, as
/// `fetch first` or `non-fast-forward`), and a `[remote rejected]` whose reason says the server lost a race on the
/// ref (another push updated or locked it between advertisement and update). Every other rejection (permissions, a
/// server hook, a size limit) fails the push: retrying cannot fix it.
fn raced_rejection(porcelain: &str) -> Option<String> {
    porcelain.lines().find_map(|line| {
        let mut fields = line.split('\t');
        let (flag, refs, summary) = (fields.next()?, fields.next()?, fields.next()?);
        if flag != "!" || !refs.ends_with(&format!(":{MAIN}")) {
            return None;
        }
        let raced = if summary.starts_with("[rejected]") {
            true
        } else if summary.starts_with("[remote rejected]") {
            [
                "non-fast-forward",
                "fetch first",
                "failed to update ref",
                "failed to lock",
                "cannot lock ref",
                // Artifacts' answer when another push updated main between the advertisement and this update.
                "stale ref",
            ]
            .iter()
            .any(|reason| summary.contains(reason))
        } else {
            false
        };
        raced.then(|| summary.to_string())
    })
}

/// Whether a failed push was turned away because another push to the same repo was in progress, which Artifacts
/// reports as `remote: artifacts_git_receive_pack_busy` before hanging up (observed 2026-11-26 with six slots pushing
/// at once). Nothing was written, so it is retried like a lost race.
fn remote_busy(stderr: &str) -> Option<String> {
    stderr
        .contains("artifacts_git_receive_pack_busy")
        .then(|| "the remote was busy with another push".to_string())
}

/// How long to wait before retry number `attempt` (1-based): exponential from `FIRST_BACKOFF_MS`, capped at
/// `MAX_BACKOFF_MS`, plus up to half that again of jitter taken from the clock, so concurrent slots spread out.
fn backoff(attempt: usize) -> std::time::Duration {
    let exponent = u32::try_from(attempt.saturating_sub(1))
        .unwrap_or(u32::MAX)
        .min(16);
    let base = FIRST_BACKOFF_MS
        .saturating_mul(1u64 << exponent)
        .min(MAX_BACKOFF_MS);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or_default();
    let jitter = nanos % (base / 2 + 1);
    std::time::Duration::from_millis(base + jitter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// A remote change collides with a local edit on the same path or when one path contains the other, and never on a
    /// mere name prefix (`plans/a` and `plans/ab` are unrelated records). The pairs keep the local side, which is what
    /// gets set aside.
    #[test]
    fn collisions_are_by_path_or_containment() {
        let changed = strings(&[
            "farhelm/plans/a/log.md",
            "farhelm/plans/ab/log.md",
            "farhelm/landing/log.md",
            "x",
        ]);
        let local = strings(&["farhelm/plans/a/log.md", "farhelm/landing", "x/y"]);
        let pair = |r: &str, l: &str| (r.to_string(), l.to_string());
        assert_eq!(
            colliding_pairs(&changed, &local),
            vec![
                pair("farhelm/landing/log.md", "farhelm/landing"),
                pair("farhelm/plans/a/log.md", "farhelm/plans/a/log.md"),
                pair("x", "x/y"),
            ]
        );
        assert!(
            colliding_pairs(
                &strings(&["farhelm/plans/ab/log.md"]),
                &strings(&["farhelm/plans/a"])
            )
            .is_empty()
        );
        assert!(inside_any(
            "farhelm/plans/a/log.md",
            &strings(&["farhelm/plans/a"])
        ));
        assert!(!inside_any(
            "farhelm/plans/ab/log.md",
            &strings(&["farhelm/plans/a"])
        ));
    }

    /// Set-aside directories are named by UTC time so they sort chronologically.
    #[test]
    fn utc_stamps_are_civil_time() {
        let at = |s: u64| utc_stamp(UNIX_EPOCH + std::time::Duration::from_secs(s));
        assert_eq!(at(0), "19700101T000000Z");
        assert_eq!(at(951_782_400), "20000229T000000Z");
        assert_eq!(at(1_795_703_524), "20261126T143204Z");
    }

    /// Only "the remote moved" rejections are retried; anything else must fail the push instead of looping.
    #[test]
    fn only_raced_rejections_are_retried() {
        let line = |summary: &str| format!("To /r.git\n!\tabc:refs/heads/main\t{summary}\nDone\n");
        for summary in [
            "[rejected] (fetch first)",
            "[rejected] (non-fast-forward)",
            "[remote rejected] (failed to update ref)",
            "[remote rejected] (cannot lock ref 'refs/heads/main': is at x but expected y)",
            "[remote rejected] (stale ref)",
        ] {
            assert_eq!(
                raced_rejection(&line(summary)).as_deref(),
                Some(summary),
                "{summary}"
            );
        }
        for summary in [
            "[remote rejected] (pre-receive hook declined)",
            "[remote rejected] (permission denied)",
        ] {
            assert_eq!(raced_rejection(&line(summary)), None, "{summary}");
        }
        assert_eq!(
            raced_rejection("!\tabc:refs/heads/other\t[rejected] (fetch first)\n"),
            None
        );
        assert_eq!(raced_rejection("error: something else\n"), None);
    }

    /// Artifacts' busy answer is retried; an unrelated hang-up is not, since nothing says retrying would help.
    #[test]
    fn a_busy_remote_is_retried() {
        let busy = " remote: artifacts_git_receive_pack_busy\n\nfatal: the remote end hung up unexpectedly\n";
        assert!(remote_busy(busy).is_some());
        assert!(remote_busy("fatal: the remote end hung up unexpectedly\n").is_none());
    }

    /// The wait grows with each attempt and never exceeds the cap plus its jitter, so twelve attempts stay bounded.
    #[test]
    fn backoff_grows_and_is_capped() {
        let ms = |a| backoff(a).as_millis() as u64;
        assert!((FIRST_BACKOFF_MS..=FIRST_BACKOFF_MS * 3 / 2).contains(&ms(1)));
        assert!(ms(3) >= FIRST_BACKOFF_MS * 4);
        for attempt in 1..MAX_PUSH_ATTEMPTS + 5 {
            assert!(ms(attempt) <= MAX_BACKOFF_MS * 3 / 2, "attempt {attempt}");
        }
    }
}
