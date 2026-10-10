//! Applying conversation reports that hooks dropped as files.
//!
//! A hook writes its report into the session's drop directory and exits
//! (`crate::hook_report` owns the format and the layout). This module is the
//! supervisor's half: a recursive file watcher runs this drain promptly, and
//! every reconciliation pass (`Supervisor::capture_now`: the two-second ticker,
//! startup, reload and Restart) runs it as well. Each drain takes every waiting
//! report, runs it through admission (the per-kind attribution and the
//! five-step capture transaction, `Supervisor::report_conversation`), and
//! deletes it once the outcome is settled.
//!
//! # What a pass does
//!
//! 1. Nothing at all while the supervisor is not recording (replaced or
//!    shutting down): accepting a report is a conclusion it has no standing to
//!    draw, and the file stays for whoever records next.
//! 2. For each published session with a drop directory, put back any slot a
//!    previous supervisor took and died holding, remove temporary files no
//!    hook can still be writing, take every waiting slot, then apply them in
//!    [`Slot::DRAIN_ORDER`] (Grok's selection before its enrichment).
//! 3. Remove the drop directory of any session the store no longer has; a
//!    hook racing a delete can recreate one after the delete removed it.
//!
//! Directories of sessions that exist but are not published yet (a create or
//! relaunch still in its publication gap) are left alone: their reports wait
//! for the next pass after publication.
//!
//! Every settled report leaves a verdict line in the session's hook log
//! (`acked`, or `refused` with the reason), beside the line the hook wrote
//! when it dropped it, and refusals are also logged by the supervisor itself.
//!
//! # Settled versus retried
//!
//! A slot is taken by renaming it to a private name, so a hook replacing the
//! slot meanwhile writes a fresh file rather than having its newer report
//! deleted with the old one. The taken report is then either settled —
//! accepted, or refused for a reason a retry cannot change (wrong kind, wrong
//! launch, failed attribution, malformed) — and deleted, or it hit something
//! transient — every refusal of kind `Internal` (a store or tmux that could
//! not be read), and a session whose published entry a relaunch has already
//! moved past — and is put back for a later pass unless a newer report has
//! refilled the slot, in which case the newer one wins. A tmux server that is
//! gone altogether is not transient: it reads as the pane being gone, which
//! is definitive. A transient outcome also ends that session's pass, so Grok's
//! enrichment is never judged before its selection.
//!
//! # Tying a report to its launch
//!
//! Every report, whatever its vendor, must carry the hook's recorded process
//! chain, and the chain must reach the session's CURRENT pane process
//! (`procs::anchor_chain`): by pid and start token while that process is
//! alive, by pid alone once it has exited and tmux still lists the pane. Every
//! launch runs a new pane process, so a report made under an earlier launch,
//! or one whose pane is gone entirely (a reboot killed tmux), is discarded.
//! The per-kind attribution (Claude's position, Codex's, Grok's and OMP's
//! corridors) then reads the same anchored chain.

use super::core::{RequestError, SessionEntry, Supervisor};
use crate::hook_report::{self, HookReport, Slot};
use farhelm_proto::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// Byte cap on a report's `conversation`, checked before anything else reads
/// it.
///
/// This envelope must fit either locator-reporting vendor's encoded ID and
/// file locator. Ordinary Claude, Codex, and Goose IDs still have a separate
/// 128-byte limit; accepting an envelope of this size does not make an
/// equally large ID valid.
pub(crate) const MAX_CONVERSATION_BYTES: usize = crate::agent_kind::MAX_LOCATOR_BYTES;

/// Byte cap on a report's `source` — the vendor's own word for why the hook
/// fired (`startup`, `resume`, `clear`, `compact`, ...).
///
/// Unlike the conversation id, this field is never stored and never reaches
/// an argv; it only ever appears in log lines. That is exactly why it needs a
/// bound of its own. A `source` is not validated for shape — a vendor may add
/// an event name at any time, and refusing an unrecognized one would throw
/// away a perfectly good report over a diagnostic string — so the field is
/// whatever the report says, and the report is written by a process inside
/// the agent's tree. Without a cap, one such process turns the supervisor log
/// into an unbounded write target.
const MAX_SOURCE_BYTES: usize = 64;

/// Longest reason a report may give for where its ancestry collection
/// stopped, once it is quoted into a refusal.
const MAX_ENDED_BYTES: usize = 256;

/// How old a hook's temporary file must be before a pass removes it. A hook
/// writes and renames its report within milliseconds; anything this old
/// belongs to a hook that died between the two.
const STALE_TEMP_AGE: Duration = Duration::from_secs(10 * 60);

/// What a pass does with a report it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Disposition {
    /// Accepted or definitively refused: delete it.
    Settled,
    /// Blocked by something a later pass may not meet: put it back, and stop
    /// this session's pass.
    Retry,
}

impl Supervisor {
    /// Apply every waiting report of the published `entries`, and remove the
    /// drop directories of sessions that no longer exist. See the module docs
    /// for the order and the settle/retry rule.
    ///
    /// Drains never overlap: the watch task, ticker, reload and Restart call this,
    /// and only one at a time gets the drain lock. That is what lets a pass
    /// treat a taken file it finds at its start as a leftover of a
    /// supervisor that died mid-pass (one supervisor holds a state directory
    /// at a time), rather than another pass's work.
    ///
    /// When another drain is already running, an ordinary caller
    /// (`wait: false`) skips draining rather than waiting: the running drain
    /// is applying the same files, and a drain can take as long as admission
    /// does (exact-record reads, a busy capture claim); a periodic pass need
    /// not repeat work the existing drain is already doing. A caller that needs every report
    /// already on disk judged when it returns (`wait: true`: the watch task,
    /// which must not lose an event to a concurrent drain; Restart, which
    /// is about to choose the conversation to resume, and the test seam)
    /// waits its turn instead.
    pub(crate) async fn apply_report_files(&self, entries: &[Arc<SessionEntry>], wait: bool) {
        if !self.may_record() {
            return;
        }
        let _drain = if wait {
            self.report_drain.lock().await
        } else {
            match self.report_drain.try_lock() {
                Ok(guard) => guard,
                Err(_) => return,
            }
        };
        // A waiting watch drain may acquire its turn after ownership was withdrawn.
        // Admission must still leave the files for the next recording supervisor.
        if !self.may_record() {
            return;
        }
        let root = self.state_dir.join(hook_report::REPORTS_DIR);
        let names = match dir_entry_names(&root).await {
            Ok(names) => names,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                warn!(path = %root.display(), %error, "could not list waiting conversation reports");
                return;
            }
        };
        if let Some(gate) = self.seams.faults.report_drain_listed() {
            gate().await;
        }
        for name in names {
            let dir = root.join(&name);
            match entries.iter().find(|entry| entry.info.id == name) {
                Some(entry) => {
                    drain_session_dir(&dir, |report| self.admit_report_file(entry, report)).await;
                }
                None => self.remove_orphan_dir(&name, &dir).await,
            }
        }
    }

    /// Settle or retry one taken report of a published session.
    ///
    /// A store that cannot be read and a row a relaunch has already moved
    /// past the published entry are both retries: the first may pass next
    /// time, and the second resolves when the relaunched entry is published.
    /// Every refusal is logged with its reason, since the hook that wrote the
    /// report is long gone and cannot see one.
    async fn admit_report_file(
        &self,
        entry: &SessionEntry,
        report: Result<HookReport, hook_report::ReadError>,
    ) -> Disposition {
        let disposition = self.judge_report_file(entry, report).await;
        if disposition == Disposition::Settled {
            self.report_retry_warned
                .lock()
                .expect("retry-warning set poisoned")
                .remove(&entry.info.id);
        }
        disposition
    }

    /// Log that a session's waiting report is being kept for a later pass:
    /// at warn the first time for the session, at debug while the same cause
    /// keeps it waiting, so a long outage of the store or tmux does not log a
    /// line every two seconds.
    fn note_retry(&self, id: &str, reason: &str, what: &str) {
        let first = self
            .report_retry_warned
            .lock()
            .expect("retry-warning set poisoned")
            .insert(id.to_string());
        if first {
            warn!(session = %id, %reason, "{what}");
        } else {
            tracing::debug!(session = %id, %reason, "{what}");
        }
    }

    /// The decision behind [`Supervisor::admit_report_file`].
    async fn judge_report_file(
        &self,
        entry: &SessionEntry,
        report: Result<HookReport, hook_report::ReadError>,
    ) -> Disposition {
        let id = entry.info.id.as_str();
        let report = match report {
            Ok(report) => report,
            Err(hook_report::ReadError::Io(reason)) => {
                self.note_retry(
                    id,
                    &reason,
                    "could not read a conversation report yet; will retry",
                );
                return Disposition::Retry;
            }
            Err(hook_report::ReadError::Invalid(reason)) => {
                warn!(session = %id, %reason, "discarded a conversation report that could not be read");
                self.log_verdict(id, "refused", &format!("unreadable {reason}"), None);
                return Disposition::Settled;
            }
        };
        let row = match self.store.session(id).await {
            Ok(Some(row)) => row,
            Ok(None) => return Disposition::Settled,
            Err(error) => {
                self.note_retry(
                    id,
                    &format!("{error:#}"),
                    "could not read the session a conversation report is for; will retry",
                );
                return Disposition::Retry;
            }
        };
        if row.generation != entry.generation {
            return Disposition::Retry;
        }
        let about = (report.conversation.clone(), report.source.clone());
        let about = Some((about.0.as_str(), about.1.as_str()));
        match self.admit_hook_report(id, &row, report).await {
            Ok(()) => {
                self.log_verdict(id, "acked", "", about);
                Disposition::Settled
            }
            Err(error)
                if error.kind == ErrorKind::Internal || self.relaunched_since(id, &row).await =>
            {
                self.note_retry(
                    id,
                    &error.message,
                    "could not apply a conversation report yet; will retry",
                );
                Disposition::Retry
            }
            Err(error) => {
                warn!(session = %id, reason = %error.message, "refused a conversation report");
                let detail = format!("{} {}", error_kind_word(error.kind), error.message);
                self.log_verdict(id, "refused", &detail, about);
                Disposition::Settled
            }
        }
    }

    /// Whether the session has a different launch generation now than `row`,
    /// the one a report was checked against. A refusal that raced a relaunch
    /// says nothing about the report itself: it was judged against a pane and
    /// a generation that are no longer current, so it is put back to be judged
    /// against the launch that replaced them. A store that cannot be read
    /// counts as moved on too, for the same retry.
    async fn relaunched_since(&self, id: &str, row: &crate::store::StoredSession) -> bool {
        !matches!(
            self.store.session(id).await,
            Ok(Some(current)) if current.generation == row.generation
        )
    }

    /// Append the verdict on one settled report to the session's hook log,
    /// next to the hook's own line for the run that wrote it: the one place
    /// a person looking at a session can see whether its report landed and,
    /// if not, why. `acked` for an accepted report; `refused` with the error
    /// kind and reason otherwise. Best effort, like every hook-log write.
    fn log_verdict(&self, id: &str, word: &str, detail: &str, about: Option<(&str, &str)>) {
        let line = hook_report::render_log_line(
            crate::store::now_unix().max(0) as u64,
            word,
            detail,
            about,
        );
        hook_report::append_hook_log(&super::core::hook_log_path(&self.state_dir, id), &line);
    }

    /// Validate one dropped report, tie it to the session's current launch,
    /// and admit it: [`check_hook_report`], then the anchor at the current
    /// pane (see the module docs), then admission's own five steps on the
    /// anchored chain.
    pub(crate) async fn admit_hook_report(
        &self,
        id: &str,
        row: &crate::store::StoredSession,
        report: HookReport,
    ) -> Result<(), RequestError> {
        let source = check_hook_report(row, &report)?;
        let chain = report
            .chain()
            .map_err(|reason| RequestError::new(ErrorKind::InvalidRequest, reason))?
            .ok_or_else(|| {
                RequestError::new(
                    ErrorKind::Conflict,
                    "the report carries no process ancestry, so it cannot be tied to this \
                     session's current launch",
                )
            })?;
        let ended = report.ancestry_ended.as_deref().map(sanitized_ended);
        let pane = self.owned_pane_anchor(row).await?;
        let anchored = crate::procs::anchor_chain(&chain, ended.as_deref(), pane)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        self.report_conversation(
            id,
            super::core::ReportedConversation {
                vendor: report.vendor,
                conversation: report.conversation,
                source,
                transcript_path: report.transcript_path,
                hook_event_name: report.hook_event_name,
                ancestry: Some(anchored.to_vec()),
                launch_generation: Some(row.generation),
            },
        )
        .await
    }

    /// Remove a drop directory whose session the store no longer has.
    ///
    /// Delete removes the directory itself; this catches one a hook recreated
    /// afterwards, and names that are not session directories at all are
    /// left alone. A session with a row but no published entry is in its
    /// publication gap and keeps its directory, as does any session whose
    /// row could not be read.
    async fn remove_orphan_dir(&self, name: &str, dir: &Path) {
        if hook_report::session_dir(Path::new(""), name).is_none() {
            return;
        }
        if !matches!(self.store.session(name).await, Ok(None)) {
            return;
        }
        if let Err(error) = tokio::fs::remove_dir_all(dir).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!(session = %name, %error, "could not remove a deleted session's waiting conversation reports");
        }
    }
}

/// The checks a report must pass before anything reads its evidence,
/// returning its `source` made safe for the log.
///
/// In this order, and for these reasons: the vendor must name this session's
/// durable kind before
/// anything vendor-specific is read; the conversation is bounded before it
/// reaches a log line or the store; a typed sub-agent marker refuses on every
/// kind, and one of an unexpected type refuses rather than being read as
/// absent; each vendor's source vocabulary is checked against the raw value
/// before it is sanitized for the log.
pub(crate) fn check_hook_report(
    row: &crate::store::StoredSession,
    report: &HookReport,
) -> Result<String, RequestError> {
    if crate::agent_kind::agent_kind_of_vendor(report.vendor) != row.agent_kind() {
        return Err(RequestError::new(
            ErrorKind::InvalidRequest,
            "the reported conversation identity does not match this session's agent kind",
        ));
    }
    if report.conversation.len() > MAX_CONVERSATION_BYTES {
        return Err(RequestError::new(
            ErrorKind::InvalidRequest,
            format!(
                "the reported conversation identity is {} bytes, more than this build stores",
                report.conversation.len()
            ),
        ));
    }
    match report.agent_id.as_ref() {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::String(marker)) if marker.is_empty() => {}
        Some(serde_json::Value::String(_)) => {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "a delegated agent may not report its session's conversation",
            ));
        }
        Some(_) => {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "the reported agent identity has an unexpected shape",
            ));
        }
    }
    if let Some(refusal) = crate::agent_kind::foreground_source_refusal(
        report.vendor,
        &report.source,
        report
            .hook_event_name
            .as_ref()
            .and_then(serde_json::Value::as_str),
    ) {
        return Err(RequestError::new(ErrorKind::InvalidRequest, refusal));
    }
    Ok(sanitized_source(&report.source))
}

/// Drain one session's drop directory: restore leftovers, sweep stale
/// temporary files, take every waiting slot, then hand the taken reports to
/// `apply` in [`Slot::DRAIN_ORDER`], deleting each one `apply` settles. When
/// `apply` asks for a retry, that report and every one not yet applied go
/// back into their slots (unless a newer report has refilled a slot) and the
/// pass for this directory ends.
///
/// All slots are taken before any is applied, and in [`Slot::TAKE_ORDER`],
/// Grok's enrichment before its selection. That pairs them correctly with a
/// hook writing at the same time: Grok writes a selection before the
/// enrichments that follow it, so whatever enrichment this pass takes, the
/// selection it depends on (or a newer one) is already in its slot when the
/// pass takes that next. Taking the selection first could miss one written a
/// moment later and then judge its enrichment against the old selection,
/// refusing and losing it.
///
/// An empty listing skips all slot takes; a newly published report waits for
/// the next pass. Recovered taken files count as waiting reports even when the
/// original listing contained no public slot. Temporary-file cleanup still runs.
///
/// Generic over `apply` so the file handling can be tested without a
/// supervisor, a store, or a pane.
pub(crate) async fn drain_session_dir<F, Fut>(dir: &Path, mut apply: F)
where
    F: FnMut(Result<HookReport, hook_report::ReadError>) -> Fut,
    Fut: std::future::Future<Output = Disposition>,
{
    let names = match dir_entry_names(dir).await {
        Ok(names) => names,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                warn!(path = %dir.display(), %error, "could not list a session's waiting conversation reports");
            }
            return;
        }
    };
    let mut has_report = names.iter().any(|name| {
        Slot::TAKE_ORDER
            .into_iter()
            .any(|slot| *name == slot.file_name())
    });
    for name in &names {
        let path = dir.join(name);
        if let Some(rest) = name.strip_prefix(hook_report::TAKEN_PREFIX) {
            let slot = Slot::DRAIN_ORDER
                .into_iter()
                .find(|slot| rest.starts_with(&format!("{}-", slot.name())));
            match slot {
                Some(slot) => {
                    has_report = true;
                    put_back(&path, &dir.join(slot.file_name())).await;
                }
                None => {
                    let _ = tokio::fs::remove_file(&path).await;
                }
            }
        } else if name.starts_with(hook_report::TEMP_PREFIX) && is_stale(&path).await {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
    // A hook that publishes after an empty listing waits for the next pass.
    // Once any slot was observed or recovered, take ALL slots in the existing
    // order: selecting only listed slots can lose a concurrently written pair.
    if !has_report {
        return;
    }
    let mut taken = Vec::new();
    for slot in Slot::TAKE_ORDER {
        let slot_path = dir.join(slot.file_name());
        let private = dir.join(format!(
            "{}{}-{}",
            hook_report::TAKEN_PREFIX,
            slot.name(),
            uuid::Uuid::new_v4()
        ));
        match tokio::fs::rename(&slot_path, &private).await {
            Ok(()) => taken.push((slot, private)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                // Applying only part of the set would judge a Grok
                // enrichment without the selection that could not be
                // taken, so everything taken goes back for a later pass.
                warn!(path = %slot_path.display(), %error, "could not take a waiting conversation report");
                for (slot, private) in taken {
                    put_back(&private, &dir.join(slot.file_name())).await;
                }
                return;
            }
        }
    }
    taken.sort_by_key(|(slot, _)| Slot::DRAIN_ORDER.iter().position(|order| order == slot));
    let mut pending = taken.into_iter();
    while let Some((slot, private)) = pending.next() {
        let read_from = private.clone();
        let report = tokio::task::spawn_blocking(move || hook_report::read_report(&read_from))
            .await
            .unwrap_or_else(|_| {
                // A read that panicked would panic again on the same file.
                Err(hook_report::ReadError::Invalid(
                    "reading it panicked".to_string(),
                ))
            });
        match apply(report).await {
            Disposition::Settled => {
                if let Err(error) = tokio::fs::remove_file(&private).await {
                    warn!(path = %private.display(), %error, "could not remove a settled conversation report");
                }
            }
            Disposition::Retry => {
                put_back(&private, &dir.join(slot.file_name())).await;
                for (slot, private) in pending {
                    put_back(&private, &dir.join(slot.file_name())).await;
                }
                return;
            }
        }
    }
}

/// The wire spelling of an [`ErrorKind`], for the hook log: routed through
/// serde so the log says exactly what the protocol calls it, and a new kind
/// cannot silently become a stale word here.
fn error_kind_word(kind: ErrorKind) -> String {
    match serde_json::to_value(kind) {
        Ok(serde_json::Value::String(word)) => word,
        _ => "unknown".to_string(),
    }
}

/// Return a taken report to its slot, unless a newer report has filled the
/// slot since, in which case the newer one wins and the taken one goes.
/// A hard link is the portable "rename unless the target exists".
async fn put_back(taken: &Path, slot: &Path) {
    match tokio::fs::hard_link(taken, slot).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            warn!(path = %slot.display(), %error, "could not put a conversation report back for a later pass");
            return;
        }
    }
    let _ = tokio::fs::remove_file(taken).await;
}

/// Whether a temporary file is old enough that no hook is still writing it.
async fn is_stale(path: &Path) -> bool {
    match tokio::fs::metadata(path)
        .await
        .and_then(|meta| meta.modified())
    {
        Ok(modified) => modified.elapsed().is_ok_and(|age| age >= STALE_TEMP_AGE),
        Err(_) => false,
    }
}

/// The UTF-8 names in a directory; anything else in it is not ours.
async fn dir_entry_names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// `source` reduced to something safe to put in a log line: at most
/// [`MAX_SOURCE_BYTES`] bytes, with every control character replaced.
///
/// Sanitizing rather than refusing, deliberately. The report itself is the
/// valuable thing and the `source` is a diagnostic beside it; rejecting a
/// report because its event name was odd would trade a correct resume for a
/// tidy log. So an over-long or control-laced value is trimmed and passed
/// on, and the report is judged on its identity alone.
///
/// Control characters are what make this more than a length cap. The log is
/// line-oriented and read by humans and by whatever tails it; a newline in
/// this field lets a process in the agent's tree forge log ENTRIES, and a
/// terminal escape lets it repaint the operator's screen. Replacement keeps
/// the value legible while making both impossible.
///
/// Truncation is on a CHARACTER boundary rather than a byte one — slicing a
/// `String` mid-UTF-8 would panic, and this input is untrusted.
fn sanitized_source(source: &str) -> String {
    bounded_printable(source, MAX_SOURCE_BYTES)
}

/// The report's account of where its ancestry collection stopped, made safe
/// to quote in a refusal the same way as `source`.
fn sanitized_ended(ended: &str) -> String {
    bounded_printable(ended, MAX_ENDED_BYTES)
}

/// At most `max_bytes` of `text`, cut on a character boundary, with every
/// control character replaced.
fn bounded_printable(text: &str, max_bytes: usize) -> String {
    text.chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .scan(0usize, |used, c| {
            *used += c.len_utf8();
            (*used <= max_bytes).then_some(c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::ReportVendor;
    use std::sync::Mutex;

    /// The `source` bound and control-character replacement, the hygiene
    /// that keeps a report's diagnostic field from becoming a log-forging or
    /// unbounded write target.
    ///
    /// The field is unvalidated by design — a vendor may add an event name
    /// at any time, and refusing an unrecognized one would throw away a
    /// good report over a diagnostic string — so whatever a process in the
    /// agent's tree writes ends up in the supervisor's log. Unbounded, that
    /// is an unbounded write target; with control characters intact, a
    /// newline forges log ENTRIES and an escape sequence repaints the
    /// operator's terminal.
    ///
    /// The multibyte case is not decoration: truncation has to land on a
    /// character boundary, because slicing a `String` mid-UTF-8 panics and
    /// this input is untrusted.
    #[test]
    fn a_reported_source_is_bounded_and_stripped_of_control_characters() {
        assert_eq!(sanitized_source("startup"), "startup");
        assert_eq!(
            sanitized_source("start\nup\u{1b}[2J"),
            "start\u{fffd}up\u{fffd}[2J",
            "newlines and escapes must not survive into a line-oriented log"
        );
        assert!(
            sanitized_source(&"a".repeat(4096)).len() <= MAX_SOURCE_BYTES,
            "an unbounded source must not become an unbounded log line"
        );
        // Four-byte characters, so a byte-indexed cap would land inside one.
        let wide = sanitized_source(&"🙂".repeat(64));
        assert!(wide.len() <= MAX_SOURCE_BYTES);
        assert_eq!(
            wide.chars().count(),
            MAX_SOURCE_BYTES / 4,
            "truncation lands on a character boundary rather than splitting one"
        );
    }

    fn report(vendor: ReportVendor, event: Option<&str>, conversation: &str) -> HookReport {
        HookReport {
            version: hook_report::FORMAT_VERSION,
            vendor,
            conversation: conversation.to_string(),
            source: "startup".to_string(),
            transcript_path: None,
            hook_event_name: event.map(|e| serde_json::Value::String(e.to_string())),
            agent_id: None,
            ancestry: None,
            ancestry_ended: None,
        }
    }

    /// Write `report` the way a hook does and return the session's drop
    /// directory.
    fn drop_report(state: &Path, report: &HookReport) -> std::path::PathBuf {
        hook_report::write_report(state, "session-1", report).expect("write the report");
        hook_report::session_dir(state, "session-1").expect("valid id")
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("drop dir")
            .map(|entry| entry.expect("entry").file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// Spec: a pass hands Codex and Grok selection to admission before their
    /// enrichment, and deletes each report once admission settles it.
    ///
    /// Why: an enrichment is refused unless its selection was applied, so
    /// applying them in the other order after a supervisor outage would
    /// discard the enrichment and keep a conversation without its record.
    #[farhelm_testtrace::test]
    async fn a_pass_applies_selection_before_enrichment_for_both_integrations() {
        for vendor in [ReportVendor::Codex, ReportVendor::Grok] {
            let state = farhelm_teststate::tempdir().expect("state dir");
            drop_report(state.path(), &report(vendor, Some("Stop"), "enrich"));
            let dir = drop_report(
                state.path(),
                &report(vendor, Some("SessionStart"), "select"),
            );
            let seen = Mutex::new(Vec::new());
            drain_session_dir(&dir, |report| {
                seen.lock()
                    .unwrap()
                    .push(report.expect("readable").conversation);
                async { Disposition::Settled }
            })
            .await;
            assert_eq!(*seen.lock().unwrap(), ["select", "enrich"]);
            assert!(names(&dir).is_empty(), "settled reports are deleted");
        }
    }

    /// Spec: a pass takes every waiting slot before it applies any, so a
    /// slot is already empty when an earlier report of the same pass is
    /// being applied.
    ///
    /// Why: a hook may write a selection and then its enrichment while a
    /// pass is running. Taking all slots first, enrichment before selection,
    /// means any enrichment the pass holds was preceded by a selection the
    /// pass also holds (or a newer one), so the enrichment is never judged
    /// against a stale selection and lost.
    #[farhelm_testtrace::test]
    async fn a_pass_takes_every_slot_before_applying_any() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        drop_report(
            state.path(),
            &report(ReportVendor::Grok, Some("Stop"), "enrich"),
        );
        let dir = drop_report(
            state.path(),
            &report(ReportVendor::Grok, Some("SessionStart"), "select"),
        );
        let seen = Mutex::new(Vec::new());
        drain_session_dir(&dir, |report| {
            let in_place = names(&dir)
                .into_iter()
                .filter(|name| !name.starts_with('.'))
                .collect::<Vec<_>>();
            seen.lock()
                .unwrap()
                .push((report.expect("readable").conversation, in_place));
            async { Disposition::Settled }
        })
        .await;
        let seen = seen.into_inner().unwrap();
        assert_eq!(seen[0].0, "select");
        assert!(
            seen[0].1.is_empty(),
            "the enrichment was already taken while the selection was applied: {:?}",
            seen[0].1
        );
    }

    /// Spec: a report file that fails to read (as opposed to failing to be
    /// a report) is handed over as an I/O error, so admission keeps it for a
    /// later pass instead of discarding it.
    ///
    /// Why: a descriptor shortage or a storage hiccup says nothing about the
    /// report, which may be the only record of a conversation switch made
    /// while the supervisor was down.
    #[farhelm_testtrace::test]
    async fn an_unreadable_file_is_kept_but_an_invalid_one_is_not() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let dir = drop_report(state.path(), &report(ReportVendor::Pi, None, "keep"));
        // A report the supervisor may not open fails the way an I/O error
        // does, and still hard-links back into its slot like any file.
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            dir.join("latest.json"),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
        assert!(
            std::fs::read(dir.join("latest.json")).is_err(),
            "fixture premise: the report cannot be read (not running as root)"
        );
        let seen = Mutex::new(Vec::new());
        drain_session_dir(&dir, |report| {
            let retry = matches!(report, Err(hook_report::ReadError::Io(_)));
            seen.lock().unwrap().push(retry);
            async move {
                if retry {
                    Disposition::Retry
                } else {
                    Disposition::Settled
                }
            }
        })
        .await;
        assert_eq!(*seen.lock().unwrap(), [true], "an I/O failure reads as Io");
        assert!(dir.join("latest.json").exists(), "and the slot is kept");

        std::fs::remove_file(dir.join("latest.json")).unwrap();
        std::fs::write(dir.join("latest.json"), b"not a report").unwrap();
        let seen = Mutex::new(Vec::new());
        drain_session_dir(&dir, |report| {
            let invalid = matches!(report, Err(hook_report::ReadError::Invalid(_)));
            seen.lock().unwrap().push(invalid);
            async { Disposition::Settled }
        })
        .await;
        assert_eq!(
            *seen.lock().unwrap(),
            [true],
            "a damaged file reads as Invalid"
        );
        assert!(!dir.join("latest.json").exists(), "and is settled away");
    }

    /// Spec: an ordinary pass skips draining while another drain holds the
    /// lock, and a waiting pass (what Restart runs) waits for it instead.
    ///
    /// Why: Restart is about to choose the conversation to resume, and a
    /// report already on disk may name it; if Restart skipped a drain that
    /// was running, it could resume the conversation the agent left, and the
    /// report would then be discarded as belonging to the old launch. A
    /// session listing must not wait behind a slow admission, though.
    #[farhelm_testtrace::test]
    async fn restart_waits_for_a_running_drain_and_a_listing_does_not() {
        let state = super::super::core::tests::StateDir::new();
        let sup = Arc::new(Supervisor::new(state.path()).await.expect("supervisor"));
        let held = sup.report_drain.lock().await;
        tokio::time::timeout(Duration::from_secs(5), sup.capture_now())
            .await
            .expect("an ordinary pass skips a running drain");
        let waiting = tokio::spawn({
            let sup = Arc::clone(&sup);
            async move { sup.capture_pass(true).await }
        });
        // A pass that skipped the held drain would finish well inside this
        // window, as `capture_now` did above.
        // sleep-ok: observation window for a pass that must still be waiting.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!waiting.is_finished(), "a waiting pass waits for the drain");
        drop(held);
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("the waiting pass finishes once the drain is free")
            .expect("no panic");
    }

    /// Spec: a report admission could not settle goes back into its slot and
    /// ends the session's pass, so the slots after it wait too; a newer
    /// report that refilled the slot meanwhile wins over the one put back.
    ///
    /// Why: a transient failure (a store or tmux that could not be read)
    /// must not lose the report, and must not let Grok's enrichment be
    /// judged without its selection. And the latest report is the one that
    /// describes the agent now.
    #[farhelm_testtrace::test]
    async fn a_retried_report_returns_to_its_slot_unless_a_newer_one_arrived() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        drop_report(
            state.path(),
            &report(ReportVendor::Grok, Some("Stop"), "enrich"),
        );
        let dir = drop_report(
            state.path(),
            &report(ReportVendor::Grok, Some("SessionStart"), "select"),
        );
        let calls = Mutex::new(0);
        drain_session_dir(&dir, |_| {
            *calls.lock().unwrap() += 1;
            async { Disposition::Retry }
        })
        .await;
        assert_eq!(*calls.lock().unwrap(), 1, "the retry ends the pass");
        assert_eq!(names(&dir), ["enrichment.json", "selection.json"]);

        let state = farhelm_teststate::tempdir().expect("state dir");
        let dir = drop_report(state.path(), &report(ReportVendor::Claude, None, "old"));
        let path = state.path().to_path_buf();
        drain_session_dir(&dir, |_| {
            drop_report(&path, &report(ReportVendor::Claude, None, "new"));
            async { Disposition::Retry }
        })
        .await;
        assert_eq!(names(&dir), ["latest.json"]);
        let kept = hook_report::read_report(&dir.join("latest.json")).expect("readable");
        assert_eq!(kept.conversation, "new", "the newer report wins");
    }

    /// Spec: a slot left taken by a supervisor that died mid-pass is put
    /// back and applied, an unreadable report is still handed over (and
    /// settled), and a fresh temporary file is left for the hook writing it.
    ///
    /// Why: a crash between taking and settling must not lose the report it
    /// held, and a pass must never pull a report out from under a hook that
    /// is still writing it.
    #[farhelm_testtrace::test]
    async fn leftovers_are_recovered_and_fresh_temporaries_left_alone() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let dir = drop_report(state.path(), &report(ReportVendor::Pi, None, "recovered"));
        std::fs::rename(dir.join("latest.json"), dir.join(".taken-latest-crashed")).unwrap();
        std::fs::write(dir.join(".tmp-latest-1-2"), b"in progress").unwrap();
        let seen = Mutex::new(Vec::new());
        drain_session_dir(&dir, |report| {
            seen.lock().unwrap().push(report.map(|r| r.conversation));
            async { Disposition::Settled }
        })
        .await;
        assert_eq!(*seen.lock().unwrap(), [Ok("recovered".to_string())]);
        assert_eq!(names(&dir), [".tmp-latest-1-2"]);

        std::fs::write(dir.join("latest.json"), b"not a report").unwrap();
        let seen = Mutex::new(Vec::new());
        drain_session_dir(&dir, |report| {
            seen.lock().unwrap().push(report.is_err());
            async { Disposition::Settled }
        })
        .await;
        assert_eq!(*seen.lock().unwrap(), [true]);
        assert_eq!(names(&dir), [".tmp-latest-1-2"]);
    }
}
