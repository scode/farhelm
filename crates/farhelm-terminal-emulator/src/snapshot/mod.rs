//! Snapshots: an emulator's whole state as a blob, and an emulator rebuilt
//! from one.
//!
//! A terminal host outlives supervisors. It keeps the latest snapshot blob a
//! supervisor gave it plus the raw output since, and a restarted supervisor
//! rebuilds the terminal's emulator from the two: [`Emulator::restore`] on
//! the blob, then the output fed as usual. The host drops raw output once it
//! accepts a snapshot, so there is no falling back to replaying everything.
//!
//! # Compatibility
//!
//! **Every snapshot format ever shipped stays readable by every later
//! supervisor, forever.** A host started by one release keeps that release's
//! blob for as long as it runs, across any number of updates, so:
//!
//! - A blob starts with the magic bytes `FHTS` and a format version.
//!   [`Emulator::snapshot`] writes the newest version ([`FORMAT_VERSION`]);
//!   [`Emulator::restore`] reads every version this module has ever had.
//! - A version, once shipped, never changes: not its layout, not what its
//!   fields mean, not how its stream is read. A change of format is a new
//!   version in a module of its own beside the old ones, whose decoders stay.
//! - Each version has golden fixtures (`tests/fixtures/snapshots/v<N>/`):
//!   blobs from scripted terminal states, checked to restore to the state
//!   recorded beside them, and checked to be what the encoder of that version
//!   still writes. They are never regenerated, only added to.
//! - A blob of a version this build does not know (a newer supervisor's,
//!   after a downgrade), or one that does not decode, is a
//!   [`SnapshotError`]. The supervisor then treats the terminal as
//!   unavailable; it never guesses at a state.
//!
//! Version 1's layout and reading rules are in the `v1` module.
//!
//! # When a snapshot can be taken
//!
//! Only at a clean boundary ([`Emulator::is_clean`]): elsewhere a fresh
//! emulator fed the snapshot and then the rest of the output would misread
//! the sequence in progress. [`Emulator::snapshot`] refuses with [`NotClean`]
//! anywhere else, and the caller keeps its previous snapshot until it finds a
//! clean boundary: it feeds a read up to [`crate::snapshot_split_candidate`]
//! and tries there, or tries at the read's end, the emulator itself deciding
//! whether the point is clean. The Restart seed ([`Emulator::history_seed`])
//! has no such rule: it keeps nothing of the parser.
//!
//! # Size
//!
//! The host refuses a blob over its `MAX_SNAPSHOT` (16 MiB), and a snapshot
//! must never fail because of size, so the caller passes that limit and the
//! encoder makes the blob fit, giving up what is worth least to the user
//! first:
//!
//! 1. The oldest entries of the title stack (titles a program saved to put
//!    back later), as many as it must.
//! 2. With no titles left, the oldest history lines, as few as it must. The
//!    history is then marked trimmed: [`Emulator::history_trimmed`] says so
//!    after a restore, so that the viewer can tell the user history was
//!    trimmed, until as many new lines as the history held have entered it
//!    (the blob carries how many are left, across any number of restores) or
//!    the program clears it.
//! 3. With no history left either, the screens' contents, keeping as many
//!    titles as then fit, marked ([`Restored::screens_dropped`]): the
//!    supervisor then asks the program to redraw, as after a gap in the
//!    host's journal.
//!
//! Real terminals stay far below the limit: 12,000 lines of 200 columns of
//! ordinary text take about 3.6 MiB. Step 1 needs a title stack of hundreds
//! of long titles, step 2 heavily styled or linked history, step 3 a screen
//! of a million styled cells. Every step keeps the blob at most `max_bytes`
//! when `max_bytes` is at least [`MIN_SNAPSHOT_LIMIT`].
//!
//! Separately, a restore allocates at most [`MAX_RESTORE_CELLS`] cells: it
//! keeps no more history than fits beside the two screens, and refuses a
//! blob whose screens alone need more, before allocating anything. The
//! encoder drops the history a restore could not hold, so that its blobs
//! restore whole (only a very wide terminal at full scrollback has any:
//! past about 1,380 columns at 12,000 lines).
//!
//! # Character widths
//!
//! The stream places characters by printing them, so where each lands
//! depends on the width table of the emulator reading it, which comes with
//! the `unicode-width` crate and changes when it is upgraded (new emoji
//! become wide, say). A blob records the table it was painted with
//! ([`WidthTable`]): the Unicode version the crate names it by, and a
//! fingerprint of the width it gives every character, which is what tells
//! two tables apart (the crate has changed widths without changing the
//! version it names). A reader whose table differs restores the blob anyway
//! and says so ([`Restored::width_table_changed`]): refusing would make every
//! running terminal unavailable at the first supervisor update after the
//! dependency changes.
//!
//! What such a restore loses is confined to the characters whose width
//! changed. The stream positions the cell after every character that is not
//! ASCII by its column (CHA), not by where the character left the cursor,
//! so nothing else on the row moves. A character now narrower leaves a
//! blank cell after it. One now wider is usually lost: the next cell is
//! placed on its second half, and printing there clears the wide character
//! (alacritty's cleanup of an overwritten half), so the row shows the next
//! cell and a blank where the character was; only when the next cell is
//! blank, and so not painted, does the wider character stay and cover it.
//! A character that became wide in a row's last column wraps to the next
//! row instead; painted on the bottom row, as all history is,
//! that scrolls one more line into history, and the history above it, with
//! its fixups, ends up one line off. A redraw, which the supervisor asks the
//! program for, repairs the screen; it does not repair history, which keeps
//! these differences until it scrolls away.
//!
//! # Hyperlinks
//!
//! A link the program sent without an `id` gets one generated by alacritty
//! (`<n>_alacritty`, from a counter in the process), and the stream restores
//! it with that `id`. A link sent after the restore may be given the same
//! generated `id` by the new process's counter. Links are told apart by `id`
//! and URI together, so this only matters for two such links with the same
//! URI, which a viewer would then highlight together on hover.
//!
//! # Extra storage
//!
//! Restoring makes cell extras (combining characters, underline colors,
//! hyperlinks) through the restored terminal's extra storage, within the
//! same limit as the program's own output (vendor/README.md, "Caps on
//! hostile output"): the stream through the cursor template like any
//! output, and fixup cells through the storage's own setters. Extras of the
//! same content are shared, the stream's and the fixups' alike, as cells
//! printed from one template share theirs, and the stream sets an
//! attribute only when it changes, so that painting cells in its own order
//! does not copy an extra the program's cells shared. A restore therefore
//! never takes the storage past its limit, and charges at most about what
//! the original did, often less; a terminal whose storage was full when the
//! snapshot was taken may restore with a few combining marks, underline
//! colors or links missing, where a change the restore makes in another
//! order than the program found the storage full.

mod v1;

use std::fmt;
use std::sync::OnceLock;
use std::time::Instant;

use alacritty_terminal::grid::Dimensions;
use unicode_width::UnicodeWidthChar;

use crate::effects::{Collected, FeedMode};
use crate::emulator::{Emulator, Size};

/// The snapshot format version [`Emulator::snapshot`] writes.
pub const FORMAT_VERSION: u16 = v1::VERSION;

/// The bytes every snapshot blob starts with.
const MAGIC: [u8; 4] = *b"FHTS";

/// The smallest `max_bytes` for which a snapshot is guaranteed to fit: room
/// for everything but grids and title stack in the largest terminal the host
/// allows (a tab stop on every one of 10,000 columns takes about 50 KB; the
/// palette, title, cursors and modes a few more).
pub const MIN_SNAPSHOT_LIMIT: usize = 256 * 1024;

/// The widest and tallest terminal a blob may describe: the terminal host's
/// own limit on a pty's size (`crates/terminal-host/SPEC.md`).
pub const MAX_SIDE: u16 = 10_000;

/// The most cells a restore allocates: both screens plus the history it
/// keeps, with the rows alacritty allocates ahead of a growing history
/// (see `restorable_history`), about 384 MiB of grid. So that a few forged
/// bytes cannot make the supervisor allocate gigabytes, a restore keeps no
/// more history than fits, and refuses a blob whose screens alone need more
/// ([`SnapshotError::TooLarge`]) before allocating anything. The encoder
/// drops the oldest history of a terminal that would need more, so its own
/// blobs restore whole, unless the screens alone exceed it (a terminal over
/// about 2,900 by 2,900), which no supervisor-side emulator holds within
/// its memory budget anyway.
///
/// This is the build's policy, not a rule of any format: a later build may
/// change it, and restores an old blob with as much history as its own
/// bound allows.
pub const MAX_RESTORE_CELLS: u64 = 1 << 24;

/// A snapshot blob, and what was left out of it to fit the size limit.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The blob, for the terminal host.
    pub blob: Vec<u8>,
    /// How many of the oldest history lines were left out.
    pub dropped_history_lines: usize,
    /// Whether the screens' contents were left out (see the module docs,
    /// "Size").
    pub dropped_screens: bool,
    /// How many of the oldest title stack entries were left out.
    pub dropped_titles: usize,
}

/// An emulator restored from a snapshot, and what the snapshot could not
/// carry.
pub struct Restored {
    /// The emulator, in the snapshot's state. Its
    /// [`Emulator::history_trimmed`] says whether history was lost to the
    /// size limit, in this snapshot or an earlier one.
    pub emulator: Emulator,
    /// The snapshot left out the screens' contents to fit the size limit:
    /// both screens are blank, and the program should be asked to redraw.
    pub screens_dropped: bool,
    /// The character width table the snapshot was painted with, when it
    /// gives some character a different width from this build's: the
    /// program should be asked to redraw, and history may be off where such
    /// characters are (module docs, "Character widths").
    pub width_table_changed: Option<WidthTable>,
}

/// Which character widths a stream was painted with: the Unicode version
/// `unicode-width` names its table by, for people, and a fingerprint of the
/// width it gives every character, which is what tells tables apart (the
/// crate has changed widths without changing the version it names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidthTable {
    /// `unicode_width::UNICODE_VERSION`: major, minor, update.
    pub unicode_version: (u8, u8, u8),
    /// FNV-1a (64 bits) over the width of every Unicode scalar value in
    /// order, one byte each (0, 1 or 2, or 255 for a control character).
    pub fingerprint: u64,
}

impl WidthTable {
    /// This build's table, worked out once per process (a few milliseconds).
    pub fn current() -> WidthTable {
        static CURRENT: OnceLock<WidthTable> = OnceLock::new();
        *CURRENT.get_or_init(|| {
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            for c in (0..=0x10_FFFF).filter_map(char::from_u32) {
                let width = c.width().map_or(255, |width| width as u8);
                hash = (hash ^ u64::from(width)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            WidthTable {
                unicode_version: unicode_width::UNICODE_VERSION,
                fingerprint: hash,
            }
        })
    }
}

/// Why a blob could not be restored. The supervisor shows the terminal as
/// unavailable rather than guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    /// Not a snapshot: too short for the header, or without the magic bytes.
    NotASnapshot,
    /// A snapshot of a format version this build does not know: a newer
    /// supervisor's, after a downgrade.
    UnknownVersion(u16),
    /// A snapshot whose two screens alone would hold more than
    /// [`MAX_RESTORE_CELLS`] cells; nothing was allocated.
    TooLarge {
        /// The cells the screens would need.
        cells: u64,
    },
    /// A snapshot whose stream would take more work to replay than version
    /// 1 allows for its size: only a forged or corrupt blob does. Nothing was
    /// replayed.
    TooMuchWork {
        /// The most work the stream was allowed, in the units the format's
        /// reading rules count.
        limit: u64,
    },
    /// A snapshot of a known version that does not decode: what was wrong.
    Malformed(String),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotError::NotASnapshot => write!(f, "not a terminal snapshot"),
            SnapshotError::UnknownVersion(version) => {
                write!(
                    f,
                    "terminal snapshot format {version} is newer than this build"
                )
            }
            SnapshotError::TooLarge { cells } => write!(
                f,
                "terminal snapshot's screens need {cells} cells, more than {MAX_RESTORE_CELLS}"
            ),
            SnapshotError::TooMuchWork { limit } => write!(
                f,
                "terminal snapshot's stream would take more than {limit} units of work to replay"
            ),
            SnapshotError::Malformed(what) => write!(f, "malformed terminal snapshot: {what}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// [`Emulator::snapshot`] was asked for a snapshot away from a clean
/// boundary. Nothing was encoded; take one at the next clean boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotClean;

impl fmt::Display for NotClean {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the terminal is not at a clean boundary")
    }
}

impl std::error::Error for NotClean {}

impl Emulator {
    /// This emulator's state as a snapshot blob of at most `max_bytes` (the
    /// host's `MAX_SNAPSHOT`), or [`NotClean`] away from a clean boundary.
    ///
    /// Everything a program or a viewer can observe is in it: both screens,
    /// the primary one's history, cursors and saved cursors, attributes,
    /// hyperlinks, modes, scroll region, tab stops, character sets, colors set
    /// by the program, cursor style, title and title stack, and the character
    /// REP repeats. What is not: the synchronized update (there is none at a
    /// clean boundary), the side effects already delivered, and the
    /// emulator's configuration, which is fixed. To fit `max_bytes` it leaves
    /// out the oldest title stack entries first, then the oldest history
    /// (module docs, "Size").
    pub fn snapshot(&self, max_bytes: usize) -> Result<Snapshot, NotClean> {
        if !self.is_clean() {
            return Err(NotClean);
        }
        let history = self.primary_grid().history_size();
        let titles = title_costs(self);
        let all_titles = titles.len();
        // History a restore could not allocate goes first, whatever the size.
        let min_skip = history.saturating_sub(self.restorable_history());

        // Step 1: the oldest titles, keeping all the history that may stay.
        let screens = v1::Content::Cells {
            skip_history: min_skip,
        };
        let trimmed_for = self.trimmed_for(min_skip > 0);
        let with_titles =
            |skip_titles, mode| v1::encode(self, screens, skip_titles, trimmed_for, mode);
        if let Some((blob, dropped_titles)) = fit_titles(&titles, max_bytes, with_titles) {
            return Ok(Snapshot {
                blob,
                dropped_history_lines: min_skip,
                dropped_screens: false,
                dropped_titles,
            });
        }

        // Step 2: no titles, and the oldest history.
        let with_history = |skip, mode| {
            let content = v1::Content::Cells { skip_history: skip };
            v1::encode(self, content, all_titles, self.trimmed_for(skip > 0), mode)
        };
        if let Some(mut snapshot) = fit_history(min_skip, history, max_bytes, with_history) {
            snapshot.dropped_titles = all_titles;
            return Ok(snapshot);
        }

        // Step 3: no cells, and as many titles as fit.
        let nothing = v1::Content::Nothing;
        let trimmed_for = self.trimmed_for(history > 0);
        let without_screens =
            |skip_titles, mode| v1::encode(self, nothing, skip_titles, trimmed_for, mode);
        // Without screens or titles, what is left fits `MIN_SNAPSHOT_LIMIT`;
        // below that it is returned even if it does not fit.
        let (blob, dropped_titles) = fit_titles(&titles, max_bytes, without_screens)
            .unwrap_or_else(|| (without_screens(all_titles, v1::Mode::KEEP).blob, all_titles));
        Ok(Snapshot {
            blob,
            dropped_history_lines: history,
            dropped_screens: true,
            dropped_titles,
        })
    }

    /// The seed for Restart: a snapshot of this emulator's history alone
    /// (the primary screen's, whichever screen shows), above a blank screen
    /// of the same size, with default modes, colors, cursor and title. Fits
    /// `max_bytes` by leaving out the oldest history.
    ///
    /// A restart runs a new program in a new host, which keeps the old
    /// terminal's scrollback but not its last frame (SPEC.md, "Lifecycle
    /// operations"); the new host starts from this blob as its initial
    /// snapshot. It works at any boundary, since it keeps nothing of the
    /// parser.
    pub fn history_seed(&self, max_bytes: usize) -> Snapshot {
        let history = self.primary_grid().history_size();
        let min_skip = history.saturating_sub(self.restorable_history());
        let with_history =
            |skip, mode| v1::encode_history(self, skip, self.trimmed_for(skip > 0), mode);
        // Without history a seed is a few dozen bytes, which only a
        // `max_bytes` far below `MIN_SNAPSHOT_LIMIT` refuses; it is returned
        // even then.
        fit_history(min_skip, history, max_bytes, with_history).unwrap_or_else(|| Snapshot {
            blob: with_history(history, v1::Mode::KEEP).blob,
            dropped_history_lines: history,
            dropped_screens: false,
            dropped_titles: 0,
        })
    }

    /// A fresh emulator in the state `blob` holds, keeping up to
    /// `scrollback_lines` lines of history (the same as [`Emulator::new`]'s),
    /// or why the blob cannot be restored.
    ///
    /// Restoring has no side effects: nothing is answered or reported for
    /// the sequences that rebuild the state.
    pub fn restore(blob: &[u8], scrollback_lines: usize) -> Result<Restored, SnapshotError> {
        if blob.len() < MAGIC.len() + 2 || blob[..MAGIC.len()] != MAGIC {
            return Err(SnapshotError::NotASnapshot);
        }
        let version = u16::from_be_bytes([blob[MAGIC.len()], blob[MAGIC.len() + 1]]);
        let body = &blob[MAGIC.len() + 2..];
        match version {
            v1::VERSION => v1::decode(body, scrollback_lines),
            other => Err(SnapshotError::UnknownVersion(other)),
        }
    }

    /// How many history lines a restore of this emulator's size may hold
    /// within [`MAX_RESTORE_CELLS`] ([`restorable_history`]).
    fn restorable_history(&self) -> usize {
        let size = self.size();
        restorable_history(size.columns, size.lines)
    }

    /// What a snapshot records as how long its history stays trimmed: what is
    /// left of an earlier trim, or, when this snapshot drops history itself,
    /// a whole scrollback's worth of lines (the dropped lines would have
    /// left history after that many anyway). 0 when not trimmed.
    fn trimmed_for(&self, dropping: bool) -> u32 {
        let left = self.trim_remaining().unwrap_or(0);
        let left = if dropping {
            left.max(self.scrollback_lines as u64).max(1)
        } else {
            left
        };
        u32::try_from(left).unwrap_or(u32::MAX)
    }
}

/// How many rows alacritty adds to a grid's storage at a time when its
/// history grows (`MAX_CACHE_SIZE` in the vendored `grid/storage.rs`): the
/// first line of history allocates a thousand rows, and so on.
const HISTORY_STEP: u64 = 1000;

/// How many lines of history a restore of a terminal of `columns` by `lines`
/// may keep, so that both screens and the history, with the rows alacritty
/// allocates ahead of it, stay within [`MAX_RESTORE_CELLS`]: whole steps of
/// [`HISTORY_STEP`] lines, since a history of one line already allocates a
/// whole step. A restore keeps no more than this, and the encoder writes no
/// more, so that this build's own blobs restore whole.
pub(crate) fn restorable_history(columns: u16, lines: u16) -> usize {
    let rows = MAX_RESTORE_CELLS / u64::from(columns.max(1));
    let history = rows.saturating_sub(2 * u64::from(lines));
    usize::try_from(history / HISTORY_STEP * HISTORY_STEP).unwrap_or(usize::MAX)
}

/// What each title stack entry, oldest first, adds to a snapshot: the title
/// and the sequences that set and push it (`OSC 2`, `CSI 22 t`), or just the
/// push for an entry without a title.
fn title_costs(emulator: &Emulator) -> Vec<usize> {
    emulator
        .term
        .title_stack()
        .iter()
        .map(|title| title.as_ref().map_or(0, |title| title.len() + 6) + 5)
        .collect()
}

/// A Restart seed built from plain history lines, as `tmux capture-pane -p -e`
/// prints them: one row of text each, with SGR sequences for its styles,
/// oldest first. For restarting an old (tmux) terminal into a terminal host:
/// the new host starts with the tmux pane's history.
///
/// The lines are printed into a scratch emulator of `size` (wrapping any
/// longer than it, as a resize would) and the seed is taken from its history
/// ([`Emulator::history_seed`]). Each line ends with its styles and any
/// hyperlink reset, so that one line's colors never bleed into the next.
/// When the lines, wrapped, are more than `scrollback_lines` hold, the oldest
/// are dropped and the seed's history is marked trimmed.
pub fn history_seed_from_lines<'a>(
    size: Size,
    scrollback_lines: usize,
    lines: impl IntoIterator<Item = &'a [u8]>,
    max_bytes: usize,
) -> Snapshot {
    let mut scratch = Emulator::new(size, scrollback_lines);
    let now = Instant::now();
    let mut ignored = Collected::default();
    let mut feed = |bytes: &[u8]| scratch.feed(bytes, now, FeedMode::ReplaySeen, &mut ignored);
    for line in lines {
        feed(line);
        feed(b"\x1b[m\x1b]8;;\x1b\\\r\n");
    }
    // Scroll every row above the cursor into history: the cursor sits on the
    // blank row after the last line.
    let cursor_line = scratch.term.grid().cursor.point.line.0;
    if cursor_line > 0 {
        let scroll = format!("\x1b[{cursor_line}S");
        scratch.feed(scroll.as_bytes(), now, FeedMode::ReplaySeen, &mut ignored);
    }
    let scrolled = scratch.primary_grid().lines_scrolled_into_history();
    if scrolled > scrollback_lines as u64 {
        scratch.mark_trimmed(scrollback_lines.max(1) as u64);
    }
    scratch.history_seed(max_bytes)
}

/// Steps 1 and 3 of the size ladder: `encode(skip, mode)` encodes, or
/// measures, with the `skip` oldest title stack entries left out, `costs`
/// saying what each entry adds. The blob with the fewest titles dropped that
/// fits `max_bytes`, and how many were; `None` if even dropping all of them
/// does not fit. Measured first, like [`fit_history`], so that a screen too
/// large to fit is never held whole.
fn fit_titles(
    costs: &[usize],
    max_bytes: usize,
    encode: impl Fn(usize, v1::Mode) -> v1::Encoded,
) -> Option<(Vec<u8>, usize)> {
    let mut skip = 0;
    loop {
        let measurement = encode(skip, v1::Mode::Measure);
        let measured = measurement.len;
        if measured <= max_bytes {
            return Some((encode(skip, measurement.keep()).blob, skip));
        }
        if skip >= costs.len() {
            return None;
        }
        // Leave out the fewest further entries whose sequences add up to the
        // excess; each costs exactly what `title_costs` says, so one more
        // measurement normally confirms it.
        let excess = measured - max_bytes;
        let mut saved = 0;
        let mut more = 0;
        for &cost in &costs[skip..] {
            if saved >= excess {
                break;
            }
            saved += cost;
            more += 1;
        }
        skip += more.max(1);
    }
}

/// The history step of the size ladder (module docs, "Size"):
/// `with_history(skip, mode)` encodes, or measures, with the `skip` oldest of
/// `history` lines left out, starting from `min_skip`. The snapshot leaving
/// out the fewest lines that fits `max_bytes`, or `None` if none does.
///
/// Sizes are found by measuring, which paints without keeping the stream,
/// and only a snapshot known to fit is encoded for real: a hostile state can
/// paint to hundreds of megabytes, which the supervisor must not hold even
/// briefly.
fn fit_history(
    min_skip: usize,
    history: usize,
    max_bytes: usize,
    with_history: impl Fn(usize, v1::Mode) -> v1::Encoded,
) -> Option<Snapshot> {
    let mut skip = min_skip.min(history);
    loop {
        let measured = with_history(skip, v1::Mode::Measure);
        let excess = measured.len.saturating_sub(max_bytes);
        if excess == 0 {
            let encoded = with_history(skip, measured.keep());
            debug_assert_eq!(encoded.blob.len(), measured.len);
            return Some(Snapshot {
                blob: encoded.blob,
                dropped_history_lines: skip,
                dropped_screens: false,
                dropped_titles: 0,
            });
        }
        if skip >= history {
            return None;
        }
        // Leave out the fewest further lines whose painting took at least
        // the excess. Painting from a later line can shift a few bytes (the
        // pen carried into it), so the result is measured again.
        let costs = &measured.history_costs;
        let start = costs.first().copied().unwrap_or(0);
        let more = costs
            .iter()
            .position(|&cost| cost - start >= excess)
            .unwrap_or(costs.len())
            .max(1);
        skip = (skip + more).min(history);
    }
}
