//! Shared /tmp discipline for Farhelm's test harnesses: one naming scheme,
//! one liveness protocol, one sweep.
//!
//! Every harness that builds state under /tmp — the Playwright stack script
//! and the Rust integration tests — leaks that state when it is killed
//! without running its cleanup (SIGKILL, OOM, a crashed shell). One long day
//! of killed runs accumulated 423 orphans (~22 GB) and hit disk-full. The
//! old defense was a shell `find` in e2e/start-stack.sh that matched only
//! its own `fh-e2e.*` prefix and only ran when a new stack started; the
//! Rust tests' `tempfile::tempdir()` dirs (`.tmpXXXXXX`) matched nothing
//! and were never reclaimed.
//!
//! This crate replaces that with three cooperating pieces:
//!
//! - A **prefix family** ([`PREFIXES`]): every harness state dir lives
//!   directly under /tmp with one of these prefixes plus a random suffix,
//!   so a single readdir enumerates everything sweepable. Directly under
//!   /tmp on purpose — supervisor sockets live inside these dirs and unix
//!   socket paths are limited to ~108 bytes (SUN_LEN) — and always with a
//!   random suffix, never a fixed name another local user could
//!   pre-create.
//! - A **lock protocol** ([`LOCK_FILE_NAME`]): each run holds an exclusive
//!   flock on a lock file inside its dir for its whole lifetime. An flock
//!   dies with its holder, so "is this run alive" stays answerable after a
//!   SIGKILL — which is exactly where a PID file would lie (PID reuse,
//!   reboots) and where the old pure age gate had to wait an hour.
//! - A **sweep** ([`sweep`]): reaps dead runs, kills their orphaned tmux
//!   servers, and runs from every harness entry point — the stack script
//!   invokes `farhelm internal sweep-test-state` at startup, and the Rust
//!   tests sweep once per process through [`tempdir`] — so a long-lived
//!   session cleans up after its dead predecessors instead of
//!   accumulating without bound.
//!
//! Product code must never depend on this crate; it exists for harnesses
//! and for the hidden `internal` CLI namespace that scripts call.

use std::fs::{self, File};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Where the family lives. Hardcoded rather than honoring TMPDIR because
/// e2e/start-stack.sh hardcodes /tmp too (short socket paths under
/// SUN_LEN, no predictable-name pre-creation) and the sweep must look
/// where creators actually create; a TMPDIR-honoring sweep on a machine
/// with TMPDIR set would silently stop matching the script's dirs.
pub const TMP_ROOT: &str = "/tmp";

/// The Playwright stack script's prefix (e2e/start-stack.sh's `mktemp`),
/// kept unchanged from the pre-crate era so orphans from before this
/// crate existed remain sweepable.
const PLAYWRIGHT_PREFIX: &str = "fh-e2e.";

/// Prefix used by [`tempdir`] for the Rust integration tests' dirs.
const RUST_HARNESS_PREFIX: &str = "fh-it.";

/// The prefix family: the complete set of prefixes the sweep considers
/// its own, built from the per-creator constants above so a creator
/// prefix cannot drift away from the sweep's view of the family. This is
/// the single authority — there is deliberately no parallel list in
/// shell.
pub const PREFIXES: &[&str] = &[PLAYWRIGHT_PREFIX, RUST_HARNESS_PREFIX];

/// Length of the random suffix both creators append (`mktemp`'s XXXXXX
/// and tempfile's default are both six alphanumerics). The sweep requires
/// exactly this shape after the prefix, so a hand-made same-owner dir
/// like `fh-it.saved-results` is out of bounds even though it starts with
/// a family prefix — the sweep only ever judges names its creators could
/// actually have generated.
const RANDOM_SUFFIX_LEN: usize = 6;

/// The lock file each run creates inside its state dir and holds an
/// exclusive flock on for its whole lifetime. A held lock proves the run
/// is live, no matter how old the dir; a free lock makes the dir
/// reapable only once past [`SweepPolicy::grace`], which covers a
/// creator still between lock creation and acquisition. A dir with no
/// lock file at all predates the protocol (or its creator died between
/// mkdir and lock creation) and falls back to [`SweepPolicy::backstop`].
pub const LOCK_FILE_NAME: &str = "fh-run.lock";

/// How deep [`reap`] looks for orphaned `tmux.sock` sockets inside a dead
/// dir. Depth 3 covers everything the harnesses build today (the stack
/// script's nested `remote/tmux.sock` is depth 2) with headroom, while
/// keeping the walk bounded on a dir full of unexpected junk. A future
/// layout deeper than this must raise the constant or its servers leak.
const TMUX_SOCKET_WALK_DEPTH: usize = 3;

/// How long one `tmux kill-server` gets before the sweep gives up on it.
/// A deadline because the sweep must never hang: a wedged server (or a
/// fake listener squatting on the socket path) can accept the connection
/// and simply never answer, and one such entry must not block every
/// subsequent harness startup.
const TMUX_KILL_DEADLINE: Duration = Duration::from_secs(5);

/// Ceiling on the time ONE sweep pass spends on tmux kills in total.
/// The per-kill deadline above bounds one wedged socket; this bounds a
/// /tmp full of them, since neither the directory count nor the socket
/// count is otherwise limited. A pass that exhausts it keeps reclaiming
/// directories without further kill attempts — disk reclamation is the
/// contract, and a wedged server can be retried by the next sweep.
const TMUX_KILL_TOTAL_BUDGET: Duration = Duration::from_secs(30);

/// The sweep's time thresholds, injectable so tests can use zero instead
/// of forging mtimes or mutating the clock.
pub struct SweepPolicy {
    /// Minimum age (lock-file mtime) before a dir whose lock file exists
    /// but is NOT held gets reaped. This covers the creator's window
    /// between creating the lock file and acquiring the flock: the sweep
    /// checks this age BEFORE contending for the lock, so a fresh
    /// unlocked lock file is never mistaken for a dead run and its
    /// mid-startup directory is never reaped. Runs are "dead
    /// the moment the holder dies", so this is minutes, not the old
    /// scheme's hour.
    pub grace: Duration,
    /// Minimum age (dir mtime) before a dir with NO lock file gets
    /// reaped. This is the old pure age gate, kept as the backstop for
    /// legacy dirs and for creators killed before the lock existed. It
    /// must stay comfortably above any legitimate mkdir-to-lock window
    /// and above suspicion of racing a concurrent run mid-setup.
    pub backstop: Duration,
}

impl Default for SweepPolicy {
    fn default() -> Self {
        Self {
            grace: Duration::from_secs(5 * 60),
            backstop: Duration::from_secs(60 * 60),
        }
    }
}

/// What one [`sweep`] pass did, for reporting. `live` counts dirs skipped
/// because their lock was positively held — the interesting number when
/// debugging "why was my dir not reaped". Entries skipped because
/// something went wrong (unreadable lock, flock error) are warned to
/// stderr and counted nowhere: they are neither proof of life nor safe
/// to reap.
pub struct SweepOutcome {
    /// Dirs actually removed, in encounter order.
    pub reaped: Vec<PathBuf>,
    /// Dirs whose lock was held by a running harness.
    pub live: usize,
}

/// Reap dead harness state dirs under `root`.
///
/// Best-effort by contract: a broken sweep must never block testing, so
/// every per-entry failure is a skipped entry (warned to stderr where
/// surprising), never an error or a panic to the caller.
///
/// Concurrency: two sweeps racing over the same entries are safe.
/// Lock-bearing dirs are reaped while holding their flock, so those
/// reaps serialize; lockless (legacy) dirs have nothing to lock, and a
/// lost removal race there is simply tolerated. What flock cannot close
/// is the pathname gap between this sweep's checks and its removal — a
/// concurrent sweep can remove a dir first and ANOTHER local user can
/// then recreate the freed name. The residual exposure is bounded: the
/// lock open refuses symlinks and non-regular files, [`reap`] re-checks
/// ownership at its own start, and `remove_dir_all` does not traverse a
/// symlink root (it unlinks the link itself, leaving the target's
/// contents alone) — accepted for what is, by construction, disposable
/// same-user harness state under sticky /tmp.
///
/// Four guards decide whether an entry is even a candidate, each
/// load-bearing (inherited from the shell sweep this replaces):
///
/// - The name must be one a creator could have generated: family prefix
///   plus exactly the random suffix shape ([`RANDOM_SUFFIX_LEN`]). The
///   sweep never matches anything a non-farhelm process creates, never
///   chases `tempfile`-default `.tmpXXXXXX` names, and never judges a
///   hand-made `fh-it.something` stash.
/// - `symlink_metadata` must say directory — a planted symlink with a
///   matching name must not steer the reaper (or its tmux kill) at
///   someone else's tree.
/// - The owner must be this process's euid — /tmp is shared; other
///   users' dirs are not ours to judge.
/// - The liveness protocol above must say dead — held flock means live,
///   no matter how old.
pub fn sweep(root: &Path, policy: &SweepPolicy) -> SweepOutcome {
    // SAFETY: geteuid has no preconditions and cannot fail.
    sweep_as_owner(root, policy, unsafe { libc::geteuid() })
}

/// [`sweep`] with the owning uid injected — the testable core. The euid
/// guard is the primary protection against deleting another user's
/// matching /tmp entry, and injecting the expected owner is the only way
/// to prove the guard without privileged `chown`.
fn sweep_as_owner(root: &Path, policy: &SweepPolicy, owner: libc::uid_t) -> SweepOutcome {
    let mut outcome = SweepOutcome {
        reaped: Vec::new(),
        live: 0,
    };
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            diag(format!("cannot read {}: {error}", root.display()));
            return outcome;
        }
    };
    let now = SystemTime::now();
    // One tmux-kill time budget for the WHOLE pass, not per socket: the
    // per-kill deadline bounds one wedged socket, but a /tmp full of them
    // times an unbounded directory count would still stall the sweep for
    // minutes. Once spent, remaining reaps skip their kill attempts and
    // keep reclaiming disk — reclaiming disk is the contract; a wedged
    // server is a process leak the next sweep can retry.
    let tmux_kill_budget = std::time::Instant::now() + TMUX_KILL_TOTAL_BUDGET;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                diag(format!("error listing {}: {error}", root.display()));
                continue;
            }
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_family_name(name) {
            continue;
        }
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            // NotFound is an ordinary race (a concurrent sweep or the
            // owner removed it); anything else deserves a trace, because
            // a silently skipped candidate is stale state with no
            // explanation.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                diag(format!("cannot stat {}: {error}", path.display()));
                continue;
            }
        };
        if !meta.is_dir() || meta.uid() != owner {
            continue;
        }
        match open_lock_file(&path.join(LOCK_FILE_NAME)) {
            Ok(Some(lock)) => {
                // Age gate FIRST, contention second: a fresh unlocked
                // lock file belongs to a run mid-startup (created the
                // file, not yet flocked), and contending would let the
                // sweep acquire the lock and reap a directory whose
                // creator is about to use it. Old locks carry no such
                // hazard — their creator either holds them (live) or
                // died (reapable).
                let stamp = lock.metadata().ok().and_then(|m| m.modified().ok());
                if is_younger_than(now, stamp, policy.grace) {
                    continue;
                }
                match try_flock(&lock) {
                    FlockResult::Contended => outcome.live += 1,
                    FlockResult::Failed(error) => {
                        // Not proof of life, not license to reap: an
                        // flock failure (ENOLCK, EIO) says the HOST is
                        // unwell, and silently doing either would hide
                        // that.
                        diag(format!("cannot lock {}: {error}", path.display()));
                    }
                    // Dead — reap while holding the flock so concurrent
                    // sweeps serialize behind us instead of
                    // double-reaping.
                    FlockResult::Acquired => reap(&path, owner, tmux_kill_budget, &mut outcome),
                }
            }
            Ok(None) => {
                if is_younger_than(now, meta.modified().ok(), policy.backstop) {
                    continue;
                }
                reap(&path, owner, tmux_kill_budget, &mut outcome);
            }
            Err(error) => {
                // A lock we cannot even open (permissions, a FIFO or
                // symlink planted at the name, fd exhaustion) makes the
                // entry unjudgeable. Skipping is the safe move, but a
                // silent skip would let stale state accumulate with no
                // trace of why — so say so.
                diag(format!(
                    "cannot inspect lock in {}: {error}",
                    path.display()
                ));
            }
        }
    }
    outcome
}

/// Best-effort stderr diagnostic. A plain `eprintln!` would PANIC if
/// stderr cannot be written, turning "could not report a skipped entry"
/// into a dead test process — and, inside [`tempdir`]'s sweep closure, a
/// poisoned `Once` that fails every later call. Cleanup must never be
/// louder than what it cleans.
fn diag(message: String) {
    use io::Write as _;
    let _ = writeln!(io::stderr().lock(), "farhelm-teststate: {message}");
}

/// True when `name` is something a family creator could actually have
/// generated: a family prefix followed by exactly the random suffix both
/// creators produce. See the [`RANDOM_SUFFIX_LEN`] docs for why the
/// suffix shape is part of the match.
fn is_family_name(name: &str) -> bool {
    PREFIXES.iter().any(|prefix| {
        name.strip_prefix(prefix).is_some_and(|suffix| {
            suffix.len() == RANDOM_SUFFIX_LEN && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        })
    })
}

/// Open a lock file for flocking, refusing anything that is not a plain
/// regular file. `Ok(None)` means the file does not exist (the legacy /
/// killed-early case); every other failure is an error the caller should
/// surface.
///
/// `O_NOFOLLOW` and `O_NONBLOCK` are the load-bearing flags: a symlink
/// planted at the lock name must not lead the sweep elsewhere, and a
/// FIFO planted there must not hang the open forever — a blocking
/// `File::open` on a writerless FIFO never returns, which would turn the
/// best-effort sweep into a harness-wide hang. The type is then verified
/// on the OPENED descriptor (fstat), not by a separate racy path lookup.
fn open_lock_file(path: &Path) -> io::Result<Option<File>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(io::Error::other("lock path is not a regular file"));
    }
    Ok(Some(file))
}

/// Print `outcome` to stderr, one line per reaped dir. stderr because the
/// two callers (the internal subcommand, [`tempdir`]'s once-per-process
/// sweep) both run where stdout may belong to something else — a test
/// runner, a script whose stdout is parsed.
pub fn report(outcome: &SweepOutcome) {
    for dir in &outcome.reaped {
        diag(format!("reaped stale test state {}", dir.display()));
    }
}

/// True when `stamp` is missing, unreadable, or newer than `window` ago.
/// Missing/unreadable counts as young: when we cannot prove age, we do
/// not delete. An age exactly equal to the window is NOT younger — with
/// a zero window everything is immediately eligible, which is what the
/// tests rely on.
fn is_younger_than(now: SystemTime, stamp: Option<SystemTime>, window: Duration) -> bool {
    match stamp.and_then(|stamp| now.duration_since(stamp).ok()) {
        Some(age) => age < window,
        // A stamp in the future (clock skew) lands here too, and is
        // likewise not evidence of staleness.
        None => true,
    }
}

/// The three ways a non-blocking exclusive flock attempt can end.
/// Contention and failure are deliberately distinct: a held lock is
/// positive evidence of a live run, while an errno like ENOLCK says
/// nothing about the run and must not be reported as liveness.
enum FlockResult {
    Acquired,
    Contended,
    Failed(io::Error),
}

/// Try to take an exclusive, non-blocking flock, retrying interruption.
fn try_flock(file: &File) -> FlockResult {
    loop {
        // SAFETY: flock's only precondition is a valid open descriptor,
        // which `file` owns for the duration of the call.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return FlockResult::Acquired;
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            // EAGAIN == EWOULDBLOCK on every target we build for; both
            // spellings mean "someone holds it".
            Some(libc::EWOULDBLOCK) => return FlockResult::Contended,
            _ => return FlockResult::Failed(error),
        }
    }
}

/// Take an exclusive flock, waiting for it. Used by the creator.
///
/// Blocking is defense in depth rather than a hot path: a sweeper never
/// contends for a lock file younger than the grace (age is checked
/// before flocking), so a creator flocking its own brand-new file should
/// see no contention at all. If contention nonetheless happens — the
/// grace misjudged, clock skew, a future refactor — the sweeper holds
/// the lock through an entire reap, and waiting behind that beats
/// nondeterministically failing the harness run.
fn flock_blocking(file: &File) -> io::Result<()> {
    loop {
        // SAFETY: as in `try_flock` — a valid owned descriptor.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(error);
    }
}

/// Kill the dead run's tmux servers, then remove its directory.
///
/// Ownership is re-verified first: between the sweep's candidate checks
/// and this call, a concurrent sweep may have removed the dir and the
/// freed name may have been recreated by someone else (see [`sweep`]'s
/// concurrency notes). The re-check shrinks that window to the
/// unavoidable minimum.
///
/// The tmux kill comes first and covers every `tmux.sock` within the
/// bounded walk ([`TMUX_SOCKET_WALK_DEPTH`]), not just the top-level
/// one: the stack script nests a second supervisor's state (and its tmux
/// server) at `remote/tmux.sock`, and a server whose socket dir is
/// deleted out from under it lingers forever — the old shell sweep had
/// exactly that gap. The kill is best-effort with a deadline; the
/// directory is removed even if a kill fails, because reclaiming disk is
/// this crate's reason to exist and the old shell sweep made the same
/// trade (a wedged server minus its socket is a process leak, not a
/// disk leak).
fn reap(
    dir: &Path,
    owner: libc::uid_t,
    tmux_kill_budget: std::time::Instant,
    outcome: &mut SweepOutcome,
) {
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() && meta.uid() == owner => {}
        _ => return,
    }
    let mut sockets = Vec::new();
    collect_tmux_sockets(dir, TMUX_SOCKET_WALK_DEPTH, &mut sockets);
    for socket in sockets {
        if std::time::Instant::now() >= tmux_kill_budget {
            diag(format!(
                "tmux kill budget exhausted; not contacting {}",
                socket.display()
            ));
            continue;
        }
        kill_tmux_server(&socket);
    }
    match fs::remove_dir_all(dir) {
        Ok(()) => outcome.reaped.push(dir.to_path_buf()),
        // Lost a remove race with a concurrent sweep; their line, not ours.
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            diag(format!("failed to remove {}: {error}", dir.display()));
        }
    }
}

/// Run `tmux -S <socket> kill-server`, bounded by
/// [`TMUX_KILL_DEADLINE`], with a scrubbed environment.
///
/// The deadline exists because the sweep must never hang on one bad
/// socket (see the constant's docs). The environment is cleared — PATH
/// excepted, which the spawn needs to find tmux — because a tmux client
/// ships its environment to the server before it can know what is
/// actually listening; a fake listener squatting on an orphan's socket
/// path must not be handed CI credentials for free. Best-effort
/// throughout: failure to spawn, a nonzero exit, or a timeout are all
/// acceptable outcomes (the server may simply be long dead), and the
/// caller removes the directory regardless.
fn kill_tmux_server(socket: &Path) {
    let mut command = std::process::Command::new("tmux");
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    let child = command
        .arg("-S")
        .arg(socket)
        .arg("kill-server")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return };
    let deadline = std::time::Instant::now() + TMUX_KILL_DEADLINE;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            // Do not walk away from a possibly-live child on a polling
            // error: SIGKILL it and reap, so a transient wait failure
            // cannot leave a stuck client (or a zombie) behind.
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
}

/// Collect every socket named exactly `tmux.sock` under `dir`, recursing
/// into at most `depth` nested directory levels (sockets sitting IN a
/// directory at the recursion floor are still collected) and never
/// following symlinks — inside a dir we are about to delete, a symlink
/// is the one thing that could point the tmux kill outside it.
///
/// Walk failures are diagnosed rather than swallowed: a subtree we
/// cannot read may hide a socket whose server will outlive the
/// directory removal, and that deserves a trace even though the removal
/// proceeds (disk first, best-effort always).
fn collect_tmux_sockets(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(error) => {
            diag(format!("cannot walk {}: {error}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                diag(format!("error listing {}: {error}", dir.display()));
                continue;
            }
        };
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                diag(format!("cannot stat {}: {error}", path.display()));
                continue;
            }
        };
        if meta.is_dir() {
            if depth > 0 {
                collect_tmux_sockets(&path, depth - 1, out);
            }
        } else if meta.file_type().is_socket() && entry.file_name() == "tmux.sock" {
            out.push(path);
        }
    }
}

/// A harness state dir under the shared scheme: a prefixed,
/// random-suffixed, mode-0700 container directly under /tmp, with the
/// run's flock held for as long as this value lives. The drop-in
/// replacement for `tempfile::tempdir()` in the integration tests.
///
/// Drop-in includes CONTENTS: [`Self::path`] names a clean inner
/// directory, while the protocol bookkeeping (the lock file) lives one
/// level up in the container. Tests hand these paths to sessions as
/// working directories and assert on exactly what appeared in them, so a
/// visible `fh-run.lock` would be an observable behavior change, not
/// just clutter.
///
/// The lock's descriptor is close-on-exec (Rust opens all files that
/// way), so children this process spawns can never inherit it — a
/// long-lived child holding a dead parent's lock would keep killed
/// harness state falsely live, which is the orphan accumulation this
/// protocol exists to prevent. When the process dies, however abruptly,
/// the kernel drops the flock with it.
///
/// Field order is the drop order and it matters: the directory is removed
/// first, while the lock is still held, so a concurrent sweep never
/// observes this dir unlocked-but-present mid-removal.
pub struct TestDir {
    /// The clean inner dir handed to callers; lives inside `dir`.
    inner: PathBuf,
    dir: tempfile::TempDir,
    _lock: File,
}

/// The container entry [`TestDir::path`] points at. Single-character
/// because these paths carry unix sockets and every byte counts against
/// SUN_LEN's ~108.
const INNER_DIR_NAME: &str = "d";

impl TestDir {
    /// The usable (initially empty) state dir, valid for this value's
    /// lifetime. One level below the sweepable container — see the type
    /// docs.
    pub fn path(&self) -> &Path {
        &self.inner
    }

    /// Remove the directory now and report failure, mirroring
    /// `TempDir::close` — for tests whose subject is what happens when a
    /// directory disappears mid-run, where a silent drop-time failure
    /// would invalidate the test rather than merely leak. The lock is
    /// released afterwards, on return, so no sweep can observe the dir
    /// unlocked while it still exists.
    pub fn close(self) -> io::Result<()> {
        self.dir.close()
    }
}

/// Create a `fh-it.` state dir under /tmp with its lock held — and, once
/// per process, sweep dead predecessors first.
///
/// The sweep riding along is the point: it makes every test binary an
/// entry point that cleans up after killed runs, with no separate step to
/// forget. Once per process, not per call, because one pass over /tmp is
/// enough and the e2e suite creates hundreds of these.
pub fn tempdir() -> io::Result<TestDir> {
    static SWEEP_ONCE: std::sync::Once = std::sync::Once::new();
    tempdir_with_sweep(Path::new(TMP_ROOT), &SWEEP_ONCE, &SweepPolicy::default())
}

/// [`tempdir`]'s testable core: the same create-with-first-call-sweep
/// behavior against an injected root, `Once`, and policy.
fn tempdir_with_sweep(
    root: &Path,
    once: &std::sync::Once,
    policy: &SweepPolicy,
) -> io::Result<TestDir> {
    once.call_once(|| report(&sweep(root, policy)));
    tempdir_in(root)
}

/// Create one locked family dir under `root` — the creation half of the
/// protocol, without the sweep, against an injectable root.
///
/// Mode 0700 explicitly rather than trusting the process umask: these
/// dirs hold supervisor sockets and agent state, and a permissive umask
/// must not make them group- or world-accessible. The flock is taken
/// blocking, not `-NB` — belt and braces: the sweep's grace check keeps
/// sweepers from ever contending on a lock file this fresh, and if that
/// protection is somehow misjudged, waiting behind the sweeper beats
/// nondeterministically failing the harness (see [`flock_blocking`]).
pub fn tempdir_in(root: &Path) -> io::Result<TestDir> {
    let dir = tempfile::Builder::new()
        .prefix(RUST_HARNESS_PREFIX)
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(root)?;
    let lock = File::create(dir.path().join(LOCK_FILE_NAME))?;
    flock_blocking(&lock)?;
    let inner = dir.path().join(INNER_DIR_NAME);
    // 0700 like the container: redundant today (the container already
    // blocks traversal) but keeps the guarantee local instead of
    // depending on the parent's mode staying restrictive.
    let mut builder = fs::DirBuilder::new();
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&inner)?;
    Ok(TestDir {
        inner,
        dir,
        _lock: lock,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zero thresholds: every dead dir is immediately reapable, so tests
    /// exercise the liveness logic rather than waiting on clocks.
    fn instant_policy() -> SweepPolicy {
        SweepPolicy {
            grace: Duration::ZERO,
            backstop: Duration::ZERO,
        }
    }

    fn euid() -> libc::uid_t {
        // SAFETY: no preconditions.
        unsafe { libc::geteuid() }
    }

    /// A fixture "state dir" with a lock file nobody holds — the shape a
    /// SIGKILLed run leaves behind.
    fn dead_dir(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir(&dir).unwrap();
        File::create(dir.join(LOCK_FILE_NAME)).unwrap();
        dir
    }

    /// The core promise: killed runs' dirs (lock file present, flock
    /// free) are reaped once past the grace — ALL of them in one pass,
    /// not just the first, because the real workload is hundreds of
    /// orphans and a sweep that stopped early would preserve the
    /// disk-exhaustion incident. Without this the whole crate is
    /// decoration.
    #[test]
    fn sweep_reaps_every_dead_dir_in_one_pass() {
        let root = tempfile::tempdir().unwrap();
        let mut dead = vec![
            dead_dir(root.path(), "fh-it.dead01"),
            dead_dir(root.path(), "fh-it.dead02"),
            dead_dir(root.path(), "fh-e2e.dead03"),
        ];
        let mut outcome = sweep(root.path(), &instant_policy());
        dead.sort();
        outcome.reaped.sort();
        assert_eq!(outcome.reaped, dead);
        for dir in &dead {
            assert!(!dir.exists());
        }
    }

    /// The safety half of the design: a held flock makes a dir unreapable
    /// no matter how permissive the policy — this is what lets the grace
    /// be minutes instead of the old scheme's hour without ever reaping a
    /// live run.
    #[test]
    fn sweep_never_reaps_a_held_lock() {
        let root = tempfile::tempdir().unwrap();
        let live = dead_dir(root.path(), "fh-it.live01");
        let lock = File::open(live.join(LOCK_FILE_NAME)).unwrap();
        assert!(matches!(try_flock(&lock), FlockResult::Acquired));
        let outcome = sweep(root.path(), &instant_policy());
        assert!(outcome.reaped.is_empty());
        assert_eq!(outcome.live, 1);
        assert!(live.exists());
        // Released, the same dir becomes ordinary dead state.
        drop(lock);
        let outcome = sweep(root.path(), &instant_policy());
        assert_eq!(outcome.reaped, vec![live]);
    }

    /// A fresh unlocked lock file is a run mid-startup (created the file,
    /// not yet flocked), and the grace must protect it.
    #[test]
    fn grace_protects_a_freshly_created_lock_file() {
        let root = tempfile::tempdir().unwrap();
        let starting = dead_dir(root.path(), "fh-it.start1");
        let policy = SweepPolicy {
            grace: Duration::from_secs(3600),
            backstop: Duration::ZERO,
        };
        let outcome = sweep(root.path(), &policy);
        assert!(outcome.reaped.is_empty());
        assert!(starting.exists());
    }

    /// A dir with no lock file at all (legacy scheme, or killed between
    /// mkdir and lock creation) is governed by the backstop age gate, in
    /// both directions.
    #[test]
    fn backstop_governs_lockless_dirs() {
        let root = tempfile::tempdir().unwrap();
        let lockless = root.path().join("fh-e2e.old001");
        fs::create_dir(&lockless).unwrap();
        let protective = SweepPolicy {
            grace: Duration::ZERO,
            backstop: Duration::from_secs(3600),
        };
        assert!(sweep(root.path(), &protective).reaped.is_empty());
        assert!(lockless.exists());
        let outcome = sweep(root.path(), &instant_policy());
        assert_eq!(outcome.reaped, vec![lockless.clone()]);
        assert!(!lockless.exists());
    }

    /// The sweep must never touch names outside the family — that
    /// discipline is what makes running it against a shared /tmp safe,
    /// and is the explicit alternative to chasing tempfile's `.tmpXXXXXX`
    /// defaults. The family is prefix PLUS generated-suffix shape:
    /// a hand-made `fh-it.saved-results` is not the sweep's to judge.
    #[test]
    fn sweep_ignores_foreign_and_ungenerated_names() {
        let root = tempfile::tempdir().unwrap();
        let foreign = [
            ".tmpAbC123",
            "fh-unrelated",
            "fh-it-nodot",
            "fh-it.saved-results",
            "fh-it.short",
            "fh-it.toolong7",
            "fh-e2e.has.dot",
            // Right length, wrong characters: a six-byte suffix with
            // punctuation is a name no creator generates, and a matcher
            // regressed to prefix-plus-length would wrongly claim it.
            "fh-it.ab-c12",
            "fh-e2e.a_bc12",
        ];
        for name in foreign {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        assert!(sweep(root.path(), &instant_policy()).reaped.is_empty());
        for name in foreign {
            assert!(root.path().join(name).exists(), "{name} must survive");
        }
    }

    /// A planted symlink wearing a family name must not steer the reaper
    /// at its target: the sweep lstats and skips non-directories.
    #[test]
    fn sweep_skips_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("victim");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("precious"), b"data").unwrap();
        std::os::unix::fs::symlink(&target, root.path().join("fh-it.plant1")).unwrap();
        assert!(sweep(root.path(), &instant_policy()).reaped.is_empty());
        assert!(target.join("precious").exists());
        assert!(root.path().join("fh-it.plant1").symlink_metadata().is_ok());
    }

    /// The euid guard is the primary protection against deleting another
    /// user's matching /tmp entry, and this proves it does something: the
    /// same stale fixture survives when judged against a different owner
    /// and is reaped when judged against the real one.
    #[test]
    fn sweep_skips_dirs_owned_by_someone_else() {
        let root = tempfile::tempdir().unwrap();
        let dead = dead_dir(root.path(), "fh-it.owner1");
        let outcome = sweep_as_owner(root.path(), &instant_policy(), euid().wrapping_add(1));
        assert!(outcome.reaped.is_empty());
        assert!(dead.exists());
        let outcome = sweep_as_owner(root.path(), &instant_policy(), euid());
        assert_eq!(outcome.reaped, vec![dead]);
    }

    /// The best-effort contract under breakage: a missing root yields an
    /// empty outcome, and an unjudgeable lock (here: a directory planted
    /// at the lock name, which opens but fstats as non-regular) is
    /// skipped without a panic, without deletion — and WITHOUT aborting
    /// the pass: a valid stale sibling must still be reaped, or one
    /// damaged entry could block all cleanup.
    #[test]
    fn sweep_tolerates_missing_roots_and_malformed_locks() {
        let root = tempfile::tempdir().unwrap();
        let outcome = sweep(&root.path().join("does-not-exist"), &instant_policy());
        assert!(outcome.reaped.is_empty());
        assert_eq!(outcome.live, 0);

        let weird = root.path().join("fh-it.weird1");
        fs::create_dir(&weird).unwrap();
        fs::create_dir(weird.join(LOCK_FILE_NAME)).unwrap();
        let stale_sibling = dead_dir(root.path(), "fh-it.stalg1");
        let outcome = sweep(root.path(), &instant_policy());
        assert_eq!(outcome.reaped, vec![stale_sibling.clone()]);
        assert!(weird.exists());
        assert!(!stale_sibling.exists());
    }

    /// A symlink planted at the LOCK path (not the dir) must neither
    /// steer the sweep at its target nor make the dir reapable:
    /// `O_NOFOLLOW` refuses the open, the entry is skipped as
    /// unjudgeable, and both the family dir and the link's target
    /// survive. Removing `O_NOFOLLOW` would follow the link, flock the
    /// unrelated target, and reap the directory.
    #[test]
    fn sweep_refuses_a_symlinked_lock() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("innocent-file");
        fs::write(&target, b"data").unwrap();
        let trapped = root.path().join("fh-it.lnlk01");
        fs::create_dir(&trapped).unwrap();
        std::os::unix::fs::symlink(&target, trapped.join(LOCK_FILE_NAME)).unwrap();
        let outcome = sweep(root.path(), &instant_policy());
        assert!(outcome.reaped.is_empty());
        assert!(trapped.exists());
        assert_eq!(fs::read(&target).unwrap(), b"data");
    }

    /// The ownership RE-check inside `reap` — the guard that shrinks the
    /// pathname-replacement window between the sweep's candidate checks
    /// and the removal — is load-bearing on its own: a reap invoked
    /// against a dir whose owner no longer matches must walk away.
    #[test]
    fn reap_rechecks_ownership_before_removing() {
        let root = tempfile::tempdir().unwrap();
        let dead = dead_dir(root.path(), "fh-it.rechk1");
        let mut outcome = SweepOutcome {
            reaped: Vec::new(),
            live: 0,
        };
        let budget = std::time::Instant::now() + Duration::from_secs(60);
        reap(&dead, euid().wrapping_add(1), budget, &mut outcome);
        assert!(outcome.reaped.is_empty());
        assert!(dead.exists());
        reap(&dead, euid(), budget, &mut outcome);
        assert_eq!(outcome.reaped, vec![dead.clone()]);
        assert!(!dead.exists());
    }

    /// A FIFO planted at the lock path must not hang the sweep — the
    /// non-blocking, type-checked open refuses it and the entry is
    /// skipped. This test HANGS (rather than failing) if the open
    /// regresses to a plain blocking `File::open`.
    #[test]
    fn sweep_refuses_a_fifo_lock_without_hanging() {
        let root = tempfile::tempdir().unwrap();
        let trapped = root.path().join("fh-it.fifo01");
        fs::create_dir(&trapped).unwrap();
        let fifo = trapped.join(LOCK_FILE_NAME);
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: a valid NUL-terminated path; mkfifo has no other
        // preconditions.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        let outcome = sweep(root.path(), &instant_policy());
        assert!(outcome.reaped.is_empty());
        assert!(trapped.exists());
    }

    /// The age comparison at its boundaries: exactly-at-window is
    /// eligible (not younger), just-under is protected, missing and
    /// future stamps are protected. The zero-window behavior is what
    /// every instant-policy test in this module leans on.
    #[test]
    fn age_comparison_boundaries() {
        let now = SystemTime::now();
        let window = Duration::from_secs(100);
        let at = now.checked_sub(window).unwrap();
        let under = now.checked_sub(window - Duration::from_secs(1)).unwrap();
        let over = now.checked_sub(window + Duration::from_secs(1)).unwrap();
        let future = now.checked_add(Duration::from_secs(10)).unwrap();
        assert!(!is_younger_than(now, Some(at), window));
        assert!(is_younger_than(now, Some(under), window));
        assert!(!is_younger_than(now, Some(over), window));
        assert!(is_younger_than(now, None, window));
        assert!(is_younger_than(now, Some(future), window));
        assert!(!is_younger_than(now, Some(now), Duration::ZERO));
    }

    /// The socket walk finds `tmux.sock` at the depths the harnesses use
    /// (top level, and the stack script's nested `remote/`), matches on
    /// exact name AND socket type — a socket by another name is not tmux
    /// and gets no protocol traffic — pins BOTH sides of the depth bound
    /// (the deepest included level and the first excluded one), and does
    /// not follow symlinks out of the tree being reaped.
    #[test]
    fn tmux_socket_walk_finds_only_real_nested_sockets() {
        use std::os::unix::net::UnixListener;

        let root = tempfile::tempdir().unwrap();
        let top = root.path().join("tmux.sock");
        let nested_dir = root.path().join("remote");
        fs::create_dir(&nested_dir).unwrap();
        let nested = nested_dir.join("tmux.sock");
        let _l1 = UnixListener::bind(&top).unwrap();
        let _l2 = UnixListener::bind(&nested).unwrap();
        // Exact name but a plain file: right name, wrong type.
        let decoy_dir = root.path().join("decoy");
        fs::create_dir(&decoy_dir).unwrap();
        fs::write(decoy_dir.join("tmux.sock"), b"").unwrap();
        // A real socket with the wrong name: right type, not tmux's.
        let _l5 = UnixListener::bind(root.path().join("other.sock")).unwrap();
        // The boundary, both sides: TMUX_SOCKET_WALK_DEPTH counts the
        // nested directory levels the walk recurses into, so a socket in
        // a/b/c (three levels down) is the deepest reachable one, and
        // a/b/c/d is the first level the walk refuses to enter.
        let boundary_dir = root.path().join("a/b/c");
        let deep_dir = boundary_dir.join("d");
        fs::create_dir_all(&deep_dir).unwrap();
        let at_bound = boundary_dir.join("tmux.sock");
        let _l6 = UnixListener::bind(&at_bound).unwrap();
        let _l3 = UnixListener::bind(deep_dir.join("tmux.sock")).unwrap();
        // A symlink from inside the tree to an outside dir holding a real
        // socket: following it would aim kill-server outside the reaped
        // state.
        let outside = tempfile::tempdir().unwrap();
        let _l4 = UnixListener::bind(outside.path().join("tmux.sock")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();

        let mut found = Vec::new();
        collect_tmux_sockets(root.path(), TMUX_SOCKET_WALK_DEPTH, &mut found);
        found.sort();
        let mut expected = vec![top, nested, at_bound];
        expected.sort();
        assert_eq!(found, expected);
    }

    /// Reaping actually terminates a real tmux server, not just its
    /// socket file — the production `tmux -S … kill-server` path end to
    /// end. Without this, a misspelled kill command leaves the suite
    /// green while orphan servers accumulate (the leak half of the
    /// incident this crate exists for).
    #[test]
    fn sweep_kills_a_real_tmux_server() {
        /// Kills the fixture server on drop, so a FAILING run of this
        /// test cannot itself orphan a tmux server past its socket dir's
        /// removal — the exact leak the test exists to prevent. Redundant
        /// after a successful sweep (kill-server on a dead socket is a
        /// failed connect), which is exactly the point of RAII here.
        struct KillServerOnDrop(PathBuf);
        impl Drop for KillServerOnDrop {
            fn drop(&mut self) {
                let _ = std::process::Command::new("tmux")
                    .arg("-S")
                    .arg(&self.0)
                    .arg("kill-server")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }

        let root = tempfile::tempdir().unwrap();
        let dead = dead_dir(root.path(), "fh-it.tmux01");
        let sock = dead.join("tmux.sock");
        let started = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .args(["new-session", "-d", "-s", "teststate"])
            .status()
            .expect("tmux must be installed for this suite");
        assert!(started.success(), "starting the fixture tmux server");
        let _guard = KillServerOnDrop(sock.clone());
        let pid_out = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .args(["display-message", "-p", "#{pid}"])
            .output()
            .expect("query tmux server pid");
        let pid: i32 = String::from_utf8_lossy(&pid_out.stdout)
            .trim()
            .parse()
            .expect("tmux pid");

        let outcome = sweep(root.path(), &instant_policy());
        assert_eq!(outcome.reaped, vec![dead.clone()]);
        assert!(!dead.exists());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            // SAFETY: signal 0 probes existence only; no signal is sent.
            if unsafe { libc::kill(pid, 0) } != 0 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "tmux server {pid} survived the sweep"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// `tempdir_in` produces the full protocol shape: a family-name
    /// container (validated against the sweep's own matcher, not a
    /// duplicated constant — a creator prefix drifting out of the family
    /// would make every new dir unsweepable), mode 0700 throughout, the
    /// flock actually held (a second open cannot take it), close-on-exec
    /// so no spawned child can keep dead state falsely live, a clean
    /// EMPTY dir behind `path()` (tests assert on their workdirs'
    /// contents, so the lock file must not be visible there), and
    /// drop-time removal.
    #[test]
    fn tempdir_in_creates_a_locked_family_dir() {
        let root = tempfile::tempdir().unwrap();
        let test_dir = tempdir_in(root.path()).unwrap();
        let inner = test_dir.path().to_path_buf();
        let container = inner.parent().unwrap().to_path_buf();
        let name = container.file_name().unwrap().to_str().unwrap();
        assert!(is_family_name(name), "{name} must be sweepable");
        for dir in [&container, &inner] {
            let mode = fs::metadata(dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{} must be private", dir.display());
        }
        assert_eq!(
            fs::read_dir(&inner).unwrap().count(),
            0,
            "path() must be an empty dir, like tempfile::tempdir()"
        );
        let second = File::open(container.join(LOCK_FILE_NAME)).unwrap();
        assert!(
            matches!(try_flock(&second), FlockResult::Contended),
            "the run's flock must be held"
        );
        // SAFETY: F_GETFD on an owned open descriptor.
        let fd_flags = unsafe { libc::fcntl(test_dir._lock.as_raw_fd(), libc::F_GETFD) };
        assert!(
            fd_flags >= 0 && (fd_flags & libc::FD_CLOEXEC) != 0,
            "the lock fd must be close-on-exec"
        );
        // Held lock means even an instant policy leaves it alone.
        assert!(sweep(root.path(), &instant_policy()).reaped.is_empty());
        drop(test_dir);
        assert!(
            !container.exists(),
            "drop removes the container like TempDir"
        );
    }

    /// The once-per-process sweep is real and once-only: stale state
    /// present before the first `tempdir()`-shaped call disappears, and
    /// stale state planted between calls survives the second call.
    #[test]
    fn first_tempdir_call_sweeps_and_later_calls_do_not() {
        let root = tempfile::tempdir().unwrap();
        let once = std::sync::Once::new();
        let stale_before = dead_dir(root.path(), "fh-it.stale1");
        let first = tempdir_with_sweep(root.path(), &once, &instant_policy()).unwrap();
        assert!(!stale_before.exists(), "the first call must sweep");
        let stale_between = dead_dir(root.path(), "fh-it.stale2");
        let second = tempdir_with_sweep(root.path(), &once, &instant_policy()).unwrap();
        assert!(stale_between.exists(), "later calls must not re-sweep");
        drop(first);
        drop(second);
    }
}
