# Farhelm M1: walking skeleton

NOTE: This is the plan for milestone 1 only. It builds toward SPEC.md using
the choices in SPEC_impl.md; where this document and those disagree, they
win. The overall motivation and the coarse milestone ladder live in
PLAN.md; only the current milestone is planned at this level of detail.

## Goal

One session, end to end, for real: a local GUI window (and the same UI in a
browser) showing official Claude Code running in a real TUI on a remote
Linux host, connected through the real architecture — helm, framing
protocol over ssh, supervisor, private tmux, xterm.js. Usable for actual
work, deliberately minimal in everything else.

The point of this shape: it forces the highest-risk column of SPEC_impl.md
through contact with reality first — the Dioxus/xterm.js island, the WS
terminal path, control-mode streaming, resize pinning, the login-shell
launch, reconnect replay. If the skeleton works, the architecture works.
Everything in M1 is a thin vertical of the permanent crates; nothing is a
prototype to be thrown away.

## User-visible outcome

- Start `farhelm supervisor run` manually on a Linux host (binary copied
  there by hand — provisioning is a later milestone).
- Locally run the app (or `farhelm helm run` plus a browser) with the
  session specified on the command line: ssh destination, working
  directory, agent command.
- A window opens with the agent's real TUI. Typing, colors, alternate
  screen, resize, mouse scrolling all behave like a local terminal.
- Close the window or browser tab, reopen: same session, scrollback and
  terminal modes intact. The agent never noticed.
- Kill the GUI for a day; the agent keeps working on the remote host.

## Scope

### In

- **CLI surface**: the `farhelm` binary (the workspace and crate stubs
  exist from M0) gains its clap subcommands (`helm run`, `supervisor run`,
  `internal stdio`).
- **Proto**: the real framing (length-prefixed, channel ids, JSON control +
  raw data frames) and the version hello, from day one. Retrofitting a
  handshake is much more painful than shipping it in the skeleton.
- **Supervisor**: private tmux server (generated config per SPEC_impl.md),
  control-mode consumption, one session = one tmux session, create /
  attach-stream / input / resize / capture-prefill, pane-mode restoration
  on reattach, login-shell launch via the `farhelm internal launch` shim,
  last-attach-wins enforcement. Session state in memory — tmux is the
  truth for M1; SQLite arrives with multi-session in M2.
- **Helm**: axum with the real create-session API and per-terminal
  WebSocket, static serving of the web UI, local (unix socket) transport to
  a same-machine supervisor and ssh transport (`ControlMaster` +
  `farhelm internal stdio`) to the remote one. The argv session is created
  through the same API any future UI dialog will call — CLI flags bypass
  the creation UI, never the creation path.
- **UI**: Dioxus, web and desktop from the same crate; the xterm.js island
  with the direct WS byte path and watermark backpressure hooks in the
  interface (the full pause/resume plumbing may be a stub, but the seam
  exists).
- **Test harness** (a deliverable, not a nicety — agents verifying the GUI
  without a human is how the rest of this project gets built):
  - the fake agent binary: scripted TUI that prompts, echoes, uses colors
    and the alternate screen, and can emit bracketed-paste/mouse mode
    sequences on cue;
  - Playwright driving the web build headless against a real helm +
    supervisor (local transport) running the fake agent: create, see
    output, type input, reconnect, assert replay;
  - Rust integration tests for the tmux driver and framing golden cases;
  - CI running all of it on Linux.

### Out (deliberately)

Session list and multi-session, SQLite, profiles (argv is the profile),
status heuristics, attachments, terminal tabs, conversation capture and
resume, interrupted/error classification, host registry and multi-host UI,
web token auth (loopback only), provisioning, `farhelm spawn`, archive,
Mac app bundling (`dx serve`-grade desktop is fine), notifications-of-any-
kind per SPEC.md.

Likely to fall out for free but not promised: session survival across a
manual supervisor restart (tmux holds everything; the supervisor
rediscovers its one session from the private socket). Worth a manual poke,
not an M1 gate.

## Order of work

Each step leaves something runnable; later steps only add.

1. Proto crate (workspace exists from M0): framing, hello, golden tests.
   Runnable as tests only.
2. Supervisor core against local tmux: create/attach/input/resize over the
   unix socket, exercised by Rust integration tests and a throwaway
   `nc`-grade REPL. Fake agent lands here (it is the test subject).
3. Helm serving the terminal WS + web UI with the xterm island, local
   transport only: first pixels — browser tab talking to a local fake
   agent session.
4. Reconnect replay: capture prefill + mode restoration; Playwright
   harness lands here and pins it.
5. ssh transport: `ControlMaster`, `internal stdio`, remote supervisor;
   same UI now reaches a real remote host. Real Claude Code, real work.
6. Desktop window (Dioxus desktop target of the same crate) + CI wiring.

## Acceptance

M1 is done when all of the following hold:

1. `farhelm supervisor run` on a remote Ubuntu host (hand-copied binary);
   locally, one command opens a window running official Claude Code in an
   existing directory on that host, specified entirely on the command line.
2. The TUI is fully usable: typing, colors, alternate screen, resize,
   wheel scrollback with native selection — no tmux UI visible anywhere.
3. Closing and reopening the window (or tab) reattaches to the same
   session with at least the SPEC.md replay floor of scrollback and
   correct terminal modes (bracketed paste still works after reattach).
4. The same session is reachable from a browser at the helm's loopback
   port; opening it there visibly detaches the window (last attach wins).
5. `cargo test` plus the Playwright suite pass in CI on Linux against the
   fake agent, covering: create, output rendering, input round-trip,
   reconnect replay, and a resize.
6. The helm↔supervisor hello refuses a deliberately mismatched protocol
   version with a clear error (tested).

## Risks this milestone retires

Dioxus web+desktop from one crate in practice; xterm.js island and the
byte path's real feel (latency, scrolling, TUI fidelity through tmux
control mode); %output passthrough unwrapping; framing over ssh exec
stdio; login-shell environment on a real host; replay correctness. These
are the bets the whole design rests on — M1 exists to test them while the
codebase is still small enough to bend.
