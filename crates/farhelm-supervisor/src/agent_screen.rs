//! One program in a real terminal host, read and typed at the way the
//! supervisor does it: the backend of `scripts/agent-tui.py` and
//! `scripts/capture-agent-screens.py`, behind the hidden
//! `farhelm internal agent-screen` command.
//!
//! Those scripts exist to see what a real, installed agent draws in a
//! Farhelm terminal, and the agent-screen fixtures that status detection is
//! tested against are captured with them
//! (`docs/agent-screen-fixtures.md`). A terminal host alone is not such a
//! terminal: it keeps the pty and journals the program's output, but
//! answers none of the program's queries, and agents wait on some of those
//! answers before they draw anything (the cursor position, the terminal's
//! attributes). So this adopts the host with the supervisor's own runtime
//! ([`HostTerminal`]), whose emulator answers them as for any session's terminal,
//! and gives the child the environment a supervisor gives an agent's host
//! ([`crate::service::host_child_env`]). What a script reads back is the
//! text status detection reads (`Emulator::screen_text`, the whole visible
//! screen whose tail the sampler takes) and the title.
//!
//! It is a development tool with no compatibility promise: the request
//! format below changes together with the two scripts, which are its only
//! callers. Nothing of the live install is involved: the host lives in the
//! run directory the caller names, which this command uses as a state
//! directory of its own, and the child gets no `FARHELM_*` variable of the
//! caller's (inside a Farhelm session those would point the program under
//! test at the live helm).
//!
//! # The control socket
//!
//! Once the host is adopted, the command binds `<run dir>/control`, prints
//! `ready` on stdout (and nothing else, ever), and serves one request per
//! connection: a line of JSON, answered with a line of JSON, either
//! `{"ok": true, ...}` or `{"ok": false, "error": "..."}`.
//!
//! - `{"op": "screen"}`: `screen` (the visible screen as text), `title`,
//!   and `exited` (whether the program has exited; its last screen stays
//!   readable until the host ends).
//! - `{"op": "type", "text": "..."}`: the text as is, in one write.
//! - `{"op": "keys", "keys": ["Enter", "C-u"]}`: named keys ([`key_input`]),
//!   encoded from the program's modes as a grid viewer's keys are. Every
//!   name is checked before any key is sent, so a refused request sends
//!   nothing.
//! - `{"op": "stop"}`: answered first, then the host is shut down (ending
//!   the program) and the command exits.
//!
//! SIGTERM and SIGINT stop it the same way, between requests: a `type` or
//! `keys` waits until the host has written its input, which a program that
//! stopped reading its terminal holds up, and nothing else is served
//! meanwhile (the scripts bound each request themselves). A command that is
//! killed outright leaves its host running; `farhelm internal stop-terminal-hosts
//! --state-dir <run dir>` ends it, which is what the scripts' `stop` falls
//! back on.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use farhelm_proto::grid::modifiers;
use serde::Deserialize;
use terminal_host::protocol::Size;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::grid_input::{KeyPlatform, encode_key, input_frames};
use crate::hosts::{
    Adoption, ChildSpec, DEFAULT_JOURNAL_CAP, HostRole, HostTag, HostTerminal, HostTerminalOptions,
    Liveness, own_host_program, spawn_host,
};

/// What `farhelm internal agent-screen` starts.
pub struct AgentScreen {
    /// The run's own directory: the host's state directory and the control
    /// socket's home. Absolute, and short enough for both socket paths
    /// (unix socket paths are limited to about 104 bytes on macOS).
    pub run_dir: PathBuf,
    /// The program's working directory, absolute.
    pub cwd: PathBuf,
    /// The terminal's size.
    pub cols: u16,
    pub rows: u16,
    /// The program and its arguments.
    pub argv: Vec<OsString>,
    /// This `farhelm`, which the host is started from, as a supervisor
    /// starts its hosts from its own program.
    pub farhelm_exe: PathBuf,
}

/// One request on the control socket (module docs).
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Screen,
    Type { text: String },
    Keys { keys: Vec<String> },
    Stop,
}

/// Start the program in a host, adopt it, and serve the control socket
/// until a `stop` request or a signal; then shut the host down.
pub async fn run(spec: AgentScreen) -> anyhow::Result<()> {
    let terminal = start(&spec).await?;
    // Whatever ends the serving, the host is shut down after it: a failure
    // here must not leave the program running with nobody to stop it.
    let served = serve_until_stopped(&spec, &terminal).await;
    let shutdown =
        terminal.shutdown().await.map(drop).map_err(|liveness| {
            anyhow::anyhow!("the terminal host did not shut down: {liveness:?}")
        });
    served.and(shutdown)
}

/// Bind the control socket, report `ready`, and serve requests until a
/// `stop` request or a signal.
async fn serve_until_stopped(spec: &AgentScreen, terminal: &HostTerminal) -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let control = spec.run_dir.join("control");
    let listener = UnixListener::bind(&control)
        .with_context(|| format!("binding the control socket {}", control.display()))?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    {
        use std::io::Write as _;
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "ready")?;
        stdout.flush()?;
    }
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    if serve(terminal, stream).await {
                        break;
                    }
                }
                // A failed accept (out of descriptors, say) costs that one
                // connection; the caller retries or stops.
                Err(error) => {
                    eprintln!("farhelm internal agent-screen: accepting: {error}");
                    // Not at once again: an accept that fails for want of
                    // descriptors keeps failing until one is free.
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            },
            _ = terminate.recv() => break,
            _ = interrupt.recv() => break,
        }
    }
    drop(listener);
    let _ = std::fs::remove_file(&control);
    Ok(())
}

/// Start the host with the production child environment and adopt it as
/// freshly spawned, so that its emulator answers the program's first
/// queries.
async fn start(spec: &AgentScreen) -> anyhow::Result<HostTerminal> {
    let cwd = spec
        .cwd
        .to_str()
        .context("the working directory is not UTF-8")?;
    let base = std::env::vars_os().filter(|(name, _)| {
        !name
            .to_str()
            .is_some_and(|name| name.starts_with("FARHELM_"))
    });
    let shell = crate::launch::resolve_shell().await;
    let child = ChildSpec {
        argv: spec.argv.clone(),
        cwd: spec.cwd.clone(),
        env: crate::service::host_child_env(base, &[], &shell, cwd),
        size: Size {
            cols: spec.cols,
            rows: spec.rows,
        },
        journal_cap: DEFAULT_JOURNAL_CAP,
        initial_snapshot: None,
    };
    // A tag like an agent's, for a session that exists nowhere: no
    // supervisor ever looks in this state directory.
    let tag = HostTag {
        session_id: uuid::Uuid::new_v4().to_string(),
        created_ns: crate::hosts::tag::creation_stamp(),
        role: HostRole::Agent { generation: 0 },
    };
    let spawned = spawn_host(
        &own_host_program(&spec.farhelm_exe),
        &spec.run_dir,
        child,
        &tag,
    )
    .await
    .context("starting the terminal host")?;
    HostTerminal::adopt(
        &spawned.socket,
        Adoption::Fresh,
        HostTerminalOptions::default(),
    )
    .await
    .context("adopting the terminal host")
}

/// Answer one connection's request. True when it was `stop`.
async fn serve(terminal: &HostTerminal, stream: UnixStream) -> bool {
    let (read, mut write) = stream.into_split();
    let mut line = String::new();
    // A caller that connects and sends nothing must not hold the command
    // up for good: the scripts send their request at once.
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        BufReader::new(read).read_line(&mut line),
    )
    .await;
    if !matches!(read, Ok(Ok(n)) if n > 0) {
        return false;
    }
    let (answer, stop) = match serde_json::from_str::<Request>(&line) {
        Ok(Request::Stop) => (serde_json::json!({ "ok": true }), true),
        Ok(request) => (
            answer(terminal, request).await.unwrap_or_else(
                |error| serde_json::json!({ "ok": false, "error": format!("{error:#}") }),
            ),
            false,
        ),
        Err(error) => (
            serde_json::json!({ "ok": false, "error": format!("not a request: {error}") }),
            false,
        ),
    };
    let mut bytes = answer.to_string().into_bytes();
    bytes.push(b'\n');
    let _ = write.write_all(&bytes).await;
    let _ = write.shutdown().await;
    stop
}

/// The answer to every request but `stop`.
async fn answer(terminal: &HostTerminal, request: Request) -> anyhow::Result<serde_json::Value> {
    match request {
        Request::Screen => {
            let screen = terminal
                .read(|emulator| emulator.screen_text())
                .await
                .context("reading the screen")?;
            let exited = !matches!(terminal.liveness(), Liveness::Running { .. });
            Ok(serde_json::json!({
                "ok": true,
                "screen": screen,
                "title": terminal.title(),
                "exited": exited,
            }))
        }
        Request::Type { text } => {
            write_input(terminal, text.as_bytes()).await?;
            Ok(serde_json::json!({ "ok": true }))
        }
        Request::Keys { keys } => {
            if let Some(unknown) = keys.iter().find(|name| key_input(name, false).is_none()) {
                anyhow::bail!("no key named {unknown:?}");
            }
            for name in &keys {
                // Encoded one at a time, from the mode in force when each is
                // sent, since an earlier key can make the program change it.
                let application_cursor = terminal.modes().application_cursor;
                let bytes = key_input(name, application_cursor).expect("checked above");
                write_input(terminal, &bytes).await?;
            }
            Ok(serde_json::json!({ "ok": true }))
        }
        Request::Stop => unreachable!("answered by the caller"),
    }
}

/// Write `bytes` to the program and wait until the host has written them to
/// the pty, so that a script's next request sees their effect start.
async fn write_input(terminal: &HostTerminal, bytes: &[u8]) -> anyhow::Result<()> {
    for frame in input_frames(bytes) {
        let receipt = terminal
            .send_input(frame)
            .await
            .map_err(|refused| anyhow::anyhow!("the terminal took no input: {refused}"))?;
        let fate = receipt.fate().await;
        anyhow::ensure!(
            fate == crate::hosts::InputFate::Written,
            "the input was not written: {fate:?}"
        );
    }
    Ok(())
}

/// The bytes a grid viewer's keydown for the named key sends to a program
/// whose cursor-key mode is `application_cursor`. The names are the ones
/// tmux's `send-keys` uses, which the scripts were first written against.
///
/// The named keys are `Enter`, `Escape`, `Tab`, `BTab` (Shift+Tab),
/// `BSpace`, `Space`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`,
/// `PageUp`/`PPage`, `PageDown`/`NPage`, `Delete`/`DC`, `S-Enter`
/// (Shift+Enter), `F1` to `F12`, and `C-<letter>` (Ctrl with a letter).
/// Anything else is `None`.
pub fn key_input(name: &str, application_cursor: bool) -> Option<Vec<u8>> {
    // A space is text, not a keydown with a sequence of its own: the grid
    // client sends it as text input, and the keydown encoder gives nothing.
    if name == "Space" {
        return Some(b" ".to_vec());
    }
    let (key, code, kc, mods): (String, String, u32, u8) = match name {
        "Enter" => ("Enter".into(), "Enter".into(), 13, 0),
        "S-Enter" => ("Enter".into(), "Enter".into(), 13, modifiers::SHIFT),
        "Escape" => ("Escape".into(), "Escape".into(), 27, 0),
        "Tab" => ("Tab".into(), "Tab".into(), 9, 0),
        "BTab" => ("Tab".into(), "Tab".into(), 9, modifiers::SHIFT),
        "BSpace" => ("Backspace".into(), "Backspace".into(), 8, 0),
        "Up" => ("ArrowUp".into(), "ArrowUp".into(), 38, 0),
        "Down" => ("ArrowDown".into(), "ArrowDown".into(), 40, 0),
        "Left" => ("ArrowLeft".into(), "ArrowLeft".into(), 37, 0),
        "Right" => ("ArrowRight".into(), "ArrowRight".into(), 39, 0),
        "Home" => ("Home".into(), "Home".into(), 36, 0),
        "End" => ("End".into(), "End".into(), 35, 0),
        "PageUp" | "PPage" => ("PageUp".into(), "PageUp".into(), 33, 0),
        "PageDown" | "NPage" => ("PageDown".into(), "PageDown".into(), 34, 0),
        "Delete" | "DC" => ("Delete".into(), "Delete".into(), 46, 0),
        _ => {
            if let Some(n) = name
                .strip_prefix('F')
                .and_then(|n| n.parse::<u32>().ok())
                .filter(|n| (1..=12).contains(n))
            {
                (name.into(), name.into(), 111 + n, 0)
            } else {
                let letter = name
                    .strip_prefix("C-")
                    .filter(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_lowercase())?;
                let upper = letter.to_ascii_uppercase();
                (
                    letter.into(),
                    format!("Key{upper}"),
                    u32::from(upper.as_bytes()[0]),
                    modifiers::CTRL,
                )
            }
        }
    };
    let bytes = encode_key(
        &key,
        &code,
        kc,
        mods,
        application_cursor,
        KeyPlatform::Other,
    );
    (!bytes.is_empty()).then_some(bytes)
}

/// Whether `run_dir` leaves room for the host socket and the control socket
/// under the platform's limit; the command refuses a longer one up front
/// rather than failing in the middle of a start.
pub fn run_dir_fits(run_dir: &Path) -> bool {
    // `<run dir>/hosts/<8-character name>` is the longer of the two.
    run_dir.as_os_str().len() + "/hosts/".len() + 8 < 100
}

#[cfg(test)]
mod tests {
    //! The key names, against the bytes a terminal program reads for them.
    //! The rest (a real host, its emulator and the control socket) is
    //! exercised by `scripts/agent-tui.py self-test`, which needs the built
    //! `farhelm`.

    use super::*;

    /// The keys the capture scripts press reach the program as a terminal
    /// sends them, cursor keys following the program's cursor-key mode.
    ///
    /// Why: the scripts drive real agents through dialogs by these names; a
    /// name that encoded to something else would answer a dialog wrongly
    /// (or not at all) and cost a capture. Spec: Enter is CR, Escape ESC,
    /// Ctrl+U 0x15, Ctrl+O 0x0f, Ctrl+C 0x03, Down `CSI B` normally and
    /// `SS3 B` in application cursor mode, Shift+Tab `CSI Z`, Shift+Enter
    /// ESC CR, F1 `SS3 P`, Space a space; unknown names, `C-` with no letter or an upper
    /// case one, and `F13` are refused.
    #[test]
    fn named_keys_send_what_a_terminal_sends() {
        let key = |name| key_input(name, false).unwrap();
        assert_eq!(key("Enter"), b"\r");
        assert_eq!(key("Escape"), b"\x1b");
        assert_eq!(key("C-u"), b"\x15");
        assert_eq!(key("C-o"), b"\x0f");
        assert_eq!(key("C-c"), b"\x03");
        assert_eq!(key("Down"), b"\x1b[B");
        assert_eq!(key_input("Down", true).unwrap(), b"\x1bOB");
        assert_eq!(key("BTab"), b"\x1b[Z");
        assert_eq!(key("S-Enter"), b"\x1b\r");
        assert_eq!(key("F1"), b"\x1bOP");
        assert_eq!(key("Space"), b" ");
        for unknown in ["Return", "C-", "C-U", "C-ab", "F13", ""] {
            assert_eq!(key_input(unknown, false), None, "{unknown:?}");
        }
    }

    /// A run directory under `/tmp` with a short random name fits both
    /// sockets, and the limit sits where the host socket's path would reach
    /// 100 bytes.
    ///
    /// Why: the scripts make their run directories directly under `/tmp`
    /// for this reason, and a refusal up front is clearer than a host that
    /// cannot bind. Spec: `/tmp/fh-tui-abcdefgh` fits; an 84-byte directory
    /// fits (its host socket path is 99 bytes) and an 85-byte one does not.
    #[test]
    fn run_dir_length_is_checked_against_the_socket_limit() {
        assert!(run_dir_fits(Path::new("/tmp/fh-tui-abcdefgh")));
        let of_length = |len: usize| PathBuf::from(format!("/tmp/{}", "x".repeat(len - 5)));
        assert!(run_dir_fits(&of_length(84)));
        assert!(!run_dir_fits(&of_length(85)));
    }
}
