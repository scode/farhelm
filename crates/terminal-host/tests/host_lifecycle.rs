//! Real-pty integration tests of a running host (SPEC.md, "Lifecycle",
//! "Protocol", "Input"): the crate's own binary on a real pty with a real
//! child, driven through its socket.
//!
//! These are the tests the "almost never changes" promise rests on: each
//! pins one behavior a client of any later release relies on.

mod common;

use common::*;
use std::time::Duration;
use terminal_host::protocol::{
    ChildExit, ClientMessage, ErrorCode, HostMessage, MAX_INPUT, Position, Size, encode_client,
    encode_spawn,
};

/// Output written before any client connects is kept and read from the
/// start; the child's exit arrives after its output, with its status.
///
/// Why: a client may connect long after the child started, or after it
/// exited; neither may lose output or the exit. Spec: a child that prints
/// and exits 3 before anyone connects is read back in full from (0, 0),
/// followed by an exit record with code 3; Info reports it exited.
#[test]
fn output_before_a_client_connects_is_kept_and_the_exit_follows_it() {
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "printf 'early output\\n'; exit 3",
    ]));
    // Let the child run and exit with nobody connected.
    eventually("the child exits", || {
        let mut client = host.connect(true);
        client.info().child_exit.is_some()
    });
    let mut client = host.connect(true);
    client.read_all();
    let (text, _) = client.output_until("early output");
    assert!(text.contains("early output"));
    let exit = client.until(|m| matches!(m, HostMessage::Exited { .. }));
    assert!(
        matches!(
            exit,
            HostMessage::Exited {
                exit: ChildExit::Code(3),
                ..
            }
        ),
        "{exit:?}"
    );
    assert_eq!(client.info().child_exit, Some(ChildExit::Code(3)));
}

/// Input reaches the program, is acknowledged once written, and the
/// program's answer comes back in the stream.
///
/// Why: typing is the whole point of a terminal; the acknowledgement is what
/// lets a client know not to resend. Spec: an `Input` to a `cat` is
/// acknowledged with the running total of bytes written and none
/// discarded, and `cat`'s echo arrives as output.
#[test]
fn input_is_written_acknowledged_and_echoed() {
    let host = start(&spec(&["/bin/cat"]));
    let mut client = host.connect(false);
    client.read_all();
    client.send(&ClientMessage::Input(b"hello cat\r".to_vec()));
    let ack = client.until(|m| matches!(m, HostMessage::InputAck { .. }));
    assert_eq!(
        ack,
        HostMessage::InputAck {
            written: 10,
            discarded: 0
        }
    );
    client.output_until("hello cat");
    client.send(&ClientMessage::Input(b"x".to_vec()));
    let ack = client.until(|m| matches!(m, HostMessage::InputAck { .. }));
    assert_eq!(
        ack,
        HostMessage::InputAck {
            written: 11,
            discarded: 0
        },
        "acks count up per connection"
    );
}

/// A resize changes the pty's size, which the program sees, and is
/// journaled in the stream.
///
/// Why: the terminal size is the program's to read; a client relies on the
/// resize record to keep its own view in step. Spec: after Resize to
/// 120x40, `stty size` in the child prints "40 120", and a Resized record
/// with index 0 and that size is in the stream.
#[test]
fn a_resize_reaches_the_program_and_the_stream() {
    let host = start(&spec(&["/bin/sh"]));
    let mut client = host.connect(false);
    client.read_all();
    client.send(&ClientMessage::Resize(Size {
        cols: 120,
        rows: 40,
    }));
    client.send(&ClientMessage::Input(b"stty size\r".to_vec()));
    let (_, others) = client.output_until("40 120");
    assert!(
        others.iter().any(|m| matches!(
            m,
            HostMessage::Resized {
                index: 0,
                size: Size {
                    cols: 120,
                    rows: 40
                },
                ..
            }
        )),
        "{others:?}"
    );
}

/// A second client is told the host is busy unless it takes over, and
/// taking over displaces the first.
///
/// Why: exactly one client at a time; a restarted client must be able to
/// take over from a stale connection, and anything else must be refused
/// distinctly so it can tell busy from gone. Spec: a second Hello without
/// take_over gets Busy and is closed while the first keeps working; with
/// take_over the first's stream ends, with nothing more sent to it (version
/// 1 has no displacement notice), and the newcomer is the client.
#[test]
fn a_second_client_is_busy_unless_it_takes_over() {
    let host = start(&spec(&["/bin/cat"]));
    let mut first = host.connect(false);
    let mut second = Client::raw(&host.socket);
    second.send(&ClientMessage::Hello {
        version: 1,
        take_over: false,
    });
    assert_eq!(second.recv(), HostMessage::Busy);
    assert!(second.closed(), "a busy reply closes the connection");
    assert!(first.info().host_pid > 0, "the first client still works");
    let mut third = host.connect(true);
    assert_eq!(first.try_recv(WAIT), None, "the first client's stream ends");
    assert!(third.info().child_exit.is_none());
}

/// Shutdown says goodbye, ends the host, removes its socket and hangs up
/// the child.
///
/// Why: Shutdown is how a client deletes a terminal; nothing may be left
/// behind. Spec: after Shutdown the client gets Goodbye and the connection
/// closes; the host process and its socket are gone; a shell child, which
/// exits on hangup, is gone too.
#[test]
fn shutdown_ends_the_host_and_hangs_up_the_child() {
    let host = start(&spec(&["/bin/sh"]));
    let mut client = host.connect(false);
    let info = client.info();
    client.send(&ClientMessage::Shutdown);
    assert_eq!(
        client.until(|m| matches!(m, HostMessage::Goodbye)),
        HostMessage::Goodbye
    );
    assert!(client.closed());
    eventually("the host exits", || !running(info.host_pid));
    assert!(!host.socket.exists(), "the socket is removed");
    eventually("the child is hung up", || !running(info.child_pid));
}

/// SIGTERM ends the host at once, like Shutdown.
///
/// Why: a service manager stopping everything sends SIGTERM; the host must
/// end and clean up rather than linger. Spec: after SIGTERM to the host its
/// process and socket are gone and the child is hung up.
#[test]
fn sigterm_ends_the_host() {
    let host = start(&spec(&["/bin/sh"]));
    let info = host.connect(false).info();
    // A pid of 0 or 1 would signal this test's own process group or init:
    // never act on one, whatever a broken host reports.
    assert!(info.host_pid > 1, "implausible host pid {}", info.host_pid);
    // SAFETY: signalling the host this test started.
    unsafe { libc::kill(info.host_pid as libc::pid_t, libc::SIGTERM) };
    eventually("the host exits", || !running(info.host_pid));
    assert!(!host.socket.exists());
    eventually("the child is hung up", || !running(info.child_pid));
}

/// The host keeps running after its child exits, until told to stop.
///
/// Why: a client may not be connected when the child exits and must still
/// learn how it ended later. Spec: a second after the child exits, the host
/// still answers Info with the exit.
#[test]
fn the_host_outlives_its_child() {
    let host = start(&spec(&["/bin/sh", "-c", "exit 0"]));
    eventually("the child exits", || {
        host.connect(true).info().child_exit.is_some()
    });
    // sleep-ok: the property is that nothing ends the host after its child's exit within a while.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        host.connect(true).info().child_exit,
        Some(ChildExit::Code(0))
    );
}

/// The child's exit is reported while a descendant still holds the pty
/// open, and on Linux what the descendant prints after it still arrives.
///
/// Why: child exit and end of output are different events (SPEC.md, "Child
/// exit and end of output"); a host that waited for end of output would
/// report the exit only when the last descendant let go, which can be
/// never. The helper ignores the hangup on purpose, so nothing but the test
/// ends it; it records its pid in a file before anything can fail, and the
/// guard on that file is armed before the host starts. Spec: a child starts
/// a helper that ignores SIGHUP, waits until the helper reports its pid (so
/// the helper runs, with the pty slave as its stdout), prints that pid and
/// exits 4. The exit record with code 4
/// arrives while the helper still runs (on Linux, still with the slave as
/// its stdout); on Linux, a line the helper prints when told to after that
/// arrives after the exit record. (On macOS the kernel revokes the terminal
/// from the rest of the session when its leader exits, so the helper can no
/// longer write to it.)
#[test]
fn the_exit_is_reported_while_a_descendant_holds_the_pty() {
    let files = tempfile::tempdir().expect("fixture: a directory for the helper");
    let ready = files.path().join("ready");
    let go = files.path().join("go");
    let pid_file = files.path().join("helper.pid");
    let _helper = KillByPidFile(pid_file.clone());
    make_fifo(&ready);
    make_fifo(&go);
    let helper = files.path().join("helper.sh");
    std::fs::write(
        &helper,
        format!(
            "trap '' HUP\necho $$ > '{}'\necho $$ > '{}'\nread x < '{}'\necho helper-after-exit\nexec /bin/sleep 1000\n",
            pid_file.display(),
            ready.display(),
            go.display()
        ),
    )
    .unwrap();
    let parent = format!(
        "/bin/sh '{}' & read pid < '{}'; echo helper=$pid; exit 4",
        helper.display(),
        ready.display()
    );
    let host = start(&spec(&["/bin/sh", "-c", &parent]));
    let mut client = host.connect(false);
    let (text, exit) = client.output_to_exit();
    let pid: u32 = text
        .split("helper=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("no helper pid in {text:?}"));
    assert_eq!(exit, ChildExit::Code(4));
    assert!(running(pid), "the helper still runs at the exit record");
    #[cfg(target_os = "linux")]
    {
        let stdout = std::fs::read_link(format!("/proc/{pid}/fd/1")).unwrap();
        assert!(
            stdout.starts_with("/dev/pts/"),
            "the helper's stdout is {stdout:?}"
        );
        release(&go);
        client.output_until("helper-after-exit");
    }
}

/// Collect messages until `predicate` matches one, failing if an
/// acknowledgement of `total` input bytes comes first (bounded as
/// [`Client::until`] is).
fn until_before_ack(
    client: &mut Client,
    total: u64,
    mut predicate: impl FnMut(&HostMessage) -> bool,
) -> HostMessage {
    client.until(|message| {
        if let HostMessage::InputAck { written, .. } = message {
            assert!(*written < total, "all the input was written after all");
        }
        predicate(message)
    })
}

/// A program that does not read its terminal holds back neither `Info`,
/// `Snapshot`, `Resize` nor `Shutdown` on the connection whose input it
/// did not take.
///
/// Why: the host must stay controllable however its program behaves
/// (SPEC.md, "Input"), on the client's own connection and not only through
/// a takeover; a host that queued control messages behind unwritten input
/// would hang the client's next snapshot, resize or teardown. Spec: with a
/// raw-mode program that never reads and 512 KiB of input sent (more than
/// the pty takes, less than `MAX_PENDING_INPUT`), an `Info` is answered, a
/// `Snapshot` at its end is accepted, a `Resize` appears in the stream and
/// the next `Info` reports it, and `Shutdown` gets `Goodbye` and ends the
/// host; no acknowledgement of the full 512 KiB arrives before any of them,
/// so the input stayed unwritten throughout.
#[test]
fn stuck_input_does_not_block_control_messages() {
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "stty raw -echo; echo ready; exec /bin/sleep 1000",
    ]));
    let mut client = host.connect(false);
    client.read_all();
    client.output_until("ready");
    let total = 8 * MAX_INPUT as u64;
    for _ in 0..8 {
        client.send(&ClientMessage::Input(vec![b'x'; MAX_INPUT]));
    }
    client.send(&ClientMessage::Info);
    let HostMessage::InfoReply(info) = until_before_ack(&mut client, total, |m| {
        matches!(m, HostMessage::InfoReply(_))
    }) else {
        unreachable!()
    };
    let position = Position {
        offset: info.end,
        resizes: info.resizes,
    };
    client.send(&ClientMessage::Snapshot {
        position,
        blob: b"state".to_vec(),
    });
    assert_eq!(
        until_before_ack(&mut client, total, |m| matches!(
            m,
            HostMessage::SnapshotAccepted(_) | HostMessage::SnapshotRejected { .. }
        )),
        HostMessage::SnapshotAccepted(position)
    );
    let size = Size {
        cols: 100,
        rows: 30,
    };
    client.send(&ClientMessage::Resize(size));
    until_before_ack(
        &mut client,
        total,
        |m| matches!(m, HostMessage::Resized { size: s, .. } if *s == size),
    );
    client.send(&ClientMessage::Info);
    let HostMessage::InfoReply(info) = until_before_ack(&mut client, total, |m| {
        matches!(m, HostMessage::InfoReply(_))
    }) else {
        unreachable!()
    };
    assert_eq!(info.size, size);
    client.send(&ClientMessage::Shutdown);
    until_before_ack(&mut client, total, |m| matches!(m, HostMessage::Goodbye));
    eventually("the host exits", || !running(host.pid));
}

/// A takeover frees a terminal whose client is stuck at the input
/// backstop, and the new client controls it at once.
///
/// Why: takeover exists for a stuck old connection (SPEC.md, "Connections
/// and version negotiation"), and the frames the backstop holds for the
/// old client must not hold anything back for the new one. Spec: with a
/// raw-mode program that never reads and 2 MiB of input sent (beyond the
/// pty and `MAX_PENDING_INPUT`, so later `Input`s wait in the host), a new
/// connection taking over is welcomed; on it a `Resize` takes effect (Info
/// reports it) and `Shutdown` gets `Goodbye`; the old connection ends.
#[test]
fn a_takeover_frees_a_client_stuck_at_the_backstop() {
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "stty raw -echo; echo ready; exec /bin/sleep 1000",
    ]));
    let mut client = host.connect(false);
    client.read_all();
    client.output_until("ready");
    let frames: Vec<u8> = (0..32)
        .flat_map(|_| encode_client(&ClientMessage::Input(vec![b'x'; MAX_INPUT])))
        .collect();
    use std::io::Write as _;
    client.stream.write_all(&frames).unwrap();
    let mut other = host.connect(true);
    assert!(client.closed(), "the old connection ends");
    let size = Size {
        cols: 100,
        rows: 30,
    };
    other.send(&ClientMessage::Resize(size));
    assert_eq!(other.info().size, size);
    other.send(&ClientMessage::Shutdown);
    other.until(|m| matches!(m, HostMessage::Goodbye));
}

/// A malformed frame gets an error and the connection is closed, and the
/// host goes on serving.
///
/// Why: a bad peer must not take the host or its terminal down. Spec: a
/// frame of an unknown kind after Welcome yields Error (malformed) and end
/// of stream; a new client then connects normally.
#[test]
fn a_malformed_frame_closes_only_that_connection() {
    let host = start(&spec(&["/bin/cat"]));
    let mut client = host.connect(false);
    use std::io::Write as _;
    client.stream.write_all(&[0, 0, 0, 1, 0x7e]).unwrap();
    let error = client.recv();
    assert!(
        matches!(
            error,
            HostMessage::Error {
                code: ErrorCode::Malformed,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(client.closed());
    assert!(host.connect(false).info().child_exit.is_none());
}

/// An accepted snapshot is what a later client starts from, followed only
/// by the output after it.
///
/// Why: this is the recovery path: a restarted client rebuilds from the
/// snapshot plus the tail. Spec: after output "one", a snapshot at the end,
/// then output "two", a fresh read gets the snapshot record and then only
/// "two".
#[test]
fn a_new_client_starts_from_the_accepted_snapshot() {
    // Echo off, so each line appears once and "one" cannot trail the
    // snapshot as the pty's echo of what was typed.
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "stty -echo; echo ready; exec cat",
    ]));
    let mut client = host.connect(false);
    client.read_all();
    client.output_until("ready");
    client.send(&ClientMessage::Input(b"one\r".to_vec()));
    client.output_until("one");
    let end = client.info().end;
    client.send(&ClientMessage::Snapshot {
        position: Position {
            offset: end,
            resizes: 0,
        },
        blob: b"state after one".to_vec(),
    });
    client.until(|m| matches!(m, HostMessage::SnapshotAccepted(_)));
    client.send(&ClientMessage::Input(b"two\r".to_vec()));
    client.output_until("two");
    let mut fresh = host.connect(true);
    fresh.read_all();
    let first = fresh.recv();
    assert_eq!(
        first,
        HostMessage::SnapshotRecord {
            position: Position {
                offset: end,
                resizes: 0
            },
            blob: b"state after one".to_vec()
        }
    );
    let (text, _) = fresh.output_until("two");
    assert!(
        !text.contains("one"),
        "only what followed the snapshot: {text:?}"
    );
}

/// Output beyond the journal cap trims the oldest output and leaves a gap.
///
/// Why: memory is bounded however much a program prints; the client must
/// be told what it lost. Spec: with a 64 KiB cap, after a program prints
/// well over that, Info reports a gap and a retained start past 0, and a
/// fresh read begins with a Gap record.
#[test]
fn output_beyond_the_cap_leaves_a_gap() {
    let mut spawn = spec(&[
        "/bin/sh",
        "-c",
        "i=0; while [ $i -lt 4000 ]; do echo line-$i-padding-padding; i=$((i+1)); done; echo done-printing",
    ]);
    spawn.journal_cap = 64 * 1024;
    let host = start(&spawn);
    let mut client = host.connect(false);
    eventually("the program finishes", || {
        client.info().child_exit.is_some()
    });
    let info = client.info();
    assert!(info.gap && info.tail_start > 0, "{info:?}");
    client.read_all();
    assert!(matches!(client.recv(), HostMessage::Gap { from: 0, .. }));
    client.output_until("done-printing");
}

/// The host refuses a child it cannot start, and leaves nothing behind.
///
/// Why: a caller waiting on the first stage must learn of the failure then,
/// not find a host whose child never ran. Spec: a nonexistent program makes
/// the first stage exit 1 with one line naming it, and no socket exists.
#[test]
fn a_child_that_cannot_start_refuses_the_start() {
    let (dir, started) = start_raw(
        &encode_spawn(&spec(&["/nonexistent/program"])),
        &["--socket", "{dir}/h"],
    );
    assert_eq!(started.status, 1, "{}", started.stderr);
    assert!(
        started.stderr.contains("/nonexistent/program"),
        "{}",
        started.stderr
    );
    assert!(!dir.path().join("h").exists());
}

/// Usage errors and an existing socket path are refused before anything
/// starts.
///
/// Why: the host never replaces a file it did not create. Spec: no
/// arguments exits 2; an existing socket path exits 1 naming it and leaves
/// the file alone; a malformed child spec exits 1.
#[test]
fn bad_starts_are_refused() {
    let (_, started) = start_raw(b"", &[]);
    assert_eq!(started.status, 2);
    let (dir, _) = start_raw(b"", &[]);
    std::fs::write(dir.path().join("taken"), b"x").unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_terminal-host"));
    let output = command
        .args(["--socket"])
        .arg(dir.path().join("taken"))
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(&encode_spawn(&spec(&["/bin/sh"])))?;
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read(dir.path().join("taken")).unwrap(), b"x");
    let (_, started) = start_raw(b"not a spec", &["--socket", "{dir}/h"]);
    assert_eq!(started.status, 1);
}

/// Frames sent just before a connection closes are still acted on.
///
/// Why: a client tearing a terminal down may send Shutdown and close
/// without waiting; losing that Shutdown would leave the host running
/// forever (SPEC.md, "Connections and version negotiation"). Spec: Hello and
/// Shutdown written in one go, followed at once by a close, end the host.
#[test]
fn a_shutdown_sent_just_before_closing_still_ends_the_host() {
    use std::io::Write as _;
    let host = start(&spec(&["/bin/cat"]));
    let mut bytes = encode_client(&ClientMessage::Hello {
        version: 1,
        take_over: true,
    });
    bytes.extend(encode_client(&ClientMessage::Shutdown));
    std::os::unix::net::UnixStream::connect(&host.socket)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    eventually("the host exits", || !running(host.pid));
}

/// A client that reads only after the program is done still gets every
/// byte, in order, and the program never waited for it.
///
/// Why: a slow client must cost the program nothing, and lose nothing
/// that the journal still holds (SPEC.md, "Reading"). Spec: with several
/// megabytes of output journaled while nobody read and the program exited
/// meanwhile, a read from (0, 0) yields contiguous output up to its last
/// line, within one bounded wait.
#[test]
fn a_slow_client_loses_nothing() {
    let mut spawn = spec(&[
        "/bin/sh",
        "-c",
        "i=0; while [ $i -lt 120000 ]; do echo line-$i-padding-padding; i=$((i+1)); done; echo last-line",
    ]);
    spawn.journal_cap = 64 * 1024 * 1024;
    let host = start(&spawn);
    let mut client = host.connect(false);
    eventually("the program finishes", || {
        client.info().child_exit.is_some()
    });
    client.read_all();
    let mut next = 0u64;
    // Only the end is looked at, so only the end is kept.
    let mut tail = Vec::new();
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(
            !left.is_zero(),
            "the last line never came; output up to {next}"
        );
        match client
            .try_recv(left)
            .expect("the host closed the connection")
        {
            HostMessage::Output { offset, data } => {
                assert_eq!(offset, next, "contiguous output");
                next += data.len() as u64;
                tail.extend(data);
                tail.drain(..tail.len().saturating_sub(64));
                if tail.ends_with(b"last-line\r\n") {
                    break;
                }
            }
            HostMessage::Exited { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// A second `Read` on a connection is refused, and the client re-reads by
/// reconnecting.
///
/// Why: version 1 has one stream per connection (SPEC.md, "Client
/// messages"); a client that wants to start over must not get a mix of two
/// streams it cannot tell apart. Spec: a second `Read` yields `Error` with
/// code 2 (`NotAllowed`) and end of stream; a new connection reads from
/// (0, 0) as usual.
#[test]
fn a_second_read_is_refused() {
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "echo hello; exec /bin/sleep 1000",
    ]));
    let mut client = host.connect(false);
    client.read_all();
    client.output_until("hello");
    client.read_all();
    let error = client.until(|m| matches!(m, HostMessage::Error { .. }));
    assert!(
        matches!(
            error,
            HostMessage::Error {
                code: ErrorCode::NotAllowed,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(client.closed());
    let mut again = host.connect(false);
    again.read_all();
    again.output_until("hello");
}

/// A host killed with SIGKILL hangs up its child, and leaves only its
/// socket file behind.
///
/// Why: anything can kill a host (the OOM killer, a user); its program must
/// not live on attached to nothing, and what is left must be plainly dead
/// (SPEC.md, "Ending"). Spec: after SIGKILL to a host running a shell, the
/// host is gone, the shell (which exits on hangup) is gone too, and the
/// socket file is still there but refuses connections.
#[test]
fn a_host_killed_with_sigkill_hangs_up_its_child() {
    let host = start(&spec(&["/bin/sh"]));
    let info = host.connect(false).info();
    // SAFETY: killing the host this test started (its pid is above 1).
    unsafe { libc::kill(host.pid as libc::pid_t, libc::SIGKILL) };
    eventually("the host is gone", || !running(host.pid));
    eventually("the child is hung up", || !running(info.child_pid));
    assert!(
        host.socket.exists(),
        "a killed host cannot remove its socket"
    );
    assert!(std::os::unix::net::UnixStream::connect(&host.socket).is_err());
}

/// `Shutdown` ends the host promptly even when its child ignores the
/// hangup.
///
/// Why: the host's end must not depend on its program (SPEC.md, "Ending":
/// it exits without waiting for anything), and cleaning up a process tree
/// is the client's business. Spec: with a child that ignores SIGHUP,
/// `Shutdown` gets `Goodbye`, and the host and its socket are gone within
/// the usual wait while the child still runs.
#[test]
fn shutdown_does_not_wait_for_a_child_that_ignores_the_hangup() {
    let host = start(&spec(&[
        "/bin/sh",
        "-c",
        "trap '' HUP; echo ready; exec /bin/sleep 1000",
    ]));
    let mut client = host.connect(false);
    client.read_all();
    client.output_until("ready");
    let child = client.info().child_pid;
    let _child = KillOnDrop(child);
    client.send(&ClientMessage::Shutdown);
    client.until(|m| matches!(m, HostMessage::Goodbye));
    eventually("the host exits", || !running(host.pid));
    assert!(!host.socket.exists());
    assert!(running(child), "premise: the child ignored the hangup");
}

/// After its child exits, the host lives on and leaves no zombie.
///
/// Why: a host lives for weeks; a child it never reaped would stay a zombie
/// as long, holding its pid (SPEC.md, "Child exit and end of output"). The
/// child's start time is taken while it runs, so that another process
/// given the same pid since cannot pass for the zombie. Spec: once the exit
/// record arrives, the host still runs and no process with the child's pid
/// and start time exists, not even as a zombie.
#[test]
fn the_exited_child_is_reaped() {
    let files = tempfile::tempdir().expect("fixture: a directory for the FIFO");
    let go = files.path().join("go");
    make_fifo(&go);
    let script = format!("read x < '{}'; exit 0", go.display());
    let host = start(&spec(&["/bin/sh", "-c", &script]));
    let mut client = host.connect(false);
    let child = client.info().child_pid;
    let started = start_time(child).expect("premise: the child runs");
    client.read_all();
    release(&go);
    client.until(|m| matches!(m, HostMessage::Exited { .. }));
    assert_ne!(
        start_time(child),
        Some(started),
        "the child {child} is left as a zombie"
    );
    assert!(running(host.pid));
}

/// A zero-length frame after Welcome is malformed and closes only that
/// client.
///
/// Why: complements the unknown-kind case; the frame splitter's own
/// refusal must reach the client too. Spec: four zero bytes after Welcome
/// yield Error (malformed) and end of stream.
#[test]
fn a_zero_length_frame_after_welcome_is_refused() {
    use std::io::Write as _;
    let host = start(&spec(&["/bin/cat"]));
    let mut client = host.connect(false);
    client.stream.write_all(&[0, 0, 0, 0]).unwrap();
    assert!(matches!(
        client.recv(),
        HostMessage::Error {
            code: ErrorCode::Malformed,
            ..
        }
    ));
    assert!(client.closed());
}
