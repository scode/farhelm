//! Conversation-identity capture: deciding, without ever guessing, which
//! of an agent's on-disk conversation records belongs to which session
//! (PLAN_M3.md item 8).
//!
//! Everything here is agent-AGNOSTIC — window arithmetic, the bounded and
//! hardened directory walk, the ambiguity rule, timestamp parsing — and
//! takes the per-kind knowledge it needs through
//! [`AgentIntegration`](super::AgentIntegration): where records live, how
//! deep, which files could be one, and how to read a record's correlators.
//! The parent module's docs explain why the line is drawn there.
//!
//! Capture is observation-only per SPEC.md: this module reads files the
//! agents write for their own purposes and writes nothing into their
//! directories, ever.
//!
//! ## Ambiguity is a first-class answer
//!
//! The failure this whole mechanism exists to exclude is resuming the
//! WRONG conversation, which is silent and unrecoverable. So every
//! uncertainty resolves to [`CaptureVerdict::Ambiguous`] — no identity for
//! the affected sessions, ever, for that launch — rather than to a best
//! guess. Two sessions in one working directory whose windows overlap
//! poison each other; two candidate records inside one window poison the
//! session they both fit. SPEC.md's fallback (an honest "never captured,
//! here is a fresh launch") is the intended outcome of a bail, not a
//! degradation to be minimized.
//!
//! The same stance governs INCOMPLETE evidence. A scan that could not
//! enumerate, open, or parse everything it was supposed to
//! ([`ScanOutcome::complete`]) is not allowed to produce a durable claim
//! at all: the record that would have made a lone candidate ambiguous is
//! exactly the one a failed read hides.
//!
//! That is also the sharpest contrast with this subsystem's other half.
//! Status sharpening is allowed to be wrong because a wrong badge is
//! cosmetic; nothing here is, because a wrong claim is a conversation
//! nobody can get back.

use super::{AgentIntegration, RECORD_PREFIX_BYTES};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// How far BEFORE a session's first-input time a record may claim to have
/// been created and still be considered that session's.
///
/// The record is written when the agent submits the first prompt, which is
/// strictly after the supervisor confirmed delivery of the first input
/// byte — so in principle this could be zero. It is not, because both
/// timestamps are whole seconds from two independent clock readings (ours
/// at delivery time, the agent's at write time), and a record that lands
/// in the same second can round either way. Small on purpose: every second
/// of slack here is a second of extra overlap between two sessions sharing
/// a working directory, and overlap costs capture.
pub const CAPTURE_WINDOW_BEFORE: Duration = Duration::from_secs(5);

/// How far AFTER a session's first-input time a record may be created and
/// still be considered that session's.
///
/// This is the one genuinely judgemental constant in the module, and it
/// trades two real costs against each other:
///
/// - **Too short** and a user whose first keystroke is not their first
///   prompt — dismissing a trust dialog, editing a long prompt before
///   pressing enter, pasting — has their record land outside the window and
///   is never captured. The cost is an honest `FreshOnly` offer.
/// - **Too long** and two sessions started in the same working directory
///   within this span overlap, so BOTH bail. The cost is again an honest
///   `FreshOnly` offer for both.
///
/// Since both failure directions are the same honest fallback, the value
/// is chosen for the case that actually happens: one session per directory
/// at a time, with a human composing a prompt. A minute covers ordinary
/// composition without making "two sessions in this directory this
/// afternoon" collide.
///
/// **Accepted limitation on the anchor itself.** The window is anchored on
/// the first input byte the supervisor confirms tmux delivered, and not
/// every such byte is a human keystroke: a terminal emulator answers
/// device-status and primary-DA queries on its own, so an agent that
/// probes the terminal at startup can provoke an `onData` reply and start
/// this clock before the user has touched anything. The failure direction
/// is a MISSED capture — the record then appears after the window and
/// nothing is claimed, yielding SPEC.md's honest fresh-launch fallback —
/// never a wrong one, because an early anchor can only ever exclude
/// candidates, never admit a record belonging to someone else.
pub const CAPTURE_WINDOW_AFTER: Duration = Duration::from_secs(60);

/// How long after a session's window closes the supervisor waits before
/// running the one COMPLETE scan that is allowed to commit a claim.
///
/// Two things need this grace, and they are different. A record created in
/// the last instant of the window may not be readable yet (the agent
/// writes and flushes; the directory entry and the content land in their
/// own time), so committing at exactly `end` could miss a candidate that
/// would have made a lone match ambiguous. And the grace is what makes the
/// overlap rule airtight in one direction: a session whose horizon has
/// closed can no longer be poisoned by a NEW session's first input, which
/// holds precisely when `grace >= before` (a later session's window starts
/// at `t2 - before`, and `t2 > t1 + after + grace >= t1 + after + before`
/// puts it strictly past this session's end). [`CaptureWindowBounds::new`]
/// enforces that relationship rather than leaving it to whoever picks the
/// numbers.
pub const CAPTURE_PUBLICATION_GRACE: Duration = Duration::from_secs(5);
/// Cap on how many directory ENTRIES one scan may visit, counting every
/// entry it looks at — files it will skip by name, subdirectories it
/// queues, and records alike — plus every directory it opens.
///
/// Counting only the entries that turned out to be records would leave the
/// budget unbounded in exactly the case it exists for: a directory holding
/// a hundred thousand unrelated files costs a hundred thousand `readdir`
/// results whether or not any of them is a `.jsonl`. Hitting the cap marks
/// the scan INCOMPLETE, so it defers a claim rather than making one from a
/// partial view.
const MAX_VISITED_ENTRIES: usize = 4096;

/// Cap on the total bytes one scan may read out of record files.
///
/// Independent of the entry cap because they bound different things: a
/// thousand 64 KiB prefixes is 64 MiB of reads from only a thousand
/// entries. Recent records are read first ([`scan_records`]), so the
/// budget is spent where a match can plausibly be rather than on the
/// oldest history.
const MAX_SCAN_BYTES: usize = 8 * 1024 * 1024;

/// Wall-clock cap on one scan.
///
/// The other two budgets bound WORK; this one bounds LATENCY, which is a
/// different property and the one that matters here: this scan runs inside
/// a `ListSessions` handler, and a slow or contended filesystem can make a
/// bounded amount of work take an unbounded amount of time. Exceeding it
/// marks the scan incomplete, so the next poll retries — a capture arriving
/// one cadence later costs nothing, while a wedged list reply costs the UI.
const MAX_SCAN_DURATION: Duration = Duration::from_millis(750);
/// How many conversation ids an ambiguity diagnostic names before it
/// summarizes. The message reaches a log and, through it, a human
/// explaining why their session offered a fresh launch; a directory with a
/// thousand in-window records must not produce a thousand-id line.
const MAX_AMBIGUITY_IDS: usize = 4;

/// The correlators one on-disk record carries, as read from its own
/// per-line JSON (never from the filesystem — see the module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCorrelators {
    /// The agent's own conversation id: exactly the value a resume
    /// template's `{conversation}` element is replaced with. Validated by
    /// [`super::is_plausible_conversation_id`] before it is ever retained.
    pub conversation: String,
    /// The working directory the agent recorded for itself. This — not the
    /// munged directory name the file was found under — is what decides
    /// whether a record could belong to a session. Compared against the
    /// session's CANONICAL cwd, because that is what the agent's own
    /// `getcwd()` reports.
    pub cwd: String,
    /// When the record was created, in seconds since the Unix epoch,
    /// parsed from the record's own timestamp field.
    pub created_at: i64,
}

/// A record file's cheap identity, for deciding whether a re-read is
/// warranted.
///
/// Deliberately NOT a content hash: this is consulted on every poll for
/// every captured session, and hashing a growing file would defeat the
/// point. See [`RecordStamp::differs`] for exactly what it can and cannot
/// notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordStamp {
    pub len: u64,
    /// `None` where the filesystem does not report one.
    pub mtime_unix: Option<i64>,
}

impl RecordStamp {
    /// Whether the file may have changed since `previous` — the append
    /// signal a re-verification is gated on.
    ///
    /// Deliberately not `!=`. Two cases need explicit handling and neither
    /// is what derived equality does:
    ///
    /// - **An unknown mtime on either side counts as changed.** A stamp
    ///   with no mtime carries strictly less information, and treating two
    ///   information-free stamps as "equal, nothing to do" would skip
    ///   re-verification forever on a filesystem that reports no mtimes.
    /// - **Same length and same second reads as UNCHANGED, and that is a
    ///   known blind spot.** mtime has one-second granularity here, so a
    ///   rewrite preserving the byte count within one second is invisible.
    ///   The consequence is a skipped re-verification, which RETAINS the
    ///   already-captured identity — the safe direction, since
    ///   re-verification can only ever confirm or log, never re-claim
    ///   (`service`'s `reverify_capture`). Recorded here rather than fixed
    ///   because the fix (hashing on every poll) costs more than the blind
    ///   spot does.
    pub fn differs(&self, previous: &RecordStamp) -> bool {
        match (self.mtime_unix, previous.mtime_unix) {
            (Some(now), Some(then)) => self.len != previous.len || now != then,
            _ => true,
        }
    }
}

/// One record file that could belong to some session: its correlators,
/// where it lives, and its change stamp.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub correlators: RecordCorrelators,
    pub path: PathBuf,
    pub stamp: RecordStamp,
}

/// What one directory scan found, and whether it found ALL of it.
///
/// `complete` is the load-bearing half. A scan that hit a budget, could
/// not read a directory, could not open a file, or could not parse one it
/// opened has an unknown number of candidates it never saw — and the
/// unseen one is exactly what would have turned a lone match into an
/// ambiguity. Callers may therefore hold a PROVISIONAL match from an
/// incomplete scan but must never commit one.
///
/// A missing ROOT is the one absence that is authoritative rather than
/// suspicious: an agent that has never run on this host has no directory,
/// and that is a complete answer of "no records".
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    pub candidates: Vec<Candidate>,
    pub complete: bool,
}

/// The window, in seconds since the Unix epoch, in which a record may have
/// been created and still be attributed to one session.
///
/// Inclusive at both ends: the bounds are already slack, and an off-by-one
/// at a boundary would only ever be arbitrary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureWindow {
    pub start: i64,
    pub end: i64,
}

impl CaptureWindow {
    /// The window a session with this first-input time owns.
    pub fn around(first_input_at: i64, bounds: CaptureWindowBounds) -> CaptureWindow {
        CaptureWindow {
            start: first_input_at.saturating_sub(bounds.before.as_secs() as i64),
            end: first_input_at.saturating_add(bounds.after.as_secs() as i64),
        }
    }

    /// Whether `created_at` falls inside this window.
    pub fn contains(&self, created_at: i64) -> bool {
        created_at >= self.start && created_at <= self.end
    }

    /// Whether two sessions' windows touch at all.
    ///
    /// Overlap is what makes two sessions in one working directory poison
    /// each other: a record landing in the shared span could honestly
    /// belong to either, and there is no evidence that would separate them.
    pub fn overlaps(&self, other: &CaptureWindow) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}

/// The bounds a supervisor applies when building [`CaptureWindow`]s, plus
/// the publication grace that decides when a claim may become durable.
///
/// Injectable rather than hard-coded at the use site for the same reason
/// `SupervisorTimeouts` is: the production values are tuned for a human
/// composing a prompt (tens of seconds), and a test that had to wait them
/// out to prove two sessions in one directory DON'T overlap would take
/// minutes. Tests shorten all three; nothing else ever varies them.
#[derive(Debug, Clone, Copy)]
pub struct CaptureWindowBounds {
    pub before: Duration,
    pub after: Duration,
    /// See [`CAPTURE_PUBLICATION_GRACE`]. Never smaller than `before`;
    /// [`CaptureWindowBounds::new`] raises it rather than accepting a
    /// combination whose overlap reasoning does not hold.
    pub grace: Duration,
}

impl CaptureWindowBounds {
    /// Build bounds, enforcing `grace >= before`.
    ///
    /// Raised silently rather than rejected because there is no caller for
    /// whom failing is better: production passes the constants, and a test
    /// picking a short window wants the shortest grace that keeps the
    /// invariant, which is exactly what this produces. See
    /// [`CAPTURE_PUBLICATION_GRACE`] for what the invariant buys.
    pub fn new(before: Duration, after: Duration, grace: Duration) -> CaptureWindowBounds {
        CaptureWindowBounds {
            before,
            after,
            grace: grace.max(before),
        }
    }

    /// The instant, in Unix seconds, after which this session's evidence
    /// is settled: its window has closed and any record written inside it
    /// has had the publication grace to become readable. Only a COMPLETE
    /// scan at or after this point may commit a claim.
    pub fn horizon(&self, first_input_at: i64) -> i64 {
        first_input_at
            .saturating_add(self.after.as_secs() as i64)
            .saturating_add(self.grace.as_secs() as i64)
    }
}

impl Default for CaptureWindowBounds {
    fn default() -> Self {
        CaptureWindowBounds::new(
            CAPTURE_WINDOW_BEFORE,
            CAPTURE_WINDOW_AFTER,
            CAPTURE_PUBLICATION_GRACE,
        )
    }
}

/// What a rescan concluded for one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureVerdict {
    /// Nothing matched. Before the horizon this means "not yet"; at or
    /// after it, with a complete scan, it means this session's agent never
    /// wrote a record this build could correlate. Deliberately never a
    /// deadline from session CREATION: the launch-to-first-prompt gap is
    /// unbounded by construction.
    NotYet,
    /// Exactly one record matched.
    Captured(String),
    /// More than one honest answer existed. The string explains what
    /// collided, for the log line SPEC.md's fallback owes the user.
    Ambiguous(String),
}
/// Every record under `root` that this scan could read, and whether it
/// read all of them.
///
/// Deliberately NOT filtered by working directory: `root` is shared by
/// every Codex session on the host, so the caller scans once per ROOT per
/// pass and buckets the results by each candidate's recorded `cwd`.
///
/// ## Order of work, and why
///
/// Enumeration first, reading second. The two phases exist so the
/// expensive phase can be PRIORITIZED: records are opened newest-first, so
/// a byte or time budget is spent on the files a match can plausibly be in
/// rather than on the oldest history in the tree. A single interleaved
/// pass could not do that, and on a host with years of rollout files it
/// would reliably exhaust its budget before reaching today's.
///
/// Within enumeration the syscalls are ordered cheapest-first:
/// `file_type()` comes off the `readdir` result itself on Linux (no extra
/// syscall), the name filter is pure string work, and only a surviving
/// candidate pays for `metadata`. The mtime lower bound is applied last,
/// before any file is opened — sound because records are append-only, so
/// a file last touched before the earliest window under consideration
/// cannot have been CREATED inside it. It is a filter on which files must
/// be OPENED, never an answer about when a record was made (the module
/// docs cover why the answer only ever comes from the record's own JSON).
///
/// ## Completeness
///
/// A missing ROOT is authoritative emptiness — an agent that has never run
/// on this host has no directory — and is the only absence reported as
/// complete. Everything else that could hide a record marks the scan
/// incomplete: an unreadable directory, an unreadable entry, a file that
/// cannot be opened or read, a parse failure, and any of the three
/// budgets. See [`ScanOutcome`] for what callers may and may not do with
/// an incomplete result.
///
/// ## Hardening
///
/// Symlinks are never followed, at either level: a symlinked directory is
/// skipped outright (it could point anywhere, including outside the home)
/// and a file is opened `O_NOFOLLOW | O_NONBLOCK` and then re-checked
/// through its own descriptor, which closes the metadata-then-open TOCTOU
/// and — with `O_NONBLOCK` — keeps a FIFO named `something.jsonl` from
/// parking a request handler on an open that never returns.
pub async fn scan_records(
    integration: &dyn AgentIntegration,
    root: &Path,
    mtime_floor: i64,
) -> ScanOutcome {
    let started = Instant::now();
    let mut complete = true;
    let mut visited = 0usize;
    let mut files: Vec<(PathBuf, RecordStamp)> = Vec::new();
    let mut queue = vec![(root.to_path_buf(), integration.record_depth())];

    while let Some((dir, depth)) = queue.pop() {
        // Every directory OPENED counts, so a tree of empty directories
        // cannot walk for free.
        visited += 1;
        if visited > MAX_VISITED_ENTRIES || started.elapsed() > MAX_SCAN_DURATION {
            return ScanOutcome {
                candidates: Vec::new(),
                complete: false,
            };
        }
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Only the ROOT's absence is authoritative. A subdirectory
                // that vanished mid-walk means the tree changed under us,
                // so this scan no longer describes one moment.
                if dir != root {
                    complete = false;
                }
                continue;
            }
            Err(_) => {
                complete = false;
                continue;
            }
        };
        loop {
            match entries.next_entry().await {
                Ok(None) => break,
                Err(_) => {
                    complete = false;
                    break;
                }
                Ok(Some(entry)) => {
                    visited += 1;
                    if visited > MAX_VISITED_ENTRIES || started.elapsed() > MAX_SCAN_DURATION {
                        return ScanOutcome {
                            candidates: Vec::new(),
                            complete: false,
                        };
                    }
                    let Ok(file_type) = entry.file_type().await else {
                        complete = false;
                        continue;
                    };
                    if file_type.is_symlink() {
                        // Never followed, at either level: a symlink here
                        // could point anywhere, and a scan that chased one
                        // would read files outside the home it was given.
                        continue;
                    }
                    if file_type.is_dir() {
                        if depth > 0 {
                            queue.push((entry.path(), depth - 1));
                        }
                        continue;
                    }
                    if !file_type.is_file() {
                        continue;
                    }
                    let name = entry.file_name();
                    let Some(name) = name.to_str() else {
                        continue;
                    };
                    if !integration.is_record_file(name) {
                        continue;
                    }
                    let Ok(metadata) = entry.metadata().await else {
                        complete = false;
                        continue;
                    };
                    let stamp = RecordStamp {
                        len: metadata.len(),
                        mtime_unix: metadata.modified().ok().and_then(unix_seconds),
                    };
                    if stamp.mtime_unix.is_some_and(|mtime| mtime < mtime_floor) {
                        continue;
                    }
                    files.push((entry.path(), stamp));
                }
            }
        }
    }

    // Newest first, so a budget is spent where a match can be. Files with
    // no mtime sort first: unknown is treated as "could be the newest",
    // the conservative direction for a budget that might cut the tail off.
    files.sort_by_key(|(_, stamp)| std::cmp::Reverse(stamp.mtime_unix));

    let mut candidates = Vec::new();
    let mut bytes = 0usize;
    for (path, stamp) in files {
        if bytes >= MAX_SCAN_BYTES || started.elapsed() > MAX_SCAN_DURATION {
            complete = false;
            break;
        }
        match read_prefix(&path).await {
            Ok(Some(text)) => {
                bytes += text.len();
                match integration.parse_record(&text) {
                    Ok(Some(correlators)) => candidates.push(Candidate {
                        correlators,
                        path,
                        stamp,
                    }),
                    Ok(None) => {}
                    Err(_) => complete = false,
                }
            }
            // Gone between enumeration and open: the tree changed under
            // this scan, so it no longer describes a single moment.
            Ok(None) => complete = false,
            Err(_) => complete = false,
        }
    }
    ScanOutcome {
        candidates,
        complete,
    }
}

/// Decide one session's identity from the candidates its working directory
/// offered.
///
/// Pure, and separate from the I/O above, because this is where the
/// no-guessing rule actually lives and it deserves to be testable without
/// a filesystem. Exactly one in-window candidate is a match; two or more
/// is a bail naming them; none is "not yet".
///
/// Says nothing about whether the match may be COMMITTED — that depends on
/// the scan's completeness and on whether the session's horizon has
/// closed, both of which are the caller's to check (`service`'s
/// `capture_pass`).
pub fn choose(candidates: &[&Candidate], window: CaptureWindow) -> CaptureVerdict {
    let matching: Vec<&&Candidate> = candidates
        .iter()
        .filter(|c| window.contains(c.correlators.created_at))
        .collect();
    match matching.as_slice() {
        [] => CaptureVerdict::NotYet,
        [only] => CaptureVerdict::Captured(only.correlators.conversation.clone()),
        many => {
            let named: Vec<&str> = many
                .iter()
                .take(MAX_AMBIGUITY_IDS)
                .map(|c| c.correlators.conversation.as_str())
                .collect();
            let elided = many.len() - named.len();
            let suffix = if elided > 0 {
                format!(" and {elided} more")
            } else {
                String::new()
            };
            CaptureVerdict::Ambiguous(format!(
                "{} conversation records fall in this session's capture window ({}{suffix}); \
                 no identity is claimed for it",
                many.len(),
                named.join(", ")
            ))
        }
    }
}

/// Read one record's correlators and current stamp.
///
/// `Ok(None)` means the file is GONE, or is a well-formed file the
/// integration positively does not recognize — the two absences that need
/// no explanation. Every other failure is an `Err` carrying why, because
/// the caller's policy (retain the already-captured identity, and say so)
/// is only honest if it can name what went wrong.
pub async fn read_record(
    path: &Path,
    integration: &dyn AgentIntegration,
) -> anyhow::Result<Option<(RecordCorrelators, RecordStamp)>> {
    let Some(stamp) = stamp_of(path).await? else {
        return Ok(None);
    };
    let Some(text) = read_prefix(path).await? else {
        return Ok(None);
    };
    Ok(integration
        .parse_record(&text)?
        .map(|correlators| (correlators, stamp)))
}

/// The current stamp of a record file, or `Ok(None)` if it is gone.
/// One `metadata` call and nothing else — this is what gates whether a
/// re-read happens at all ([`RecordStamp::differs`]).
pub async fn stamp_of(path: &Path) -> anyhow::Result<Option<RecordStamp>> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(Some(RecordStamp {
            len: metadata.len(),
            mtime_unix: metadata.modified().ok().and_then(unix_seconds),
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("stat of {}", path.display()))),
    }
}

/// Read at most [`RECORD_PREFIX_BYTES`] of a file as (lossy) text, or
/// `Ok(None)` if it is gone.
///
/// Opened `O_NOFOLLOW | O_NONBLOCK` and then re-validated through the
/// resulting descriptor: `O_NOFOLLOW` refuses a symlink placed where a
/// record should be, `O_NONBLOCK` keeps a FIFO from parking this task on
/// an open that would never return, and the `fstat`-based regular-file
/// check is what closes the gap between the enumeration's `file_type` and
/// this open (the entry could have been replaced in between).
///
/// Lossy UTF-8 rather than strict because the prefix routinely ends
/// mid-character, and a strict decode would fail the whole read over a
/// truncation that never touches the correlators on the first line.
async fn read_prefix(path: &Path) -> anyhow::Result<Option<String>> {
    use tokio::io::AsyncReadExt;

    let opened = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .await;
    let file = match opened {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(anyhow::Error::new(e).context(format!("opening {}", path.display())));
        }
    };
    let metadata = file
        .metadata()
        .await
        .map_err(|e| anyhow::Error::new(e).context(format!("stat of open {}", path.display())))?;
    if !metadata.is_file() {
        anyhow::bail!(
            "{} is not a regular file; refusing to read it as a conversation record",
            path.display()
        );
    }
    let mut buffer = Vec::new();
    file.take(RECORD_PREFIX_BYTES as u64)
        .read_to_end(&mut buffer)
        .await
        .map_err(|e| anyhow::Error::new(e).context(format!("reading {}", path.display())))?;
    Ok(Some(String::from_utf8_lossy(&buffer).into_owned()))
}

/// Seconds since the Unix epoch for a `SystemTime`, or `None` for a
/// pre-epoch value (which no real file has and which nothing here needs to
/// reason about).
fn unix_seconds(when: SystemTime) -> Option<i64> {
    when.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

/// Now, in seconds since the Unix epoch. Degrades to 0 rather than failing
/// a caller over a pre-epoch system clock, matching `store`'s own
/// `now_unix`.
pub fn now_unix() -> i64 {
    unix_seconds(SystemTime::now()).unwrap_or(0)
}

// ---------------------------------------------------------------------
// RFC 3339 timestamps
//
// Hand-rolled rather than pulled from a date-time crate, and that is a
// deliberate trade: the supervisor's dependency set is deliberately small
// (it cross-compiles to musl for provisioning), and what is actually
// needed here is a total function from the two agents' timestamp spelling
// to a Unix second — no calendars, no zones beyond a fixed offset, no
// formatting locales.
//
// The parser VALIDATES rather than merely extracting, and that is not
// pedantry: a timestamp is the correlator that decides window membership,
// so silently normalizing a nonsense instant (February 31st, hour 25) into
// a real one would place a record in a window it never belonged to — the
// wrong-conversation failure this module exists to exclude, reached
// through the back door.
// ---------------------------------------------------------------------

/// Days in `month` of `year`, proleptic Gregorian. Zero for a month
/// outside 1..=12, which makes every day comparison against it fail.
fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Parse a fixed-width run of ASCII digits at `offset`, or `None`.
///
/// Explicit rather than `str::parse`, which accepts forms this grammar
/// does not: `"+1".parse::<i64>()` succeeds, and a two-character field of
/// this format is never a signed number.
fn digits(text: &str, offset: usize, width: usize) -> Option<i64> {
    let slice = text.get(offset..offset + width)?;
    if !slice.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    slice.parse().ok()
}

/// Parse an RFC 3339 timestamp into seconds since the Unix epoch, or
/// `None` if it is not one.
///
/// Strict about everything that could change which window a record falls
/// in, and total: every rejection returns `None`, and a `None` makes the
/// record a parse failure rather than a candidate with a guessed time.
///
/// - Layout is exact: `YYYY-MM-DDThh:mm:ss`, with `T` (either case) or a
///   space as the separator, and the input must be CONSUMED — no trailing
///   garbage after the zone.
/// - Ranges are real: month 1–12, day within that month in that year
///   (leap years included), hour 0–23, minute and second 0–59, offset hour
///   0–23 and offset minute 0–59. A leap second (`:60`) is rejected;
///   neither vendor writes one, and admitting it would need a policy for
///   what instant it names.
/// - A `.` must be followed by at least one digit.
/// - A zone is MANDATORY. A local-time timestamp with no offset cannot be
///   placed on the epoch line at all, and assuming UTC for one would shift
///   a record by hours — straight into or out of somebody's window.
///
/// Fractional seconds are accepted and TRUNCATED, not rounded: the whole
/// correlation works at second granularity with multi-second slack on both
/// sides ([`CaptureWindow`]), so sub-second precision could not change any
/// verdict and rounding would only add a boundary case.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let year = digits(text, 0, 4)?;
    let month = digits(text, 5, 2)? as u32;
    let day = digits(text, 8, 2)? as u32;
    let hour = digits(text, 11, 2)?;
    let minute = digits(text, 14, 2)?;
    let second = digits(text, 17, 2)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        rest = &fraction[digits..];
    }
    // Offsets are applied by SUBTRACTION: a `+02:00` timestamp names a
    // moment two hours earlier in UTC than its digits read.
    let offset_seconds = match rest.as_bytes().first() {
        Some(b'Z' | b'z') if rest.len() == 1 => 0,
        Some(sign @ (b'+' | b'-')) => {
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            let offset_hour = digits(rest, 1, 2)?;
            let offset_minute = digits(rest, 4, 2)?;
            if offset_hour > 23 || offset_minute > 59 {
                return None;
            }
            let sign = if *sign == b'-' { -1 } else { 1 };
            sign * (offset_hour * 3600 + offset_minute * 60)
        }
        // Everything else — a missing zone, `Zjunk`, a stray character —
        // is a refusal rather than a default.
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds)
}

/// Format a Unix second as an RFC 3339 UTC timestamp
/// (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Public because the fake-agent fixture writes records the supervisor
/// then parses, and sharing one implementation is what keeps a fixture bug
/// from masquerading as a parser bug (or the reverse).
pub fn format_rfc3339(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60
    )
}

/// Days since 1970-01-01 for a proleptic-Gregorian civil date (Howard
/// Hinnant's `days_from_civil`). Exact for every year this could ever see.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = ((month + 9) % 12) as i64;
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`] (`civil_from_days`), for formatting.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_kind::{ClaudeIntegration, CodexIntegration, RECORD_PREFIX_LINES};

    /// Window arithmetic is where the no-guessing rule becomes mechanical,
    /// so all three parts are pinned: membership decides whether a record
    /// can be a session's at all, overlap decides whether two sessions in
    /// one directory poison each other, and the horizon decides when a
    /// match may become durable. Bounds are inclusive — an off-by-one here
    /// would silently change which pairs bail.
    #[test]
    fn capture_windows_are_inclusive_and_overlap_symmetrically() {
        let bounds = CaptureWindowBounds::new(
            Duration::from_secs(2),
            Duration::from_secs(10),
            Duration::from_secs(0),
        );
        assert_eq!(
            bounds.grace,
            Duration::from_secs(2),
            "a grace below `before` is raised, never accepted"
        );
        let window = CaptureWindow::around(1_000, bounds);
        assert_eq!(
            window,
            CaptureWindow {
                start: 998,
                end: 1010
            }
        );
        assert!(window.contains(998));
        assert!(window.contains(1010));
        assert!(!window.contains(997));
        assert!(!window.contains(1011));
        assert_eq!(bounds.horizon(1_000), 1_012);

        let later = CaptureWindow::around(1_011, bounds);
        assert!(window.overlaps(&later), "1009..1021 still touches ..1010");
        assert!(later.overlaps(&window), "overlap is symmetric");
        let much_later = CaptureWindow::around(1_100, bounds);
        assert!(!window.overlaps(&much_later));

        // The property `grace >= before` exists to buy exactly this: no
        // session whose first input lands after this one's horizon can
        // overlap it, so a horizon that has closed can never be reopened.
        let past_horizon = CaptureWindow::around(bounds.horizon(1_000) + 1, bounds);
        assert!(!window.overlaps(&past_horizon));
    }

    fn candidate(conversation: &str, created_at: i64) -> Candidate {
        Candidate {
            correlators: RecordCorrelators {
                conversation: conversation.to_string(),
                cwd: "/work".to_string(),
                created_at,
            },
            path: PathBuf::from(format!("/records/{conversation}.jsonl")),
            stamp: RecordStamp {
                len: 0,
                mtime_unix: None,
            },
        }
    }

    /// The core no-guessing rule, in the one place it is a pure function.
    /// Two candidates inside one window is the shape a second agent
    /// running in the same directory produces, and claiming either would
    /// be exactly the silently-wrong-conversation resume SPEC.md forbids.
    /// The diagnostic has to NAME them, because the user is about to be
    /// offered a fresh launch and the log is the only place that says why.
    #[test]
    fn exactly_one_in_window_candidate_is_captured_and_two_bail() {
        let window = CaptureWindow {
            start: 100,
            end: 200,
        };
        assert_eq!(choose(&[], window), CaptureVerdict::NotYet);
        let one = candidate("solitary-one", 150);
        assert_eq!(
            choose(&[&one], window),
            CaptureVerdict::Captured("solitary-one".to_string())
        );
        let early = candidate("far-too-early", 50);
        let late = candidate("far-too-late", 250);
        assert_eq!(
            choose(&[&early, &late], window),
            CaptureVerdict::NotYet,
            "out-of-window candidates are not candidates"
        );
        let rival = candidate("rival-of-solitary", 160);
        match choose(&[&one, &rival], window) {
            CaptureVerdict::Ambiguous(why) => {
                assert!(
                    why.contains("solitary-one") && why.contains("rival-of-solitary"),
                    "names both: {why}"
                );
            }
            other => panic!("expected an ambiguity bail, got {other:?}"),
        }
    }

    /// The diagnostic is bounded: a directory with many in-window records
    /// must produce a log line a human can read, not a thousand ids. The
    /// COUNT stays exact, because that is the part that says how bad the
    /// collision was.
    #[test]
    fn the_ambiguity_diagnostic_is_bounded_but_still_counts_everything() {
        let window = CaptureWindow {
            start: 100,
            end: 200,
        };
        let owned: Vec<Candidate> = (0..12).map(|i| candidate(&format!("c{i}"), 150)).collect();
        let refs: Vec<&Candidate> = owned.iter().collect();
        match choose(&refs, window) {
            CaptureVerdict::Ambiguous(why) => {
                assert!(why.contains("12 conversation records"), "{why}");
                assert!(why.contains("and 8 more"), "{why}");
                assert!(
                    !why.contains("c11"),
                    "the tail is elided, not listed: {why}"
                );
            }
            other => panic!("expected an ambiguity bail, got {other:?}"),
        }
    }
    /// Timestamps decide window membership, so a parser that NORMALIZED a
    /// nonsense instant (February 31st becoming March 3rd, hour 25
    /// becoming the next day) would place a record in a window it never
    /// belonged to — the wrong-conversation failure reached through the
    /// back door. Table-driven because the failure modes are many and each
    /// one is a different way of being lenient.
    #[test]
    fn rfc3339_parsing_rejects_everything_that_is_not_an_instant() {
        for bad in [
            "",
            "nope",
            "2026-07-29T12:00",
            "2026-13-29T12:00:00Z",
            "2026-00-29T12:00:00Z",
            "2026-02-31T12:00:00Z",
            "2025-02-29T12:00:00Z",
            "2026-07-00T12:00:00Z",
            "2026-07-32T12:00:00Z",
            "2026-07-29T24:00:00Z",
            "2026-07-29T25:99:99Z",
            "2026-07-29T12:60:00Z",
            "2026-07-29T12:00:60Z",
            "2026-07-29T12:00:00+99:99",
            "2026-07-29T12:00:00+24:00",
            "2026-07-29T12:00:00+01",
            "2026-07-29T12:00:00+0100",
            "2026-07-29T12:00:00.Z",
            "2026-07-29T12:00:00.123",
            "2026-07-29T12:00:00",
            "2026-07-29T12:00:00Zjunk",
            "2026-07-29T12:00:00Z ",
            "2026-07-29X12:00:00Z",
            "2026-07-29T+2:00:00Z",
        ] {
            assert_eq!(parse_rfc3339(bad), None, "{bad:?} must not parse");
        }

        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2026-07-29T12:00:05Z"), Some(1_785_326_405));
        assert_eq!(
            parse_rfc3339("2026-07-29T12:00:05.987654Z"),
            Some(1_785_326_405),
            "fractional seconds truncate rather than round"
        );
        assert_eq!(
            parse_rfc3339("2026-07-29T14:00:05+02:00"),
            Some(1_785_326_405),
            "a positive offset names an earlier UTC moment"
        );
        assert_eq!(
            parse_rfc3339("2026-07-29T10:00:05-02:00"),
            Some(1_785_326_405)
        );
        assert_eq!(parse_rfc3339("2026-07-29 12:00:05Z"), Some(1_785_326_405));
        assert!(
            parse_rfc3339("2024-02-29T00:00:00Z").is_some(),
            "a real leap day must still parse"
        );

        for unix in [0, 1_785_326_405, 951_782_400, 4_102_444_800] {
            assert_eq!(
                parse_rfc3339(&format_rfc3339(unix)),
                Some(unix),
                "formatting and parsing must be exact inverses at {unix}"
            );
        }
    }

    /// The stamp is what gates re-verification, so its change predicate is
    /// a contract rather than a derived `PartialEq`: an unknown mtime must
    /// read as "changed" (otherwise a filesystem reporting none would
    /// freeze re-verification forever), and the same-second same-length
    /// blind spot must be a documented no-op rather than an accident.
    #[test]
    fn the_record_stamp_change_predicate_is_conservative_about_unknowns() {
        let a = RecordStamp {
            len: 10,
            mtime_unix: Some(100),
        };
        assert!(!a.differs(&a));
        assert!(a.differs(&RecordStamp {
            len: 11,
            mtime_unix: Some(100)
        }));
        assert!(a.differs(&RecordStamp {
            len: 10,
            mtime_unix: Some(101)
        }));
        let unknown = RecordStamp {
            len: 10,
            mtime_unix: None,
        };
        assert!(unknown.differs(&unknown), "unknown never means unchanged");
        assert!(unknown.differs(&a));
        assert!(a.differs(&unknown));
    }

    /// Two working directories that munge identically share one Claude
    /// project directory, so a scan that trusted the directory would hand
    /// one session the other's conversation. The scan itself deliberately
    /// does NOT filter by cwd (Codex's root is shared by every session on
    /// the host), so this pins that both records come back carrying the
    /// distinguishing field for the caller to bucket on.
    #[tokio::test]
    async fn a_munged_cwd_collision_is_separated_by_the_recorded_cwd_field() {
        let home = tempfile::tempdir().unwrap();
        let dir = home
            .path()
            .join(".claude")
            .join("projects")
            .join("-tmp-a-b");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let stamp = format_rfc3339(now_unix());
        tokio::fs::write(
            dir.join("one.jsonl"),
            format!(
                "{{\"sessionId\":\"dotted\",\"cwd\":\"/tmp/a.b\",\"timestamp\":\"{stamp}\"}}\n"
            ),
        )
        .await
        .unwrap();
        tokio::fs::write(
            dir.join("two.jsonl"),
            format!(
                "{{\"sessionId\":\"dashed\",\"cwd\":\"/tmp/a-b\",\"timestamp\":\"{stamp}\"}}\n"
            ),
        )
        .await
        .unwrap();

        for cwd in ["/tmp/a.b", "/tmp/a-b"] {
            let root = ClaudeIntegration.record_root(home.path(), cwd);
            let outcome = scan_records(&ClaudeIntegration, &root, 0).await;
            assert!(outcome.complete);
            assert_eq!(
                outcome.candidates.len(),
                2,
                "both cwds resolve to one shared directory ({cwd})"
            );
            let mut found: Vec<(String, String)> = outcome
                .candidates
                .iter()
                .map(|c| {
                    (
                        c.correlators.cwd.clone(),
                        c.correlators.conversation.clone(),
                    )
                })
                .collect();
            found.sort();
            assert_eq!(
                found,
                vec![
                    ("/tmp/a-b".to_string(), "dashed".to_string()),
                    ("/tmp/a.b".to_string(), "dotted".to_string()),
                ]
            );
        }
    }

    /// Codex's rollout files are nested under a date hierarchy, so the
    /// walk has to descend — but only as far as the integration declares,
    /// or a stray deep tree turns a rescan into a filesystem crawl. A
    /// missing root must also be silent AND complete: an agent that has
    /// never run on this host is the ordinary case on a fresh machine, and
    /// reporting it incomplete would block capture for every session
    /// forever.
    #[tokio::test]
    async fn codex_descends_exactly_its_declared_depth_and_a_missing_root_is_complete() {
        let home = tempfile::tempdir().unwrap();
        let root = CodexIntegration.record_root(home.path(), "/work");
        let missing = scan_records(&CodexIntegration, &root, 0).await;
        assert!(missing.candidates.is_empty());
        assert!(
            missing.complete,
            "a missing root is authoritative emptiness"
        );

        let line = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                    \"payload\":{\"id\":\"roll-1\",\"cwd\":\"/work\"}}\n";
        let at_depth = root.join("2026").join("07").join("29");
        tokio::fs::create_dir_all(&at_depth).await.unwrap();
        tokio::fs::write(at_depth.join("rollout-a.jsonl"), line)
            .await
            .unwrap();
        // One level deeper than declared: the directory is simply not
        // descended into, so its record is never seen.
        let too_deep = at_depth.join("extra");
        tokio::fs::create_dir_all(&too_deep).await.unwrap();
        tokio::fs::write(too_deep.join("rollout-b.jsonl"), line)
            .await
            .unwrap();

        let outcome = scan_records(&CodexIntegration, &root, 0).await;
        assert!(outcome.complete);
        assert_eq!(outcome.candidates.len(), 1);
        assert_eq!(outcome.candidates[0].correlators.conversation, "roll-1");
    }

    /// The mtime lower bound is what bounds a rescan's cost, and it is
    /// only sound because records are append-only (mtime is at least
    /// creation time). Pinned by making the skipped file UNPARSEABLE: if
    /// the filter ever stopped applying before the open, this test fails
    /// with an incomplete scan rather than passing quietly.
    #[tokio::test]
    async fn the_mtime_floor_skips_files_without_ever_parsing_them() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".claude").join("projects").join("-work");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("old.jsonl"), "this is not JSON at all\n")
            .await
            .unwrap();

        let above_everything = now_unix() + 3_600;
        let skipped = scan_records(&ClaudeIntegration, &dir, above_everything).await;
        assert!(skipped.candidates.is_empty());
        assert!(
            skipped.complete,
            "a file the floor excluded must never have been opened, let alone parsed"
        );

        let read = scan_records(&ClaudeIntegration, &dir, 0).await;
        assert!(
            !read.complete,
            "the same file, once actually read, fails to parse and marks the scan incomplete"
        );
    }

    /// A FIFO wearing a record's name must never park a request handler on
    /// an open that would not return — the reason [`read_prefix`] passes
    /// `O_NONBLOCK` — and the enumeration must recognize it for what it is
    /// rather than treating it as evidence it could not read.
    ///
    /// Both halves are asserted, because they defend different moments.
    /// Enumeration sees the FIFO's type from `readdir` and SKIPS it: that
    /// is a positive decision ("this is not a record"), so the scan stays
    /// COMPLETE and the real record beside it is still reported.
    /// [`read_prefix`] is then exercised directly on the FIFO, which is
    /// exactly the post-TOCTOU state — a regular file replaced between
    /// `readdir` and `open` — and must fail rather than hang.
    #[tokio::test]
    async fn a_record_named_fifo_is_recognized_and_never_parks_the_reader() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".claude").join("projects").join("-work");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let stamp = format_rfc3339(now_unix());
        tokio::fs::write(
            dir.join("good.jsonl"),
            format!("{{\"sessionId\":\"good\",\"cwd\":\"/work\",\"timestamp\":\"{stamp}\"}}\n"),
        )
        .await
        .unwrap();

        let fifo = dir.join("trap.jsonl");
        let path = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
        // SAFETY: a plain libc call with a valid NUL-terminated path; the
        // return value is checked and the FIFO lives in this test's own
        // tempdir.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

        let outcome = tokio::time::timeout(
            Duration::from_secs(10),
            scan_records(&ClaudeIntegration, &dir, 0),
        )
        .await
        .expect("a FIFO named like a record must never park the scan");
        assert!(
            outcome.complete,
            "recognizing a FIFO is a decision, not a failure to read"
        );
        assert_eq!(
            outcome.candidates.len(),
            1,
            "the real record beside it is still found"
        );

        let direct = tokio::time::timeout(Duration::from_secs(10), read_prefix(&fifo))
            .await
            .expect("O_NONBLOCK is what keeps this open from never returning");
        assert!(
            direct.is_err(),
            "a non-regular file reached through the open must be refused, not read"
        );
    }

    /// A symlink is never followed, at either level: a link placed where a
    /// record should be could point anywhere the supervisor's user can
    /// read, and a scan that chased one would read files outside the home
    /// it was given.
    #[tokio::test]
    async fn symlinked_records_are_skipped_without_failing_the_scan() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let stamp = format_rfc3339(now_unix());
        let line = format!(
            "{{\"sessionId\":\"elsewhere\",\"cwd\":\"/work\",\"timestamp\":\"{stamp}\"}}\n"
        );
        tokio::fs::write(elsewhere.path().join("secret.jsonl"), &line)
            .await
            .unwrap();

        let dir = home.path().join(".claude").join("projects").join("-work");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        std::os::unix::fs::symlink(
            elsewhere.path().join("secret.jsonl"),
            dir.join("link.jsonl"),
        )
        .unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), dir.join("linked-dir")).unwrap();

        let outcome = scan_records(&ClaudeIntegration, &dir, 0).await;
        assert!(
            outcome.candidates.is_empty(),
            "a symlinked record is skipped"
        );
        assert!(
            outcome.complete,
            "skipping a symlink is a decision, not a failure to read"
        );
    }

    /// The 64 KiB / 64-line prefix is a real boundary, so both sides of it
    /// are pinned: correlators just inside are found, and correlators
    /// pushed just outside make the file unparseable (an error, marking
    /// the scan incomplete) rather than silently absent.
    #[tokio::test]
    async fn the_record_prefix_bounds_are_exact() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".claude").join("projects").join("-work");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let stamp = format_rfc3339(now_unix());
        let correlators =
            format!("{{\"sessionId\":\"deep\",\"cwd\":\"/work\",\"timestamp\":\"{stamp}\"}}\n");
        let filler = "{\"type\":\"filler\"}\n";

        // Line 64 — the last one examined — still counts.
        tokio::fs::write(
            dir.join("inside.jsonl"),
            filler.repeat(RECORD_PREFIX_LINES - 1) + &correlators,
        )
        .await
        .unwrap();
        let outcome = scan_records(&ClaudeIntegration, &dir, 0).await;
        assert!(outcome.complete);
        assert_eq!(outcome.candidates.len(), 1);

        // Line 65 does not.
        tokio::fs::remove_file(dir.join("inside.jsonl"))
            .await
            .unwrap();
        tokio::fs::write(
            dir.join("outside.jsonl"),
            filler.repeat(RECORD_PREFIX_LINES) + &correlators,
        )
        .await
        .unwrap();
        assert!(!scan_records(&ClaudeIntegration, &dir, 0).await.complete);

        // And the byte bound, independent of the line bound: one huge
        // first line pushes the correlators past 64 KiB.
        tokio::fs::remove_file(dir.join("outside.jsonl"))
            .await
            .unwrap();
        tokio::fs::write(
            dir.join("huge.jsonl"),
            format!(
                "{{\"pad\":\"{}\"}}\n{correlators}",
                "x".repeat(RECORD_PREFIX_BYTES)
            ),
        )
        .await
        .unwrap();
        assert!(!scan_records(&ClaudeIntegration, &dir, 0).await.complete);
    }

    /// The entry budget counts EVERY visited entry, not just the ones that
    /// look like records — a directory holding a hundred thousand
    /// unrelated files costs a hundred thousand `readdir` results whether
    /// or not any of them is a `.jsonl`. Hitting it must report
    /// incompleteness, which is what stops a partial view from producing a
    /// claim.
    #[tokio::test]
    async fn the_entry_budget_counts_non_record_files_and_reports_incompleteness() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".claude").join("projects").join("-work");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        for i in 0..(MAX_VISITED_ENTRIES + 16) {
            tokio::fs::write(dir.join(format!("noise-{i}.txt")), "")
                .await
                .unwrap();
        }
        assert!(
            !scan_records(&ClaudeIntegration, &dir, 0).await.complete,
            "a scan that ran out of entry budget saw only part of the directory"
        );
    }
}
