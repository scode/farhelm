//! The three per-platform process-table reads the kill sweep is built on,
//! and nothing else.
//!
//! `service::sweep` owns every DECISION a stop, delete, or close makes: the
//! PPID closure, the environment-marker union that finds reparented
//! daemons, start-time validation against pid reuse, the
//! SIGTERM/SIGSTOP-quiesce/SIGKILL escalation, and the confirm-gone poll.
//! None of that is platform-specific. What is platform-specific is narrow
//! and mechanical — how one asks the kernel three questions:
//!
//! 1. every process this user owns, with its parent and its start time
//!    ([`snapshot`]);
//! 2. one pid's parent, start time, and whether it has become a zombie
//!    ([`read_process`]);
//! 3. one pid's exec-time environment ([`read_environ`]).
//!
//! Keeping the seam exactly that small is the whole point. Linux and macOS
//! must run the SAME sweep rather than two sweeps that happen to agree,
//! because the promise stop and delete make to a user — "nothing this
//! session started is still running" — is supposed to mean the same thing
//! on both. Until this module existed the sweep read `/proc` inline, so on
//! macOS every enumeration failed with "reading /proc: No such file or
//! directory" and, since the sweep fails CLOSED by design, stop and delete
//! failed with it (SPEC_impl.md recorded the Mac variant as deferred; this
//! is it).
//!
//! # Contracts that hold on every platform
//!
//! **Start time is an opaque identity token, never a timestamp.** Linux
//! reports jiffies since boot (`/proc/<pid>/stat` field 22); macOS reports
//! the process's start `timeval` folded to whole microseconds. Both are
//! injective and stable for the life of a process on one boot, which is
//! all the sweep needs — it only ever compares a value from this module
//! against ANOTHER value from this module for equality. Nothing may
//! subtract them, order them, compare them across a reboot, or read a wall
//! clock out of them. The one property the platforms must genuinely share
//! is that [`snapshot`] and [`read_process`] derive the value the SAME way,
//! since pid-reuse detection compares one against the other.
//!
//! **The environment is the exec-time environment, and only that.** On
//! Linux that is `/proc/<pid>/environ`; on macOS it is the environment
//! region of `KERN_PROCARGS2`, with the argv region deliberately excluded
//! (see `parse_procargs2`). A process can be claimed by a sweep only for
//! what its parent handed it at exec, never for text it happens to have on
//! its command line. NOTE: macOS 26+ withholds that region entirely for
//! Apple platform binaries — see [`read_environ`] for the observation and
//! what it costs the caller.
//!
//! **Unreadable is not an error for environments, and IS one for
//! everything else.** A foreign-uid or hardened process makes its
//! environment unreadable as a matter of routine, so [`read_environ`]
//! answers `None` and the sweep reads that as "carries no marker" — the
//! safe direction, since a sweep never signals a process it could not
//! identify. Enumeration is the opposite: [`snapshot`] returning `Err`
//! rather than an empty map is what stops a host where the process table
//! cannot be read at all from reporting a falsely-clean sweep.

use std::collections::HashMap;

/// The only distinction the sweep draws between one live process and
/// another.
///
/// A zombie has already exited and is merely waiting for an ancestor to
/// reap it. Nothing this supervisor does can force that reap, and a zombie
/// cannot run code, so the sweep's confirmation step (`service::sweep`) counts
/// one as gone; treating it as still-alive would fail a stop for a reason
/// no amount of signaling could ever fix. Everything that is not a zombie
/// — running, sleeping, stopped, traced — is [`ProcessState::Running`],
/// because the sweep has no use for the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessState {
    /// Alive as far as this sweep cares: it may still execute code.
    Running,
    /// Exited, not yet reaped. Counts as gone.
    Zombie,
}

/// What one walk of the process table yields: pid → (ppid, start time).
///
/// A map rather than a list because the sweep's PPID closure is a
/// repeated "is this pid's parent already in the found set" question, and
/// a pair rather than a struct because these two numbers travel together
/// through the whole sweep and neither means anything without the other —
/// a ppid names an edge to walk, a start time names the identity that
/// makes acting on that pid later safe.
pub(crate) type ProcessTable = HashMap<u32, (u32, u64)>;

/// One full walk of the process table, restricted to this euid: pid →
/// (ppid, start time), plus soft per-process errors.
///
/// Fails closed. A problem that prevents the walk from happening AT ALL
/// (no `/proc` to open, a `sysctl` that refuses) is `Err`, never an empty
/// map: reporting "found nothing" when nothing was ever looked at is
/// precisely the failure mode the sweep exists to avoid, so the caller
/// must be able to tell "the tree is clean" from "the tree was never
/// examined". A problem confined to ONE process (an unreadable row on
/// Linux) lands in the returned `Vec<String>` instead, so a single odd
/// process does not blind the scan to every other one.
///
/// The scope is same-euid, mirroring what each platform can actually read:
/// another user's processes are neither killable by this supervisor nor
/// inspectable for markers, so including them would only add rows nothing
/// downstream can act on — and, on a `hidepid`-hardened Linux host, a
/// stream of permission errors for every unrelated process on the machine.
///
/// This is a plain sequential scan, not an atomic snapshot: a fork racing
/// the walk may or may not appear, and (in principle) a pid may exit and
/// be recycled while the walk is still running. That is not fixed here and
/// cannot be; `sweep::kill_process_tree` compensates by re-walking between
/// signal phases, and `sweep::signal_validated` re-checks identity at the
/// moment of signaling.
pub(crate) fn snapshot() -> Result<(ProcessTable, Vec<String>), String> {
    imp::snapshot()
}

/// One process's `(ppid, start time, state)`, or `Ok(None)` when it is
/// simply gone.
///
/// `Ok(None)` is the ordinary, expected outcome of racing a process's own
/// exit and is never worth reporting. Everything else that goes wrong — a
/// permission error for a pid this supervisor is supposed to own, a
/// malformed row, a syscall failure that is not "no such process" — comes
/// back as `Err` rather than being folded into "gone". That direction is
/// load-bearing in both of this function's callers: `signal_validated`
/// would otherwise skip a process it could not read and call the sweep
/// clean, and `confirm_gone` would count an unreadable survivor as
/// confirmed dead.
pub(crate) fn read_process(pid: u32) -> Result<Option<(u32, u64, ProcessState)>, String> {
    imp::read_process(pid)
}

/// One process's exec-time environment as the kernel's own
/// NUL-delimited `KEY=VALUE\0KEY=VALUE\0...` block, or `None` when it
/// cannot be read.
///
/// There is deliberately no error channel. Every way this can fail —
/// the process exited, it belongs to another user, it is non-dumpable
/// (`PR_SET_DUMPABLE(0)`, or any setuid exec, which clears dumpability),
/// macOS refuses `KERN_PROCARGS2` for a process this one does not own — is
/// routine for a scan that sweeps every process on the host, and all of
/// them mean the same thing to the caller: no marker could be read here,
/// so this process is not claimed. That is the SAFE direction (a sweep
/// never signals what it could not identify) and it is also the accepted
/// residual documented on `sweep::environ_marker_verdict`: an
/// environment-scrubbing descendant escapes the marker scan, and only a
/// cgroup can close that.
///
/// One macOS-specific answer shape callers must know about: for an Apple
/// PLATFORM binary, macOS 26+ answers `Some` with the environment region
/// simply ABSENT (argv-only, even for a same-uid direct child — observed
/// on macOS 26.5.1, where `/bin/sleep` answered 29 bytes from a megabyte
/// buffer while a locally built child answered in full). To this module
/// that is indistinguishable from a genuinely empty environment, so it is
/// NOT collapsed to `None`; it surfaces as "no marker", and the residual
/// it creates for the sweep is documented where the sweep reasons about
/// residuals. The test pinning the behavior is
/// `a_platform_binary_childs_environment_is_withheld_on_modern_macos`.
pub(crate) fn read_environ(pid: u32) -> Option<Vec<u8>> {
    imp::read_environ(pid)
}

/// Extract the environment region of a macOS `KERN_PROCARGS2` buffer,
/// re-joined as the NUL-delimited block [`read_environ`] promises.
///
/// # Why this excludes argv, and why that is load-bearing
///
/// `KERN_PROCARGS2` hands back one blob holding the exec path, the full
/// argv, AND the environment; `/proc/<pid>/environ` on Linux holds the
/// environment alone. The platforms MUST agree that only what a process
/// was exec'd with in its envp can claim it for a kill sweep, because argv
/// is trivially attacker- and accident-controlled from outside: a user
/// running `grep FARHELM_SESSION_ID=<id> ...`, or an editor opening a file
/// whose name contains the marker text, would otherwise be swept up and
/// SIGKILLed by that session's stop. So the argv region is parsed only to
/// be skipped over.
///
/// # The layout, and what this tolerates
///
/// `[argc: i32 native-endian][exec path\0][\0 padding][argv[0]\0 ...
/// argv[argc-1]\0][env\0 ...]`. The padding after the exec path is
/// alignment slack the kernel inserts, so it is skipped before argv is
/// counted — miscounting there would slide the argv/env boundary and let
/// command-line bytes into the result, which is the one outcome this
/// function must never produce. The environment region ends at the first
/// EMPTY entry or at the end of the buffer; a trailing entry with no
/// terminating NUL (a truncated read) is dropped rather than guessed at.
///
/// The tail of that region may also contain the kernel's "apple" strings
/// (`executable_path=`, `ptr_munge=`, `stack_guard=`, ...), which are not
/// environment variables but are shaped like them. They are harmless
/// passengers: marker matching is by complete, exact NUL-delimited entry,
/// and none of those keys is a farhelm marker.
///
/// `None` for a structurally unusable buffer (too short to hold `argc`, no
/// terminated exec path). Callers treat that exactly like an unreadable
/// environment.
///
/// Compiled on every platform, not just macOS, so its tests run in ordinary
/// CI on Linux: this is pure byte parsing with no syscall behind it, and it
/// is the part of the macOS path most worth pinning. The `test` arm of the
/// cfg is what keeps it from being dead code in a non-mac release build.
#[cfg(any(target_os = "macos", test))]
fn parse_procargs2(buf: &[u8]) -> Option<Vec<u8>> {
    let argc_bytes: [u8; 4] = buf.get(..4)?.try_into().ok()?;
    let argc = i32::from_ne_bytes(argc_bytes).max(0) as usize;

    // The exec path, then whatever alignment padding follows it. The
    // padding skip is unconditional rather than bounded by a count: only
    // one such run exists in the buffer (right here, before argv[0]), and
    // an empty argv[0] — the one thing this could swallow by mistake — is
    // not a shape any exec in this tree produces.
    let mut rest = buf.get(4..)?;
    let path_end = rest.iter().position(|&b| b == 0)?;
    rest = &rest[path_end + 1..];
    let argv_start = rest.iter().position(|&b| b != 0).unwrap_or(rest.len());
    rest = &rest[argv_start..];

    // Step over exactly argc NUL-terminated strings. Running out of buffer
    // mid-argv means there is no environment region at all, which is an
    // empty environment rather than a parse failure.
    for _ in 0..argc {
        match rest.iter().position(|&b| b == 0) {
            Some(end) => rest = &rest[end + 1..],
            None => return Some(Vec::new()),
        }
    }

    let mut environ = Vec::new();
    while let Some(end) = rest.iter().position(|&b| b == 0) {
        if end == 0 {
            // An empty entry terminates the environment region.
            break;
        }
        environ.extend_from_slice(&rest[..end]);
        environ.push(0);
        rest = &rest[end + 1..];
    }
    Some(environ)
}

/// Linux: the process table is `/proc`.
///
/// This is the original M2 implementation, moved here unchanged rather
/// than rewritten — including the `hidepid` handling and the last-`)`
/// `comm` parsing, both of which exist because of specific things real
/// hosts do.
#[cfg(not(target_os = "macos"))]
mod imp {
    use super::ProcessState;
    use std::collections::HashMap;

    /// Whether an I/O error reading some `/proc/<pid>/...` path means the
    /// process (or its row) is simply gone, as opposed to a genuine problem
    /// this sweep must report. `ENOENT` is the ordinary shape (the path
    /// itself vanished), but `ESRCH` comes through this path too: opening a
    /// still-listed `/proc/<pid>/stat` whose process dies mid-read fails
    /// with ESRCH rather than ENOENT (observed on CI — the confirmation poll
    /// raced a SIGKILL'd pid's teardown and reported a false sweep failure),
    /// so both mean the same thing here: nothing left to worry about.
    fn is_gone_errno(e: &std::io::Error) -> bool {
        e.kind() == std::io::ErrorKind::NotFound || e.raw_os_error() == Some(libc::ESRCH)
    }

    /// Read one stat field by its position AFTER the `comm` field (index 0 is
    /// `state`, the third stat field overall). `rest` is everything following
    /// the LAST `)` in a `/proc/<pid>/stat` line — see [`parse_stat`] for why
    /// that boundary is found on raw bytes before this ever touches `&str`.
    fn stat_field(rest: &str, index: usize) -> Option<&str> {
        rest.split_whitespace().nth(index)
    }

    /// Parse a `/proc/<pid>/stat` line's state, parent pid, and kernel
    /// start-time (state is field 3 overall, the first token after `comm`;
    /// start-time is field 22, the 20th token after `comm`) from raw bytes.
    ///
    /// Bytes in, not `&str`, and the `comm` field's own bytes are never
    /// decoded at all: `comm` (the process name in parentheses) is whatever
    /// bytes the process named itself with via `PR_SET_NAME`/`argv[0]` and can
    /// contain arbitrary, non-UTF-8 data, spaces, or even parentheses — so
    /// this locates the LAST `)` as a raw byte search (valid regardless of
    /// what came before it) and only decodes the fixed-format, always-ASCII
    /// fields after it. A `comm` with a non-UTF-8 byte would otherwise fail
    /// the whole read, silently misreporting a live process as "gone" to
    /// [`super::snapshot`]'s caller — exactly the kind of
    /// resource-exhaustion-disguised-as-success bug the sweep works hard
    /// elsewhere to avoid.
    ///
    /// State is what lets the sweep's confirmation step recognize a zombie —
    /// a process that has already exited but has no ancestor left to reap it
    /// — as gone rather than as a stuck SIGKILL. Start-time is what makes a
    /// discovered pid safe to act on LATER, after other work (a signal, a
    /// sleep, another `/proc` walk) has given the kernel a chance to reuse
    /// it: `sweep::signal_validated` re-reads this same field immediately
    /// before signaling and refuses to act unless it still matches, which is
    /// the only way a numeric pid recorded minutes, seconds, or even
    /// microseconds ago can still be trusted.
    fn parse_stat(bytes: &[u8]) -> Result<(u32, u64, char), String> {
        let Some(after_comm) = bytes.iter().rposition(|&b| b == b')') else {
            return Err(format!(
                "stat content has no ')' delimiting comm: {bytes:?}"
            ));
        };
        let rest = std::str::from_utf8(&bytes[after_comm + 1..])
            .map_err(|e| format!("stat fields after comm are not valid UTF-8: {e}"))?;
        let state = stat_field(rest, 0)
            .ok_or("stat content is missing the state field")?
            .chars()
            .next()
            .ok_or("stat content has an empty state field")?;
        let ppid = stat_field(rest, 1)
            .ok_or("stat content is missing the ppid field")?
            .parse::<u32>()
            .map_err(|e| format!("stat ppid field is unparseable: {e}"))?;
        let starttime = stat_field(rest, 19)
            .ok_or("stat content is missing the starttime field")?
            .parse::<u64>()
            .map_err(|e| format!("stat starttime field is unparseable: {e}"))?;
        Ok((ppid, starttime, state))
    }

    /// This process's own effective uid, for [`is_own_pid_dir`].
    fn euid() -> u32 {
        // SAFETY: geteuid takes no arguments and cannot fail.
        unsafe { libc::geteuid() }
    }

    /// Whether `/proc/<pid>` is owned by this process's own effective uid.
    ///
    /// Exists for hosts whose `/proc` is mounted `hidepid=1` (or stricter): a
    /// legitimate, common hardening option under which OTHER users' pid
    /// directories stay visible to `readdir` — so the walk below still
    /// enumerates them — but their contents (`stat`, `environ`, ...)
    /// become `EACCES`. That is routine and expected, not a sweep failure,
    /// so this check runs BEFORE any fail-closed stat parsing: a foreign-uid
    /// pid is skipped outright, rather than letting an ordinary permission
    /// restriction turn into a reported error for every unrelated process on
    /// a shared or hidepid-hardened host. A pid that has already exited (or
    /// otherwise can't be stat'd at the directory level) is not this check's
    /// business to adjudicate — [`read_stat`]'s own `ENOENT` handling covers
    /// that — so failure to read the directory's metadata defaults to "ours",
    /// leaving the decision to the caller's normal fail-closed path.
    fn is_own_pid_dir(pid: u32) -> bool {
        use std::os::unix::fs::MetadataExt;
        match std::fs::metadata(format!("/proc/{pid}")) {
            Ok(metadata) => metadata.uid() == euid(),
            Err(_) => true,
        }
    }

    /// Read and parse `/proc/<pid>/stat`, keeping the raw state character
    /// so [`snapshot`] can ignore it and [`read_process`] can classify it.
    ///
    /// `Ok(None)` means the process is simply gone (`ENOENT`/`ESRCH`).
    /// Anything else that goes wrong — a permission error this process
    /// should never see for a pid it is supposed to own (callers are
    /// expected to have already screened out foreign-uid pids via
    /// [`is_own_pid_dir`]), a malformed or unrecognized stat format — comes
    /// back as `Err` rather than being folded into "gone": treating a real
    /// failure as absence would let the sweep silently under-collect a live
    /// descendant and report itself clean when it was not.
    fn read_stat(pid: u32) -> Result<Option<(u32, u64, char)>, String> {
        match std::fs::read(format!("/proc/{pid}/stat")) {
            Ok(bytes) => parse_stat(&bytes).map(Some),
            Err(e) if is_gone_errno(&e) => Ok(None),
            Err(e) => Err(format!("reading /proc/{pid}/stat: {e}")),
        }
    }

    /// See [`super::snapshot`]. `readdir` over `/proc`, one numeric entry
    /// at a time; foreign-uid pids are skipped before either read can turn
    /// an ordinary permission restriction into a reported failure.
    pub(super) fn snapshot() -> Result<(super::ProcessTable, Vec<String>), String> {
        let mut stats = HashMap::new();
        let mut soft_errors = Vec::new();
        let entries = std::fs::read_dir("/proc").map_err(|e| format!("reading /proc: {e}"))?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    soft_errors.push(format!("iterating /proc: {e}"));
                    continue;
                }
            };
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            if !is_own_pid_dir(pid) {
                continue;
            }
            match read_stat(pid) {
                Ok(Some((ppid, starttime, _state))) => {
                    stats.insert(pid, (ppid, starttime));
                }
                Ok(None) => {}
                Err(e) => soft_errors.push(e),
            }
        }
        Ok((stats, soft_errors))
    }

    /// See [`super::read_process`]. `Z` is the kernel's zombie state
    /// letter; every other state means the process can still run.
    pub(super) fn read_process(pid: u32) -> Result<Option<(u32, u64, ProcessState)>, String> {
        Ok(read_stat(pid)?.map(|(ppid, starttime, state)| {
            let state = if state == 'Z' {
                ProcessState::Zombie
            } else {
                ProcessState::Running
            };
            (ppid, starttime, state)
        }))
    }

    /// See [`super::read_environ`]. `/proc/<pid>/environ` is already
    /// exactly the NUL-delimited block the contract describes, so there is
    /// nothing to parse: every failure (gone, foreign uid, non-dumpable)
    /// collapses to `None`.
    pub(super) fn read_environ(pid: u32) -> Option<Vec<u8>> {
        std::fs::read(format!("/proc/{pid}/environ")).ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// `parse_stat`'s whole reason to exist: a `comm` field containing
        /// spaces AND a stray closing paren must not fool the last-`)` search
        /// into stopping early. This pins the kernel's actual escape hatch —
        /// `comm` can contain anything, including `)`, so only the LAST `)`
        /// in the whole line is the real delimiter, no matter how many
        /// look-alikes precede it.
        #[test]
        fn parse_stat_handles_comm_with_parens_and_spaces() {
            // comm = "1 (weird) name)" — spaces, an internal paren pair, AND
            // a trailing stray ')' that is NOT the kernel's own delimiter.
            // Wrapped by the kernel in its own parens, the line's tail reads
            // "...name))" — two closing parens back to back — and only the
            // second is real.
            let line: &[u8] =
                b"123 (1 (weird) name)) S 456 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 789";
            let (ppid, starttime, state) =
                parse_stat(line).expect("well-formed synthetic stat line");
            assert_eq!(ppid, 456, "ppid must be read from AFTER the true delimiter");
            assert_eq!(starttime, 789, "starttime is the 20th field after comm");
            assert_eq!(state, 'S', "state is the first field after comm");
        }

        /// `comm` is whatever bytes the process named itself with — it can be
        /// genuinely non-UTF-8 — and `parse_stat` must not choke on that, since
        /// only the LAST `)` is located via a raw byte search and everything
        /// before it (the non-UTF-8 comm included) is never decoded at all.
        /// Failing this would misreport a live, oddly-named process as
        /// unparseable, folding it into a reported sweep error over nothing
        /// more than a name it never chose to be `/proc`-friendly about.
        #[test]
        fn parse_stat_survives_non_utf8_bytes_in_comm() {
            let mut line = b"123 (bad".to_vec();
            line.push(0xff); // not valid UTF-8 on its own or in context here
            line.extend_from_slice(b"name) Z 456 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 789");
            let (ppid, starttime, state) =
                parse_stat(&line).expect("a non-UTF-8 comm must not fail parsing");
            assert_eq!(ppid, 456);
            assert_eq!(starttime, 789);
            assert_eq!(state, 'Z');
        }

        /// A stat line with no `)` at all (never happens for a real kernel-
        /// written row, but a corrupted read or a hostile fixture could
        /// produce one) must be a reported parse error, not a silent "gone" —
        /// conflating "malformed" with "absent" would let a genuinely live,
        /// misread process vanish from a sweep without a trace.
        #[test]
        fn parse_stat_rejects_a_line_with_no_delimiter() {
            assert!(parse_stat(b"garbage with no parens at all").is_err());
        }

        /// The zombie mapping is the one state distinction the sweep's
        /// confirmation step depends on, and it is asserted here against a
        /// REAL process rather than a fixture because the mapping is only
        /// meaningful if the live read produces it: this process is its own
        /// witness, and it is certainly not a zombie.
        #[test]
        fn a_live_process_reads_back_as_running() {
            let me = std::process::id();
            let (_, _, state) = read_process(me)
                .expect("reading this process's own stat must not error")
                .expect("this process certainly exists");
            assert_eq!(state, ProcessState::Running);
        }
    }
}

/// macOS: the process table is `sysctl`, since there is no `/proc`.
///
/// Two `sysctl` trees carry everything the sweep needs. `kern.proc`
/// (`KERN_PROC_ALL` / `KERN_PROC_PID`) yields `struct kinfo_proc` rows with
/// parent pid, effective uid, start time, and process state; `kern.procargs2`
/// yields one process's exec-time argv and environment. Both are readable
/// without privilege for one's OWN processes and refuse (or omit) other
/// users', which is the same same-euid scope the Linux walk enforces
/// explicitly.
///
/// # Why the layout is spelled out here
///
/// The `libc` crate defines `kinfo_proc` for the BSDs but not for Apple, so
/// the layout below is transcribed from `<sys/sysctl.h>` (`kinfo_proc`,
/// `eproc`, `_pcred`, `_ucred`) and `<sys/proc.h>` (`extern_proc`) — a
/// frozen, decades-old ABI that `ps` and every process lister on the
/// platform already depend on. Only four fields are ever read; every other
/// field exists solely to place those four at the right offsets, which is
/// why the whole thing is `#[allow(dead_code)]`. Because a silent layout
/// mistake here would misread parent pids and uids rather than fail loudly,
/// the size and all four read offsets are asserted at COMPILE time against
/// the values Apple's headers produce.
#[cfg(target_os = "macos")]
mod imp {
    use super::ProcessState;
    use std::collections::HashMap;
    use std::ffi::{c_char, c_int, c_short, c_uchar, c_uint, c_ushort, c_void};
    use std::mem::{offset_of, size_of};

    /// `struct extern_proc` from `<sys/proc.h>`: the "process" half of a
    /// `kinfo_proc` row. Only `p_starttime` and `p_stat` are read.
    ///
    /// The leading field is a union in C (`p_un`, holding either a
    /// run-queue pointer pair or the start time); both arms are 16 bytes,
    /// and only the start-time arm is meaningful in a `kinfo_proc` handed
    /// out by `sysctl`, so it is spelled as the `timeval` directly.
    #[repr(C)]
    #[allow(dead_code, non_snake_case)]
    struct ExternProc {
        p_starttime: libc::timeval,
        p_vmspace: *mut c_void,
        p_sigacts: *mut c_void,
        p_flag: c_int,
        p_stat: c_char,
        p_pid: libc::pid_t,
        p_oppid: libc::pid_t,
        p_dupfd: c_int,
        user_stack: *mut c_char,
        exit_thread: *mut c_void,
        p_debugger: c_int,
        sigwait: c_int,
        p_estcpu: c_uint,
        p_cpticks: c_int,
        p_pctcpu: u32,
        p_wchan: *mut c_void,
        p_wmesg: *mut c_char,
        p_swtime: c_uint,
        p_slptime: c_uint,
        p_realtimer: libc::itimerval,
        p_rtime: libc::timeval,
        p_uticks: u64,
        p_sticks: u64,
        p_iticks: u64,
        p_traceflag: c_int,
        p_tracep: *mut c_void,
        p_siglist: c_int,
        p_textvp: *mut c_void,
        p_holdcnt: c_int,
        p_sigmask: u32,
        p_sigignore: u32,
        p_sigcatch: u32,
        p_priority: c_uchar,
        p_usrpri: c_uchar,
        p_nice: c_char,
        p_comm: [c_char; 17],
        p_pgrp: *mut c_void,
        p_addr: *mut c_void,
        p_xstat: c_ushort,
        p_acflag: c_ushort,
        p_ru: *mut c_void,
    }

    /// `struct _pcred` from `<sys/sysctl.h>`. Never read; present to place
    /// the `_ucred` that follows it.
    #[repr(C)]
    #[allow(dead_code)]
    struct PCred {
        pc_lock: [c_char; 72],
        pc_ucred: *mut c_void,
        p_ruid: libc::uid_t,
        p_svuid: libc::uid_t,
        p_rgid: libc::gid_t,
        p_svgid: libc::gid_t,
        p_refcnt: c_int,
    }

    /// `struct _ucred` from `<sys/sysctl.h>`. `cr_uid` is the effective uid
    /// the same-euid filter compares against.
    #[repr(C)]
    #[allow(dead_code)]
    struct UCred {
        cr_ref: i32,
        cr_uid: libc::uid_t,
        cr_ngroups: c_short,
        cr_groups: [libc::gid_t; 16],
    }

    /// `struct vmspace` from `<sys/vm.h>`, in the shape `eproc` embeds it.
    /// Never read; present for its 64 bytes of offset.
    #[repr(C)]
    #[allow(dead_code)]
    struct VmSpace {
        vm_refcnt: c_int,
        vm_shm: *mut c_char,
        vm_rssize: i32,
        vm_swrss: i32,
        vm_tsize: i32,
        vm_dsize: i32,
        vm_ssize: i32,
        vm_taddr: *mut c_char,
        vm_daddr: *mut c_char,
        vm_maxsaddr: *mut c_char,
    }

    /// `struct eproc` from `<sys/sysctl.h>`: the "external" half of a
    /// `kinfo_proc` row. Only `e_ucred.cr_uid` and `e_ppid` are read.
    #[repr(C)]
    #[allow(dead_code)]
    struct EProc {
        e_paddr: *mut c_void,
        e_sess: *mut c_void,
        e_pcred: PCred,
        e_ucred: UCred,
        e_vm: VmSpace,
        e_ppid: libc::pid_t,
        e_pgid: libc::pid_t,
        e_jobc: c_short,
        e_tdev: i32,
        e_tpgid: libc::pid_t,
        e_tsess: *mut c_void,
        e_wmesg: [c_char; 8],
        e_xsize: i32,
        e_xrssize: c_short,
        e_xccount: c_short,
        e_xswrss: c_short,
        e_flag: i32,
        e_login: [c_char; 12],
        e_spare: [i32; 4],
    }

    /// `struct kinfo_proc` from `<sys/sysctl.h>` — one row of the
    /// `kern.proc` sysctl's answer.
    #[repr(C)]
    struct KinfoProc {
        kp_proc: ExternProc,
        kp_eproc: EProc,
    }

    /// The layout guard. These are the values Apple's own headers produce
    /// on 64-bit Darwin (verified by compiling `offsetof` assertions against
    /// them); a transcription slip anywhere above would move one of them and
    /// break the build, which is enormously preferable to reading a parent
    /// pid out of the middle of some other field at runtime and quietly
    /// sweeping the wrong tree.
    const _: () = {
        assert!(size_of::<KinfoProc>() == 648);
        assert!(offset_of!(KinfoProc, kp_proc.p_starttime) == 0);
        assert!(offset_of!(KinfoProc, kp_proc.p_stat) == 36);
        assert!(offset_of!(KinfoProc, kp_eproc.e_ucred.cr_uid) == 420);
        assert!(offset_of!(KinfoProc, kp_eproc.e_ppid) == 560);
    };

    /// How many times the `KERN_PROC_ALL` fetch re-asks for a size after
    /// the table grew between the sizing call and the fetch.
    ///
    /// The race is inherent to the two-call `sysctl` idiom (ask how big,
    /// then read) and is ordinary on a busy host, not pathological — each
    /// retry re-sizes against a fresher table, so convergence is the normal
    /// case and this bound is only a guard against livelock under a fork
    /// storm. Exhausting it is a hard [`snapshot`] error, never an empty
    /// map, per this module's fail-closed contract.
    const MAX_SIZE_RETRIES: usize = 5;

    /// Slack rows added on top of the size `sysctl` reported, so the common
    /// case of a handful of processes starting between the two calls costs
    /// nothing instead of a retry.
    const SIZE_HEADROOM_ROWS: usize = 32;

    /// The process's own effective uid — the scope [`snapshot`] restricts
    /// itself to, mirroring the Linux walk's `hidepid` skip.
    fn euid() -> libc::uid_t {
        // SAFETY: geteuid takes no arguments and cannot fail.
        unsafe { libc::geteuid() }
    }

    /// The start-time identity token for one row: the process's start
    /// `timeval` folded to whole microseconds.
    ///
    /// Injective and stable over a process's life, which is all the sweep's
    /// pid-reuse check needs — and it must be computed here and ONLY here,
    /// because [`snapshot`] and [`read_process`] compare their results
    /// against each other. Clamping the components at zero rather than
    /// casting blindly keeps a nonsensical negative from the kernel (which
    /// should never happen) from wrapping into a value that could collide
    /// with a real one.
    fn starttime_key(start: &libc::timeval) -> u64 {
        let secs = start.tv_sec.max(0) as u64;
        let usecs = start.tv_usec.max(0) as u64;
        secs.saturating_mul(1_000_000).saturating_add(usecs)
    }

    /// Classify one row's `p_stat`. `SZOMB` (from `<sys/proc.h>`) is the
    /// value a process wears once it has exited and is waiting to be
    /// reaped; every other value — running, sleeping, stopped, idle —
    /// means it can still execute code, which is the only distinction the
    /// sweep draws.
    fn state_of(row: &KinfoProc) -> ProcessState {
        if row.kp_proc.p_stat as u32 == libc::SZOMB {
            ProcessState::Zombie
        } else {
            ProcessState::Running
        }
    }

    /// Fetch `kern.proc` rows for one selector (`KERN_PROC_ALL` with `arg`
    /// 0, or `KERN_PROC_PID` with a pid).
    ///
    /// Sizes with a `NULL` probe, allocates with headroom, and retries on
    /// `ENOMEM` — the classic Darwin process-table race, where the answer
    /// outgrows the buffer between the two calls. An empty result is a
    /// legitimate answer (`KERN_PROC_PID` for a pid that no longer exists),
    /// not an error.
    fn kern_proc(selector: c_int, arg: c_int) -> std::io::Result<Vec<KinfoProc>> {
        let mut mib: [c_int; 4] = [libc::CTL_KERN, libc::KERN_PROC, selector, arg];
        for _ in 0..MAX_SIZE_RETRIES {
            let mut needed: libc::size_t = 0;
            // SAFETY: `mib` is a live array of exactly the 4 elements
            // declared to sysctl; a NULL `oldp` with a non-NULL `oldlenp`
            // is the documented "how big is the answer" form and writes
            // only through `needed`.
            let sized = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    4,
                    std::ptr::null_mut(),
                    &mut needed,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if sized != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if needed == 0 {
                return Ok(Vec::new());
            }

            let rows = needed.div_ceil(size_of::<KinfoProc>()) + SIZE_HEADROOM_ROWS;
            let mut buf: Vec<KinfoProc> = Vec::with_capacity(rows);
            let mut len = rows * size_of::<KinfoProc>();
            // SAFETY: `buf` has capacity for `rows` rows and `len` is
            // exactly that many bytes, so sysctl writes only within the
            // allocation; it reports how much it actually wrote back
            // through `len`, and `set_len` below admits only whole rows the
            // kernel initialized. Every field of `KinfoProc` is a plain
            // integer, array, or raw pointer, so any initialized bit
            // pattern is a valid value.
            let fetched = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    4,
                    buf.as_mut_ptr().cast::<c_void>(),
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if fetched != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ENOMEM) {
                    // The table grew past even the headroom: re-size and
                    // try again rather than reporting a failure for what is
                    // just a busy host.
                    continue;
                }
                return Err(err);
            }
            // SAFETY: sysctl wrote `len` bytes into the allocation; whole
            // rows within that are initialized.
            unsafe { buf.set_len(len / size_of::<KinfoProc>()) };
            return Ok(buf);
        }
        Err(std::io::Error::other(format!(
            "the kern.proc table kept outgrowing its buffer across {MAX_SIZE_RETRIES} attempts"
        )))
    }

    /// See [`super::snapshot`]. One `KERN_PROC_ALL` sysctl, filtered to
    /// this euid.
    ///
    /// The soft-error list is always empty here and that is not an
    /// oversight: the whole table arrives in a single syscall, so there is
    /// no per-process read that could fail on its own. Either the table was
    /// read or it was not.
    ///
    /// pid 0 (the kernel task) is dropped rather than filtered by uid: it
    /// runs as root on every Mac, so the euid filter already excludes it
    /// for an ordinary user, and dropping it explicitly means a supervisor
    /// running as root cannot end up with it as a closure root.
    pub(super) fn snapshot() -> Result<(super::ProcessTable, Vec<String>), String> {
        let rows = kern_proc(libc::KERN_PROC_ALL, 0)
            .map_err(|e| format!("reading the kern.proc process table: {e}"))?;
        let me = euid();
        let mut stats = HashMap::new();
        for row in &rows {
            let pid = row.kp_proc.p_pid;
            if pid <= 0 || row.kp_eproc.e_ucred.cr_uid != me {
                continue;
            }
            let ppid = row.kp_eproc.e_ppid.max(0) as u32;
            stats.insert(pid as u32, (ppid, starttime_key(&row.kp_proc.p_starttime)));
        }
        Ok((stats, Vec::new()))
    }

    /// See [`super::read_process`]. One `KERN_PROC_PID` sysctl.
    ///
    /// An empty answer means the pid is gone, and so does `ESRCH`: Darwin
    /// has answered both ways for a vanished pid across releases, and the
    /// two mean the same thing to the sweep. No uid filter here, matching
    /// Linux: callers name pids they already screened (or the pane process
    /// they were handed), and a foreign pid simply fails the identity check
    /// that follows.
    pub(super) fn read_process(pid: u32) -> Result<Option<(u32, u64, ProcessState)>, String> {
        let rows = match kern_proc(libc::KERN_PROC_PID, pid as c_int) {
            Ok(rows) => rows,
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
            Err(e) => return Err(format!("reading kern.proc for pid {pid}: {e}")),
        };
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        Ok(Some((
            row.kp_eproc.e_ppid.max(0) as u32,
            starttime_key(&row.kp_proc.p_starttime),
            state_of(row),
        )))
    }

    /// The kernel's `kern.argmax`: the hard ceiling on one process's
    /// combined argv and environment, and therefore on any
    /// `KERN_PROCARGS2` answer.
    ///
    /// This is [`read_environ`]'s ONLY buffer sizing, not a fallback, and
    /// that is a correctness requirement rather than a simplification: XNU
    /// has a long-standing `KERN_PROCARGS2` bug (observed on this
    /// project's own hardware, macOS 26.5.1) where the NULL-`oldp` size probe
    /// underestimates — it fails to count the 17-byte `executable_path=`
    /// prefix — and a fetch into a buffer of exactly the probed size does
    /// not fail: it SUCCEEDS and fills the buffer with zeros. A zero-fill
    /// parses as an empty environment, which silently disabled the entire
    /// marker half of the sweep. Apple's own `ps` has always sized this
    /// buffer from `kern.argmax` instead of probing, and crashpad and
    /// golang's x/sys carry explicit workarounds for the same bug:
    /// <https://groups.google.com/a/chromium.org/g/crashpad-dev/c/ASKdHGWG5bA>,
    /// <https://github.com/golang/go/issues/60047>. Since the args area
    /// can never exceed argmax by definition, an argmax-sized buffer can
    /// never be "too small" and the buggy path is unreachable.
    ///
    /// Cached because it is fixed for the life of the boot while
    /// [`read_environ`] runs once per process per sweep round. A failed
    /// read falls back to a generous fixed size rather than giving up —
    /// deliberately NOT the 4 KiB POSIX `ARG_MAX` floor, which on a host
    /// with a bigger real argmax would recreate the undersized-buffer
    /// zero-fill above. One megabyte is `kern.argmax`'s actual value on
    /// every macOS this project has met.
    fn arg_max() -> usize {
        static CACHED: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        *CACHED.get_or_init(|| {
            let mut mib: [c_int; 2] = [libc::CTL_KERN, libc::KERN_ARGMAX];
            let mut value: c_int = 0;
            let mut len = size_of::<c_int>();
            // SAFETY: `mib` holds exactly the 2 elements declared, and the
            // out-buffer is one live `c_int` with `len` saying so.
            let ok = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    2,
                    (&raw mut value).cast::<c_void>(),
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            } == 0;
            if ok && value > 0 {
                value as usize
            } else {
                1024 * 1024
            }
        })
    }

    /// See [`super::read_environ`]. One `kern.procargs2` sysctl, parsed by
    /// [`super::parse_procargs2`] — which is where the argv region gets
    /// dropped, and where the reasoning for dropping it lives.
    ///
    /// Sized from [`arg_max`], never from the NULL-`oldp` probe. A probe
    /// looks like the ordinary idiom and is exactly wrong for THIS node:
    /// XNU's estimate comes back short, and a fetch into a buffer of the
    /// probed size succeeds while writing zeros — see [`arg_max`]'s docs
    /// for the bug's shape and references. The first cut here probed, and
    /// on real hardware every environment came back empty: the sweep's
    /// marker half was silently disabled while every test that does not
    /// spawn a real process still passed.
    ///
    /// Every failure collapses to `None`: Darwin answers `EINVAL` for a
    /// process this one does not own (there is no readable-but-empty case
    /// to distinguish), `ESRCH` for one that exited, and the caller treats
    /// all of it as "no marker" regardless.
    pub(super) fn read_environ(pid: u32) -> Option<Vec<u8>> {
        let mut mib: [c_int; 3] = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as c_int];
        let needed = arg_max();
        let mut buf = vec![0u8; needed];
        let mut len = needed;
        // SAFETY: `buf` owns `needed` bytes and `len` says exactly that, so
        // sysctl writes only within the allocation; `len` comes back as the
        // number of bytes actually written, and the truncate below keeps
        // the parser away from anything the kernel did not fill in.
        let fetched = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                buf.as_mut_ptr().cast::<c_void>(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if fetched != 0 {
            return None;
        }
        buf.truncate(len);
        super::parse_procargs2(&buf)
    }
}

/// A marked, long-lived child process for kill-sweep and environ tests —
/// THIS test binary re-invoked in a sleeper mode, never a system binary.
///
/// Why the contortion instead of `sh -c "sleep 30"`: macOS 26+ withholds
/// the entire environment region of `KERN_PROCARGS2` when the target's
/// exec'd image is an Apple PLATFORM binary — observed on macOS 26.5.1,
/// where `/bin/sleep` and `sh` children answered argv-only (`len` covering
/// exactly argc + exec path + argv) while the same read of a locally
/// compiled child returned the full environment. A test child that the
/// marker scan cannot see does not exercise the marker mechanism; it
/// proves nothing and fails on every modern Mac. The one binary a test can
/// rely on being locally built — and therefore readable — is the test
/// executable itself, which libtest happily re-invokes as a single-test
/// child. (The withholding itself is pinned by its own test below, so if
/// Apple ever lifts it, the record gets corrected rather than silently
/// drifting.)
///
/// The child announces itself on stdout before sleeping and [`spawn`]
/// waits for that line. The handshake is load-bearing, not tidiness:
/// `spawn` returns once the child EXISTS, not once it has finished
/// `exec`ing, and a pre-exec child still shows its parent's environment —
/// under load that read raced and failed for reasons unrelated to what any
/// caller pins. It also confirms libtest really reached the sleeper test,
/// rather than exiting early over an argument-parsing change.
#[cfg(test)]
pub(crate) mod sleeper {
    /// Printed by the sleeper child once its own test body is running —
    /// i.e., strictly after `exec`, when its environment is its own.
    const READY_LINE: &str = "SLEEPER_READY";

    /// The env var that turns the sleeper test below into an actual
    /// sleeper. Set on the CHILD only (`Command::env`); the test process's
    /// own environment is never touched (a repo-wide rule).
    const SLEEPER_MODE_ENV: &str = "FARHELM_TEST_SLEEPER_MODE";

    /// Build the sleeper's command: the test binary re-invoked in sleeper
    /// mode, carrying exactly the farhelm markers `extra_env` declares and
    /// no others.
    ///
    /// The ambient-marker scrub is the load-bearing part. The test runner
    /// itself may be running inside a Farhelm session — the ordinary state
    /// of an agent developing farhelm on a farhelm-supervised box — where
    /// `FARHELM_AGENT_ID` (or, from a tab, `FARHELM_TAB_ID`) sits in the
    /// runner's own environment. A sleeper inheriting a marker its caller
    /// never declared is no longer the shape the caller meant to spawn:
    /// the sweep tests' marked process, declared with a session marker
    /// only, picked up the host session's agent marker and read to the
    /// sweep as another launch's agent — which the cross-session boundary
    /// correctly refuses, failing four sweep tests on such hosts while CI
    /// (marker-free) stayed green. Same bug class, and same child-only
    /// remedy, as `MarkedDecoy::command`'s scrub in the e2e harness.
    /// `env_remove` runs before `extra_env` is applied, so a test that
    /// WANTS a marker still gets it by declaring it.
    ///
    /// Split from [`spawn`] so a test can assert the CONFIGURED env ops
    /// via `Command::get_envs`: the scrub only changes a live child's
    /// environment on a host whose runner carries the markers, so
    /// inspecting the child would prove nothing on clean CI, while the
    /// builder's op list is the same everywhere.
    fn command(extra_env: &[(&str, &str)]) -> std::process::Command {
        let exe = std::env::current_exe().expect("the test binary knows its own path");
        let mut cmd = std::process::Command::new(exe);
        // `--exact` addresses the one test by its full libtest path;
        // `--nocapture` is what lets its READY line reach the pipe instead
        // of libtest's capture buffer.
        cmd.args([
            "--exact",
            "procs::sleeper::runs_as_a_sleeper_child_when_asked",
            "--nocapture",
        ]);
        cmd.env(SLEEPER_MODE_ENV, "1");
        cmd.env_remove(crate::launch::SESSION_ID_ENV_VAR);
        cmd.env_remove(crate::launch::AGENT_ID_ENV_VAR);
        cmd.env_remove(crate::launch::TAB_ID_ENV_VAR);
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        cmd
    }

    /// Spawn the sleeper with `extra_env` in its environment, returning
    /// once it is provably past `exec`. The caller owns `kill`/`wait`;
    /// clippy's zombie lint cannot see across that handoff, and the panic
    /// paths in here leak at most one 30-second sleeper.
    #[allow(clippy::zombie_processes)]
    pub(crate) fn spawn(extra_env: &[(&str, &str)]) -> std::process::Child {
        use std::io::BufRead as _;
        let mut child = command(extra_env)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("re-spawning the test binary as a sleeper");
        let stdout = child.stdout.take().expect("a piped stdout was requested");
        // libtest prints its own header lines first; scan until the
        // sleeper speaks. EOF before that means it never started sleeping
        // (typically a libtest CLI change) — fail here, loudly, rather
        // than let the caller's assertion fire for the wrong reason.
        let mut lines = std::io::BufReader::new(stdout).lines();
        loop {
            let line = lines
                .next()
                .expect("the sleeper child exited before announcing readiness")
                .expect("reading the sleeper child's stdout");
            if line.trim() == READY_LINE {
                return child;
            }
        }
    }

    /// The sleeper's builder must scrub every farhelm marker the runner
    /// could ambiently carry, and a declared marker must survive the
    /// scrub.
    ///
    /// This pins the fix for the four sweep tests that failed only when
    /// the suite ran inside a Farhelm session (see [`command`]'s docs for
    /// the mechanism). Asserted against `Command::get_envs` — the
    /// CONFIGURED operations — because a live child's environment only
    /// differs from a clean one on a polluted host, so this is the one
    /// observation that fails the same way everywhere, CI included.
    #[test]
    fn sleeper_command_scrubs_ambient_markers_but_keeps_declared_ones() {
        let cmd = command(&[(crate::launch::SESSION_ID_ENV_VAR, "declared-session")]);
        let configured: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .map(|(key, value)| (key.to_os_string(), value.map(|v| v.to_os_string())))
            .collect();
        assert_eq!(
            configured
                .get(std::ffi::OsStr::new(crate::launch::SESSION_ID_ENV_VAR))
                .cloned(),
            Some(Some("declared-session".into())),
            "a marker the caller declares must survive the scrub"
        );
        for removed in [
            crate::launch::AGENT_ID_ENV_VAR,
            crate::launch::TAB_ID_ENV_VAR,
        ] {
            assert_eq!(
                configured.get(std::ffi::OsStr::new(removed)).cloned(),
                Some(None),
                "{removed} must be configured as removed, or a runner inside a farhelm \
                 session leaks it into every sleeper"
            );
        }
    }

    /// Not a test of anything: the body [`spawn`]'s children execute. In
    /// an ordinary suite run the mode env var is absent and this returns
    /// immediately as a trivially green test; as a spawned child it
    /// announces readiness and sleeps. The 30s bound is the leak ceiling
    /// if a caller panics before its sweep runs, same as the `sleep 30`
    /// it replaced.
    #[test]
    fn runs_as_a_sleeper_child_when_asked() {
        if std::env::var_os(SLEEPER_MODE_ENV).is_none() {
            return;
        }
        use std::io::Write as _;
        println!("{READY_LINE}");
        std::io::stdout()
            .flush()
            .expect("flushing the readiness line");
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `KERN_PROCARGS2` buffer the way the Darwin kernel lays one
    /// out, so the parser is exercised against the real shape rather than
    /// against its own assumptions: `argc`, the exec path, alignment
    /// padding, argv, then the environment.
    fn procargs2(argv: &[&str], environ: &[&str], padding: usize) -> Vec<u8> {
        let mut buf = (argv.len() as i32).to_ne_bytes().to_vec();
        buf.extend_from_slice(b"/usr/bin/thing\0");
        buf.extend(std::iter::repeat_n(0u8, padding));
        for arg in argv {
            buf.extend_from_slice(arg.as_bytes());
            buf.push(0);
        }
        for entry in environ {
            buf.extend_from_slice(entry.as_bytes());
            buf.push(0);
        }
        buf
    }

    /// The parser's contract in one pass: the environment region comes back
    /// as a NUL-delimited block, and the exec path, the alignment padding,
    /// and every argv element are gone from it.
    ///
    /// Worth pinning as its own test because the argv skip is counted, not
    /// searched for: an off-by-one in the padding skip or the argc loop
    /// shifts the boundary and silently leaks command-line bytes into what
    /// the sweep treats as a process's environment.
    #[test]
    fn procargs2_parsing_yields_the_environment_region_alone() {
        let buf = procargs2(
            &["thing", "--flag", "value"],
            &["PATH=/bin", "FARHELM_SESSION_ID=abc-123"],
            7,
        );
        let environ = parse_procargs2(&buf).expect("a well-formed buffer must parse");
        assert_eq!(environ, b"PATH=/bin\0FARHELM_SESSION_ID=abc-123\0");
    }

    /// The reason argv is excluded at all: a process whose COMMAND LINE
    /// carries a session marker must not be claimed by that session's
    /// sweep.
    ///
    /// This is the platform-agreement claim, and it is a safety one. Linux
    /// reads `/proc/<pid>/environ`, which can never contain argv; macOS
    /// reads a buffer that holds both. If the argv region leaked through,
    /// `grep FARHELM_SESSION_ID=<id> logfile` — or an editor with such a
    /// file open — would be SIGKILLed by a stop it has nothing to do with,
    /// on macOS only.
    #[test]
    fn a_marker_on_the_command_line_never_reaches_the_environment_block() {
        let marker = "FARHELM_SESSION_ID=abc-123";
        let buf = procargs2(&["grep", marker, "app.log"], &["PATH=/bin"], 3);
        let environ = parse_procargs2(&buf).expect("a well-formed buffer must parse");
        assert_eq!(environ, b"PATH=/bin\0");
        assert!(
            !environ
                .windows(marker.len())
                .any(|w| w == marker.as_bytes()),
            "argv bytes must not appear anywhere in the environment block"
        );
    }

    /// An empty entry ends the environment region, and a truncated trailing
    /// entry is dropped rather than guessed at.
    ///
    /// Both matter for the same reason: the block this returns is fed
    /// straight to exact, whole-entry marker matching, so a half-read entry
    /// admitted as if it were complete could match a marker whose value was
    /// cut short — claiming a process for the wrong session.
    #[test]
    fn procargs2_parsing_stops_at_an_empty_entry_and_drops_a_truncated_tail() {
        let mut buf = procargs2(&["thing"], &["A=1"], 1);
        buf.push(0); // the empty entry that ends the region
        buf.extend_from_slice(b"B=2\0");
        assert_eq!(parse_procargs2(&buf).unwrap(), b"A=1\0");

        let mut truncated = procargs2(&["thing"], &["A=1"], 1);
        truncated.extend_from_slice(b"FARHELM_SESSION_ID=abc"); // no NUL
        assert_eq!(parse_procargs2(&truncated).unwrap(), b"A=1\0");
    }

    /// Structurally unusable buffers must answer `None` (which the caller
    /// reads as "no marker") rather than panicking or inventing an
    /// environment out of whatever bytes are present. A short read from a
    /// process exiting mid-sysctl is the realistic source.
    #[test]
    fn procargs2_parsing_rejects_buffers_it_cannot_trust() {
        assert!(parse_procargs2(b"").is_none());
        assert!(parse_procargs2(b"\x01\x00").is_none(), "argc is truncated");
        assert!(
            parse_procargs2(b"\x01\x00\x00\x00/usr/bin/thing").is_none(),
            "the exec path has no terminator, so nothing after it can be located"
        );
        // argc claims more argv entries than the buffer holds: an empty
        // environment, not a parse failure.
        let buf = procargs2(&["a"], &[], 1);
        let mut greedy = 9i32.to_ne_bytes().to_vec();
        greedy.extend_from_slice(&buf[4..]);
        assert_eq!(parse_procargs2(&greedy).unwrap(), b"");
    }

    /// This process must be able to find ITSELF in the table, with its own
    /// parent and a start time that matches what the single-pid read
    /// reports.
    ///
    /// The agreement between the two reads is the point. `sweep` records a
    /// start time from [`snapshot`] and re-checks it via [`read_process`]
    /// immediately before signaling; if a platform derived the two
    /// differently, every validated signal would be skipped and the sweep
    /// would silently stop killing anything while still reporting success.
    #[test]
    fn snapshot_and_single_reads_agree_about_this_process() {
        let me = std::process::id();
        let (stats, soft_errors) = snapshot().expect("this host's process table must be readable");
        let &(ppid, starttime) = stats
            .get(&me)
            .expect("the walk must contain the process performing it");
        let (single_ppid, single_starttime, state) = read_process(me)
            .expect("reading this process must not error")
            .expect("this process exists");
        assert_eq!(ppid, single_ppid, "both reads must report the same parent");
        assert_eq!(
            starttime, single_starttime,
            "both reads must derive start time identically, or every signal is skipped"
        );
        assert_eq!(state, ProcessState::Running);
        assert!(
            soft_errors.is_empty() || !stats.is_empty(),
            "soft errors must never come at the cost of an empty table: {soft_errors:?}"
        );
    }

    /// A pid that cannot exist is `Ok(None)` — gone — and never an error.
    ///
    /// The distinction is the whole reason the return type is nested:
    /// `confirm_gone` counts `Ok(None)` as a process successfully reaped
    /// and treats `Err` as an unconfirmed kill that fails the stop, so
    /// collapsing them in either direction breaks a user-visible promise.
    #[test]
    fn an_impossible_pid_reads_as_gone_rather_than_as_an_error() {
        // Above every platform's pid_max, so it can never name a process.
        assert_eq!(read_process(u32::from(u16::MAX) * 1024 + 7), Ok(None));
    }

    /// The environment read must return the marker block the sweep matches
    /// on, for a process whose environment this test controls — the seam's
    /// end-to-end claim on whichever platform it runs.
    ///
    /// The child is the test binary itself in sleeper mode, not a system
    /// binary, and on macOS that choice is what the test MEANS: see
    /// [`super::sleeper`] for the platform-binary withholding that makes a
    /// `sh` child unreadable there, and the companion macOS test below
    /// that pins the withholding itself.
    #[test]
    fn a_childs_exec_time_environment_is_readable_as_nul_delimited_entries() {
        let mut child = sleeper::spawn(&[("FARHELM_PROCS_TEST_MARKER", "sentinel-value")]);
        let environ = read_environ(child.id()).expect("a child's environment must be readable");
        let found = environ
            .split(|&b| b == 0)
            .any(|entry| entry == b"FARHELM_PROCS_TEST_MARKER=sentinel-value");
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            found,
            "the exec-time environment must come back as complete NUL-delimited entries"
        );
    }

    /// macOS 26+ withholds the ENVIRONMENT region of `KERN_PROCARGS2` for
    /// Apple platform binaries, even for a same-uid direct child: the
    /// fetch succeeds and answers argv-only. Observed on macOS 26.5.1
    /// (2026-08): a `/bin/sleep` child answered with `len` covering
    /// exactly argc + exec path + argv, while the same read of this test
    /// binary's own child returned the full environment.
    ///
    /// This pins a LIMITATION, deliberately: the sweep's marker scan
    /// cannot see a reparented descendant whose exec'd image is a platform
    /// binary (shells, chiefly), which is an accepted residual documented
    /// on `sweep::environ_marker_verdict` — the deferred follow-up is a
    /// session-id membership channel. If this test ever FAILS, Apple has
    /// started returning environments for platform binaries again: good
    /// news, and the cue to update that documentation rather than a bug.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_platform_binary_childs_environment_is_withheld_on_modern_macos() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .env("FARHELM_PROCS_TEST_MARKER", "sentinel-value")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawning /bin/sleep");
        // No handshake is possible with a child that prints nothing, so
        // poll past the exec window instead; the marker CANNOT appear
        // pre-exec either (the test process's own environment does not
        // carry it), so a positive sighting is definitive whenever it
        // lands.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut marker_seen = false;
        while std::time::Instant::now() < deadline {
            if let Some(environ) = read_environ(child.id()) {
                marker_seen = environ
                    .split(|&b| b == 0)
                    .any(|entry| entry == b"FARHELM_PROCS_TEST_MARKER=sentinel-value");
                if marker_seen {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            !marker_seen,
            "macOS returned a platform binary's environment — the withholding this test \
             pins has been lifted; update the marker-scan residual docs accordingly"
        );
    }
}
