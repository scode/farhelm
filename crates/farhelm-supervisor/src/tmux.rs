//! Which tmux a Farhelm installation may rely on: the version floor, the
//! `tmux -V` probe, and how a tmux program is named and found.
//!
//! The supervisor no longer drives tmux. Its sessions run in terminal hosts,
//! and the one tmux-related thing it still does at startup, ending the
//! private server an earlier release ran sessions in, needs no tmux program
//! at all (`service::core::old_terminals`). What is left here is the policy
//! the rest of an installation still applies to the tmux it requires:
//! `farhelm helm setup` probes and pins one into the supervisor's service
//! unit, provisioning installs one on remote hosts, and the desktop app
//! checks one before starting its supervisor. Those requirements go with
//! their own changes; until then this module is what they share, so that a
//! below-floor tmux is refused the same way everywhere.

use anyhow::{Context, bail};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A tmux release as Farhelm orders it: the numeric `major.minor` pair
/// plus tmux's patch-release letter.
///
/// The letter is not decoration, which is why this type exists at all.
/// [`TMUX_FLOOR`] names one specific regression-tested build, and the
/// letter is the only thing distinguishing it from the distinct
/// below-floor releases it shares a `major.minor` with: 3.7, 3.7a and
/// 3.7b are three separate releases, and 3.7b in particular is heavily
/// exercised in this file's own history — below the current pin, not
/// untried. The version check this replaced compared `(major, minor)`
/// alone, so it could not tell them apart and would have accepted any of
/// them as the pinned one.
///
/// Ordering is the derived lexicographic one, and it matches tmux's own
/// release order because `None < Some('a')`: a bare `3.7` shipped before
/// `3.7a`, which shipped before `3.7b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TmuxVersion {
    major: u32,
    minor: u32,
    /// tmux's patch-release letter (`3.7b`'s `b`); `None` for a bare
    /// `3.7`. At most one lowercase letter — see [`parse_tmux_version`]
    /// for why anything else is refused rather than approximated.
    patch: Option<char>,
}

impl std::fmt::Display for TmuxVersion {
    /// Round-trips [`parse_tmux_version`]'s input spelling, because this
    /// is what the refusal message and the release pin are compared
    /// against — a version that printed differently from how tmux spells
    /// it would make both unreadable.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)?;
        if let Some(patch) = self.patch {
            write!(formatter, "{patch}")?;
        }
        Ok(())
    }
}

/// The oldest tmux Farhelm will drive: the version its own regression
/// suites are run against, not the oldest one that happens to work.
///
/// Farhelm drives tmux far harder than interactive use does — control
/// mode, output-client teardown, pane-death timing — and versions are not
/// interchangeable there. Crashes have been observed on older ones (a
/// distro 3.4 server hosting live sessions aborted on 2026-08-19, and
/// BUGS.md records the same abort class reproduced on distro 3.6), while
/// the pinned build is the one the release gate's tests run against. So
/// the floor tracks the pinned build in `.github/release/source-pins.env`
/// rather than tracking what distributions ship.
///
/// It is DESIGNED to exclude current distro packages (Ubuntu 24.04 ships
/// 3.4, 26.04 about 3.6, Debian 13 and Fedora 42 3.5a). The documented
/// ways to satisfy it are Homebrew/Linuxbrew, Farhelm's own private build
/// that provisioning installs, and the `--tmux` / `FARHELM_TMUX` override
/// (see [`resolve_tmux_program`]).
///
/// Bumping it is a deliberate act taken TOGETHER with the pinned build and
/// its regression suite, never on its own. The tests'
/// `floor_and_release_pin_cannot_drift` enforces that half mechanically:
/// it fails the moment this constant and `TMUX_VERSION=` in
/// `.github/release/source-pins.env` disagree.
pub const TMUX_FLOOR: TmuxVersion = TmuxVersion {
    major: 3,
    minor: 7,
    patch: Some('c'),
};

/// The exact prefix `tmux -V` puts in front of the version. Nothing else
/// is accepted: a line that does not start this way did not come from a
/// tmux this project knows how to reason about.
const TMUX_VERSION_LINE_PREFIX: &str = "tmux ";

/// Parse a whole `tmux -V` line — `"tmux 3.7b\n"` — into an orderable
/// version.
///
/// Conservative by contract: anything this cannot read exactly is an
/// error, never a guess. Every caller turns that error into "refuse" or
/// "install our own build", which is the safe direction — a version
/// Farhelm cannot name is a version nobody has audited it against, and
/// guessing high buys a wedged or crashing substrate.
///
/// THE WHOLE LINE is validated, not just a token lifted out of it. An
/// earlier revision took `split_whitespace().nth(1)`, which happily read
/// `not-tmux 3.7c`, `tmux 3.7c vendor-patch` and `tmux +3.07c` as clean
/// releases. That is not a cosmetic gap: this same parser decides whether
/// provisioning SKIPS installing Farhelm's own audited build, so a
/// decorated or forged line that scans as an audited release is exactly
/// how an unaudited substrate gets adopted silently. So: the `tmux `
/// prefix exactly, one version token and nothing after it, at most one
/// trailing newline, and canonical decimal components (no sign, no
/// leading zeros).
///
/// It takes the whole line rather than a bare number because that is the
/// shape callers have — the provisioning probe forwards a remote host's
/// `tmux -V` output verbatim, and "tmux is not installed" text must fail
/// parsing rather than resemble a version.
///
/// DELIBERATELY NARROW, and this is a product decision rather than a
/// parser limitation: Farhelm supports stable releases and tmux's
/// single-letter patch releases, so tmux's official development
/// (`next-3.8`) and release-candidate (`3.8-rc`, `3.8-rc2`) spellings are
/// refused even though they are numerically above the floor. Ordering
/// those stages against a stable release is guesswork, and the `--tmux`
/// escape hatch exists for anyone who wants to own that risk in a build
/// of their own.
pub fn parse_tmux_version(v_output: &str) -> anyhow::Result<TmuxVersion> {
    // Exactly one optional trailing newline; a second one, a `\r`, or any
    // other trailing whitespace falls through to the token check below
    // and is refused there.
    let line = v_output.strip_suffix('\n').unwrap_or(v_output);
    let version = line
        .strip_prefix(TMUX_VERSION_LINE_PREFIX)
        .with_context(|| {
            format!("tmux -V returned {line:?}, which is not a {TMUX_VERSION_LINE_PREFIX:?} line")
        })?;
    if version.is_empty() || version.contains(char::is_whitespace) {
        bail!("tmux -V returned {line:?}, which is not exactly one version token");
    }
    parse_tmux_version_number(version)
        .with_context(|| format!("tmux -V returned an unrecognized version {version:?}"))
}

/// [`parse_tmux_version`] minus the `tmux ` prefix and the line framing.
///
/// The error context naming the offending token is added by the caller,
/// which knows how the token was framed.
fn parse_tmux_version_number(version: &str) -> anyhow::Result<TmuxVersion> {
    let (major, rest) = version
        .split_once('.')
        .context("expected a major.minor version")?;
    let major = parse_version_component(major, "major")?;
    let suffix_at = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    let (minor, suffix) = rest.split_at(suffix_at);
    let minor = parse_version_component(minor, "minor")?;
    // tmux's own patch spelling is exactly one lowercase letter. Anything
    // longer is a build this project has never seen, and pretending the
    // first letter of it is a patch level would silently admit it.
    let patch = match suffix.as_bytes() {
        [] => None,
        [letter] if letter.is_ascii_lowercase() => Some(char::from(*letter)),
        _ => bail!("unrecognized patch suffix {suffix:?}"),
    };
    Ok(TmuxVersion {
        major,
        minor,
        patch,
    })
}

/// One numeric component of a version, in tmux's own canonical spelling.
///
/// Stricter than `u32::from_str` on purpose. `parse` accepts `+3` and
/// `0007`, both of which would then round-trip through [`TmuxVersion`]'s
/// `Display` as a DIFFERENT string from the one tmux printed — and the
/// refusal message, the release-pin lockstep test, and provisioning's
/// floor comparison all trade in those printed forms. Refusing the
/// non-canonical spellings keeps parse and print exact inverses.
fn parse_version_component(text: &str, which: &str) -> anyhow::Result<u32> {
    if text.is_empty() {
        bail!("the {which} version is empty");
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("the {which} version {text:?} is not a plain decimal number");
    }
    if text.len() > 1 && text.starts_with('0') {
        bail!("the {which} version {text:?} has a leading zero");
    }
    text.parse()
        .with_context(|| format!("parsing the {which} version {text:?}"))
}

/// Where one discovered tmux version sits relative to [`TMUX_FLOOR`].
///
/// A pure verdict, deliberately separated from the messages: each caller
/// words its own refusal or warning while sharing one definition of
/// acceptable. Splitting them also makes the policy testable without
/// capturing log output, so "3.7c must not warn, 3.7d must" is an ordinary
/// assertion instead of a tracing-subscriber fixture.
///
/// Public because `farhelm helm setup` decides the same question about a
/// tmux it is about to pin into a unit file, and the desktop app about the
/// tmux it checks before starting its supervisor, and both must reach that
/// verdict through this policy rather than their own comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmuxSupport {
    /// Older than the regression-tested build: refuse.
    BelowFloor,
    /// Exactly the pinned, regression-tested build: the silent, blessed
    /// case.
    AtFloor,
    /// Newer than the pin: accepted, but nobody has audited Farhelm
    /// against it, so the caller warns.
    AbovePin,
}

/// Classify a discovered version against the floor.
///
/// Newer-than-pinned is deliberately NOT a refusal. Homebrew will ship a
/// version nobody has audited Farhelm against long before this floor
/// moves, and refusing it would strand users on a release the project
/// itself recommends installing; the warning its callers emit records
/// that the combination is untested so a later bug report starts from the
/// right place.
pub fn classify_tmux_version(found: TmuxVersion) -> TmuxSupport {
    match found.cmp(&TMUX_FLOOR) {
        std::cmp::Ordering::Less => TmuxSupport::BelowFloor,
        std::cmp::Ordering::Equal => TmuxSupport::AtFloor,
        std::cmp::Ordering::Greater => TmuxSupport::AbovePin,
    }
}

/// What running `<program> -V` proved about one candidate tmux.
///
/// `program` is echoed back because the caller's candidate may have come
/// from a `PATH` search it did not do itself, and every message about a
/// rejected tmux has to name which binary answered.
#[derive(Debug, Clone)]
pub struct TmuxProbe {
    pub program: PathBuf,
    pub version: TmuxVersion,
    pub support: TmuxSupport,
}

/// The three ways a candidate tmux fails to produce a verdict, kept apart
/// because they are different things to tell a user: one binary could not
/// be run at all, one ran and said something this project cannot read, and
/// one ran but would not shut up or finish.
#[derive(Debug)]
pub enum TmuxProbeError {
    NotRunnable(std::io::Error),
    Unparseable(String),
    /// The candidate exceeded the probe's time or output budget. Carries a
    /// human-readable statement of which.
    Overran(String),
}

impl std::fmt::Display for TmuxProbeError {
    /// The detail a refusal message appends after naming the candidate.
    /// Deliberately a fragment, not a sentence: the caller supplies the
    /// subject ("a tmux at /x/tmux that could not be run").
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunnable(error) => write!(formatter, "{error}"),
            Self::Unparseable(printed) if printed.is_empty() => {
                formatter.write_str("it printed nothing")
            }
            Self::Unparseable(printed) => write!(formatter, "it printed {printed:?}"),
            Self::Overran(detail) => formatter.write_str(detail),
        }
    }
}

/// How long a candidate gets to answer `-V`.
///
/// Generous for a program whose whole job here is to print one line, and
/// short enough that a wedged candidate cannot hold up a CLI command the
/// operator is watching.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// How much of each stream the probe will read.
///
/// A version line is tens of bytes. Anything past this is not an answer,
/// and reading it unbounded is how a hostile or broken candidate turns a
/// version check into an out-of-memory kill.
const PROBE_CAPTURE_LIMIT: usize = 4 * 1024;

/// Ask a specific tmux binary what version it is.
///
/// Synchronous and side-effect free by design: `tmux -V` answers without
/// touching the socket, the config, or any server, which is what lets
/// `farhelm helm setup` — an ordinary CLI command with no async runtime —
/// apply the floor policy.
///
/// BOUNDED in both directions, because setup points this at whatever the
/// operator named or `PATH` produced, which is not necessarily tmux: the
/// candidate gets [`PROBE_DEADLINE`] to exit and [`PROBE_CAPTURE_LIMIT`]
/// bytes per stream, and exceeding either is [`TmuxProbeError::Overran`]
/// rather than a hang or an unbounded allocation. A candidate that spawns
/// a descendant holding the captured pipes open cannot stall the probe
/// either: the collection step has its own short budget and gives up on
/// the readers rather than waiting for EOF.
///
/// A non-zero exit is [`TmuxProbeError::Unparseable`] carrying the trimmed
/// STDERR — something ran under that name and did not answer as tmux,
/// which is the same actionable fact as an unreadable version line, and
/// stderr is where such a program says why. `program` is used verbatim, so
/// a bare name is resolved by the OS against the caller's own `PATH`
/// rather than this function's idea of one.
pub fn probe_tmux(program: &Path) -> Result<TmuxProbe, TmuxProbeError> {
    use std::os::unix::process::CommandExt as _;

    let mut child = std::process::Command::new(program)
        .arg("-V")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        // Its OWN process group. That is what makes the containment below
        // possible at all: a candidate that forks before misbehaving
        // otherwise leaves descendants this function has no handle on, no
        // way to name, and no way to kill — and they inherited the pipes,
        // so they can hold the reader threads open after the probe has
        // returned. One group means one kill reaches everything the probe
        // started.
        .process_group(0)
        .spawn()
        .map_err(TmuxProbeError::NotRunnable)?;
    // `process_group(0)` makes the leader's pid the group id.
    let group = ProbeGroup(child.id());
    let stdout = capture_bounded(child.stdout.take().expect("piped probe stdout"));
    let stderr = capture_bounded(child.stderr.take().expect("piped probe stderr"));

    let deadline = std::time::Instant::now() + PROBE_DEADLINE;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() >= deadline => {
                return Err(group.retire(
                    &mut child,
                    stdout,
                    stderr,
                    format!(
                        "it did not answer -V within {} seconds",
                        PROBE_DEADLINE.as_secs()
                    ),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(TmuxProbeError::NotRunnable(error)),
        }
    };

    // The leader is gone, which does NOT mean the pipes are closed: a
    // descendant it forked inherited them and can hold them open forever.
    // Collection is bounded for that reason, and an expired budget is
    // itself the evidence that something is still alive in the group.
    let collected = match (
        stdout.receiver.recv_timeout(PROBE_DRAIN_BUDGET),
        stderr.receiver.recv_timeout(PROBE_DRAIN_BUDGET),
    ) {
        (Ok(Ok(out)), Ok(Ok(err))) => Ok((out, err)),
        (Ok(Err(())), _) => Err(overflowed("stdout")),
        (_, Ok(Err(()))) => Err(overflowed("stderr")),
        (Err(_), _) => Err(still_held("stdout")),
        (_, Err(_)) => Err(still_held("stderr")),
    };
    let (out, err) = match collected {
        Ok(streams) => streams,
        Err(detail) => return Err(group.retire(&mut child, stdout, stderr, detail)),
    };
    // Nothing is left to contain on this path: both readers reached EOF,
    // which every writer had to close for. Join them so the probe owns no
    // live thread when it returns.
    stdout.finish();
    stderr.finish();

    if !status.success() {
        return Err(TmuxProbeError::Unparseable(err.trim().to_string()));
    }
    let version = parse_tmux_version(&out)
        .map_err(|_| TmuxProbeError::Unparseable(out.trim().to_string()))?;
    Ok(TmuxProbe {
        program: program.to_path_buf(),
        version,
        support: classify_tmux_version(version),
    })
}

fn overflowed(which: &str) -> String {
    format!("it printed more than {PROBE_CAPTURE_LIMIT} bytes on {which}")
}

fn still_held(which: &str) -> String {
    format!("its {which} was still held open after it exited")
}

/// The process group one probe created, and the only handle that reaches
/// everything that probe started.
struct ProbeGroup(u32);

impl ProbeGroup {
    /// Kill the whole group, reap the leader, and abandon both reader
    /// threads, then report the overrun that got us here.
    ///
    /// The order is load-bearing. The group is killed BEFORE the leader is
    /// reaped where that is still possible, because a group whose members
    /// have all been reaped no longer reserves its id.
    ///
    /// The readers are NOT joined. The kill reaches only the probe's own
    /// process group, and a descendant that left it (`setsid`, or a shell's
    /// job control) survives with the pipes still open, so a reader blocked
    /// on EOF could wait for as long as that descendant lives. Dropping the
    /// captures detaches those threads instead: the probe returns within its
    /// budget, and a detached reader ends on its own once the pipe closes,
    /// having already read at most its bounded share.
    ///
    /// The residual risk is honest to state: if the leader was already
    /// reaped and the group is now genuinely empty, its id could in
    /// principle have been reused by an unrelated group, and this signals
    /// that one. It takes a full pid-space wrap between the two moments,
    /// and the alternative — leaving a hostile candidate's descendants
    /// running with our pipes — is the worse trade.
    fn retire(
        &self,
        child: &mut std::process::Child,
        stdout: Capture,
        stderr: Capture,
        detail: String,
    ) -> TmuxProbeError {
        // SAFETY: `kill` with a negative pid signals a process group and
        // touches no memory. A group that is already gone answers ESRCH,
        // which is the outcome this wants anyway.
        unsafe { libc::kill(-(self.0 as i32), libc::SIGKILL) };
        let _ = child.kill();
        let _ = child.wait();
        drop(stdout);
        drop(stderr);
        TmuxProbeError::Overran(detail)
    }
}

/// One child stream being read on its own thread, bounded.
struct Capture {
    receiver: std::sync::mpsc::Receiver<Result<String, ()>>,
    reader: std::thread::JoinHandle<()>,
}

impl Capture {
    /// Wait for the reader thread to end. Only safe to call once every
    /// writer for its pipe is gone, which is why it is called only after
    /// both readers saw EOF; an overrun abandons the readers instead (see
    /// [`ProbeGroup::retire`]).
    fn finish(self) {
        let _ = self.reader.join();
    }
}

/// Read at most [`PROBE_CAPTURE_LIMIT`] bytes from one child stream on a
/// thread, reporting overflow rather than growing.
///
/// A thread rather than sequential reads because the child writes both
/// streams and a single-threaded reader can deadlock against the pipe
/// buffer of the one it is not reading. The result is SENT before the
/// thread ends, so a caller can have the answer while the thread is still
/// waiting for EOF on a pipe some descendant holds.
fn capture_bounded(mut stream: impl std::io::Read + Send + 'static) -> Capture {
    let (tx, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        use std::io::Read as _;

        let mut buffer = Vec::new();
        // One byte past the limit is how overflow is detected without
        // reading the rest of whatever the candidate is producing.
        let read = stream
            .by_ref()
            .take(PROBE_CAPTURE_LIMIT as u64 + 1)
            .read_to_end(&mut buffer);
        let answer = match read {
            Ok(_) if buffer.len() > PROBE_CAPTURE_LIMIT => Err(()),
            Ok(_) => Ok(String::from_utf8_lossy(&buffer).into_owned()),
            Err(_) => Ok(String::new()),
        };
        let _ = tx.send(answer);
    });
    Capture { receiver, reader }
}

/// How long the probe waits for a reader once the child is gone.
///
/// The child has exited by then, so this is only about a DESCENDANT it
/// left holding the pipe. Waiting for that would reintroduce the hang the
/// deadline exists to prevent, so an expired budget becomes an overrun —
/// and, unlike before, the group kill that follows retires the descendant
/// and the reader with it.
const PROBE_DRAIN_BUDGET: Duration = Duration::from_millis(250);

/// Find one program by name on a `PATH` value.
///
/// `path` is a parameter rather than a read of this process's environment
/// so callers can search the environment they captured (and so tests never
/// have to mutate their own). Returns `None` when nothing on `PATH` under
/// that name looks executable — a caller that wants the OS's own error
/// should spawn the bare name instead.
///
/// NOTE that "looks executable" is an approximation (see
/// [`is_executable_file`]), so the first hit is not guaranteed to run. A
/// caller that will actually SPAWN the result should walk
/// [`candidates_on_path`] instead and skip the ones that fail to start,
/// which is what `execvp` does and what this cannot do on its own.
pub fn find_on_path(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    candidates_on_path(path, name).next()
}

/// Every `PATH` entry that plausibly holds this program, in `PATH` order.
///
/// Exists because [`find_on_path`]'s answer can be wrong in a way only
/// spawning reveals: a `noexec` mount, an execute bit set for a group this
/// process is not in, or an LSM denial all look executable to a metadata
/// check. `execvp` skips such an entry and keeps walking, so a caller that
/// spawns must be able to as well — otherwise one unusable shadow early on
/// `PATH` hides a perfectly good tmux later.
pub fn candidates_on_path<'a>(
    path: &'a std::ffi::OsStr,
    name: &'a str,
) -> impl Iterator<Item = PathBuf> + 'a {
    let program = Path::new(name);
    std::env::split_paths(path)
        // Only absolute entries. An empty entry means the current directory
        // to the shell, and `.`, `./bin` or `node_modules/.bin` mean the same
        // thing spelled differently: whatever directory the caller happened
        // to run in. Setup, the one production caller, pins the result into
        // a boot-time systemd unit, and that directory (an untrusted checkout,
        // say) is never a defensible thing to pin; its tmux would then run
        // every session at every boot. A tmux the user really wants from
        // such a place can still be named explicitly.
        .filter(|dir| dir.is_absolute())
        .map(move |dir| dir.join(program))
        .filter(|candidate| is_executable_file(candidate))
}

/// The shared search behind [`find_on_path`] and [`program_display_path`].
fn search_path(path_var: &std::ffi::OsStr, program: &Path) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable_file(candidate))
}

/// The environment variable naming the tmux binary an installation
/// relies on.
///
/// Public because the desktop app sets it on the supervisor it spawns:
/// macOS GUI apps inherit no shell PATH, so the app probes the Homebrew
/// prefixes itself and hands the winner down through this variable. The
/// supervisor no longer runs it; what it still does with the variable is
/// keep it from the programs its terminals start
/// (`service::core::host_launch`).
pub const TMUX_PROGRAM_ENV: &str = "FARHELM_TMUX";

/// What Farhelm uses when nothing overrides it: plain `tmux`, resolved
/// by the operating system against `PATH` at spawn time, exactly as every
/// release before the override knob existed.
pub const DEFAULT_TMUX_PROGRAM: &str = "tmux";

/// Resolve which tmux binary is meant: an explicit `--tmux` wins, then
/// [`TMUX_PROGRAM_ENV`], then [`DEFAULT_TMUX_PROGRAM`] off `PATH`.
///
/// Both inputs are parameters rather than reads of this process's
/// environment, which is what makes the precedence testable: this repo's
/// tests never mutate the test process's environment, and a resolution
/// that read `FARHELM_TMUX` itself could only be exercised by doing so.
///
/// An empty `FARHELM_TMUX` counts as unset. A systemd unit or shell
/// profile that writes `FARHELM_TMUX=` means "no override" to whoever
/// wrote it, and the alternative reading — spawn a program with an empty
/// name — can only ever fail.
///
/// Choosing a binary here does not exempt it from anything: the result is
/// floor-checked like any other candidate and refused by name if it is too
/// old (see [`classify_tmux_version`]). The override means "you own the
/// substrate", not "this configuration is supported" — Farhelm drives tmux
/// harder than interactive use, versions below [`TMUX_FLOOR`] have crashed
/// under it, and versions above it are unaudited.
pub fn resolve_tmux_program(flag: Option<&Path>, env: Option<&std::ffi::OsStr>) -> PathBuf {
    if let Some(flag) = flag {
        return flag.to_path_buf();
    }
    match env {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(DEFAULT_TMUX_PROGRAM),
    }
}

/// Spell a program the way a human can act on it: a bare name is reported
/// as the `PATH` entry it resolves to.
///
/// ADVISORY, not authoritative, and the distinction is the whole reason
/// this doc exists. Nothing is executed through the returned path: a
/// caller keeps the SPELLING it was given and lets `Command` redo the
/// `PATH` lookup on every spawn, because resolving here and spawning by
/// absolute path would open a time-of-check gap the OS's own lookup does
/// not have. The flip side is honest to state: between this lookup and
/// the spawn, the winning `PATH` entry can be replaced, so the path in a
/// diagnostic is the best available name for what answered, not proof of
/// which inode did.
///
/// It exists because "tmux 3.6 is too old" is unactionable while "the
/// tmux 3.6 at /usr/bin/tmux is too old" tells the reader which entry to
/// fix. `path_var` is a parameter so the search is testable without
/// touching the test process's environment.
///
/// Public so the desktop app's own below-floor refusal can name the
/// resolved binary, rather than printing whatever bare spelling
/// `FARHELM_TMUX` happened to carry — see
/// `farhelm_ui::desktop::run_tmux_preflight_or_exit`.
pub fn program_display_path(program: &Path, path_var: Option<&std::ffi::OsStr>) -> PathBuf {
    // A name with any separator is already a path; the OS would not
    // consult PATH for it either.
    if program.components().count() != 1 {
        return program.to_path_buf();
    }
    path_var
        .and_then(|path_var| search_path(path_var, program))
        // Falling back to the bare name is right: the spawn that follows
        // will fail with the OS's own "no such file" and that error is
        // clearer than a path this function invented.
        .unwrap_or_else(|| program.to_path_buf())
}

/// Whether a `PATH` candidate LOOKS like something the OS would execute.
///
/// An approximation of `execvp`'s test, and knowingly so: it asks whether
/// ANY execute bit is set rather than whether THIS process's uid/gid can
/// use the one that applies, and it cannot see a `noexec` mount or an
/// LSM denial. That is the right trade for a diagnostic — the cheap check
/// removes the shadows that actually occur (data files, unstripped
/// permissions on a copied binary) without a permission model this code
/// would then have to keep true.
///
/// Because it is an approximation, a caller that will SPAWN the result
/// must be prepared for it to fail anyway and move on; that is what
/// [`candidates_on_path`] is for.
fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    // Windows has no execute bit; being a regular file with the right
    // name is as close as this diagnostic can get.
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Version ordering must separate a release from its own patch
    /// letters, which is the whole reason [`TmuxVersion`] exists: the
    /// check this replaced compared `(major, minor)` and could not tell
    /// the regression-tested 3.7c from a 3.7 nobody has run Farhelm on.
    ///
    /// Pins the two orderings that are easy to get wrong — a bare release
    /// sorts BEFORE its first patch letter (tmux shipped 3.7 then 3.7a),
    /// and the minor number is numeric, not textual, so 3.10 is newer
    /// than 3.9 rather than older.
    #[farhelm_testtrace::test]
    fn version_ordering_separates_patch_letters_and_compares_minors_numerically() {
        let parse = |line: &str| parse_tmux_version(line).expect("a well-formed version");
        assert!(parse("tmux 3.7") < parse("tmux 3.7a"));
        assert!(parse("tmux 3.7a") < parse("tmux 3.7b"));
        assert!(parse("tmux 3.7b") < parse("tmux 3.7c"));
        assert!(parse("tmux 3.7c") < parse("tmux 3.8"));
        assert!(parse("tmux 3.9") < parse("tmux 3.10"));
        assert!(parse("tmux 3.10") < parse("tmux 4.0"));
        assert_eq!(parse("tmux 3.7c"), TMUX_FLOOR);
    }

    /// Parsing is the trust boundary in front of the floor, so what it
    /// REFUSES matters as much as what it reads: every shape here would
    /// otherwise have to be guessed at, and a wrong guess admits an
    /// unaudited substrate. Also pins the round-trip through `Display`,
    /// since the refusal message and the release-pin lockstep test both
    /// compare printed versions.
    #[farhelm_testtrace::test]
    fn version_parsing_refuses_everything_it_cannot_read_exactly() {
        for line in ["tmux 3.7b", "tmux 3.7", "tmux 4.0", "tmux 3.10"] {
            let parsed = parse_tmux_version(line).expect("a well-formed version");
            assert_eq!(format!("tmux {parsed}"), line, "Display must round-trip");
        }
        // A trailing newline is the shape tmux actually emits, so it must
        // parse identically to the trimmed line.
        assert_eq!(
            parse_tmux_version("tmux 3.7c\n").expect("tmux's own output"),
            TMUX_FLOOR
        );
        // No version token at all, a development build, a decorated
        // distro string, and a suffix that is not tmux's single lowercase
        // patch letter.
        for line in [
            "",
            "tmux",
            "not-a-version",
            "tmux next-3.8",
            "tmux 3.7-rc",
            "tmux 3.7ab",
            "tmux 3.7B",
            "tmux 3",
            "tmux .7",
            "tmux 3.a",
        ] {
            assert!(
                parse_tmux_version(line).is_err(),
                "{line:?} must not parse as a version"
            );
        }
        // Whole-line shapes an earlier token-only parser read as clean
        // releases. Each one is a way an unaudited or forged substrate
        // could have been classified as the pinned build — the wrong
        // program answering, a vendor decoration, or a non-canonical
        // spelling that would then print back differently than it came
        // in. `tmux 3.8-rc` and `tmux next-3.8` above are the same
        // refusal by policy rather than by accident: see
        // [`parse_tmux_version`] on why tmux's official development and
        // release-candidate stages are out of contract.
        for line in [
            "not-tmux 3.7c",
            "TMUX 3.7c",
            "tmux3.7c",
            " tmux 3.7c",
            "tmux 3.7c vendor-patch",
            "tmux 3.7c\ntmux 3.6",
            "tmux 3.7c\n\n",
            "tmux 3.7c\r\n",
            "tmux 3.7c ",
            "tmux +3.7c",
            "tmux 3.+7c",
            "tmux -3.7c",
            "tmux 03.7c",
            "tmux 3.07c",
            "tmux 3.8-rc",
            "tmux 3.8-rc2",
        ] {
            assert!(
                parse_tmux_version(line).is_err(),
                "{line:?} must not parse as a version"
            );
        }
    }

    /// The floor's THREE outcomes, pinned as values rather than as "the
    /// call returned Ok".
    ///
    /// The version this replaced asserted only success, so it stayed green
    /// if the newer-than-pinned warning were deleted, if it fired at the
    /// exact pinned version, or if a later patch letter of the pinned
    /// release were mishandled. Those are the three ways the policy can go
    /// wrong quietly: an unaudited substrate that says nothing, and noise
    /// on the one combination that is actually blessed. Classifying into
    /// an enum is what makes them assertable without installing a
    /// process-global tracing subscriber, which would collide with every
    /// other test in the binary.
    #[farhelm_testtrace::test]
    fn classification_is_silent_at_the_pin_and_flags_everything_newer() {
        let at = |line: &str| {
            classify_tmux_version(parse_tmux_version(line).expect("a well-formed version"))
        };
        assert_eq!(at("tmux 3.7c"), TmuxSupport::AtFloor);
        // The next patch letter of the SAME release is still not the
        // build the regression suites run against.
        assert_eq!(at("tmux 3.7d"), TmuxSupport::AbovePin);
        assert_eq!(at("tmux 3.8"), TmuxSupport::AbovePin);
        assert_eq!(at("tmux 4.0"), TmuxSupport::AbovePin);
        assert_eq!(at("tmux 3.7b"), TmuxSupport::BelowFloor);
        assert_eq!(at("tmux 3.6"), TmuxSupport::BelowFloor);
    }

    /// The floor and the release pin must not be able to drift apart.
    ///
    /// [`TMUX_FLOOR`] means "the version Farhelm's regression suites
    /// actually run against", and the build that gets regression-tested is
    /// the one `.github/release/source-pins.env` names. Nothing else
    /// couples the two files, so bumping the pinned tarball without
    /// bumping the floor would silently leave the floor pointing at a
    /// build the project no longer ships — and bumping the floor alone
    /// would refuse the build provisioning installs. Failing here is the
    /// intended way to learn that the other half of the bump is missing.
    ///
    /// EVERY assignment is collected and exactly one is required, rather
    /// than reading the first. The pin file is SOURCED by shell scripts,
    /// where the LAST assignment wins, so an old line left above a new one
    /// would leave this test comparing the floor against a version nothing
    /// actually builds — the precise drift it exists to catch, dressed as
    /// a pass.
    #[farhelm_testtrace::test]
    fn floor_and_release_pin_cannot_drift() {
        // Reaching outside the crate with a relative `include_str!` is
        // fine here: this is a workspace-only repository, never packaged
        // or published as a standalone crate.
        const SOURCE_PINS: &str = include_str!("../../../.github/release/source-pins.env");
        let pinned: Vec<&str> = SOURCE_PINS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("TMUX_VERSION="))
            .collect();
        assert_eq!(
            pinned.len(),
            1,
            "source-pins.env must declare TMUX_VERSION exactly once, found {pinned:?}"
        );
        assert_eq!(
            pinned[0],
            TMUX_FLOOR.to_string(),
            "TMUX_FLOOR and the pinned tmux release must be bumped together"
        );
    }

    /// Validate a fixture path for insertion into an outer shell's single quotes.
    ///
    /// The returned text is not shell-escaped: callers must supply the quotes.
    /// Spaces and shell syntax are safe only inside those quotes; nested scripts
    /// receive the path as a positional argument rather than as executable text.
    fn pidfile_arg(path: &Path) -> String {
        let text = path.display().to_string();
        assert!(
            !text.contains('\''),
            "fixture paths must not contain a quote: {text}"
        );
        text
    }

    /// Write one executable shell script fixture and hand back its path.
    ///
    /// `farhelm helm setup` points [`probe_tmux`] at whatever the operator
    /// named or `PATH` produced, so the probe's behaviour against programs
    /// that are NOT tmux is production behaviour and needs real child
    /// processes to exercise.
    #[cfg(unix)]
    fn probe_fixture(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;

        let path = dir.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o755)
            .open(&path)
            .expect("fixture script");
        // The warm-up guard is the first thing the script does, so the
        // ETXTBSY check below costs nothing even for a fixture whose whole
        // point is to sleep for two minutes or flood its stdout.
        write!(
            file,
            "#!/bin/sh\ncase \"$1\" in --probe-warmup) exit 0;; esac\n{body}\n"
        )
        .expect("write fixture");
        drop(file);
        // Exec the fresh script once before returning it: writing an
        // executable and immediately spawning it races ETXTBSY against any
        // other thread's fork window, and every caller here spawns.
        for _ in 0..200 {
            match std::process::Command::new(&path)
                .arg("--probe-warmup")
                .output()
            {
                Ok(_) => return path,
                Err(error) if error.raw_os_error() == Some(26) => {
                    // sleep-ok: retry only ETXTBSY while an inherited writer closes; successful exec is the fixture boundary.
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("fixture {name} is not runnable: {error}"),
            }
        }
        panic!("fixture {name} stayed busy");
    }

    /// The probe is pointed at operator-supplied and PATH-supplied
    /// executables, so a candidate that hangs or floods must produce a
    /// refusal rather than wedging or killing the CLI. Both budgets are
    /// exercised with real children.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_candidate_that_hangs_or_floods_is_refused_within_the_budget() {
        let dir = tempfile::tempdir().expect("fixture dir");

        // Sleeps well past the deadline. The probe must return early, and
        // the elapsed time proves it did not simply wait for the child.
        let hanging = probe_fixture(dir.path(), "hangs", "sleep 120");
        let started = std::time::Instant::now();
        let error = probe_tmux(&hanging).expect_err("a hanging candidate is not an answer");
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
        assert!(error.to_string().contains("did not answer -V"), "{error}");
        assert!(
            started.elapsed() < PROBE_DEADLINE + Duration::from_secs(5),
            "the probe waited {:?}, past its own deadline",
            started.elapsed()
        );

        // Writes far more than the capture limit. `head` keeps the fixture
        // from depending on how the probe closes its end of the pipe.
        let flooding = probe_fixture(dir.path(), "floods", "yes farhelm-flood | head -c 200000");
        let error = probe_tmux(&flooding).expect_err("a flood is not a version");
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
        assert!(error.to_string().contains("more than"), "{error}");
    }

    /// A descendant that left the probe's process group cannot stall the
    /// probe either.
    ///
    /// Why it matters: the group kill cannot reach such a descendant, so it
    /// keeps the inherited stdout open, and the probe used to join its
    /// reader thread without a limit, hanging `farhelm helm setup` or the
    /// desktop app's startup check for as long as the descendant lived.
    /// `set -m` gives the fixture's background job its own process group,
    /// the same escape a daemonizing wrapper performs. Spec: the probe still
    /// returns `Overran` within its budget.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_descendant_outside_the_group_cannot_stall_the_probe() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let pidfile = dir.path().join("escaped.pid");
        let leader_file = dir.path().join("leader.pid");
        // Run under bash, not the script's /bin/sh: dash (Linux's usual sh)
        // cannot enable job control without a terminal, and would silently
        // leave the background job in the probe's group, where the group
        // kill closes the pipes and the test proves nothing. `exec` keeps
        // the probe's own process as the leader, whose pid is recorded so
        // the escape itself can be checked below.
        let escaping = probe_fixture(
            dir.path(),
            "escapes",
            &format!(
                "exec bash -c 'printf \"%s\\n\" \"$$\" > \"$1\"; set -m; sleep 120 & printf \"%s\\n\" \"$!\" > \"$2\"; printf \"tmux {TMUX_FLOOR}\\n\"' bash '{leader}' '{pid}'",
                leader = pidfile_arg(&leader_file),
                pid = pidfile_arg(&pidfile),
            ),
        );

        let (done, answer) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(probe_tmux(&escaping));
        });
        let result = answer.recv_timeout(Duration::from_secs(20));
        let read_pid = |path: &Path| -> Option<i32> {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|pid| pid.trim().parse().ok())
        };
        let escaped = read_pid(&pidfile);
        let leader = read_pid(&leader_file);
        // SAFETY: getpgid only reads the process table.
        let escaped_group = escaped.map(|pid| unsafe { libc::getpgid(pid) });
        if let Some(pid) = escaped {
            // SAFETY: failure-safe cleanup of the fixture's own descendant,
            // which the probe's group kill deliberately could not reach.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        let error = result
            .expect("the probe must return within its budget despite the escaped descendant")
            .expect_err("a held-open pipe is not an answer");
        assert!(
            escaped_group.is_some_and(|group| group > 0 && Some(group) != leader),
            "fixture premise: the descendant left the probe's process group \
             (group {escaped_group:?}, probe leader {leader:?})"
        );
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
    }

    /// The probe must not leave anything of its own running.
    ///
    /// The nastiest shape is not a slow candidate but a fast one that
    /// FORKS: the leader prints a perfectly good version line and exits,
    /// while the descendant it left behind holds the inherited stdout open
    /// for two minutes. The bounded collection alone would return an
    /// overrun and walk away from both the descendant and the reader
    /// thread blocked on that pipe, so a machine that probes such a
    /// candidate repeatedly accumulates one of each per refusal.
    ///
    /// The whole probe therefore runs in its own process group, and an
    /// overrun kills the group. This proves it: the descendant records its
    /// pid, and after the probe returns that pid is gone.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_descendant_holding_the_pipes_open_is_killed_with_the_group() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let pidfile = dir.path().join("grandchild.pid");
        // The `sleep` inherits stdout; the leader records its pid, prints
        // a version, and exits immediately.
        let forking = probe_fixture(
            dir.path(),
            "forks",
            &format!(
                "sleep 120 & printf '%s\\n' \"$!\" > '{}'; printf 'tmux {TMUX_FLOOR}\\n'",
                pidfile_arg(&pidfile)
            ),
        );

        let started = std::time::Instant::now();
        let error = probe_tmux(&forking).expect_err("a held-open pipe is not an answer");
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
        assert!(error.to_string().contains("still held open"), "{error}");
        assert!(
            started.elapsed() < PROBE_DEADLINE,
            "collection must be bounded by the drain budget, not the deadline: {:?}",
            started.elapsed()
        );

        let pid: i32 = std::fs::read_to_string(&pidfile)
            .expect("the fixture recorded its descendant")
            .trim()
            .parse()
            .expect("a pid");
        // Reparenting and reaping are not instant, so allow a moment for
        // the kill to be collected — but only a moment: the point is that
        // the process is gone rather than merely doomed.
        let gone = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            // SAFETY: signal 0 performs the permission and existence check
            // without delivering anything.
            let alive = unsafe { libc::kill(pid, 0) } == 0;
            if !alive {
                break;
            }
            assert!(
                std::time::Instant::now() < gone,
                "the descendant holding the probe's stdout survived the probe"
            );
            // sleep-ok: a delivered kill does not imply completed reaping; poll the recorded descendant's existence.
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The probe's ordinary verdicts, each with the payload a refusal
    /// message needs: which binary answered, what it printed, and where
    /// its version sits against the floor.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_probe_reports_the_program_version_and_what_a_non_tmux_printed() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let root = dir.path().to_path_buf();

        let at_floor = probe_fixture(&root, "at-floor", &format!("printf 'tmux {TMUX_FLOOR}\\n'"));
        let probe = probe_tmux(&at_floor).expect("a floor build is an answer");
        assert_eq!(probe.program, at_floor);
        assert_eq!(probe.version, TMUX_FLOOR);
        assert_eq!(probe.support, TmuxSupport::AtFloor);

        let old = probe_fixture(&root, "old", "printf 'tmux 3.4\\n'");
        let probe = probe_tmux(&old).expect("an old build still answers");
        assert_eq!(probe.support, TmuxSupport::BelowFloor);
        assert_eq!(probe.version.to_string(), "3.4");

        // A non-zero exit reports STDERR, which is where a program that is
        // not tmux says what it is.
        let failing = probe_fixture(&root, "failing", "echo 'not a multiplexer' >&2; exit 3");
        let error = probe_tmux(&failing).expect_err("a failing candidate is not a version");
        assert!(
            matches!(&error, TmuxProbeError::Unparseable(printed) if printed == "not a multiplexer"),
            "{error:?}"
        );

        let decorated = probe_fixture(&root, "decorated", "printf 'tmux 3.7c vendor-patch\\n'");
        let error = probe_tmux(&decorated).expect_err("a decorated line is not a release");
        assert!(
            error.to_string().contains("tmux 3.7c vendor-patch"),
            "{error}"
        );

        assert!(matches!(
            probe_tmux(Path::new("/nonexistent/tmux")),
            Err(TmuxProbeError::NotRunnable(_))
        ));
    }

    /// PATH discovery must behave like `execvp`: an entry that merely
    /// LOOKS executable but cannot be spawned is skipped, not fatal. A
    /// `noexec` mount or a group-only execute bit produces exactly that
    /// shape, and stopping there would hide a perfectly good tmux later on
    /// PATH. Empty and relative entries are dropped as well — the current
    /// directory, however it is spelled, is not something to pin into a
    /// unit file.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn path_candidates_are_offered_in_order_without_empty_entries() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        // Looks executable, is not: a directory-less shebang is the
        // cheapest reproduction of "spawns and fails".
        probe_fixture(&first, "tmux", "true");
        std::fs::write(first.join("tmux"), b"#!/nonexistent/interpreter\n").unwrap();
        let usable = probe_fixture(&second, "tmux", &format!("printf 'tmux {TMUX_FLOOR}\\n'"));

        // A relative entry that really reaches an executable tmux from this
        // test process's working directory (read, never changed): without
        // the absolute-only filter it would be the first candidate.
        let reachable = dir.path().join("reachable-through-relative");
        std::fs::create_dir_all(&reachable).unwrap();
        probe_fixture(
            &reachable,
            "tmux",
            &format!("printf 'tmux {TMUX_FLOOR}\\n'"),
        );
        let cwd = std::env::current_dir().unwrap();
        let mut relative = std::path::PathBuf::new();
        for _ in cwd.components().skip(1) {
            relative.push("..");
        }
        let relative = relative.join(reachable.strip_prefix("/").unwrap());
        assert!(
            relative.join("tmux").is_file(),
            "premise: {}",
            relative.display()
        );
        let path = std::env::join_paths([
            std::path::PathBuf::new(),
            std::path::PathBuf::from("."),
            relative,
            first.clone(),
            second.clone(),
        ])
        .unwrap();
        let candidates: Vec<_> = candidates_on_path(&path, "tmux").collect();
        assert_eq!(candidates, [first.join("tmux"), usable.clone()]);
        assert_eq!(find_on_path(&path, "tmux"), Some(first.join("tmux")));
        // The first candidate is the one `find_on_path` reports and the
        // one a spawning caller must be able to skip.
        assert!(matches!(
            probe_tmux(&candidates[0]),
            Err(TmuxProbeError::NotRunnable(_))
        ));
        assert!(probe_tmux(&candidates[1]).is_ok());
    }

    /// The override's precedence is a user-facing contract: `--tmux` beats
    /// `FARHELM_TMUX` beats PATH, so an operator can override a unit
    /// file's environment from the command line without editing it.
    ///
    /// Both inputs are injected rather than read from the process, which
    /// is deliberate — this repo's tests never mutate the test process's
    /// environment, and a shared `FARHELM_TMUX` would leak into every
    /// concurrently running harness.
    #[farhelm_testtrace::test]
    fn the_flag_beats_the_environment_which_beats_path() {
        use std::ffi::OsStr;

        let flag = PathBuf::from("/opt/flag/tmux");
        let env = OsStr::new("/opt/env/tmux");
        assert_eq!(
            resolve_tmux_program(Some(&flag), Some(env)),
            PathBuf::from("/opt/flag/tmux")
        );
        assert_eq!(
            resolve_tmux_program(Some(&flag), None),
            PathBuf::from("/opt/flag/tmux")
        );
        assert_eq!(
            resolve_tmux_program(None, Some(env)),
            PathBuf::from("/opt/env/tmux")
        );
        assert_eq!(
            resolve_tmux_program(None, None),
            PathBuf::from(DEFAULT_TMUX_PROGRAM)
        );
        // An empty override is what a unit file or profile writes to mean
        // "no override"; the other reading spawns a program with no name.
        assert_eq!(
            resolve_tmux_program(None, Some(OsStr::new(""))),
            PathBuf::from(DEFAULT_TMUX_PROGRAM)
        );
    }

    /// Write an executable shell script, for the tmux stand-ins below.
    ///
    /// Factored out because several override tests need a program that
    /// exists, runs, and answers in a controlled way; a non-executable
    /// file would be refused by the OS long before the behavior under
    /// test.
    #[cfg(unix)]
    fn write_executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).expect("write the stand-in");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("make the stand-in executable");
    }

    /// The refusal names a resolvable path even when the program was a
    /// bare name off PATH — which is the case that needs it most, since
    /// "some tmux on PATH is too old" leaves the reader with nothing to
    /// edit. A name that already carries a separator is reported as
    /// written, and an unresolvable one falls back to the bare spelling
    /// rather than inventing a path that does not exist.
    ///
    /// The shadow case is the one with teeth: `execvp` walks PAST a
    /// non-executable `tmux` and runs a later entry, so a lookup that
    /// stopped at the first file by name would print a path that never
    /// ran. That mismatch turns the diagnostic from helpful into
    /// actively misleading — the reader edits or deletes a file that had
    /// nothing to do with the version they were refused for.
    ///
    /// Unix-only: the shadow it pins is an execute-bit distinction, which
    /// [`is_executable_file`] cannot make on a platform without one.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_bare_program_name_is_reported_as_the_path_entry_it_resolves_to() {
        use std::ffi::OsString;

        let shadow = tempfile::tempdir().expect("tempdir");
        let dir = tempfile::tempdir().expect("tempdir");
        // Same name, earlier on PATH, not executable: the OS skips it.
        std::fs::write(shadow.path().join("tmux"), b"not a program\n")
            .expect("write the non-executable shadow");
        let binary = dir.path().join("tmux");
        write_executable(&binary, "#!/bin/sh\n");
        let path_var =
            std::env::join_paths([shadow.path(), dir.path()]).expect("a joinable PATH fixture");

        assert_eq!(
            program_display_path(Path::new("tmux"), Some(&path_var)),
            binary,
            "a non-executable shadow must not be reported as the tmux that answered"
        );
        assert_eq!(
            program_display_path(Path::new("/opt/homebrew/bin/tmux"), Some(&path_var)),
            PathBuf::from("/opt/homebrew/bin/tmux")
        );
        assert_eq!(
            program_display_path(Path::new("absent-tmux"), Some(&path_var)),
            PathBuf::from("absent-tmux")
        );
        assert_eq!(
            program_display_path(Path::new("tmux"), None),
            PathBuf::from("tmux")
        );
        // A PATH holding only the shadow resolves to nothing, and the
        // bare spelling is a better answer than a path that was skipped.
        let shadow_only = OsString::from(shadow.path());
        assert_eq!(
            program_display_path(Path::new("tmux"), Some(&shadow_only)),
            PathBuf::from("tmux")
        );
    }
}
