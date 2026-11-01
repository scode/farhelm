//! The comparison rig: one pinned tmux 3.7c pane and one emulator, fed the
//! same bytes, compared at cut points.
//!
//! # How bytes reach the pane unchanged
//!
//! The pane runs `/bin/sh`, which puts its terminal in raw mode without echo
//! and then copies a FIFO to the terminal and the terminal's input to a file.
//! Raw mode matters both ways: output processing would turn every `\n` into
//! `\r\n`, and echo would print tmux's answers to the program's queries onto
//! the screen being compared. The test writes the program's output into the
//! FIFO without blocking, under a deadline, and reports whether the pane is
//! still alive when a write or a wait does not finish; tmux's answers land in
//! the file, in the order tmux gave them.
//!
//! # Knowing that tmux has read everything
//!
//! tmux reads a pane's output whenever it gets to it, so a capture taken right
//! after a write could miss part of it. After each chunk the rig writes a
//! DECRQM for a private mode no terminal knows (60000 to 64999, a different
//! number from the previous chunk's), and waits until tmux's "not
//! recognized" answer to that exact number appears in the answer file. tmux
//! parses a pane's output in order and answers as it parses, so by then the
//! grid holds the whole chunk, and every answer before the marker's is an
//! answer to something in the chunk. The marker is never fed to the
//! emulator, and it is written only where both parsers are between sequences
//! (callers cut where the emulator is clean, or inside a synchronized update
//! after a complete sequence).
//!
//! The marker changes one piece of tmux's state: like every control sequence
//! it ends tmux's "a character was just printed" state, which REP (`CSI Ps
//! b`) needs to repeat anything. vte keeps the last printed character across
//! other sequences, so the two already differ for a REP after any sequence
//! (a pinned known difference); a cut right before a REP would add the
//! marker as such a sequence. Callers therefore never cut right before a REP:
//! generated output sends none outside the pinned case, and the recording
//! replay skips such offsets.
//!
//! # What is compared, and what is not
//!
//! At each cut: the screen text (`capture-pane -p` against
//! `Emulator::screen_text`), the cursor position (`#{cursor_x}` and
//! `#{cursor_y}`), the title (`#{pane_title}` against `Emulator::title`), and
//! the answers to queries in the chunk. These differences are documented
//! properties of the emulator rather than parity failures, and are applied
//! here instead of being rediscovered by every test (`Emulator::title`,
//! `farhelm_terminal_emulator`'s `queries` table):
//!
//! - Before a program sets a title, tmux reports the host name and the
//!   emulator nothing; vte trims whitespace around a title that tmux keeps.
//! - The secondary device attributes and the version report, where each
//!   names itself: tmux as tmux, the emulator as alacritty's terminal type
//!   and as Farhelm.
//! - A wide character that does not fit in a row's last column goes to the
//!   next row in both, but tmux leaves whatever that last cell held while
//!   alacritty blanks it. Such a row may end, in its last column exactly, in
//!   one more character in tmux.
//!
//! The cursor is compared the way tmux reports it: while a wrap is pending
//! (the program has just written the last column with autowrap on) tmux's
//! cursor is one past the last column, where alacritty keeps it on the last
//! column with a flag.
//!
//! Inside an open synchronized update (mode 2026) the two differ by design:
//! tmux applies the update's bytes as they arrive and only delays drawing to
//! its clients, so `capture-pane` shows the half-drawn frame, while vte holds
//! the bytes until the update ends and the emulator shows the frame before it.
//! The same holds for the cursor, titles and answers to queries inside the
//! update, which vte delivers when the update ends. A cut inside an update
//! therefore compares nothing and says so (`Cut::in_update`), and callers
//! count those cuts and require that the cut after every update is compared;
//! the answers tmux gave during the update are held and compared with the
//! emulator's at the first cut after it.
//!
//! The emulator is fed at one fixed instant, so vte's 150 ms safety timeout
//! never ends an update on its own: the comparison does not depend on how
//! fast the machine runs the test.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read as _, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use farhelm_proto::AgentKind;
use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::cell::{Cell, Flags};
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size};
use farhelm_teststate::process::{CommandRunLimits, run_bounded};
use farhelm_teststate::tmux::guard::TmuxServerGuard;

use crate::agent_kind::screen_reader::{
    SAMPLE_TAIL_BYTES, SampleCounts, Screen, ScreenState, reader_for,
};
use crate::tmux::retain_pane_tail;

/// The tmux every comparison runs against: the pinned release the supervisor
/// is tested on (`scripts/build-pinned-tmux-ci.sh`).
pub(super) const PINNED_TMUX: &str = "tmux 3.7c";

/// How long one tmux command, one write into the pane, or tmux's answer to
/// one marker may take before the comparison fails. Generous because the
/// machine running it is shared.
const TMUX_WAIT: Duration = Duration::from_secs(20);

/// The first marker mode number; see the module docs. Far above any mode a
/// terminal defines.
const FIRST_MARKER_MODE: u32 = 60000;

/// How many marker numbers are used before they repeat. Each marker's answer
/// is consumed before the next marker is written, so a number only has to
/// differ from the one before it; cycling keeps long recordings from running
/// out.
const MARKER_SPAN: u32 = 5000;

/// How often a write that cannot proceed checks whether the pane died.
const LIVENESS_EVERY: Duration = Duration::from_millis(200);

/// Whether the pinned tmux is first on `PATH`. When it is not, says so on
/// stderr as `SKIPPED`, as the repository's other tmux comparisons do, and the
/// caller returns without comparing anything.
pub(super) fn pinned_tmux_or_skip(test: &str) -> bool {
    let version = match Command::new("tmux").arg("-V").output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        Err(_) => {
            eprintln!("SKIPPED {test}: no tmux on PATH");
            return false;
        }
    };
    if version != PINNED_TMUX {
        eprintln!(
            "SKIPPED {test}: PATH's tmux is {version:?}, not {PINNED_TMUX} \
             (put scripts/build-pinned-tmux-ci.sh's directory first on PATH)"
        );
        return false;
    }
    true
}

/// The instant every emulator feed in these tests claims to happen at.
fn t0() -> Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    *T0.get_or_init(Instant::now)
}

/// Call `poll` every few milliseconds until it returns a value or `deadline`
/// passes.
fn poll_until<T>(deadline: Instant, mut poll: impl FnMut() -> Option<T>) -> Option<T> {
    loop {
        if let Some(value) = poll() {
            return Some(value);
        }
        if Instant::now() >= deadline {
            return None;
        }
        // sleep-ok: polling interval behind the marker-answer and writability oracles, which alone decide readiness.
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub(super) use crate::agent_kind::screen_fixtures::agent_kind;

/// What `kind`'s screen reader concludes from a screen's tail and title, the
/// way the sampler asks it: a run with history, the screen just changed.
/// Returns the state and whether recognized content anchored it.
pub(super) fn read(kind: AgentKind, tail: &str, title: &str) -> (ScreenState, bool) {
    let counts = SampleCounts {
        samples: 9,
        unchanged_streak: 0,
    };
    let reading = reader_for(kind).read(counts, &Screen { text: tail, title });
    (reading.state, reading.anchored)
}

/// A private tmux server with Farhelm's server configuration, on a socket in
/// a directory the test owns. Dropping it collects diagnostics and stops that
/// server, and only that one.
pub(super) struct TmuxServer {
    /// Declared before `dir` so the server stops before its directory goes.
    _guard: TmuxServerGuard,
    dir: farhelm_teststate::TestDir,
    socket: PathBuf,
    config: PathBuf,
    /// Panes started so far, for unique session and file names.
    panes: usize,
}

impl TmuxServer {
    /// Start nothing yet: tmux starts the server with the first pane, reading
    /// the configuration the supervisor writes for its own servers.
    pub(super) fn new() -> TmuxServer {
        let dir = farhelm_teststate::tempdir().expect("test directory");
        let socket = dir.path().join("s");
        let config = dir.path().join("tmux.conf");
        std::fs::write(&config, crate::tmux::TmuxDriver::config_body()).expect("tmux config");
        TmuxServer {
            _guard: TmuxServerGuard::new(socket.clone()),
            dir,
            socket,
            config,
            panes: 0,
        }
    }

    /// Run one tmux command against this server, bounded, and return its
    /// standard output. Panics with tmux's error output if it fails.
    pub(super) fn tmux(&self, args: &[&str]) -> String {
        let mut command = Command::new("tmux");
        command
            .arg("-S")
            .arg(&self.socket)
            .arg("-f")
            .arg(&self.config)
            .args(args);
        let limits =
            CommandRunLimits::new(TMUX_WAIT, Duration::from_secs(2), 1 << 20, 1 << 16, 1 << 20)
                .expect("limits");
        let outcome = run_bounded(&mut command, &limits).expect("limits");
        let ok = outcome.status.is_some_and(|status| status.success())
            && !outcome.timed_out
            && outcome.stdout.complete;
        assert!(
            ok,
            "tmux {args:?} failed: {outcome:?}, stderr {:?}",
            String::from_utf8_lossy(&outcome.stderr.prefix)
        );
        String::from_utf8(outcome.stdout.prefix).expect("tmux prints UTF-8")
    }

    /// A new pane of `size` in a session of its own, running the copying shell
    /// described in the module docs.
    pub(super) fn pane(&mut self, size: Size) -> Pane<'_> {
        self.panes += 1;
        let name = format!("p{}", self.panes);
        let fifo = self.dir.path().join(format!("{name}.out"));
        let answers = self.dir.path().join(format!("{name}.in"));
        let fifo_c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).expect("path");
        // SAFETY: a valid NUL-terminated path; mkfifo has no other preconditions.
        let made = unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) };
        assert_eq!(made, 0, "mkfifo: {}", std::io::Error::last_os_error());
        File::create(&answers).expect("answer file");
        // Opened read-write so that opening never blocks waiting for the
        // pane's reader, and the reader never sees end of file while the test
        // still has bytes to send; Linux defines this for a FIFO, POSIX
        // leaves it open, and these tests have only been run on Linux.
        // Nonblocking, so that a pane that stops reading turns into a bounded
        // wait with a report rather than a write that never returns.
        let writer = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo)
            .expect("open fifo");
        let reader = File::open(&answers).expect("open answer file");
        let script = "stty raw -echo || exit 1; cat \"$1\" & exec cat > \"$2\"";
        let columns = size.columns.to_string();
        let lines = size.lines.to_string();
        self.tmux(&[
            "new-session",
            "-d",
            "-s",
            &name,
            "-x",
            &columns,
            "-y",
            &lines,
            "/bin/sh",
            "-c",
            script,
            "sh",
            fifo.to_str().expect("UTF-8 path"),
            answers.to_str().expect("UTF-8 path"),
        ]);
        let target = format!("={name}:");
        let mut pane = Pane {
            server: self,
            target,
            size,
            writer,
            reader,
            unread: Vec::new(),
            markers: 0,
            deadline: TMUX_WAIT,
        };
        // Fixture premise: the shell reached `cat` with its terminal raw.
        // Until it has, a marker could be echoed or line-buffered, so the
        // first round trip is the readiness check.
        let answers = pane.round_trip(b"");
        assert!(answers.is_empty(), "a fresh pane answered {answers:?}");
        pane
    }
}

/// One tmux pane fed through its FIFO, and read through `capture-pane` and
/// `display-message`.
pub(super) struct Pane<'a> {
    server: &'a TmuxServer,
    /// `=session:`, which names the session's only pane exactly.
    target: String,
    size: Size,
    writer: File,
    reader: File,
    /// Answer bytes read from the file but not yet handed out.
    unread: Vec<u8>,
    markers: u32,
    /// How long a write or a marker answer may take ([`TMUX_WAIT`]; the
    /// harness's own test shortens it).
    pub(super) deadline: Duration,
}

/// The pane's state as one tmux report: the cursor, then the title, after
/// the visible rows `capture-pane` prints. The title goes last because it is
/// the one field a program controls.
const STATE_FORMAT: &str = "#{cursor_x},#{cursor_y}\n#{pane_title}";

impl Pane<'_> {
    /// Write `bytes` and wait until tmux has parsed them; returns tmux's
    /// answers to queries among them, in order. Panics, saying whether the
    /// pane is alive, if either step does not finish in time.
    pub(super) fn round_trip(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.try_round_trip(bytes)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// [`Pane::round_trip`], with its failure as a value.
    pub(super) fn try_round_trip(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mode = FIRST_MARKER_MODE + self.markers % MARKER_SPAN;
        self.markers += 1;
        let mut chunk = bytes.to_vec();
        chunk.extend_from_slice(format!("\x1b[?{mode}$p").as_bytes());
        self.write_bounded(&chunk)?;
        let marker_answer = format!("\x1b[?{mode};0$y").into_bytes();
        let reader = &mut self.reader;
        let unread = &mut self.unread;
        let answered = poll_until(Instant::now() + self.deadline, || {
            reader.read_to_end(unread).expect("read answers");
            let at = unread
                .windows(marker_answer.len())
                .position(|window| window == marker_answer)?;
            let answers = unread[..at].to_vec();
            unread.drain(..at + marker_answer.len());
            Some(answers)
        });
        answered.ok_or_else(|| {
            format!(
                "tmux did not answer marker {mode} within {:?}; the pane is {}",
                self.deadline,
                self.liveness()
            )
        })
    }

    /// Write all of `bytes` into the FIFO, waiting for room under the
    /// deadline, and giving up early if the pane died.
    fn write_bounded(&mut self, bytes: &[u8]) -> Result<(), String> {
        let deadline = Instant::now() + self.deadline;
        let mut rest = bytes;
        let mut next_check = Instant::now() + LIVENESS_EVERY;
        while !rest.is_empty() {
            match self.writer.write(rest) {
                Ok(written) => rest = &rest[written..],
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    let now = Instant::now();
                    if now >= next_check {
                        let liveness = self.liveness();
                        if liveness != "alive" {
                            return Err(format!(
                                "the pane stopped reading with {} bytes unwritten: it is {liveness}",
                                rest.len()
                            ));
                        }
                        next_check = now + LIVENESS_EVERY;
                    }
                    if now >= deadline {
                        return Err(format!(
                            "the pane did not take {} more bytes within {:?}; it is {}",
                            rest.len(),
                            self.deadline,
                            self.liveness()
                        ));
                    }
                    // sleep-ok: polling interval while the pane's reader drains the FIFO; the deadline and liveness check above bound it.
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => return Err(format!("writing to the pane: {error}")),
            }
        }
        Ok(())
    }

    /// "alive", or "dead (exit status N)", as tmux reports the pane (the
    /// server keeps dead panes, `remain-on-exit on`).
    pub(super) fn liveness(&self) -> String {
        let out = self.server.tmux(&[
            "display-message",
            "-p",
            "-t",
            &self.target,
            "#{pane_dead} #{pane_dead_status}",
        ]);
        match out.trim_end().split_once(' ') {
            Some(("1", status)) => format!("dead (exit status {status})"),
            _ if out.starts_with('1') => "dead".to_owned(),
            _ => "alive".to_owned(),
        }
    }

    /// The pane's visible screen as the status sampler captures it, its
    /// cursor `(column, line)` and its title, in one tmux call so that all
    /// describe the same moment.
    fn capture(&self) -> (String, (usize, usize), String) {
        let out = self.server.tmux(&[
            "capture-pane",
            "-p",
            "-t",
            &self.target,
            ";",
            "display-message",
            "-p",
            "-t",
            &self.target,
            STATE_FORMAT,
        ]);
        // capture-pane prints exactly one line per row; the state follows.
        let mut split = 0;
        for _ in 0..self.size.lines {
            split += out[split..].find('\n').expect("a line per row") + 1;
        }
        let (cursor, title) = out[split..]
            .split_once('\n')
            .expect("cursor line and title");
        let (x, y) = cursor.split_once(',').expect("cursor_x,cursor_y");
        let cursor = (x.parse().expect("cursor_x"), y.parse().expect("cursor_y"));
        let title = title.strip_suffix('\n').unwrap_or(title);
        (out[..split].to_owned(), cursor, title.to_owned())
    }

    /// The host name tmux reports as a pane's title until its program sets
    /// one.
    fn host(&self) -> String {
        let out = self
            .server
            .tmux(&["display-message", "-p", "-t", &self.target, "#{host}"]);
        out.trim_end_matches('\n').to_owned()
    }

    /// Resize the pane's window, as the supervisor does when a viewer's size
    /// changes.
    fn resize(&mut self, size: Size) {
        let columns = size.columns.to_string();
        let lines = size.lines.to_string();
        self.server.tmux(&[
            "resize-window",
            "-t",
            &self.target,
            "-x",
            &columns,
            "-y",
            &lines,
        ]);
        self.size = size;
    }
}

/// The tmux answers that differ from the emulator's on purpose, each with the
/// emulator's answer in its place (see the module docs).
fn expected_answers(tmux: &[u8]) -> Vec<u8> {
    const SUBSTITUTIONS: &[(&[u8], &[u8])] = &[
        (b"\x1b[>84;0;0c", b"\x1b[>0;2600;1c"),
        (
            b"\x1bP>|tmux 3.7c\x1b\\",
            farhelm_terminal_emulator::VERSION_REPORT.as_bytes(),
        ),
    ];
    let mut out = tmux.to_vec();
    for (theirs, ours) in SUBSTITUTIONS {
        out = replace_all(&out, theirs, ours);
    }
    out
}

/// `haystack` with every occurrence of `needle` replaced by `with`.
fn replace_all(haystack: &[u8], needle: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut rest = haystack;
    while let Some(at) = rest.windows(needle.len()).position(|w| w == needle) {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(with);
        rest = &rest[at + needle.len()..];
    }
    out.extend_from_slice(rest);
    out
}

/// A row the emulator left with a blanked last cell for a wide character that
/// did not fit (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stranded {
    /// The row, from the top.
    row: usize,
    /// How many columns the row's text occupies before its trailing blanks,
    /// in the emulator's grid.
    occupied: usize,
}

/// Whether tmux's row `theirs` is the emulator's row `ours` plus one narrow
/// character in the last of `columns` columns exactly: `ours`, which occupies
/// `occupied` columns, then blanks up to the last column, then that
/// character.
fn stranded_row_agrees(theirs: &str, ours: &str, occupied: usize, columns: usize) -> bool {
    let Some(padding) = (columns - 1).checked_sub(occupied) else {
        return false;
    };
    let Some(rest) = theirs.strip_prefix(ours) else {
        return false;
    };
    let Some(last) = rest.strip_prefix(&" ".repeat(padding)) else {
        return false;
    };
    let mut chars = last.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c != ' ' && is_narrow(c))
}

/// Whether `c` takes one column, by the emulator's own width rules (the
/// supervisor has no width table of its own): written at the start of a row,
/// it leaves the cursor in the second column.
fn is_narrow(c: char) -> bool {
    let mut emulator = Emulator::new(
        Size {
            columns: 4,
            lines: 1,
        },
        0,
    );
    let mut ignored = Collected::default();
    emulator.feed(
        c.to_string().as_bytes(),
        t0(),
        FeedMode::ReplaySeen,
        &mut ignored,
    );
    emulator.term().grid().cursor.point.column.0 == 1
}

/// What tmux and the emulator showed at one cut.
#[derive(Debug, Clone)]
pub(super) struct Cut {
    /// Bytes fed to both so far.
    pub(super) offset: usize,
    pub(super) tmux_text: String,
    pub(super) emulator_text: String,
    /// What the sampler would read from the emulator: its own tail
    /// (`Emulator::screen_tail`), the call a host terminal's sampler makes.
    pub(super) emulator_tail: String,
    /// The cursor as `(column, line)` from the top left, counted from 0, as
    /// tmux reports it (see the module docs for a pending wrap).
    pub(super) tmux_cursor: (usize, usize),
    pub(super) emulator_cursor: (usize, usize),
    /// The title the status reader would get from tmux (the host name before
    /// the program set one).
    pub(super) tmux_title: String,
    pub(super) emulator_title: String,
    /// Whether a synchronized update was open in the emulator, so that
    /// nothing was comparable.
    pub(super) in_update: bool,
    /// tmux's answers since the last comparable cut, with the deliberate
    /// differences already replaced by the emulator's answers.
    pub(super) tmux_answers: Vec<u8>,
    pub(super) emulator_answers: Vec<u8>,
    /// Rows whose last cell the emulator blanked for a wide character that
    /// did not fit there (see the module docs).
    stranded: Vec<Stranded>,
    columns: usize,
}

impl Cut {
    /// The tail of tmux's capture the sampler reads.
    pub(super) fn tmux_tail(&self) -> String {
        retain_pane_tail(&self.tmux_text, SAMPLE_TAIL_BYTES)
    }

    /// Whether the screen texts agree, allowing tmux one more last-column
    /// character on the stranded rows.
    fn texts_agree(&self) -> bool {
        let tmux: Vec<&str> = self.tmux_text.split('\n').collect();
        let emulator: Vec<&str> = self.emulator_text.split('\n').collect();
        tmux.len() == emulator.len()
            && tmux
                .iter()
                .zip(&emulator)
                .enumerate()
                .all(|(row, (theirs, ours))| {
                    theirs == ours
                        || self
                            .stranded
                            .iter()
                            .find(|stranded| stranded.row == row)
                            .is_some_and(|stranded| {
                                stranded_row_agrees(theirs, ours, stranded.occupied, self.columns)
                            })
                })
    }

    /// Every way this cut fails parity, as readable lines; empty when it
    /// holds, and when it is inside an update and so not compared (the
    /// caller counts those through `in_update`).
    pub(super) fn differences(&self, host: &str) -> Vec<String> {
        let mut out = Vec::new();
        if self.in_update {
            return out;
        }
        if !self.texts_agree() {
            out.push(format!(
                "screen text at byte {}:\n  tmux:     {:?}\n  emulator: {:?}",
                self.offset, self.tmux_text, self.emulator_text
            ));
        }
        if self.tmux_cursor != self.emulator_cursor {
            out.push(format!(
                "cursor (column, line) at byte {}: tmux {:?}, emulator {:?}",
                self.offset, self.tmux_cursor, self.emulator_cursor
            ));
        }
        let expected_title = if self.tmux_title == host {
            ""
        } else {
            self.tmux_title.trim()
        };
        if expected_title != self.emulator_title {
            out.push(format!(
                "title at byte {}: tmux {:?}, emulator {:?}",
                self.offset, self.tmux_title, self.emulator_title
            ));
        }
        if self.tmux_answers != self.emulator_answers {
            out.push(format!(
                "answers at byte {}: tmux {:?}, emulator {:?}",
                self.offset,
                String::from_utf8_lossy(&self.tmux_answers),
                String::from_utf8_lossy(&self.emulator_answers)
            ));
        }
        out
    }
}

/// Whether a cell shows nothing but a space.
fn is_blank(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.zerowidth().is_none()
        && !cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
}

/// A pane and an emulator of the same size that have been fed the same bytes.
pub(super) struct Rig<'a> {
    pub(super) pane: Pane<'a>,
    pub(super) emulator: Emulator,
    /// tmux's title for a pane whose program set none.
    pub(super) host: String,
    offset: usize,
    /// Answers given since the last comparable cut (see the module docs).
    tmux_answers: Vec<u8>,
    emulator_answers: Vec<u8>,
}

impl<'a> Rig<'a> {
    pub(super) fn new(server: &'a mut TmuxServer, size: Size) -> Rig<'a> {
        let pane = server.pane(size);
        let host = pane.host();
        Rig {
            pane,
            emulator: Emulator::new(size, 1000),
            host,
            offset: 0,
            tmux_answers: Vec::new(),
            emulator_answers: Vec::new(),
        }
    }

    /// Feed `chunk` to both and capture both. The caller must cut only where
    /// the emulator is between sequences and not right before a REP (see the
    /// module docs); this checks the first.
    pub(super) fn step(&mut self, chunk: &[u8]) -> Cut {
        self.step_and_maybe_flush(chunk, false)
    }

    /// [`Rig::step`], then end the emulator's open synchronized update, if
    /// any, before capturing, as the supervisor does when the program exits.
    /// For output that stops inside an update: tmux has applied all of it,
    /// so after the flush the two are comparable.
    pub(super) fn step_and_flush(&mut self, chunk: &[u8]) -> Cut {
        self.step_and_maybe_flush(chunk, true)
    }

    fn step_and_maybe_flush(&mut self, chunk: &[u8], flush: bool) -> Cut {
        let mut effects = Collected::default();
        self.emulator
            .feed(chunk, t0(), FeedMode::Live, &mut effects);
        if flush {
            self.emulator.flush_sync(FeedMode::Live, &mut effects);
        }
        let in_update = self.emulator.sync_deadline().is_some();
        assert!(
            self.emulator.is_clean() || in_update,
            "cut at byte {} is inside a sequence",
            self.offset + chunk.len()
        );
        self.tmux_answers
            .extend(expected_answers(&self.pane.round_trip(chunk)));
        self.emulator_answers.extend(effects.replies);
        self.offset += chunk.len();
        let (tmux_text, tmux_cursor, tmux_title) = self.pane.capture();
        let grid = self.emulator.term().grid();
        let columns = grid.columns();
        let stranded = (0..grid.screen_lines())
            .filter_map(|row| {
                let cells = &grid[Line(row as i32)];
                if !cells[Column(columns - 1)]
                    .flags
                    .contains(Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    return None;
                }
                let occupied = (0..columns - 1)
                    .rev()
                    .find(|&column| !is_blank(&cells[Column(column)]))
                    .map_or(0, |column| column + 1);
                Some(Stranded { row, occupied })
            })
            .collect();
        // The vendored alacritty's wrap flag stands for tmux's cursor one past
        // the last column (`cx == sx`): `Term::input` raises it at the last
        // column only with autowrap on, where tmux moves past the column.
        let cursor = &grid.cursor;
        let emulator_cursor = (
            cursor.point.column.0 + usize::from(cursor.input_needs_wrap),
            cursor.point.line.0 as usize,
        );
        let (tmux_answers, emulator_answers) = if in_update {
            (Vec::new(), Vec::new())
        } else {
            (
                std::mem::take(&mut self.tmux_answers),
                std::mem::take(&mut self.emulator_answers),
            )
        };
        Cut {
            offset: self.offset,
            tmux_text,
            emulator_text: self.emulator.screen_text(),
            emulator_tail: self.emulator.screen_tail(SAMPLE_TAIL_BYTES),
            tmux_cursor,
            emulator_cursor,
            tmux_title,
            emulator_title: self.emulator.title().to_owned(),
            in_update,
            tmux_answers,
            emulator_answers,
            stranded,
            columns,
        }
    }

    /// Resize both. The emulator flushes an open synchronized update first;
    /// any answers that releases are compared at the next cut.
    pub(super) fn resize(&mut self, size: Size) {
        self.pane.resize(size);
        let mut effects = Collected::default();
        self.emulator.resize(size, FeedMode::Live, &mut effects);
        self.emulator_answers.extend(effects.replies);
    }
}

/// The stranded-row allowance accepts tmux's extra character only in the
/// last column, after exactly the emulator's text and blanks.
///
/// Why: the allowance exists for one documented difference, the last cell a
/// wide character could not fit into; accepted anywhere else, it would hide
/// missing or misplaced text. Spec: on 20 columns with the emulator's row
/// `abc` (3 columns), tmux's `abc` + 16 blanks + `X` agrees; `abcX`,
/// `abc` + 15 blanks + `X`, a wide last character, a space, and two
/// characters do not. A box-drawing character, which is narrow but outside
/// ASCII, agrees like `X`.
#[test]
fn the_stranded_row_allowance_is_the_last_column_only() {
    let agrees = |theirs: &str| stranded_row_agrees(theirs, "abc", 3, 20);
    assert!(agrees(&format!("abc{}X", " ".repeat(16))));
    assert!(!agrees("abcX"));
    assert!(!agrees(&format!("abc{}X", " ".repeat(15))));
    assert!(agrees(&format!("abc{}╮", " ".repeat(16))));
    assert!(!agrees(&format!("abc{}中", " ".repeat(16))));
    assert!(!agrees(&format!("abc{} ", " ".repeat(16))));
    assert!(!agrees(&format!("abc{}XY", " ".repeat(15))));
    assert!(!stranded_row_agrees("abX", "abc", 3, 20));
}

/// A write to a pane whose program died fails within the deadline and says
/// the pane is dead, instead of blocking the test forever.
///
/// Why: the test holds the FIFO open for reading itself, so a dead reader
/// never causes `EPIPE`; once the pipe's buffer fills, a plain write would
/// block with no deadline and no diagnosis. Spec: after the pane's program
/// is replaced by one that exits with status 3, a round trip of more than a
/// pipe's capacity returns an error naming the pane as dead with exit
/// status 3.
#[test]
fn a_write_to_a_dead_pane_fails_with_a_report() {
    if !pinned_tmux_or_skip("a_write_to_a_dead_pane_fails_with_a_report") {
        return;
    }
    let mut server = TmuxServer::new();
    let mut pane = server.pane(Size {
        columns: 20,
        lines: 4,
    });
    pane.deadline = Duration::from_secs(5);
    pane.server
        .tmux(&["respawn-pane", "-k", "-t", &pane.target, "exit 3"]);
    // Fixture premise: the pane is dead before the write.
    let died = poll_until(Instant::now() + Duration::from_secs(5), || {
        (pane.liveness() != "alive").then_some(())
    });
    assert!(died.is_some(), "the respawned pane did not exit");
    let error = pane
        .try_round_trip(&vec![b'x'; 1 << 20])
        .expect_err("a write of 1 MiB to a dead pane succeeded");
    // The write's own report, not the marker wait's, which would also name
    // the pane's state.
    assert!(
        error.contains("stopped reading") && error.contains("dead (exit status 3)"),
        "unexpected report: {error}"
    );
}
