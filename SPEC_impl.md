# Farhelm implementation specification

NOTE: This documents implementation _choices_ in service of SPEC.md, together with the motivation for each choice so
future changes are made with the original reasoning in hand. It is not a build plan — sequencing, milestones, and PR
breakdown live elsewhere. SPEC.md defines what the product does; when this document and SPEC.md disagree about
observable behavior, SPEC.md wins.

Standing rule: the user-facing CLI surface described here (command names, subcommands, flags a user actually types) must
be kept in sync with SPEC.md wherever SPEC.md references it. Changing a command here means updating SPEC.md's mentions
of it in the same change.

## Language and runtime

Rust throughout, async on tokio, one cargo workspace. The only sanctioned non-Rust runtime code is the xterm.js terminal
island and the thin JS interop around it (see GUI), plus Playwright test code in TypeScript.

Motivation: single language across supervisor, helm, CLI, and UI maximizes shared types and lets one test suite exercise
real components. tokio because the chosen web stack (axum, tungstenite) lives there; no exotic async needs exist that
would justify anything else.

## Workspace layout

- `crates/farhelm` — the single multi-call binary (see CLI).
- `crates/farhelm-supervisor` — session management, tmux driver, agent-kind integrations, SQLite state.
- `crates/farhelm-helm` — the host registry (SPEC.md's term: the helm's record of registered hosts and their SSH
  destinations), SSH transport, aggregation, axum API, static UI serving.
- `crates/farhelm-proto` — wire types and protocol version, shared by both ends and by tests.
- `crates/farhelm-ui` — the Dioxus application, built for web (wasm32) and desktop from the same crate.

Motivation: the proto crate is the seam that keeps helm and supervisor honestly decoupled (they meet only over the wire,
even in-process), and a single binary crate keeps provisioning to one artifact.

## GUI: Dioxus

Dioxus, version pinned at the workspace level, rendering the same component tree in two targets: web (wasm32, real DOM,
served by the helm) and desktop (wry webview wrapping the identical DOM). No dioxus-fullstack / server functions — the
UI is a pure client of the helm's HTTP/WS API.

Motivation: the project's standing constraints — recorded here, because SPEC.md deliberately stays
implementation-neutral and does not contain them: as much Rust as possible, one implementation for web and native, and a
GUI that agents can test visually without a human in the loop. Those force a DOM-based Rust framework. Canvas-rendering
toolkits (egui, Iced, Slint) fail the testing constraint: Playwright against a canvas is blind screenshot-diffing with
no semantic selectors. Among DOM-based Rust options, Dioxus is the most active and has a first-party desktop story;
Tauri+Leptos would mean gluing two frameworks for no clear gain. Skipping dioxus-fullstack keeps the API a first-class
tested surface (the spawn CLI and test fixtures need it anyway) and avoids the framework's most churn-prone part.

Known risks, accepted deliberately:

- API churn between Dioxus 0.x releases. Mitigation: pin, avoid internals, budget for migrations.
- Desktop is WKWebView on macOS while tests drive Chromium (no usable WebDriver exists for WKWebView on macOS).
  Mitigation: the tested surface is the web build; desktop-only glue is kept as small as possible and is the one
  manually-verified path.
- Clipboard and drag-drop are where WKWebView diverges from Chromium, and paste interception is a headline feature. The
  default is the same DOM paste/drop event path on both targets — WebKit does deliver file/image data on those events,
  but with documented engine-specific restrictions (pasted-HTML sanitization, gesture gating on the async clipboard
  API), so the honest framing is "same event model, engine differences expected", not "same as Chromium". No special
  desktop solution is built until a real deficiency shows up in our actual flows; native-side wry hooks are the known
  fallback. One concrete thing to check early rather than debug late: wry's own file-drop handling swallows DOM drop
  events unless configured not to. (The browser path is a secure context on loopback, so web clipboard APIs are fully
  available there.) Also established the hard way during M2 dogfooding: wry implements NO native JS dialogs on macOS —
  `window.confirm()` silently does nothing — so any confirmation or prompt the UI needs must be in-page DOM, never a
  browser dialog. SPEC.md's confirmation language is deliberately mechanism-agnostic; this is the constraint that picks
  the mechanism.
- Blitz (Dioxus's native renderer) is not production-ready. The plan assumes webview desktop indefinitely; nothing may
  depend on Blitz landing.

## Terminal widget: xterm.js island

The terminal is xterm.js, vendored as a static asset (no CDN — the UI must be fully self-contained, consistent with
SPEC.md's no-public-relay, no-third-party-services posture and the loopback deployment), mounted as a JS island inside
the Dioxus tree. PTY bytes flow WebSocket → `term.write()` directly, bypassing Dioxus state entirely. Dioxus owns
everything around the terminal (tabs, status, dialogs), not the terminal's content path.

Motivation: xterm.js is the only battle-tested embeddable terminal (VS Code) and full escape-sequence fidelity is a
SPEC.md requirement. Routing high-frequency PTY output through a reactive framework would be a performance disaster, so
the bypass is load-bearing, not an optimization. A pure-Rust wasm terminal (alacritty_terminal grid + canvas renderer)
was rejected: it is a project in itself and reintroduces the untestable-canvas problem inside the most important widget.

The bypass alone is not sufficient (audited): `term.write()` is non-blocking with a hard ~50MB buffer that silently
discards beyond the cap, and xterm.js parses at roughly 5–35 MB/s while a PTY can produce far faster. The terminal path
therefore carries end-to-end backpressure — write-completion callbacks drive watermark pause/resume messages over the
WebSocket, and the supervisor throttles its pane reads accordingly. Interactive agent output never approaches these
rates; `cat` of a huge file must degrade to slow, never to silent data loss. Precisely (sharpened while planning M2.5,
when the original sentence met tmux's actual flow-control mechanics): no code Farhelm owns may ever drop a terminal byte
— every Farhelm-side bound is backpressure or a visible detach, never discard.

What "degrade to slow" is allowed to slow includes, on one of the two tmux behaviors below, the AGENT's own writes for
the duration of a viewer's pause. SPEC.md's stall bullet now states that bounded-slowdown contract directly, and this
document's job is only to record the mechanism: nothing here throttles the agent deliberately, and the block is bounded
by the flow-control window and ultimately by the stall detach.

The producer-side bound is tmux's, and tmux implements it in one of two ways. With `pause-after` set on the supervisor's
control client, a client that stops reading gets EITHER of these (audited 2026-07-29 on 3.3a, 3.4, and 3.7b, both with a
standalone control client and through the full supervisor stack):

- **tmux throttles the pane.** It stops reading the PTY, the agent's own `write` blocks, and nothing is queued or
  dropped. On resume, delivery continues from exactly where it stopped — a genuine end-to-end degrade-to-slow, with no
  recovery needed.
- **tmux reads ahead into history and pauses the client's stream.** The agent free-runs into scrollback (tmux server RSS
  stays flat), the bytes queued for the stalled client age past `pause-after`, and tmux then cuts that client's stream
  with `%pause` and discards what it had queued for it. Recovery is replay from retained history, exactly like a
  reattach.

Which one happens is NOT a property of the tmux version — an earlier draft of this paragraph claimed it was, and the
audit does not support that. All three versions were observed taking both paths across repeated identical trials; the
deciding factor is how far tmux happens to have read ahead of the client at the moment it stalls, which in turn depends
on how fast that client was consuming beforehand. Both paths satisfy the contract, so nothing above this layer may
depend on which one occurs, and the supervisor implements both (it honors `%pause` whenever it arrives and simply keeps
reading when it does not).

The xterm.js scrollback capacity is therefore sized to at most the tmux history floor (both currently 12,000 lines) — an
invariant tests must pin — which makes the replay-based catch-up's end state observably equivalent to lossless slow
delivery: every byte still within the terminal's own retention is present, and bytes beyond it would have been evicted
from scrollback even had they been delivered one at a time.

## Terminal substrate: private tmux server

Each supervisor runs a dedicated tmux server on a private socket (`~/.local/state/farhelm/tmux.sock`) with a locked-down
generated config: status bar off, `history-limit` sized to SPEC.md's replay floor, `remain-on-exit on`. One tmux session
per Farhelm session; window 0 is the agent terminal, additional windows are the terminal tabs. The user's own tmux usage
and config are untouched.

Farhelm requires tmux ≥ 3.3 (dependable control mode) to operate at all. One capability sits higher: restoring bracketed
paste on reattach reads the `bracket_paste_flag` format, which tmux only gained in 3.7 — below that the supervisor warns
once at first attach (a startup probe cannot tell "old tmux" from "no pane to inspect yet") and loses that one mode,
everything else working normally. A second, smaller accepted degradation below 3.4 (found during M2.5's 3.3a validation,
2026-07-29): `capture-pane -N` on 3.3a does not preserve trailing styled padding, so a stop snapshot's dead-pane frame
can lose the styling of trailing padding cells — cosmetic fidelity only, accepted rather than complicating the capture
path for the floor's last dot release; CI pins the full behavior on 3.4+. This matters in practice because Ubuntu 24.04
ships tmux 3.4. Releases bundle a private tmux build per platform, used whenever the host's tmux is missing or below the
floor — the Mac app bundles one too, since macOS ships no tmux at all. A host tmux at or above the floor is acceptable;
version is checked, not just presence.

tmux is a headless PTY holder and history store. The supervisor's only client is a non-rendering control-mode client
(`tmux -C`, the interface iTerm2's tmux integration is built on; `pipe-pane` is the fallback shape). Sizing (audited on
tmux 3.7): a control-mode client is an attached client, but tmux ignores it for window sizing until it declares a size
via `refresh-client -C` — the supervisor never declares one, so tmux ignores it for sizing entirely, and geometry comes
from explicit `resize-window` calls tracking the attached GUI client's dimensions (`resize-window` sets
`window-size manual` on the window it touches, which is where that setting comes from). NOTE: setting
`window-size manual` globally in the config crashes the tmux 3.4 server outright — the version Ubuntu 24.04 ships — so
it must stay out of the generated config; the two mechanisms above make it redundant anyway. The supervisor streams raw
pane output to the client; input goes in as `send-keys -t <pane> -H <hex bytes...>` commands written to the same
attached control-mode client's stdin that streams that attachment's output. An earlier design tried `load-buffer -` over
stdin followed by `paste-buffer -d -r` instead, specifically to keep input bytes off a process's argv (see below) — and
had to be abandoned: verified empirically against tmux 3.7b, `paste-buffer` caret-escapes control bytes on the way into
the pane (DEL arrives as the two literal characters `^?`, ESC as `^[`, ctrl-C as `^C`), silently breaking backspace,
arrow keys, and ctrl-C. Keystrokes are not pastes, and no `paste-buffer` flag changes that. `send-keys -H` delivers
bytes verbatim instead (also verified against 3.7b) and keeps the security property that motivated stdin delivery in the
first place: hex-encoded input never touches a process's argv, because it rides an _already-running_ process's stdin
rather than a freshly spawned `tmux send-keys` command's arguments — the earlier concern was a spawned process's argv
being world-readable via `/proc/<pid>/cmdline`, which matters because input includes credentials typed at agent prompts,
and that risk never applied to bytes written to a pipe. Each `send-keys` command is chunked at 256 bytes because tmux
rejects a command carrying on the order of ~1000 arguments as "command too long" and each input byte becomes one hex
argument; every command's `%begin`/`%end` reply rides back on the same stdout the client's other notifications use,
which is safe to ignore because the output-streaming loop already discards every notification it has no use for (see
below). Passthrough sequences (audited): the control-mode pane-output stream carries `\ePtmux;...\e\\`-wrapped payloads
still wrapped, regardless of the `allow-passthrough` option — that option only gates forwarding to rendering clients,
which Farhelm has none of — so the supervisor unwraps passthrough payloads itself before they reach xterm.js. Reconnect
replay prefills xterm.js from `capture-pane -e` history, then continues with live bytes from the same control client —
that is how the 10,000-line floor is met without a gap between the two. The handoff ordering is load-bearing: the
incumbent control client is killed and awaited, the window is resized, and the replacement attaches with `no-output`.
Pane modes, a history snapshot, a visible-screen snapshot, and a final `refresh-client -f
!no-output,pause-after=N` are
submitted as one semicolon-separated command group through that replacement. The matching `%end` for the final refresh
block is the cutover: earlier pane bytes are represented by the snapshot, later ones arrive as live output, and
`no-output` advances rather than queueing a second copy for delivery. Normal-screen replay selects the history snapshot;
alternate-screen replay selects the visible snapshot so normal history is not mixed into a full-screen app.

Setting `pause-after` on that same cutover (M2.5) changes the dialect the client then reads, which the parser must
handle rather than discard: pane bytes arrive as `%extended-output <pane-id> <age> ... : <data>` instead of `%output`,
and `%pause`/`%continue` notifications appear. Both output dialects are accepted unconditionally and decoded
identically, including across a switch mid-stream, because the passthrough decoder carries state between notifications.
`%pause` is acted on — it means tmux cut this client's stream, and the dropped bytes are recoverable only by replaying
history — while `%continue` is discarded like any other chatter, since it arrives inside the reply block of the command
that requested it and nothing waits on it. The extensible-argument rule matters too: everything between the age and a
lone `:` field is reserved for future tmux versions and is skipped by scanning for that separator field rather than by
counting fields, so a future argument cannot silently shift the payload.

This boundary was checked against tmux 3.3a, 3.4, and 3.7b with `scripts/check-tmux-cutover.py` under a continuously
busy pane. The corresponding tmux source has the same ordering in all three versions: one input line appends the
complete command group, synchronous `capture-pane` and `refresh-client` commands drain from that queue before the server
loop returns to pane reads, and `CLIENT_CONTROL_NOOUTPUT` advances the client offset instead of queueing a backlog. Each
command still produces its own `%begin`/`%end` block, so the parser keeps their numeric identities and does not declare
the stream live at an earlier block. A command `%error`, partial EOF, timeout, output before cutover, or missing
matching marker fails the attach rather than replaying an incomplete snapshot.

Content alone is not enough: pane modes (alternate screen, bracketed paste, mouse reporting, application cursor keys,
cursor position) are read from tmux pane format variables and re-synthesized into xterm.js after the prefill — without
that, a reattached full-screen agent silently loses paste bracketing and mouse reporting. Replay remains bounded by
tmux's retained history. One deeper tmux limitation also remains: `capture-pane` serializes rendered cells, not an
in-progress terminal escape parser. If reconnect lands after tmux has consumed only a prefix of one escape sequence, the
snapshot cannot serialize that hidden parser state for xterm.js; a later application repaint repairs the display.
Farhelm does preserve split printable output and keeps its own passthrough decoder across live pane-output notification
boundaries (resetting it only when a `%pause` catch-up abandons the stream it belonged to). The supervisor enforces
SPEC.md's one-attachment rule itself.

Exited-session semantics: `remain-on-exit on` keeps dead panes viewable per SPEC.md, and exit codes come from the dead
pane's status. Exec failure versus ran-and-died cannot be told apart by exit code alone (a missing command yields 127
and a non-executable file 126 — both indistinguishable from a program exiting with that code), so classification does
not rely on exit codes: the shell execs `farhelm internal launch`, a shim that always exists, which resolves and execs
the profile invocation and, on exec failure, writes a sentinel with the errno detail to a per-session status file before
exiting. The supervisor classifies **error** only on that sentinel. NOTE: a sentinel written by the shell after a failed
`exec` was audited and rejected — interactive bash survives a failed exec, but zsh terminates on it in every mode, so
shell-side code after `exec` never runs for zsh users; the shim works identically under any `$SHELL`.

Motivation for never rendering through a normal attach: a rendering tmux client takes over the outer terminal on the
alternate screen and draws everything itself, which kills native scrolling — xterm.js would accumulate no scrollback,
wheel scrolling would need tmux `mouse on` copy-mode with tmux-flavored selection UX, and the capture-pane prefill would
land in a buffer the alt screen makes unreachable. Streaming raw pane bytes instead lets xterm.js own scrollback,
selection, and search natively; inner alt-screen apps (vim in a tab) still render correctly because their escape
sequences pass through in the stream; and mouse-reporting apps still work because xterm.js's mouse sequences are
forwarded as input. tmux's own UI (status bar, prefix key, copy-mode) never appears anywhere.

Motivation: tmux delivers exactly the hard guarantees SPEC.md makes — processes and terminal state survive supervisor
restarts, scrollback retention, screen re-render on reattach — with a decade of hardening, and the approach is validated
by herdr. The rejected alternative (per-session Rust holder daemons owning PTYs plus our own terminal-grid engine for
replay) buys independence from tmux at the cost of owning a terminal state machine's bugs; not a v1 trade. The
supervisor's internal terminal interface stays narrow (create, attach-cutover, resize, input, kill) so a Rust holder
could replace tmux behind it later without touching anything above.

Consequence to keep in mind: tmux sits in the escape-sequence path. Fidelity issues (new terminal features, passthrough
sequences) get debugged at the tmux layer first; the generated config is the knob.

## Helm ↔ supervisor transport: system ssh + stdio protocol

The helm shells out to the user's `ssh` binary (tokio::process), one ControlMaster per host (`ControlPersist`) so
interactive latency stays low and reconnects are cheap. The supervisor is reached by executing `farhelm internal stdio`
on the remote side, which proxies stdio to the supervisor's unix socket. Supervisors listen on that unix socket only —
no network port, exactly as SPEC.md requires.

On top of that byte pipe: a multiplexed framing protocol — length-prefixed frames carrying a channel id and a type tag;
control messages are serde_json, terminal data channels are raw bytes. The same protocol runs over the unix socket
locally, so "local host" and "remote host" differ only in transport. Connection setup exchanges protocol and build
versions; it refuses protocol-version incompatibility per SPEC.md's version-skew rule, while build versions travel for
diagnostics only — mixed builds with a compatible protocol are the normal steady state SPEC.md describes.

Motivation: SPEC.md promises provisioning and transport ride "the user's keys, agent, and config" — only the real ssh
binary honors ~/.ssh/config fully (ProxyJump, Match blocks, agent forwarding, ControlMaster). russh was rejected for
exactly that: partial config support would quietly break the promise. JSON control frames keep the protocol debuggable
by eye; raw binary data channels keep PTY throughput off the JSON path.

## Supervisor internals

- State in SQLite (rusqlite) at `~/.local/state/farhelm/supervisor.db`: sessions and their metadata (SPEC.md's
  supervisor-authoritative list), agent profiles and each session's profile snapshot taken at creation (SPEC.md's
  snapshot rule shapes the session schema), spawn idempotency keys, captured conversation identities, host identity, and
  the boot id last seen. Comparing the stored boot id against the current one (`/proc/sys/kernel/random/boot_id`;
  equivalent on macOS) is how "interrupted" is classified per SPEC.md.
- Host identity: generated once at first run, stored in the db.
- Sessions launch through the user's shell as an interactive login shell inside the PTY —
  `$SHELL -l -i -c 'exec farhelm internal launch ...'` as the window's command, with the shim doing the final exec of
  the profile invocation (see exited-session semantics) — evaluated per launch. The `-i` is load-bearing, by different
  mechanisms per shell (audited): zsh sources `.zshrc` directly when interactive; bash login shells never source
  `.bashrc` themselves under any flags — only the profile chain — and `-i` matters because it puts `i` in `$-`, so the
  stock Debian/Ubuntu `.bashrc` interactivity guard doesn't bail out when the profile chains it. Either way the sourced
  file set matches an SSH-and-type session, which is the contract. When `$SHELL` is unset (user-manager services on
  systemd older than 255 don't set it), the supervisor falls back to the passwd database, then `/bin/sh`.
- Status heuristics: periodic sampling of tmux pane activity and captured tail content, sharpened per agent kind (see
  below). Sampling must never sit on the attach/input path — SPEC.md forbids status from gating interaction.
- Agent-kind integrations live in the supervisor as a small trait (`AgentKind`): status regexes over captured tail, and
  conversation-identity capture. Claude Code: watch `~/.claude/projects/<munged-cwd>/` for the session record. Audited
  specifics that shape this: the record appears at first prompt submission, not at launch, so correlation keys on
  first-input time and tolerates an unbounded launch-to-first-input gap; the cwd munging is non-injective (`/`, `.`, `_`
  all become `-`); and per-line JSON fields (sessionId, cwd, timestamps) are the reliable correlators — file birth times
  can postdate content after rewrites. Codex: same approach against `~/.codex/sessions` rollout files. An identity is
  claimed only when the correlation is unambiguous — two near-simultaneous launches in one cwd stay uncaptured, which
  triggers SPEC.md's explicit fallback instead of a silent wrong guess. Plain resume appends to the existing record
  under the same id for both agents (audited on current versions; a new id appears only on explicit forks —
  `--fork-session`, `forked_from_id`), so a captured identity survives restarts; the watcher treats appends as the
  resume signal and cheaply re-verifies identity after each restart rather than baking in either behavior. Capture is
  observation-only per SPEC.md — no hooks, no agent configuration.
- Per-session spawn credential: random token in the session's environment (`FARHELM_SESSION_ID`,
  `FARHELM_SESSION_TOKEN`, socket path), checked by the supervisor on the unix socket.
- Process-tree ownership (SPEC.md's stop/reap promises): killing the tmux pane is not enough — tmux signals the
  foreground process group, and daemonized descendants escape it. M2 ships the portable sweep: enumerate the pane's
  descendants by walking /proc PPIDs, unioned with a scan for processes whose environment carries the session's
  `FARHELM_SESSION_ID` marker (which catches daemons that already reparented to init), then SIGTERM, a short grace,
  SIGSTOP-quiesce, re-enumerate, SIGKILL — with process start-time validation so a recycled pid is never signaled.
  `systemd-run --user --scope` cgroup scopes layer on top as the Linux hardening (M3): where a functional systemd user
  manager exists — probed once by actually running a trivial transient scope and then showing, killing, and confirming
  the collection of it, not by `which`, and through absolute binary paths so a login shell's `$PATH` cannot substitute
  what the probe approved — each launch is wrapped in its own generation-named scope (audited on systemd 255: the
  wrapper execs in place, so the pane's process tree, exit codes, and liveness checks see exactly the unwrapped shape),
  the per-launch SELECTION is recorded durably as a boolean while the unit name is re-derived from session id plus
  generation at every use (a stored name would let a tampered row aim a kill at another session's unit), and stop kills
  through the scope first — SIGTERM, the same grace the sweep gives, SIGKILL, then confirming the unit was actually
  collected, because `systemctl kill` returning only proves delivery. The sweep ALWAYS runs afterwards as the backstop,
  and is the whole mechanism where no user manager exists — a missing manager never degrades stop below the sweep's
  guarantees, and neither does a broken one: the sweep's verdict is the answer, and the scope's troubles are diagnostic.
  A wrapper that fails runs before the shim can write its exec-failure sentinel, so the supervisor classifies that shape
  (a launch spec nothing ever consumed, on a dead pane, for a scoped launch) as **error** rather than letting it
  masquerade as a plain exit.

  **What the cgroup does and does not promise.** It targets ACCIDENTAL daemonization — the dev server, MCP server, or
  build watcher that double-forks and execs away its environment marker, which is exactly the shape the sweep provably
  cannot find. It does NOT contain a deliberately adversarial descendant: one that runs `systemd-run --user --scope` on
  itself migrates into a sibling unit under the same user manager, and with its marker scrubbed is then invisible to
  both mechanisms (reproduced, not theorized). Containing that needs a delegation boundary — a parent slice the
  supervisor owns, with the manager refusing migrations out of it — which v1 does not build and SPEC.md does not
  promise. Agent descendants run with the user's own privileges by design, so a descendant determined to outlive its
  session can always arrange to; the honest claim is that stop reaps what a normal program leaves behind. The macOS
  variant of the sweep (no /proc there) arrives with the Mac supervisor work. See
  lore/2026-07-27-m2-process-tree-stop.md for the alternatives as they looked when this was decided.
- Attachments land in `~/.local/state/farhelm/attachments/<session-id>/`, deleted with the session.
- The rest of the state directory: `supervisor.sock` (the unix socket that is the supervisor's only doorway — mode 0600,
  inside a 0700 directory, because reaching it means running commands as the user), `tmux.sock` and `tmux.conf` for the
  private tmux server, and `launch/` holding one 0600 JSON spec per session. A launch spec carries the agent's full
  command line, which users put credentials into, so the shim unlinks it as soon as it has read it, creation removes it
  if the session never starts, and the supervisor sweeps leftovers at startup.
- Symlink TOCTOU hardening of the state directory is intentionally absent. The directory create, chmod, lock, socket,
  and sweep operations are plain path-based calls that follow symlinks; making them airtight means `O_NOFOLLOW` opens,
  dir-fd-relative operations, and ownership verification throughout. Exploiting the gap requires write access to a
  parent of the state directory — the user's own home — and an attacker with that already runs arbitrary code as the
  user, so the rewrite buys nothing against any attacker this tool could plausibly face. Decided won't-fix during the M1
  review. Revisit only if the state directory ever moves somewhere group- or world-writable. (The one place symlink
  safety is load-bearing anyway, launch-spec creation, uses `O_EXCL` and is safe.)

## Helm internals

- State in SQLite at `~/.local/state/farhelm/helm.db`: host registry (SSH destinations, host identities), last-known
  session cache (survives helm restarts per SPEC.md), web token hash, browser device sessions, remembered defaults
  (last-used profile per host).
- axum serving: REST for CRUD (sessions, profiles, hosts), a WebSocket event stream for live session-list updates, a
  WebSocket per attached terminal, and the static UI bundle. Loopback bind enforced — refuses non-loopback per SPEC.md.
- Web token: random 128-bit value, stored hashed; browser auth exchanges it once for a device-session cookie; rotation
  deletes all device sessions.
- The native app embeds farhelm-helm in-process and manages the bundled local supervisor; the Linux helm is the same
  code behind `farhelm helm run`.

## Logging

`tracing` everywhere, with `tracing-subscriber` env-filter semantics. The intended mature shape uses spans carrying
session and host ids, so SPEC.md's required diagnostic trails (creation, PTY lifecycle, attachment transfer,
reconnection, resume) fall out of structured context rather than ad-hoc log lines.

M1 emits structured lifecycle and failure events, attaching session or channel fields where that context exists. It does
not yet establish the systematic session/host spans above: the host registry, reconnection, and resume are later
milestones, so their diagnostic trails cannot exist yet either.

Logs go to stderr, and deliberately not stdout: under `farhelm internal stdio` the process's stdout IS the protocol
channel, so a stray line there corrupts frames. File logging under `~/.local/state/farhelm/logs/` with rotation
(tracing-appender) and a `--log-level` flag are the intended shape but are not built yet — today verbosity is
`RUST_LOG`-style env only.

Motivation: tracing is the ecosystem standard, and span context is the cheap way to make "logs are available for X" a
property of the architecture instead of a discipline.

## CLI

clap (derive), one multi-call binary named `farhelm`, clean subcommand grammar. The user-facing surface:

- `farhelm helm run` — run the helm (flags: `--port`, `--state-dir`, ...).
- `farhelm helm token show|rotate` — web-token bootstrap and rotation.
- `farhelm supervisor run` — run the supervisor in the foreground; this is SPEC.md's "run the binary with arguments in a
  terminal" path.
- `farhelm spawn --cwd <dir> [--title ...] [--agent ...] [--parent ...]
  [--idempotency-key ...]` — the in-session
  spawn CLI from SPEC.md.

Internal commands live under a hidden-from-help `internal` namespace — `farhelm internal stdio` is the ssh-exec stdio
proxy. (An underscore prefix like `_stdio` was considered; it is not a recognized convention, while an explicit
`internal` namespace is self-describing and gives future internal commands a home.)

Motivation: one binary is one provisioning artifact and guarantees the spawn CLI exists inside every session (the
supervisor puts its own binary on the session PATH). clap-derive because it is the standard and keeps the grammar
declared next to the types.

## Native app packaging

Dioxus desktop (wry) wrapping farhelm-ui, bundled via `dx bundle` into a Mac app that embeds helm + supervisor from the
same workspace version. Native glue (dock/menu integration, plus whatever proves genuinely necessary — see the clipboard
note in the Dioxus risks) lives behind a feature flag in farhelm-ui, kept deliberately thin.

## Provisioning

Implemented in the helm over the same system-ssh access: sftp the cross-compiled `farhelm` binary (plus a private static
tmux build when the host has no tmux) into `~/.local/lib/farhelm/`, write user-level systemd units,
`systemctl --user enable --now`, `loginctl enable-linger` as the optional-step (proceed-without-if-privileged per
SPEC.md). Discovery-first: probe for a running supervisor via `farhelm internal stdio` before proposing any of this, and
show the full concrete action list before touching the host.

Motivation for shipping tmux ourselves when absent or too old: apt needs root, and SPEC.md forbids requiring it; a
static tmux under our own lib dir keeps the no-root promise without asking the user to install anything.

The provisioning payloads — linux-musl `farhelm` binaries for both architectures plus the static tmux builds — ship
inside the helm's own distribution: embedded in the Mac app bundle, included in the Linux release artifact. Motivation:
provisioning must work with no third-party downloads (consistent with the security posture), and coupling payload
version to helm version means a provisioned host runs exactly what the provisioning helm expects, which keeps the
version-skew story simple. The size cost (tens of MB) is accepted.

## Cross-compilation and targets

Supervisor-side artifacts: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, static, built with
cargo-zigbuild in CI (audited: rusqlite-bundled static musl builds work for both). The Mac app (`aarch64-apple-darwin`)
is built and bundled on a macOS CI runner with the native toolchain — cross-building it from Linux was audited and
rejected: zig cannot link the Apple frameworks wry needs (WebKit, AppKit) without a licensed macOS SDK, cross ships no
darwin images, `dx bundle` produces .app bundles only on the native platform, and codesigning wants macOS regardless.

Motivation: musl-static sidesteps glibc version skew across Ubuntu releases, which matters because provisioning drops
binaries on machines we do not control; "predicated on whatever cross-compiled binaries are available" in SPEC.md
becomes exactly this target list.

## Testing

The testing story is a first-class requirement — agents verifying GUI behavior without a human is a standing project
constraint (see the GUI section's motivation), not an afterthought:

- **Playwright (TypeScript) drives the web build in headless Chromium** against a real helm and real supervisor on
  Linux. DOM assertions and screenshots both work because the UI is real DOM. This is the canonical GUI verification
  path for agents.
- **A fake agent** — `farhelm internal fake-agent --script basic|altscreen|binary`, a hidden subcommand of the one
  binary rather than a separate artifact — stands in for Claude Code/Codex in M1 tests. Its deterministic scripts cover
  prompt/echo input, terminal modes, alternate-screen rendering, and byte-clean live output without vendor auth. Later
  milestones extend this fixture with fake on-disk records for status heuristics, conversation capture, and resume.
  Real-agent smoke testing stays manual.
- Rust integration tests exercise supervisor+tmux directly (CI provides tmux) and the framing protocol with golden
  cases; farhelm-proto keeps wire compatibility testable.
- The desktop shell's native glue is the acknowledged manual-test gap (see GUI risks); everything else must be coverable
  without a human.

## Version and skew

One version number across the workspace; the protocol hello carries protocol and build versions; incompatibility refuses
with a clear error at the edge (helm↔supervisor connect, client↔helm load) per SPEC.md. Protocol version bumps only with
incompatible frame changes.
