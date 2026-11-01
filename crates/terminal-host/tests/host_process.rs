//! Real-pty integration tests of what the host and its child look like as
//! processes (SPEC.md, "Start and readiness", "The child", "Security"): the
//! terminal settings, the child's signal state, environment and
//! descriptors, and the host's own command line, environment and
//! descriptors.
//!
//! Where Linux shows a process's state in `/proc`, macOS needs `ps`, `lsof`
//! or the child's own report instead; such tests come in a variant per
//! platform, and the macOS CI suite runs the macOS ones.

mod common;

use common::*;
use std::os::unix::process::CommandExt as _;
use std::time::Duration;
use terminal_host::protocol::{ChildExit, ClientMessage, HostMessage};

/// Run `argv` in a host and return everything it printed until it exited.
fn run_to_exit(argv: &[&str]) -> String {
    run_to_exit_with(argv, |_| {})
}

/// [`run_to_exit`] with a starter configured by `configure`.
fn run_to_exit_with(argv: &[&str], configure: impl FnOnce(&mut std::process::Command)) -> String {
    read_to_exit(&start_with(&spec(argv), configure))
}

/// Everything `host`'s child printed, read from the start until its exit
/// record: all of its output, however the reads split it (bounded, see
/// [`Client::output_to_exit`]).
fn read_to_exit(host: &Host) -> String {
    host.connect(false).output_to_exit().0
}

/// The pty's terminal settings are those tmux 3.7c gives a new pane:
/// erase is DEL and IUTF8 is on, everything else the kernel's defaults.
///
/// Why: programs moved from tmux panes to hosts must see the same
/// terminal, or line editing and UTF-8 input change under them (SPEC.md,
/// "The child"). Spec: `stty -g` in the host's child equals `stty -g` in a
/// pane of a fresh tmux 3.7c server started with no client; independently,
/// `stty -a` shows `erase = ^?` and `iutf8`. Without tmux 3.7c on PATH the
/// comparison is skipped (said on stderr) and only the independent checks
/// run.
#[test]
fn the_terminal_settings_match_a_tmux_pane() {
    let ours = run_to_exit(&["/bin/sh", "-c", "stty -g; stty -a"]);
    assert!(ours.contains("erase = ^?"), "{ours}");
    assert!(ours.contains("iutf8") && !ours.contains("-iutf8"), "{ours}");
    let ours_g = ours.lines().next().unwrap().trim().to_string();

    let version = std::process::Command::new("tmux").arg("-V").output();
    let Ok(version) = version else {
        eprintln!("SKIPPED tmux comparison: no tmux on PATH");
        return;
    };
    if String::from_utf8_lossy(&version.stdout).trim() != "tmux 3.7c" {
        eprintln!("SKIPPED tmux comparison: PATH's tmux is not 3.7c");
        return;
    }
    // A short private socket path: socket paths are limited to ~104 bytes.
    let dir = tempfile::Builder::new()
        .prefix("th")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = dir.path().join("s");
    let out = dir.path().join("g");
    /// Ends the private tmux server however the comparison goes, so that a
    /// failed wait cannot leave it running.
    struct KillServer<'a>(&'a std::path::Path);
    impl Drop for KillServer<'_> {
        fn drop(&mut self) {
            let _ = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.0)
                .arg("kill-server")
                .status();
        }
    }
    let server = KillServer(&socket);
    let status = std::process::Command::new("tmux")
        .arg("-S")
        .arg(&socket)
        .args([
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-x",
            "80",
            "-y",
            "24",
        ])
        .arg(format!("stty -g > {}", out.display()))
        .status()
        .unwrap();
    assert!(status.success());
    eventually("the tmux pane writes its settings", || {
        std::fs::read_to_string(&out).is_ok_and(|text| text.ends_with('\n'))
    });
    drop(server);
    let theirs = std::fs::read_to_string(&out).unwrap();
    assert_eq!(
        ours_g,
        theirs.trim(),
        "the host's pty settings differ from a tmux 3.7c pane's"
    );
}

/// A starter that ignores SIGQUIT and blocks `blocked`, as a `pre_exec` of
/// the starter's own process (the test process is left alone).
fn odd_signals(command: &mut std::process::Command, blocked: &'static [libc::c_int]) {
    // SAFETY: signal, sigemptyset, sigaddset and sigprocmask are
    // async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            libc::signal(libc::SIGQUIT, libc::SIG_IGN);
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            for &signal in blocked {
                libc::sigaddset(&mut set, signal);
            }
            if libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// A signal set field (`SigIgn:`, `SigBlk:`) of a `/proc/<pid>/status`
/// text, as its bits: signal `n` is bit `n - 1`.
#[cfg(target_os = "linux")]
fn signal_bits(status: &str, field: &str) -> u64 {
    let line = status
        .lines()
        .find(|line| line.starts_with(field))
        .unwrap_or_else(|| panic!("{field} in {status}"));
    let hex = line
        .split_whitespace()
        .nth(1)
        .expect("a value after the field");
    u64::from_str_radix(hex, 16).unwrap_or_else(|_| panic!("{line}"))
}

/// Check that a starter configured by [`odd_signals`] with `blocked`
/// really reaches its `exec` ignoring SIGQUIT and blocking `blocked`: a
/// process run straight from such a starter shows them in its own status.
///
/// The tests that rely on such a starter would otherwise pass without
/// testing anything if `pre_exec` stopped running after std's own reset of
/// the mask and of SIGPIPE.
#[cfg(target_os = "linux")]
fn assert_starter_premise(blocked: &'static [libc::c_int]) {
    let mut command = std::process::Command::new("/bin/cat");
    command.arg("/proc/self/status");
    odd_signals(&mut command, blocked);
    let output = command.output().expect("fixture: running cat");
    let status = String::from_utf8_lossy(&output.stdout);
    assert_ne!(
        signal_bits(&status, "SigIgn:") & (1 << (libc::SIGQUIT - 1)),
        0,
        "premise: the starter ignores SIGQUIT: {status}"
    );
    for &signal in blocked {
        assert_ne!(
            signal_bits(&status, "SigBlk:") & (1 << (signal - 1)),
            0,
            "premise: the starter blocks {signal}: {status}"
        );
    }
}

/// The child starts with no signal ignored and none blocked, whatever its
/// host and the host's starter had.
///
/// Why: ignored dispositions survive exec and a shell cannot undo them; a
/// host (itself a Rust program, which ignores SIGPIPE, and which ignores
/// SIGHUP and SIGINT itself) that passed its own on, or its starter's,
/// would leave Ctrl-C or broken pipes dead in everything the terminal runs.
/// Spec: a process run straight from a starter that ignores SIGQUIT and
/// blocks SIGUSR1 shows both in its `/proc/self/status`; a child run from
/// the host such a starter started shows SigIgn and SigBlk both zero. (The
/// blocked half checks the host's and the child's reset together: the host
/// clears its own mask, so the child's own clearing never has anything left
/// to clear.)
#[cfg(target_os = "linux")]
#[test]
fn the_child_starts_with_default_signals_and_an_empty_mask() {
    assert_starter_premise(&[libc::SIGUSR1]);
    let status = run_to_exit_with(&["/bin/cat", "/proc/self/status"], |command| {
        odd_signals(command, &[libc::SIGUSR1])
    });
    for field in ["SigIgn:", "SigBlk:"] {
        assert_eq!(signal_bits(&status, field), 0, "{field} in {status}");
    }
}

/// The same child setup on macOS: the child starts with no signal ignored
/// and none blocked.
///
/// Why: as for the Linux test above, which reads `/proc`; macOS has none,
/// and its child setup (a different signal count) needs its own check.
/// macOS's `ps` reports a process's blocked signals but has no keyword for
/// its ignored ones (`sigignore` is Linux's), so the child reports both
/// itself: Perl reads `%SIG` as `IGNORE` for a signal ignored when it
/// started, and its POSIX module gives the mask. That this report sees an
/// ignored and a blocked signal at all is checked first, outside any host.
/// SIGFPE is left out of the report: Perl ignores it itself as it starts
/// (perl.h's `PERL_FPU_INIT`, so its own arithmetic cannot kill it), and
/// then reports it ignored whatever it inherited, which is what the macOS
/// CI run showed (`ignored=[FPE]` from a host child). Spec: run straight
/// from a starter that ignores SIGQUIT and blocks SIGUSR1, the reporter
/// names both; run as the host's child from such a starter, it names
/// neither, nor any other signal but SIGFPE.
#[cfg(target_os = "macos")]
#[test]
fn the_child_starts_with_default_signals_and_an_empty_mask() {
    const REPORT: &str = r#"use POSIX; my $mask = POSIX::SigSet->new; sigprocmask(SIG_BLOCK, POSIX::SigSet->new, $mask) or die "sigprocmask: $!"; my @blocked = grep { $mask->ismember($_) } 1..31; my @ignored = grep { $_ ne "FPE" && ($SIG{$_} // "") eq "IGNORE" } keys %SIG; @ignored = sort @ignored; print "blocked=[@blocked] ignored=[@ignored]\n";"#;
    let mut premise = std::process::Command::new("/usr/bin/perl");
    premise.args(["-e", REPORT]);
    odd_signals(&mut premise, &[libc::SIGUSR1]);
    let output = premise.output().expect("fixture: running perl");
    let direct = String::from_utf8_lossy(&output.stdout).into_owned();
    // The test's own environment may ignore or block more (a CI runner's
    // shell, say), so the premise only needs these two among the rest.
    let listed = |field: &str| -> Vec<String> {
        direct
            .split_once(&format!("{field}=["))
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(list, _)| list.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    assert!(
        listed("ignored").contains(&"QUIT".to_owned())
            && listed("blocked").contains(&libc::SIGUSR1.to_string()),
        "premise: the report sees what its starter set: {direct:?}"
    );
    let text = run_to_exit_with(&["/usr/bin/perl", "-e", REPORT], |command| {
        odd_signals(command, &[libc::SIGUSR1])
    });
    assert!(text.contains("blocked=[] ignored=[]"), "{text:?}");
}

/// The host clears a signal mask its starter left, so SIGTERM still ends it
/// and its child's exit is still noticed with nothing else to wake it.
///
/// Why: a mask survives fork and exec. With SIGTERM blocked, a service
/// manager could not stop the host; with SIGCHLD blocked, a child that
/// exits after closing its terminal would stay an unreaped zombie, its exit
/// unreported, until some connection happened to wake the host (SPEC.md,
/// "Start and readiness"). Spec: with a starter that blocks SIGTERM and
/// SIGCHLD (on Linux, shown first in a process run straight from such a
/// starter) and a child that closes its terminal descriptors: on Linux the
/// host's SigBlk is zero; once the client is gone and the host idle, the
/// child killed from outside is reaped (its pid gone, not a zombie); and
/// SIGTERM then ends the host. (On macOS the pty ends only when the child
/// exits, which wakes the host anyway, so there only the SIGTERM half
/// distinguishes.)
#[test]
fn the_host_clears_its_starters_signal_mask() {
    #[cfg(target_os = "linux")]
    assert_starter_premise(&[libc::SIGTERM, libc::SIGCHLD]);
    let host = start_with(
        &spec(&[
            "/bin/sh",
            "-c",
            "exec </dev/null >/dev/null 2>&1; exec /bin/sleep 1000",
        ]),
        |command| odd_signals(command, &[libc::SIGTERM, libc::SIGCHLD]),
    );
    let info = host.connect(false).info();
    let _child = KillOnDrop(info.child_pid);
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{}/status", host.pid)).unwrap();
        assert_eq!(signal_bits(&status, "SigBlk:"), 0, "{status}");
    }
    // sleep-ok: lets the host see the client's close and the pty's end of output, so that nothing but SIGCHLD wakes it after the kill; a slower host only makes the test pass for the wrong reason, never fail.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        info.child_pid > 1,
        "implausible child pid {}",
        info.child_pid
    );
    // SAFETY: killing the host's child, which this test started.
    unsafe { libc::kill(info.child_pid as libc::pid_t, libc::SIGKILL) };
    eventually("the child to be reaped", || !exists(info.child_pid));
    // SAFETY: signalling the host this test started.
    unsafe { libc::kill(host.pid as libc::pid_t, libc::SIGTERM) };
    eventually("the host to end", || !running(host.pid));
}

/// The host's command line, as the system shows it to any process: its
/// arguments, in order.
fn command_line(pid: u32) -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap();
        cmdline
            .split(|&b| b == 0)
            .filter(|arg| !arg.is_empty())
            .map(|arg| String::from_utf8_lossy(arg).into_owned())
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        // `-ww`: never cut to a terminal's width. The test's paths have no
        // spaces, so splitting on whitespace gives the arguments back.
        let output = std::process::Command::new("/bin/ps")
            .args(["-ww", "-o", "command=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }
}

/// The child's environment is exactly the spec's, and the host's command
/// line carries none of it.
///
/// Why: a host lives for weeks with a world-readable command line, and its
/// child's environment can hold credentials; the child spec arrives on
/// stdin precisely so it never reaches the command line (SPEC.md, "Command
/// line and child spec"). Spec: `env` in the child prints exactly the
/// spec's entries, collected up to the child's exit; the host's command
/// line (`/proc/<pid>/cmdline` on Linux, `ps` on macOS) contains no spec
/// entry and is only the program, `--socket` and the path. (The host's own
/// environment is its starter's, and keeping secrets out of it is the
/// starter's business, SPEC.md "Security"; a check of it here could not
/// fail.)
#[test]
fn the_childs_environment_is_exact_and_stays_off_the_host() {
    let mut spawn = spec(&["/usr/bin/env"]);
    spawn.env = vec![
        b"SECRET_FOR_THE_CHILD_ONLY=s3cret".to_vec(),
        b"PATH=/usr/bin:/bin".to_vec(),
    ];
    let host = start(&spawn);
    let args = command_line(host.pid);
    assert!(
        !args.iter().any(|arg| arg.contains("s3cret")),
        "the secret reached the host's command line: {args:?}"
    );
    assert_eq!(args.len(), 3, "{args:?}");
    assert_eq!(args[1], "--socket");
    assert_eq!(std::path::Path::new(&args[2]), host.socket);
    let text = read_to_exit(&host);
    let mut lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .collect();
    lines.sort();
    assert_eq!(
        lines,
        vec!["PATH=/usr/bin:/bin", "SECRET_FOR_THE_CHILD_ONLY=s3cret"]
    );
}

/// Every descriptor process `pid` holds, with what it refers to.
///
/// One the process closes while it is listed is left out: a process that is
/// starting (the starter below, at its runtime's start) opens and closes
/// files of its own, and those are not what any test looks for.
fn descriptors(pid: u32) -> Vec<(i32, String)> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.unwrap();
                let fd = entry.file_name().to_string_lossy().parse().unwrap();
                let target = std::fs::read_link(entry.path()).ok()?;
                Some((fd, target.display().to_string()))
            })
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        lsof(pid, "0-4096")
            .into_iter()
            .map(|(fd, name)| (fd.parse().unwrap(), name))
            .collect()
    }
}

/// The working directory of process `pid`.
fn working_directory(pid: u32) -> std::path::PathBuf {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/cwd")).unwrap()
    }
    #[cfg(target_os = "macos")]
    {
        let entries = lsof(pid, "cwd");
        assert_eq!(entries.len(), 1, "{entries:?}");
        std::path::PathBuf::from(&entries[0].1)
    }
}

/// `lsof`'s files of process `pid` for the descriptor set `set` (macOS has
/// no `/proc`), as (descriptor, name) pairs. In `-F` output each file is an
/// `f` line (its descriptor) followed by an `n` line (its name).
#[cfg(target_os = "macos")]
fn lsof(pid: u32, set: &str) -> Vec<(String, String)> {
    let output = std::process::Command::new("/usr/sbin/lsof")
        .args(["-w", "-a", "-p", &pid.to_string(), "-d", set, "-F", "fn"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut fd = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix('f') {
            fd = Some(value.to_owned());
        } else if let Some(name) = line.strip_prefix('n') {
            entries.push((
                fd.take().expect("an n line follows its f line"),
                name.to_owned(),
            ));
        }
    }
    assert!(!entries.is_empty(), "lsof listed nothing: {text:?}");
    entries
}

/// A Perl one-liner that prints the descriptors it holds, found with
/// `fstat` on each number rather than by listing a directory: `ls /dev/fd`
/// opens the directory it lists, and macOS's `ls` (which walks with `fts`)
/// keeps `.` open as well, so a listing shows descriptors of its own.
const DESCRIPTOR_REPORT: &str =
    r#"use POSIX; print join(" ", grep { my @s = POSIX::fstat($_); @s } 0..1023), "\n";"#;

/// Neither the host nor its child keeps a descriptor the host's starter
/// left open, the host's standard descriptors are each on `/dev/null`, and
/// its working directory is `/`.
///
/// Why: a host holding its starter's sockets, locks or pipes for weeks
/// would keep them alive long after the starter meant to release them, a
/// standard descriptor left free would be taken by the next socket or pipe
/// (where a stray write to stderr would then land), and a host or child
/// keeping a directory busy pins it. macOS closes what was inherited by
/// listing `/dev/fd`, which is a different file system there than Linux's
/// `/proc`, so both need the check. The child reports its own descriptors
/// with [`DESCRIPTOR_REPORT`], which opens none; run straight from a starter
/// that holds only 0 to 2, it gives the baseline, which is whatever the
/// reporter itself holds (0, 1 and 2, unless a platform's `perl` keeps one
/// of its own). Spec: with an extra descriptor (7, or 8 if the test's own
/// file got 7) seen open in the starter before it reads its spec, the
/// host's descriptors 0, 1 and 2 each refer to /dev/null, and the rest are
/// only its socket, the pty, the signal pipe and connections; the child's
/// are exactly the baseline, which starts 0, 1, 2; the host's cwd is `/`.
#[test]
fn descriptors_are_not_inherited_and_the_host_does_not_pin_a_directory() {
    use std::io::Write as _;
    let report = |output: &[u8]| -> Vec<String> {
        String::from_utf8_lossy(output)
            .split_whitespace()
            .map(String::from)
            .collect()
    };
    let mut direct = std::process::Command::new("/usr/bin/perl");
    direct.args(["-e", DESCRIPTOR_REPORT]);
    // SAFETY: close is async-signal-safe. Whatever the test process holds
    // without close-on-exec (a harness's jobserver pipe, say) is not the
    // reporter's own.
    unsafe {
        direct.pre_exec(|| {
            for fd in 3..1024 {
                libc::close(fd);
            }
            Ok(())
        });
    }
    let baseline = report(&direct.output().expect("fixture: running perl").stdout);
    assert!(
        baseline.starts_with(&["0".into(), "1".into(), "2".into()]),
        "premise: the reporter sees its standard descriptors: {baseline:?}"
    );

    let dir = tempfile::tempdir().unwrap();
    let leak = std::fs::File::create(dir.path().join("leak")).unwrap();
    let leak_fd = std::os::fd::AsRawFd::as_raw_fd(&leak);
    // A dup2 onto the file's own number would leave it close-on-exec.
    let target = if leak_fd == 7 { 8 } else { 7 };
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_terminal-host"));
    command.arg("--socket").arg(dir.path().join("h"));
    // SAFETY: dup2 is async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(leak_fd, target) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.stdin(std::process::Stdio::piped()).spawn().unwrap();
    // The starter waits for its spec on stdin, so it can be looked at
    // first: the descriptor must really have reached it.
    let starter: Vec<(i32, String)> = descriptors(child.id());
    assert!(
        starter
            .iter()
            .any(|(fd, name)| *fd == target && name.ends_with("/leak")),
        "premise: the starter holds the extra descriptor {target}: {starter:?}"
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&terminal_host::protocol::encode_spawn(&spec(&[
            "/usr/bin/perl",
            "-e",
            DESCRIPTOR_REPORT,
        ])))
        .unwrap();
    assert!(child.wait().unwrap().success());
    let socket = dir.path().join("h");
    let host = Host::adopt(dir, socket);
    let fds = descriptors(host.pid);
    for fd in 0..3 {
        assert!(
            fds.contains(&(fd, "/dev/null".to_owned())),
            "the host's descriptor {fd}: {fds:?}"
        );
    }
    assert!(
        !fds.iter().any(|(_, target)| target.ends_with("/leak")),
        "the host kept an inherited descriptor: {fds:?}"
    );
    assert!(fds.len() <= 9, "{fds:?}");
    assert_eq!(working_directory(host.pid), std::path::Path::new("/"));
    let listed = report(read_to_exit(&host).as_bytes());
    assert_eq!(listed, baseline, "the child's descriptors");
}

/// A short input (Shift+Enter's ESC CR) sent to a program waiting for it
/// on a terminal with room reaches it in one read.
///
/// Why: programs tell Shift+Enter from Escape-then-Enter by whether the two
/// bytes arrive together, so the ordinary case must keep them together end
/// to end. The host's own promise is narrower (SPEC.md, "Input": one
/// `write` offers a small `Input` whole, which the in-process
/// `a_small_input_is_offered_in_one_write` pins; a nearly full pty may
/// still take part of it, and the program's reads are its own); this is the
/// real pty and program around that promise, in the case it is for. Spec: a
/// raw-mode `dd` already waiting to read one block receives both bytes of a
/// single two-byte Input in that block.
#[test]
fn a_short_input_reaches_a_waiting_program_in_one_read() {
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "stty raw -echo; echo ready; dd bs=64 count=1 2>/dev/null | od -An -c; sleep 1",
    ]));
    let mut client = host.connect(false);
    client.read_all();
    client.output_until("ready");
    // sleep-ok: let dd reach its read before the input; the property needs it already waiting.
    std::thread::sleep(Duration::from_millis(200));
    client.send(&ClientMessage::Input(b"\x1b\r".to_vec()));
    let (text, _) = client.output_until("\\r");
    assert!(
        text.contains("033  \\r") || text.contains("033 \\r"),
        "{text:?}"
    );
}

/// A child killed by a signal is reported with that signal.
///
/// Why: a client tells a crash from a clean exit by this. Spec: a child
/// that sends itself SIGTERM yields an exit record with signal 15.
#[test]
fn a_signal_exit_is_reported_as_such() {
    let host = start(&spec(&["/bin/sh", "-c", "kill -TERM $$"]));
    let mut client = host.connect(false);
    client.read_all();
    let exit = client.until(|m| matches!(m, HostMessage::Exited { .. }));
    assert!(
        matches!(
            exit,
            HostMessage::Exited {
                exit: ChildExit::Signal(15),
                ..
            }
        ),
        "{exit:?}"
    );
}

/// Input after the terminal has no reader is acknowledged as discarded.
///
/// Why: a client must not wait forever for an acknowledgement that cannot
/// come, nor take input that went nowhere for input the program got
/// (SPEC.md, "Input"). Spec: after the child exited and nothing holds the
/// slave, an Input of 17 bytes is acknowledged with 0 written and 17
/// discarded.
#[test]
fn input_after_the_terminal_closed_is_acknowledged() {
    let host = start(&spec(&["/bin/sh", "-c", "exit 0"]));
    let mut client = host.connect(false);
    client.read_all();
    client.until(|m| matches!(m, HostMessage::Exited { .. }));
    client.send(&ClientMessage::Input(b"nobody reads this".to_vec()));
    assert_eq!(
        client.until(|m| matches!(m, HostMessage::InputAck { .. })),
        HostMessage::InputAck {
            written: 0,
            discarded: 17
        }
    );
}

/// Connections that never say Hello do not crowd out a client, and a
/// client that disconnects in the middle of a frame does not hurt the host.
///
/// Why: the host must stay reachable whatever stray or broken connections
/// do (SPEC.md, "Connections"). Spec: with more silent connections than
/// the pending limit, and one that sent half a frame and closed, a new
/// client still connects and works.
#[test]
fn silent_and_broken_connections_do_not_block_a_client() {
    use std::io::Write as _;
    let host = start(&spec(&["/bin/cat"]));
    let silent: Vec<std::os::unix::net::UnixStream> = (0..12)
        .map(|_| std::os::unix::net::UnixStream::connect(&host.socket).unwrap())
        .collect();
    {
        let mut broken = std::os::unix::net::UnixStream::connect(&host.socket).unwrap();
        let hello = terminal_host::protocol::encode_client(&ClientMessage::Hello {
            version: 1,
            take_over: false,
        });
        broken.write_all(&hello[..3]).unwrap();
    }
    let mut client = host.connect(false);
    assert!(client.info().child_exit.is_none());
    drop(silent);
}

/// A bare program name is looked up in the child's own PATH, and a name
/// found nowhere refuses the start.
///
/// Why: the child's PATH, not the host's, decides what runs (SPEC.md,
/// "Command line and child spec"); a lookup in the host's environment would
/// run a different program than the one the caller's environment names.
/// Spec: `sh` with a PATH that has an empty entry and `/bin` runs; a name in
/// no PATH entry makes the start exit 1 naming it; with no PATH at all the
/// POSIX default `/usr/bin:/bin` is searched.
#[test]
fn bare_names_are_found_in_the_childs_path() {
    let mut spawn = spec(&["sh", "-c", "echo found-it"]);
    spawn.env = vec![b"PATH=:/nonexistent:/bin".to_vec()];
    assert!(run_spawn_to_exit(&spawn).contains("found-it"));
    let mut spawn = spec(&["sh", "-c", "echo default-path"]);
    spawn.env = vec![];
    assert!(run_spawn_to_exit(&spawn).contains("default-path"));
    let (_, started) = start_raw(
        &terminal_host::protocol::encode_spawn(&spec(&["no-such-program-anywhere"])),
        &["--socket", "{dir}/h"],
    );
    assert_eq!(started.status, 1);
    assert!(
        started.stderr.contains("no-such-program-anywhere"),
        "{}",
        started.stderr
    );
}

/// A child spec beyond its size limit is refused, and one of exactly the
/// limit is not refused for its size.
///
/// Why: the spec comes from a pipe; the host must not read without bound,
/// but the limit is part of the contract, so a spec at it must be read
/// whole. Spec: SPEC.md, "Command line and child spec": the frame's length
/// field is at most `MAX_SPEC`, so more than `MAX_SPEC` + 4 bytes on stdin
/// exit 1 saying the spec is too long; exactly `MAX_SPEC` + 4 bytes (here
/// zeros, which are not a valid spec) are decoded and refused as malformed
/// instead.
#[test]
fn an_oversized_child_spec_is_refused() {
    let limit = terminal_host::protocol::MAX_SPEC + 4;
    let (_, started) = start_raw(&vec![0u8; limit + 1], &["--socket", "{dir}/h"]);
    assert_eq!(started.status, 1);
    assert!(started.stderr.contains("too long"), "{}", started.stderr);
    let (_, started) = start_raw(&vec![0u8; limit], &["--socket", "{dir}/h"]);
    assert_eq!(started.status, 1);
    assert!(
        started.stderr.contains("malformed") && !started.stderr.contains("too long"),
        "{}",
        started.stderr
    );
}

/// The command line is exactly `--socket <absolute path>`; anything else
/// is a usage error.
///
/// Why: the host's command line is private to one build, but a malformed
/// one must fail loudly at once rather than start a host on a socket nobody
/// will find. A relative path is one of those: the host changes to `/`
/// before it binds, so it would be checked in one directory and bound in
/// another (SPEC.md, "Command line and child spec"). Spec: no arguments, an
/// empty path, a relative path, another flag, and an extra argument each
/// exit 2 with the usage line, before reading the spec; no socket appears.
#[test]
fn the_command_line_takes_only_a_socket_path() {
    let valid = terminal_host::protocol::encode_spawn(&spec(&["/bin/sh", "-c", "exit 0"]));
    for args in [
        &[][..],
        &["--socket", ""][..],
        &["--socket", "relative-socket-of-a-test"][..],
        &["--sock", "{dir}/h"][..],
        &["--socket", "{dir}/h", "extra"][..],
    ] {
        let (dir, started) = start_raw(&valid, args);
        assert_eq!(started.status, 2, "{args:?}: {}", started.stderr);
        assert!(
            started.stderr.contains("usage"),
            "{args:?}: {}",
            started.stderr
        );
        assert!(!dir.path().join("h").exists(), "{args:?} started a host");
    }
    for root in [std::path::Path::new("/"), std::path::Path::new(".")] {
        assert!(!root.join("relative-socket-of-a-test").exists());
    }
}

/// A start that fails after the socket is bound removes the socket again.
///
/// Why: the first stage reports the failure, and a retry on the same path
/// would otherwise be refused because the path exists, for a host that
/// never ran (SPEC.md, "Start and readiness"). Without fault injection,
/// a descriptor limit in the starter is what makes a later step fail.
/// Spec (Linux, where the descriptors the start uses are numbered as
/// counted here): with at most 7 descriptors the pty cannot be opened, and
/// with at most 9 the pipe that reports the child's start cannot be made;
/// either start exits 1, naming that step (the child's error pipe in words
/// of its own, not the first stage's "creating a pipe", which fails before
/// any socket exists), and leaves no socket.
#[cfg(target_os = "linux")]
#[test]
fn a_failure_after_binding_leaves_no_socket() {
    // The first stage needs 0 to 5 for its readiness pipe (renumbered
    // through a spare descriptor). The second stage holds 0 to 2 and the
    // readiness pipe's writer at 3; the signal pipe takes 4 and 5 before
    // the listener takes 6, then the pty needs 7 and 8 and the child's
    // error pipe 9 and 10. (The signal pipe comes before the socket, so its
    // failure leaves no socket to remove.)
    for (limit, step) in [(7, "opening a pty"), (9, "creating the child's error pipe")] {
        let (dir, started) = start_raw_with(
            &terminal_host::protocol::encode_spawn(&spec(&["/bin/sleep", "60"])),
            &["--socket", "{dir}/h"],
            |command| {
                // SAFETY: close and setrlimit are async-signal-safe; they act
                // only on the starter (and the host it forks), not the test.
                unsafe {
                    command.pre_exec(move || {
                        // Whatever else the test process holds without
                        // close-on-exec (a harness's jobserver pipe, say)
                        // would shift the numbering counted above.
                        for fd in 3..1024 {
                            libc::close(fd);
                        }
                        let limit = libc::rlimit {
                            rlim_cur: limit,
                            rlim_max: limit,
                        };
                        if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
            },
        );
        assert_eq!(started.status, 1, "limit {limit}: {}", started.stderr);
        assert!(
            started.stderr.contains(step),
            "premise: limit {limit} fails at the {step}: {}",
            started.stderr
        );
        assert!(
            !dir.path().join("h").exists(),
            "limit {limit} left the socket"
        );
    }
}

/// The PATH lookup takes the first entry that is a regular file the host's
/// user may execute, skipping a same-named directory, a file without
/// execute permission, and one whose execute permission is someone else's.
///
/// Why: a lookup that accepted the first match of any kind would try to run
/// a data file or a directory and fail the start, where a shell would have
/// found the program further along PATH (SPEC.md, "Command line and child
/// spec"). An execute bit for the group or others alone does not let the
/// file's owner run it: `execve` would fail with EACCES. Spec: with PATH = a
/// directory holding a 0755 directory named `prog`, then one holding a 0644
/// file `prog`, then one holding a 0641 file `prog` of the test's own user,
/// then one holding an executable `prog`, the executable one runs. (Root
/// may execute any file with an execute bit, so as root the 0641 entry is
/// left out of PATH, and said so on stderr.)
#[test]
fn the_path_lookup_skips_what_cannot_be_run() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = tempfile::tempdir().expect("fixture: PATH directories");
    let dirs: Vec<_> = ["a", "b", "c", "d"]
        .iter()
        .map(|name| root.path().join(name))
        .collect();
    for dir in &dirs {
        std::fs::create_dir(dir).unwrap();
    }
    std::fs::create_dir(dirs[0].join("prog")).unwrap();
    std::fs::set_permissions(dirs[0].join("prog"), std::fs::Permissions::from_mode(0o755)).unwrap();
    for (dir, mode) in [(&dirs[1], 0o644), (&dirs[2], 0o641), (&dirs[3], 0o755)] {
        let name = dir.file_name().unwrap().to_string_lossy();
        std::fs::write(dir.join("prog"), format!("#!/bin/sh\necho from-{name}\n")).unwrap();
        std::fs::set_permissions(dir.join("prog"), std::fs::Permissions::from_mode(mode)).unwrap();
    }
    // SAFETY: geteuid has no preconditions.
    let root_user = unsafe { libc::geteuid() } == 0;
    if root_user {
        eprintln!("SKIPPED the 0641 entry: running as root, who may execute it");
    }
    let path = dirs
        .iter()
        .enumerate()
        .filter(|&(index, _)| !(root_user && index == 2))
        .map(|(_, dir)| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(":");
    let mut spawn = spec(&["prog"]);
    spawn.env = vec![format!("PATH={path}").into_bytes()];
    let text = run_spawn_to_exit(&spawn);
    assert!(text.contains("from-d"), "{text:?}");
}

/// Start a host for `spawn` and return everything its child printed.
fn run_spawn_to_exit(spawn: &terminal_host::protocol::Spawn) -> String {
    read_to_exit(&start(spawn))
}
