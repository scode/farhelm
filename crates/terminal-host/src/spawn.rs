//! Starting a host: the command line, the child spec, the two-stage start,
//! the pty and the child's process setup (SPEC.md, "Command line and child
//! spec" and "Lifecycle").
//!
//! Everything between a `fork` and the `exec` or `_exit` that follows it
//! runs in a copy of this process where only async-signal-safe calls are
//! allowed, so all of it is prepared before forking (C strings, the
//! resolved program path, the signal set) and the child code itself is a
//! short list of raw system calls. Coverage tools cannot see that code
//! (the process never returns to report it); it is exercised by the
//! real-pty integration tests instead.

use crate::protocol::{MAX_SPEC, Size, Spawn, decode_spawn};
use std::ffi::{CString, OsString};
use std::io::{self, Read as _, Write as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

/// Run a host from the command line: parse `args` (without the program
/// name), read the child spec from stdin, and start the host. Returns the
/// first stage's exit status: 0 once the host and its child run, 1 on a
/// start failure (after one line on stderr), 2 on a usage error.
///
/// The socket path must be absolute. The second stage changes to `/`
/// before it binds, so a relative path would be checked against the
/// caller's directory and bound in another; refusing it is simpler than
/// resolving a path whose last part does not exist yet.
///
/// Must be called from a single-threaded process before anything else
/// starts threads: it forks, and the second stage keeps running as the host.
pub fn main_entry(args: impl IntoIterator<Item = OsString>) -> i32 {
    let args: Vec<OsString> = args.into_iter().collect();
    let socket = match args.as_slice() {
        [flag, path] if flag == "--socket" && Path::new(path).is_absolute() => PathBuf::from(path),
        _ => {
            eprintln!("usage: terminal-host --socket <absolute path> < child-spec");
            return 2;
        }
    };
    match start(&socket) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("terminal-host: {error}");
            1
        }
    }
}

/// Read and check the child spec, then fork the second stage and wait for
/// it to report readiness or its error.
fn start(socket: &Path) -> Result<(), String> {
    // One frame whose length field is at most MAX_SPEC: the field itself
    // plus that many bytes. One byte more is read to tell "too long" apart
    // from "exactly at the limit".
    let limit = MAX_SPEC + 4;
    let mut input = Vec::new();
    io::stdin()
        .lock()
        .take(limit as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|error| format!("reading the child spec: {error}"))?;
    if input.len() > limit {
        return Err("the child spec is too long".into());
    }
    let spawn = decode_spawn(&input).map_err(|error| error.to_string())?;
    // The host proper runs inside this function's frame for weeks, so
    // nothing of the start may stay alive here: the raw spec goes now, and
    // the decoded one moves into the second stage, which keeps only what
    // the host needs (a 16 MiB initial snapshot would otherwise be held
    // several times over, and the child's environment for no reason).
    drop(input);
    if std::fs::symlink_metadata(socket).is_ok() {
        return Err(format!("{} already exists", socket.display()));
    }
    let prepared = Prepared::new(&spawn)?;
    // Both ends above the standard descriptors: a starter that closed its
    // stdout or stderr would otherwise get the pipe there, and the second
    // stage's redirection to /dev/null would then overwrite its end of it.
    let (ready_read, ready_write) = pipe()
        .and_then(|(read, write)| Ok((above_stdio(read)?, above_stdio(write)?)))
        .map_err(|error| format!("creating a pipe: {error}"))?;
    // SAFETY: fork has no preconditions here: this process is single-threaded
    // (the caller's contract), so the child is a complete copy.
    match unsafe { libc::fork() } {
        -1 => Err(format!("forking the host: {}", io::Error::last_os_error())),
        0 => {
            drop(ready_read);
            let ready_write = std::fs::File::from(ready_write);
            let status = match second_stage(socket, spawn, prepared, ready_write.as_raw_fd()) {
                Ok(host) => match report_ready(host, ready_write) {
                    Some(host) => {
                        host.run();
                        0
                    }
                    None => 1,
                },
                Err(error) => {
                    let _ = (&ready_write).write_all(error.as_bytes());
                    1
                }
            };
            // An ordinary exit, not `_exit`: the first stage registers no
            // exit handlers and buffers no output before the fork, so there
            // is nothing copied to run twice, and an ordinary exit is what
            // lets a coverage-instrumented build write its profile from the
            // daemon (the only way the code after readiness is measured).
            std::process::exit(status)
        }
        _second_stage => {
            drop(ready_write);
            let mut report = Vec::new();
            let _ = std::fs::File::from(ready_read)
                .take(64 * 1024)
                .read_to_end(&mut report);
            match report.as_slice() {
                b"ok" => Ok(()),
                b"" => Err("the host ended before it was ready".into()),
                error => Err(String::from_utf8_lossy(error).into_owned()),
            }
        }
    }
}

/// The second stage up to readiness: a session of its own, clean
/// descriptors and signals, the socket, the pty and the child.
///
/// The signal handlers come before the socket: a SIGTERM that arrived
/// between binding and installing them would end the second stage at its
/// default disposition and leave the socket file behind, while one that
/// arrives after them is noted, fails the start if it came before the
/// child's start or the readiness report ([`after_bind`]), and otherwise
/// ends the host on its first iteration, which removes the socket. Every
/// failure once the socket is bound removes it again too: the first stage
/// reports the failure, and a retry on the same path must not find the
/// socket of a host that never ran.
fn second_stage(
    socket: &Path,
    spawn: Spawn,
    prepared: Prepared,
    ready: RawFd,
) -> Result<crate::host::Host, String> {
    // SAFETY: setsid has no preconditions; the forked second stage is not a
    // process group leader, so it succeeds.
    if unsafe { libc::setsid() } == -1 {
        return Err(format!("setsid: {}", io::Error::last_os_error()));
    }
    // The redirection first, so that closing what was inherited can leave
    // exactly 0 to 2 (now /dev/null) and the readiness pipe.
    null_stdio()?;
    close_inherited(&[ready])?;
    std::env::set_current_dir("/").map_err(|error| format!("changing to /: {error}"))?;
    // Before the child exists, so its exit always finds the handler, and
    // before the socket (see above).
    let signals = crate::host::install_signal_handlers()?;
    // The starter's mask survives fork and exec. With SIGTERM blocked the
    // host would never end on it, and with SIGCHLD blocked a child that
    // exits after closing its terminal would go unreaped until something
    // else woke the loop. Only now, with the handlers in place, may they
    // arrive.
    // SAFETY: sigset_t is plain data, filled by sigemptyset.
    let unblocked = unsafe {
        let mut empty: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut empty);
        libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut())
    };
    if unblocked != 0 {
        return Err(format!(
            "clearing the signal mask: {}",
            io::Error::last_os_error()
        ));
    }
    let (listener, socket_file) = crate::host::listen(socket)?;
    after_bind(
        socket,
        socket_file,
        spawn,
        prepared,
        (listener, signals),
        &crate::host::terminated,
    )
    .inspect_err(|_| crate::host::remove_socket(socket, socket_file))
}

/// Tell the first stage the host is ready, and hand the host back to run;
/// or, when that report cannot be written, end the start as a failed one and
/// hand back nothing.
///
/// The report fails when the first stage is gone (killed, say, by a starter
/// that gave up waiting): the pipe has no reader, and with SIGPIPE ignored
/// the write returns `EPIPE`. Then nobody learned that this host exists, and
/// whoever started it was told the start failed or never finished, so a host
/// that ran on would serve a terminal nobody knows about. It is ended the
/// way a SIGTERM during the start ends one: the child's process group is
/// killed and the child reaped, and the socket is removed.
fn report_ready(host: crate::host::Host, ready: std::fs::File) -> Option<crate::host::Host> {
    match (&ready).write_all(b"ok") {
        Ok(()) => Some(host),
        Err(_) => {
            host.abandon();
            None
        }
    }
}

/// Kill the started child's whole process group and reap the child: what a
/// start that fails after the child exists does. The child leads its own
/// session and process group, so anything it forked in the moment since it
/// started dies with it rather than outliving a start that failed.
pub(crate) fn kill_child_group(child: u32) {
    // SAFETY: signalling this process's own child's group and reaping the
    // child, which nothing else waits for.
    unsafe {
        libc::kill(-(child as libc::pid_t), libc::SIGKILL);
        libc::waitpid(child as libc::pid_t, std::ptr::null_mut(), 0);
    }
}

/// What a start fails with when a SIGTERM came first.
const TERMINATED_DURING_START: &str = "terminated during the start";

/// The second stage from the bound socket to the running child.
///
/// Takes the decoded spec by value and keeps only what the running host
/// needs from it: its tag, and the initial snapshot, which moves into the
/// journal. The argv, working directory and environment were already turned
/// into `prepared`, which the child's start consumes; they are freed here.
///
/// `terminated` says whether a SIGTERM has arrived (the real one is
/// [`crate::host::terminated`]; a test passes its own, since the test
/// process installs no handler). It is asked before the child is started
/// and again before the host is handed back to report readiness: a host
/// told to end during its start fails the start instead of starting a child
/// for nothing and reporting ready. A child already started then is killed
/// and reaped, like one that failed to start; the caller never learns its
/// pid, so nothing else could. A SIGTERM after the second check is the
/// running host's to handle, on its first iteration.
fn after_bind(
    socket: &Path,
    socket_file: crate::host::SocketFile,
    spawn: Spawn,
    prepared: Prepared,
    (listener, signals): (OwnedFd, OwnedFd),
    terminated: &dyn Fn() -> bool,
) -> Result<crate::host::Host, String> {
    let Spawn {
        size,
        journal_cap,
        tag,
        initial_snapshot,
        ..
    } = spawn;
    let pty = open_pty(size)?;
    if terminated() {
        return Err(TERMINATED_DURING_START.into());
    }
    let child = spawn_child(&pty, prepared)?;
    if terminated() {
        kill_child_group(child);
        return Err(TERMINATED_DURING_START.into());
    }
    Ok(crate::host::Host::new(
        crate::host::Setup {
            listener,
            socket: socket.to_path_buf(),
            socket_file,
            master: pty.master,
            child,
            size,
            journal_cap,
            tag,
        },
        initial_snapshot,
        Some(signals),
    ))
}

/// What the child's `exec` needs, built before forking.
pub(crate) struct Prepared {
    program: CString,
    argv: Vec<CString>,
    env: Vec<CString>,
    cwd: CString,
}

impl Prepared {
    /// Resolve the program in the child's own `PATH` (not this process's)
    /// and turn everything into C strings.
    pub(crate) fn new(spawn: &Spawn) -> Result<Self, String> {
        let c = |bytes: &[u8]| {
            CString::new(bytes).map_err(|_| "a NUL byte in the child spec".to_string())
        };
        let argv: Vec<CString> = spawn
            .argv
            .iter()
            .map(|arg| c(arg))
            .collect::<Result<_, _>>()?;
        let env: Vec<CString> = spawn
            .env
            .iter()
            .map(|entry| c(entry))
            .collect::<Result<_, _>>()?;
        let cwd = Path::new(std::ffi::OsStr::from_bytes(&spawn.cwd));
        let program = resolve_program(&spawn.argv[0], &spawn.env, cwd)?;
        Ok(Self {
            program: c(program.as_os_str().as_bytes())?,
            argv,
            env,
            cwd: c(&spawn.cwd)?,
        })
    }
}

/// Find `name` the way a shell would: as given when it contains a slash
/// (relative to the child's working directory), otherwise in each entry of
/// the child's `PATH` (POSIX's `/usr/bin:/bin` when it has none).
fn resolve_program(name: &[u8], env: &[Vec<u8>], cwd: &Path) -> Result<PathBuf, String> {
    let name_path = Path::new(std::ffi::OsStr::from_bytes(name));
    if name.contains(&b'/') {
        return Ok(cwd.join(name_path));
    }
    let path = env
        .iter()
        .find_map(|entry| entry.strip_prefix(b"PATH="))
        .unwrap_or(b"/usr/bin:/bin");
    for dir in path.split(|&b| b == b':') {
        let dir = if dir.is_empty() { b".".as_slice() } else { dir };
        let candidate = cwd
            .join(Path::new(std::ffi::OsStr::from_bytes(dir)))
            .join(name_path);
        if is_executable(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "{} not found in the child's PATH",
        String::from_utf8_lossy(name)
    ))
}

/// Whether `path` is a regular file this process may execute. The child
/// runs as the host's own user, so the kernel's own check for this process
/// (`faccessat` with the effective ids, which also sees ACLs) is the one its
/// `execve` will make. Any execute bit alone is not enough: a file of the
/// user's own with mode 0641 has one, but not for its owner, and taking it
/// would end the search on a file that cannot run when a later `PATH` entry
/// holds one that can.
fn is_executable(path: &Path) -> bool {
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: faccessat with a valid NUL-terminated path and no out data.
    let executable = unsafe {
        libc::faccessat(
            libc::AT_FDCWD,
            c_path.as_ptr(),
            libc::X_OK,
            libc::AT_EACCESS,
        )
    } == 0;
    executable && std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// A pty pair, both ends close-on-exec.
pub(crate) struct Pty {
    pub(crate) master: OwnedFd,
    pub(crate) slave: OwnedFd,
}

/// Open a pty of `size` with the terminal settings SPEC.md gives a new
/// terminal: the kernel's defaults, then `VERASE` = DEL and `IUTF8`.
pub(crate) fn open_pty(size: Size) -> Result<Pty, String> {
    let (master, slave) = open_pair().map_err(|error| format!("opening a pty: {error}"))?;
    set_nonblocking(master.as_raw_fd()).map_err(|error| format!("configuring the pty: {error}"))?;
    let winsize = libc::winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCSWINSZ on the slave with a valid winsize.
    if unsafe { libc::ioctl(slave.as_raw_fd(), libc::TIOCSWINSZ as _, &winsize) } != 0 {
        return Err(format!(
            "setting the terminal size: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: termios is plain data; tcgetattr fills it from a valid fd.
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut termios) } != 0 {
        return Err(format!(
            "reading terminal settings: {}",
            io::Error::last_os_error()
        ));
    }
    termios.c_cc[libc::VERASE] = 0x7f;
    termios.c_iflag |= libc::IUTF8;
    // SAFETY: as above, with a termios this function filled.
    if unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &termios) } != 0 {
        return Err(format!(
            "setting terminal settings: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(Pty { master, slave })
}

/// A new pty's master and slave, both close-on-exec and neither becoming the
/// caller's controlling terminal (the second stage leads a session of its
/// own, which would otherwise take the first terminal it opens).
///
/// On Linux both ends are opened close-on-exec in the call that makes them
/// (`posix_openpt`, then `TIOCGPTPEER`, which opens the slave from the
/// master without a path; kernels before 4.13 lack it, and get the slave by
/// its name, still in one call). glibc's `openpty` has no such flag, and a
/// child another thread forks between it and setting the flag would carry
/// the pty past its `exec` (see [`pipe`]). macOS's `openpty` has none
/// either and macOS has no `TIOCGPTPEER`, so there the flags are set after.
#[cfg(target_os = "linux")]
fn open_pair() -> io::Result<(OwnedFd, OwnedFd)> {
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC;
    // SAFETY: posix_openpt with constant flags.
    let master = unsafe { libc::posix_openpt(flags) };
    if master < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: posix_openpt returned a fresh descriptor this function owns.
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    // SAFETY: grantpt and unlockpt on the master just opened.
    if unsafe { libc::grantpt(master.as_raw_fd()) } != 0
        || unsafe { libc::unlockpt(master.as_raw_fd()) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: TIOCGPTPEER opens this master's slave with `flags`.
    let mut slave = unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCGPTPEER as _, flags) };
    if slave < 0 {
        let mut name = [0 as libc::c_char; 128];
        // SAFETY: ptsname_r writes a NUL-terminated name into `name`, and
        // open takes it with constant flags.
        slave = unsafe {
            if libc::ptsname_r(master.as_raw_fd(), name.as_mut_ptr(), name.len()) != 0 {
                return Err(io::Error::last_os_error());
            }
            libc::open(name.as_ptr(), flags)
        };
        if slave < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: a fresh descriptor this function now owns.
    Ok((master, unsafe { OwnedFd::from_raw_fd(slave) }))
}

/// See the Linux version: here the flags are set after `openpty`.
#[cfg(not(target_os = "linux"))]
fn open_pair() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;
    // SAFETY: the out pointers are valid; a null name, termios and window
    // size ask for the kernel's defaults.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openpty returned two fresh descriptors this function now owns.
    let pair = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    set_cloexec(pair.0.as_raw_fd())?;
    set_cloexec(pair.1.as_raw_fd())?;
    Ok(pair)
}

/// Start the child on `pty`'s slave and return its pid once it has exec'd
/// its program, or the error that kept it from doing so. The slave is
/// closed in the host afterwards.
pub(crate) fn spawn_child(pty: &Pty, prepared: Prepared) -> Result<u32, String> {
    let argv: Vec<*const libc::c_char> = prepared
        .argv
        .iter()
        .map(|arg| arg.as_ptr())
        .chain([std::ptr::null()])
        .collect();
    let envp: Vec<*const libc::c_char> = prepared
        .env
        .iter()
        .map(|entry| entry.as_ptr())
        .chain([std::ptr::null()])
        .collect();
    // Its own wording, not the readiness pipe's "creating a pipe": a test
    // that fails this one on purpose must be able to tell which failed.
    let (error_read, error_write) =
        pipe().map_err(|error| format!("creating the child's error pipe: {error}"))?;
    let slave = pty.slave.as_raw_fd();
    let error_fd = error_write.as_raw_fd();
    // SAFETY: the child below only makes async-signal-safe calls on data
    // prepared before the fork, then execs or _exits.
    let pid = unsafe { libc::fork() };
    if pid == -1 {
        return Err(format!("forking the child: {}", io::Error::last_os_error()));
    }
    if pid == 0 {
        // SAFETY: async-signal-safe calls only; see the function's contract.
        unsafe { child_after_fork(slave, &prepared, &argv, &envp, error_fd) }
    }
    drop(error_write);
    let mut report = Vec::new();
    let _ = std::fs::File::from(error_read).read_to_end(&mut report);
    if report.is_empty() {
        return Ok(pid as u32);
    }
    let errno = i32::from_ne_bytes(report[..4.min(report.len())].try_into().unwrap_or([0; 4]));
    // The child has already _exit'ed; reap it so nothing is left behind.
    let mut status = 0;
    // SAFETY: pid is this process's own child.
    unsafe { libc::waitpid(pid, &mut status, 0) };
    Err(format!(
        "starting {}: {}",
        prepared.program.to_string_lossy(),
        io::Error::from_raw_os_error(errno)
    ))
}

/// The child between fork and exec: its own session with the pty slave as
/// controlling terminal and stdio, every signal at its default disposition
/// and an empty mask, the requested working directory, then exec. Any
/// failure writes errno to `error_fd` and exits 127.
///
/// # Safety
///
/// Only call in a freshly forked child; uses only async-signal-safe calls.
#[cfg_attr(coverage_nightly, coverage(off))]
unsafe fn child_after_fork(
    slave: RawFd,
    prepared: &Prepared,
    argv: &[*const libc::c_char],
    envp: &[*const libc::c_char],
    error_fd: RawFd,
) -> ! {
    #[cfg_attr(coverage_nightly, coverage(off))]
    unsafe fn fail(error_fd: RawFd) -> ! {
        unsafe {
            let errno = *errno_location();
            libc::write(error_fd, (&errno as *const i32).cast(), 4);
            libc::_exit(127)
        }
    }
    unsafe {
        if libc::setsid() == -1 {
            fail(error_fd);
        }
        if libc::ioctl(slave, libc::TIOCSCTTY as _, 0) == -1 {
            fail(error_fd);
        }
        for target in 0..3 {
            if libc::dup2(slave, target) == -1 {
                fail(error_fd);
            }
        }
        // Default dispositions and an empty mask: ignored signals would
        // survive exec, and a shell cannot un-ignore what it inherits.
        for signal in 1..signal_count() {
            if signal != libc::SIGKILL && signal != libc::SIGSTOP {
                libc::signal(signal, libc::SIG_DFL);
            }
        }
        let mut empty: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut empty);
        libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
        if libc::chdir(prepared.cwd.as_ptr()) == -1 {
            fail(error_fd);
        }
        libc::execve(prepared.program.as_ptr(), argv.as_ptr(), envp.as_ptr());
        fail(error_fd)
    }
}

/// One past the highest signal number. Called only after fork.
#[cfg_attr(coverage_nightly, coverage(off))]
fn signal_count() -> libc::c_int {
    #[cfg(target_os = "linux")]
    {
        // SIGRTMAX is a function call on glibc; 65 covers it.
        65
    }
    #[cfg(not(target_os = "linux"))]
    {
        32
    }
}

/// This thread's `errno`, for code that may not call into std: the child
/// between fork and exec, and the signal handler in `crate::host`.
#[cfg(target_os = "linux")]
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) unsafe fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}

/// See the Linux version.
#[cfg(target_os = "macos")]
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) unsafe fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__error() }
}

/// A close-on-exec pipe: atomically on Linux (`pipe2`), in two steps on
/// macOS, which has no `pipe2`.
///
/// The daemon is single-threaded, so nothing can fork between the two
/// steps there. The in-process tests are not: libtest runs them as threads
/// of one process, and a child another test starts in that window would
/// carry the descriptor past its `exec` (an error pipe's write end held that
/// way keeps `spawn_child` waiting for that unrelated child to die, and a
/// pty end keeps a test from ever seeing its end of output). The same holds
/// for the pty ([`open_pair`]), the listener and accepted connections in
/// `crate::host`. Linux makes all of them atomically; macOS has no atomic
/// form for any of them. So the crate's tests run with one test thread
/// under libtest on both platforms (TESTING.md), and nextest, which runs
/// each test in a process of its own, needs nothing.
pub(crate) fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    #[cfg(target_os = "linux")]
    {
        // SAFETY: fds has room for the two descriptors pipe2 writes.
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: pipe2 returned two fresh descriptors.
        Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
    }
    #[cfg(not(target_os = "linux"))]
    {
        // SAFETY: fds has room for the two descriptors pipe writes.
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: pipe returned two fresh descriptors.
        let pair = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        set_cloexec(fds[0])?;
        set_cloexec(fds[1])?;
        Ok(pair)
    }
}

/// Set `FD_CLOEXEC` on `fd`, keeping its other descriptor flags. For the
/// descriptors macOS cannot open close-on-exec in one call; Linux opens
/// every one of them that way, so there only the tests call it.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) fn set_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl on a valid descriptor with integer arguments.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags == -1 || libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(crate) fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl on a valid descriptor with integer arguments.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags == -1 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Close every descriptor above stderr except `keep`: the host must not
/// hold its starter's sockets, locks or pipes for weeks. Runs after
/// [`null_stdio`], which is what 0 to 2 then hold.
fn close_inherited(keep: &[RawFd]) -> Result<(), String> {
    let open: Vec<RawFd> = std::fs::read_dir("/dev/fd")
        .map_err(|error| format!("listing open descriptors: {error}"))?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    for fd in open {
        if fd > 2 && !keep.contains(&fd) {
            // SAFETY: closing a descriptor number this process may hold; one
            // already gone (the listing's own) just fails.
            unsafe { libc::close(fd) };
        }
    }
    Ok(())
}

/// Put stdin, stdout and stderr on `/dev/null`, whichever of them the
/// starter left open or closed.
fn null_stdio() -> Result<(), String> {
    let null: OwnedFd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|error| format!("opening /dev/null: {error}"))?
        .into();
    // With a standard descriptor closed, the open above returns that very
    // number; `dup2` onto it would do nothing, and closing `null` would
    // then close it again, leaving the slot for the next socket or pipe.
    let null = above_stdio(null).map_err(|error| format!("opening /dev/null: {error}"))?;
    for target in 0..3 {
        // SAFETY: dup2 onto the standard descriptors.
        if unsafe { libc::dup2(null.as_raw_fd(), target) } == -1 {
            return Err(format!("redirecting stdio: {}", io::Error::last_os_error()));
        }
    }
    Ok(())
}

/// `fd` renumbered to 3 or above (close-on-exec), the original closed: for
/// a descriptor that must not sit where stdin, stdout or stderr go, because
/// those are about to be redirected. Always copies, which costs one call
/// where `fd` is already above them.
fn above_stdio(fd: OwnedFd) -> io::Result<OwnedFd> {
    // SAFETY: fcntl on a valid descriptor; F_DUPFD_CLOEXEC returns a fresh
    // one at or above 3.
    let moved = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if moved < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a fresh descriptor this function now owns.
    Ok(unsafe { OwnedFd::from_raw_fd(moved) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Descriptor flags as `fcntl` reports them.
    fn flags(fd: RawFd) -> (libc::c_int, libc::c_int) {
        // SAFETY: fcntl queries on a descriptor the test owns.
        unsafe {
            (
                libc::fcntl(fd, libc::F_GETFD),
                libc::fcntl(fd, libc::F_GETFL),
            )
        }
    }

    /// Setting close-on-exec or non-blocking adds the flag and keeps it
    /// set when it already was, leaving the other flags alone.
    ///
    /// Why: some descriptors arrive with a flag already set (on macOS an
    /// accepted connection inherits `O_NONBLOCK` from the listener), and a
    /// helper that toggled instead of setting would turn it off there and
    /// leave the host's loop blocked on one connection. Spec: after one or
    /// two calls, `FD_CLOEXEC` and `O_NONBLOCK` are set; the access mode is
    /// unchanged.
    #[test]
    fn the_descriptor_flag_helpers_set_and_never_toggle() {
        let (read, _write) = pipe().expect("fixture: a pipe");
        let fd = read.as_raw_fd();
        let (_, before) = flags(fd);
        for _ in 0..2 {
            set_cloexec(fd).unwrap();
            set_nonblocking(fd).unwrap();
            let (fd_flags, status) = flags(fd);
            assert_ne!(fd_flags & libc::FD_CLOEXEC, 0);
            assert_ne!(status & libc::O_NONBLOCK, 0);
            assert_eq!(status & libc::O_ACCMODE, before & libc::O_ACCMODE);
        }
    }

    /// A SIGTERM noted during the start fails the start: before the child is
    /// started, or once it is, with the child killed and reaped.
    ///
    /// Why: a host told to end while it starts must neither report ready
    /// nor leave behind a child started for nothing (SPEC.md, "Start and
    /// readiness"). The test process installs no handler that could note a
    /// SIGTERM, so the check is handed in. Spec: with a SIGTERM noted from
    /// the start, the start fails saying so after a single check, and the
    /// child never ran (its pid file never appears); with one noted only
    /// once the child runs (it records its pid in a file first), the start
    /// fails after the second check, and that pid then names no process, not
    /// even a zombie.
    #[test]
    fn a_sigterm_during_the_start_fails_it() {
        use std::cell::Cell;
        let dir = tempfile::tempdir().expect("fixture: a directory");
        let pid_file = dir.path().join("pid");
        let script = format!("echo $$ > '{}'; exec /bin/sleep 1000", pid_file.display());
        let spawn = Spawn {
            argv: ["/bin/sh", "-c", script.as_str()]
                .iter()
                .map(|arg| arg.as_bytes().to_vec())
                .collect(),
            cwd: b"/".to_vec(),
            env: vec![b"PATH=/usr/bin:/bin".to_vec()],
            size: Size { cols: 80, rows: 24 },
            journal_cap: 1 << 20,
            tag: Vec::new(),
            initial_snapshot: None,
        };
        let attempt = |terminated: &dyn Fn() -> bool| {
            let socket = dir.path().join("h");
            let _ = std::fs::remove_file(&socket);
            let (listener, file) = crate::host::listen(&socket).expect("fixture: a socket");
            let (signals, _) = pipe().expect("fixture: a pipe");
            let prepared = Prepared::new(&spawn).expect("fixture: the spec");
            after_bind(
                &socket,
                file,
                spawn.clone(),
                prepared,
                (listener, signals),
                terminated,
            )
            .err()
        };

        let calls = Cell::new(0);
        let error = attempt(&|| {
            calls.set(calls.get() + 1);
            true
        });
        assert_eq!(error.as_deref(), Some(TERMINATED_DURING_START));
        assert_eq!(calls.get(), 1, "the start went past the first check");
        assert!(!pid_file.exists(), "a child was started");

        let calls = Cell::new(0);
        let child = Cell::new(None);
        let error = attempt(&|| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                return false;
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let pid = loop {
                let text = std::fs::read_to_string(&pid_file).unwrap_or_default();
                if let Ok(pid) = text.trim().parse::<libc::pid_t>() {
                    break pid;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the child never wrote its pid"
                );
                // sleep-ok: polling interval for a pid file the test's own child writes.
                std::thread::sleep(std::time::Duration::from_millis(20));
            };
            child.set(Some(pid));
            true
        });
        assert_eq!(error.as_deref(), Some(TERMINATED_DURING_START));
        let pid = child.get().expect("premise: the child ran");
        assert!(pid > 1, "implausible pid {pid}");
        // SAFETY: probing, and if need be ending, the test's own child.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if alive {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            }
        }
        assert!(!alive, "the child {pid} was left behind");
    }

    /// A start whose readiness report has no reader fails: its child is
    /// killed and reaped and its socket removed; one whose report is read
    /// hands the host on to run.
    ///
    /// Why: a starter that gave up (a supervisor whose spawn timed out and
    /// killed the first stage) told its caller the start failed, so a host
    /// that ran on would serve a terminal nobody knows about, and a tab
    /// found later by its tag would pass for one the user was told never
    /// opened (SPEC.md, "Start and readiness", step 4). Spec: with a real
    /// pty and a child that records its pid, a report written to a pipe
    /// whose read end is closed gives no host, and afterwards the child's pid
    /// names no process (not even a zombie) and the socket file is gone; a
    /// report written to an open pipe gives the host back and the reader
    /// gets `ok`.
    #[test]
    fn a_readiness_report_nobody_reads_ends_the_start() {
        let dir = tempfile::tempdir().expect("fixture: a directory");
        let pid_file = dir.path().join("pid");
        let script = format!("echo $$ > '{}'; exec /bin/sleep 1000", pid_file.display());
        let spawn = Spawn {
            argv: ["/bin/sh", "-c", script.as_str()]
                .iter()
                .map(|arg| arg.as_bytes().to_vec())
                .collect(),
            cwd: b"/".to_vec(),
            env: vec![b"PATH=/usr/bin:/bin".to_vec()],
            size: Size { cols: 80, rows: 24 },
            journal_cap: 1 << 20,
            tag: Vec::new(),
            initial_snapshot: None,
        };
        let socket = dir.path().join("h");
        let started = || {
            let _ = std::fs::remove_file(&pid_file);
            let (listener, file) = crate::host::listen(&socket).expect("fixture: a socket");
            let (signals, _) = pipe().expect("fixture: a pipe");
            let prepared = Prepared::new(&spawn).expect("fixture: the spec");
            let host = after_bind(
                &socket,
                file,
                spawn.clone(),
                prepared,
                (listener, signals),
                &|| false,
            )
            .expect("fixture: the start");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let pid = loop {
                let text = std::fs::read_to_string(&pid_file).unwrap_or_default();
                if let Ok(pid) = text.trim().parse::<libc::pid_t>() {
                    break pid;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the child never wrote its pid"
                );
                // sleep-ok: polling interval for a pid file the test's own child writes.
                std::thread::sleep(std::time::Duration::from_millis(20));
            };
            assert!(pid > 1, "implausible pid {pid}");
            (host, pid)
        };

        let (host, pid) = started();
        let (read, write) = pipe().expect("fixture: a pipe");
        drop(read);
        let kept = report_ready(host, std::fs::File::from(write));
        // SAFETY: probing, and if need be ending, the test's own child.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if alive {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            }
        }
        assert!(kept.is_none(), "an unreported start handed its host on");
        assert!(!alive, "the child {pid} was left behind");
        assert!(
            std::fs::symlink_metadata(&socket).is_err(),
            "the socket was left behind"
        );

        let (host, _) = started();
        let (read, write) = pipe().expect("fixture: a pipe");
        let host = report_ready(host, std::fs::File::from(write)).expect("a reported start");
        let mut report = Vec::new();
        std::fs::File::from(read)
            .read_to_end(&mut report)
            .expect("reading the report");
        assert_eq!(report, b"ok");
        // Ends this fixture's child and socket the same way.
        host.abandon();
        assert!(std::fs::symlink_metadata(&socket).is_err());
    }

    /// An abandoned start ends the child's whole process group, not only the
    /// child.
    ///
    /// Why: a program forks as soon as it runs (a shell starting a
    /// background job, say), and a start that fails must not leave what it
    /// forked running for nobody (SPEC.md, "Start and readiness"). Killing
    /// the child's pid alone would end the shell and orphan the job. The job
    /// ignores SIGHUP, as `nohup` or a daemon would: otherwise the hangup the
    /// kernel sends the terminal's foreground group when its session leader
    /// (the child) dies would end it anyway and hide the difference. Spec:
    /// with a child shell that starts a background `sleep` ignoring SIGHUP
    /// (in the shell's own process group, as a shell without job control
    /// does) and records both pids, abandoning the start before readiness is
    /// reported leaves the job not running (gone, or a zombie awaiting its
    /// new parent) within a bounded wait.
    #[test]
    fn an_abandoned_start_ends_the_childs_process_group() {
        let dir = tempfile::tempdir().expect("fixture: a directory");
        let pids = dir.path().join("pids");
        let script = format!(
            "trap '' HUP; /bin/sleep 1000 & echo \"$$ $!\" > '{}.tmp'; mv '{0}.tmp' '{0}'; wait",
            pids.display()
        );
        let spawn = Spawn {
            argv: ["/bin/sh", "-c", script.as_str()]
                .iter()
                .map(|arg| arg.as_bytes().to_vec())
                .collect(),
            cwd: b"/".to_vec(),
            env: vec![b"PATH=/usr/bin:/bin".to_vec()],
            size: Size { cols: 80, rows: 24 },
            journal_cap: 1 << 20,
            tag: Vec::new(),
            initial_snapshot: None,
        };
        let socket = dir.path().join("h");
        let (listener, file) = crate::host::listen(&socket).expect("fixture: a socket");
        let (signals, _) = pipe().expect("fixture: a pipe");
        let prepared = Prepared::new(&spawn).expect("fixture: the spec");
        let host = after_bind(
            &socket,
            file,
            spawn.clone(),
            prepared,
            (listener, signals),
            &|| false,
        )
        .expect("fixture: the start");

        let wait = std::time::Duration::from_secs(10);
        let deadline = std::time::Instant::now() + wait;
        let (child, grandchild) = loop {
            // Written whole by a rename, so a read never sees half of it.
            let text = std::fs::read_to_string(&pids).unwrap_or_default();
            let mut fields = text.split_whitespace().map(str::parse::<libc::pid_t>);
            if let (Some(Ok(child)), Some(Ok(grandchild))) = (fields.next(), fields.next()) {
                break (child, grandchild);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the child never recorded its pids"
            );
            // sleep-ok: polling interval for a pid file the test's own child writes.
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(
            child > 1 && grandchild > 1,
            "implausible pids {child} {grandchild}"
        );
        // SAFETY: getpgid only reads the process's group.
        assert_eq!(
            unsafe { libc::getpgid(grandchild) },
            child,
            "premise: the background job is in the child's process group"
        );

        let (read, write) = pipe().expect("fixture: a pipe");
        drop(read);
        assert!(report_ready(host, std::fs::File::from(write)).is_none());

        let deadline = std::time::Instant::now() + wait;
        let mut ended = !running(grandchild);
        while !ended && std::time::Instant::now() < deadline {
            // sleep-ok: polling interval for the end of a process the test's own child started.
            std::thread::sleep(std::time::Duration::from_millis(20));
            ended = !running(grandchild);
        }
        if !ended {
            // SAFETY: ending the background job the test's own child started.
            unsafe { libc::kill(grandchild, libc::SIGKILL) };
        }
        assert!(
            ended,
            "the child's background job {grandchild} outlived the start"
        );
    }

    /// Whether `pid` exists and is not a zombie: an orphan killed with its
    /// group stays a zombie until whoever adopted it reaps it, which this
    /// test does not control.
    fn running(pid: libc::pid_t) -> bool {
        // SAFETY: signal 0 only checks for the process.
        if unsafe { libc::kill(pid, 0) } != 0 {
            return false;
        }
        #[cfg(target_os = "linux")]
        let state = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| stat.rsplit_once(") ").map(|(_, rest)| rest.to_owned()))
            .unwrap_or_default();
        #[cfg(target_os = "macos")]
        let state = std::process::Command::new("/bin/ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .unwrap_or_default();
        !state.is_empty() && !state.starts_with('Z')
    }
}
