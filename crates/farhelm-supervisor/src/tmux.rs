//! The tmux driver: a private, headless tmux server used purely as a PTY
//! holder and history store (SPEC_impl.md, "Terminal substrate").
//!
//! Nothing here ever runs a rendering `tmux attach`. Output is consumed
//! through a non-rendering control-mode client (`tmux -C`), input goes in
//! via `load-buffer`/`paste-buffer` over stdin, sizing is pinned by
//! explicit `resize-window` calls, and replay is `capture-pane -e`
//! history plus re-synthesized pane modes. The motivations — native xterm.js
//! scrolling, no tmux UI anywhere — are recorded in SPEC_impl.md; this
//! module is that design made concrete.

use anyhow::{Context, bail};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};

/// History (and therefore replay) floor. SPEC.md promises at least the
/// screen plus 10,000 lines; the margin covers lines scrolled during the
/// capture itself.
pub const HISTORY_LIMIT: u32 = 12_000;

/// One tmux control-mode notification may expand each terminal byte into
/// a four-byte octal escape. Bound the line above the protocol frame cap
/// without rejecting the largest valid escaped notification.
const MAX_CONTROL_LINE: usize = farhelm_proto::MAX_FRAME_LEN as usize * 4 + 1024;

/// One deadline covers attaching the control client, taking the replay
/// snapshot, and enabling live output. A wedged tmux command must fail
/// the attach request instead of leaving it holding the global
/// attachment lock forever.
const CONTROL_EXCHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The format is deliberately comma-separated. See [`PaneModes::parse`].
const PANE_MODE_FORMAT: &str = "#{alternate_on},#{bracket_paste_flag},#{mouse_any_flag},\
                                #{mouse_button_flag},#{mouse_standard_flag},#{mouse_sgr_flag},\
                                #{cursor_flag},#{keypad_cursor_flag},#{cursor_x},#{cursor_y}";

/// Handle to the private tmux server. All tmux invocations go through
/// this so the `-S <socket> -f <config>` isolation is impossible to
/// forget — the user's own tmux server and config must never be touched.
#[derive(Debug, Clone)]
pub struct TmuxDriver {
    socket: PathBuf,
    config: PathBuf,
}

/// Enforce the control-mode floor before starting or adopting a server.
///
/// tmux suffixes patch releases (`3.3a`) and development builds with
/// letters; only the numeric major/minor pair decides compatibility.
fn require_supported_tmux(output: &str) -> anyhow::Result<()> {
    let version = output
        .split_whitespace()
        .nth(1)
        .context("tmux -V returned no version")?;
    let (major, minor) = version
        .split_once('.')
        .context("tmux -V returned an unrecognized version")?;
    let major: u32 = major.parse().context("parsing tmux major version")?;
    let minor_digits: String = minor
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    let minor: u32 = minor_digits.parse().context("parsing tmux minor version")?;
    if (major, minor) < (3, 3) {
        bail!("tmux {version} is unsupported; Farhelm requires tmux 3.3 or newer");
    }
    Ok(())
}

/// Pane state needed to make a fresh xterm.js behave as if it had been
/// attached all along. Captured from tmux format variables at attach
/// time; content replay alone silently loses these (SPEC_impl.md:
/// bracketed paste and mouse reporting are the headline casualties).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneModes {
    pub alternate_on: bool,
    pub bracket_paste: bool,
    pub mouse_any: bool,
    pub mouse_button: bool,
    pub mouse_standard: bool,
    pub mouse_sgr: bool,
    pub cursor_visible: bool,
    pub app_cursor_keys: bool,
    /// Cursor column, 0-based as tmux reports it. The CUP escape sequence
    /// is 1-based, so `post_content_sequences` adds one — do not
    /// pre-adjust here or the cursor lands a cell off on every reattach.
    pub cursor_x: u16,
    /// Cursor row, 0-based; see `cursor_x`.
    pub cursor_y: u16,
}

impl PaneModes {
    /// Parse the comma-separated format expansion produced by
    /// `TmuxDriver::pane_modes`. Empty or unparseable fields take the
    /// mode's default (off, except cursor visibility which defaults on),
    /// so a tmux build lacking one format name loses exactly that one
    /// mode rather than corrupting the rest.
    fn parse(line: &str) -> PaneModes {
        let mut it = line.split(',');
        let mut flag = |default: bool| -> bool {
            match it.next() {
                Some("") | None => default,
                Some(v) => v == "1",
            }
        };
        let alternate_on = flag(false);
        let bracket_paste = flag(false);
        let mouse_any = flag(false);
        let mouse_button = flag(false);
        let mouse_standard = flag(false);
        let mouse_sgr = flag(false);
        let cursor_visible = flag(true);
        let app_cursor_keys = flag(false);
        let mut num = || -> u16 { it.next().and_then(|v| v.parse().ok()).unwrap_or(0) };
        let cursor_x = num();
        let cursor_y = num();
        PaneModes {
            alternate_on,
            bracket_paste,
            mouse_any,
            mouse_button,
            mouse_standard,
            mouse_sgr,
            cursor_visible,
            app_cursor_keys,
            cursor_x,
            cursor_y,
        }
    }

    /// Escape sequences that must precede the content prefill.
    ///
    /// The alternate-screen switch belongs here and nowhere else:
    /// `\x1b[?1049h` switches to a *cleared* alternate buffer, so
    /// emitting it after the prefill would wipe the very content just
    /// replayed and leave the reattaching user staring at a blank
    /// screen. The cutover snapshot returns alt-screen contents when the
    /// pane is on the alternate screen, so the buffer must be selected
    /// first.
    pub fn pre_content_sequences(&self) -> String {
        if self.alternate_on {
            "\x1b[?1049h".to_string()
        } else {
            String::new()
        }
    }

    /// Escape sequences that follow the content prefill: input modes,
    /// then cursor placement. These must come after the content because
    /// writing the prefill moves the cursor.
    pub fn post_content_sequences(&self) -> String {
        let mut s = String::new();
        if self.bracket_paste {
            s.push_str("\x1b[?2004h");
        }
        if self.mouse_standard {
            s.push_str("\x1b[?1000h");
        }
        if self.mouse_button {
            s.push_str("\x1b[?1002h");
        }
        if self.mouse_any {
            s.push_str("\x1b[?1003h");
        }
        if self.mouse_sgr {
            s.push_str("\x1b[?1006h");
        }
        if self.app_cursor_keys {
            s.push_str("\x1b[?1h");
        }
        // Cursor position is 1-based in the escape sequence.
        s.push_str(&format!(
            "\x1b[{};{}H",
            self.cursor_y + 1,
            self.cursor_x + 1
        ));
        // Visibility last so a hidden cursor stays hidden through the
        // positioning above.
        if !self.cursor_visible {
            s.push_str("\x1b[?25l");
        }
        s
    }
}

impl TmuxDriver {
    /// `state_dir` owns the socket and generated config. The config file
    /// is rewritten whenever the driver starts, while a server already
    /// running on the private socket retains its live option values until
    /// explicitly changed or restarted.
    pub fn new(state_dir: &Path) -> TmuxDriver {
        TmuxDriver {
            socket: state_dir.join("tmux.sock"),
            config: state_dir.join("tmux.conf"),
        }
    }

    /// The generated server config. Every line is load-bearing:
    /// - `exit-empty off`: without it `start-server` on a fresh socket
    ///   leaves no server at all (verified: `list-sessions` reports "no
    ///   server running"), and an existing server would exit the moment
    ///   its last session was killed.
    /// - `status off` / no prefix: tmux UI must never appear (SPEC_impl).
    /// - `history-limit`: the SPEC.md replay floor. New servers derive it
    ///   from the same `HISTORY_LIMIT` that replay capture requests. An
    ///   adopted server can retain an older live value, which is why
    ///   changing this constant needs an explicit option-migration path.
    /// - `remain-on-exit on`: dead panes stay viewable (SPEC.md).
    /// - `default-terminal xterm-256color`: what xterm.js actually is;
    ///   inner apps probe $TERM.
    /// - `escape-time 0`: tmux waits after a lone ESC byte to see whether
    ///   an escape sequence follows. The default is 500ms before tmux 3.5
    ///   and 10ms from 3.5 on — so at the 3.3 floor the delay is half a
    ///   second, showing up as visibly laggy Esc handling in agent TUIs
    ///   and vim; 0 removes it entirely.
    ///
    /// NOT set here: `window-size manual`. It crashes the tmux 3.4 server
    /// outright — `new-session` then returns "server exited unexpectedly",
    /// in any spelling (`set -g`, `setw -g`, `set -w -g`) — and 3.4 is
    /// what Ubuntu 24.04 ships, so it took out every session on CI while
    /// passing locally on 3.7. It is unnecessary anyway: the supervisor's
    /// control-mode client never declares a size (no `refresh-client -C`),
    /// so tmux ignores it for sizing, and `resize-window` sets
    /// `window-size manual` on the window it touches. Both verified
    /// against 3.4 directly.
    ///
    /// Option tables are named explicitly — `set -s` for server options,
    /// `set -g` for session, `setw -g` for window — rather than relying
    /// on tmux inferring the table. Inference works on recent versions;
    /// being explicit works on every version at the 3.3 floor, and a
    /// config line tmux cannot place is a config line that silently does
    /// nothing.
    fn config_body() -> String {
        format!(
            "set -s exit-empty off\n\
             set -s escape-time 0\n\
             set -s default-terminal 'xterm-256color'\n\
             set -g status off\n\
             set -g prefix None\n\
             set -g history-limit {HISTORY_LIMIT}\n\
             setw -g remain-on-exit on\n"
        )
    }

    /// A tmux `Command` already pointed at the private server.
    ///
    /// EVERY tmux invocation in this module is built here, which is what
    /// makes the `-S`/`-f` isolation unforgettable: touching the user's
    /// own tmux server and config would be a serious violation, and a
    /// forgotten flag is exactly how that happens.
    fn command(&self) -> Command {
        let mut cmd = Command::new("tmux");
        cmd.arg("-S").arg(&self.socket).arg("-f").arg(&self.config);
        cmd
    }

    /// Run one tmux command against the private server and return its
    /// stdout, turning a non-zero exit into an error carrying tmux's own
    /// stderr — tmux explains its refusals in prose ("can't find session",
    /// "command too long") and that text is the whole diagnostic.
    async fn run(&self, args: &[&str]) -> anyhow::Result<String> {
        let out = self.run_bytes(args).await?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    /// Like [`TmuxDriver::run`], but returning stdout as raw bytes.
    ///
    /// Avoids adding another lossy UTF-8 conversion to `capture-pane`
    /// output. Tmux may already canonicalize invalid source bytes while
    /// storing its terminal grid, but valid multibyte and non-ASCII
    /// content should reach replay unchanged. The live `%output` path is
    /// byte-clean and bypasses the grid (see `OutputStream::next_output`).
    async fn run_bytes(&self, args: &[&str]) -> anyhow::Result<Vec<u8>> {
        // `stdin` is null so tmux can never inherit the supervisor's own
        // stdin — under `farhelm internal stdio` that stdin is the
        // protocol stream.
        let out = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .await
            .context("spawning tmux")?;
        if !out.status.success() {
            bail!(
                "tmux {:?} failed ({}): {}",
                args,
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(out.stdout)
    }

    /// Like [`TmuxDriver::run`], but feeding `input` to tmux's stdin.
    /// Exists for `load-buffer -`: bytes that travel via stdin never
    /// appear in `/proc/<pid>/cmdline`, which is the point (see
    /// `send_input`).
    async fn run_with_stdin(&self, args: &[&str], input: &[u8]) -> anyhow::Result<()> {
        use tokio::io::AsyncWriteExt;
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning tmux")?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(input).await.context("writing tmux stdin")?;
        drop(stdin);
        let out = child.wait_with_output().await.context("waiting for tmux")?;
        if !out.status.success() {
            bail!(
                "tmux {:?} failed ({}): {}",
                args,
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    /// Start (or adopt) the private server. Idempotent: an already
    /// running server on this socket is left exactly as it is, per the
    /// discovery-first rule — never restart a running substrate.
    pub async fn ensure_server(&self) -> anyhow::Result<()> {
        let version = Command::new("tmux")
            .arg("-V")
            .output()
            .await
            .context("checking tmux version")?;
        if !version.status.success() {
            bail!(
                "tmux -V failed ({}): {}",
                version.status,
                String::from_utf8_lossy(&version.stderr).trim()
            );
        }
        require_supported_tmux(&String::from_utf8_lossy(&version.stdout))?;
        if let Some(dir) = self.socket.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&self.config, Self::config_body()).await?;
        self.run(&["start-server"]).await?;
        Ok(())
    }

    /// Create a session running `window_cmd` (argv, executed directly by
    /// tmux — no extra shell layer beyond the one in the argv itself).
    /// Returns the pane id (`%N`), the stable handle for all later
    /// operations on this terminal.
    ///
    /// Dimensions are clamped exactly like [`TmuxDriver::resize_window`]:
    /// `new-session -x 0` is a hard tmux error ("width too small"), and a
    /// caller that has no real terminal yet (a browser mid-layout, a
    /// script) must get a session, not a confusing tmux refusal.
    pub async fn create_session(
        &self,
        name: &str,
        cwd: &Path,
        cols: u16,
        rows: u16,
        window_cmd: &[String],
    ) -> anyhow::Result<String> {
        let cols_s = cols.clamp(1, 10_000).to_string();
        let rows_s = rows.clamp(1, 10_000).to_string();
        let cwd_s = cwd.to_string_lossy().into_owned();
        // `-P -F` prints the pane id from the same invocation that
        // creates the session. One call, not new-session followed by a
        // display-message query: if the follow-up query failed, the
        // session (and the agent already launching in it) would exist
        // untracked — a live process with no owner, violating the
        // "failed create leaves nothing behind" contract.
        let mut args: Vec<&str> = vec![
            "new-session",
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-s",
            name,
            "-x",
            &cols_s,
            "-y",
            &rows_s,
            "-c",
            &cwd_s,
        ];
        args.extend(window_cmd.iter().map(String::as_str));
        let pane = self.run(&args).await?;
        Ok(pane.trim().to_string())
    }

    /// Resize a session's window. `cols`/`rows` are clamped to tmux's
    /// accepted range: a browser reporting 0 columns (or an absurd
    /// value) must not turn into a tmux error, because callers treat
    /// resize as fire-and-forget.
    pub async fn resize_window(&self, name: &str, cols: u16, rows: u16) -> anyhow::Result<()> {
        let cols = cols.clamp(1, 10_000);
        let rows = rows.clamp(1, 10_000);
        self.run(&[
            "resize-window",
            "-t",
            name,
            "-x",
            &cols.to_string(),
            "-y",
            &rows.to_string(),
        ])
        .await?;
        Ok(())
    }

    /// Send raw bytes as pane input, via a tmux paste buffer loaded over
    /// stdin.
    ///
    /// stdin delivery is a security property, not a convenience: input
    /// includes credentials users type at agent login/API-key prompts,
    /// and the previous `send-keys -H` shape put every input byte,
    /// hex-encoded, on a spawned process's argv — world-readable through
    /// `/proc/<pid>/cmdline` for the life of each spawn. (It also capped
    /// chunks at 512 bytes because tmux rejects ~1000-argument commands;
    /// buffers have no such limit, so one call handles any paste.)
    ///
    /// Flags are load-bearing: `-r` stops `paste-buffer`'s default
    /// LF→CR rewriting — bytes here are exact terminal input (escape
    /// sequences, mouse reports, bracketed pastes xterm.js already
    /// wrapped) and must arrive verbatim; `-d` deletes the buffer after
    /// pasting so the input does not linger, readable, in the tmux
    /// server's buffer list.
    ///
    /// The buffer name carries a per-call counter, not just the pane id:
    /// two overlapping sends to one pane would otherwise share a buffer,
    /// and the interleaving load/load/paste/paste pastes one caller's
    /// bytes under the other's name and then fails the second paste with
    /// "no buffer" — input silently swapped or dropped. A failed paste
    /// also deletes the buffer explicitly, since `-d` never ran and the
    /// bytes may be credentials typed at an agent prompt.
    pub async fn send_input(&self, pane: &str, bytes: &[u8]) -> anyhow::Result<()> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);

        if bytes.is_empty() {
            return Ok(());
        }
        let buf = format!(
            "farhelm-in-{}-{}",
            pane.trim_start_matches('%'),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        // Cleanup runs on BOTH failure paths: a failed load can still
        // have created the buffer (a mid-write stdin error leaves the
        // child free to finish with a truncated payload), and a failed
        // paste means `-d` never ran. Either way the bytes must not
        // linger; deleting a buffer that never existed is harmless.
        let result = async {
            self.run_with_stdin(&["load-buffer", "-b", &buf, "-"], bytes)
                .await?;
            self.run(&["paste-buffer", "-d", "-r", "-b", &buf, "-t", pane])
                .await?;
            Ok(())
        }
        .await;
        if result.is_err()
            && let Err(e) = self.run(&["delete-buffer", "-b", &buf]).await
        {
            // Best-effort, but never silent: if this ALSO failed, bytes
            // that may be credentials are sitting readable in the tmux
            // server's buffer list, and no other diagnostic says so.
            tracing::warn!(buffer = %buf, error = %e, "could not delete input buffer");
        }
        result
    }

    /// Open one control client, capture replay, then turn on its live
    /// output without leaving a gap between the two.
    ///
    /// The client attaches with tmux's `no-output` flag. Mode query, two
    /// snapshots, and `refresh-client -f !no-output` are then submitted
    /// as one command group through that same client. tmux runs a command
    /// group synchronously before returning to pane reads, so the final
    /// command's `%end` is the cutover: pane bytes before it are already
    /// represented by the selected snapshot, while bytes after it arrive
    /// as `%output` on this stream.
    ///
    /// Both history and visible-only snapshots are taken because modes
    /// decide which one is valid. Normal-screen replay includes
    /// scrollback; alternate-screen replay must not mix in the normal
    /// screen's history. Keeping both captures in the same command group
    /// avoids a second mode/capture race without depending on nested
    /// `if-shell` reply blocks, whose shape changed across supported tmux
    /// releases.
    pub async fn open_replay_stream(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>, OutputStream)> {
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-f")
            .arg("no-output")
            .arg("-t")
            .arg(session)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux control-mode client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let mut stream = OutputStream {
            child,
            stdin,
            reader: BufReader::new(stdout),
            line: Vec::with_capacity(8192),
            passthrough: PassthroughDecoder::default(),
        };
        read_command_block(
            &mut stream.reader,
            &mut stream.line,
            deadline,
            "control-mode attach",
        )
        .await?;
        let (modes, prefill) = stream.snapshot_then_enable(pane, deadline).await?;
        Ok((modes, prefill, stream))
    }
}

/// A control-mode client streaming one session's pane output.
///
/// It starts with output disabled and is only returned after replay
/// capture and the live cutover have completed. The client counts as
/// attached in tmux's eyes but never declares a size (no
/// `refresh-client -C`), so tmux ignores it for sizing entirely —
/// geometry comes only from explicit `resize-window` calls. Dropping it
/// kills the client process (`kill_on_drop`), which detaches it; the tmux
/// server and pane are unaffected.
pub struct OutputStream {
    child: Child,
    /// Held open for the lifetime of the client. It also carries the
    /// single command group that performs replay cutover.
    stdin: ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
    /// Stateful because tmux may split one passthrough wrapper across
    /// several `%output` notifications.
    passthrough: PassthroughDecoder,
}

impl OutputStream {
    /// Capture modes and content, then enable live output at the final
    /// command block boundary.
    ///
    /// The four expected blocks are deliberately explicit. Treating the
    /// group as one opaque reply makes it too easy to enable output after
    /// only the first `%end`, which reintroduces the replay/live overlap
    /// this method exists to remove.
    async fn snapshot_then_enable(
        &mut self,
        pane: &str,
        deadline: tokio::time::Instant,
    ) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        let history = format!("-{HISTORY_LIMIT}");
        let command = format!(
            "display-message -p -t {pane} '{PANE_MODE_FORMAT}' ; \
             capture-pane -p -e -N -t {pane} -S {history} ; \
             capture-pane -p -e -N -t {pane} ; \
             refresh-client -f !no-output\n"
        );
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        tokio::time::timeout(remaining, async {
            self.stdin
                .write_all(command.as_bytes())
                .await
                .context("writing tmux replay cutover commands")?;
            self.stdin
                .flush()
                .await
                .context("flushing tmux replay cutover commands")
        })
        .await
        .context("timed out writing tmux replay cutover commands")??;

        let modes_output =
            read_command_block(&mut self.reader, &mut self.line, deadline, "pane modes").await?;
        let history_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "history snapshot",
        )
        .await?;
        let visible_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "visible snapshot",
        )
        .await?;
        let _cutover = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "live-output cutover",
        )
        .await?;

        let modes_output = strip_command_output_terminator(&modes_output);
        let modes_text = String::from_utf8_lossy(modes_output);
        warn_once_about_missing_bracket_paste(&modes_text);
        let modes = PaneModes::parse(&modes_text);
        let snapshot = if modes.alternate_on {
            visible_output
        } else {
            history_output
        };
        let snapshot = strip_command_output_terminator(&snapshot);
        Ok((modes, normalize_capture(snapshot)))
    }

    /// Next chunk of pane output bytes, or None when the client exits
    /// (session killed, server gone). Non-%output notifications are
    /// consumed and ignored — command replies, layout changes, and
    /// `%exit` chatter are not terminal content.
    ///
    /// Reads BYTES, never lines-as-`String`: tmux's control-mode escaping
    /// only octal-escapes bytes below 0x20 (and backslash), so anything
    /// ≥ 0x80 crosses this stream raw. Decoding as UTF-8 would fail on
    /// any pane emitting binary or non-UTF-8 output — `cat` of a binary
    /// file — and one such byte would kill the terminal for good.
    pub async fn next_output(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        loop {
            self.line.clear();
            let n = read_control_line(&mut self.reader, &mut self.line).await?;
            if n == 0 {
                return Ok(None);
            }
            let line = strip_line_ending(&self.line);
            if let Some(rest) = line.strip_prefix(b"%output ") {
                // Format: "%<pane-id> <escaped-data>"
                if let Some(sep) = rest.iter().position(|&b| b == b' ') {
                    let bytes = unescape_control_output(&rest[sep + 1..]);
                    let bytes = self.passthrough.push(&bytes);
                    if !bytes.is_empty() {
                        return Ok(Some(bytes));
                    }
                }
            } else if line.starts_with(b"%exit") {
                return Ok(None);
            }
        }
    }

    /// Kill the control-mode client and wait for it to actually be gone.
    ///
    /// The orderly teardown for a forwarder that ran to completion. A
    /// forwarder cancelled mid-flight never reaches this — the task stops
    /// at an await point and `kill_on_drop` does the killing instead,
    /// which is why the takeover path aborts *and then awaits* the
    /// forwarder rather than just aborting it. Either way the client must
    /// be dead before another attaches: overlapping control clients
    /// reproducibly froze the newcomer's stream after the replay. The
    /// mechanism was never pinned down (in isolation two attached control
    /// clients DO both receive `%output` — audited), so treat the
    /// ordering, not any particular explanation, as the invariant; the
    /// attach handler's open-stream comment tells the same story.
    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
    }
}

/// The identity tmux repeats on one command reply's begin/end markers.
///
/// Pane content is allowed to start with `%`, including text that looks
/// like some other command's marker. Only an end marker with this exact
/// identity closes the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ControlBlockId {
    timestamp: u64,
    command: u64,
    flags: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlMarker {
    Begin(ControlBlockId),
    End(ControlBlockId),
    Error(ControlBlockId),
}

/// Parse only the three numeric marker forms tmux itself emits.
///
/// A line merely beginning with `%begin`, `%end`, or `%error` is not
/// enough: capture-pane output is unescaped, so terminal content may use
/// those words. Requiring the complete numeric shape keeps such content
/// inside the snapshot.
fn parse_control_marker(line: &[u8]) -> Option<ControlMarker> {
    let mut fields = line.split(|byte| *byte == b' ');
    let kind = fields.next()?;
    let parse_number = |field: &[u8]| {
        (!field.is_empty() && field.iter().all(u8::is_ascii_digit))
            .then(|| std::str::from_utf8(field).ok()?.parse::<u64>().ok())
            .flatten()
    };
    let id = ControlBlockId {
        timestamp: parse_number(fields.next()?)?,
        command: parse_number(fields.next()?)?,
        flags: parse_number(fields.next()?)?,
    };
    if fields.next().is_some() {
        return None;
    }
    match kind {
        b"%begin" => Some(ControlMarker::Begin(id)),
        b"%end" => Some(ControlMarker::End(id)),
        b"%error" => Some(ControlMarker::Error(id)),
        _ => None,
    }
}

/// Read one complete command reply without consuming the notification
/// after its closing marker.
///
/// tmux writes command output as raw lines between `%begin` and the
/// matching `%end`/`%error`. Mismatched marker-shaped lines remain
/// content; commands in this client are serialized, so another real
/// reply cannot nest here. EOF and timeout are hard errors because
/// accepting a partial snapshot would manufacture terminal history.
async fn read_command_block<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    line: &mut Vec<u8>,
    deadline: tokio::time::Instant,
    purpose: &str,
) -> anyhow::Result<Vec<u8>> {
    let id = loop {
        line.clear();
        let read = read_control_line_before(reader, line, deadline, purpose).await?;
        if read == 0 {
            bail!("tmux control client exited before the {purpose} reply began");
        }
        let stripped = strip_line_ending(line);
        match parse_control_marker(stripped) {
            Some(ControlMarker::Begin(id)) => break id,
            Some(ControlMarker::End(_) | ControlMarker::Error(_)) => {
                bail!("tmux control protocol ended a block before beginning the {purpose} reply");
            }
            None if stripped.starts_with(b"%output ") => {
                bail!("tmux emitted live output before the replay cutover completed");
            }
            None if stripped.starts_with(b"%exit") => {
                bail!("tmux control client exited before the {purpose} reply");
            }
            None => {}
        }
    };

    let mut output = Vec::new();
    loop {
        line.clear();
        let read = read_control_line_before(reader, line, deadline, purpose).await?;
        if read == 0 {
            bail!("tmux control client exited inside the {purpose} reply");
        }
        let stripped = strip_line_ending(line);
        match parse_control_marker(stripped) {
            Some(ControlMarker::End(end)) if end == id => return Ok(output),
            Some(ControlMarker::Error(end)) if end == id => {
                let reason = String::from_utf8_lossy(strip_command_output_terminator(&output));
                bail!("tmux {purpose} command failed: {}", reason.trim());
            }
            _ => output.extend_from_slice(line),
        }
    }
}

/// Read one control line before the shared exchange deadline.
async fn read_control_line_before<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut Vec<u8>,
    deadline: tokio::time::Instant,
    purpose: &str,
) -> anyhow::Result<usize> {
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    tokio::time::timeout(remaining, read_control_line(reader, out))
        .await
        .with_context(|| format!("timed out waiting for {purpose}"))?
        .with_context(|| format!("reading tmux control protocol during {purpose}"))
}

/// Read one tmux control-mode notification without permitting an
/// unterminated line to grow memory without bound.
async fn read_control_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut Vec<u8>,
) -> std::io::Result<usize> {
    read_control_line_with_limit(reader, out, MAX_CONTROL_LINE).await
}

/// Limit-parameterized core for small boundary tests; production always
/// uses [`MAX_CONTROL_LINE`].
async fn read_control_line_with_limit<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut Vec<u8>,
    limit: usize,
) -> std::io::Result<usize> {
    let start = out.len();
    loop {
        let (take, complete) = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                let partial = out.len() - start;
                if partial == 0 {
                    return Ok(0);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("tmux control-mode stream ended with a {partial}-byte partial line"),
                ));
            }
            let take = available
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if out.len() + take > limit {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("tmux control-mode line exceeds {limit} bytes"),
                ));
            }
            out.extend_from_slice(&available[..take]);
            (take, available[take - 1] == b'\n')
        };
        reader.consume(take);
        if complete {
            return Ok(out.len() - start);
        }
    }
}

/// Warn once per process if this tmux cannot report bracketed paste.
///
/// Checked here rather than at startup because it needs a real pane:
/// with no target every format expands empty, so a startup probe cannot
/// tell "tmux is too old" from "there is nothing to inspect yet" without
/// creating a throwaway session just to interrogate it.
fn warn_once_about_missing_bracket_paste(line: &str) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if bracket_paste_flag_is_missing(line)
        && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        tracing::warn!(
            "this tmux lacks bracket_paste_flag (added in tmux 3.7): bracketed paste will not \
             be restored when reattaching to a session. Everything else works."
        );
    }
}

/// Whether a pane-mode expansion shows a tmux without
/// `bracket_paste_flag`.
///
/// The distinction is drawn from the expansion itself: `alternate_on`
/// (the first field) exists on every supported tmux, so a populated
/// first field with an empty second means this tmux genuinely lacks
/// `bracket_paste_flag` (pre-3.7) — while an all-empty expansion means
/// there was no pane to inspect, which must NOT warn. A predecessor of
/// this check got that inverted (lore/: warned on every healthy start,
/// silent on old tmux), which is why the predicate is split out and
/// unit-tested separately from the once-per-process latch.
fn bracket_paste_flag_is_missing(line: &str) -> bool {
    let mut fields = line.split(',');
    let alternate = fields.next().unwrap_or("");
    let bracket = fields.next().unwrap_or("");
    !alternate.is_empty() && bracket.is_empty()
}

/// Turn raw `capture-pane` output into replayable terminal content.
///
/// Two transforms, both load-bearing (a past live bug — see lore/):
/// capture-pane emits LF line endings, and a terminal needs CRLF or
/// every line inherits the previous line's column; and the trailing
/// newline must go, because capture-pane terminates the LAST row too —
/// replaying it verbatim scrolls the screen one row past the content,
/// landing the restored cursor a row low on the normal screen and
/// destroying the app's top row on the alternate screen (which has no
/// scrollback to absorb the scroll).
fn normalize_capture(out: &[u8]) -> Vec<u8> {
    let body = out.strip_suffix(b"\n").unwrap_or(out);
    let mut normalized = Vec::with_capacity(body.len() + body.len() / 32);
    for &b in body {
        if b == b'\n' {
            normalized.push(b'\r');
        }
        normalized.push(b);
    }
    normalized
}

/// Incrementally remove tmux passthrough wrappers before bytes reach the
/// real terminal.
///
/// tmux may split `ESC P tmux; ... ESC \` across `%output`
/// notifications, including inside the opener or on either side of an
/// escaped `ESC`. Keeping only the parser's few bytes of boundary state
/// avoids both split-wrapper corruption and an unbounded whole-wrapper
/// buffer for large inline images.
#[derive(Default)]
struct PassthroughDecoder {
    opener: Vec<u8>,
    in_wrapper: bool,
    pending_escape: bool,
}

impl PassthroughDecoder {
    const OPEN: &'static [u8] = b"\x1bPtmux;";

    /// Decode one arbitrary output chunk. Empty output means the chunk
    /// ended inside an opener or immediately after an escaped byte; the
    /// next call resumes from that exact state.
    fn push(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            if self.in_wrapper {
                if self.pending_escape {
                    self.pending_escape = false;
                    match byte {
                        // Payload ESC bytes are doubled inside a wrapper.
                        0x1b => out.push(0x1b),
                        // A single ESC + backslash closes the wrapper.
                        b'\\' => self.in_wrapper = false,
                        // Malformed but lossless: preserve a lone ESC and
                        // its follower instead of inventing a sequence.
                        other => {
                            out.push(0x1b);
                            out.push(other);
                        }
                    }
                } else if byte == 0x1b {
                    self.pending_escape = true;
                } else {
                    out.push(byte);
                }
                continue;
            }

            self.opener.push(byte);
            while !Self::OPEN.starts_with(&self.opener) {
                out.push(self.opener.remove(0));
            }
            if self.opener == Self::OPEN {
                self.opener.clear();
                self.in_wrapper = true;
            }
        }
        out
    }
}

/// One-shot test oracle for complete passthrough sequences. Production
/// streaming keeps a [`PassthroughDecoder`] across notifications.
#[cfg(test)]
fn unwrap_passthrough(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = PassthroughDecoder::default();
    let mut out = decoder.push(bytes);
    if decoder.in_wrapper {
        // A one-shot caller cannot wait for a split wrapper. Preserve it
        // byte-for-byte; the streaming path never needs this fallback.
        return bytes.to_vec();
    }
    out.append(&mut decoder.opener);
    out
}

/// Strip the terminator from a control-mode notification line.
///
/// Every marker check in this module compares against the line *without*
/// its ending, because the bounded line reader leaves it attached and
/// tmux is not consistent about whether a `\r` precedes it. Applies only to
/// tmux's own notification lines — pane output is escaped payload inside
/// `%output` and must never be touched.
fn strip_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Remove the newline control mode adds after one command's stdout.
///
/// This is separate from [`normalize_capture`]: capture-pane already
/// terminates its final row, so a command block contains two trailing
/// newlines. One belongs to control mode and is removed here; the other
/// belongs to capture-pane and is removed during terminal normalization.
fn strip_command_output_terminator(output: &[u8]) -> &[u8] {
    let output = output.strip_suffix(b"\n").unwrap_or(output);
    output.strip_suffix(b"\r").unwrap_or(output)
}

/// Undo control-mode escaping.
///
/// tmux octal-escapes bytes below 0x20 *and* backslash itself — a literal
/// backslash arrives as `\134` (verified against tmux 3.7). Everything
/// else, including every byte ≥ 0x80, passes through verbatim, which is
/// why this works on bytes rather than `str`.
pub fn unescape_control_output(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && matches!(b[i + 1], b'0'..=b'3')
            && matches!(b[i + 2], b'0'..=b'7')
            && matches!(b[i + 3], b'0'..=b'7')
        {
            let value = (b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0');
            out.push(value);
            i += 4;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The octal unescaping is the one lossy-looking transform in the
    /// output path; pin it against real control-mode escaping shapes.
    #[test]
    fn unescape_handles_octal_sequences() {
        assert_eq!(unescape_control_output(b"plain"), b"plain");
        assert_eq!(
            unescape_control_output(br"\033[1mhi\033[0m"),
            b"\x1b[1mhi\x1b[0m"
        );
        assert_eq!(unescape_control_output(br"bell\007"), b"bell\x07");
        // Invalid byte escapes stay literal rather than wrapping.
        assert_eq!(unescape_control_output(br"x\477"), br"x\477");
        // Trailing lone backslash must not panic or eat bytes.
        assert_eq!(unescape_control_output(br"x\"), b"x\\");
    }

    /// The alternate-screen switch must be in the PRE-content half and
    /// nowhere else: it clears the buffer it switches to, so emitting it
    /// after the replay would erase the replay. Cursor placement belongs
    /// after the content, because writing content moves the cursor.
    #[test]
    fn alt_screen_switch_precedes_content_and_cursor_follows_it() {
        let modes = PaneModes {
            alternate_on: true,
            bracket_paste: true,
            cursor_visible: false,
            cursor_x: 4,
            cursor_y: 2,
            ..Default::default()
        };
        assert_eq!(modes.pre_content_sequences(), "\x1b[?1049h");

        let post = modes.post_content_sequences();
        assert!(!post.contains("\x1b[?1049h"));
        assert!(post.contains("\x1b[?2004h"));
        let pos = post.find("\x1b[3;5H").expect("1-based cursor position");
        let hide = post.find("\x1b[?25l").expect("hidden cursor stays hidden");
        assert!(pos < hide, "visibility must come after positioning");
    }

    /// Every tmux mode field maps to a distinct terminal escape. A
    /// swapped field still produces plausible output, so each branch
    /// needs an independent oracle rather than one all-flags snapshot.
    #[test]
    fn pane_modes_restore_each_mouse_and_cursor_mode() {
        let cases = [
            ("0,0,0,0,1,0,1,0,0,0", "\x1b[?1000h"),
            ("0,0,0,1,0,0,1,0,0,0", "\x1b[?1002h"),
            ("0,0,1,0,0,0,1,0,0,0", "\x1b[?1003h"),
            ("0,0,0,0,0,1,1,0,0,0", "\x1b[?1006h"),
            ("0,0,0,0,0,0,1,1,0,0", "\x1b[?1h"),
        ];
        for (fields, expected) in cases {
            let output = PaneModes::parse(fields).post_content_sequences();
            assert!(
                output.contains(expected),
                "{fields} did not restore {expected:?}: {output:?}"
            );
        }
    }

    /// A pane on the normal screen must not emit the alt-screen switch —
    /// doing so would blank a perfectly good replay.
    #[test]
    fn normal_screen_emits_no_alt_switch() {
        let modes = PaneModes {
            cursor_visible: true,
            ..Default::default()
        };
        assert_eq!(modes.pre_content_sequences(), "");
    }

    /// Field parsing must survive a tmux build that does not know one of
    /// the format names. With whitespace splitting, the empty expansion
    /// collapsed and every later field shifted left — restoring wrong
    /// modes and misplacing the cursor. Comma-delimited fields degrade
    /// one position at a time, which is the documented contract.
    #[test]
    fn unknown_format_degrades_only_its_own_field() {
        // bracket_paste (field 2) unknown; everything after it must keep
        // its own value.
        let modes = PaneModes::parse("1,,0,0,0,1,1,0,7,3");
        assert!(modes.alternate_on);
        assert!(!modes.bracket_paste, "unknown field reads as off");
        assert!(modes.mouse_sgr, "field after the gap keeps its own value");
        assert!(modes.cursor_visible);
        assert_eq!((modes.cursor_x, modes.cursor_y), (7, 3));
    }

    /// Passthrough payloads must survive the trip to xterm.js. tmux
    /// hands the wrapper through control mode intact (audited), so
    /// without unwrapping the terminal treats it as an unknown DCS and
    /// drops the contents — losing OSC 52 clipboard writes and inline
    /// images from any agent that uses them.
    #[test]
    fn passthrough_wrappers_are_unwrapped_with_esc_undoubled() {
        // ESC P tmux; <ESC ESC ]52;c;aGk= BEL> ESC backslash
        let wrapped = b"before\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\after";
        assert_eq!(
            unwrap_passthrough(wrapped),
            b"before\x1b]52;c;aGk=\x07after".to_vec()
        );
        // A payload that itself ends in ST (doubled to ESC ESC \ on the
        // wire) must not have the doubled pair mistaken for the wrapper's
        // terminator — that truncated the payload one byte early and
        // leaked the real close as garbage. This is the common case for
        // ST-terminated OSC and any DCS payload (sixel).
        let st_payload = b"\x1bPtmux;\x1b\x1b]52;c;aGk=\x1b\x1b\\\x1b\\tail";
        assert_eq!(
            unwrap_passthrough(st_payload),
            b"\x1b]52;c;aGk=\x1b\\tail".to_vec()
        );
        // Ordinary output is returned byte-for-byte.
        assert_eq!(
            unwrap_passthrough(b"plain\x1b[1m"),
            b"plain\x1b[1m".to_vec()
        );
        // A wrapper split across notifications must not be swallowed.
        let partial = b"\x1bPtmux;\x1b\x1b]52;c;";
        assert_eq!(unwrap_passthrough(partial), partial.to_vec());
    }

    /// `%output` boundaries are unrelated to terminal escape-sequence
    /// boundaries. Every possible two-chunk split of the wrapper must
    /// decode identically to the one-shot form, including splits inside
    /// the opener, doubled ESC, and closing ST.
    #[test]
    fn passthrough_decoder_survives_every_notification_split() {
        let wrapped = b"before\x1bPtmux;\x1b\x1b]52;c;aGk=\x1b\x1b\\\x1b\\after";
        let expected = b"before\x1b]52;c;aGk=\x1b\\after";
        for split in 0..=wrapped.len() {
            let mut decoder = PassthroughDecoder::default();
            let mut actual = decoder.push(&wrapped[..split]);
            actual.extend(decoder.push(&wrapped[split..]));
            assert!(!decoder.in_wrapper, "wrapper left open at split {split}");
            assert!(
                !decoder.pending_escape,
                "escape left pending at split {split}"
            );
            actual.extend(&decoder.opener);
            assert_eq!(actual, expected, "failed at split {split}");
        }
    }

    /// The supported-version check accepts patch suffixes but rejects the
    /// control-mode versions below the documented 3.3 floor.
    #[test]
    fn tmux_version_floor_is_enforced() {
        assert!(require_supported_tmux("tmux 3.3").is_ok());
        assert!(require_supported_tmux("tmux 3.3a").is_ok());
        assert!(require_supported_tmux("tmux 3.10").is_ok());
        assert!(require_supported_tmux("tmux 4.0").is_ok());
        assert!(require_supported_tmux("tmux 3.2a").is_err());
        assert!(require_supported_tmux("not-a-version").is_err());
    }

    /// An unterminated control notification must fail at the configured
    /// boundary rather than letting `read_until` grow a process-sized
    /// allocation.
    #[tokio::test]
    async fn control_mode_lines_are_bounded() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(32);
        writer.write_all(b"12345").await.unwrap();
        writer.shutdown().await.unwrap();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();

        let error = read_control_line_with_limit(&mut reader, &mut line, 4)
            .await
            .expect_err("line beyond the cap must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    /// EOF cannot turn a truncated notification into terminal data. Tmux
    /// control records are newline-delimited, so a partial final record
    /// means the control client died mid-write.
    #[tokio::test]
    async fn control_mode_partial_line_at_eof_is_an_error() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(32);
        writer.write_all(b"%output %0 partial").await.unwrap();
        writer.shutdown().await.unwrap();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();

        let error = read_control_line_with_limit(&mut reader, &mut line, 64)
            .await
            .expect_err("partial notification must not be accepted at EOF");
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    /// Snapshot content is raw command output, so lines beginning with
    /// `%` are ordinary content unless they exactly close this block.
    /// A loose prefix parser would truncate a pane displaying tmux
    /// protocol examples or diagnostics.
    #[tokio::test]
    async fn command_block_requires_its_exact_end_marker() {
        let input = b"%session-changed $0 session\n\
                      %begin 10 20 1\n\
                      ordinary\n\
                      %end 10 999 1\n\
                      %error not-a-marker\n\
                      %end 10 20 1\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let output = read_command_block(
            &mut reader,
            &mut line,
            tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
            "test snapshot",
        )
        .await
        .expect("complete block");
        assert_eq!(output, b"ordinary\n%end 10 999 1\n%error not-a-marker\n");
    }

    /// `%error` closes the matching command block and must retain tmux's
    /// plain-text diagnostic. Otherwise an attach failure becomes an
    /// unexplained protocol error at the service boundary.
    #[tokio::test]
    async fn command_block_reports_tmux_error_text() {
        let input = b"%begin 10 20 1\ncan't find pane: %9\n%error 10 20 1\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let error = read_command_block(
            &mut reader,
            &mut line,
            tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
            "history snapshot",
        )
        .await
        .expect_err("tmux error must fail the block");
        assert!(
            format!("{error:#}").contains("can't find pane: %9"),
            "tmux diagnostic was lost: {error:#}"
        );
    }

    /// EOF inside a block cannot turn a truncated capture into valid
    /// replay. The outer line reader also rejects a partial final line;
    /// this pins the distinct case where the last content line was
    /// complete but the closing marker never arrived.
    #[tokio::test]
    async fn command_block_rejects_eof_before_its_end_marker() {
        let input = b"%begin 10 20 1\ncomplete content line\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let error = read_command_block(
            &mut reader,
            &mut line,
            tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
            "visible snapshot",
        )
        .await
        .expect_err("unterminated block must fail");
        assert!(
            format!("{error:#}").contains("inside the visible snapshot reply"),
            "unexpected error: {error:#}"
        );
    }

    /// Reading the final refresh reply must stop at its `%end` and leave
    /// the first live notification buffered for `next_output`, even when
    /// the underlying read splits that notification. This boundary is
    /// the whole no-gap handoff contract.
    #[tokio::test]
    async fn final_cutover_block_leaves_live_output_unconsumed() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(1024);
        writer
            .write_all(
                b"%begin 10 20 1\nmodes\n%end 10 20 1\n\
                  %begin 10 21 1\nhistory\n%end 10 21 1\n\
                  %begin 10 22 1\nvisible\n%end 10 22 1\n\
                  %begin 10 23 1\n%end 10 23 1\n\
                  %output %0 live\\015",
            )
            .await
            .unwrap();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
        for purpose in ["modes", "history", "visible", "cutover"] {
            read_command_block(&mut reader, &mut line, deadline, purpose)
                .await
                .expect("complete block");
        }
        writer.write_all(b"\\012\n").await.unwrap();
        line.clear();
        read_control_line(&mut reader, &mut line)
            .await
            .expect("read first live notification");
        assert_eq!(line, b"%output %0 live\\015\\012\n");
    }

    /// Control mode adds one newline around command output. Removing it
    /// before capture normalization preserves the capture's own final
    /// terminator, which normalization must remove separately.
    #[test]
    fn command_output_and_capture_terminators_are_distinct() {
        let block = b"row one\nrow two\n\n";
        assert_eq!(
            normalize_capture(strip_command_output_terminator(block)),
            b"row one\r\nrow two"
        );
    }

    /// Pin every generated option and its table, while excluding the
    /// `window-size` setting that crashes tmux 3.4.
    ///
    /// Exact lines are the contract here: moving an option to a different
    /// tmux table can silently disable it while leaving a substring test
    /// green.
    #[test]
    fn generated_config_pins_every_load_bearing_option() {
        let cfg = TmuxDriver::config_body();
        assert_eq!(
            cfg,
            format!(
                "set -s exit-empty off\n\
                 set -s escape-time 0\n\
                 set -s default-terminal 'xterm-256color'\n\
                 set -g status off\n\
                 set -g prefix None\n\
                 set -g history-limit {HISTORY_LIMIT}\n\
                 setw -g remain-on-exit on\n"
            )
        );
    }

    /// The warn-on-old-tmux predicate has three shapes and got one of
    /// them wrong once before (lore/: a capability probe warned on every
    /// healthy tmux and stayed silent on genuinely old ones — precisely
    /// inverted). Pin all three: modern tmux quiet, old tmux warns,
    /// no-pane expansion quiet.
    #[test]
    fn bracket_paste_warning_fires_only_for_genuinely_old_tmux() {
        // Modern tmux: both fields populated.
        assert!(!bracket_paste_flag_is_missing("0,1,0,0,0,0,1,0,0,0"));
        assert!(!bracket_paste_flag_is_missing("1,0,0,0,0,0,1,0,3,7"));
        // Pre-3.7 tmux: alternate_on expands, bracket_paste_flag does not.
        assert!(bracket_paste_flag_is_missing("0,,0,0,0,0,1,0,0,0"));
        // No pane to inspect: everything empty — must NOT read as "old".
        assert!(!bracket_paste_flag_is_missing(""));
        assert!(!bracket_paste_flag_is_missing(",,,,,,,,,"));
    }

    /// The replay-content normalization was a live bug (lore/: replay
    /// scrolled one row past the content, destroying the alt-screen top
    /// row), so its two transforms are pinned: LF→CRLF, and the trailing
    /// terminator dropped.
    #[test]
    fn capture_normalization_converts_line_endings_and_drops_the_last() {
        assert_eq!(normalize_capture(b"a\nb\n"), b"a\r\nb");
        assert_eq!(normalize_capture(b"a\nb"), b"a\r\nb");
        assert_eq!(normalize_capture(b"\n"), b"");
        assert_eq!(normalize_capture(b""), b"");
        // Escape sequences and blank interior lines pass through.
        assert_eq!(
            normalize_capture(b"\x1b[1mx\x1b[0m\n\ny\n"),
            b"\x1b[1mx\x1b[0m\r\n\r\ny"
        );
        // Non-UTF-8 pane content must survive byte-for-byte: replay goes
        // through here, and lossy decoding would render a `cat` of a
        // binary file differently after a reattach than it did live.
        assert_eq!(
            normalize_capture(&[0xff, 0xfe, b'\n', 0x80, b'\n']),
            vec![0xff, 0xfe, b'\r', b'\n', 0x80]
        );
    }

    /// A short or empty expansion must not panic or produce garbage.
    #[test]
    fn truncated_format_output_uses_defaults() {
        let modes = PaneModes::parse("");
        assert_eq!(modes, PaneModes::parse("0"));
        // Cursor visibility defaults on: hiding a cursor we know nothing
        // about would be a visible regression on every reattach.
        assert!(PaneModes::parse("").cursor_visible);
    }
}
