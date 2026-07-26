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
  available there.)
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
rates; `cat` of a huge file must degrade to slow, never to silent data loss.

## Terminal substrate: private tmux server

Each supervisor runs a dedicated tmux server on a private socket (`~/.local/state/farhelm/tmux.sock`) with a locked-down
generated config: status bar off, `history-limit` sized to SPEC.md's replay floor, `window-size manual`,
`remain-on-exit on`. One tmux session per Farhelm session; window 0 is the agent terminal, additional windows are the
terminal tabs. The user's own tmux usage and config are untouched.

Farhelm requires tmux ≥ 3.3 (dependable control mode). Releases bundle a private tmux build per platform, used whenever
the host's tmux is missing or below the floor — the Mac app bundles one too, since macOS ships no tmux at all. A host
tmux at or above the floor is acceptable; version is checked, not just presence.

tmux is a headless PTY holder and history store. The supervisor's only client is a non-rendering control-mode client
(`tmux -C`, the interface iTerm2's tmux integration is built on; `pipe-pane` is the fallback shape). Sizing (audited on
tmux 3.7): a control-mode client is an attached client, but tmux ignores it for window sizing until it declares a size
via `refresh-client -C` — the supervisor never declares one, and instead pins sizing with `window-size manual` plus
explicit `resize-window` tracking the attached GUI client's dimensions (`resize-window` itself forces
`window-size manual` on the window, so the config setting and the command agree). The supervisor streams raw pane output
to the client; input goes in via `send-keys`/`paste-buffer`. Passthrough sequences (audited): the control-mode `%output`
stream carries `\ePtmux;...\e\\`-wrapped payloads still wrapped, regardless of the `allow-passthrough` option — that
option only gates forwarding to rendering clients, which Farhelm has none of — so the supervisor unwraps passthrough
payloads itself before they reach xterm.js. Reconnect replay prefills xterm.js from `capture-pane -e` history, then
switches to the live stream — that is how the 10,000-line floor is met. Content alone is not enough, though: pane modes
(alternate screen, bracketed paste, mouse reporting, application cursor keys, cursor position) are read from tmux pane
format variables and re-synthesized into xterm.js after the prefill — without that, a reattached full-screen agent
silently loses paste bracketing and mouse reporting. The supervisor enforces SPEC.md's one-attachment rule itself.

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
supervisor's internal terminal interface stays narrow (create, attach-stream, resize, capture, kill) so a Rust holder
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
  foreground process group, and daemonized descendants escape it. On Linux, each session's command runs inside its own
  `systemd-run --user --scope` cgroup and stop/reap kills the cgroup; where no user systemd manager exists, and on
  macOS, the fallback is process-group kill plus a sweep for surviving processes carrying the session's marker
  environment variable.
- Attachments land in `~/.local/state/farhelm/attachments/<session-id>/`, deleted with the session.

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

`tracing` everywhere, with `tracing-subscriber` env-filter semantics. Components write human-readable logs to
`~/.local/state/farhelm/logs/` with rotation (tracing-appender); verbosity via `RUST_LOG`-style env and a `--log-level`
flag. Spans carry session ids and host ids so SPEC.md's required diagnostic trails (creation, PTY lifecycle, attachment
transfer, reconnection, resume) fall out of structured context rather than ad-hoc log lines.

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
- **A fake agent binary** (part of the workspace) stands in for Claude Code/Codex in tests: scripted TUI output,
  question prompts, alternate screens, and fake on-disk session records matching each agent kind's layout. This makes
  end-to-end tests — including status heuristics, conversation capture, and resume — deterministic and free of vendor
  auth. Real-agent smoke testing stays manual.
- Rust integration tests exercise supervisor+tmux directly (CI provides tmux) and the framing protocol with golden
  cases; farhelm-proto keeps wire compatibility testable.
- The desktop shell's native glue is the acknowledged manual-test gap (see GUI risks); everything else must be coverable
  without a human.

## Version and skew

One version number across the workspace; the protocol hello carries protocol and build versions; incompatibility refuses
with a clear error at the edge (helm↔supervisor connect, client↔helm load) per SPEC.md. Protocol version bumps only with
incompatible frame changes.
