//! Conversation reports handed from an agent's hook to the supervisor as
//! files, and the on-disk format both sides share.
//!
//! A per-launch hook (`farhelm internal hook`, the Goose helper, the Pi and
//! OMP extension assets) learns which conversation its agent is in and has
//! to tell the supervisor. It does that by writing a report file into a
//! per-session drop directory under the supervisor's state directory and
//! exiting; the supervisor applies waiting files on watch events, with its
//! periodic reconciliation pass as the backstop (`service::report_files`).
//! Nothing here talks to a socket, so a report made while the supervisor is not running (on the
//! Mac, whenever the desktop app is closed) waits on disk instead of being
//! lost, and a hook never stalls its agent waiting for a reply.
//!
//! # Layout
//!
//! ```text
//! <state_dir>/hook-reports/<session-id>/latest.json      single-report integrations
//! <state_dir>/hook-reports/<session-id>/selection.json   Codex / Grok SessionStart
//! <state_dir>/hook-reports/<session-id>/enrichment.json  Codex Stop; Grok UserPromptSubmit / Stop
//! ```
//!
//! Each slot holds only the LATEST report of its kind, replaced by an atomic
//! rename. The single-report integrations carry independently usable identities.
//! Codex and Grok instead select a conversation before a later event confirms
//! its persisted record. An enrichment is refused unless a selection for that
//! conversation came first. Their two slots
//! are drained selection first, so a long supervisor outage can never evict
//! the selection in favour of later enrichments. The directory is therefore
//! bounded at two report files, with no queue, cap, or eviction rule; a
//! queue of files would need one, and a capped queue dropping its oldest
//! entries would drop exactly that selection.
//!
//! Names starting with `.` are private to one side. The hook writes under
//! [`TEMP_PREFIX`] and renames into the slot, so a reader never sees a
//! half-written report. The supervisor takes a slot by renaming it to a name
//! under [`TAKEN_PREFIX`] before applying it, so a hook replacing the slot
//! meanwhile does not have its newer report deleted with the old one.
//!
//! # Trust
//!
//! The file sits in the supervisor's private state directory, which only the
//! same Unix user can write, and that user already owns the agent, its
//! credential, and its vendor files. So the file is not a security boundary,
//! and attribution never claimed to be one. What it must still do is let the
//! supervisor stop honest mistakes — a native sub-agent, a nested agent, or a
//! shelled-out child replacing the session's conversation — so it carries
//! the hook's own process ancestry ([`RecordedLink`]), recorded when the
//! report was made, for the supervisor's per-kind attribution to read. The
//! supervisor still treats every field as untrusted input: it bounds the read
//! and re-applies the attribution budgets before using the evidence.

use crate::procs;
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The directory under the state directory that holds one drop directory
/// per session.
pub const REPORTS_DIR: &str = "hook-reports";

/// The version of the report file format this build writes and reads. A
/// file carrying another version is refused, not guessed at: within one
/// installation the hook and the supervisor are the same binary, and the
/// upgrade gap where they differ is accepted as a briefly missed report.
pub const FORMAT_VERSION: u32 = 1;

/// Prefix of a report the hook is still writing. The supervisor ignores
/// these, and removes ones old enough that no hook can still be writing them.
pub const TEMP_PREFIX: &str = ".tmp-";

/// Prefix of a slot the supervisor has taken for processing. One left over
/// at the start of a pass belongs to a supervisor that died mid-pass, and is
/// put back.
pub const TAKEN_PREFIX: &str = ".taken-";

/// Largest report file the supervisor reads. A report is a few hundred
/// bytes plus its ancestry, whose command lines the hook bounds at 1 MiB in
/// total; base64 grows that by a third. The hook drops ancestry links from
/// the top to stay under this (see `serialize_within_cap`), so anything
/// larger was not written by this build's hook and is refused unread.
pub const MAX_REPORT_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Longest recorded working directory the supervisor keeps; longer is not a
/// real working directory, and the link is read as having none.
const MAX_CWD_BYTES: usize = 4096;

/// Longest session id this module will turn into a directory name.
const MAX_SESSION_ID_BYTES: usize = 128;

/// One report slot in a session's drop directory. See the module docs for
/// why there are exactly these three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The latest independently usable report.
    Latest,
    /// Codex or Grok's latest `SessionStart`, selecting a conversation.
    Selection,
    /// Codex's latest `Stop`, or Grok's `UserPromptSubmit` / `Stop`, enriching the
    /// selected conversation.
    Enrichment,
}

impl Slot {
    /// The order the supervisor applies slots in. Selection comes
    /// before its enrichment, because an enrichment is only accepted for a
    /// conversation already selected.
    pub const DRAIN_ORDER: [Slot; 3] = [Slot::Selection, Slot::Enrichment, Slot::Latest];

    /// The order the supervisor takes slots in before applying any:
    /// enrichment before its selection, so a selection written just before
    /// the enrichment a pass takes is taken by the same pass (see
    /// `service::report_files::drain_session_dir`).
    pub const TAKE_ORDER: [Slot; 3] = [Slot::Latest, Slot::Enrichment, Slot::Selection];

    /// The slot's file name, and the word naming it in private file names.
    pub fn name(self) -> &'static str {
        match self {
            Slot::Latest => "latest",
            Slot::Selection => "selection",
            Slot::Enrichment => "enrichment",
        }
    }

    /// The slot's file name in the drop directory.
    pub fn file_name(self) -> String {
        format!("{}.json", self.name())
    }
}

/// One conversation report as the hook wrote it.
///
/// After `version`: `vendor` is the entry point the report came through
/// (never inferred from the payload), `conversation` the vendor's id or
/// encoded locator, `source` the vendor's reason for the hook, and the three
/// optional JSON values raw vendor evidence that admission validates rather
/// than the hook. `ancestry` is the hook's own process chain, recorded when
/// it made the report, from the hook upwards (what attribution reads), or `None`
/// when the hook could not read it (the supervisor then refuses the report,
/// whatever its vendor: without it, the report cannot be tied to the
/// session's current launch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookReport {
    pub version: u32,
    pub vendor: farhelm_proto::ReportVendor,
    pub conversation: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_event_name: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<serde_json::Value>,
    #[serde(default)]
    pub ancestry: Option<Vec<RecordedLink>>,
    /// Why ancestry collection stopped before running out of parents (an
    /// ancestor that could not be read, one that changed mid-walk), or
    /// `None` when it reached the top. Context for a refusal when the
    /// session's pane is not in the chain; never a reason by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ancestry_ended: Option<String>,
}

impl HookReport {
    /// Ask the integration which durable slot this event replaces. Selection
    /// and enrichment must survive independently while the supervisor is down.
    pub fn slot(&self) -> Slot {
        crate::agent_kind::conversation_report_slot(
            self.vendor,
            self.hook_event_name.as_ref().and_then(|v| v.as_str()),
        )
    }

    /// Decode the recorded ancestry into attribution evidence.
    ///
    /// The file is input to the supervisor, so its evidence is bounded again
    /// rather than trusted to be what this build's hook wrote: at most
    /// [`procs::MAX_ATTRIBUTION_ANCESTORS`] links here, and anchoring at the
    /// session's pane (`procs::anchor_chain`) refuses a chain with a link
    /// over the per-process command-line budget or a slice over the walk
    /// budget. Collection never records such a link, so one can only come
    /// from a file this build did not write. `Ok(None)` is a report written
    /// without ancestry.
    pub(crate) fn chain(&self) -> Result<Option<Vec<procs::ChainLink>>, String> {
        let Some(links) = &self.ancestry else {
            return Ok(None);
        };
        if links.len() > procs::MAX_ATTRIBUTION_ANCESTORS {
            return Err("the report's process ancestry is longer than attribution allows".into());
        }
        links
            .iter()
            .map(RecordedLink::decode)
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}

/// One link of a hook's process ancestry, as recorded in a report file.
///
/// The byte fields are base64 because executable paths, command lines, and
/// working directories are arbitrary bytes, not UTF-8, and attribution
/// matches them byte for byte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedLink {
    pub pid: u32,
    pub ppid: u32,
    /// The kernel start token: opaque, comparable only with another token
    /// read on the same boot (see `procs`).
    pub start: u64,
    pub exe: String,
    pub argv: Option<Vec<String>>,
    pub cwd: Option<String>,
}

impl From<&procs::ChainLink> for RecordedLink {
    fn from(link: &procs::ChainLink) -> Self {
        RecordedLink::encode(link)
    }
}

impl RecordedLink {
    fn encode(link: &procs::ChainLink) -> Self {
        use base64::Engine as _;
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        RecordedLink {
            pid: link.pid,
            ppid: link.ppid,
            start: link.start,
            exe: b64(&link.exe),
            argv: link
                .argv
                .as_ref()
                .map(|argv| argv.iter().map(|arg| b64(arg)).collect()),
            cwd: link.cwd.as_deref().map(b64),
        }
    }

    fn decode(&self) -> Result<procs::ChainLink, String> {
        use base64::Engine as _;
        let unb64 = |text: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .map_err(|_| "the report's process ancestry is not valid base64".to_string())
        };
        let argv = match &self.argv {
            None => None,
            Some(argv) => Some(
                argv.iter()
                    .map(|arg| unb64(arg))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Ok(procs::ChainLink {
            pid: self.pid,
            ppid: self.ppid,
            start: self.start,
            exe: unb64(&self.exe)?,
            argv,
            cwd: self
                .cwd
                .as_deref()
                .map(unb64)
                .transpose()?
                .filter(|cwd| cwd.len() <= MAX_CWD_BYTES),
        })
    }
}

/// A hook's own recorded ancestry, ready to go into a [`HookReport`]'s
/// `ancestry` and `ancestry_ended`.
pub struct RecordedAncestry {
    pub links: Vec<RecordedLink>,
    pub ended: Option<String>,
}

/// Record the calling process's own ancestry for a report: the hook, its
/// parents, and so on up to wherever collection stops (see
/// `procs::collect_ancestry`). The hook does not know which pane the
/// session runs in; the supervisor finds the pane in the chain later.
pub fn record_own_ancestry() -> Result<RecordedAncestry, String> {
    let me = procs::ProcessIdentity::read(std::process::id())
        .ok_or_else(|| "this process's own identity could not be read".to_string())?;
    let ancestry = procs::collect_ancestry(me)?;
    Ok(RecordedAncestry {
        links: ancestry.links.iter().map(RecordedLink::encode).collect(),
        ended: ancestry.ended,
    })
}

/// The drop directory for one session, or `None` when the session id could
/// not safely be a single directory name.
///
/// Session ids are UUIDs minted by the supervisor, but the hook reads its id
/// from the environment, so it is checked here rather than trusted to stay
/// inside [`REPORTS_DIR`]: only ASCII letters, digits, `-` and `_`, at most
/// 128 bytes.
pub fn session_dir(state_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let safe = !session_id.is_empty()
        && session_id.len() <= MAX_SESSION_ID_BYTES
        && session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    safe.then(|| state_dir.join(REPORTS_DIR).join(session_id))
}

/// Write `report` into its slot in the session's drop directory, replacing
/// whatever report the slot held.
///
/// The report is written under a private temporary name and renamed into
/// place, so the supervisor reads either the old report or the new one,
/// never part of one. Creates [`REPORTS_DIR`] and the session's directory
/// (private to the user) when missing, but never the state directory itself:
/// a state directory that does not exist means no supervisor ever ran for
/// this session there, and creating one would only leave a stray tree behind.
pub fn write_report(
    state_dir: &Path,
    session_id: &str,
    report: &HookReport,
) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
    let dir = session_dir(state_dir, session_id).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the session id cannot name a report directory",
        )
    })?;
    for path in [state_dir.join(REPORTS_DIR), dir.clone()] {
        match std::fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let bytes = serialize_within_cap(report)?;
    let slot = report.slot();
    // Unique per process and instant, so two hooks firing at once never
    // write into each other's temporary file.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let temp = dir.join(format!(
        "{TEMP_PREFIX}{}-{}-{nonce}",
        slot.name(),
        std::process::id()
    ));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .and_then(|mut file| file.write_all(&bytes))
        .and_then(|()| std::fs::rename(&temp, dir.join(slot.file_name())));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Why a report file could not be read, split by what the reader should do
/// about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadError {
    /// Opening or reading the file failed (a descriptor shortage, a storage
    /// error): the report itself may be fine, so it is kept for a later try.
    Io(String),
    /// The file is not a report this build reads: larger than any it
    /// writes, not JSON of this shape, or another format version. Trying
    /// again cannot change that.
    Invalid(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(reason) | ReadError::Invalid(reason) => f.write_str(reason),
        }
    }
}

/// Serialize `report`, dropping ancestry links from the top until the file
/// fits [`MAX_REPORT_FILE_BYTES`].
///
/// The walk budget bounds command-line bytes, not the JSON around them, and
/// many short arguments cost far more as base64 strings than they count
/// against that budget; a file over the cap would be refused unread. The
/// links that go are the ones furthest from the hook, above the session's
/// pane in any real ancestry, and `ancestry_ended` says the chain was cut, so
/// a report that loses its pane to this is refused with that reason.
fn serialize_within_cap(report: &HookReport) -> std::io::Result<Vec<u8>> {
    let bytes = serde_json::to_vec(report)?;
    let Some(links) = report.ancestry.as_ref() else {
        return Ok(bytes);
    };
    if bytes.len() as u64 <= MAX_REPORT_FILE_BYTES {
        return Ok(bytes);
    }
    // Measure everything but the links once, with the cut noted, then keep
    // as many links from the hook's end as fit beside it, counting each
    // link's own encoding and the comma that separates it.
    const CUT: &str = "the recorded ancestry was cut to fit the report file";
    let mut trimmed = report.clone();
    trimmed.ancestry_ended = Some(match &report.ancestry_ended {
        Some(ended) => format!("{ended}; {CUT}"),
        None => CUT.to_string(),
    });
    trimmed.ancestry = Some(Vec::new());
    let mut used = serde_json::to_vec(&trimmed)?.len() as u64;
    let mut keep = 0;
    for link in links {
        let cost = serde_json::to_vec(link)?.len() as u64 + 1;
        if used + cost > MAX_REPORT_FILE_BYTES {
            break;
        }
        used += cost;
        keep += 1;
    }
    trimmed.ancestry = Some(links[..keep].to_vec());
    Ok(serde_json::to_vec(&trimmed)?)
}

/// Read one report file, refusing anything this build would not have
/// written: larger than [`MAX_REPORT_FILE_BYTES`], not JSON of this shape,
/// or another [`FORMAT_VERSION`]. The error says whether the file failed to
/// read or failed to be a report (see [`ReadError`]), as a sentence for the
/// supervisor's log.
pub(crate) fn read_report(path: &Path) -> Result<HookReport, ReadError> {
    use std::io::Read as _;
    // A slot that is not a plain file (a directory, a symlink loop) will
    // not become one by waiting; anything else that stops the open (a
    // descriptor shortage, a permission or storage error) might.
    let file = std::fs::File::open(path).map_err(|error| {
        let reason = format!("could not open it: {error}");
        if error.kind() == std::io::ErrorKind::IsADirectory
            || error.raw_os_error() == Some(libc::ELOOP)
        {
            ReadError::Invalid(reason)
        } else {
            ReadError::Io(reason)
        }
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReadError::Io(format!("could not read it: {error}")))?;
    if bytes.len() as u64 > MAX_REPORT_FILE_BYTES {
        return Err(ReadError::Invalid(
            "it is larger than any report this build writes".into(),
        ));
    }
    let report: HookReport = serde_json::from_slice(&bytes)
        .map_err(|error| ReadError::Invalid(format!("it is not a report: {error}")))?;
    if report.version != FORMAT_VERSION {
        return Err(ReadError::Invalid(format!(
            "it is report format {}, and this build reads only {FORMAT_VERSION}",
            report.version
        )));
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// The per-session hook log
// ---------------------------------------------------------------------------
//
// `<state_dir>/hook-log/<session-id>.log` is the one place a report's fate is
// visible to a person: the hook may not print, and the supervisor that judges
// the report runs later and elsewhere. Two writers share it, one line each:
// the hook, for what its run did (`written`, `bad-payload`, ...; see the
// `farhelm` crate's `hook` module), and the supervisor, for its verdict on
// each report it settles (`acked`, or `refused` with the reason). Both render
// through [`render_log_line`] and append through [`append_hook_log`], so the
// line shape and its sanitizing have one definition.

/// Size past which the hook log is truncated before the next append.
///
/// Truncation rather than rotation is deliberate: this file is a
/// last-resort diagnostic for a session someone is actively looking at,
/// and losing an old line is cheaper than owning a rotation scheme (and
/// its own failure modes) for a file nothing else reads.
pub const MAX_LOG_BYTES: u64 = 64 * 1024;

/// Longest conversation id a log line carries, in characters.
const MAX_LOGGED_ID_CHARS: usize = 128;

/// Render one hook-log line:
/// `<unix-seconds> <word> [<detail> ]<conversation-id> <source>`, with the
/// identity pair present only when `about` names one and an empty source
/// rendered as `-` so the line keeps a fixed shape (a trailing space is
/// invisible in a log and turns "no source" into "unparsable line"). Every
/// interpolated value is sanitized; see `sanitize`.
pub fn render_log_line(
    seconds: u64,
    word: &str,
    detail: &str,
    about: Option<(&str, &str)>,
) -> String {
    let mut line = format!("{seconds} {word}");
    if !detail.is_empty() {
        line.push(' ');
        line.push_str(&sanitize(detail, 512, false));
    }
    if let Some((conversation, source)) = about {
        line.push(' ');
        line.push_str(&sanitize(conversation, MAX_LOGGED_ID_CHARS, true));
        line.push(' ');
        if source.is_empty() {
            line.push('-');
        } else {
            line.push_str(&sanitize(source, 64, true));
        }
    }
    line
}

/// Make a value safe to interpolate into one log line.
///
/// Every character [`farhelm_proto::text::is_presentation_unsafe`] names
/// becomes `_`. Control characters are the obvious case: they stop an
/// agent-supplied conversation id from embedding a newline and forging an
/// extra line. The rest do the same damage without being controls (line
/// separators many viewers break on, bidi controls that make a `refused` line
/// render as a `written` one) or let two different ids read identically
/// (zero-width characters), which in an audit log is the same lie.
/// `single_token` additionally collapses spaces, and is used for the
/// trailing identity fields: those are positional, so a space inside one
/// would silently shift the other. Free-form detail keeps its spaces,
/// because an I/O error is a sentence and mangling it would defeat the
/// log's only purpose.
///
/// The length cap is counted in chars and applied before replacement, so a
/// hostile value cannot outgrow its field.
fn sanitize(value: &str, max_chars: usize, single_token: bool) -> String {
    value
        .chars()
        .take(max_chars)
        .map(|c| {
            if farhelm_proto::text::is_presentation_unsafe(c) || (single_token && c.is_whitespace())
            {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Append one line to the hook log, ignoring every failure.
///
/// Creates the log's own directory 0700 if missing — it holds one file per
/// session under the supervisor's state directory, and nothing outside
/// that state directory has any business reading them — but never the
/// state directory itself. Truncates the file
/// first when it has grown past [`MAX_LOG_BYTES`].
///
/// ## One line, one write
///
/// The line and its trailing newline go out in a SINGLE `write_all`, never
/// as a `writeln!` that may reach the descriptor in pieces. There is no
/// locking here and deliberately so: several agents can be launched in one
/// session, and the supervisor appends its verdicts to the same file, so two
/// writers appending at once would, with a split write, interleave halfway
/// through a line and destroy the one property this format promises. With one write per line the worst case is whole lines
/// out of order, which costs a reader nothing. This relies on
/// `O_APPEND` + a single small write, which is atomic on the local
/// filesystems this file lives on; it is a diagnostic, not a ledger, and
/// that is the right amount of guarantee to buy for it.
///
/// ## Why no symlink hardening
///
/// The path is inside the supervisor's own 0700 state directory. Any
/// process able to plant a symlink or a FIFO there already holds the
/// session credential sitting beside it and already has the user's own
/// file access, so `O_NOFOLLOW` would defend a boundary that was crossed
/// before this function ran.
///
/// Nothing here reports failure, by design: this function exists to
/// explain a broken run, and a hook (or a report) that fails because its
/// own diagnostics failed would be the worst outcome of all.
pub fn append_hook_log(path: &Path, line: &str) {
    use std::os::unix::fs::DirBuilderExt as _;

    // Only the log's own directory is created, never the state directory
    // above it: a hook pointed at a state directory that does not exist
    // has no supervisor to explain anything to, and creating one would leave
    // a stray tree behind (the same rule `write_report` keeps).
    if let Some(parent) = path.parent() {
        let _ = std::fs::DirBuilder::new().mode(0o700).create(parent);
    }
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        // Truncate rather than rotate; see MAX_LOG_BYTES.
        let _ = std::fs::File::create(path);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = file.write_all(format!("{line}\n").as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::ReportVendor;

    fn report(vendor: ReportVendor, event: Option<&str>) -> HookReport {
        HookReport {
            version: FORMAT_VERSION,
            vendor,
            conversation: "conv-1".to_string(),
            source: "startup".to_string(),
            transcript_path: None,
            hook_event_name: event.map(|e| serde_json::Value::String(e.to_string())),
            agent_id: None,
            ancestry: None,
            ancestry_ended: None,
        }
    }

    /// Codex and Grok keep selection independently of later confirmation.
    /// A supervisor outage must not lose the clear/new transition to a Stop;
    /// integrations with complete independent reports retain one latest slot.
    ///
    /// Why: Grok's enrichment is refused unless its selection was applied
    /// first. With one slot, enrichments made during a long supervisor
    /// outage would overwrite the selection and the conversation would be
    /// lost; the slot split is what keeps the drop directory bounded at two
    /// files without that loss.
    #[test]
    fn selecting_integrations_keep_enrichment_in_a_separate_slot() {
        assert_eq!(
            report(ReportVendor::Grok, Some("SessionStart")).slot(),
            Slot::Selection
        );
        assert_eq!(
            report(ReportVendor::Grok, Some("Stop")).slot(),
            Slot::Enrichment
        );
        assert_eq!(
            report(ReportVendor::Grok, Some("UserPromptSubmit")).slot(),
            Slot::Enrichment
        );
        assert_eq!(
            report(ReportVendor::Codex, Some("SessionStart")).slot(),
            Slot::Selection
        );
        assert_eq!(
            report(ReportVendor::Codex, Some("Stop")).slot(),
            Slot::Enrichment
        );
        for vendor in [
            ReportVendor::Claude,
            ReportVendor::Goose,
            ReportVendor::Pi,
            ReportVendor::Omp,
        ] {
            assert_eq!(report(vendor, Some("SessionStart")).slot(), Slot::Latest);
        }
        assert_eq!(
            Slot::DRAIN_ORDER
                .iter()
                .position(|slot| *slot == Slot::Selection),
            Some(0),
            "selection drains before enrichment"
        );
    }

    /// Spec: a written report replaces its slot whole, reads back
    /// unchanged, and leaves no temporary file behind; the hook creates the
    /// drop directories but not a missing state directory.
    ///
    /// Why: the supervisor may read the slot at any moment, so a report must
    /// appear in it atomically, and the latest report must win. Creating a
    /// state directory would scatter trees wherever a stray environment
    /// points.
    #[test]
    fn a_report_replaces_its_slot_atomically() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let mut first = report(ReportVendor::Claude, Some("SessionStart"));
        first.ancestry = Some(record_own_ancestry().expect("own ancestry").links);
        write_report(state.path(), "session-1", &first).expect("first write");
        let mut second = first.clone();
        second.conversation = "conv-2".to_string();
        write_report(state.path(), "session-1", &second).expect("second write");

        let dir = session_dir(state.path(), "session-1").expect("valid id");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .expect("drop dir")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, ["latest.json"], "one slot file and nothing else");
        let read = read_report(&dir.join("latest.json")).expect("readable");
        assert_eq!(read.conversation, "conv-2");
        let chain = read.chain().expect("decodes").expect("has ancestry");
        assert_eq!(
            chain[0].pid,
            std::process::id(),
            "the chain starts at the writer"
        );

        let missing = state.path().join("no-such-state");
        assert!(write_report(&missing, "session-1", &first).is_err());
        assert!(!missing.exists(), "the state directory is never created");
    }

    /// Spec: a report file's JSON shape — field names, the vendor's
    /// spelling, base64 byte fields, omitted empty optionals — is exactly
    /// this.
    ///
    /// Why: these files cross builds. A hook binary and a supervisor of
    /// different versions meet across an update, and the format version only
    /// helps if the shape under one version never drifts; a serde attribute
    /// change would compile and round-trip while writing files the other
    /// side cannot read.
    #[test]
    fn the_report_file_shape_is_pinned() {
        let mut pinned = report(ReportVendor::Claude, Some("SessionStart"));
        pinned.ancestry = Some(vec![RecordedLink {
            pid: 2,
            ppid: 1,
            start: 3,
            exe: "L2Jpbi9zaA==".to_string(),
            argv: Some(vec!["c2g=".to_string()]),
            cwd: None,
        }]);
        assert_eq!(
            serde_json::to_value(&pinned).unwrap(),
            serde_json::json!({
                "version": 1,
                "vendor": "claude",
                "conversation": "conv-1",
                "source": "startup",
                "hook_event_name": "SessionStart",
                "ancestry": [{
                    "pid": 2,
                    "ppid": 1,
                    "start": 3,
                    "exe": "L2Jpbi9zaA==",
                    "argv": ["c2g="],
                    "cwd": null,
                }],
            })
        );
    }

    /// Spec: a report whose ancestry would make the file larger than the
    /// supervisor reads is written with links dropped from the top until it
    /// fits, and says the chain was cut.
    ///
    /// Why: the walk budget counts command-line bytes, but many short
    /// arguments cost far more once encoded, and a file over the cap would be
    /// refused unread, losing the report. The links nearest the hook are the
    /// ones attribution needs, so those are kept.
    #[test]
    fn an_oversized_ancestry_is_cut_to_fit_the_file() {
        use base64::Engine as _;
        let state = farhelm_teststate::tempdir().expect("state dir");
        let short_args: Vec<String> = (0..30_000)
            .map(|_| base64::engine::general_purpose::STANDARD.encode(b"a"))
            .collect();
        let mut big = report(ReportVendor::Claude, None);
        big.ancestry = Some(
            (0..40u32)
                .map(|i| RecordedLink {
                    pid: 100 + i,
                    ppid: 101 + i,
                    start: 1,
                    exe: "L2Jpbi9zaA==".to_string(),
                    argv: Some(short_args.clone()),
                    cwd: None,
                })
                .collect(),
        );
        assert!(
            serde_json::to_vec(&big).unwrap().len() as u64 > MAX_REPORT_FILE_BYTES,
            "fixture premise: untrimmed, the report is over the cap"
        );
        write_report(state.path(), "session-1", &big).expect("write");
        let path = session_dir(state.path(), "session-1")
            .unwrap()
            .join(Slot::Latest.file_name());
        assert!(std::fs::metadata(&path).unwrap().len() <= MAX_REPORT_FILE_BYTES);
        let read = read_report(&path).expect("the cut report reads back");
        let links = read.ancestry.expect("ancestry kept");
        assert!(
            !links.is_empty() && links.len() < 40,
            "{} links",
            links.len()
        );
        assert_eq!(links[0].pid, 100, "the hook's end of the chain is kept");
        assert!(
            read.ancestry_ended
                .is_some_and(|ended| ended.contains("cut"))
        );
    }

    /// Spec: a session id that is not a plain single name is refused before
    /// anything is written.
    ///
    /// Why: the hook takes its session id from the environment, and an id
    /// like `../x` would otherwise write a report outside the drop tree.
    #[test]
    fn an_unsafe_session_id_names_no_directory() {
        let state = std::path::Path::new("/state");
        for id in ["", "..", "../x", "a/b", "a b", &"x".repeat(129)] {
            assert!(session_dir(state, id).is_none(), "{id:?}");
        }
        assert!(session_dir(state, "0c3b1f6e-5a1d-4c55-9f7e-2d1f7b0a9e11").is_some());
    }

    /// Spec: the reader refuses oversized files and other format versions,
    /// and a recorded command line over the per-process budget refuses the
    /// report at anchoring rather than being kept or cut short.
    ///
    /// Why: the file is input to the supervisor. Its bounds must hold even
    /// for a file this build's hook did not write, and a truncated command
    /// line could be mistaken by attribution for a whole one.
    #[test]
    fn reading_rebounds_the_file_and_its_evidence() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let path = state.path().join("latest.json");
        std::fs::write(&path, vec![b' '; MAX_REPORT_FILE_BYTES as usize + 1]).expect("write");
        assert!(read_report(&path).is_err(), "oversized");

        let mut future = report(ReportVendor::Pi, None);
        future.version = FORMAT_VERSION + 1;
        std::fs::write(&path, serde_json::to_vec(&future).unwrap()).expect("write");
        assert!(read_report(&path).is_err(), "another format version");

        use base64::Engine as _;
        let mut long = report(ReportVendor::Claude, None);
        long.ancestry = Some(vec![RecordedLink {
            pid: 1,
            ppid: 0,
            start: 1,
            exe: base64::engine::general_purpose::STANDARD.encode(b"/bin/sh"),
            argv: Some(vec![
                base64::engine::general_purpose::STANDARD.encode(vec![
                    b'x';
                    procs::MAX_ARGV_BYTES_PER_PROCESS
                        + 1
                ]),
            ]),
            cwd: None,
        }]);
        let chain = long.chain().expect("decodes").expect("has ancestry");
        let pane = procs::PaneAnchor {
            pid: 1,
            start: None,
        };
        let error = procs::anchor_chain(&chain, None, pane).expect_err("over-budget argv refuses");
        assert!(error.contains("per-process evidence budget"), "{error}");
    }
}
