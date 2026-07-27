//! The fake agent: a deterministic scripted TUI standing in for Claude
//! Code / Codex in tests.
//!
//! Why it exists (PLAN_M1.md): end-to-end tests — including replay and
//! mode restoration — must run without vendor auth, tokens, or
//! nondeterminism. The scripts exercise exactly the terminal behaviors
//! the real agents use: colored output, a prompt that echoes input,
//! bracketed paste mode, the alternate screen, and output that continues
//! while a client reattaches. Real-agent smoke testing stays manual.
//!
//! Contract with tests: every script prints `FAKE-AGENT READY` once
//! its modes are set and it is listening — tests key on that marker
//! instead of sleeping.

use std::io::{BufRead, Read, Write};

/// Which terminal behavior to act out. A closed set, so clap validates it
/// at parse time and `--help` documents it, rather than failing at
/// runtime on a typo in a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Script {
    /// Prompt-and-echo with color and bracketed paste enabled.
    Basic,
    /// Full-screen app on the alternate screen.
    Altscreen,
    /// Raw non-UTF-8 output for byte-fidelity tests.
    Binary,
    /// Continuous numbered records for replay/live cutover tests.
    Counter,
    /// Raw-mode hex echo of every input byte, for input-fidelity tests.
    Hexecho,
}

/// Act out one script and exit. Runs synchronously on blocking stdio on
/// purpose: this stands in for a real agent's terminal behavior, and
/// nothing about it should depend on an async runtime being present.
pub fn run(script: Script) -> anyhow::Result<()> {
    match script {
        Script::Basic => basic(),
        Script::Altscreen => altscreen(),
        Script::Binary => binary(),
        Script::Counter => counter(),
        Script::Hexecho => hexecho(),
    }
}

/// Prompt-and-echo with color, bracketed paste, and test control commands.
///
/// Ordinary lines come back as `echo:<line>`; `spam`, `size`, and `quit`
/// exercise scrollback, real PTY geometry, and clean exit respectively.
/// Bracketed paste stays enabled so reattach tests can assert the mode
/// survives replay (the audited silent-loss case in SPEC_impl.md).
fn basic() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    // Bracketed paste on. Set before the ready marker so a test that
    // reattaches the moment it sees the marker always finds the mode
    // already live in tmux's pane state.
    write!(out, "\x1b[?2004h")?;
    writeln!(out, "\x1b[1;32mfake-agent\x1b[0m starting (script=basic)\r")?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    write!(out, "> ")?;
    out.flush()?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        // Strip bracketed paste markers so pasted and typed input assert
        // identically in tests.
        let line = line.replace("\x1b[200~", "").replace("\x1b[201~", "");
        let trimmed = line.trim();
        if trimmed == "quit" {
            writeln!(out, "bye\r")?;
            out.flush()?;
            return Ok(());
        }
        // Report the real PTY geometry on demand. The browser resize test
        // uses this rather than trusting xterm's local dimensions, which
        // can change even when the WebSocket-to-tmux resize path is
        // disconnected.
        if trimmed == "size" {
            let size = std::process::Command::new("stty")
                .arg("size")
                .stdin(std::process::Stdio::inherit())
                .output()?;
            if !size.status.success() {
                anyhow::bail!(
                    "stty size failed: {}",
                    String::from_utf8_lossy(&size.stderr).trim()
                );
            }
            writeln!(
                out,
                "size:{}\r",
                String::from_utf8_lossy(&size.stdout).trim()
            )?;
            write!(out, "> ")?;
            out.flush()?;
            continue;
        }
        // `spam N` emits N numbered lines, so replay tests can push
        // content off the visible screen and prove that scrollback —
        // not just the current frame — comes back on reattach.
        if let Some(count) = trimmed.strip_prefix("spam ")
            && let Ok(n) = count.trim().parse::<usize>()
        {
            for i in 1..=n {
                writeln!(out, "spam-line-{i}\r")?;
            }
            write!(out, "> ")?;
            out.flush()?;
            continue;
        }
        writeln!(out, "echo:\x1b[36m{trimmed}\x1b[0m\r")?;
        write!(out, "> ")?;
        out.flush()?;
    }
    Ok(())
}

/// Enter the alternate screen, draw a full-screen frame, wait for a line,
/// then leave. Lets tests assert alt-screen passthrough end to end and
/// alt-screen replay on reattach.
fn altscreen() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?1049h\x1b[2J\x1b[H")?;
    writeln!(out, "\x1b[7m ALT-SCREEN APP \x1b[0m\r")?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    write!(out, "\x1b[?1049l")?;
    writeln!(out, "left alt screen\r")?;
    out.flush()?;
    Ok(())
}

/// Emit a byte that is invalid UTF-8, then remain alive for streaming.
///
/// This script exists because terminal output is bytes, not text. A
/// lossy conversion in the live control path would replace 0xff and
/// still leave every ordinary fake-agent test green. Capture replay is
/// deliberately outside this contract: tmux may canonicalize invalid
/// source bytes when it stores the terminal grid.
fn binary() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(b"\xffBINARY-MARKER\r\nFAKE-AGENT READY\r\n")?;
    out.flush()?;

    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(())
}

/// Emit an unbroken numbered stream until the surrounding tmux session
/// kills the process.
///
/// Each record is flushed separately and fits in one PTY write. Replay
/// cutover tests can therefore distinguish a real gap or overlap from
/// buffering in this fixture: the expected transcript is a consecutive
/// integer range with every value appearing once.
fn counter() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;
    for sequence in 0_u64.. {
        writeln!(out, "CUTOVER-{sequence:08}\r")?;
        out.flush()?;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    unreachable!("the counter fixture runs until its session is killed")
}

/// Echo every input byte back as lowercase hex, one space-separated line
/// per read, so a test can observe input-byte fidelity through terminal
/// OUTPUT.
///
/// Exists because the tmux `paste-buffer` input-mangling bug (control
/// bytes like DEL/ESC/ETX arriving caret-escaped as `^?`/`^[`/`^C`) was
/// invisible to `basic` and every other script here: they read stdin in
/// the pty's default canonical mode, where the kernel line discipline
/// itself intercepts or reinterprets those bytes (erase, escape, SIGINT)
/// before an app ever sees them — so a mangled byte and a correct one
/// produced the same visible effect. `hexecho` puts its stdin into raw
/// mode specifically to remove that filter: whatever byte crosses the
/// wire is exactly the byte printed here, which is what makes the fixture
/// able to tell "arrived as 0x7f" apart from "arrived as the two
/// characters `^` and `?`".
fn hexecho() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();

    // Raw mode BEFORE the ready marker — unlike `basic`'s bracketed-paste
    // enable, which is a pane escape sequence tmux records the instant it
    // is written. Terminal mode bits live in the pty's line discipline,
    // not the pane, so a test that sends input the instant it sees READY
    // must always find raw mode already established; printing READY
    // first would let a fast test race a still-canonical pty and land its
    // control bytes in cooked mode, defeating the whole point of this
    // fixture.
    set_raw_mode()?;

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    use std::fmt::Write as _;
    let mut stdin = std::io::stdin().lock();
    let mut buf = [0u8; 4096];
    loop {
        let n = stdin.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        let mut line = String::with_capacity(n * 3);
        for (i, byte) in buf[..n].iter().enumerate() {
            if i > 0 {
                line.push(' ');
            }
            write!(line, "{byte:02x}").expect("String write is infallible");
        }
        writeln!(out, "{line}\r")?;
        out.flush()?;
    }
}

/// Put this process's controlling terminal into raw mode: no canonical
/// line editing, no signal-generating control characters, no local echo.
///
/// Nothing upstream configures the pty this way — tmux hands out an
/// ordinary cooked-mode pty, and `basic` deliberately reads via
/// `BufRead::lines` to exercise that default. `cfmakeraw` is not POSIX
/// itself (POSIX standardizes termios but not this convenience function);
/// it is a BSD-originated libc extension present on Linux and every other
/// target this project builds for, and it flips every relevant termios
/// flag at once, rather than hand-listing `ICANON`/`ECHO`/`ISIG`/`IXON`
/// and hoping the list stays complete.
fn set_raw_mode() -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;

    let fd = std::io::stdin().as_raw_fd();
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is stdin's own fd, valid for the process lifetime, and
    // `term` is a plain-old-data struct large enough for `tcgetattr` to
    // fill in (zero-initialized so any field it doesn't touch is still
    // well-defined).
    if unsafe { libc::tcgetattr(fd, &mut term) } != 0 {
        anyhow::bail!("tcgetattr failed: {}", std::io::Error::last_os_error());
    }
    // SAFETY: `term` was just populated by `tcgetattr` above.
    unsafe { libc::cfmakeraw(&mut term) };
    // SAFETY: same fd and a `term` value `cfmakeraw` just produced from a
    // real `tcgetattr` snapshot.
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &term) } != 0 {
        anyhow::bail!("tcsetattr failed: {}", std::io::Error::last_os_error());
    }
    Ok(())
}
