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
the duration of a viewer's pause. SPEC.md's stall bullet states that bounded-slowdown contract directly, and this
document's job is only to record the mechanism: nothing here throttles the agent deliberately, and the block is bounded
by the flow-control window and ultimately by the stall detach. The permission is deliberately left standing even though
the supervisor no longer takes it up (see the session sink, below) — it is what keeps the layers above free of any
assumption about which way tmux answered.

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

The first path is nonetheless one the supervisor now prevents from arising against a Farhelm session, and the reason is
the multi-terminal shape tabs introduced rather than any change of heart about degrading to slow. tmux stops reading a
pane when no attached client is able to consume it, and that judgement is about the PANE, not about the stalled client's
own terminal — so once a session has several terminals, a stalled viewer on a background tab could block the agent's
writes, which is a very different bargain from a viewer slowing the terminal it is itself looking at. Two further
measurements sharpened it (2026-08-02, tmux 3.4 and 3.7b): the block is not bounded by `pause-after` at all (observed
persisting for a full 45-second window, ending only when the stalled client went away), and it reproduces only at high
output rates, which is why an audit can honestly report it as intermittent. So every session with a live attachment now
also carries one always-drained control client of its own — a session sink — whose only job is to be somebody tmux can
always deliver to. With it attached, only the second path remains reachable, and the per-terminal clients additionally
turn the session's other panes off for themselves (`refresh-client -A <pane>:off`), which is safe only because the sink
is there to keep those panes readable. Nothing above this layer changes: `%pause` is still honored whenever it arrives,
and code may still never assume which path tmux took.

One qualification belongs with that claim rather than in a footnote, because it is the only hole left in it: a sink is a
process, and a process can die. From the moment one does until its replacement has attached — a process spawn and one
control-mode round trip, retried with exponential backoff capped at a few seconds, forever, for as long as any terminal
of that session is attached — the session's terminals still have their foreign panes filtered off with nothing holding
those panes readable, so a pane nobody is watching can stop being read for that window. The window is bounded by the
backoff cap and is not otherwise defended against: closing it entirely would mean keeping a second sink permanently
attached to every session, paying a certain cost against an uncertain one. An attach that arrives during such a window
waits for the sink to come back rather than installing filters into it, which is the one case where the gap must not be
allowed to widen.

The xterm.js scrollback capacity is therefore sized to at most the tmux history floor (both currently 12,000 lines) — an
invariant tests must pin — which makes the replay-based catch-up's end state observably equivalent to lossless slow
delivery: every byte still within the terminal's own retention is present, and bytes beyond it would have been evicted
from scrollback even had they been delivered one at a time.

## Terminal substrate: private tmux server

Each supervisor runs a dedicated tmux server on a private socket (`~/.local/state/farhelm/tmux.sock`) with a locked-down
generated config: status bar off, `history-limit` sized to SPEC.md's replay floor, `remain-on-exit on`. One tmux session
per Farhelm session; window 0 is the agent terminal in practice, additional windows are the terminal tabs. Neither is
identified by position: the supervisor stamps each window it creates with a tmux user option — the agent's window with
the session id, a tab's window with a minted tab id that is also that tab's whole record. The agent terminal is
identified by its durable pane record first, with the marker as the recovery aid for a session whose record is empty;
tabs have no durable record at all and are rediscovered from their markers alone, because a pane's own processes inherit
`TMUX` and can conjure windows a positional scan would adopt. The user's own tmux usage and config are untouched.

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
that is how the 10,000-line floor is met without a gap between the two. The handoff ordering is load-bearing: a separate
tmux command process targets the incumbent control client by its tmux-assigned name and switches it back to `no-output`;
only after that process succeeds is the incumbent's stdin closed and the process reaped. The acknowledgement cannot
share the output client's protocol stream because cancellation may leave older positional command replies unread there.
tmux applies `no-output` by discarding all pending pane blocks for that client and refusing new ones, so this is a
client-wide boundary rather than a racy list of panes that existed when teardown began. Closing or killing tmux 3.7b's
client while one of those blocks remains can abort the whole private server with `fatal: not enough data`; the
acknowledged transition is therefore part of the handoff contract, not cleanup polish. Pane modes, a history snapshot, a
visible-screen snapshot, and a final `refresh-client -f !no-output,pause-after=N` are submitted as one
semicolon-separated command group through that replacement. The matching `%end` for the final refresh block is the
cutover: earlier pane bytes are represented by the snapshot, later ones arrive as live output, and `no-output` advances
rather than queueing a second copy for delivery. Normal-screen replay selects the history snapshot; alternate-screen
replay selects the visible snapshot so normal history is not mixed into a full-screen app.

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
the profile invocation and, on exec failure, writes a sentinel with the errno detail to a per-launch status file (named
by session and launch generation, so a sentinel left by a failed earlier launch can never describe a later relaunch)
before exiting. The supervisor classifies **error** on that sentinel; the one sentinel-less error path is a
cgroup-scoped launch whose `systemd-run` wrapper died before the shim ever ran, recognized only by its full evidence
shape (dead pane, launch spec still unconsumed, no sentinel) so it can never claim an agent that actually started. NOTE:
a sentinel written by the shell after a failed `exec` was audited and rejected — interactive bash survives a failed
exec, but zsh terminates on it in every mode, so shell-side code after `exec` never runs for zsh users; the shim works
identically under any `$SHELL`.

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

The ssh child's stderr is piped and relayed as bounded, control-escaped tracing events attributed to the host, not
inherited. Inheriting is defensible for the single-host path a user started by hand; for a registered host it hands a
remote party a direct, unbounded write channel to the operator's own terminal, escape sequences included. The relay
drains continuously (a full stderr pipe would wedge the child), caps each line, stops logging after a per-connection
budget while still draining, and Debug-escapes what it does log — the same treatment the supervisor gives tmux's exit
reasons, for the same reason. Peer-supplied error text is normalized the same way wherever it is logged or retained in a
host's state, and repeated identical failures stop being logged after a few, with the suppressed count reported by the
next different one — a host that is down, or a peer that errors on every refresh, must not be able to write the log
indefinitely.

The hello's two free-text fields carry generous LENGTH caps checked at handshake decode (256 bytes each): both are
retained for the connection's whole life by the peer's counterpart, so an unbounded one is a memory cost a peer chooses
for the other side and re-chooses on every reconnect. Over-long is refused, never truncated — a shortened identity would
map two distinct hosts onto one claim. Length only, never shape: an identity is opaque to every consumer by design, so a
format check would invent a compatibility rule nothing else has.

The ssh argv puts its option terminator (`--`) BEFORE the destination, and that placement is a security boundary rather
than a style choice: a destination is user-supplied text, and one shaped like `-oProxyCommand=...` is read by OpenSSH's
own option parser and executed locally — a command injection with no ssh connection involved at all — for as long as the
terminator sits anywhere after it. The registry additionally refuses option-shaped destinations when they are
registered, so the user gets a clear error rather than a puzzling ssh failure; the argv ordering is the actual guard,
since it also covers callers that never go through the registry.

On top of that byte pipe: a multiplexed framing protocol — length-prefixed frames carrying a channel id and a type tag;
control messages are serde_json, terminal data channels are raw bytes. The same protocol runs over the unix socket
locally, so "local host" and "remote host" differ only in transport. Connection setup exchanges protocol and build
versions; it refuses protocol-version incompatibility per SPEC.md's version-skew rule, while build versions travel for
diagnostics only — mixed builds with a compatible protocol are the normal steady state SPEC.md describes.

Motivation: SPEC.md promises provisioning and transport ride "the user's keys, agent, and config" — only the real ssh
binary honors ~/.ssh/config fully (ProxyJump, Match blocks, agent forwarding, ControlMaster). russh was rejected for
exactly that: partial config support would quietly break the promise. JSON control frames keep the protocol debuggable
by eye; raw binary data channels keep PTY throughput off the JSON path.

The session list is cursor-paginated on this wire (protocol 8, M6). The contract, recorded here because the milestone
plan that settled it is history the moment M6 closes: pages walk a total order — creation time descending, session id
ascending as the tiebreak — over columns that never change for a live session, so an issued cursor stays a valid resume
point for as long as its holder cares to use it. The cursor is opaque: it encodes the last-returned entry's ordering
key, callers store and replay it verbatim, and an undecodable cursor is refused as an invalid request. A decodable
cursor is simply an ordering key, and deliberately so — cursors carry no authority in a single-user supervisor, any
well-formed key is a valid resume position (which is also what lets a deleted session's cursor resume cleanly), so there
is nothing a forged cursor obtains that honest paging does not. Resumption is strictly-after, which is also what makes a
cursor whose own session was deleted resume cleanly. Under concurrent mutation the walk promises no duplicates and no
tearing, and deliberately NOT completeness: a session created mid-walk can land behind the cursor (same-second creations
tie-break by id; a clock rollback places new sessions mid-order) and is only guaranteed by the next walk from the start.
Both page cuts — the count limit and the frame-size budget — carry the same continuation cursor, so truncation is a
resumable state rather than M2's terminal flag, and the reply's total keeps reporting the full count before any cut. One
case is neither cut: a record too large to ship even alone (nothing bounds a title below the byte budget) is an explicit
error, never a fake exhaustion — an empty page with no cursor would otherwise claim the walk was done when a session
remains that can never be represented on any page.

## Supervisor internals

- State in SQLite (rusqlite) at `~/.local/state/farhelm/supervisor.db`: sessions and their metadata (SPEC.md's
  supervisor-authoritative list), agent profiles and each session's profile snapshot taken at creation (SPEC.md's
  snapshot rule shapes the session schema), spawn idempotency keys, captured conversation identities, host identity, and
  the boot id last seen. Comparing the stored boot id against the current one (`/proc/sys/kernel/random/boot_id`;
  equivalent on macOS) is how "interrupted" is classified per SPEC.md.
- Host identity: generated once at first run, stored in the db.
- Agent profiles live in a `profiles` table in that same db, bounded on both axes — 128 profiles per host, 8 KiB of
  caller-supplied text per profile — so the unpaginated catalog reply can never outgrow a frame. That bound is not
  tidiness: the listing is also how a client finds the profile it wants to delete, so a catalog too large to list would
  be one nobody could trim back. The starter profiles SPEC.md promises (Claude Code, Codex) are seeded by the schema
  migration that creates the table, not by a check at startup. A migration step runs exactly once per database, so a
  deleted starter stays deleted and an edited one stays edited, with no "already seeded" flag that could disagree with
  the table it describes and re-seed what the user threw away. A profile names its kind explicitly (`generic` is the
  spelling for "no kind"), and an absent resume template means the kind's own default, derived at create time from that
  profile's invocation. A session records the id and name of the profile it was created from and nothing mutable;
  whether that profile still exists, and whether it has since been renamed, is derived by one catalog lookup when a
  reply is built (one per reply, not one per session), so an edit or a delete never rewrites historical rows and there
  is only one copy of existence truth. Creating a session from a profile that has since been deleted fails as a
  precondition — before any launch, with no session left behind — and never falls back to another profile.
- Sessions launch through the user's shell as an interactive login shell inside the PTY —
  `$SHELL -l -i -c 'exec farhelm internal launch ...'` as the window's command, with the shim doing the final exec of
  the profile invocation (see exited-session semantics) — evaluated per launch. The `-i` is load-bearing, by different
  mechanisms per shell (audited): zsh sources `.zshrc` directly when interactive; bash login shells never source
  `.bashrc` themselves under any flags — only the profile chain — and `-i` matters because it puts `i` in `$-`, so the
  stock Debian/Ubuntu `.bashrc` interactivity guard doesn't bail out when the profile chains it. Either way the sourced
  file set matches an SSH-and-type session, which is the contract. When `$SHELL` is unset (user-manager services on
  systemd older than 255 don't set it), the supervisor falls back to the passwd database, then `/bin/sh`.
- Status heuristics: periodic sampling of tmux pane activity and captured tail content, sharpened per agent kind (see
  below). Sampling must never sit on the attach/input path — SPEC.md forbids status from gating interaction. The
  supervisor's own ticker takes the samples; classification is a pure read of the sample beside the durable outcome, and
  sits BELOW the recorded-error and dead-pane rules in the existing precedence, so a heuristic can only ever choose
  among the live statuses. The generic baseline is observed output alone, counted in a session's OWN samples rather than
  in elapsed time: three consecutive samples showing an unchanged screen reads idle, anything else live reads running,
  and a session not yet sampled twice reads running since that is what a session that just launched is. Counting samples
  rather than seconds is load-bearing — the sampler works through live panes on a budgeted round robin, so a session's
  real sampling period grows with the fleet, and any wall-clock window would eventually report a continuously-working
  agent as idle because the HOST was busy. Waiting is never derived from activity at all (a blocked agent and a finished
  one are equally quiet); it comes only from per-kind sharpening.
- Agent-kind integrations live in the supervisor as a small trait (`AgentIntegration`; `AgentKind` is the wire enum
  naming the kind itself): status sharpening over the sampled tail, and conversation-identity capture. Sharpening is a
  DEFAULTED trait method that may only promote a live baseline to waiting, never invent liveness, and never panic on
  arbitrary terminal bytes; the default is "no sharpening", which is deliberately different from the no-integration case
  (generic sessions still get the baseline). Recognition is conservative by design — a vendor question phrase AND a
  rendered menu of numbered answers, both at the bottom of the screen — because a status that reads waiting at a working
  session teaches users to ignore the column, while a missed prompt merely reads idle. Claude Code: watch
  `~/.claude/projects/<munged-cwd>/` for the session record. Audited specifics that shape this: the record appears at
  first prompt submission, not at launch, so correlation keys on first-input time and tolerates an unbounded
  launch-to-first-input gap; the cwd munging is non-injective (`/`, `.`, `_` all become `-`); and per-line JSON fields
  (sessionId, cwd, timestamps) are the reliable correlators — file birth times can postdate content after rewrites.
  Codex: same approach against `~/.codex/sessions` rollout files. An identity is claimed only when the correlation is
  unambiguous — two near-simultaneous launches in one cwd stay uncaptured, which triggers SPEC.md's explicit fallback
  instead of a silent wrong guess. Plain resume appends to the existing record under the same id for both agents
  (audited on current versions; a new id appears only on explicit forks — `--fork-session`, `forked_from_id`), so a
  captured identity survives restarts; the watcher treats appends as the resume signal and cheaply re-verifies identity
  after each restart rather than baking in either behavior. Capture is observation-only per SPEC.md — no hooks, no agent
  configuration.
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
  session can always arrange to; the honest claim is that stop reaps what a normal program leaves behind. macOS has no
  /proc, so the three reads the sweep needs — the same-euid process table, one pid's parent/start-time/zombie state, and
  one pid's environment — go through a platform seam that answers them with `sysctl` there (`KERN_PROC_ALL`,
  `KERN_PROC_PID`, and `KERN_PROCARGS2`) and with /proc on Linux; every decision above it, and therefore everything stop
  promises, is one shared implementation. The Mac marker source is deliberately narrowed to the environment region of
  `KERN_PROCARGS2` with argv discarded, so that neither platform can claim a process for marker text that merely appears
  on its command line. One Mac residual on top of the shared ones: macOS 26+ withholds that environment region for Apple
  platform binaries even from a same-uid parent (observed on real hardware, pinned by a macOS-only test), so a
  reparented descendant exec'd into `/bin/sh` or another platform binary escapes the marker scan there; the PPID closure
  still reaps it while it remains in the pane's tree. The planned close is a session-id membership channel — tmux panes
  are session leaders and a SID survives fork, exec, and reparenting — deferred until the gap proves to matter in
  practice. See lore/2026-07-27-m2-process-tree-stop.md for the alternatives as they looked when this was decided.
- Attachments land in `~/.local/state/farhelm/attachments/<session-id>/`, deleted with the session. There is no size cap
  in v1: the bytes are the user's, on the user's own machine, and every hop streams them under a credit window, so a
  large file costs time rather than memory. A disk that fills up is therefore a failed upload with nothing published and
  a visible error, never a truncated file at the published path.
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
  session cache (survives helm restarts per SPEC.md), recoverable web token, hashed browser device sessions, remembered
  defaults (last-used profile per host).
- The host registry (PLAN_M6.md item 3) reserves one row for the machine running the helm itself: auto-created at `open`
  if absent, never user management surface, never removable. It exists specifically so the local host has a cache row to
  serve stale sessions from when its own supervisor is down — the plan's first draft made this row optional, and review
  caught that a row-less local host would have nowhere to cache into, breaking the very promise (stale sessions survive
  a down host) the cache exists to keep. An SSH row also carries optional `remote_farhelm`/`remote_state_dir` fields
  (the argv fields M1's `--remote-farhelm`/`--remote-state-dir` carried), `None` meaning "use the remote's own default",
  not "unset for now". Two distinct SQL mechanisms enforce two distinct invariants here, not one: the `hosts` table's
  own `CHECK` constraint is what pins the local row's NULL destination/remote-field shape, while separate partial unique
  indexes enforce at most one local row, uniqueness of an SSH row's destination among SSH rows, and — see below — at
  most one row claiming any given host identity.
- A host's `host_identity` is `NULL` until first contact ever succeeds for that row — including the local row, which is
  minted with no identity and learns one the same way any other host does. Recording it is split into two operations so
  silent identity merging is structurally impossible at the storage layer (SPEC.md: never silently merge): first contact
  writes only when the stored identity is still `NULL`, or is a no-op when it already matches what was just reported: a
  DIFFERENT stored identity is refused outright, changing nothing, with the mismatch surfaced as a value the caller acts
  on rather than an error. Adoption is a separate, explicit compare-and-swap that only a user's adopt choice may invoke.
- At most one registry row may hold a given identity, and that is a SCHEMA invariant (a partial unique index), not a
  check the connection manager performs before writing. The difference is not stylistic: with a check-then-record shape,
  two entries reaching one freshly installed supervisor can both see no twin and both record, and at the next helm start
  each sees the other as its twin and both freeze as duplicates — so a live host appears zero times. First contact and
  adoption therefore resolve the claim inside the same transaction as the write, and a loser gets a typed outcome naming
  the row that holds it, which the manager renders as the ordinary duplicate state. Databases predating the constraint
  are resolved by its migration: the lowest host id keeps the claim, later rows are demoted to unclaimed and lose the
  cache that was only meaningful under it, so they re-learn at next contact and freeze as duplicates properly.
- Both identity writes also carry the connection-defining configuration the attempt was DIALED under (destination,
  remote farhelm, remote state dir) and are refused if the row no longer matches. A hello that crossed the wire while
  the user was retargeting a row describes the old endpoint, and committing its identity under the new configuration
  would durably attribute one machine's identity to another. Tearing down in-flight attempts on an edit narrows that
  window; checking the dialed configuration inside the write's own transaction closes it.
- Each host's session cache is replaced wholesale on every successful list refresh — delete then insert in one
  transaction, never a partial mix — so a session dropped from a host's live list is dropped from its cache too, and
  ordering never depends on parsing every cached row's JSON (created_at and session id are extracted as columns at write
  time). Removing a host cascades its cache rows (SPEC.md's disposal rule). Adopting a new identity at a known
  destination purges that host's cache in the same transaction as the identity write: the old identity's cached sessions
  describe a dead install, and carrying them forward under the new identity would misattribute one install's history to
  another. A cache write also carries the identity it was produced under, checked against the stored value in the same
  transaction: this closes the window where a refresh already in flight when a user adopts a new identity could land
  after the adoption's purge and repopulate the cache with the dead install's sessions by a side door. Reads of the two
  tables disagree on purpose: a cache row that no longer decodes is skipped and logged rather than failing the read (it
  is last-known display data, not authority), while a corrupt registry row still fails `list_hosts` loudly (the registry
  is authority for which hosts exist at all).
- One connection actor per registry row, the local row included (PLAN_M6.md item 4), each owning its transport
  connection, its reconnect state machine, and its slice of the session cache. A row's connection is always in exactly
  one of six states, and the last three exist because folding them into "unreachable" would throw away the only
  information that makes the situation fixable: **connecting** (active retries in progress), **unreachable-reprobing**
  (the active window is spent, background probes continue forever, no give-up), **connected**, **version-skew** (the
  hello was answered and refused; carries both protocol versions, the peer's build, and the remediation text, since
  SPEC.md demands actionable rather than merely diagnostic errors), **identity-mismatch** (frozen, carrying both
  identities, connecting nothing until the user adopts or fixes the destination), and **duplicate** (this entry's
  identity is already another entry's; connects nothing while it stays one, so the HOST appears exactly once under the
  twin while the entry stays visible as something to resolve). The local row's unreachable state additionally
  distinguishes "no supervisor is running on this machine" from a generic transport failure, because that is the one
  case whose remedy is a command on the machine the user is already sitting at — a manual-path hint, never an offer to
  install (provisioning is M7's). A seventh state exists that is not about the host at all — **retired** — for an entry
  whose actor has stopped: a panicked task, or one that outlived its own registry row. Without it, an actor's last
  published status stands forever after the actor is gone, so a task that died mid-connection would leave the entry
  reading connected, with a routable client, and nothing left running to ever correct it. Each actor is therefore
  supervised by the task the manager actually holds, which publishes the retired state (client dropped) when the actor
  it wraps finishes for any reason other than being cancelled on purpose.
- A host's state and its live connection are read TOGETHER, from one borrow of the actor's published status. The pair
  has an invariant — a client exists exactly while the state is connected — and session routing is built on it, so two
  separate reads straddling a transition would let a caller refuse an operation against a host that is up, or route one
  onto a connection that is already gone.
- Shutting the manager down is terminal: the flag it sets is checked in the same lock hold that reconciliation does its
  insertions in, so a reconcile that read the registry just before the shutdown becomes a no-op instead of repopulating
  the map with actors nothing can stop.
- Cadences (user decision 2026-08-04, "snappy"), all injectable so tests can drive real transports without waiting out
  production timescales: active retries wait 1, 2, 4, 8, 15 and 30 seconds between attempts — an immediate attempt plus
  six, spread over about a minute — and then background re-probing takes over at 45 seconds, forever. The whole point of
  those numbers is that a host which comes back is noticed within about a minute of returning, while a fleet of down
  hosts costs a little over one connection attempt per host per minute. A connected host's session list refreshes every
  3 seconds, matching the UI's own poll interval, so multi-host aggregation does not make the visible list staler than
  the single-host path already is. The two regimes are distinct rather than one repeating ladder: a re-probe is a SINGLE
  attempt, and a fresh active window is granted only where something changed — startup, a connection that was up and was
  lost, or the resolution of a freeze. A re-probe also leaves the host's existing state alone while it dials, so an
  entry that has been unreachable overnight reads as unreachable instead of flickering into "connecting" every 45
  seconds. Version-skewed and duplicate entries ride the same 45-second cadence: the first so an upgraded host
  resurfaces by itself, the second to re-ask the registry whether the collision is still there. Identity-mismatch is
  deliberately the one state with no timer at all, because no amount of waiting answers a question only a user can.
- Two DEADLINES bound what a peer can do to an actor by saying nothing, both injectable alongside the cadences. One
  connection attempt (dial and hello together) is bounded at 20 seconds, and expiry is an ordinary failed attempt so the
  ladder and the re-probe cadence carry on unchanged; one cache refresh is bounded at 30 seconds, and expiry drops the
  connection so the actor re-enters its normal loss handling. Without them a transport that accepts and then goes silent
  parks the whole state machine indefinitely while every layer below looks healthy — no error, no EOF, and a host that
  reads as connecting or connected forever.
- Editing a registry row's connection-defining fields RECONNECTS the host rather than waiting for its current connection
  to end on its own: the connection is torn down, a non-connected state is published together with the new row (so a
  hosts list can never pair an edited destination with the old connection's state), and the actor gets a fresh active
  window, which is the same treatment resolving a freeze earns. An explicit retry is the same restart without the fresh
  window — a user clicking retry is not evidence that a down host is back, so it makes one attempt and returns to the
  re-probe cadence, while a connected host's retry is a genuine reconnect rather than an early poll.
- A cache write refused because this connection's identity is no longer the row's also ends the connection. Every later
  refresh on it would be refused identically, so keeping it up would show a host as healthily connected while its stale
  list silently stopped advancing; dropping it re-asks the identity question against the row as it now stands.
- A connected host's cache refresh is drain-then-replace: follow the supervisor's `next_cursor` to exhaustion, then
  replace that host's whole cache slice in one identity-bound write. The page limit is left unset so the supervisor
  applies its own default cap, which is sized so an ordinary host's entire list arrives in one page — that matters
  beyond round-trip count, because the supervisor's conversation-capture sweep rides the `ListSessions` handler and
  therefore runs once per page, so a smaller limit would multiply whole-host scans for every host on every refresh. A
  failed refresh records the failure and keeps the previous cache, never wiping it: the cache's whole job is to answer
  "what did this host have, last we knew" while the host is unavailable, so clearing it on failure would destroy the
  answer exactly when it becomes the only one available, and would make a transient failure look identical to "this host
  genuinely has no sessions". A host whose supervisor reports no identity at all connects and serves live but writes no
  cache, since the identity binding has nothing to bind to. The walk's termination is never the peer's to decide: it is
  bounded by pages followed, by sessions accumulated (ten of the supervisor's own default pages), and by a refusal to
  follow a cursor identical to the one that produced it — each catching a shape the others cannot, and all three landing
  as an ordinary failed refresh that keeps the previous cache.
- The served session list is a MERGE, and it is served from what the helm has already recorded rather than from the
  hosts. Every connected host's actor drains its supervisor's paginated list to exhaustion into helm.db; the list
  endpoint then merges what is there — live hosts' latest refresh and down hosts' last-known entries alike — into one
  order, tagging each row with its host and marking it stale unless that host is connected right now. A host being
  connected changes only that flag, never where its rows are read from.
- The order is `created_at` DESCENDING, then session id ascending, then HOST ID ascending. The first two are the wire's
  own total order; the third is what keeps the merged order total even in a database whose one-owner index is absent,
  and a cursor over a non-total order can skip or repeat rows.
- One host does not fit the cache rule and cannot be made to: a supervisor reporting NO identity, against a registry row
  that has none on record either, has nothing for the identity-bound cache write to bind to. Its refreshes are kept in
  the connection manager's memory and merged into the list and the owner lookup from there; they serve while it is
  connected and vanish when it is not, because with no durable copy there is nothing to stand behind. A row that HAS a
  recorded identity meeting an identity-less hello is a different situation entirely and fails closed — see below.
- The REST list is paginated with a helm-level cursor that is deliberately DECOUPLED from the wire cursor underneath it.
  Composing per-host wire cursors into the REST cursor would tie one browser page fetch to N live host round trips, so a
  single flapping host would break a page walk that has nothing to do with it and a slow host would set every page's
  latency. Draining into the cache first makes the REST cursor a plain resume point over local data — an opaque
  base64url-JSON ordering key, resuming strictly after the last row a page returned, so pages are stable under
  concurrent creation and deletion for the same reason the wire cursor is. The decoupling is enforced rather than
  intended: the helm's cursor carries a domain-and-version tag and spells its key's components differently, so neither
  decoder can read the other's tokens — without that they were byte-compatible and each silently resumed at a position
  the other had named.
- The page is a PAGE all the way down. The resume predicate and the limit go into one indexed query against helm.db's
  merged-order index, so a poll reads and JSON-decodes only the rows it returns; the alternative, loading the whole
  fleet's cache per request, made a full walk quadratic in the fleet's size. Two independent cuts apply, mirroring the
  supervisor's own list discipline: the caller's limit (capped, with an over-large request refused rather than silently
  clamped) and an encoded-byte budget that shrinks a page of fat records rather than oversizing the reply. The ordering
  key carries the host id as its final component so the order is total even where the one-owner index below is absent —
  a cursor over a non-total order can skip or repeat rows.
- At most one HOST may cache a given session id, as a schema invariant. Session ids are supervisor-minted UUIDs, so two
  hosts naming one is either a bug or a hostile supervisor claiming a session it does not own — and the consequence is a
  routing decision, not a display one: owner lookup would resolve one host while the list showed another's row, so a
  stop aimed at one machine could land on a different one. The first claim holds and the later claimant's row is
  dropped, so the LIST stays coherent; but while both hosts keep reporting the id, ROUTING fails closed naming both,
  because the helm has no basis for choosing which one the user meant. That contest is per-host REFRESH STATE,
  reconstructed from each drain's own evidence rather than remembered: it clears itself when a claimant stops reporting
  the id, goes with the host when it is removed, goes with the cache when an adoption purges it, and needs no schema to
  survive a restart — a restart forgets the marker and the next drains re-observe the collision if it is still real,
  which costs one refresh interval in which a genuine collision routes to the cached owner. A host that lists one
  session id twice in a single reply is a different failure: a list that contradicts itself is refused whole, and the
  previous cache is kept.
- The wire order is VALIDATED, not assumed. A drain rejects a list that is not creation-time descending with the session
  id ascending — within a page and across page boundaries — because this side does not merely display that order: an
  identity-less host's list is binary-searched for a resume point and merged in lockstep with the persisted page, and
  both are meaningless over an unsorted sequence. The failure would otherwise be silent pages that skip or repeat
  entries. Session ids are bounded at every peer ingress for a related reason: an id near the frame limit produces a
  cursor no client could replay, which would strand a walk at that row forever.
- Every mutation whose result changes what the helm has RECORDED records it before answering: a create seeds its new
  session, a restart and a rename store the reply's fresh `SessionInfo`, and a delete forgets the row. The merged list
  and the owner lookup are both served from those records, so a mutation that recorded nothing leaves the list
  contradicting the answer the caller just got — a session that cannot be operated on, a restart that still reads
  `exited`, a deleted row sitting beside its replacement. All of it is best effort and none of it can fail the mutation:
  the operation succeeded, and reporting a success as a failure is the one outcome SPEC.md's creation contract rules
  out. Each write carries the CLAIM its operation was routed under — a manager-wide connection token that is never
  reused, plus the identity — and is dropped if the connection has changed since, so a delayed reply cannot file one
  install's session under another's name. Writes are serialized against the host's own refresh, and a refresh whose
  drain predates one of them declines to commit rather than erasing it.
- One field of such a reply is NOT taken as given: a status of `unknown` never overwrites a definite one. The protocol
  is explicit that `ListSessions` is the only reply computing a real liveness answer and that everywhere else `unknown`
  means "not yet known" rather than "not running" — a create's and a restart's replies carry it deliberately, because at
  the instant they are built the pane exists but the agent's own exec inside it has not been observed. Recording that
  verbatim answered a successful restart with a badge saying the helm had no idea, for a session it had definite
  knowledge about a moment earlier. Keeping the previous value would leave it stale until the next refresh computes the
  truth, so such a write also WAKES that host's refresh — a refresh-only wake that cuts short the wait between drains
  and touches nothing else (distinct from the retry verb, which drops the connection and re-enters the retry ladder).
  The definite answer then arrives in one `ListSessions` round trip rather than one cadence interval, which is what
  keeps a restart of an exited session from reading `exited` afterwards. The wake is sent after the write's own epoch
  bump, so the drain it provokes is a post-write one and commits rather than declining as a pre-write snapshot would.
- Identity-less serving is only for a row with NO identity on record. A row that HAS one, meeting a peer that reports
  none, FREEZES in its own non-connected state (`identity-unverified`) and connects nothing. There is no identity to
  compare, so the mismatch check cannot see the situation at all — and connecting anyway would put an unverified peer in
  charge of a host whose cache, written under the recorded identity and still in scope for the list, describes a
  different install: the silent merge SPEC.md forbids, arriving through the one door that check cannot cover. The old
  cache stays and serves stale like any other non-connected host's, which is the honest reading — it is still the last
  thing this helm actually verified. Distinct from `identity-mismatch` because the remedy differs and offering the wrong
  one would be worse than offering none: nothing was presented, so there is nothing to ADOPT, and the ways out are
  fixing the host, retargeting the entry, or removing it. Re-probed automatically, unlike a mismatch, because there is
  no human decision available to wait for.
- Session operations route by OWNER LOOKUP in that merged view — from the cache's COLUMNS, never from the stored
  metadata, so a row whose payload no longer decodes still routes and a live session is never made unreachable by a
  corrupt copy of its own details. A session whose host is in any non-connected state is refused with the state named
  and nothing queued. Unreachable is not special-cased; a version-skewed, identity- mismatched, duplicate, or retired
  host refuses identically, because a caller that handled four of six would silently mis-handle the rest. The routing
  decision reads the host's state and its live connection from a SINGLE borrow of the actor's published status — split
  across two reads it could pair a fresh `Connected` with a dead connection, which is precisely how an operation gets
  routed onto a corpse. Creation takes the target host in the body (defaulting to the local row, the tail of SPEC.md's
  own creation default) and refuses a non-connected one as a precondition failure. Reading a session's DETAIL is the one
  route a non-connected host does not refuse: SPEC.md requires a stale session's metadata to be viewable behind the
  host-unreachable notice, so that read is served from the cache and marked stale, while a reachable host's detail is
  always fetched live — the cache exists for the stale list, not as a general serving layer. The live path drains the
  owner's list to exhaustion rather than reading one page, since a session sitting past the supervisor's default page is
  exactly the case a busy host has most of.
- Host management commits durably first and converges the live actors after, so each verb states how it fails closed:
  add rolls its row back if no actor could be started (a registered host with no actor is invisible and un-dialed, while
  its destination is taken); retarget converges instead of rolling back, because the durable write is what the user
  asked for and the actor can be told to reconnect through a path that cannot fail; remove tears the actor down by the
  id it just committed, needing no registry read that could fail. Retry reports whether it found a host, and a RETIRED
  host's retry respawns its actor from the current row — nothing else ever restarts one, so without that an actor that
  panicked left its host permanently dark. Adopting names the identity the user was shown and is refused if the host has
  since started reporting a different one, because a re-probe between the decision and the request would otherwise adopt
  something nobody approved.
- `--ensure-hosts <file>` is a JSON5 floor under the registry, applied through the same registration path as a REST add
  before serving begins and never consulted again. It adds what is missing and touches nothing else: an already
  registered destination keeps its fields and its learned identity, because helm.db is the durable authority and a
  startup file that overwrote user edits every boot would make the two fight. Validation is all-or-nothing — a malformed
  file, an unusable destination, or a destination listed twice fails startup with the entry named and nothing written,
  since a helm that came up with three of five guaranteed hosts looks healthy and is not.
- axum serving: REST for CRUD (sessions, profiles, hosts), a WebSocket event stream for live session-list updates, a
  WebSocket per attached terminal, and the static UI bundle. Loopback bind enforced — refuses non-loopback per SPEC.md.
- Web token: random 128-bit value minted on the helm's first run and stored recoverably in helm.db so `token show` can
  print it. Browser auth exchanges it once for a random 128-bit device secret returned in the response body; the browser
  keeps that secret in origin-scoped localStorage, whose origin includes the loopback port, and sends it explicitly as a
  Bearer credential on REST requests and a credential-bearing WebSocket subprotocol during upgrades. The helm stores
  only the device secret's SHA-256 hash, and rotation deletes all device sessions. This deliberately gives up HttpOnly:
  script execution in the authenticated origin can read the secret, but such a script can already drive the same API,
  while port scoping prevents an unrelated loopback service from receiving an ambient host-scoped credential. The
  loopback Origin guard remains defense in depth; no ambient browser credential remains, so this flow has no CSRF edge.
- The native app embeds farhelm-helm in-process and manages the bundled local supervisor; the Linux helm is the same
  code behind `farhelm helm run`.

## Logging

`tracing` everywhere, with `tracing-subscriber` env-filter semantics. The intended mature shape uses spans carrying
session and host ids, so SPEC.md's required diagnostic trails (creation, PTY lifecycle, attachment transfer,
reconnection, resume) fall out of structured context rather than ad-hoc log lines.

M1 emits structured lifecycle and failure events, attaching session or channel fields where that context exists. The
host half of the span discipline above now exists: M6's connection manager runs every actor inside a span carrying the
host id, kind, and destination, so the reconnection trail SPEC.md requires — connection attempts, phase transitions,
hello refusals, identity decisions (first contact, mismatch, adoption, duplicate), refresh outcomes, and recovery —
falls out of that context rather than out of per-call-site discipline. Two decisions are made by the manager rather than
by an actor, and therefore outside that span: adopting an identity and reconfiguring an edited host. Both attach the
same host metadata explicitly, so the trail has no gap where a user's decision should be. The destination is attached
per event rather than carried in the span, because a span's fields are fixed at creation and a retargeted host would
otherwise keep being described by the address it no longer uses — including in the very lines about reconnecting to its
new one. Phase transitions are logged when the phase actually changes, never on every republish, so a connected host
refreshing on its poll cadence does not bury the handful of lines that describe what happened to it. The session half
and resume's own trail are still later milestones.

Logs go to stderr, and deliberately not stdout: under `farhelm internal stdio` the process's stdout IS the protocol
channel, so a stray line there corrupts frames. File logging under `~/.local/state/farhelm/logs/` with rotation
(tracing-appender) and a `--log-level` flag are the intended shape but are not built yet — today verbosity is
`RUST_LOG`-style env only.

Motivation: tracing is the ecosystem standard, and span context is the cheap way to make "logs are available for X" a
property of the architecture instead of a discipline.

## CLI

clap (derive), one multi-call binary named `farhelm`, clean subcommand grammar. The user-facing surface:

- `farhelm helm run` — run the helm (flags: `--port`, `--state-dir`, `--ui-dist`, `--ensure-hosts <file>`). It takes no
  session or transport flags: M1's `--ssh`, `--cwd`, `--agent`, `--title`, `--remote-farhelm`, and `--remote-state-dir`
  were dropped with M6's registry (user decision 2026-08-04). A helm drives every registered host at once, so a flag
  naming one of them could only ever have meant the wrong thing; the last two live on as per-host registry fields, and
  creation is `POST /api/sessions`, which is where the host selection belongs.
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

ADD and UPDATE both retain that concrete plan behind an opaque, one-use confirmation id. Planning is inspection-only;
confirmation consumes the id, revalidates the host and registry facts the plan relied on, and only then admits the
host-scoped run. Discovery records the resolved supervisor binary, state directory, and identity together so a later
helm dials the same installation that answered the probe. UPDATE starts from those recorded coordinates rather than
assuming the standard layout.

Artifacts land under temporary names in their final flat directories and are atomically renamed into place. There are no
version directories or `current` symlinks: a failed transfer leaves the installed file intact, while a running binary
keeps its old inode until the explicit supervisor restart. Hash checks skip identical payloads and unit files are
written only when their content differs, so rerunning provisioning converges from wherever an earlier run stopped.
Matching content also repairs mode drift, and provisioning creates or repairs its directories with explicit modes; the
supervisor state directory is private to its user (`0700`).

The supervisor unit uses `KillMode=process`. Sessions started through Farhelm belong to the private tmux server that the
supervisor launches, so systemd's default `control-group` policy would kill that server and every session whenever an
explicit UPDATE restarts the supervisor. Limiting the unit stop to its main process preserves the same ownership model
as running `farhelm supervisor run` manually: stopping the supervisor detaches management, while tmux continues to own
the session processes and terminals until the user deletes them or the host reboots.

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
- **A fake agent** — `farhelm internal fake-agent --script basic|altscreen|binary|mouse-modes|spawn`, a hidden
  subcommand of the one binary rather than a separate artifact — stands in for Claude Code/Codex across this suite's
  integration and e2e tests. Its deterministic scripts cover prompt/echo input, terminal modes, alternate-screen
  rendering, byte-clean live output, and mouse-mode reporting, without vendor auth. Later milestones extend this fixture
  with fake on-disk records for status heuristics, conversation capture, and resume. The spawn suite also has an
  automated real-Claude leg that creates a jj workspace and spawns into it; CI leaves it gated because vendor
  credentials and network access are absent, and a developer enables it manually with `FARHELM_REAL_AGENT=1`.
- Rust integration tests exercise supervisor+tmux directly (CI provides tmux) and the framing protocol with golden
  cases; farhelm-proto keeps wire compatibility testable.
- **`node --test` unit-tests the asset-JS layer's pure functions**, under `crates/farhelm-ui/js-tests/` (outside
  `assets/` for source/test separation — bundling itself is by explicit `asset!` registration, so placement alone
  neither includes nor excludes a file). It currently covers `term-bytes.js`'s byte-domain conversion for
  `term.onBinary` — the byte-for-byte contract pinned at the boundaries (0x00, 0x7f, 0x80, 0xff), empty input, and a
  mouse-report-shaped sequence. Node's built-in runner over vitest/jest: node is already a CI requirement for
  Playwright, so this is zero new dependencies, and the asset-JS layer has no bundler for a module-tooling-heavy runner
  to pay for (PLAN_M6_5.md item 1).
- The desktop shell's native glue is the acknowledged manual-test gap (see GUI risks); everything else must be coverable
  without a human.

## Version and skew

One version number across the workspace; the protocol hello carries protocol and build versions; incompatibility refuses
with a clear error at the edge (helm↔supervisor connect, client↔helm load) per SPEC.md. Protocol version bumps with any
incompatible change — which includes a field whose omission changes what the receiver DOES, not only changes to frames
and message sets. A serde-additive field can still be semantically load-bearing: the non-displacing attach is the worked
example (a peer that ignores it displaces a client it was asked to leave alone, silently, on both ends), and decode
tolerance is why such a bump is required rather than why it is unnecessary.

The client↔helm edge has no hello to refuse at, so the helm stamps its build on every reply and the UI compares it
against the one compiled into its bundle. A mismatch — including a helm that reports no build at all — surfaces a reload
prompt and, more importantly, withdraws every UNATTENDED behavior that depends on the helm honoring this milestone's
vocabulary: the terminal heartbeat and automatic reconnect both stop, while anything the user explicitly asks for keeps
working.
