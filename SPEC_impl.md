# Farhelm implementation specification

NOTE: This documents implementation _choices_ in service of SPEC.md, together with the motivation for each choice so
future changes are made with the original reasoning in hand. It is not a build plan — sequencing, milestones, and PR
breakdown live elsewhere. SPEC.md defines what the product does; when this document and SPEC.md disagree about
observable behavior, SPEC.md wins.

Standing rule: the user-facing CLI surface described here (command names, subcommands, flags a user actually types) must
be kept in sync with SPEC.md wherever SPEC.md references it. Changing a command here means updating SPEC.md's mentions
of it in the same change.

## Language and runtime

Rust throughout, async on tokio, one cargo workspace. The old Dioxus UI's terminal island and its JavaScript left with
that UI; the GPUI app has no non-Rust runtime code.

Motivation: single language across supervisor, helm, CLI, and UI maximizes shared types and lets one test suite exercise
real components. tokio because the chosen web stack (axum, tungstenite) lives there; no exotic async needs exist that
would justify anything else.

## Who owns an accepted action

Once the helm or a supervisor accepts a request that changes state, the server owns carrying it out. The work runs on a
task the server owns, not on the task serving the client's connection, and that connection's handler only waits for the
result. A client that disconnects, reloads, or is cancelled loses the reply, never part of the work. This applies
wherever a connection's task can be cancelled: helm HTTP and WebSocket handlers, which the web framework drops when the
client goes away, supervisor request handlers, which are aborted after a grace period when the helm's connection closes,
and the helm's answers to agent requests relayed by a supervisor, whose read-only answers are aborted when that
supervisor's connection ends.

For authenticated helm HTTP routes, ownership is the router's default for methods other than GET and HEAD. It begins
after authentication and before typed request parsing, so a valid request can finish parsing and take effect after its
caller disconnects. Invalid requests still receive their existing rejection responses. Routes whose cancellation is
intentional live outside that layer: read-only POSTs, tab operations, transfers, and handlers with narrower admission or
owned-work boundaries documented at their registrations. GET, its implicit HEAD companion and WebSocket upgrades keep
their existing lifetimes. Desktop preflight and the standalone token exchange remain outside ownership.

Ordinary HTTP handlers rely on that router boundary rather than spawning an additional whole-handler task. Narrower
operations still own their work where they have non-HTTP callers or a different contract: template persistence and its
notification, provisioning service operations, preference consent cleanup and hint invalidation. Their existing task
error mappings and lock boundaries remain part of those operations; router ownership does not replace them.

The reason is that dropping an async task stops it at whatever step it had reached, while blocking work already handed
to other threads, such as a database transaction, still finishes. A handler written as a sequence of steps (commit a
change, then reconcile running state with it; mark a host busy, then start the job that clears the flag; start killing a
process tree, then relaunch) is left half done by a disconnect, in a state nothing later repairs. Tolerating that with
cleanup or recovery code at each step is the wrong fix; move the execution instead.

Stop and Delete in the supervisor already follow this, so disconnect cleanup cannot strand a frozen process tree. The
rule does not ask for work to outlive the process doing it: surviving a helm or supervisor restart is a separate matter,
covered by each operation's own recovery rules. Nor does it prevent an accepted action from being cancelled on purpose;
an explicit cancel, such as a Delete interrupting an upload, is part of the action's own logic, not a side effect of a
connection closing.

The helm and the supervisor each own their runtime and retain their state-directory claim until that runtime has
dropped, including on unwind, so a successor cannot share their state with blocking work still finishing during
teardown. For the supervisor that work includes the startup sweep of cached state and prepared checkouts, and a failed
create's file cleanup.

## Transactional database representations

A database may store multiple materialized representations of the same information, such as a session's full JSON record
alongside columns extracted for lookup, filtering, or ordering. This is an accepted implementation choice: writers must
derive mutually consistent values and update them together in the same transaction. Good test coverage of those writes
and relevant migrations establishes the invariant; readers may rely on it.

Sanity checks comparing representations that should agree are encouraged where they are extremely trivial and add no
meaningful performance cost or implementation complexity. They are optional. The absence of such a check, or unequal
coverage of these checks across read paths, is not itself a bug. Do not add repeated decoding, extra queries, recovery
machinery, or other overhead merely to detect hypothetical corruption of correctly maintained transactional data. Review
findings should identify a concrete writer, migration, or transaction-boundary defect that can violate the invariant,
rather than require every reader to re-prove it. This reliance on internal transactional writes does not remove
validation requirements at external input boundaries or excuse an established write-consistency bug.

## Supervisor metadata retention and nonresponse

The helm may retain a fixed amount of metadata about a supervisor indefinitely, including while that supervisor is
unavailable. Retention duration alone is not a leak or a bug: the relevant distinction is whether state accumulates
without bound over time, not whether an unavailable supervisor's metadata eventually expires.

Separately, defending against state accumulation caused by a malicious or buggy supervisor selectively failing to answer
requests is outside the implementation's requirements. For example, a supervisor may keep a connection alive while
leaving some requests unanswered, retaining their reply-routing bookkeeping. The lack of special cleanup, timeouts, or
quotas to prevent growth under that behavior is not a bug; do not spend implementation complexity on defending against
it. This exception does not excuse unbounded accumulation during ordinary operation with a correctly behaving supervisor
or failure to perform the specified cleanup when a connection is retired or a host is removed.

SPEC.md's "Remote input, session defaults, and availability" records the broader 2026-10-11 acceptance of denial of
service by a malicious or arbitrarily broken host; the examples here do not narrow that rule.

## Leftover files

Confirmed 2026-09-28: files left behind are not a leak, and need no extra cleanup machinery, when they are bounded by a
fixed amount, grow only with a rare and explicit user action (such as switching the release download source), or are
removed by the next attempt at the same operation or by the next start of the process that owns them. As with retained
supervisor metadata, the line is accumulation without bound over time during ordinary operation.

## Cleanup on quit

Confirmed 2026-10-20: when quitting leaves temporary files or other resources behind, the default fix is to clean them
up on quit. That holds when the quit skips the code that would normally do the cleanup, as the desktop app's Quit does
by ending in a hard exit that runs no destructors: the fix is to run that cleanup on the quit path. Do not instead add a
sweep that reclaims leftovers at a future start-up, and do not add inter-process locking (lock files, advisory locks,
ownership markers or anything else that coordinates between processes) or similar machinery to tell a live owner's
resources from abandoned ones. Either needs the maintainer's approval for the specific case first. That includes
covering crashes and force-quits, which cleanup on quit cannot reach: leaving those uncovered is the default, and a
sweep for them is an addition that needs approval.

The process that is quitting knows exactly which resources are its own. A sweep at the next start does not, so it has to
work out whether another running process still owns what it finds, which is where the locks, ownership markers and
compatibility cases come from. On 2026-10-20 a plan to reclaim the partial files that a desktop download interrupted by
Quit leaves in Downloads grew a per-download lock that way, then blocked over directories written by older builds that
the lock could not classify, when the app could have removed its own downloads' staging directories as it quit.

"Leftover files" says when files left behind are not a leak, and the next start of their owning process removing them is
one such case. It describes cleanup that already exists; it is not a reason to choose a start-up sweep over cleanup on
quit.

The desktop event loop's `LoopDestroyed` hook closes native-download admission and removes this process's live staging
directories before hard exit. Weak directory registrations preserve ordinary save cleanup; no transfer is awaited and
completed files outside staging remain. Earlier-build, crash and force-quit leftovers are not reclaimed.

Approved start-up sweep: terminal hosts. The maintainer approved one start-up cleanup for terminal hosts, recorded with
the terminal-host design as the user's decision "Approve startup reclaim": "at startup the supervisor shuts down any
live host whose tag names no known session or tab, and removes the sockets of hosts that are gone." Cleanup on quit
cannot cover this case: a host outlives the supervisor that started it on purpose, so no quit is the moment it should
end, and the tag a host carries already says which session and tab it is for, so telling an abandoned host from a live
owner's needs no new machinery. What the supervisor does under it: a session is known when the session database has a
row or a create reservation for it, or when the launch directory holds a host launch record named for it, and a tab is
known when its session's runtime holds it or a tab record is named for it (reading "known" by the records too is a
decision logged for this change, on 2026-11-02: records live beside the database, so a database that is removed,
replaced or restored from an older copy costs no running terminal). A live host whose tag names no known session or tab
is shut down, and its program gets the terminal's hangup; nothing else of it is looked for. A host this build cannot
read is never shut down, and nothing at all is done while the database has no session and no reservation, or when the
launch directory cannot be listed whole (a record not read would leave a known host looking like an orphan). Sockets
nothing serves are removed, and so are the launch records of hosts confirmed gone that no session in the database holds:
those of sessions known only by their records, and a known session's tab records whose host the restart found gone. A
live host of a session known only by its records is left at every start, with a warning each time: the session has no
row, so nothing in Farhelm lists or deletes it, and it runs until it is stopped by hand (SIGTERM to the process serving
the socket the warning names), after which the next start clears its record. The rule is in "Terminal host", under the
startup reclaim of orphaned hosts.

## Session IDs in logs

Session IDs must not inject log lines or use invisible or direction-changing characters to disguise the ID shown in
logs. Rejecting IDs outside plain printable ASCII is an acceptable way to meet this requirement; choose that option when
it is simpler. There is no requirement to accept or carry arbitrary Unicode session IDs through the system. Safe
escaping by the configured logging formatter also satisfies the requirement without a separate escaping helper at each
call site. Prefer the simplest implementation that meets this behavior; do not add duplicate escaping or new
infrastructure where the logger already provides it. This rule concerns session IDs, not a new general validation
framework for every externally supplied value.

## Workspace layout

- `crates/farhelm` — the single multi-call binary (see CLI).
- `crates/farhelm-supervisor` — session management, ending the old terminals at startup, terminal hosts' client and
  runtimes, agent-kind integrations, SQLite state.
- `crates/terminal-host` — the terminal host, a self-contained process per terminal that depends on no other Farhelm
  crate, with its own specification (see "Terminal host").
- `crates/farhelm-terminal-emulator` — the supervisor's terminal emulator for terminals in terminal hosts, and their
  snapshot format, over the patched crates in `vendor/` (see "Terminal host", "The supervisor's terminal emulator").
- `crates/farhelm-helm` — the host registry (SPEC.md's term: the helm's record of registered hosts and their SSH
  destinations), SSH transport, aggregation, axum API.
- `crates/farhelm-proto` — wire types and protocol version, shared by both ends and by tests; also the few HTTP tokens
  the UI branches on in the helm's replies.
- `crates/farhelm-desktop` — the desktop app's process (D6): the bootstrap that embeds the helm and starts or finds the
  local supervisor, the pre-window refusal surface, the updater and the app's state files, as a library the GUI is wired
  into, and the `farhelm-desktop` binary. It is the only crate that depends on both the helm and the GUI ("Where the GUI
  meets the helm"). Until the GPUI app is wired in, the binary runs the bootstrap without a window. It is excluded from
  the workspace's `default-members`, like the GUI crates, so `-p farhelm-desktop` or a `--workspace` run is what
  compiles it.
- `crates/gui/` — the GPUI GUI crates (see "GUI architecture"). `farhelm-gui` also holds the harness marks' geometry as
  plain data (`docs/harness-marks.md`).

Motivation: the proto crate is the seam that keeps helm and supervisor honestly decoupled (they meet only over the wire,
even in-process). `farhelm` remains the single multi-call artifact provisioning ever has to move — farhelm-desktop is a
second release binary for the Mac desktop alone, never provisioned to a host.

## GUI architecture

This section governs the GPUI desktop app (macOS and Linux) that replaces the Dioxus GUI. Where a rule names an owner
that is not built yet, its path is recorded in the foundation ownership table (`docs/gpui/foundation-ownership.md`) when
it is built.

This is a rulebook in the style of SPEC.md's "Keyboard focus": each app-wide concern has one owner, a rule, and the
mechanism that enforces it. The old GUI decided these concerns one feature at a time (focus, popover placement, late
replies, escaping, drafts), and an audit of its fix history found most bugs were a rule one path followed and a sibling
path missed. GPUI adds a class of its own: it redraws a window when an entity it read while drawing notifies, but it
cannot see a value a view copied out of shared state, nor a change made without a notification, so either goes stale
silently. The answer in both cases is the same. Build each mechanism once, route every feature through it, and make
going around it fail a lint, a type check or a test rather than a reviewer's attention.

Here's the short version: the GUI talks to the helm only through one ordered UI channel, plus terminal streams and plain
HTTP for transfers. The store holds what the helm pushed, and views draw from it. One owner each moves focus, draws
overlays, asks questions, holds pending work, decides notices and runs animation time. On any break of the channel the
whole UI resets.

Port, not redesign: the GPUI app copies the old app's layout, spacing, colors and behavior as of reference commit
`24bc0580`. Read old behavior from git at that commit and check screens side by side with the reference screenshots.
Text renders differently in GPUI, so pixels will not match; a deviation that cannot be avoided is recorded and surfaced
to the maintainer, never decided quietly.

### Where the GUI meets the helm

GUI crates do not depend on the helm crate. Everything the GUI knows about the helm arrives over the UI channel, a
terminal stream or plain HTTP, even in the desktop app, which runs the helm in its own process. The desktop binary that
embeds the helm is the only place both meet. This keeps the helm/UI split real: a remote helm stays possible, and the
GUI cannot grow a dependency on helm internals that would make it impossible.

GUI crates live under `crates/gui/`, and so do the framework-free GUI crates (the terminal grid model, input mapping,
the escape table, the store and drafts logic), so every ban covers them too. Framework-free crates have no `gpui`
dependency and can be tested without a window. Of these, only the escape table is shared with the command line. The
desktop binary, which embeds the helm and holds the bootstrap, menus and quit, stays outside `crates/gui/`; it and the
helm client deny `unwrap_used` and `expect_used` in their own `[lints]`, like the GUI crates. Network I/O (the UI
channel's socket, terminal streams, HTTP transfers) lives in the helm client, a framework-free crate outside
`crates/gui/`: it is the one runtime owner, owning the async runtime and its threads, and it hands results to the GUI
through GPUI's executor. The helm client is the GUI's one exemption from the ban on threads and other runtimes, and it
counts as a lint exception like any `#[allow]`; no other crate the GUI depends on gets one.

Enforced by `scripts/check-crate-layering.py`, which reads `cargo metadata` and fails when a GUI crate depends, directly
or transitively, on the helm crate, when a framework-free crate depends on `gpui`, or when a GUI crate, the desktop
binary's crate or the helm client stops denying `unwrap_used` and `expect_used`.

### The UI channel

Owner: the UI channel, one WebSocket per UI. It carries everything except bulk data, in one ordered stream:

- commands from the GUI, each an envelope of `{id, method, path, body}`;
- the helm's reply to each command, with its status and body: done, refused, or accepted (for long work);
- state the helm pushes: a full snapshot of the pushed collections when the channel opens, then changes per collection.

The channel is the WebSocket `GET /api/ui` (`farhelm_proto::ui_channel::UI_CHANNEL_PATH`), and its wire types live in
`farhelm_proto::ui_channel`. The session-list query is the channel-local command `PUT /ui/session-query`, which the
channel handles itself and never routes. Rename, the ordinary session delete, Stop, Restart, Replace and the ordinary
create bound their host wait at 20 seconds (`HOST_REPLY_BOUND` in the helm's sessions module) and then reply that the
host timed out while the work continues; the GUI's watchdog therefore waits longer than that. The launcher's host reads
(Browse and its folder check, the checkout preview, the repository search) stop waiting at the same bound and drop the
read, since nothing is left to finish; the host's own scan may still run as long as SPEC.md's "Slow hosts" allows, and a
repository search that times out keeps its recent repositories. Host management's routes that can wait as long answer
within the same bound, each saying where its late outcome shows: a host probe (a supervisor that answers late is still
registered, so it appears in the pushed hosts list; otherwise the user probes again), an Update's plan (nothing changed,
so the user asks again), and a destination or alias change, which waits for any setup or update running on that host
(SPEC.md, "Waiting between operations on one host") and lands when the run ends; those two first refuse at once what the
registry already refuses, so the reply that they are still waiting is not given for a change that cannot land as things
stand. Environment and sandbox deletes, Suspend, Resume and the uninstall plan are not bounded yet and are bounded, or
take the accepted form, when the work that ports them needs it.

The helm runs each command through its existing HTTP router in process, so a command gets the same handlers, validation
and authentication middleware as an HTTP request. There is no second, typed command API. A route that genuinely cannot
work this way is recorded as a decision with the reason. The router's existing contracts hold: the cancellable group in
`api_router` is dropped when its caller goes away, as today, and work behind `own_state_change` finishes after a reset
(see "Who owns an accepted action").

The router has one client. After the browser UI is gone, only the GUI calls the HTTP API, plus terminal streams and file
transfers. The command line and agents reach the helm through the supervisor relay and the shared `do_*` handler
functions, with their own credentials and approvals. A route may therefore change to its accepted form outright, with no
second blocking mode kept for a client that does not exist. When a shared `do_*` function changes behavior, check the
relay path that also calls it.

Commands are admitted in the order they arrive. The channel's owner starts each command only after the previous one has
replied (done, refused or accepted), and it serializes replies and pushes onto the wire. This is what makes the stream
ordered: without it, a quick launch could overtake the template save sent just before it, which the old GUI had to fence
by hand. Heartbeats bypass the queue.

Serial admission only works if every handler replies promptly, so every handler does:

- A timeout against a host is an ordinary reply ("the host timed out"). The helm's links to supervisors stay fallible,
  and the user sees that, but it never leaks into the transport. Bounding a handler's host wait is preferred over adding
  an accepted form.
- A command whose normal duration can exceed the host-wait bound uses the accepted form: provisioning, host update,
  trash deletion, recipe builds, and sandbox and container (environment) session creates, which take minutes. Every
  other command bounds its host wait. A later click therefore waits behind at most one bounded command, never behind a
  long launch. Each route converted to accepted names the state that carries its eventual outcome (success, partial
  result or refusal). Reuse existing domain progress where it exists; otherwise the outcome goes in one bounded
  operation-result collection keyed by envelope id, with explicit completion and removal rules. "Accepted" settles the
  transport watchdog. It does not mean the work succeeded.
- Routes are converted as the work that first ports them needs them, not in a sweep.

Every change a command causes is pushed before that command's reply. When the GUI sees "done", its store already shows
the result, so the GUI needs no optimistic updates (the sidebar's width while the user moves it is the one exception,
under "The store, and how views redraw").

### What is pushed, and how changes are found

Owner: the UI channel.

Pushed collections are the helm-local data the fleet feed already invalidates: sessions, hosts, templates, preferences,
notices, approvals, announcements and host progress. The PR that adds a collection lists it. Each is read through its
GET route: sessions keyed by their served id, templates by name, notices by id, approvals by id, hosts by registry id;
preferences and the announcement queue travel as one entry each; host progress is `GET /api/provisioning`, every run
this helm process retains, keyed by host id (a host with no run is absent, not idle). Their entry types live in
`farhelm-proto` beside the channel's. The snapshot sent when a channel opens never waits on a remote host. Every other
read (directory browse, repository lists, the trash list, launch history, file stat) is an ordinary GET command whose
reply body is the result. Data the old GUI polled stays a query that the GUI issues again; it does not become a
replicated collection.

The session list is a query. Its filter, sort and cap are this channel's state, set by a command, and the helm keeps
serving the filtered, capped view with its `total`, `matching` and `truncated` counts. A channel starts unfiltered, in
the list order stored as the shared preference (most recent activity when none is stored or this build does not serve
the stored word), so its first snapshot, a reset's included, is already in the user's order without a query from the
GUI. Until a query from the GUI names a sort of its own, the channel keeps following that preference, so a change made
elsewhere reorders the open list. Collections carry their metadata: completeness or unavailability, counts and order.

Changes are sent per collection, keyed by id, and an entry is always replaced whole; there are no field-level patches.
The GUI applies every collection's changes with one generic piece of code, not code per kind. The helm finds changes by
re-running the collections' existing GET handlers in process, after every fleet feed bump and after each command's
handler returns, and diffing against what it last sent this UI. Only successful, comparable projections are diffed: a
failed or partial read is never an empty collection and never evidence that something was deleted.

The fleet feed stays an invalidation hint that carries no data. No typed per-mutation events are added. Every mutation
of a pushed collection bumps the feed at its shared mutation boundary, including agent-relay writes and supervisor
updates, so the projection runs whoever made the change.

Wire types live in one framework-free crate (`farhelm-proto` holds shared types today): the envelope, the push messages
and every collection's entry type. The helm serializes them and the GUI deserializes the same types. There are no
hand-written mirrors on either side; two copies of one type that drift apart is a documented bug class of the old GUI.

Enforced by a round-trip contract test per collection, tests that a preference patch and an agent-originated change each
reach a connected UI, and, in GUI tests, a consistency check at quiet points (after each test step) that compares the
GUI's copy with a fresh full snapshot and fails loudly on a mismatch.

### Paths are a trust boundary

Owner: the command path type, `farhelm_proto::ui_channel::ApiPath`, which both sides of the channel use.

GUI commands are built only through one path builder that percent-encodes each segment. Ids that come from a supervisor
(tab ids, for example) get no check on the helm side, so an id like `../../victim` must never be able to retarget a
request. The helm refuses non-canonical paths: dot segments (plain or percent-encoded), empty segments, fragments, and
characters no URI carries. Refusing is decoding: a command whose path is not canonical does not decode, so it never
reaches the router, and the helm closes the channel as for any message it cannot read. A correct GUI never meets this,
because its builder cannot produce such a path, and the type has no public constructor from a string. A segment holding
an encoded `/` or `\` is canonical (a host's id may contain one), so it decodes; the dispatcher refuses that command
with an ordinary reply and the channel continues, so a host-supplied id cannot reset the GUI.

Enforced by that type being the envelope's path field (not a plain string feature code can fill in), with the builder
and the decoding check in one place so they cannot drift apart, by tests of each refused form and that every built path
decodes back unchanged, and by a test that a command with a non-canonical path ends the channel unrouted.

### One failure path: the reset

Owner: the UI channel's watchdog, and the UI-session owner.

A heartbeat, plus one watchdog that treats any command left unanswered for N seconds as a broken channel, replaces every
per-request deadline. The heartbeat is the GUI's: the helm sends no keepalive of its own, and closes a channel that has
sent nothing for about a minute, so a GUI that vanished without closing gives up the seat. A command's clock starts when
the channel admits it (when the previous command's reply arrived), not when the GUI sent it, so a queue behind a slow
command does not trip the watchdog. N is set above the helm's own bound on host waits. That bound is also the longest
any queued click can wait behind a slow host, so it is kept short (the exact value is chosen with the channel's
implementation). However the channel broke, the same code runs: the window goes inert behind "Reconnecting to the
helm…", everything that came from the helm is dropped, and the window rebuilds from the new snapshot. One generic notice
says the connection was lost and that something done just before may or may not have happened. No action tracks its own
fate.

In the embedded topology the UI reconnects to the same helm. If the helm task itself has ended, the app shows the
existing startup-refusal surface and exits; there is no in-process helm restarter. A refused credential shows a terminal
"cannot connect" state with Retry, not a reconnect loop. The UI has at most one connection attempt at a time.

A reset drops the UI's async owners, not just its data. Transient stores, and their UI-side tasks, subscriptions and
callbacks, live under one resettable UI-session owner. On reset that owner is dropped and its queued callbacks are
rejected before the new snapshot is installed. A view that acts on pushed state as it changes, rather than only drawing
it, must move its store subscription to each new session's store and drop the old one, so it neither goes on observing a
retired store nor misses the store that replaced it. New views do that through the owner's helper
(`session::observe_current`); the session list and the session pane still do it by hand, keyed on the session
(`PerSession`). The one survivor the UI keeps itself is the selected session, by id, if the new snapshot still has it;
it lives in a small foundation of its own outside the UI-session owner (`crates/gui/farhelm-gui/src/local_state.rs`,
owned paths in `docs/gpui/foundation-ownership.md`), which is not a device-local preferences store and has no room for
one. The window itself, its size included, is not part of the UI session and is untouched. The settings the old app kept
per device (appearance, sidebar width, host rail, status sounds, terminal text size) become helm preferences in the new
app, so they return with the new snapshot like the rest of the helm's state (maintainer decision, 2026-11-13). SPEC.md
no longer calls the sidebar width per device; it still calls the others per device until the changes that move them. The
store fed by the channel holds only helm state. Half-typed drafts do not survive, and neither do open dialogs and menus:
going inert closes every layer through the layer manager (`layers::close_all`). Menus would close anyway with the ground
they are anchored to, but a dialog closes only when its opener or its subject says so, and one left open would keep its
veil over the inert window, taking the clicks and Tab presses meant for Take back or Retry. Work the helm accepted
continues independently of the reset.

What never gets written again on the GUI side: lost-reply reconciliation, retry keys, request fences, outcome-unknown
reports, and their tests. On the helm side only machinery that existed solely for the old GUI goes; idempotency the
command line or agents use stays (the launch idempotency keys in `crates/farhelm/src/agent_client.rs`, for example).

Enforced by a test of a completion queued across a reset (it must not reach the new UI session), a test that a dialog
open at a break is closed and Take back still takes a click, and by the review gate.

### The seat: one UI per helm

Owner: the UI channel.

The helm accepts one UI channel. A new one displaces the old; the displaced window shows "another window took over" and
does not reconnect on its own. Only the live channel's "displaced" message counts as a takeover. The displaced window is
the reset's inert state, with that message in place of "Reconnecting to the helm…" and no automatic reconnect, plus a
"Take back" control, which runs the ordinary connect: there is no separate machinery. Like a reset, it tears down its
terminal attachments and their automatic reconnect workers, and none of them restarts until the user takes the seat
back.

Enforcing the seat is a small check when a channel is accepted. If it ever needs more than that, enforcement is deferred
(the maintainer accepted that) while the code keeps assuming a single UI and adds nothing to cope with several.

Whether a window is open, for agent approvals, is the live seat, released on disconnect and on takeover. It is not a
count of feed subscribers: the helm's own internal subscriptions to the feed never count as windows.

Enforced by tests of an agent request with a connected UI, after a disconnect and after a takeover, and a test that a
displaced UI cannot take a terminal back.

NOTE: Interim, until the desktop app's approvals area lands (SPEC.md, "Agent-spawned sessions", decided by the
maintainer on 2026-11-13): in the embedded helm a connected window does not get cards, so a request that needs approval
is refused at once, with its own message, instead of waiting. It is one switch, `window_without_cards`, set by the
embedded helm's startup and read only where the approvals module decides whether to card a request; that module's docs
say how to remove it. A standalone `farhelm helm run` keeps the card path, which only the end-to-end tests reach,
through a stand-in UI on the test credential; a remote helm a desktop app attaches to would need the switch too.

### Bulk data stays separate

Terminal streams stay one stream per terminal, with their own reconnect handling, because the supervisor and host behind
each can fail independently of the helm. File uploads and downloads use plain HTTP; a failed transfer is that transfer's
own visible outcome, and the user retries it. Neither touches the command path, so a large upload never delays a click.

### The store, and how views redraw

Owner: the store.

The store is fed only by the channel. Views read from it when they draw; a view never keeps its own copy of shared
state. Shared state changes only through store methods that notify. Caching a view goes only through one wrapper that
computes the view's inputs from the store and redraws only when they differ, so a forgotten notification cannot leave a
cached view stale. A child view talks to its parent only by emitting events that the parent subscribes to; a child never
holds a parent handle to call `update` on it (see the re-entrancy trap below).

A session terminal is the one deliberate exception to store-fed, wrapper-cached views (the migration's orchestrator
decided this on 2026-11-13): its grid is bulk data streamed for one attachment, not shared state, and its inputs are the
whole grid, which the wrapper's comparison of computed inputs cannot express. The terminal view
(`crates/gui/farhelm-gui/src/terminal.rs`) owns its grid model, is never cached, and notifies itself through the methods
that change what it shows; render-count tests pin that shown output redraws it and output it does not show does not.

The sidebar's width is the one value a view shows ahead of the store (decided by the migration's orchestrator on
2026-11-14; `crates/gui/farhelm-gui/src/sidebar.rs`). It covers only the width the user is moving, and only until its
write's reply: during a drag, and from each divider write until the helm replies to it, the sidebar shows the width the
user set rather than the stored one, because falling back to the stored width for the round trip would snap the sidebar
back and resize the terminal twice, and a second arrow press would step from the stale width. Only the latest write's
reply ends it, a refused write falls back to the stored width, and a reset forgets it. No other view keeps such a copy.

The session list builds only the rows in view and a few beyond its edges (GPUI's variable-height `list`), because an
uncached view re-renders on every window redraw and the running pulse and the terminal's cursor blink redraw the window
several times a second: built whole, 200 rows idled at about 12% of a core on the test Mac (2026-11-14), against about
2% for five. SPEC.md's "served and rendered WHOLE" rules out paging, cursors and incremental listing, and none of that
changes: the helm serves the whole capped fleet, the store holds all of it, and every row is reachable by scrolling.
What does change is that assistive technology and the test bridge see only the rows drawn.

Enforced by tests that count redraws per view (an app-level render counter compiled into test builds; no GPUI patch), by
a test for each store that a change through its public methods redraws the views that read it, by clippy
`disallowed-methods` on GPUI's view caching (`Entity::cached` and `AnyView::cached`) everywhere in the GUI crates except
the cache wrapper, and by the per-PR review gate.

### Focus: the focus coordinator

Owner: the focus coordinator. It implements SPEC.md's "Keyboard focus" as one module: the "user moved on" counter, home
(the selected session's terminal), the dialog stack's trap and restore, and the record of who opened what.

It is the only Farhelm code that moves focus. In GPUI, moving focus also redraws the whole window and drops pending
keystrokes, which is one more reason for one owner. While a dialog is open, focus moves only inside it: the one move
between parts of the topmost layer (along a radio group, into a question drawn inline in the dialog and back out)
happens only from the control that holds focus. GPUI Kit moves focus inside its own text inputs on user clicks; those
moves are accepted, and the per-step focus invariant covers them.

GPUI also focuses any element that tracks a focus handle when it is pressed, so tracking focus goes through the
coordinator too (`focus::track`), which applies its own press rule in place of GPUI's. An element that takes focus when
pressed (the terminal, a dialog's empty space) takes it only from a press that did not move focus into it, because GPUI
hands the press to the controls under the pointer first: a click into a dialog's text field is the field's, not a click
on the dialog's empty space. What tells them apart is where focus went, not a prevented default (GPUI's usual signal),
because the coordinator prevents the default on every control to keep GPUI from focusing it, including a macOS button
that a click leaves unfocused. A move a closing layer makes during the same press (focus going back to a menu's opener
when the press closed the menu) does not count as the press being taken, so a click on a dialog's empty space focuses
the dialog whether or not a select's list inside it was open. Which controls Tab reaches follows the platform (settled
for the GPUI migration on 2026-11-13): on macOS the system's Keyboard navigation setting, which the desktop binary reads
and the coordinator asks on every Tab and Shift+Tab press, so a change applies at once (off, the default: Tab reaches
text fields and pop-ups only and Option-Tab reaches every control; on: Tab reaches every control); on Linux, every
control. Key presses count toward "the user moved on" where GPUI shows every keystroke before any binding runs: a key a
binding handles (Farhelm's Tab and Escape, GPUI Kit's editing keys in a text input) never reaches an ordinary key
listener.

A focus move made by the keyboard or by the app, never by a pointer press or hover itself, scrolls every scrolling
container around the focused control that opted in (`focus::scroll_region`, as the dialog frame's body and card do),
innermost first, by the least distance that shows the whole control, as a browser's scroll-into-view does. It works from
where controls were last drawn, moved by any scroll since, and asks for its own frame; virtualized lists keep their own
reveal. A control focused before it is first drawn (a layer's start) has no position and is not scrolled to; its owner
scrolls it, as Settings' trust hints and the archive's global confirmation will need to (SPEC.md).

Enforced by clippy `disallowed-methods` on GPUI's focus calls and on `track_focus` everywhere in the GUI crates except
the coordinator, and by the focus invariant that in-process tests check after every step: exactly one focused element,
inside the topmost modal (or a menu opened above it) or in an agent approval card if a modal is open (SPEC.md, "Keyboard
focus"). When nothing else holds focus it rests where SPEC.md's "Where focus rests" says (the selected session's
terminal, New, a stopped session's card), and on the window's root view only when none of those is on screen, so a key
press always reaches Farhelm's own key bindings rather than GPUI Kit's.

### Overlays: the layer manager

Owner: the layer manager. It owns the dialog slot, the popover stack and tooltips.

Every overlay is anchored to an element, not to coordinates (SPEC.md has no menu opened at a pointer position). Escape
from a focused terminal goes to its program (SPEC.md); otherwise it closes only the topmost layer, unless a mode SPEC.md
says captures the keyboard (the quick-launch sequence) is armed, or the topmost dialog's dismissal hook keeps it open,
when Escape closes nothing, not even a tooltip: an editor whose save is in flight stays, and an editor leaving an
unsaved draft stays and asks first, then closes itself if the answer says to leave. A question drawn inline in a dialog
takes Escape first while focus is in it, as its Cancel, so the dialog stays open. A dialog's own Close control goes
through the same hook; a close its opener makes, a subject going and a reset do not ask. A dialog that asks for it (the
quick switcher, as in the old app) also closes on a press on its veil outside it, through Escape's path, so the same
hook decides; any other dialog's veil only takes the press. Dialogs are centered, or near the window's top for an opener
that asks (the quick switcher again, so it does not move as its results grow). An overlay closes when its subject is
gone, not when it moves; anchoring to the element makes a moved row's menu follow the row. A popover is as wide as its
content, or exactly as wide as its anchor when its opener asks (a select's list, a list floating under its field, as the
old launcher's were `left: 0; right: 0` under theirs), taken from the frame being drawn so a resize never leaves it at
the old width. A popover may also never take focus: opening it moves none, it traps no Tab, closing it returns none, and
a press on it leaves focus where it was, so the field the user types in keeps the keys, as with the old launcher's
floating lists, whose options were never focusable. Escape, an outside press and its anchor going close it as any
popover. The launcher's floating lists and the terminal's overlays on a zero-size anchor element use it, and wherever
the focus rules say "the topmost layer" (Tab's trap, where focus recovers) they mean the topmost layer that holds focus,
which such a popover never is. Whether a click on a dialog's empty space (label text included) keeps focus on the
control in use is a property of each dialog, set as SPEC.md's list says; a dialog without it takes focus itself, so
Enter then presses its main action. Whether a terminal link's destination disclosure is covered is a hit test, never a
hand-kept list of covering surfaces. Things the user did not start never open a layer over what they are doing (SPEC.md,
"Keyboard focus").

Enforced by clippy bans on GPUI's `deferred` and `anchored` outside the layer manager; by `disallowed-types` on GPUI
Kit's Dialog, Sheet, Popover, HoverCard, PopupMenu, ContextMenu, DropdownMenu, Notification and Tooltip, and its Select,
Combobox and DatePicker, which draw their dropdowns through `deferred` and `anchored` (dropdowns go through the layer
manager instead); by `disallowed-methods` on Kit's `open_dialog`, `open_sheet` and `push_notification`, the
`.tooltip(..)` methods on Kit controls, and the `context_menu` and `dropdown_menu` extension methods; and by the overlay
invariant checked after every test step (every open popover's anchor was drawn, and a popover that never takes focus
does not hold it). The Kit bans apply inside the component layer too: only the layer manager draws overlays. Kit Input's
built-in right-click menu is disabled, matching today's app, whose release builds have no context menu.

### Questions: the prompt

Owner: the prompt, the one way to ask the user a question.

A confirmation freezes what it shows when it opens: targets, consequences and preconditions. Its answer buttons come
pre-wired to that frozen state, so feature code never wires an answer by hand. Commands name their targets by stable id,
never by list position, and the helm refuses a target that is gone. The existing per-route refusals of a target that
changed since its prompt stay (a delete or restart that goes beyond what its confirmation said is refused, commit
`6aef8dc2`). No new generic precondition protocol is added; new precondition checks go only where SPEC already requires
them.

This is the structural fix for the old GUI's largest bug class: an action that ran on state the user was not shown.

SPEC.md's "One GUI at a time" (2026-10-10) lets input delivered before a redraw run as delivered, so a Confirm queued
behind a Cancel may still submit. What the prompt guarantees is that whatever runs acts on the state the confirmation
froze when it opened, never on state the user was not shown.

A question has Cancel and its confirming answer at the ends, and may carry extra answers between them, in the order the
feature adds them, for the questions SPEC.md asks with more than two (the template editor's "Keep editing, Discard, or
Save", the host confirmations' permanent answer, the YOLO confirmation's guarded and permanent answers). Each extra
answer follows the same rules: it acts on the frozen value, the first answer pressed wins among all of them, and focus
never starts on one. Cancel may be relabelled ("Keep editing") or given its own hover help without changing what it
does.

A question is centered over a veil, anchored beside the control it is about, or drawn inline inside the surface that
owns it (the launcher's YOLO question in its body, the template editor's leave question at its foot), with the same
rules in all three. An inline question is not a layer: its owner decides where it is drawn, and the prompt decides how
it looks and answers. It takes focus on its starting answer when it opens, unless the user has moved on, and an answer
returns focus to what held it before, inside the owner's dialog. Escape on it is Cancel and does not reach the owner's
dialog. It draws plain, or loud as the YOLO confirmation is (decided by the migration's orchestrator on 2026-11-14).

An answer whose action can fail keeps its question open, every answer frozen, until the action reports how it went:
success closes the question as any answer does, and failure shows the reason inside it and arms the answers again
(SPEC.md: the YOLO question "stays up with the reason" when stopping to ask fails). Focus stays on that answer
throughout. While the attempt is unsettled, the owner's dismissal hook keeps the owner open, as an editor whose save is
in flight stays, so the owner cannot close under the running action. Only an inline question may carry one: its owner
decides its own dismissal, while a question shown as a layer can also close on a press outside it or its subject going,
which nothing can veto.

Enforced by the prompt being the only type that renders answer buttons for a question, and by a generic test, for every
prompt kind, that changes the confirmation's subject after it opens, then sends its answers (Cancel, Confirm and an
extra answer) with no draw between them, and checks that any command sent names the frozen targets and preconditions.

### Work that finishes later: the operation stores

Owner: the operation stores, app-level stores that outlive the views that start work.

Task handles are stored under the operation that owns them. "Latest wins" applies only to work that is explicitly
supersedable for the same resource, such as a search being retyped. Independent actions, such as a Stop on one session
while a Delete on another is in flight, each keep their completion handling until they settle or the UI resets. Dropping
a waiter never cancels work the helm accepted. A completion that would focus, open, select or close something asks the
focus coordinator whether the user has moved on.

The store files each piece of work under a typed key naming what it acts on: a session, a host, a template, the
preferences, or a launch or a host being added, neither of which has its id yet. A key admits one request at a time, and
keys of different kinds never collide, so a host and a session that share an id string stay independent. The command
itself is built by the feature that sends it, with its route's API semantics; the store knows no routes. A reply settles
as done, refused, or accepted (202: long work the helm carries on with; the store keeps no result for it, and for host
setup, Update and Uninstall the result arrives in the hosts' pushed progress). An accepted save leaves its draft
touched, since the stored value has not changed yet.

A read (a GET for something the helm does not push) is not work on a key: it changes nothing, so it has no busy state
and leaves no refusal on a key. A feature's read goes through the store's one read helper (the quick switcher's, written
before the helper, is the one still to move onto it), which words a refusal exactly as an operation's is worded, so a
failed read and a failed save say the helm's reason the same way; the feature shows the result itself. A reset drops a
read's wait like any other.

Feature code does not detach tasks. GUI crates do not spawn threads or other runtimes' tasks, sleep, or read the system
clock; time comes from the ticker and async work from GPUI's executor, which keeps tests deterministic. The runtime that
network I/O needs lives in the helm client, outside `crates/gui/` (see "Where the GUI meets the helm").

Enforced by clippy `disallowed-methods` for detaching, thread and runtime spawns, sleeps and clock reads in the GUI
crates, by a test of overlapping actions on two sessions, and by a test of work on a host and a template beside a
session's, each with its own busy state and refusal, and by tests of a read's wording and of its send and reset.

### Drafts

Owner: drafts.

Each draft field records whether the user touched it, and a pushed change never overwrites a touched field. Saves keep
today's API semantics. Partial saves are used only where the API already supports them, or where SPEC.md's "a GUI write
must not undo a change an agent or the command line made" needs them. The two confirmed last-write-wins exceptions
(templates and number keys, SPEC.md 2026-10-21) stay. A failed save is reported, never retried, and never treated as
saved; whether the field reverts or keeps its text for editing follows that editor's SPEC rule (several explicit-save
editors keep the draft). A save is work in the operation store under the key of what it saves, so an editor of any kind
(a session's title, a template, the preferences, a host's settings) saves the same way. A text field over a draft is one
binding owned by the drafts module, not wiring each editor repeats: it mirrors the user's edits into the draft, shows a
pushed value only while the draft follows it, is read-only while its save is in flight, and after a refused save takes
up the last pushed value if the user had not touched it. While a save is in flight its Save button is busy rather than
disabled: it keeps focus and its place in Tab's order and ignores presses, as the old app's Save did, because a disabled
button gives up focus.

Enforced by draft tests that push a change under a touched field, by save tests that load, save untouched and compare,
by a save under a template's key, by tests of the bound field (a push under a touched field, a push and a keystroke
while saving, the refusal follow-up), and by a test that a busy button keeps focus and ignores presses.

### Host-supplied text

Owner: host text.

Host-supplied text is its own type from the moment it is decoded. The only element that can draw it escapes hidden
characters and isolates text direction, using one escape table shared with the command line. This covers host-authored
text shown anywhere outside the terminal, including link disclosures and accessible names. Terminal grid content keeps
its own rendering contract (SPEC.md: "Terminal output is not part of this rule"). The only strings that come out of host
text are its escaped presentation, used for display and accessible names, and its raw value, used for editing and
operations. Raw values keep their bytes, and an escaped presentation never becomes a saved value.

Direction isolation is structural: every piece of host text is laid out as its own text element, never concatenated into
a string with the app's words, so a right-to-left value reorders only within itself, as HTML's `<bdi>` did in the old
app; a sentence that mixes both keeps each part its own element. The table lists Unicode's whole format category, which
the old app's terminal-link disclosure escaped, so that disclosure uses it too.

Hover help may name a host value (the agent mark's, the permission shield's) only as a host sentence: the tooltip draws
it through the escaping element, and the control's accessible description is the sentence's accessible text, the same
words with each host value escaped.

Enforced by the type: host text has no other conversion to a string, and the component layer accepts it only through the
escaping element. A corpus of spoofing strings is pushed through the host-text element and the views that render it in
the slice.

### The component layer and theme

Owner: the component layer, over GPUI Kit.

Component constructors require a label, hover help and an accessibility role, so the old rule that every control has a
tooltip becomes something the compiler checks; the test bridge also reads roles and labels to find controls. Where the
old coverage rule exempted items that carry a visible description, a constructor's description counts as its hover help.
One exception to keyboard reach per control, decided by the migration's orchestrator on 2026-11-14: a list's options
driven by the field that owns them (`OptionRow`; Up and Down move the selection, Enter picks, focus stays in the field)
are reached through that field and never take focus, as the old quick switcher's rows did. The next such list reuses
`OptionRow`. A second exception, decided by the migration's orchestrator on 2026-11-14: a pointer shortcut inside a
control, whose action the same control offers by keyboard elsewhere (its row menu item), may take no focus
(`MarkToggle`, a session row's status dot that marks it read or unread). Colors come only from theme tokens, which are
copied from the old stylesheet (`crates/farhelm-ui/assets/app.css` at the reference commit). One dialog frame keeps the
answer buttons pinned and scrolls the body, and when even those do not fit, the whole frame scrolls, as today's dialogs
do. The window's minimum size is in SPEC.md ("Desktop window chrome"; `MIN_WINDOW_SIZE` in the app's bootstrap holds
it).

Enter on a choice control is one rule of the component layer (`components::enter_choice`), the old launcher's
`enter_choice`. A control takes it only when its owner hands it the dialog's main action, through an `on_enter` builder
on the radio group, the checkbox and the select. The launcher draws its own Agent and Command tab strip, as the
migration's orchestrator decided on 2026-11-14, and gives its tabs the rule through `components::on_enter_choice`; that
decision is no general exception to the constructor rule above. With the action, a plain Enter applies the focused
choice and runs that action: a segment or chip applies its option first, while a radio, a checkbox and a closed select
apply nothing and run it with the values shown, as the old launcher did. A held Enter is taken and does nothing, a
modified Enter goes on under the shared key rule, and Space and a click only choose. Without the action, every control
keeps its own Enter.

GPUI Kit is used for `Root`, text inputs and named primitives, as the prototype did. Kit's own overlays (dialogs,
sheets, popovers, menus, notifications, tooltips) are not used, not even inside the component layer: Kit moves focus
internally and has its own modal, notification and popover layers, which Farhelm's lints cannot see into, so the layer
manager draws every overlay instead. Kit is not forked to satisfy a lint.

Enforced by `disallowed-types` and `disallowed-methods` entries that ban raw color construction and Kit's overlays (the
overlay bans are listed under the layer manager), by `scripts/check-crate-layering.py`, which lets only the GPUI app
crate depend on GPUI Kit, and by one golden screenshot per component and per dialog frame. Known gap: inside the app
crate, clippy paths cannot confine the rest of Kit to the component layer's module, so that confinement is checked by
review.

### Actions and keymap

Owner: actions and keymap, a module unless crate layering needs a crate.

All actions are declared in one place. Buttons, menu items and keys dispatch the same action. A terminal pass-through
table lists the keys the terminal always receives. No key context that contains a terminal may hold a multi-key chord
prefix, because GPUI holds the first key of a chord back until the next key shows whether the chord goes on. It gives up
after a second only when that key alone is bound too, or when it types text into an element that accepts text input;
otherwise it waits with no time limit, which includes typing into a terminal that has no text input handler.

The shortcuts SPEC.md says ignore repeat list their actions in one place. GPUI runs a binding's action on every
auto-repeat of a held key, so a keystroke interceptor drops a held repeat before any binding matches it whenever the
binding GPUI would try first, where focus is, runs a listed action. The repeat then neither runs the action again nor
reaches anything behind the binding, a focused terminal's program included. The interceptor goes by that binding's
action rather than by the chord, so where a deeper binding takes the same chord (a text field's, say), its repeats keep
coming. A listed action's handler must take every key that reaches it, even when it does nothing. Composition is not
covered: GPUI's key events do not say whether an input method is composing, so refusing a shortcut typed mid-composition
falls to the element that owns the input handler.

The shortcuts SPEC.md matches by key position rather than printed character (the template number keys, Keyboard
shortcuts, terminal text size) are rows of one physical-key table in the same module, because a binding matches only the
logical key. A row names the key's position and its exact modifiers on each platform; the position comes from the key
event's native identity (an XKB position name on Linux, a virtual keycode on macOS), so a non-US layout's key at the
same place matches and a keypad digit does not. The same keystroke interceptor matches them before any binding or
listener: it dispatches the row's action and stops the key, so no terminal program or text field receives it, whether or
not a feature handles the action yet. A held repeat of a row whose action ignores repeat is stopped without dispatching.
Only an event with no native identity at all (a key the test bridge injects) is matched by its unshifted US key instead;
on real hardware such events are input-method text and replayed keystrokes, never these chords. Because the interceptor
takes these keys before the element that owns the input handler, refusing one typed mid-composition (SPEC.md asks it of
Keyboard shortcuts) falls to the action's handler, which can ask the focused input whether it is composing.

Enforced by a test that walks the keymap and fails on a chord prefix in any context containing a terminal, by a test
that every pass-through key reaches the terminal with the app's bindings loaded, and by tests that a held listed
shortcut runs once, from a focused terminal too and without reaching it, and again on a fresh press, while an unlisted
key and a deeper binding of a listed chord keep every repeat. The physical-key table is enforced by tests on both
platforms' identities: each row runs from a focused terminal without reaching it, and a held one acts once or on every
repeat as its action says; a key without an identity falls back to its US key; the position decides on another layout; a
keypad digit, an extra modifier or a missing one matches nothing; and no row's key types into a text field.

### Animation time: the ticker

Owner: the ticker, one shared clock and animation ticker.

It serves three kinds of time. Periodic animation ticks come at most 10 times a second and pause while the window is
inactive. One-shot deadlines (hover delay, the approval card's pause, banner expiry, copy confirmation, "still waiting"
thresholds) fire on time whether or not the window is active. It reads reduced motion from the OS. Wall-clock time is
for text that says how long ago something happened (a session row's age, the details bar's, the stopped card's): the
ticker reads the time of day from GPUI's scheduler, which tests advance like every other clock, and one shared 30-second
clock, as in the old app, notifies each view that shows such text, whether or not its window is active and whatever the
reduced-motion setting, because an age is information rather than motion and an inactive window is often still on
screen. A view holds one clock handle for all its rows, and a view that draws an age is drawn uncached, since the cache
wrapper cannot see the time. No row or view runs its own timer. This carries over the old rule that nothing redraws on
its own more than 10 times a second; without it, one animating row redraws its whole list at display rate.

GPUI's `with_animation` and `with_animations` are used only in the component layer, and only for one-shot transitions:
no loops, and switched off under reduced motion.

Enforced by the clock bans under "Work that finishes later"; by `disallowed-methods` on GPUI's
`BackgroundExecutor::timer`, `request_animation_frame` and `on_next_frame` in the GUI crates outside the ticker and the
channel's watchdog and heartbeat, and on `with_animation` and `with_animations` outside the component layer; by a test
that an animating row redraws at most 10 times a second and not at all while the window is inactive; and by a test that
a view holding the clock redraws every 30 seconds in an inactive window under reduced motion and leaves no timer once it
is gone. The wall clock itself is held by review, not by the clock bans: no lint stops GUI code from reading GPUI's
scheduler clock outside the ticker, or a view from reading the time without holding a clock handle.

### Notices: the notice center

Owner: the notice center. It is a code owner, not a new surface.

It is the single place that decides when to post to and withdraw from the existing surfaces: the bell list and notices
panel, approval cards, banners such as the version-mismatch and outage cards, and chimes. The surfaces look as they do
today. The only new notice is the generic connection-lost one after a reset, in an existing style.

Enforced by the review gate.

### Startup and quit: the bootstrap

Owner: the bootstrap.

One bootstrap installs all app-wide state (globals, the channel, the stores, the keymap) before anything runs. No
`unwrap` or `expect` in code that platform callbacks can reach: a panic inside an AppKit callback aborts the process.
State is saved continuously, because GPUI gives quit handlers only 200 ms.

A foundation reads the global the bootstrap installed for it through one bootstrap helper (`bootstrap::installed`),
never through GPUI's panicking `global`. A missing global fails an assertion in a debug build, which every test and
agent build is, and in a release build is logged while the caller goes on without it, so an omission in the bootstrap
fails tests instead of quietly turning a feature off. State that is optional by design (the helm connection, which some
tests never start, and the agent startup overrides) keeps its own lookup.

Enforced by denying clippy's `unwrap_used` and `expect_used` crate-wide in the GUI crates, the desktop binary's crate
and the helm client, with tests exempt (each crate's `clippy.toml`), and by `scripts/check-crate-layering.py` failing
when one of those crates loses the denial.

### Dependencies and licenses

GPUI is pinned to gpui-pre 0.3.12 and GPUI Kit to 0.9.1 exactly, with the lockfile committed, unless the early macOS
checks force a change. Upgrades happen only at milestones, as a dedicated change. Patched GPUI crates are vendored under
`vendor/` with their patch files, per `vendor/README.md`. Besides the layers ported from the GPUI prototype, Farhelm
carries two layers written for the shipping app. `frame-source` keeps GPUI's older `CVDisplayLink` frame source on macOS
14 and later for typing latency at or below the old app's; the maintainer accepted its costs (slightly higher idle CPU,
no power-policy frame rates), and a TODO.md entry revisits it. `no-kde-blur` removes `wayland-protocols-plasma`, a crate
declared MIT that bundles LGPL-licensed protocol files compiled into the Linux build, with GPUI's KDE-blur fallback that
was its only use (Farhelm never asks for a blurred window).

No GPL-family code (GPL, AGPL, LGPL) enters the tree, as a dependency or as copied or closely paraphrased source.
MPL-2.0 is allowed only as an unmodified dependency. Agents read GPL or other copyleft source only in an isolated
research agent that reports back general approaches and no source details. Zed's crates other than `gpui`,
`gpui_macros`, `gpui_platform`, `gpui_util`, `gpui_tokio` and `scheduler` are assumed GPL unless checked, and so are
Zed's keymap and settings JSON. Code adapted from a permissive source carries a comment linking to it.

Every binary that contains (links) code ported or derived from another project carries that project's license notice,
however small the part. `farhelm` embeds the notices for the supervisor's xterm.js port and the vendored terminal
emulator's Unicode-derived tables and prints them with `farhelm licenses`; the desktop app links both through the
supervisor it embeds and prints them with `farhelm-desktop --licenses`. When the desktop app first links the GUI crates'
ports (`crates/gui/farhelm-grid-model` and `crates/gui/farhelm-input-mapping`, each with its `LICENSE-PORTED`), the same
change embeds their notices and makes the app show them to the user.

The license notices of every third-party crate the shipped binaries link are generated from `Cargo.lock` into
`crates/farhelm/notices/dependencies.txt` by `scripts/dependency-notices.py` (a pinned cargo-about), checked in, and
printed by `farhelm licenses` after the ported-code notices and by `farhelm-desktop --licenses`; the ordinary checks
fail when the file no longer matches the graph, or when a text that must carry its copyright line carries a template
placeholder instead. The desktop app shows the same notices, with the license of the fonts it embeds, in an "Open-Source
Notices" window opened from its app menu (the maintainer's choice, on 2026-11-13); until that window exists,
`farhelm-desktop --licenses` is the interim surface.

Enforced by cargo-deny: a license allowlist (no GPL, AGPL or LGPL) and a sources section that denies every git source,
because cargo-deny cannot check a git repository's license. Copying cannot be caught by a tool, so the review gate asks.

### How the rules are enforced

The GUI bans are clippy `disallowed-methods` and `disallowed-types`, each entry with a reason, configured in
`crates/gui/clippy.toml` (clippy uses the nearest `clippy.toml`). That scopes them to the GUI crates: the helm, the
supervisor, the command line, the desktop binary and the helm client legitimately spawn, sleep and read clocks and are
unaffected. Each owner holds one `#[allow]` for the call it owns. `cargo clippy --all-targets -- -D warnings` stays
green, and every lint has a demonstration that it fails on a violation: `scripts/check-gui-lints.py` lints
`crates/gui/lint-demo`, which commits one violation per ban, and fails when any ban stays silent, when an `#[allow]` of
a ban sits anywhere but on its owner's item, or when a GUI crate switches a ban off as a whole (its manifest's `[lints]`
table, or a clippy configuration of its own that would replace the shared one).

The test automation bridge is a cargo feature that shipped builds and devbuilds never enable: its socket is
unauthenticated, and local agent sessions run as the same user, so with the bridge in a build any agent could click
Allow on its own approval card. A check fails the build if a release or devbuild profile contains the feature. Bridge
runs use a private 0700 directory.

What no tool checks, the per-PR review gate asks.

### Testing the GUI

In-process GPUI tests use GPUI's seeded scheduler to reorder async completions. End-to-end tests drive the real app,
against a real helm and supervisor with fake agents, through the bridge. Behavior is ported from SPEC.md and the old
Playwright specs' acceptance cases at the reference commit, not one spec file at a time.

### GPUI cookbook: traps

NOTE: These are traps, not a map of the code. Each one cost a shipping GPUI app, the prototype, or both. Public issues
are cited by number; "REPORT.md" is the GPUI prototype's report, in a private repository. None of this is copied code.
Traps that an earlier rule already prevents get one line pointing at it.

- **Re-entrant entity updates panic,** and abort the app inside an AppKit callback (sheprd#1297, zed#65140); see "The
  store, and how views redraw".
- **Missed notifications are silent.** The prototype had five confirmed and one candidate missed-notification bugs,
  found by review or by turning caching on, never by its own tests (REPORT.md, "Missed-notification bugs"); see "The
  store, and how views redraw".
- **A notify requested during prepaint is dropped.** Request redraws from event handlers and updates, not from paint.
- **Effects flush only after the outermost update returns.** Emits and notifies queue until then, so a test calls
  `run_until_parked()` outside `update_window` and renders a frame before querying.
- **Caching has sharp edges.** A cache miss re-renders the whole subtree uncached, nested cached views miss when their
  parent re-renders (zed#66451), a cached view is laid out from its style alone so it needs an explicit size, and
  per-frame paint callbacks are skipped on replay, and so are the view's accessibility nodes, focus report and actions
  while assistive technology is active. Rule: flat, explicitly sized cached views behind the wrapper, and none with
  accessibility roles until replay restores them (decided by the migration's orchestrator on 2026-11-14).
- **`focus()` clears pending keystrokes and refreshes the whole window;** see "Focus: the focus coordinator".
- **A focus move made in a focus-lost listener draws no frame.** GPUI runs those listeners while it finishes a frame,
  when a refresh is ignored, so the drawn frame keeps showing nothing focused until something else redraws; found when
  dismissing this app's own banner left an idle window that way. Rule: the focus coordinator's listener asks for one
  more frame (`window.defer`).
- **GPUI keeps a frame's input handler past that frame,** and with it a strong handle to whatever implements it. A view
  that was its own `ElementInputHandler` outlived its retirement, focus handle included, so focus stayed on an element
  nobody drew and the coordinator's recovery never ran (found when the terminal got its input handler). Rule: the
  handler is a small entity of its own holding a weak reference to the view.
- **A focus handle is a Tab stop only when built with `.tab_stop(true)`.** The default blocks Tab.
- **A press reaches every element under the pointer** until one blocks it, so a card drawn over the page without a veil
  passes a click on it through to what lies under it (a click on an anchored question's answer also focused the terminal
  under the card). Rule: the layer manager's popover and anchored cards `occlude()`.
- **GPUI turns the release of Enter or Space on a focused element into a click,** when focus stayed on it from the
  key-down, which already reached the element's key handler, and also on a held key's repeat. A control that activates
  on key-down and on click therefore acted twice per press, and a held Enter that answered one question answered the
  next one too. Rule: controls take clicks through `components::activate_on_click`, which ignores that keyboard click.
- **A virtualized row keeps focus only if registered with `ListState::splice_focusable`.** Otherwise a focused row that
  scrolls out, or is re-spliced, fires focus-lost.
- **Key bindings beat key listeners.** Bindings dispatch before capture-phase key listeners, and a binding with no
  context ranks as the deepest. In the prototype, the Mac Edit-menu work made Kit Root's `ctrl-c` binding active, and it
  swallowed Ctrl+C on Linux, so programs never got ^C. Rule: the terminal pass-through table, with `NoAction` bindings
  in the terminal context, and a test per pass-through key.
- **Chord prefixes hold the first key back, often with no time limit;** see "Actions and keymap".
- **Bound keys never reach the input method,** so a bare printable key cannot be bound in a text field.
- **Mac menus read the keymap once.** `set_menus` captures shortcuts when it runs, and menu items are enabled only when
  the focused dispatch path handles their action. Bind keys first.
- **Actions can vanish at link time.** Actions register through `inventory`; a crate whose actions the binary never
  references can be dropped by the linker. Keep an `init()` that the binary calls.
- **`uniform_list` measures only item 0.** No header or empty-state row at index 0.
- **A list with no definite height draws nothing,** and logs nothing (herdr#930). Every virtualized list needs a
  definite height up its container chain.
- **Element ids from indices go wrong.** The same id with a different state type panics, and an index id follows the
  position, not the thing. Rule: `(component, domain id)` ids.
- **`text!` in an iterator gives every item the same id,** because the id comes from the call site. Duplicate ids are a
  debug panic only while assistive technology is active, so ordinary runs never see them.
- **Dropping a `Task` cancels it; detaching one leaks it;** see "Work that finishes later: the operation stores".
- **Quit gets 200 ms;** see "Startup and quit: the bootstrap".
- **`cx.global` panics when the global is missing** (Runner #472); see "Startup and quit: the bootstrap".
- **Work inside `open_window`'s builder closure is a trap.** A task spawned there updates its model without scheduling a
  frame, and a resize there doubles every `px()`. Rule: do the work after the window exists.
- **Headless Mutter reports the window inactive forever.** `Window::is_window_active()` stays false even after Mutter
  focuses the window, so a ticker that pauses while inactive never ticks in headless end-to-end runs. Rule: the ticker
  takes its notion of "active" through a test override.
- **The test context's text system is fake.** Every character advances 0.6 em (1.2 em outside the BMP), there is one
  font, no fallback and no wide CJK. Rule: tests whose outcome depends on layout (wrapping, truncation, the terminal
  cell grid) use the real text system (`HeadlessAppContext` with a real text system).
- **Leak checks check nothing by default.** `assert_no_new_leaks` is a no-op unless GPUI is built with `leak-detection`;
  `test-support` does not turn it on. Turn the feature on for the dev-dependency. `cx.processor` captures a strong
  handle, unlike `cx.listener`.
- **Parking is forbidden under the deterministic scheduler.** Waiting on anything woken outside GPUI's dispatcher (a
  real socket, tokio, an OS thread) panics; timers jump to their deadline. Rule: a fake, ordered, in-process channel in
  tests, and executor timers only. Animations use the wall clock, so tests use reduced motion.
- **A hanging test may be GPUI's.** zed#66801 is an open scheduler deadlock; rule it out before blaming the app.
- **The glyph atlas never evicts.** Every rasterized size stays in memory, so zooming the terminal grows memory without
  bound (one report, infiniterm#758, saw 286 MB after 18 hours). Rule: keep terminal text size to the existing steps
  (every whole point size from 9 to 28), which bounds what zooming adds to the atlas; call `drop_image` for every
  released image.
- **Paths are a trust boundary;** see "Paths are a trust boundary".
- **The helm refuses any request that carries an `Origin` header** (SPEC.md, "Security"). The GUI's channel, terminal
  and HTTP clients never send one; a client library that adds it by default must be configured not to.
- **GPUI Kit is a black box to Farhelm's lints** (about 83 focus call sites in 0.9.1, its own overlay layers); see "The
  component layer and theme".
- **Parents see presses on child Kit buttons.** A parent's `on_mouse_down` sees presses on child Kit buttons, because
  Kit calls `prevent_default`, not stop-propagation, so check `default_prevented` (REPORT.md, "GPUI behavior and API
  surprises").
- **Kit details.** Kit's single-line Input binds Up and Down and does not propagate them, so ancestors need
  `capture_action`; a disabled Kit Button still prevents default on press (REPORT.md, "GPUI Kit surprises").
- **Layout costs show only at scale.** `flex_1` on a repeated item with text makes layout shape the label on every
  sizing probe; give such items a computed width and `flex_none()`. Flexible children that must shrink need `min_w_0` or
  `min_h_0`.
- **Debug builds are not representative for speed.** Measure frame times on an optimized build.

## GUI: Dioxus

NOTE: The Dioxus GUI's sections were deleted with that app in the GPUI migration: this one, "GUI: sidebar, sounds, text
size, quick switcher and shortcuts", and "Terminal widget: the grid island" with its "Links in terminals" and "The grid
client". These two stub headings remain only because open review-queue items cite them. Read the old text at commit
`6ef88c9a`, or at the GPUI port's reference commit `24bc0580`.

### The grid client

Deleted; see "GUI: Dioxus" above.

## Terminal host

`crates/terminal-host/` is a tiny long-lived process that owns one pty and the program in it, journals the program's
output in memory, and serves the journal, input and resizes to one client over a unix socket. Its own `SPEC.md` is the
contract; this file stays authoritative for how Farhelm uses it. It is deliberately self-contained (no dependency on any
other Farhelm crate, no Farhelm concepts in its code, `libc` its only runtime dependency) because running hosts outlive
every update: the protocol only grows, and every client release must keep working with every host ever shipped (its
"Change policy"). It ships inside the `farhelm` binary as `farhelm internal terminal-host`, which takes no version lock
(a host lives for weeks and must keep neither pruning nor uninstall waiting) and runs before anything starts a thread,
since the host forks. Every create starts its agent in a host, every new tab opens in one, and every Restart starts a
new one ("Creating agents in hosts", "Tabs in hosts" and "Restarting a host agent" below): every terminal the supervisor
serves is a host. Old tmux terminals, which earlier supervisors created, are ended at startup ("Ending the old private
tmux server"). Each session row records its agent terminal's kind (the supervisor schema 32 column `terminal_kind`),
which is `tmux` for a row an earlier release wrote and `host` once a create or a Restart wrote it, and where that
terminal is (`terminal_address`: a tmux pane id, never looked up again, or a host's socket name). Schema 33 changes
nothing stored: it came with the flag that first turned host terminals on (since removed), so that every build at 32,
which stores the kind without handling a host row everywhere, refuses a database such a supervisor has opened. Schema 34
stores no tmux concepts for a terminal that is not a tmux one: no row stores a tmux session name (a session's was `fh-`
and its id), and the address that held a host's socket name in a column named `pane` has a name of its own.

The supervisor's side of a host is `crates/farhelm-supervisor/src/hosts/`:

- **Socket directory.** Host sockets live in `<state dir>/hosts`, created with mode 0700. A directory there that is not
  exactly 0700, belongs to another user, or is not a directory (a symlink included) is refused, not repaired: its
  permissions are the only boundary a host has, and a looser one may already have let someone in.
- **Socket names** are 8 random base32 characters. `hosts/` plus 8 is one byte shorter than `supervisor.sock`, so a host
  socket path is always shorter than the supervisor's own, and every state directory that works today works for hosts
  under macOS's 104-byte socket path limit. Names mean nothing; what a host is for is in its tag.
- **Starting a host** runs the supervisor's own executable (`/proc/self/exe` on Linux, which stays the running build
  through an update's rename; its own program on the Mac, the versioned one when run by the desktop app, never the
  forwarder) as `internal terminal-host --socket <path>` (the path made absolute), with an empty environment, writes the
  child spec to its stdin, and takes readiness from its first stage's exit status alone. The empty environment keeps the
  session markers that Stop's sweep searches for, and any credential, off the host; the child's environment travels only
  in the spec. A start that does not finish (the deadline, a first stage killed by a signal, a failed wait) is never
  reported as a clean refusal: the host may have reported ready just before, and a tab found later by its tag would pass
  for one the user was told never opened. The spawn kills and reaps the first stage, ends whatever answers at the socket
  (a second stage that has not reported yet ends itself when its report finds no reader), and returns either "nothing
  left" or the socket where something may remain. A caller may choose the socket name itself and start the first stage
  apart from giving it the spec, which the first stage needs whole before it starts anything: in between it learns the
  first stage's process, pinned to its start identity, and can record it ("Creating agents in hosts" below).
- **The client tag** records the session, the role (agent with its launch generation, or tab with its tab id and whether
  it runs in its own scope) and the spawn time in nanoseconds, which orders a session's tabs without new persistence.
  Its encoding is permanent, because a host keeps its tag across every update: every tag starts with the magic `FHTG`
  and a version, a shipped version never changes (new fields take a new version), every version keeps golden fixtures in
  `crates/farhelm-supervisor/tests/fixtures/host-tag/`, and a supervisor that cannot read a tag (another version, not
  Farhelm's, malformed) keeps the host and shows it as unavailable, never reclaiming it as an orphan. The layout is in
  `hosts/tag.rs`.
- **Connecting** (`hosts/client.rs`) always takes over the host's connection: the supervisor holds its state directory's
  lock, so any other client is a stale one, and refusing would lock a restarted supervisor out of its own terminals. A
  connection ends in one of three classes. A missing socket file is gone; a socket that refuses every connection is gone
  only after three seconds of refusals (on macOS a live host with a full backlog refuses too). `Busy` is retried and is
  never gone. Anything else that cannot be used (a protocol version the supervisor does not speak, a malformed answer,
  no answer within five seconds) is unavailable: the host may be running, so it is left alone. The host's pid comes from
  the connection's peer credentials, pinned to its process start identity. A socket is never judged gone, and an
  unavailable host never lacks a pid, while the process table shows a live process of the same user whose command line
  ends with `--socket <an absolute path with that socket's file name>` (rows the walk cannot read, and command lines
  that cannot be read, are passed over): a stopped or wedged host stops accepting, the connections that time out against
  it fill its backlog, and from then on Linux answers `EAGAIN` and macOS refuses, as if nothing listened. Only the file
  name is compared there, whatever the spelling of its directory (a symlinked home, `/tmp` against `/private/tmp`): the
  name is 40 random bits, and a false match can only make a socket unavailable rather than gone. Such a process is only
  a candidate: a signal, below, needs the exact spelling. Each host has one owning connection inside the supervisor,
  since a second connection takes the host over from the first without notice. Unacknowledged input stays within 256
  KiB, a quarter of what the host holds before it stops reading its client, so a program that stops reading never holds
  back snapshots, resizes or shutdown; input whose fate a disconnect left unknown is reported, never resent.
- **Ending a host the protocol cannot end** (wedged, stopped, or speaking another version) is a SIGTERM, followed by a
  SIGCONT so that a stopped host acts on it, and after five seconds a SIGKILL, since a host only notes SIGTERM for its
  event loop and a wedged loop never looks. Each signal goes only to the pid from the connection (or the process table)
  after its start identity has been re-read and its command line still ends with `--socket <that socket>`
  (`hosts/process.rs`).
- **Stopping every host of a state directory** is `farhelm internal stop-terminal-hosts --state-dir <dir>`
  (`hosts/stop.rs`), for removing an install or cleaning one up with no supervisor involved. `<dir>` must be an absolute
  path; a relative or empty one is a usage error, refused before anything is looked at. The command finds hosts by
  place: the sockets directly inside `<dir>/hosts`, listed through one handle opened without following a symlink, which
  is also the handle a leftover socket file is removed through. A `hosts` that is a symlink or not a directory, or that
  cannot be opened, is refused and nothing it leads to is contacted: following it could reach another state directory's
  hosts. For each socket it connects with take-over and sends `Shutdown`, and falls back to the verified signal above
  for a host that does not answer, stays busy, speaks another protocol version, or took `Shutdown` and did not end. A
  socket nothing serves counts as stopped, and the socket file a gone host left behind is removed. A host counts as
  stopped only once its process is seen gone (its socket file, when the platform names no process), or nothing serves
  its socket: a `Goodbye` or a closed connection is not enough, and nor is a signal's candidate turning out already gone
  (it may have been an unrelated process with a socket of the same name elsewhere) until the socket is judged gone too.
  Through another spelling of the state directory than the supervisor used (a symlink to it), a live host still stops
  through the protocol, but one that cannot take `Shutdown` stays unconfirmed, since the signal needs the exact
  spelling. A host whose socket file was deleted by hand inside an intact `hosts` directory is not found. On Linux the
  remote uninstall's stop of the supervisor's unit ends such hosts with the unit's control group while the unit is still
  running; a host left behind by a unit that was already inactive or failed, with its socket deleted by hand, is not
  found. Hosts are handled at once. The whole command takes at most 30 seconds: finding and stopping the hosts at most
  29, after which a listing not finished, or a host still being handled, counts as not confirmed; then at most one
  second for its report on stderr, which it gives up on rather than wait for a reader that stopped reading. (In the Mac
  app's version layout the version hold every `farhelm` command takes first can add its own wait of up to five seconds
  before that.) Stdout stays empty; stderr has one line per host for people and logs, which nothing may parse. It ends
  the hosts and nothing else: the programs in them get the terminal's hangup, descendants that ignore it are left to the
  session's scope or the supervisor's unit, and it takes no lock, so a supervisor still running there loses its hosts.
  **Exit status 0 is the only success**, and all it means is this: every host whose socket the listing found directly in
  `<dir>/hosts` was confirmed stopped, and `<dir>/hosts` was listed whole as a real directory (or does not exist). The
  directory is listed once, so a host that a supervisor still running there starts afterwards is not covered; callers
  stop the supervisor first. Anything else means a host may still run: 1 when one could not be confirmed stopped or the
  listing was refused or incomplete, 2 for a usage error, and anything else (such as a panic's 101) for a failure of the
  command itself. A `farhelm` from before the command also exits 2, as for any unknown subcommand, so a caller never
  reads 2 as "no hosts". A caller decides whether to run the command by whether `<dir>/hosts` exists: every `farhelm`
  that can create that directory has the command. **This command line and that meaning of exit status 0 are a stable
  surface.** A newer helm runs the command on remote hosts whose installed `farhelm` may be older than itself, so every
  later release keeps accepting this spelling with this meaning, under the rules of "What running sessions hold across
  versions" (new spellings beside old ones, never a new meaning for an old one; a later release may make exit 0
  stronger, never weaker).
- **Discovery** (`hosts/registry.rs`) finds a state directory's hosts again by scanning its socket directory, never by a
  stored list: it connects to every socket there, reads each host's `Info` and decodes its tag, and identifies hosts
  only by their tags. A missing socket directory means no hosts; one in a refused shape fails the scan before anything
  is contacted, since a socket others could have placed there may not be the supervisor's own. Each socket lands in one
  class. **Live** hosts answered with a tag this build reads; the scan's connection becomes the host's owning
  connection, so the caller adopts it rather than reconnecting. Live means attributable, not adoptable: a host whose
  snapshot record this build cannot decode is live to the scan, and its adopter then treats it as unavailable.
  **Unavailable** hosts may be running but cannot be used or attributed (another protocol version, no answer, busy, no
  `Info`, a tag of an unknown version, not Farhelm's or malformed): they are left as they are, never treated as orphans
  and never have their sockets removed, because a newer supervisor may own them and come back after a downgrade.
  **Gone** sockets are the ones nothing listens on, by the client's rules above, and are the only ones whose files may
  be removed. The removal does not trust the scan, nor the file's device and inode, which a socket bound after the old
  file was unlinked can reuse: at removal it judges the socket gone again (plain connections without `Hello` refused for
  the whole refusal window, then no live process naming the socket on its command line, which also covers a host between
  its `bind` and its `listen`), and only then unlinks the file, if it still has the identity the scan listed. Anything
  it cannot settle (a failed or slow process search) keeps the file. What remains is the moment between that check and
  the unlink; it is closed by a precondition, not a mechanism: removal runs only in the supervisor holding the state
  directory's lock, never concurrently with one of its own spawns in that directory. When to remove them is the startup
  reclaim's policy, not discovery's. Entries that are not sockets are skipped and never touched. Lookups find a
  session's agent host by launch generation (two live hosts claiming the same launch are an error for the caller to
  resolve, never a guess) and its tab hosts in creation-stamp order, flagging a tab id that more than one host carries;
  they see live hosts only, so finding nothing proves a session has no host only when nothing was unavailable. A scan
  takes over every host it reaches, which cuts off any other client, so it is only for the process holding the state
  directory's lock, and inside a running supervisor it must skip both the hosts the supervisor drives and every spawn in
  flight (a spawn's socket is connectable before the spawn returns), by serializing scans with spawns or by reserving a
  spawn's socket name before the host binds it. Up to 16 hosts are classified at once, each within the client's connect
  and reply deadlines plus at most one process-table walk, itself bounded (ten seconds; a walk that does not finish is
  never read as "no process"). Walks run one at a time in the supervisor: a walk that runs out of time cannot be
  cancelled and keeps its thread, and whatever stalled it (a process stuck in the kernel) stalls later walks too, so
  while it is still stuck every later search fails at once, which leaves sockets unavailable rather than gone, instead
  of stranding one more thread each.
- **A host terminal's runtime** (`hosts/terminal.rs`) is one task per host that owns the host's one connection and the
  terminal's emulator. It reads the host's journal from the start, restores the stored snapshot record before adoption
  returns (so a host whose snapshot cannot be restored is never a terminal that looks usable), and feeds the rest. It
  knows the snapshot record has arrived from the one exchange whose order every host promises
  (`crates/terminal-host/SPEC.md`, "Reading"): owed nothing else, it sends `Read` and one `Info`, waits for that reply,
  and sends a second `Info`, whose reply comes behind the stream's first record when the stream holds one; it writes the
  emulator's answers to the program's queries back as input, and draws a synchronized update the program left open once
  its 150 ms deadline passes on the runtime's own clock, which tests advance by hand. For a host the supervisor has just
  spawned, everything is live, so the program's first queries are answered even when it asked before the supervisor
  connected; output the host's `delivered_end` says some client was already sent is replay all the same. For a host that
  ran before, what was journaled before adoption is replay, under the replay policy below. Answers waiting for room in
  the input window are capped at 64 KiB and dropped past it, since a program that floods queries without reading its
  terminal would otherwise grow the supervisor without bound. Input is sent whole and in order and never resent; input
  in flight when a connection ends is reported as unknown, and caller input asked for before that loss is reported not
  sent rather than written after the hole. Each input carries an epoch, which a connection that lost caller input moves
  on once: a one-off input takes the epoch of its call, and the parts of a paste sent through one input sequence share
  the sequence's, so no part asked for after a loss is written even when the sender never waited for a receipt. A
  connection that a different process than the adopted host answers is never named as the host's: the terminal becomes
  unavailable as replaced, naming the adopted process. A connection that ends while the host answers is resumed from the
  position processed, so nothing is fed twice, and the latest size asked for is sent again if the host reports another;
  after three reconnects within a minute the terminal is made unavailable rather than fight whoever keeps taking the
  host over. Unavailable ends a runtime, not a host's chances: for a reason that can pass (a timed-out connect, a host
  briefly stopped, a connection that kept ending), the host is retried while the supervisor runs, as is one startup
  could not adopt ("Across supervisor restarts", re-adoption). The child's pid is a kill root only while the host
  reports the child alive and the process under that pid still has the start identity pinned at adoption (between two
  `Info` replies that both say it runs; a pid is not reused before the host reaps it), re-read each time a root is asked
  for. A terminal counts as exited only once the stream's exit record is processed, which the host sends after the
  child's drained output, so whatever acts on the exit reads the final screen; every exit, end or loss of a host
  terminal is also sent to one shared notifier, so one task can wait on every exit.
- **Recovery and checkpoints** (`hosts/terminal.rs`, `hosts/terminal/checkpoint.rs`). The **replay policy** decides what
  replayed output may do, by the host's `delivered_end` (the end of the output it wrote whole to some client). Output in
  the stored snapshot is never replayed: a restore has no side effects. Output below `delivered_end` was sent to an
  earlier supervisor, so it is fed with no side effects at all. Output from there to where the journal ended at adoption
  was written while no supervisor was attached: its queries are answered, since the program may still be waiting for
  them, and its bells and clipboard writes are dropped, since acting on them minutes later would surprise the user.
  Effects follow the bytes, not the moment they are interpreted: a synchronized update holds its bytes until something
  ends it, and the emulator gives what it then interprets the effects of that moment, so where the replay crosses
  `delivered_end` it draws any update still open with seen effects, and it ends by drawing any update still open with
  unseen ones. No query is answered twice. **The delivery ambiguity accepted** (a decision, not an oversight; only an
  acknowledgement in the host's protocol, which only grows, could remove it): output written whole to a supervisor
  counts as seen even if that supervisor never interpreted it, so a query in it goes unanswered and the program waits
  out its own timeout; and an answer a supervisor had worked out but not written is lost the same way. That holds for a
  crash, and for an orderly stop too, for the output in flight between the host and the stopping supervisor when it lets
  the connection go. Writes to the clipboard reach notice subscribers only from live output, so the user never sees one
  twice (bells, title changes and writes to the primary selection are not sent: a viewer does nothing with any of them,
  and a burst of them would push clipboard writes out of a subscriber's backlog); each viewer attachment subscribes on
  its own, every subscription gets every notice, and one that falls behind by more than a short backlog loses the oldest
  (logged) rather than holding the terminal up. Every replacement of the emulator (a restored snapshot) goes through one
  place and is counted under the emulator's lock, beside the gap's history-trimmed mark, and so is every resize that
  changes the size (a resize reflows history without replacing the emulator), so that a viewer's grid stream reading the
  emulator can tell that its history must be sent again. The status carries the program's modes and the terminal's size
  as of each change, so that a viewer's input is encoded without touching the emulator. A resize to the size the host
  already has is not sent to it. A paste reaches the runtime whole, as one request of at most 8 MiB, which it queues as
  all of its `Input`s in one turn, so that nothing lands between them: another producer's input waits for them, and the
  runtime's answers to the program's queries queue behind them. Pastes waiting for the runtime share a budget of 8 MiB
  per terminal, from before one enters its queue until its last piece is sent, so pastes left by ended attachments
  cannot pile up. The runtime counts the bracketed pastes begun on a connection (an opening piece sent) and not closed
  (a closing piece acknowledged); a loss while one is open and anything is unacknowledged owes the program a closing
  marker, which goes first on the new connection, ahead of the answers kept from before the loss. The marker is input of
  its own, never packed with answers, and stays owed until the host acknowledges it, so a second loss before then owes
  it again; one marker closes any number of pastes. A loss with nothing unacknowledged owes nothing: the rest of the
  paste goes on whole. Since an unacknowledged `Input` may have been written whole, in part or not at all, the program
  can read a closing marker alone (when the loss caught the paste's opening piece unwritten) or twice (when a lost
  closing piece or marker was written after all), which was accepted over leaving it inside a paste. A runtime that ends
  with a marker owed closes nothing, and neither does the runtime that adopts the host next. **Checkpoints** keep the
  host's tail short: once the output since the previous attempt reaches four times the latest snapshot's size (at least
  1 MiB, at most a quarter of the host's journal cap), the runtime snapshots its emulator at the next clean boundary
  (after a read's split candidate, at a read's end, after a synchronized update's deadline draws it, or where the replay
  finishes) and hands the snapshot to the host, which then drops the output before it; a rejected snapshot leaves the
  previous one. Nothing makes a checkpoint due by time: a quiet terminal's short tail replays in milliseconds, so it
  costs nothing. The host's `Info` does not report its cap, so a runtime assumes the default unless its creator passes
  the policy for another. The release `encode_speed` benchmark in `crates/farhelm-terminal-emulator/tests/` measures
  full scrollback at 200 columns, 60 screen rows and 12,000 history lines, with a 16 MiB snapshot limit. Before the
  encoder optimization, median encode times were 0.269 s for typical output, 1.568 s for true color and 9.333 s for
  image-like output with per-cell foreground and background colors; afterwards they are 0.126 s, 0.424 s and 2.235 s.
  These are medians of five encodes per fixture on the same AMD EPYC Processor (with IBPB) KVM guest, with 18 reported
  cores and 18 vCPUs. Shared-machine load varied; the final image-like range was 1.915 to 2.348 s. These wall times are
  observations, not CPU costs or guaranteed input latencies. Encoding still takes long, and so does restoring a snapshot
  (0.25 to 0.46 s), so both run on tokio's blocking pool, never on an async worker. For an encode the emulator moves out
  to the blocking task and back, and what status sampling reads (screen text, title, modes, size, whether history was
  trimmed) comes from a view left in its place, without waiting; a reader of the whole emulator waits for it to come
  back, without blocking. The runtime waits for an encode before it does anything else, so input, resizes and requests
  wait as long. Letting input through meanwhile was decided against: the host stops acting on a client's requests while
  more than 256 KiB of its output waits unread, so during a flood, where an encode hurts, a Ctrl-C would not take effect
  sooner; reading the connection while the emulator is away would overflow the client's bounded event queue, or need a
  backlog of megabytes that still runs out; and typed input's echo cannot be drawn until the emulator is back. An encode
  task lost with the emulator (not an encoder panic, which hands it back) ends the runtime, unavailable. At 100 KiB/s,
  the 8 MiB checkpoint spacing of these full-scrollback fixtures fills in about 82 s; a quiet terminal spends nothing. A
  connected terminal can still gap: with the default 32 MiB journal, the rough peak tail is
  `8 MiB + output_rate * (encode + catch-up)`. The measured 2.235 s image-like encode alone gives about 10.235 MiB at 1
  MiB/s, and fills the remaining 24 MiB at about 11 MiB/s. Catch-up was not measured, and catch-up time and
  shared-machine load can make a gap possible at lower rates. The 1 MiB/s case is improved, not guaranteed gap-free. An
  orderly stop asks for a checkpoint within a deadline that bounds the encode and the host's answer. A processed output
  tail below the stop policy's threshold (normally 1 MiB) is left for recovery to replay without an encode, including
  when an automatic checkpoint shortened it after the stop queued the request. Otherwise a snapshot is taken where the
  terminal is if it is clean there, else none, so that the host keeps its last accepted snapshot and the tail after it,
  which a recovery replays; an encode the deadline cuts short is finished and sent as an ordinary checkpoint, unless the
  runtime is stopped first, which cancels every encode it runs (the encoder gives up between rows, keeping nothing). A
  host's **journal cap** is 32 MiB by default: output-only checkpoint thresholds are at most a quarter of it; resize
  records consume that cap independently. This normally leaves about four minutes of an agent's heaviest redraw traffic,
  and hours of a shell's, for a supervisor's absence before the oldest output is trimmed into a **gap**. A gap never
  resets a healthy emulator: the runtime carries on after it, marks the history trimmed for the viewer (for as long as
  that runtime runs: a snapshot cannot carry the mark), and, once any replay is over, asks the program to redraw by
  resizing the terminal one row smaller and back, since Linux sends no `SIGWINCH` for a resize to the size in force
  (checked on Linux 6.8; the BSD tty code macOS uses compares the same way, read from its source, not tested). It asks
  the same after restoring a snapshot that left out the screens or was drawn with another character width table. A
  redraw cannot bring back the scrollback in the gap, or a mode or other terminal setting (title, palette, character
  set, scroll region, cursor style) the lost output set until the program sets it again; nothing makes a program do that
  on `SIGWINCH`, and many set their modes only at startup, so the setting can stay wrong until the program restarts.
  Unverified either way: no test covers what real programs resend on a resize.
- **The terminal switch points** (`service/terminals.rs`'s `Terminal`, which is a host's runtime: the supervisor serves
  no other kind of terminal) answer for a session from its host's runtime. Status reads three classes of evidence: a
  running child is live, a processed exit record is an exit with its code, a host that has ended is gone, and a host
  that cannot be read is unattributed, which records nothing durable at all (not even a launch sentinel), lists an
  unsettled session as unknown, and makes Stop, Restart, a guarded Delete and prompt delivery refuse, also when it
  becomes unreadable between a verb's liveness check and its taking the kill root (an unconditional Delete ends it
  instead, below). The ticker samples a host's screen and title from the emulator once the runtime has caught up with
  the host's journal, and the shared notifier wakes its reap pass, so a host agent's exit is recorded promptly. Stop and
  Delete take their kill root from the child as pinned at adoption, re-checked against the process table; the host
  itself carries no session marker and sits outside the launch's scope, so Stop leaves it running and showing the final
  screen. A launch that never started is classified, and quoted from the emulator's screen (a host runs one child, so
  the screen is this launch's) only once the host's exit record has been processed, since the child can be reaped before
  its last output arrives; a Stop that finds the child gone waits two seconds for the record and otherwise records a
  plain exit, which the observer can still turn into the launch's error (the launch shim's own failure report quotes no
  screen and is recorded at once). A hook report is anchored at the host's child; while the session's entry is not yet
  published the report waits for a later pass rather than being refused. Prompts check bracketed paste in the emulator's
  modes, once the runtime has caught up with its host's journal (until then, after a restart, a prompt is refused, since
  the modes are a past screen's), and send the paste as one paste through an input sequence, then the submit through the
  same sequence once all of the paste was written; input written between the two (an answer to a query the program asked
  meanwhile, a viewer's keys) lands in the prompt, which was accepted rather than have the runtime hold its answers
  across two sends. The orderly stop's hold wait compares the "held" mark with the child's pid. An attach is refused for
  a client that cannot show grids, permanently, with `InvalidRequest`; one that can gets a grid stream (below). Every
  create, tab open and Restart makes a host terminal, which a restarted supervisor finds again ("Across supervisor
  restarts" below); a Restart replaces the agent's host with a new one ("Restarting a host agent" below), starts a
  session that has no terminal (an old one ended at startup included) in a new, empty one, and leaves the session's tab
  hosts as they are.
- **Every terminal in a host.** A supervisor creates every terminal in a terminal host and never starts a tmux server
  (one an earlier supervisor left is ended at startup, "Ending the old private tmux server"). A supervisor inside a test
  binary cannot start a host from its own executable; in builds with test seams, one whose launch shim program is not
  its own executable starts hosts from that program (the built `farhelm` for the e2e suite's in-process supervisors)
  unless `SupervisorSeams::host_program` names another, while a real `farhelm supervisor run` starts them the shipped
  way even when built with test seams. The e2e harness's terminal helpers read a grid stream's rows as text and type as
  grid `text` input. The supervisor crate's own unit tests have no `farhelm`, so their test binary is its own host
  program: started with a host's command line (`internal terminal-host --socket <path>`), a loader initializer in it
  runs the host crate's entry point before libtest's `main` starts any thread, and exits (`hosts::test_program`). It is
  registered on glibc Linux only; elsewhere every host start from a unit test is refused (libtest rejects the
  arguments), so those tests fail by name rather than run on another kind of terminal. Those that need a real terminal
  run on such hosts (`tests::host_supervisor`); the tests of ending the old terminals build an earlier release's tmux
  server by hand. The README and docs media stacks run the bare `farhelm supervisor run`, so they test what ships. Known
  gaps, behaviors terminal hosts showed differently from what this document says, found when their tests moved to hosts
  and reported rather than fixed: a create or a tab open in a directory that cannot be entered is refused by the host's
  own start rather than recorded as a launch error (or a refused open) naming the directory, and a 0x0 attach leaves the
  terminal at its size.
- **Creating agents in hosts** (`service/core/host_launch.rs`): every create starts its agent in a host, and a new tab
  opens in one ("Tabs in hosts" below). A create commits the kind with the launching row, before anything starts, so a
  crash between the start and its confirmation leaves a row that says where to look. A retried keyed create whose
  interrupted row is a tmux row (left by an older supervisor) keeps that kind through the store's takeover, so a row
  that turns out to have launched replays as itself; one the takeover claims is refused before anything starts and
  rolled back like a launch whose spec never landed, the refusal settled against its intent key: the evidence that
  allowed the retry was gathered for tmux, so starting it in a host instead would be a guess, and this supervisor
  creates no tmux terminal. The launch is built up to its command as any launch is (the spec, the hooks, the login
  shell, and the shim's command line with its `systemd-run --user --scope` wrapper), and the host's child runs that
  command: the agent keeps its launch scope, and the host gets no scope of its own, living in the supervisor's control
  group. The child's environment (`host_child_env`, the same for agent and tab hosts) is the supervisor's own
  environment with the launch environment over it, then the terminal's own variables over both: `TERM=xterm-256color`
  (the terminal type the emulator implements), `COLORTERM=truecolor`, `TERM_PROGRAM=farhelm` and `TERM_PROGRAM_VERSION`
  set to the build's version, the one the emulator's XTVERSION answer names. These four are always set, so a supervisor
  started from some other terminal passes none of its claims on. Then `SHELL` (the user's login shell, also when a test
  seam replaces the launch shell), `PWD`, and `PATH` as `/usr/bin:/bin` when nothing gave one. `TMUX`, `TMUX_PANE` and
  systemd's `LISTEN_*` are never passed on: a supervisor started inside the user's own tmux has the first two, and the
  child is in no tmux. Nothing claims tmux, and no tmux is asked anything for the launch. A login shell's startup file
  that starts tmux when `TMUX` is unset therefore runs during the agent's launch, as in any terminal outside tmux
  (SPEC.md, "Session view"); until 2026-11 the child got `TERM_PROGRAM=tmux`, the private server's version and a
  placeholder `TMUX` that held such an autostart back, and a host started then keeps that environment until its agent is
  restarted. The container environment uses this ordinary path inside its own supervisor. The host's tag binds it to the
  session and the launch generation, and confirming the launch writes the socket's name to the row's `terminal_address`.
  **The launch record**: the socket's name is drawn before the host's first stage starts, and before that first stage is
  given the spec (it starts nothing until it has read it whole), the launch's artifacts record the socket's name and the
  first stage's process, pinned to its start identity (`<state>/launch/<id>.<generation>.host`). A supervisor that dies
  after the record leaves a first stage that can still start the host on its own, and the record is how a retry finds
  it; one that dies before it leaves a first stage whose stdin ends without a spec, which starts nothing. A failed host
  start is classified from the first stage's exit status: a refusal, or an unfinished start the spawn confirmed ended,
  leaves nothing, and the create rolls back as for a launch that started nothing; a host that may still run (an
  unfinished start not confirmed ended, or a host that reported ready but cannot be used) is ended through the protocol
  or by the verified signal, and only when that cannot be confirmed is the launching row kept (with its record). A
  create that lost to Delete, or whose confirmation failed, sweeps the agent's process tree and then ends its host the
  same way: through its runtime's `Shutdown` within 15 seconds (a runtime takes it up only between its other work, which
  a snapshot encode can hold), and otherwise, with the runtime stopped so that it cannot take the host back, by its
  socket within `stop-terminal-hosts`'s per-host limits. The socket file a host ended by a signal leaves is then
  removed, judged gone again first. A retried create whose row shows no launch asks, in order: whether the session's
  published entry holds a host (a create kept as a launching record publishes the host it could not end), which is the
  launch; whether a launch record exists, without which no host start ever got its spec; whether the recorded first
  stage still runs, in which case it is waited for up to five seconds and then ended with the verified signal; and
  finally what is at the recorded socket. A host whose tag names the session's first launch is the launch, and anything
  else that answers leaves the retry unresolved. Nothing there is absence only once no live process names the socket on
  its command line: a second stage the first stage forked before it ended still binds the socket and starts the child
  before its readiness report finds the first stage gone and it ends both, so the socket is looked at again for up to
  five seconds while such a process remains, and the retry is left unresolved if one stays. The record stays through an
  agent's launch error (the error settles the agent, not its host, which may still run with the failed launch's final
  screen and is named nowhere else while the row's `terminal_address` is empty). A retry that finds the host replays the
  session as created and starts nothing; the interrupted attempt's host itself stays running, and a restarted
  supervisor's reload adopts it by its tag. The supervisor-wide `host_scan` lock keeps host spawns (holding it shared
  from just before the first stage starts until the host's entry is published, or until the create gives the host up)
  apart from what must not overlap them (holding it exclusively): the removal of an ended host's socket file (by a
  create or by Delete), whose re-verifying removal requires that nothing binds sockets in the directory meanwhile, and
  any scan of the socket directory inside a running supervisor (today `serve`'s reload), since a scan would take a host
  over from the create that is starting it. Delete's settling of its session's launch records takes no part in it: the
  session's lifecycle claim keeps that session's own creates out. A session kept as a launching record with its host
  unconfirmed ended has that host ended by its Delete, which finds it by the launch record.
- **Tabs in hosts** (`service/core/host_tabs.rs`): every tab is a terminal host of its own. The tab-open gate asks the
  agent's host: it must still run (with its program or its final screen); one that has ended is the restart gap, and one
  that cannot be read refuses. The tab host's child is today's tab chain (the forwarder, then `internal tab-shell`, in
  the tab's own scope when there is a user manager), with the environment an agent's command gets plus the tab's markers
  and credential, which reach only the child; it starts at the size of the agent's terminal (its host's emulator; 80x24
  when that cannot be read). Its tag names the session, the tab, whether it runs in its own scope, and its creation
  time, which is never at or below the newest of the session's tabs, so a clock stepped backwards cannot reorder them.
  Like an agent's start, a tab host's writes a launch record before its first stage is given the spec
  (`<state>/launch/<id>.tab-<tab id>.host`, the same format), under the same `host_scan` fence until the tab is
  published; the record is removed only once its host is confirmed ended. It is not a registry of tabs: it is how Delete
  finds a tab host no runtime holds, and how a restarted supervisor knows which socket to ask, cross-checked against the
  host's tag. The session's tab runtimes live in its session cells, which a Restart shares, so tabs keep running across
  Restart. Stop closes them after stopping the agent. The open waits up to 250 ms for the shell to die, from the
  runtime's view of the child, and a shell dead by then refuses the open with the emulator's screen as its last words
  once the host's exit record has been processed (two seconds at most); the `host_scan` fence is let go as soon as the
  shell is seen not alive, so that wait holds up no other host's spawn or socket removal. Any failure after the host
  started ends the host (its `Shutdown`, else the verified signal), sweeps the tab's scope and marker (rooted at the
  child only when a runtime pinned it, so a failure before the runtime adopted the host sweeps with no root, never one
  read from an unverified pid) and removes its record, so a failed open leaves nothing; a host that cannot be confirmed
  ended keeps its record, which its session's Delete finds it by, and whatever could not be confirmed cleaned up is in
  the refusal. The tab strip and every tab lookup list a session's tabs by their tags' creation times; a tab whose shell
  exited, or whose host ended under it, is hidden at once. Closing a tab: the session's cell holds only hosts whose tag
  names one of its tabs (the open wrote it, a reload checked it), and the agent's own runtime is refused; the reap's
  "only if dead" is the runtime's processed exit record, checked again under the session's lifecycle claim; the scope is
  the tag's; the sweep roots at the child only while it runs; then the attached viewer is told the tab closed (before
  the host ends, since a grid stream would otherwise report its terminal ended first), the host is ended as above (for a
  runtime that judged its host gone from its socket alone, the process it adopted is confirmed gone first, by the
  verified signal), a marker-only sweep catches what outlived the hangup, and the record goes once the end is confirmed.
  A close that cannot confirm the end keeps the tab and its record and marks it, and since its stopped runtime announces
  nothing, the reap closes it again until one does: a second after the failure, then doubling to at most a minute
  between tries, so an end that keeps failing holds the session's lifecycle claim and a reap slot only that often. A
  host tab that cannot be read refuses to close; its session's Delete ends it. The automatic reap is driven by the
  host's exit event: the runtime announces its processed exit record on the supervisor's liveness notifier, which wakes
  the ticker's reap pass at once, with no poll; the tick runs it too, for a close a busy session put off. A takeover
  displaces the previous client's attachments to every terminal of the session, and a grid attach to a tab goes through
  the grid stream as an agent's does.
- **Restarting a host agent** (`service/core/host_restart.rs`): a host runs one child for its whole life, so a Restart
  of an agent in a host (or of a host row whose session holds no terminal) never reuses the host. Under the session's
  lifecycle claim, once the consent a working agent needs has been checked (so a refusal touches nothing) and before
  anything is stopped, the session's agent launch records after the generation of the host its terminal holds (by the
  host's tag; every record when it holds no host) are settled as a retried create settles its own: a host start that
  outlived the request or supervisor that made it still holds the whole spec in its first stage, and could start a
  second agent after the new launch, with no marker or scope a sweep would find. Its first stage is waited for and then
  ended with the verified signal, a record whose socket holds nothing is removed, a program found running in a host the
  record names is rooted (at its pinned child) into the agent's stop, and a start that cannot be settled refuses the
  Restart with nothing stopped. Earlier records belong to hosts a confirmed later launch replaced, whose programs the
  Restart that replaced them stopped; they are left to a later start and to Delete, so a wedged one never holds a
  Restart up. A tab's record (`<id>.tab-<tab id>.host`) is never settled: its host runs a tab, which Restart leaves
  running (the session cells it shares hold the tab runtimes), and its sweep spares tab-marked processes. Since the
  settle can wait seconds, and the agent's input is not held meanwhile, the consent check is repeated right before the
  stop: an agent that started working while the settle ran refuses the Restart, leaving only what the settle did to
  strays. After Restart's usual stop of the agent, and still before the generation is claimed, the old host's history is
  read: once its runtime has caught up with the host's journal and processed the exit record (a host that is gone at
  once; at most five seconds, after which the history is what had been processed, logged), encoded on the blocking pool
  by the old runtime's own task, which keeps the emulator whatever becomes of the request (the old runtime is still on
  the session map then, so an orderly stop cancels the encode, and the Restart is refused with the agent stopped; a seed
  missing for another reason, such as an encoder that failed, starts the new terminal empty). Only a running runtime
  gives a seed: a session whose host ended while the supervisor ran (its runtime has ended) restarts with an empty
  terminal, as after a supervisor restart, where a host that is gone has no runtime to read at all. This is the
  maintainer-approved narrowing of the goal's "from the old emulator" (DECISION, 2026-11-02): SPEC.md promises the
  scrollback only while the terminal still exists, and keeping it for a host that died would need a second encode path
  for a rare case. The new launch then starts in a new host through the create's start (the launch record, `host_scan`,
  adoption) at the previous terminal's size, seeded with an initial snapshot of that history above a blank screen in
  default modes with no title, fitted to `MAX_SNAPSHOT` by leaving out the oldest history (marked trimmed): the
  scrollback stays and the previous run's visible frame is not painted back, which is SPEC.md's Restart promise. A
  session with no old runtime to read restarts with an empty terminal. A viewer attached to the old host is detached
  before the relaunch ("session restarted") and attaches again to the new host's grid stream; nothing streams from the
  old host afterwards. Once the new launch is published (confirmed, or published as launching when only the
  confirmation's write failed), the old host is ended through its runtime's `Shutdown`, else by the verified signal to
  the process the runtime adopted (only on a platform that named no peer, so that no process was adopted, a host at its
  socket whose tag names this session is shut down over a new connection), and every launch record of the session naming
  its socket is removed; a host that cannot be confirmed ended is logged and left to the session's Delete, which finds
  it by its record, and to the next start. Ending it before the reply keeps Delete out meanwhile; a wedged old host can
  hold the reply for the runtime's 15-second `Shutdown` bound plus the verified signal's graces. A replacement that does
  not start is unwound as a failed create is: a host the start may have left is ended, the launch's spec, waypoint and
  host record are removed and its process tree swept, and the failure is definitive, so the previous outcome, the row's
  `terminal_address` and the old host as the session's terminal come back (the generation stays advanced, so the row's
  `terminal_address` then names a host tagged with an earlier generation, which reload takes as the row's own). The
  restored terminal, like a published one, goes back only before an orderly stop gathers the host terminals (otherwise a
  placeholder, its runtime stopped), and a runtime the restored entry does not hold is stopped. Only a replacement host
  that cannot be confirmed ended keeps the launching record: the session then holds no terminal, the old host's runtime
  is stopped, and both hosts stay reachable by their records. A restart that finds its row deleted when it confirms
  sweeps the new agent's tree and ends both hosts. A restarted agent's runtime is published only before an orderly stop
  gathers the host terminals, as a create's is. The session's host tabs keep running across a Restart, but while the
  session is off the session map for the Restart's window its tab cell is held beside the map, under the same lock as
  the stop's flag, so a stop in the window still checkpoints and closes the tab runtimes; a Restart that puts the
  session back (published or restored) after the stop began stops them, as it stops the new agent runtime. **A crash in
  a restart** leaves both generations' records, from which the next start converges with no other state. Before the
  replacement had a launch record (the generation claimed, or a failed replacement cleaned up before the previous record
  was restored), the row's own generation has no host, no `terminal_address` and no record; then, and only then, the
  host of the newest earlier generation that has an agent launch record is taken as the row's terminal when the scan
  read it at the record's socket. A record whose host the scan found gone is passed over for the next older one, since a
  dead host has nothing to keep or supersede; any other record whose host was not read there, or a launch directory that
  cannot be listed in full, leaves the row with none, and the next start tries again, so that an older host with older
  history is never taken while the newer one is merely unreadable. Its socket is recorded in `terminal_address` (the row
  lists the previous run's exit), so the next Restart seeds from it and ends it. After the new host started, the reload
  finds it by its tag and confirms it, and the old host is superseded. **Superseded agent hosts** (the plan's D-25): an
  old host left running is no orphan, since its session exists. So every reload, after its reconciliation has committed
  (a restart whose new host it just confirmed by its tag counts), ends each live agent host no row claimed whose tag
  names a host row at a later generation whose launch is confirmed (a `terminal_address` naming another socket), all at
  once within one stop budget, over the scan's own connection (`Shutdown`, then the verified signal to its peer), and
  removes its launch record. The current generation's host is never one, nor is a host the scan could not read, nor a
  tab's host (its tag names a tab, not a launch generation). The old program was stopped by the restart; a descendant
  that survives the host's hangup is left to the session's next marker sweep. **The residual**: the earlier host of a
  restart whose replacement started but was never confirmed and is gone by the next start (its launch record names
  nothing, so the fallback above does not apply) stays until the session's Delete, or a later restart is confirmed and a
  supervisor starts. So does the old host of a restart that failed ambiguously in a running supervisor (a replacement
  that could not be confirmed ended): the session holds no terminal, so the next Restart from that supervisor starts
  empty while the old host, with the history, runs on until a later start supersedes it or Delete ends it. And a runtime
  stopped while it encodes a checkpoint or a seed leaves its emulator away for good; a runtime is stopped only where its
  host is given up (a stopping supervisor, or a create's, a Restart's or a reload's cleanup, or the end of a superseded
  host), so nothing reads it afterwards.
- **Delete ends every terminal host of the session** (`service/session_hosts.rs`), since nothing else would: a host
  outlives its program and carries no session marker. Delete acts only on what the session records, never on a scan of
  the socket directory: the agent's host when its runtime holds one, the session's tab runtimes, and the session's
  launch records (`<state>/launch/<id>.<generation>.host`, and a tab host's `<state>/launch/<id>.tab-<tab id>.host`),
  written before every host start and kept until that host is confirmed ended. A tab runtime is treated as the agent's:
  its child, as pinned at adoption, is a sweep root, its tag's scope is named, the guard counts it as an open tab while
  its shell runs and as "cannot tell" when it can no longer read its host, and it is ended by the same path; a tab's
  startup placeholder is settled from its record like the rest. The startup sweep of the launch directory keeps every
  host record, whoever owns it, so that a record goes only with its session's Delete or with a confirmed end of its
  host; a record of a session no longer on record stays too: it keeps that session known to the startup reclaim of
  orphaned hosts (below), which removes it only once its host is gone. The record's first line names its format
  (`farhelm launch host 1`); a record Delete cannot read or decode (a newer release's format, damage) makes it refuse
  and keep the session, since the host it names may still run. Each record whose socket no runtime holds is settled as a
  retried create settles its own, all of them at once: its recorded first stage is waited for and then ended with the
  verified signal, so a start a crashed supervisor left half done cannot bind its socket after Delete looked, and a
  socket nothing serves is gone only once no live process names it (a forked second stage still on its way to binding).
  Then what the socket holds decides. Nothing: there is nothing to end, and a socket file a killed host left is removed,
  judged gone again first. A host whose tag names the session: Delete's guard counts it (its program running is a live
  agent, of whatever launch generation), its child is pinned over the connection as a sweep root, and it is ended over
  that same connection. A host whose tag names anything else: Delete refuses and touches nothing. Something that does
  not answer usefully (stopped, wedged, busy, another protocol, or a valid `Info` with a tag this build cannot read,
  which nothing but the socket's name ties to the record) whose process is known (the connection's peer credentials, or
  the process naming the socket): a guarded Delete refuses, since whether its program runs cannot be told, and an
  unconditional one ends that process with the verified signal after the sweep, without connecting again; it gives the
  sweep no root, so its program is reached by the marker sweep, the scope kill and its hangup. One whose process is not
  known: Delete refuses. The sweep never roots at a host or at an exited child's pid; the agent's runtime gives its
  child as pinned at adoption, checked against the process table whatever the host now says (a departure from "a root
  only while the host reports the child alive": the pid and start identity together are the child, which an unreadable
  or stopped host does not change). After the sweep, and before the attachment lock, every host is ended at once: the
  agent's through its runtime's `Shutdown`, then, whenever that does not confirm the end (its 15-second bound ran out,
  the runtime had ended, or an earlier Delete had stopped it), the verified signal to the process the runtime adopted,
  which the runtime keeps from its adoption whatever happens to it since, and never whatever now serves the socket: a
  process renamed over the socket is neither shut down nor signalled, and its socket file stays. A host the supervisor
  holds no adopted process for (a failed create's start, a platform that named no peer) is shut down over a new
  connection only when its tag names the session. The others are ended as above, and then the socket file a signal
  leaves is removed. Settling the records gets 60 seconds, and ending the hosts another 60 of its own, so that the sweep
  and the rest of the teardown between them count against neither; a record not settled, or a host not confirmed ended,
  within its budget or at all, keeps the session and its records for a retry, so Delete never reports a session gone
  while one of its hosts may still run. The command-line read the verified signal makes runs off the async workers
  through the same gate as the process-table walks: one that is stuck in the kernel fails the check within ten seconds,
  keeps one blocking thread until it returns, and makes every later walk or check fail at once meanwhile rather than
  strand a thread each. A host's grid stream ends with its host, before Delete takes the attachment lock to detach the
  session's viewers, so until a Delete has detached them (or returned) every natural verdict of a stream of that session
  ("session terminal ended", a failed stream or input) waits (`Supervisor::deletes_in_progress`, one entry per session,
  added under its lifecycle claim). The viewer then hears the Delete's notice: "session deleted", or, when the Delete
  failed after detaching it, why it failed; only when the Delete failed before detaching it is the natural verdict sent.
  A stall detach does not wait. **The residual**: a host the session no longer records (a record deleted by hand) is not
  found; a recorded host that answers nothing gets the verified signal but gives no sweep root; a read stuck in the
  kernel holds one thread until it returns; and these keep a session undeletable until a person deals with them: a
  record that cannot be read or decoded, a record whose socket serves another terminal's host, a recorded host whose
  process is not known, whose command line the verified signal rejects (it does not end with `--socket` and that
  socket's path), or that survives SIGKILL, and a recorded host whose child runs but cannot be read to pin it. A session
  whose agent terminal is a startup placeholder (a host a restarted supervisor could not adopt, "Across supervisor
  restarts" below) has no runtime: the launch record naming its socket is settled like any other. A placeholder no
  record names is ended only when the host at its socket answers with a tag naming the session (then it is counted,
  rooted and shut down like a recorded host of the session); anything else there keeps the session and is never
  signalled, since nothing but the socket's name ties that process to the session.
- **Across supervisor restarts** (`service/core/host_handover.rs`) a host terminal keeps running, and its session keeps
  its state. An orderly stop checkpoints hosts with at least 1 MiB of processed output since their latest known snapshot
  under one ten-second deadline that runs beside the output clients' ten seconds. Its own encodes use at most half the
  machine's cores (at least one), with slots admitted in order of output tail bytes, largest first, so those that finish
  save the most replay while leaving room for the update or desktop app. Automatic encodes already running or started by
  a runtime still waiting for a slot are not counted against those slots. Hosts below the threshold take no slot and
  their runtimes close immediately; the runtime checks the same threshold again when it handles the request, since an
  automatic checkpoint can finish after the gather. A deliberate skip is logged at debug and counts as success. The stop
  closes each runtime's connection once its checkpoint is settled or the deadline passes (every host terminal in the map
  is closed, those already ended included, since one may still be settling an encode). Closing a runtime cancels any
  encode it still runs: the encoder looks at its cancel between rows and gives up within about a millisecond, keeping
  nothing, and the stop waits until every encode has ended. That is what keeps the stop, and so the process's exit and
  the release of its state directory's claim, within the deadline: an encode runs on tokio's blocking pool, which the
  supervisor's runtime waits for as it drops. From the moment the stop gathers its host terminals nothing publishes a
  host runtime: a create whose host is ready only then stops its runtime and publishes a placeholder, its row and launch
  record kept for the next supervisor, and a re-adoption stops the runtime it made. After the pending-hold wait the stop
  closes every host terminal then in the map again, waiting up to a second for their encodes, and a runtime cancels its
  encodes whenever it ends, also when the process's tokio runtime drops its task (it does so before waiting for the
  blocking pool), so a stop that could not take the session map's lock in time still exits within one encode's look at
  its cancel. A host whose checkpoint was skipped, did not finish, never got a core, or could not be taken (a sequence
  left open, a runtime still replaying) keeps its previous snapshot and the journal still retained after it, subject to
  the host's cap, for the next supervisor to replay. The skip counts only processed output bytes, not resizes or output
  journaled but not yet read. Decided 2026-11-04: extreme cumulative resizes on a quiet terminal can exhaust that
  journal without making a checkpoint due, since each applied resize costs 64 bytes at the host but counts toward
  neither checkpoint threshold. The default 32 MiB cap takes about 524,288 applied resizes to fill. If those records
  evict output the live emulator still holds, a later crash or an orderly stop that skips its save can lose that
  terminal history on recovery; this specific case is accepted. The skip threshold is separate from the automatic one
  and is clamped to a quarter of the journal cap for a smaller-cap host. Each encode is of the whole terminal ("Recovery
  and checkpoints" gives the measured cost). Closing the connection right after the checkpoint limits the output the
  stopping supervisor counts as delivered without acting on it to what the host had already written to the connection,
  which the next supervisor replays without side effects (the delivery ambiguity above); output after the close is
  unseen, and its queries are answered. A crash or a kill leaves each host its latest checkpoint and the tail after it,
  the same way. A starting supervisor reloads its sessions only once it holds its state directory and has opened, and
  checked, its database, so a build that cannot read the database refuses before contacting any host. Reload scans the
  socket directory (discovery, above) and matches each host row to a host by its tag (session and launch generation),
  else by the socket the row names (its `terminal_address`, or for a launch never confirmed, the launch record of its
  own generation, never an earlier one's). A host at the row's `terminal_address` whose tag names the same session's
  agent at an earlier generation is the row's own: only a confirmation writes `terminal_address`, a Restart empties it,
  and one that failed definitively puts back the `terminal_address` of the host that stays the session's terminal. A
  host found is adopted as recovered (its snapshot restored, the tail replayed under the replay policy) and taken as a
  found terminal is: an unconfirmed launch found by its tag is confirmed with its socket's name. A host that may run and
  cannot be used (no answer, another protocol version, busy, a tag this build cannot read, a tag naming another launch,
  a socket directory that cannot be scanned, a launch record that cannot be read) is left as it is and never treated as
  gone: the session holds a handle that answers as unavailable, never no terminal, which records nothing durable (not
  even a launch sentinel, also in later observations), lists the session as unknown and makes the verbs refuse. A host
  row's host is absent only when no live host carries its tag and nothing serves the socket the row names (names are
  drawn fresh per launch) or its file is missing, or when the launch never reached a host (unconfirmed, with no launch
  record); the session is then reconciled as a vanished terminal's. A host whose child exited while no supervisor was
  attached reads as running until its runtime has replayed the exit record, after which the ticker records the exit,
  with any never-started classification; reload confirms such a session's launch, but leaves a durable stop request as
  it is, so that the exit is recorded as stopped by the user, and clears it only for a child the host reports alive. The
  scan takes over the hosts it reaches, so a supervisor without its state directory's claim scans nothing (its host rows
  are unavailable to it); `serve`'s second reload keeps the terminals the constructor's made, skips their sockets and
  those found gone (socket names are never reused), and runs under a lock re-adoption takes too; hosts no row claims are
  left as they were, except the superseded agent hosts of restarts ("Restarting a host agent" above). Every session's
  tab hosts are found again beside this, whatever its agent runs in: each tab record of a session the store holds names
  a socket, and a live host there whose tag names that session and tab is adopted as recovered into the session's cell;
  a host there that may run and cannot be used, a tag naming anything else, or a record that cannot be read leaves a
  placeholder that lists the tab and refuses its close (its session's Delete ends it), and a socket the scan found
  nothing at leaves no tab only once the record is settled as a retried create settles its own (the first stage waited
  for, then ended with the verified signal; the socket looked at again while a forked second stage may still bind it),
  since a start its supervisor did not finish has its spec and may still bring the host up, during the scan or after it:
  a host that comes up is adopted, and a start that cannot be settled leaves a placeholder. Settling costs nothing much
  for a first stage long gone and a missing socket file, the client's three-second refusal window for a socket file a
  killed host left, and at worst, for a first stage that never finishes, about twenty seconds (its five-second wait, the
  verified signal's graces, a forked second stage's five-second wait); every record is settled at once, so a reload pays
  the slowest. `serve`'s second reload keeps the constructor's tab runtimes, and its scan for agent hosts skips their
  sockets. Tab placeholders whose reason may pass and tab runtimes that become unavailable are retried while the
  supervisor runs, by the same re-adoption task as agent terminals. Tab records of sessions the store no longer holds
  are left to the startup reclaim of orphaned hosts, and so is the record of a tab dropped as gone. The orderly stop
  checkpoints and closes the tab runtimes with the agents'. The spare-checkout claims at startup read a host row's
  terminal, and for a host row with no terminal, whether a socket file is still at the path the row (or its launch
  record) names and was not found gone by the reload: a claim is released only then. **Re-adoption**: a running
  supervisor retries an agent or tab host that startup could not adopt, or whose runtime became unavailable later, for a
  reason that can pass (no answer in time, busy, closed connections, a child that could not be pinned, a socket
  directory that could not be scanned, an encode task lost with its emulator). Retries start after a second and double
  to a minute, each next try scheduled before the try. The attempt count is kept for that session and launch (or session
  and tab id) even after success. A tab's count survives an agent Restart, whose temporary removal of the session entry
  keeps the tab cell aside. Every episode of a connection that kept ending waits at least the runtime's full minute
  reconnect window before its first try, so re-adoption does not resume a takeover fight at once. Healthy launches keep
  their counts but contribute no wake time. The ticker forwards a passing unavailability notice to the retry task,
  without waiting for its ordinary tick. A Restart's republication also wakes retries that may have parked while the
  session was off-map. A try takes the session's lifecycle claim without waiting (a busy session is tried later), checks
  that the entry still holds that very handle for the same launch generation and that no orderly stop has begun, looks
  only at the one socket the handle names (its tag must name the session's launch, or, as at startup, an earlier one of
  its agent's), and swaps the outcome in under the session map after checking again, keeping the entry's run and session
  cells; a launch or stop request is then confirmed as reload would for a child the host reports alive. A runtime it
  made but does not install is stopped, leaving the host as it was. An orderly stop sets a flag under the session map as
  it gathers the host terminals it closes, after which no try installs a runtime. A handle whose reason will not pass on
  its own (another protocol version, snapshot format or tag, a launch record that cannot be read) is not retried. A
  failed try leaves a placeholder with its new reason, so a refusal that will not pass stops the retries. An
  unconditional Delete that aborts an unavailable runtime without finishing may leave it eligible for re-adoption;
  Delete's retry then acts through the replacement runtime. A tab retry checks the same session cell still holds that
  tab id and exact handle, and that no close has failed; a failed close belongs to the reap. A tab runtime that ran
  keeps its creation stamp and scope through retry outcomes, while a startup placeholder learns both from the recovered
  host's tag. Failed tab retries keep the earlier runtime's adopted process through every placeholder. A tab found gone
  gets a gone handle carrying that process, so the normal reap confirms that process ended and removes the launch record
  before forgetting the tab. A startup tab whose interrupted start could not be settled and left an `Unidentified`
  placeholder remains final, as before.
- **The startup reclaim of orphaned hosts** (`service/core/host_reclaim.rs`), the start-up cleanup the maintainer
  approved ("Cleanup on quit"). Delete ends every host a session records before it removes the session's records and
  row, every host start writes its record first, and a tab's close ends its host before it removes the record, so a live
  host whose session nothing knows is a leftover made by hand; the likelier way a host's session goes missing is a
  database that went backwards (removed, replaced, or restored from an older copy, as the desktop app's
  identity-mismatch message tells people to do), and the rules are built so that this never costs a running terminal. A
  session is known when the store has a row or a create reservation for it (in whatever state: a create claims its
  reservation first, and a retry of it finds its host by the tag), or when the launch directory holds a host launch
  record named for it (counted by name, so a record that cannot be read counts too). A tab is known when a runtime of
  its session holds it or a tab record is named for it. An orphan is a host this build can read whose tag names no known
  session, or a tab of a known session that is not known. Everything else is left: an agent host or a known tab's host
  of a known session, whatever its launch generation and whether or not a runtime holds it (a superseded agent host of
  an existing session is the reload's to end, "Restarting a host agent" above, before the reclaim runs); a live host of
  a session only its records know, with a warning naming the session and the socket and saying the database was likely
  restored, replaced or removed (the session has no row, so nothing in Farhelm lists or deletes it: it runs until it is
  stopped by hand, with SIGTERM to the process serving that socket, the warning repeats at every start meanwhile, and
  the start after it ends clears its record); a host that cannot be read (no answer, busy, another protocol version, no
  `Info`, a tag this build cannot read, or a snapshot this build cannot restore), never signalled, its socket and record
  kept, which the reclaim learns by adopting a host before it ends it, over the connection that read its tag and exactly
  as a reload adopts a recovered host (which answers the queries its program asked while no supervisor was attached);
  and whatever a terminal on the session map holds, runtime or startup placeholder, which is never contacted. Nothing at
  all is reclaimed when the socket directory is in a shape the supervisor refuses (a symlink, another owner, a looser
  mode: a host reached through it may be another installation's), when the store cannot be read, when it has no session
  and no reservation (at every start while that holds, as after a database went missing), or when the launch directory
  cannot be listed whole: a missing launch directory holds no records, but one that cannot be opened, or whose listing
  stops partway, leaves records unseen whose sessions and tabs would then look unknown. Launch records of sessions known
  only by their records are settled as a retried create settles its own (the recorded first stage waited for, then ended
  with the verified signal; the socket looked at again while a forked second stage may still bind it), so a start a
  crashed supervisor left half done cannot bring its host up after the reclaim looked; when nothing serves the socket
  then, its file and the record are removed, and a host that answers is left. Only regular files are read as records.
  The record of a known session's tab that the reload dropped as gone is settled the same way and removed, with any
  socket file its killed host left, once nothing serves the socket; anything there then is left for the next start's
  reload. Then a scan of the socket directory, skipping the sockets those records name and every socket the session map
  holds, finds what no record names: a live orphan is ended, and every socket nothing serves is removed, judged gone
  again first. Only the host of an orphan is ended, through the runtime its adoption made (`Shutdown`, else the verified
  signal to the process the adoption pinned, never whatever serves the socket by then), and its socket file removed; its
  program gets the terminal's hangup, and descendants that ignore it are not looked for, as with
  `farhelm internal stop-terminal-hosts`. It runs once per start, in `serve` only (a supervisor that only constructs
  never reclaims): once it holds the right to serve and the second reload has adopted every host the sessions account
  for and settled the reservations, and before it binds its socket, so no create, tab operation or Delete runs beside it
  and a client that sees the socket sees a state directory already reclaimed; the scan and its removals hold the socket
  directory's spawn fence exclusively, as the reload's scan does. Everything runs within 60 seconds, discovery included:
  discovery is a task of its own, and one that does not finish in time ends the reclaim before anything is ended or
  removed. After discovery, records, the scan and the endings run at once; what is still running at the budget is
  dropped, logged, and left for the next start, as is the work of any task that panics (discovery's included), so the
  reclaim never keeps the supervisor from starting; nothing is retried while the supervisor runs. A first stage that
  never finishes behind a wedged host can need more than the budget (its fence alone can take 40 seconds), so it may
  take more than one start. The ordinary start reclaims nothing and costs a listing of the launch directory and a scan
  that contacts no socket; each stale socket costs the refusal window twice (the scan's, then the removal's own check).
- **Grid streams** (`service/grid_stream.rs`, the encoding in `grid_output.rs`): an attach to a host terminal gets a
  sender task of its own in the forwarder's place, with the attachment machinery (takeover, detach, pause, the stall
  detach and its verdicts). It waits up to a second for the viewer's size to take effect, then sends `hello`, `state`,
  the visible screen, the history once (oldest first, in messages of about 32 KiB, which bounds what one attachment can
  have queued on the connection), and `ReplayComplete`; then, whenever the terminal changes, at most every 8 ms, one
  cycle of what changed: `state`, the lines that entered history, and the screen rows that differ from what this viewer
  was last sent, shifted when output scrolled. It reads the emulator only when it is home (never waiting through a
  snapshot encode), marking the status seen just before each read, so a change published after a read wakes it again.
  While the writer queue is full or the viewer has paused it sends nothing, and the next cycle starts from the emulator
  as it is then, so skipped screen revisions cost nothing. History is not skipped: lines are numbered by the vendored
  count of lines scrolled into history, and the viewer is sent every line from the one it is owed, unless that line
  scrolled off the far end or history changed shape (a rebuild or a size-changing resize, which the runtime counts, an
  erasure, which the emulator counts, or a redefined palette entry, which recolors rows already sent), when it is sent
  `history_reset` and the history held; a change of shape is acted on once it has held for a quarter second, or two
  seconds after the first of a run of changes that does not settle, so a window dragged to a new size resends the
  history once, not at every step; after a reset forced by the two seconds, the next run waits until it settles, so
  changes that never stop resend the history once, not every two seconds. A resend transfers the whole retained history,
  as an attach does (the opt-in grid benchmark measures its bytes and attach cost), and a redefined palette entry costs
  one whether or not any history row uses it, since telling would mean scanning the history. A live cycle sends at most
  about 256 KiB of history ahead of its screen frame (an estimate that errs high, counting escaped text and each run's
  link target, plus at most one row more), so a resend goes out over many cycles with the screen current in each. The
  attach's catch-up ends early when history changes shape under it, leaving the reset to the live path's settle rule,
  and is followed at once by a live cycle, since its later reads take history only. The program's exit is `state`'s,
  over the final screen; a host that is gone ends the attachment as a dead terminal's, and an unavailable one as a
  failed stream. An attach to a host that cannot be read is refused as `Unavailable` before anything is taken over, with
  a message that says what can help: a handle whose reason may pass becomes attachable once re-adoption replaces it with
  a runtime (the viewer attaches again then), while a reason that will not pass stays refused until the supervisor
  restarts. Clipboard writes reach the viewer from the attachment's notice subscription; while the viewer is behind only
  the latest is kept. Viewer input is decoded and encoded where the connection reads it, with the modes and size the
  status carries, never waiting there, and handed to the attachment's own input pump, which writes it through one input
  sequence per attachment. A paste is collected in the attachment and handed on only with its last part, as one paste
  the runtime writes whole; one whose last part never arrives is dropped whole. What an attachment holds (a paste being
  collected, input waiting for the pump, a paste not yet written) is bounded at 8 MiB. A malformed message, an
  interrupted paste, input past the bound, or input whose fate is unknown or not sent ends the attachment as an input
  failure, which the sender notices wherever it waits, a pause included. Since another producer's input lost with a
  connection moves the terminal's input epoch too, an attachment's next input after any such loss fails it, and the
  viewer attaches again. An attachment that ends drops the input its pump had not handed to the runtime and stops the
  pump at once; what the pump handed over is written before anything a replacement attachment sends.

Test harnesses end the hosts they leave behind: `farhelm-teststate` ends every host whose socket is in a `hosts`
directory within an owned run directory (its dead-run sweep, and every `TestDir` drop) through a small blocking client
(take over, `Shutdown`, then wait for the end of the connection and for the host's socket file to go), and keeps the
directory for a later sweep when it cannot confirm a host ended or could not read the whole tree. A socket that only
refuses connections counts as left by a killed host after three seconds of refusals, never sooner, since on macOS a live
host with a full backlog refuses too, and then only when no process of the same user still serves that socket by its
command line. The e2e fixtures and the supervisor's own unit tests keep their state in such directories. The desktop
smoke instead runs `farhelm internal stop-terminal-hosts` on each state directory that has a `hosts` entry, but only
once the supervisor that could start another host there is seen gone (the stack waits for its helm too). They keep the
directory when that supervisor is not gone within its bound or the command does not exit 0, and its run directory then
stays for a person to retry.

### Launch failure classification

Exited-session semantics: a terminal host keeps its program's last screen after the program exits, as SPEC.md requires,
and the exit code comes from the host's exit record. What a dead terminal shows is exactly what its emulator holds and
nothing more: the supervisor takes no screen snapshot before stopping a live process, stores no frame, and a Restart's
new host starts with the old history above a blank screen and nothing painted into it ("Restarting a host agent" above).
SPEC.md forbids a painted frame with no process behind it (the restart rules under Lifecycle operations, the no-snapshot
rule under Terminal experience); do not reintroduce one. The one trace left is a startup sweep that deletes a
`<state>/snapshots/` directory an older build wrote (`sweep_legacy_snapshots_dir`), so an upgrade does not strand
captured frames on disk. Exec failure versus ran-and-died cannot be told apart by exit code alone (a missing command
yields 127 and a non-executable file 126 — both indistinguishable from a program exiting with that code), so
classification does not rely on exit codes: the shell execs `farhelm internal launch`, a shim that always exists, which
resolves and execs the session's invocation and, on exec failure, writes a sentinel with the errno detail to a
per-launch status file (named by session and launch generation, so a sentinel left by a failed earlier launch can never
describe a later relaunch) before exiting. The supervisor classifies **error** on that sentinel; the one sentinel-less
error path is a launch whose login shell or `systemd-run` scope wrapper died before the shim ever ran, recognized only
by its full evidence shape (owned dead terminal, launch spec still unconsumed, no sentinel). An absent or unreadable
terminal does not qualify. A pending user Stop excludes every terminal-based launch inference, including checkout
preparation, in observation, reload and an ended-agent Stop. Direct and inferred failures stay distinguishable through
the outcome transition and temporary error cache: the store's transaction and the cache's read-side outcome check both
reject an inference when Stop was recorded during an earlier asynchronous evidence read. A shim failure report still
supplies direct evidence while the stop is pending; once an annotated exit commits, it keeps its annotation. Each
stop-annotating transition carries whether the launch spec was still unconsumed after the terminal ended: that yields
"stopped before the agent finished starting", otherwise "stopped by user". An inconclusive spec read uses the latter,
and the first committed annotation wins. The choice is free text on the wire; older clients show the new wording but
retain their ordinary exited icon in compact mode. A scoped window command first execs
`farhelm internal launch-wrapped`, which creates an empty generation-named `.waypoint` file without following symlinks
and then execs the wrapper with the ordinary shim command appended. No waypoint means the login shell exited before
Farhelm's command; a waypoint means the systemd scope wrapper exited before starting the agent. The error names that
step and includes the terminal's exit code when known. New specs carry `scope_waypoint_expected`; an older scoped spec
without that bit retains the two-suspect explanation because it never promised a waypoint. An unscoped command has no
wrapper, so its unconsumed spec already identifies the shell, including for older launches. A failed wrapper exec
records the ordinary sentinel with the wrapper and errno. NOTE: a sentinel written by the shell after a failed `exec`
was audited and rejected — interactive bash survives a failed exec, but zsh terminates on it in every mode, so
shell-side code after `exec` never runs for zsh users; the shim works identically under any `$SHELL`.

Command launches keep two transient representations at spawn. The existing `shell_words` projection supplies validation,
hook policy, launch classification, conversation capture and the stored resume arguments. The original command or resume
text supplies execution. A native Bash syntax tree certifies the source spans of unquoted placeholder words; those spans
must also match the existing literal placeholder slots. Only those spans are replaced, using single-quoted literal
values. Backtick substitutions are decoded and reparsed with the same grammar before their marker spans are certified,
because escaped nested backticks introduce another shell parsing stage. All other source bytes survive. Stored-row
decoding keeps the original shape checks so a previously accepted quoted placeholder cannot prevent the supervisor from
loading; native save/admission and spawn apply the placement check. Restart checks it before stopping the previous
agent. Conversation decoding and working-directory/trust filling retain their existing boundaries, and hook arguments
are computed once for both representations. Only `LaunchSpec.argv` receives `["bash", "-c", line]`. A missing Bash still
produces the shim's exec sentinel; a missing program inside the line is an ordinary Bash exit, usually 127, with no
exit-code classification. Wrapped agent launches likewise retain a literal vector for metadata. Its prefix length comes
from the stored selection; those raw words are joined with spaces, followed by every composed suffix word single-quoted
with the shared text quoting helper. Cwd/trust, hooks and the initial prompt are filled before quoting the suffix. The
editor keeps raw word slices while using the existing literal splitter for shape checks; stored vectors keep their old
shape validation and acquire Bash meaning without migration. Display joins only the prefix raw, then quotes the literal
agent words. The shim and its container branch receive the same Bash vector, so expansion happens on the session's host
or inside its container. Unwrapped agent launches and legacy snapshots keep direct exec.

Never-started errors may retain the last five nonblank lines of the failed terminal's final screen, at most 500 UTF-8
bytes after presentation-unsafe characters are escaped. The excerpt follows Farhelm's diagnosis in the existing durable
error detail, with `|` between lines so the existing peer-text escaping in the badge and tooltip does not turn framing
newlines into visible control-character escapes. The separators count toward the byte bound. It is not a replay snapshot
or a conversation history store. A host runs one child for its whole life, so everything on its screen is this launch's
output; the screen is quoted only once the host's exit record has been processed, which follows the child's drained
output, and a Restart's new host starts with a blank screen, so no quote reaches back into the previous run. Only the
background observer and an already-dead Stop classification request this quote, after proving the unconsumed
spec/no-sentinel shape; the Stop waits briefly for the exit record first and otherwise quotes nothing. Session lists
never quote. A live Stop never quotes, and reboot and shim-reached classifications are unchanged.

### The supervisor's terminal emulator

For a terminal in a terminal host, the supervisor's emulator is the one place its output is interpreted: one
`farhelm_terminal_emulator::Emulator` per terminal, held by that terminal's runtime (above).
`crates/farhelm-terminal-emulator/` is `alacritty_terminal` 0.26.0 and `vte` 0.15.0, vendored with Farhelm's patches
(`vendor/README.md`: the caps on hostile output, the history counters a grid stream numbers lines by, the parser's
ground-state check a snapshot needs, and fixes), plus Farhelm's policy around them. It answers the program's terminal
queries, tracks the modes that input encoding needs, gives status detection its screen text and title, serves the grid
stream, and is snapshotted ("Terminal snapshots", below). The screen text is Farhelm's own format, defined in the
crate's `screen` module (the visible or alternate screen, one line per row, trailing blanks trimmed, tabs over blank
cells kept as tabs). It has the shape of tmux's `capture-pane -p` on purpose: many of the captured agent screens status
detection is tested against were captured that way, and one set of status reader rules reads both.

Its query answers are Farhelm's own table, recorded in the crate's `queries` module and pinned by its tests: an answer
says what is true of this emulator or names the assumption it makes, and the emulator declines (answers nothing) rather
than claim a feature it lacks. Its rows are answers the supported agents are tested against, each kept only where it is
true of this emulator; the rows that depart say why (DECID is answered as DA1, which vte cannot tell it from, and color
answers end the way their query did). So device attributes say `CSI ? 1 ; 2 c` (alacritty alone says a VT102); the
secondary attributes are alacritty's own and claim no other terminal, since a program that believes it runs in tmux may
wrap its output in tmux's passthrough envelope, which this emulator drops; `CSI 14 t` assumes a 16 by 32 pixel cell,
since nobody reports the viewer's; XTVERSION names Farhelm and its version, because Claude Code asks nothing further,
synchronized output included, without an answer; and color queries are answered from Farhelm's terminal theme (Ghostty's
palette, as the browser's) or the colors the program set. Known limitations, accepted: a few queries `vte` does not
parse go unanswered, and a query inside an open synchronized update is answered when the update ends rather than at
once. Keyboard input keeps today's legacy xterm encoding: the kitty keyboard protocol stays off (turning it on would
change the Shift+Enter and Escape handling SPEC.md promises) and its query unanswered, and OSC 52 writes are honored
while reads are never answered. The encoding itself is the supervisor's (`grid_input.rs`). `vte` holds a synchronized
update's bytes until the update ends, and its 150 ms safety deadline is not self-driving: the runtime wakes at the
deadline on its own clock and draws the update, and also draws one left open when the child exits, when a replay ends
and before a resize; `vte` itself ends one whose bytes reach its 2 MiB buffer. Each of those can draw a frame the
program had not finished. The safety deadline, the buffer limit, a resize, the child's exit and the end of a replay are
examples of the deliberate exceptions to frames never being drawn half done; a recovering replay also draws an open
update where it crosses from output an earlier supervisor was sent to output none was, and where it crosses a gap,
though it carries on to the same final state. The crate's own tests pin the answer table, the screen-text format and the
modes as Farhelm's, and the supervisor's screen fixture tests paint every captured agent screen into the emulator and
run its status reader on the emulator's text.

Emoji and text variation selectors follow Ghostty's default Unicode width rules: only eligible bases change width,
selectors after a combining mark are ignored, and text presentation can narrow a wide character. Two exceptions keep the
existing saved state sufficient: with autowrap off at the right edge, a plain space counts as empty when choosing the
selector's target, where Ghostty distinguishes a printed space from an erased cell; and a one-column terminal keeps the
existing drop of too-wide input before processing a pending wrap. Both are pinned against the source-derived behavior of
Ghostty commit `c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed`
([target selection, lines 1939–1967](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1939-L1967),
[one-column input, lines 2189–2262](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L2189-L2262)).

Unattached vowel or trailing signs use ordinary scalar printing, including zero-width combining storage. Selectors apply
only to the immediately preceding stored character, so a retained sign blocks a following selector. At the right edge
with autowrap off, an unattached zero-width sign instead joins the cell before the last-column character, so a following
selector applies to the last-column character; this is how Farhelm's terminals behaved before terminal hosts, kept as it
is, and the resulting retained-text order is accepted.

Terminals discard the ignored-format character class before storing output, even without an emoji or selector:
zero-width space, directional marks, embeddings and isolates, line and paragraph separators, word joiner, BOM, two
Hangul fillers, and the remaining ranges in the vendored `emoji_ignored_format` classifier. Scrollback, snapshots and
copied text therefore omit those characters. The program's input and files are unaffected. This broader formatting
policy is accepted; it is not a promise to preserve every codepoint a program prints.

### Terminal snapshots

The supervisor interprets a host terminal's output with its own emulator (`crates/farhelm-terminal-emulator/`,
`alacritty_terminal` with the patches in `vendor/`), and gives the host snapshots of it, so that a restarted supervisor
rebuilds the emulator from the host's latest snapshot plus the output since. The host drops output once it accepts a
snapshot, so there is no replaying from the start. Hence the compatibility rule: **every snapshot format ever shipped
stays readable by every later supervisor, forever.** A blob starts with magic bytes and a format version; a shipped
version never changes, and a new format is a new version whose reader sits beside the old ones. Each version keeps
golden fixtures (`tests/fixtures/snapshots/v<N>/`: blobs from scripted states, the state each must restore to, and a
check that the encoder still writes them byte for byte), added to and never regenerated. A blob of an unknown version (a
newer supervisor's, after a downgrade) or one that does not decode is an error, and the terminal shows as unavailable;
the supervisor never guesses at a state.

Version 1 is an escape-sequence stream that rebuilds the state in a fresh emulator, plus fixups written directly for the
few states no sequence produces; a snapshot spike proved the round trip at arbitrary clean boundaries before the format
was fixed, and the crate's property tests keep proving it. A snapshot is taken only at a clean boundary (the parser
between sequences and no synchronized update open); elsewhere it is refused and the previous snapshot stays. A blob
records the character width table it was painted with: the Unicode version its crate names, and a fingerprint of the
width it gives every character, which is what tells two tables apart (the crate has changed widths without changing the
version). A reader with another table restores it anyway and asks the program to redraw, since refusing would make every
running terminal unavailable after a dependency upgrade.

Version 1 streams replay with the emoji variation-width rule and its changed REP policy off, preserving the layout and
repetition state old streams described. Both return to the live rule before any host-journal output is replayed. Widened
and narrowed cells use the existing cell fixups; the encoder's bytes and per-character width fingerprint stay unchanged.

A blob is untrusted on the way back: a corrupt or forged one costs a typed error, never a panic, unbounded memory or
unbounded work. Before anything is allocated, screens too large for the restore's cell bound are refused, and history
beyond it is dropped (and marked trimmed) the way a smaller scrollback drops it, so the bound stays each build's own
policy rather than a rule of the format. Before any of the stream is fed, it must hold only the sequences version 1
writes, and the work replaying them takes, counted with grid-wide operations and scrolls, must stay within a limit set
by the stream's length and the grids' size. Fixups must hold only cells live output can make.

A snapshot never fails for size. When it would exceed the host's `MAX_SNAPSHOT` (16 MiB), it drops the oldest
title-stack entries first, as many as it must; with no titles left, the oldest history lines, and marks the history
trimmed. The mark survives later snapshots and restores, the blob carrying how many more lines must enter history before
it ends, so that the viewer can say history was trimmed until the program clears its history or the history has turned
over. Only if no history is left either does it drop the screens' contents (the supervisor then asks for a redraw).
Ordinary terminals stay far below the limit: 12,000 lines of 200 columns encode to about 3.6 MiB. Restart seeds the new
host the same way, with a snapshot of the old host's history above a blank screen in default modes, read from the old
host's emulator.

The memory budget for one supervisor-side terminal at Farhelm's full scrollback (12,000 lines, 200 columns) is 80 MiB
for ordinary output (measured: 56 MiB, about 24 bytes a cell), and hostile output is bounded at about 190 MiB: the grid
(56 MiB), at most 64 MiB of cell extras (combining characters, underline colors and hyperlinks varied from cell to cell,
under a vendored per-terminal cap that no path can exceed; past it marks are dropped, colors fall back to the default
and links end), 38 MiB measured outside the grid (title stack, an unterminated OSC, the update buffer: 34 MiB of buffers
and the allocator's overhead) and about 32 MiB while a snapshot is taken (twice the blob). Measured hostile cases reach
120 MiB; without the cap the same output takes 387 MiB, or about 6 GB with hyperlinks. A supervisor therefore holds at
least 12 terminals per GiB doing ordinary work and at least 5 per GiB if all of them are hostile at once. The crate docs
("Memory budget") have the measurements and the arithmetic. Each terminal host process is bounded on its own by its
journal cap and `MAX_SNAPSHOT`.

## Ending the old private tmux server

Releases before terminal hosts ran every terminal in a private tmux server per state directory, and later releases kept
those old terminals running beside terminal hosts; a starting supervisor ends that server. ("Provisioning" covers
removing the tmux build earlier releases installed.)

Ending the old terminals (`service::core::old_terminals`) is the one tmux-related step the running supervisor keeps, and
works whether or not any tmux program exists. A supervisor that holds its state directory's claim runs it after the
store opens and before sessions are reloaded. It connects to `<state dir>/tmux.sock` with a plain socket connect. A
missing socket file, or (on Linux) a refused connection, means no server: the leftover `tmux.sock`, `tmux.conf` and
`.tmux.conf.tmp-*` files are removed. Any other failure is logged and the files are left, so a later start can still
reach the server: a wedged server with a full backlog answers `EAGAIN` on Linux, and macOS refuses a connection to a
live listener as it refuses one to a dead socket; that residual is accepted. Something answering is identified by the
connection's peer credentials (the pid pinned to its start identity, `hosts::process::peer_process`, and refused unless
the process is this user's). Its subtree is read from the process table, each pane (a direct child of the server)
attributed to its session's agent or one of its tabs by the `FARHELM_*` markers in its environment; then it is ended
with SIGTERM (and SIGCONT), and after a two-second grace SIGKILL, each signal sent only while the process is still the
pinned one and its command line still names the socket after `-S` (tmux renames the process, not its arguments, so the
host's last-argument test does not fit; `hosts::process::terminate_process_within` takes the test as a parameter), and
its socket and config are removed once it is gone. The reload then takes every old-kind row as having no terminal, in
any state, without looking its recorded pane id up: one whose agent was launching, running or being stopped is converted
to interrupted (`Transition::TerminalEnded`, beside the reboot conversion) unless its launch sentinel makes it an error,
its launch artifacts are removed as a settled launch's are, and it gets one `OldTerminalEnded` notification; one whose
agent had already ended keeps its outcome. Its `terminal_address` stays as stored, because the reservation settlement
reads a recorded one as evidence that the launch happened. Before that conversion commits, what the old terminals left
is swept, so that a crash part way leaves the rows for the next start to convert and sweep again: each converted row's
agent as a Stop of an agent that already exited is (`SweepTarget::AgentOnly` through the row's recorded launch scope,
the legacy session-only marker shape included), with the server's subtree under that agent's pane as extra roots, and
each old tab whose id can be found as a tab close is (`SweepTarget::Tab`): from a tab-marked pane in the subtree, or
from a tab scope unit the user manager lists for a session whose agent was converted or whose old tab the subtree held,
unless a terminal host's tab launch record names that tab. A whole session is never swept, since its host terminals
carry the same session marker; what cannot be attributed to an old tab is left for the session's Delete, which sweeps
the whole session. Once the server is gone, an old tab is found only through the scope units of a session whose agent
this start converted, so a scoped tab of any other session (its agent already ended, or in a terminal host), an unscoped
tab, and one whose shell's environment macOS hides wait for that Delete. The subtree counts only once the process
answering is verified as the server and signalled; otherwise none of it is swept. A sweep that runs out of time leaves
the rest for the session's Restart or Delete: the conversion commits regardless. A crash between the conversion's commit
and the artifact removal and notice after it leaves the row interrupted without its notice, its launch files for its
Delete; startup-only and short, so accepted, like the reboot conversion's same window. Every start also marks every
unresolved `old_terminal` notification resolved. A failure anywhere is logged and never fatal (the maintainer chose to
start anyway and treat old sessions as ended, accepting that a Restart could then start a second agent beside one still
running), and the step's own work, the server's end and the sweeps (not the reload's host discovery between them), has
one 15-second budget (`OLD_TERMINALS_DEADLINE`), half the 30 seconds the helm allows a supervisor it restarted during
Update to come back (`farhelm-helm`'s provisioning `ATTACH_TIMEOUT`).

## Helm ↔ supervisor transport: system ssh + stdio protocol

Podman routes add a local or SSH-nested exec pipe to this same transport, as described under
[Disposable environments](#disposable-environments). The SSH sharing, authentication and framing rules below continue to
apply to the SSH layer. Container routes share the child-process connection's diagnostic handling.

The helm shells out to the user's `ssh` binary (tokio::process) with connection sharing (`ControlMaster`,
`ControlPersist`) so interactive latency stays low and reconnects are cheap. Each host gets two shared connections, one
per purpose: the long-lived supervisor connection's master socket is the helm's state directory plus OpenSSH's own `%C`
(a hash of the resolved host, port and user) and nothing else, and every provisioning step rides a second master at the
same directory plus `p%C`. The two exist so Farhelm stays workable on hosts that restrict how a connection may be used.
Every ssh command is one session on its master, and on a host whose sshd allows one session per connection, a single
shared master would spend that session on the supervisor connection for as long as the helm is connected and refuse
every provisioning command; OpenSSH then falls back to a fresh connection, which cannot succeed under `BatchMode` where
every new login also needs an interactive approval. The split is unconditional, so both paths run on every host; on an
ordinary host it costs one more login when provisioning starts with no provisioning master open. Provisioning's name
carries its letter as a prefix because only a trailing `%C` stays an expansion token; the expanded names differ in
length, so a provisioning socket can never share a name with a supervisor socket. The supervisor socket fits the Unix
socket limit for the usernames SPEC.md supports on Linux and most of them on macOS; provisioning's is one character
longer, and each connection decides its own fit. Where a socket cannot fit, or the state path contains a literal `${`
(which OpenSSH would expand even inside the quoted `ControlPath`), that connection runs with connection sharing
explicitly off (`ControlMaster=no`, `ControlPath=none`) instead of failing. The supervisor is reached by executing
`farhelm internal stdio` on the remote side, which proxies stdio to the supervisor's unix socket. That Unix socket is
the supervisor's connection doorway. Its only network listeners are the IPv4 loopback ports a full-authority helm
connection claims for configured recipe tunnels; they are not protocol doorways and end with that connection.

The stdio helper writes one exact, newline-terminated `farhelm-supervisor-connected-v1` line to stderr after its socket
connect succeeds, before either copy direction starts. This proves only that a supervisor socket accepted the
connection; the framed hello still has to finish. Stdout remains exclusively the protocol stream, so the marker changes
neither its framing nor its version. Initial not-found/refused socket errors exit 75; other initial dial failures
exit 1. Provisioning still requires its own wrapper records before treating 75 as positive absence: the connected marker
grants no setup permission and supplies no resolved executable path.

The authenticated `POST /api/ssh-connections` composes read-only command help from registered SSH destinations and an
optional extra destination from Add host. It does not probe or mutate hosts. The helm uses its own state directory, the
same connection overrides, socket fit checks and ControlPath encoder as its SSH clients, then shell-quotes every local
argument. A relative state directory keeps its socket spelling and length gate: the pasted command uses a quoted
subshell that changes to the helm's working directory first. The masters use `-N -f`, `BatchMode=no`,
`ControlMaster=yes` and `ControlPersist=yes` so authentication can prompt and the idle connection stays available before
a client attaches. Unsupported paths are explained once per connection at the top of the response; `/api/hosts` still
exposes no helm state-directory field. The dialog reads once per opening and uses the existing native-first clipboard
path to copy exact command bytes. From Add host the parent stays mounted with its draft, hidden while the child modal
owns isolation; closing restores its isolation and focus. The SSH transport itself is unchanged.

The ssh child's stderr is piped and relayed as bounded, control-escaped tracing events attributed to the host, not
inherited. Inheriting is defensible for the single-host path a user started by hand; for a registered host it hands a
remote party a direct, unbounded write channel to the operator's own terminal, escape sequences included. The relay
drains continuously (a full stderr pipe would wedge the child), caps each line, stops logging after a per-connection
budget while still draining, and Debug-escapes what it does log. All peer-supplied text is normalized the same way
wherever the helm logs or retains it, including identifiers and successful-attachment trace fields, not only errors.
Host-manager diagnostics may be noisy and may repeat indefinitely: every failed attempt and refresh remains in the
stderr trail, including a peer that returns the same error on every refresh. A collision check still emits one summary
event for that refresh, rather than one event per colliding row. The SSH stderr relay's separate per-connection budget
still bounds which child lines it logs while it continues draining the pipe.

For failed connection attempts the relay also retains the newest two diagnostic lines, each capped at 512 raw bytes,
including a current unterminated segment. It continues observing markers and the tail after the logging budget is spent;
exact connected markers are excluded from diagnostics. Failure display uses at most 128 raw bytes per line, followed by
the existing bounded, control-escaped normalization of the assembled diagnostic. Live Connecting detail carries only the
closed stage vocabulary, never the current attempt's stderr. The process and its stderr can finish independently. An
early transport close while waiting for hello or final setup has one two-second allowance to observe natural exit and
drain final stderr, inside the same 20-second attempt budget; expiry snapshots immediately and adds no cleanup wait. The
read half and attempt diagnostics share child ownership, while historical observations own neither the child nor the
drainer. Dropping the last owner aborts stderr drainage and drops the kill-on-drop child; no detached waiter can keep a
cancelled connection alive.

Before hello, natural SSH exit 255 identifies an SSH connection/login failure, 126/127 identify a remote Farhelm that
could not be executed/found, and 75 identifies Farhelm with no supervisor running and supplies the start command, or
describes it when the shown command would not be exact (too long, or altered by the diagnostic's escaping). Other
numeric statuses, including zero before hello, report a failed remote command. Missing status, a signal or a failed wait
does not invent a numeric exit code. Already observed malformed or partial protocol data keeps its protocol diagnosis.
Without a current connected marker, a timeout remains uncertain between connecting/logging in and an older remote that
cannot report progress, unless this helm process already observed the marker for that host's current connection
configuration. That capability is memory only, scoped to the existing destination/program/state-directory snapshot; a
retarget does not borrow another endpoint's evidence. A completed hello is stronger evidence than the independently
scheduled stderr marker and always keeps the attempt in final setup.

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
control messages are serde_json, and data channels carry their stream's own bytes (a terminal's grid messages, a file's
or a relay's bytes). The same protocol runs over the unix socket locally, so "local host" and "remote host" differ only
in transport. Connection setup exchanges protocol and build versions; it refuses protocol-version incompatibility per
SPEC.md's version-skew rule, while build versions travel for diagnostics only — mixed builds with a compatible protocol
are the normal steady state SPEC.md describes.

Motivation: SPEC.md promises provisioning and transport ride "the user's keys, agent, and config", apart from
Tensorlake's identity policy — only the real ssh binary honors ~/.ssh/config fully (ProxyJump, Match blocks, the agent
used to authenticate, ControlMaster). russh was rejected for exactly that: partial config support would quietly break
the promise. What the config does not get is what rides Farhelm's connections: every ssh invocation carries `-o`
overrides (`CONNECTION_OVERRIDES` in `crates/farhelm-helm/src/ssh.rs`) for agent, X11 and port forwarding,
`RemoteCommand`, `RequestTTY` and `PermitLocalCommand`, which win over the config file. They set an OpenSSH 7.6 floor on
the helm machine, and settings whose override keywords are newer (`SessionType`, `StdinNull`, `ForkAfterAuthentication`,
all 8.7) are left alone rather than break older ssh. A port forward set up by a shared connection's master that predates
an upgrade survives if the new helm reuses that master within its 60 seconds of persistence (the ordinary path for the
macOS desktop app: quit, install, reopen), and lasts until the master exits (left unused for over a minute, its
connection to the host dropped, or `ssh -O exit` on its socket); see the `CONNECTION_OVERRIDES` docs.
`ClearAllForwardings` leaves ProxyJump working, because the jump runs as its own `ssh -W` child; that was checked with a
real connection through a jump on localhost. JSON control frames keep the protocol debuggable by eye; binary data
channels keep terminal and transfer throughput off the control path.

### Grid streams

Grid streams are the form in which terminals reach their viewers: the terminal runs in a terminal host, the supervisor
interprets its output, and it sends the screen as rows of styled runs. Protocol 68 added them beside the byte streams
that terminals in the supervisor's private tmux server sent, with a `grid` capability on `Attach`, a stream kind on
`Attached` and a `{"type":"stream","kind":...}` notice from the helm that told the browser which terminal component to
build. Protocol 77 removed the byte streams with the old terminals and xterm.js: every attachment's data channel carries
a grid stream, so `Attach`, `Attached` and the browser's socket say nothing about it, and `SessionInfo` and `TabInfo` no
longer report a terminal kind. The removal is a bump rather than an additive change because an older peer of the same
protocol would still send and expect what a newer one no longer reads (a supervisor streaming an old terminal as bytes,
a helm refusing a grid answer it did not ask for); a helm and supervisor of different protocols refuse each other at the
hello, so a remote host shows Needs update until it is updated. The helm keeps the last listing of each host in
`helm.db` as `SessionInfo` JSON, whose decode ignores unknown fields, so rows cached before 77 still read. A page loaded
from the previous helm and left open across the update must be reloaded; nothing keeps it working. Its terminals stop
reconnecting on their own once the page sees the helm's new build (the build-mismatch latch, which can take a few
seconds), and any reconnect from it, automatic in that window or manual, misdraws the terminal: that page builds its old
terminal component for a terminal whose kind the listing no longer names and takes the missing stream notice for an
older helm's byte stream, so it draws grid messages as terminal output, and what is typed into it ends the attachment as
malformed input. Keeping the stream notice for such pages was considered and not done: a reload is the whole remedy.

An attachment carries one message per data frame in each direction, and the helm relays them without reading them
(`crates/farhelm-helm/src/terminal.rs`, "Bounds and flow control"). It admits frames up to the grid bound of 1 MiB (one
row of the widest grid with its links can run to hundreds of KiB), up to 4,096 of them, holding no more than 16 MiB per
attachment; it forwards each browser message as one data frame and ends the attachment on a message over the 64 KiB
input bound rather than forward it; and it pauses the supervisor itself. The grid client sends no pause or resume (the
helm ignores one a page sends), and the helm reads its supervisor connection without waiting on any one terminal, so
without that a browser slower than the supervisor's link would fill its attachment's queue at the first large catch-up
(a full scrollback can be thousands of messages) and be detached as stalled. Instead the helm sends `PauseOutput` once
an attachment's queue holds 2 MiB or 512 messages, and `ResumeOutput` once it has drained to 1.5 MiB and 384. A paused
grid stream is sent nothing, history included, until the resume, then what it owes; it costs screen frames, never
output.

The supervisor ends an attachment that stays paused past its 60 second stall timeout, counted from the pause, which is
what ends a browser that stopped reading. It also sets the slowest browser that still reads and is not ended: from the
pause it must drain the 512 KiB between the marks plus whatever was already on its way (up to the supervisor's writer
queue, 64 messages of about 32 KiB, and what the link holds) within the 60 seconds. That is about 47 KB/s locally during
a scrollback catch-up, the realistic case for a pause, since the sender then fills its writer queue; about 10 KB/s when
little was in flight; and about 110 KB/s over ssh if its channel window and socket buffers hold about 4 MiB more, a
figure that moves with however much the link actually holds. The resume's own wait in the helm's shared writer queue,
behind other input, uploads and control messages, comes out of the same 60 seconds. Rearming the deadline while the
browser keeps draining would remove the floor, but needs the helm to report progress and the supervisor to act on it, a
protocol and supervisor change; the marks keep the floor low enough without it. The pause leaves 14 MiB of the budget
for what arrives after it, plenty at the sender's 32 KiB packing. Rows too large for that packing go alone in messages
of up to 1 MiB, and a writer queue full of them (output that makes huge rows) can exceed the budget; that ends the
attachment as stalled and the browser shows it as any stall until the user reconnects. The budget stays the hard bound;
per-attachment credits that would avoid it are not built.

The format is not permanent: it changes with ordinary protocol bumps, unlike the terminal host's protocol and snapshots.
Protocol 72 replaces the JSON `rows`, `frame` and `history` messages with fixed-field little-endian binary records. The
JSON encoding repeated keys and style arrays through the full scrollback; the committed grid benchmark supplies matching
workloads for measuring the replacement. A zero byte followed by the kind (1 rows, 2 frame, 3 history) distinguishes
these records from JSON. Controls (`hello`, `state`, `history_reset`, `clipboard`) and all viewer input remain JSON. A
peer never accepts the former row-bearing JSON as a fallback: it is an incompatible wire change, so the hello's protocol
check must refuse mismatched helms and supervisors before an attachment starts. The page is already gated on the helm's
build stamp.

Binary rows carry a byte length, a wrap bit and runs with fixed-width columns, widths and metadata. Metadata holds
twelve style flags and presence bits for nondefault foreground/background, an explicit underline color and a link index;
present default foreground/background colors are refused, while an explicit default underline stays distinct from
absence. Text and message-local hyperlink strings are length-prefixed UTF-8. No style or hyperlink dictionary survives a
message. Counts and lengths are bounded before allocation or slicing, every read checks remaining bytes, counters stay
within JavaScript's exact integers, and unknown kinds, malformed UTF-8, truncation and trailing bytes are errors. Shared
row, color, link and ordering validation still applies. Packing counts the actual binary envelope, rows and links
against the same 32 KiB target and 1 MiB hard bound; a whole row that exceeds the target still goes alone.

Its contract, bounds and golden fixtures are in `farhelm-proto` (`crates/farhelm-proto/src/grid.rs` and
`crates/farhelm-proto/grid-fixtures/`). Binary output fixtures pair bytes with JSON descriptions, including a fixed-seed
varied-row corpus and malformed records; there is no tolerated binary category within the locked protocol version.
`farhelm-proto`'s Rust tests validate every fixture and fail when checked-in bytes drift from the encoder. The grid
client's node tests decode the same binary records, compare their content and model effects with the descriptions, and
hold the test-only JavaScript feed encoder byte-equal to Rust. They also apply every JSON control fixture and compare
the input builders with the valid input fixtures, some of which are allowed wire forms the client never sends (an empty
last paste part, for one); they also hold the client's hand-copied input bounds to the invalid input fixtures.

`SessionInfo` carries a `last_activity_at` (unix seconds) beside `created_at`: the last time the supervisor observed
that session's agent terminal change. It was added WITHIN protocol version 11 rather than bumping it, per the running
rule every version since 3 has followed — a new optional field with a decode default, whose omission a receiver can
ignore harmlessly, is additive; a new tagged variant or a required field is not. Absent means the sender predates the
field and decodes to 0, which a receiver reads as "unknown, fall back to `created_at`" and never as an instant in 1970.
This is recorded here because the per-version changelog in `lore/` is frozen at the moment it was written and is not
maintained as the protocol grows. The identity hook's pair — `ControlMsg::ReportConversation` and `ConversationReported`
— went the other way and took the protocol to version 12, since two new tagged variants are exactly what an older
decoder refuses outright instead of ignoring. The pair was later removed without a bump, when reports moved to files
(see "The per-launch identity hook"): only a hook ever sent it, a helm and a supervisor sharing a version never exchange
it, and a hook older than its supervisor during an upgrade is the accepted gap that move describes.

Protocol version 31 gives every terminal detach a code beside its reason. `ControlMsg::Detached` carries a `DetachCode`
(`taken_over`, `stalled`, `tab_closed`, `replaced`, `other`), a non-displacing attach refused because another client
holds the terminal is `ErrorKind::TakenOver` rather than a `Conflict` recognized by its message, and the helm forwards
the code in the browser's `detached` notice. Clients decide behavior from the code only (the browser latches a takeover,
holds a stall, and hides a closed tab) and show the reason as text, so reason wording is free to change. Before 31 the
helm and `terminal.js` compared English sentences they each kept a copy of, and only the disabled browser suite would
have noticed a rewording.

The helm also uses `tab_closed` for a tab attach refused as `NotFound`. That refusal describes current state, not a
passing condition: the supervisor looks the tab up among its live tabs, and a session or host that cannot be found has
taken its tabs with it. Reported as `other`, it sent the browser's reconnect ladder through every attempt for every
listed tab, rebuilding a terminal each time. The browser stops retrying on it, but paints the reason as a banner rather
than hiding the pane, because silence is the reap's promise for a tab this view had working. The agent terminal's
`NotFound` stays `other`: its reason is the supervisor's account of the session, such as a reboot and what restart would
do.

### TCP relay channels

Protocol version 54 adds helm-only raw TCP relay channels for the Mac app's localhost tunnels. `BeginTcpRelay` fixes a
nonzero channel's bidirectional byte meaning; `TcpRelayStarted` correlates admission, while `TcpRelayAck`,
`CloseTcpRelay` and `TcpRelayEnded` belong to the channel. Older peers cannot decode these tagged variants, so remote
hosts require updating. The supervisor checks session existence and confines the target to literal loopback, or tries
IPv4 then IPv6 loopback for named-localhost and wildcard targets; it performs no DNS lookup.

Both directions use the attachment chunk cap and byte window, with cumulative acknowledgements only after the target
socket accepts the bytes. TCP reads are forwarded immediately, including short reads: waiting to fill a file-sized frame
would deadlock a short HTTP request whose client is waiting for a response. A shared bounded byte inbox coalesces
arbitrary packet boundaries without the file-download queue's full-frame assumption. Demultiplexers never wait for a
consumer. Supervisor admission, data and end share the existing bulk FIFO below interactive traffic; the helm's outgoing
data retains the existing connection-wide upload enqueue allowance through the writer. Channel ids remain reserved after
ending or cancellation, and a relay cannot inherit an earlier terminal/upload/download channel.

Each relay future owns its accepted TCP socket, route and pending admission cleanup. Connection death or cancellation
ends parked work. `TcpRelayEof` ends only its sender's direction, after preceding data in the same FIFO; the receiver
drains those bytes and shuts down its socket's write half while the opposite pump continues. Both directions ending
completes the relay; a failure cancels both pumps and cannot be overwritten by a later success. There is no per-relay
idle timer: an idle WebSocket is healthy, and task ownership plus transport teardown supply cancellation. Existing
shared-writer stall limits still apply to actual transport writes. Session-authenticated peers cannot open, acknowledge
or close these channels; the restricted operation allowlist does not admit them.

The same relay pump carries supervisor-accepted connections. A full-authority connection sends `ClaimTcpTunnelPorts`
once for at most 32 distinct nonzero ports other than 22. The supervisor binds every IPv4 loopback listener or none, and
a failed claim can be retried. There is no release or takeover: listeners, pending accepted sockets and active relays
belong to the claiming connection and are withdrawn before its unrelated teardown waits. The supervisor knows neither
the environment provider nor the recipe's destinations. The helm restricts claims to environment hosts.

`TcpTunnelAccepted` names only the claimed port and a connection-local accepted id. The helm selects a destination from
its own recipe snapshot, dials it with a bounded timeout, and either sends `BeginAcceptedTcpRelay` with its own fresh
channel id or refuses that acceptance. The supervisor never chooses a destination or a channel. A supervisor-wide
64-connection budget covers accepted sockets still waiting for an answer and active accepted relays; excess sockets
close at once. The helm independently reserves one of its 64 permits at event ingress, keeping it through queued events,
destination dialing and active relays, so an untrusted supervisor cannot exceed that bound. Session-authenticated peers
cannot claim ports or manage either kind of relay. These messages and directional EOF require a protocol bump.

The embedded helm owns IPv4 loopback listeners by session, canonical target and remote port. Each listens on a
kernel-selected port bound without `SO_REUSEADDR` and never on the remote port number: that number could be a port a
local program expects to use, such as the standalone helm's 7433 (the rule began when a signed-in browser kept its helm
credential at that origin), and on macOS a reusable `127.0.0.1` bind may shadow another program's wildcard listener.
Reuse stays off for the kernel-chosen bind as well, since BSD-derived kernels check only exact-address clashes when
picking a port for a reusable socket. If the kernel hands back the remote number by chance, the helm binds again while
holding it. Listeners live in memory through agent stop/restart and host disconnect. Every accepted socket resolves the
current coherent session route again; the original host and any known installation identity must still match. Successful
deletion or replacement closes the old session's listeners and admitted relays; failed deletion preserves them. Shutdown
closes them before host connections, and weak application references keep an idle listener from retaining the helm.

Helm schema 55 adds nullable `preferences.tunnel_consent`. Only true authorizes publication. The mutation gate orders
listener creation, consent persistence and successful deletion cleanup; it never covers relay traffic or the native
opener, which may block in the OS. A successful false/null write closes all listeners and active relays before its
response. The write and close tail run in an owned task so HTTP cancellation cannot persist revocation without cleanup.
A failed database write preserves both the previous consent and active resources. No tunnel URL or opener diagnostic is
logged, and the listener has no authentication beyond its loopback bind.

An agent's tunnel approval also carries its requesting host and connection into that publication path. The manager
compares them to the exact owner/client snapshot used for the listener; a preceding approval lookup cannot authorize a
different owner found after adoption or reconnect. The native terminal route is the user's direct selection and has no
agent-origin restriction. The UI keeps an accepted warning's action claim until its consent save finishes, even if its
source view retires; the existing source-generation check then suppresses opening from that retired gesture.

### Multiplexing, chunks and writer priority

The [transport above](#helm--supervisor-transport-system-ssh--stdio-protocol) carries control replies, terminals,
uploads, downloads and [TCP relays](#tcp-relay-channels) on one framed connection per host. A frame has a four-byte
length followed by a one-byte kind, a four-byte channel id and its body; channel zero carries control. The 8 MiB
`MAX_FRAME_LEN` ceiling counts kind, channel and body, not the length prefix. It bounds decode allocation, not the
ordinary data chunk. Each side has one writer, which writes and flushes one complete frame before taking another;
partial frames from different channels never interleave.

Uploads, downloads and relay data use at most 32 KiB per frame and a 4 MiB byte-credit window per stream direction.
Acknowledgements count cumulative bytes. Uploads earn one after each disk write; relays return credit after a socket
write. Downloads earn one when the helm's consumer takes a chunk from its bounded receive queue, before it writes or
forwards those bytes. Uploads are rechunked independently of HTTP body boundaries. Downloads fill frames across short
file reads and send a partial frame only at EOF. Relays forward short socket reads immediately rather than waiting to
fill a frame, as the [relay contract](#tcp-relay-channels) explains. Grid messages target 32 KiB and may need a larger
frame for one row. Terminals use [pause and resume](#grid-streams), not transfer credit.

The 32 KiB transfer cap matches OpenSSH's default session-channel packet size
([`CHAN_SES_PACKET_DEFAULT`](https://github.com/openssh/openssh-portable/blob/master/channels.h)), so larger frames
bought little on remote hosts. The window remains 4 MiB: smaller chunks increase the number of frames and acks per
window, without changing its byte credit. Protocol 71 makes this incompatible change explicit. A former peer can send
256 KiB chunks that the new size checks refuse; in the other direction, its 17-slot file receiver cannot hold a new
128-frame window. The hello therefore refuses mixed protocol versions before transfers begin.

The writer queues deliberately differ by direction:

| Writer             | Queues and selection                                                                                                                                                                        |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Helm to supervisor | One 64-frame FIFO, with heartbeat selection biased ahead of it. Upload and relay data share four enqueue permits across the connection, held until each frame is written.                   |
| Supervisor to helm | Priority (32 frames, upload and relay acknowledgements), ordinary (64 frames, terminal output and control replies), then bulk (downloads and relay traffic), selected in that strict order. |

The supervisor's bulk queue and each upload's receiving queue use `ceil(window / chunk) + 1`: now 129 frames rather than
17, still about 4 MiB of full-sized payload. The helm's download receiver uses the same derivation. Fixed queue depths
and scheduling are unchanged. The helm's four-frame allowance now bounds bulk payload queued ahead of a keystroke to 128
KiB rather than about 1 MiB. This is no latency promise: bytes already accepted by the pipe, ssh or TCP buffers remain
in flight.

Terminal output from a host intentionally takes precedence over downloads and tunnel data toward the helm. Those
transfers can slow or stall while terminal output stays heavy. Uploads and tunnel data toward the host are not queued
behind that output; their returning credit has priority over it. A frame already being written is never preempted. On
either writer, a 60-second window with no byte progress ends the entire connection and its channels. Progress rearms the
window without dropping or retrying a partial frame; a flush that never completes is bounded too. This is separate from
a terminal's pause deadline and from an upload's progress deadlines.

### One owning helm per supervisor

SPEC.md (Topology) makes one helm the owner of each supervisor and every session on it. Design for exactly that. Do not
add code, state or protocol whose purpose is to let a supervisor or session be shared between helms or move from one to
another, and do not justify a choice by it ("this is the rule that stays correct if several helms connect" is not a
reason). When the supervisor has to pick a helm connection to send something to, it picks the owning helm's connection;
which connection a session's terminal happens to be attached through says nothing about ownership. Policy (approvals,
permissions, templates) belongs to the owning helm, so the supervisor never has to reconcile policy from more than one.

What a supervisor does see more than one of is connections, all from the same owner or from tools that are not helms:
the helm's host connection, its previous one while a reconnect leaves the old one half-open, the helm's provisioning and
update probe, other full-authority tools, and session-authenticated peers such as an agent's `farhelm` command.
Per-connection state (channel ids, request ids, attachments, tunnel claims) stays per connection, and the guards that
keep a reconnecting helm's overlapping connections apart are correct and stay. They are not multi-helm accommodations.

Ownership is not enforced yet: the supervisor stores no owning-helm identity and treats every full-authority connection
alike (TODO.md, Near term). Until it is, a second helm connecting is out of scope rather than handled. Code need not
detect it, defend against it, or degrade gracefully under it.

### What the helm believes from a supervisor

The helm's trust in an attached supervisor is scoped by effect, not by connection. A supervisor is believed about things
that affect only its own host: which of its sessions sent an upcall, that one of its sessions exited, what its terminals
show. A lie about those damages only that host, where the supervisor already has full authority. Anything whose effect
reaches beyond its own host (another host's sessions, another supervisor, the helm's machine, or the helm's own state
such as settings and credentials) is allowed only where the spec grants it, and a request's arrival on a supervisor
connection adds nothing to that grant. The grants are the agent verbs any agent may use, each carried out only once the
user has approved it or the requesting host's "Run farhelm commands from this host without asking" setting is on
(SPEC.md, Agent-spawned sessions; see "Permission prompts for agent actions"). Changes to hosts themselves (an agent's
host add, edit and remove) happen only on the user's Allow of that one request, never on the connection's authority:
neither the requesting host's "without asking" setting nor an Always allow answer covers them, and even an approved edit
reaches only a host's alias, icon and color, never its permissions or destination.

### Errors crossing levels of abstraction

An error code or response at one level of abstraction is never, by default, equivalent to one at another level. The same
name or number does not carry the same meaning once it crosses a layer: a supervisor's `ErrorKind::Unauthorized` says a
session-scoped peer asked for something outside its slice, while the helm's HTTP 401 tells a browser it is signed out.
Translating a code across levels is valid only through explicit, case-by-case reasoning, recorded where the translation
happens. A blanket mapping from one layer's codes to another's is exactly the default equivalence this rule forbids,
even when every current code happens to line up.

The helm applies this at its HTTP boundary. `SupervisorError` records whether the helm decided the kind itself or built
it from a supervisor's `ControlMsg::Error` reply, and `http_error` translates a supervisor-decided kind through
`supervisor_reply_status`, where each arm says why its status is the right one given that the supervisor is untrusted. A
supervisor's `Unauthorized` becomes 502, never 401. Signals that make a client act on its own state are produced only by
the helm's own reasoning: the device-authentication 401 comes only from the auth middleware, the definitely-unaccepted
create outcome only for a local failure, a supervisor's `CheckoutConflict` (whose mkdir it owns), or the narrowly scoped
create-refusal proof below, and the stale-connection precondition only as a header from the helm's own precondition
check, never from anything a supervisor wrote in a message body. The agent relay's error classification (`error_kind`)
and the terminal WebSocket were not audited against this rule when it was written.

Protocol 53 adds `create_unaccepted` to the existing `Error` reply. An ordinary keyed create can set it only when the
reservation matches the full request and key lifetime, has settled `Failed`, has no surviving session row, and is within
the 24-hour refusal window. Legacy tombstone fingerprint matching remains readable; new refusals keep the fingerprint
and error detail as written, and new Deletes remove the reservation. The ordinary rollback paths settle that state only
after confirming the session's terminal is gone; a retained or ambiguous launch does not qualify. That is not proof that
every descendant of the failed launch stopped: the rollback after a failed terminal creation accepts a systemd scope
kill it could not confirm once the portable sweep finds nothing, and after a failed database confirmation a sweep error
only adds to the returned error. A survivor of either can then keep running beside the fresh attempt this proof allows,
which needs two coinciding faults and falls under SPEC.md's two-fault exception in "Lifecycle operations". Keeping the
session when such cleanup is unconfirmed is deferred in TODO.md's entry on requiring a systemd user manager on Linux.
Managed-checkout and agent-request recovery encodings are excluded. This is reply metadata: the stored refusal and its
same-key replay do not change. Missing proof, including a failed proof lookup, stays inconclusive. The helm accepts this
host-local statement only on a `CreateSession` reply and translates it to its existing definitely-unaccepted HTTP
marker. A flag on a failed Delete cannot retire a successful replacement's identity. Launchers drop a create identity on
that marker, so repairing a missing folder and pressing Launch again starts a fresh attempt without weakening recovery
after an uncertain or partially successful attempt.

`SessionInfo::last_work_started_at` is the millisecond ordering key for the session list's stable work bursts. It was
added within protocol version 20 under the same additive rule: absent decodes to zero, and zero falls back to
`created_at * 1000` with saturating integer arithmetic. It never falls back to `last_activity_at`, because continued
output would then undo the stable ordering. A stale mutation reply merges this field by maximum in both the helm's
in-memory and durable caches, so delayed request traffic cannot move a later supervisor observation backward.

Version 13 adds the one shape on this wire that travels UPWARD as a request: `ControlMsg::AgentRequest`, answered by
`AgentResponse`. Both legs of its journey carry the same pair. An agent inside a session dials its own supervisor's
socket with the per-session credential — exactly as `farhelm spawn` does — and the supervisor forwards the request to
the helm, because the session has no route, address, or credential back to the machine the helm runs on. Nothing else
changed direction then: the helm still learns about sessions by drain, so version 10's "no supervisor-edge push channel"
held for everything but this (version 33's change hint, below, carries no session data either). The supervisor sends the
request to the owning helm's newest connection; version 73, below, is how it knows which connection that is. Request ids
are per connection on this protocol and stay that way: the asking process numbers its own leg, the supervisor numbers
the upcall from a counter it keeps per helm connection, and the relay holds the mapping for one round trip.

Version 73 changes how the supervisor picks that connection. Until then it picked the one holding a terminal attachment
to the asking session, on the reasoning that this was the helm the user was looking at and would stay correct if several
helms per supervisor were ever supported. Both halves were wrong. The UI attaches only the session on screen, so an
agent in any other session was refused with "no helm is attached to this session" while its helm was connected the whole
time; a monitor session that launches and deletes other sessions in the background could not use agent commands at all,
and switching the UI away from a session broke its agent's commands within seconds. And several helms per supervisor is
ruled out by policy rather than left open: one helm owns each supervisor and every session in it, so there is only one
right helm to ask, and which session it displays says nothing about that. The supervisor now routes by connection. The
helm's host connection, the only connection built with an agent-request handler, declares itself in its hello
(`ControlMsg::Hello::owning_helm`), and the supervisor registers an upward link only for a full-authority connection
that did. Requests go to the newest such link, because the one case with two is a helm reconnecting while its old
transport is still half-open, and the old one may never answer. With none registered the request is refused at once with
"no helm is connected to this host". The declaration exists because the supervisor cannot tell a helm from other
full-authority peers by anything else (`role` is diagnostic text): the provisioning and update probe sends the same
`role: "helm"`, sends nothing after its hello, and can connect while the host connection is up, so "the newest
full-authority connection" would hand requests to a probe that never answers them. The field is not additive within a
version, because a helm that omitted it would silently stop receiving agent requests; hence the bump, and since a
mismatched peer is refused at the hello there is no older helm to fall back for. Only declaring connections receive
change hints too (version 33, below), which loses nothing: in production the host connection is the only one that reads
them.

Version 33 adds a second message in that direction, `ControlMsg::SessionsChanged`: a content-free hint that something a
user can see about the host's sessions changed. The helm still learns WHAT changed by drain; the hint only moves the
drain earlier. The supervisor owes a hint whenever a value it would list actually changes (a terminal exiting, a status
transition, the activity and work-start stamps advancing, a restart offer changing, and every lifecycle mutation), never
merely because a listing was served, so hints and the refreshes they cause cannot feed each other. It coalesces owed
hints into at most one per 200 ms and sends each on the ordinary writer queue of every connection that declared itself
the owning helm's (version 73, above), dropping it rather than blocking when that queue is full. Replies never run a
capture sweep or commit outcomes; a terminal-exit wake hints before observation and an outcome change hints again. The
helm refreshes the hinted host at once, unless a refresh that started with a hint pending began less than that same 200
ms ago (`SESSIONS_CHANGED_MIN_GAP` in the protocol crate, which both ends use); then it waits out the rest of the gap,
and the one refresh that follows answers every hint that arrived meanwhile. Supervisor messages are untrusted, so the
helm enforces the gap itself rather than relying on the supervisor's spacing. Only a hint's own wake waits: the poll, a
user's refresh and a nudge do not. A refresh a hint caused always raises a feed event, even when the cache compares
equal: a tab opened and exited between two refreshes leaves the cache unchanged but a client's optimistic tab behind.
The three-second poll stays as the backstop and keeps its changed-only rule. A hint stays pending until a refresh that
STARTED after it completes; a refresh the helm discarded because one of its own seeded writes overtook it is retried as
soon as the gap allows rather than left to the poll.

The helm's client closes its transport when its final owning handle drops, even with an unanswered upcall. Answer tasks
hold writer senders of their own, so closing the client's sender alone cannot make a quiet connection reach EOF. The
destructor aborts its registered answer tasks and signals the existing reader and writer cancellation paths.
Cancellation also interrupts a frame already being written: the connection is closing, so preserving frame
synchronization cannot justify retaining its transport until a live-peer stall timeout. The manager also retires a
withdrawn connection explicitly: it must close while callers still retain obsolete handles, rather than waiting for
final-owner cleanup. Neither path cancels a mutation that has started: only read-only answers are registered for abort
(see "Who owns an accepted action").

The failure vocabulary is two kinds, split by whether a retry is free. `ErrorKind::Unavailable` means nothing is holding
the request: no helm is connected to the host, its connection died before or during the request, the request could not
be delivered onto that connection at all, or the helm itself is not in a state to answer — still starting up, shutting
down, or serving a connection with no fleet behind it. `ErrorKind::Timeout` means the opposite and only that: the
request was QUEUED for delivery on the helm's connection and no answer arrived within the supervisor's budget. It may or
may not have reached the helm — the supervisor observes its own writer queue accepting the frame, never the writer
transmitting it — so a retry is neither provably free nor provably duplicative, and the message says exactly that rather
than claiming a delivery nobody watched. Closing that residual gap needs a per-frame transmission receipt from the
connection's writer, with the answer budget starting at the receipt and a write failure before it reported as
`Unavailable`; that is the known refinement, deferred because the queue is one `mpsc::Sender<Frame>` shared by every
attachment site in the supervisor and a receipt changes that type at all of them. Both kinds are emitted by the relay
and by the helm; the earlier rule that only the supervisor emits them was too narrow, because the helm has its own
transient states and a bare `Internal` would tell a caller nothing about retrying. The DELIVERY leg gets its own short
budget (5 seconds) separate from the helm's answer budget (30 seconds, plus the nine-minute approval wait for a verb
that may wait for the user; see "Permission prompts for agent actions") precisely to keep this distinction honest: a
request that spent its whole budget waiting for room on a full writer queue was never sent, and reporting that as
`Timeout` would invert the one thing the two kinds exist to say. Both budgets live on the supervisor because it is the
only party that can tell them apart; the asking CLI blocks with no deadline of its own so that the specific answer
reaches it.

A MUTATING verb is fenced against its own asker being deleted mid-flight, and its failures speak a different vocabulary
from a listing's. The credential that admits an `AgentRequest` is validated once, but a rename/stop/restart stays in
flight to the helm and back for as long as the user takes to answer its card, up to nine and a half minutes, which is
ample room for a `DeleteSession` to revoke that very credential underneath it. So the supervisor claims a
per-asking-session fence (`Supervisor::agent_request_locks`) BEFORE it checks the credential — checking first and
claiming after leaves a gap a whole delete fits inside — and `handle_delete_session` waits on the same key before
tearing anything down. The fence is released when the MUTATION ends, not when the CLI's answer budget does: a budget
expiring says nothing about whether the helm is still working, so the guard is held until the helm answers or the
connection dies. That is bounded by the LINK's life rather than by a clock, which is only a bound if the link can be
counted on to end — and it cannot, because a response naming no pending entry is dropped, which is right for an ordinary
late answer and indistinguishable from a helm answering under an id it has already used. So the retention has a last
resort of its own (ten minutes), and its expiry RETIRES THE LINK rather than dropping the guard: dropping it would be
the same budget-shaped release on a longer clock, still guessing that the mutation ended, whereas ending the connection
makes every pending upcall on it resolve as the delivered-outcome-unknown ending the relay already speaks. A response
correlated to a `req_id` that was NEVER ISSUED is retired the same way and immediately, on both legs of the relay: it
cannot be a late answer, so the only readings are a broken peer and a hostile one, and on a connection that stays
healthy the waiter it strands has nothing else to end it. Correspondingly, a connection lost after the request was
queued is reported to a mutating caller as `Timeout` ("delivered, outcome unknown") rather than `Unavailable` ("never
delivered, retry freely"), with a remedy that says to look at the session before retrying — the change may already have
taken effect, and the retry-safe kind would be an invitation to apply it twice. A listing keeps `Unavailable`, having
nothing to double-apply. The fence's "until the connection dies" is accepted as a gap: the helm owns a mutation it has
started (see "Who owns an accepted action") and may still be carrying it out after the link that asked is gone, so a
delete of the asking session can proceed while, for example, a create it asked for still lands. Nothing is lost when it
does: the new session appears in the list like any other. Which verbs are mutating is `AgentVerb::is_mutating`, one
exhaustive match in the protocol crate that both the supervisor and the helm read, so a verb added later cannot be
fenced on one side and not the other.

That vocabulary is a rule about a PHASE, not a list of failures, and every hop applies it the same way: once a mutation
has been handed to the next hop, the only endings that may speak plainly are the expected success reply and a refusal
the peer itself authored. Everything else — the link dying, a frame that will not decode, a reply correlated to another
request, a well-formed reply of the wrong shape, a reply of the RIGHT shape whose payload this side has to refuse (a
created session id that breaks the ingress rule: empty, past the cap, carrying control characters, or `.` or `..`), a
refusal that could not be enqueued — is delivered-outcome-unknown for a mutating verb, because each of them leaves the
same question unanswered and a peer broken enough to produce one is not thereby proof that nothing happened. The
refused-payload case is the one where "the peer probably did it" is strongest rather than weakest: the target answered
with the very reply that says the session was started, and the id that could address it afterwards is precisely what got
thrown away. The helm's own connection to a target supervisor enforces this by ending rather than by guessing: an agent
answer it cannot even refuse (its writer queue full) closes the connection instead of dropping the refusal, since the
link dying is the terminal event a mutation's retained fence is waiting for, and a silent drop on a link that then
recovers holds that fence until the retention's own last-resort bound expires. A wrong-shape reply is named by its
`ControlMsg` VARIANT and nothing else, for the same reason the phase rule exists at all: the full rendering of a legal
near-frame-limit listing pushed the agent's own reply frame past the protocol limit, whereupon the size backstop
replaced the whole outcome and the mutation vocabulary was lost to a bare `Internal` — and it carried a session's raw
invocation and cwd into an agent-facing error chain besides. That backstop now preserves an outcome-unknown verdict's
kind and remedy when it has to drop oversized prose, for the same reason: a size check must not be able to revoke a
claim about durable state. The phase rule is confined to the AGENT path (`agent_requests::transport_outcome`) rather
than folded into `error_kind`, so the REST surface keeps mapping the same transport failure to `Internal`: an HTTP
caller has its own idempotency story, and inventing a status for this would be a contract change no client asked for.

The class that decides the vocabulary is the failed REQUEST's, not the verb's, and clone is where the two come apart: it
snapshots its source with an ordinary listing before any create is dispatched, so a transport failure there is a
listing's — retry-safe, with no remedy attached — even though the verb around it mutates. The helm marks that phase at
the read itself rather than inferring it at the classifier, since only the caller knows which of its requests had
nothing at stake.

The relay identifies the origin host by its connection. The supervisor authenticates the per-session credential and
refuses a peer asking as a session it is not; from there the helm accepts the forwarded `session_id`, and the claim that
the connection it arrived on belongs to that session's host, without re-verification — it never sees the credential, so
there is nothing on its side to check against. The host already controls its own sessions, so their credentials do not
provide containment from that host. Accepting a forwarded session claim does not make the supervisor's messages trusted
or authorize effects outside the named fleet operations, which act across hosts only with the user's approval or the
requesting host's setting (see "Permission prompts for agent actions"). Provisioning a supervisor does not establish
trust in its responses. What the helm does check is that the connection is still the CURRENT one for that host row,
since registry rows outlive the machines behind them. Version 14 replaced session-list pagination with a bounded
whole-list reply, and version 15 carried helm-resolved launch bundles and upward profile resolution, which protocol 38
removed with profiles. The historical paragraph below describes why 13 was current at the time; later released additions
took the wire to 26. Version 16 introduced the durable optional structured launch snapshot carried with a create and
`SessionInfo`. The snapshot is declarative provenance beside the resolved invocation, never a browser-owned compiler
input; old sessions remain absent rather than being reconstructed from a command. Version 17 adds `BrowseDirectory` and
`DirectoryListing`: the helm routes one authenticated, connection-incarnation-guarded request to the chosen supervisor,
which expands `~` from its own recorded home, canonicalizes the requested directory, and returns only a sorted bounded
immediate child-directory listing plus parent and truncation state. Neither the helm nor the client reads the target
filesystem. Version 18 adds accepted-create `canonical_cwd`, the identity fact that binds folder history to the
destination the target supervisor actually accepted. Version 19 adds OpenCode to the structured-harness enum. Version 26
retires session archival and its wire fields and messages. A supervisor must retain that snapshot alongside the resolved
invocation, so an older peer that cannot decode the new enum value refuses the connection rather than silently losing
the selection. The following 13 paragraph is historical context, not the current protocol version; the frozen changelog
stops at 11. Version 13 also carries `AgentVerb::Rename`/`Stop` and the two creating verbs `AgentVerb::Create`/`Clone`
(answered by `AgentReply::Created`), all added additively within the version rather than as version bumps of their own —
which was possible ONLY because 13 itself had not yet shipped when they landed, still being developed on this branch
with no released build speaking it yet. That is a one-time allowance for a version still in flight, not a standing
license to keep adding to 13 after it ships; once a protocol version has shipped, a wire-shape addition needs a version
of its own, same as any other. The same allowance covers the one thing in 13 that is not an addition at all:
`AgentSession::host` became `Option<String>`, so a reply carrying a row the helm just mutated or created can say "there
is a session here but no host name I can vouch for" instead of encoding that as an empty string indistinguishable from a
real value. A decoder built against 13 EARLIER IN ITS OWN DEVELOPMENT rejects `host: null` outright — the running
additive rule does not stretch to cover it under any reading — so it is allowed here only because nothing released
speaks 13 yet. It must not be carried forward the same way once 13 ships: the identical edit made afterwards needs a
version of its own.

Version 25 adds `AgentVerb::Restart`, `AgentReply::Restarted`, and the non-secret `AgentSession::restart_offer`
discovery field. The new tagged request and reply require an exact-version handshake refusal for older peers. Each verb
is routed and recorded through the same `sessions.rs` functions the corresponding REST route uses — `route_session`, the
client call and `record_session` for the lifecycle four, and `do_create_session` (the shared internal function
`POST /api/sessions` was refactored onto) for the creating two. So a refusal that comes out of the SHARED operation — an
unknown session, a disconnected host, a title the owning supervisor rejects — is the identical sentence the UI would
have shown, and a session an agent creates is seeded into the helm's cache and published exactly as one the create
dialog made.

The equivalence covers that shared path and stops there, deliberately, besides the user's approval, which every acting
agent verb passes first, and the stricter YOLO rule create, clone and spawn also pass (see "Permission prompts for agent
actions"). The relay adds a doorway check of its own (`validate_agent_verb`) that the REST surface has no counterpart
for, since only the relay puts an attacker-chosen target, title, directory and host name onto two byte-unbounded queues
before anything downstream can look at them; its refusals are relay-specific by construction. The helm's agent doorway
also accepts the exact asking session's bare ID on its requesting entry and points other bare or malformed targets to
`farhelm agent sessions`; REST requires a full ID and tells stale browsers to reload. And the agent CLI escapes and caps
a refusal before printing it, because the destination is a terminal rather than a browser — so a shared operation's
WORDING is the UI's, while the bytes may be escaped and the tail cut. The relay's bounds can refuse a request before the
shared operation; they do not relax that operation's checks.

`Created` is a distinct reply tag from `Session` even though the payload is identical, because the tag is the only thing
separating "what your creating verb produced" from "the row you changed" and the CLI checks it before printing an id. It
is not a novelty claim: a create or clone carrying an idempotency key the target has already served replays that
session, which arrives under this same tag. The relay stores an agent's key on the target as
`agent-v3-<SHA-256 of the registry namespace, host entry UUID, session and key>`, with length-delimited strings. Each
helm database has a durable random namespace, minted at creation or migration, so a fresh registry cannot reuse an
earlier registry's approved retry when numeric host rows coincide. The entry UUID keeps approvals tied to the logical
host across adoption. A number reused after rollback cannot inherit another entry's approval.

Including the entry UUID changes retry keys: an in-flight keyed retry across this upgrade needs approval again and, if
approved, creates a new session or checkout instead of returning the earlier one. The namespace is read only for keyed
agent requests, so offline commands can still open older schemas without migrating them. The fixed-length hash keeps any
agent key within the target's intent-key limit. The helm draws the one distinction that matters from it — a clone whose
result is the asking session or the named source is refused rather than reported, before anything durable is recorded
for the replayed row. The two creating verbs name their target host by display name, matching `AgentHost::name`.
Registry IDs join host and session discovery and distinguish duplicate labels; they are not destination selectors. A
name matching two registered hosts is refused as a `Conflict` naming the collision, never resolved to whichever row the
listing ordered first: display names are not unique by construction (the local row renders as `this machine`, and an ssh
destination may be spelled the same), and guessing between them would put a session on a machine nobody chose. A
registered name carrying a control character can never be typed back at all, since the relay refuses one in `--host`;
the not-found refusal says so by count, because the fix is a rename and an agent has no rename verb for hosts.

An agent's create sends a `LaunchRequest`: a command launch (`--command`, the required `--yolo`/`--no-yolo` assertion,
an optional declared `--agent` and `--resume-command`), which the relay's doorway checks by the command-launch rules
before forwarding. The type also has an agent variant (an agent type and its choices, composed by the helm exactly as
for the launcher) for the agent CLI's launch flags; it never carries composed commands, so an agent cannot hand the helm
arbitrary start and resume commands under an agent launch's YOLO verdict. A clone copies its source's stored launch
exactly; a legacy source is refused with a remedy, because SPEC.md has agent clone refuse a session from before launch
kinds.

`Clone` resolves the explicitly named source's live owner, drains that owner's pinned connection, and rechecks the owner
before dispatching to the destination. It does not use the helm's cache: a clone built from a cached row could copy a
title or directory the session no longer has, or read from a host that stopped owning it. It refuses a result whose id
and reporting entry together identify either the SOURCE or the ASKING session: a keyed replay must never be reported as
a new child. Equal bare suffixes on different entries are distinct sessions. Both verbs take the fence on
`agent_request_locks` that the lifecycle verbs take, since a create that completes while the asking credential is being
invalidated would otherwise leave a session running that nobody was told about.

A KEYED RETRY IS MATCHED BY THE REQUEST AS THE AGENT SENT IT, not by the launch it resolves to: a clone retried after
its source's launch, folder or title changed replays the first copy, and the same key with a different request conflicts
(see "Agent launches from the CLI").

The relay's own doorway bound treats the host name SEPARATELY from the create payload (`AGENT_HOST_NAME_CAP`, 1024
bytes). This unchanged wire bound is independent of the helm's 40-byte bare session-ID admission limit. A host name is
routing metadata the helm consumes and no supervisor ever sees, so charging it against SPEC.md's 64 KiB create-field
allowance would let a long registered host name make an otherwise-legal create fail through the agent surface alone.
What the aggregate covers is exactly what a create carries: directory, selector and title, summed.

And a created session's references are checked at INGRESS, where the reply enters the helm — its ID, parent and working
copy origin each satisfy the same non-empty, control-free, 40-byte bare-ID rule used for listed rows, including the
refusal of `.` and `..`. The created ID is the value on this path with a machine consumer: it is printed on the CLI's
stdout as the answer, and an id carrying a newline forges a second line in whatever captured it. A reply that fails the
check is refused rather than sanitized, because a scrubbed id is not the session's id and everything done with it
afterwards would address something that does not exist.

Every keyed create gets a session-lifetime reservation: app creates, agent `create`/`clone`, and spawn all retain the
key until the created session is deleted. This lifetime follows the created session on its target supervisor, regardless
of whether that supervisor knows the asking session. Caller scoping is separate: the helm relay namespaces every agent
key by the asking session and its host, so a key only ever replays for the session that used it. Refusals without a
surviving session retain their fingerprint and error detail for 24 hours; a retained error session keeps its reservation
until Delete, without an age limit.

This reverses the July 2026 keep-forever decision, which protected against a retry racing Delete and re-creating the
session. The maintainer chose session lifetime because a key that outlives its session protects nothing, losing a reply
and then deleting and retrying is rare, and an agent's fresh request still needs the user's approval under the ordinary
rules. An approved managed-checkout retry can finish that exact approved attempt without another card only while its
created session still exists; after Delete it is a fresh request too (SPEC.md, Local authority and trust between hosts).
Every spawn is an agent `create` placed on the asking session's own host and goes through the helm, so the user can be
asked first (SPEC.md, Agent-spawned sessions); there is no offline spawn. With `--inherit-agent` the asking session's
supervisor fills in that session's exact stored launch (`SpawnPlacement::inherited_launch`, overwriting whatever the CLI
sent) before relaying it, and the helm launches that launch rather than resolving templates and flags. A
session-authenticated `CreateSession` sent straight to the supervisor is refused outright (protocol 41), so the relay is
the only way a session creates one.

The discovery verbs are answered from the helm's own listings, narrowed to what an agent can name and act on. Two
narrowings are contractual rather than incidental. The session listing is the same whole-fleet listing the UI reads, cut
at the same cap (`LIST_SESSIONS_CAP`, a few hundred rows) and additionally at an encoded-byte allowance of 6 MiB —
leaving the reply's envelope room under the 8 MiB frame limit — and carries a `truncated` flag when either cuts it,
because a partial fleet listing is otherwise shaped exactly like a complete one and "that session does not exist" would
be indistinguishable from "that session is past the cut". The byte allowance exists because rows bound nothing about
size: session creation admits tens of kilobytes of caller-supplied text per row, and a fleet of legally fat records
would otherwise produce an answer no frame could carry — discarded whole, reaching the agent as `Internal` rather than
as the partial listing the verb promises. The per-session `agent` field is a non-secret label drawn from a closed
vocabulary — the word for the integrated agent kind the supervisor recorded for the session (carried on `SessionInfo`),
or `custom` when there is none — and never text derived from the invocation. Users put credentials in command lines,
this listing is readable with any one attached session's credential, and its reader is a model that will quote what it
read, so nothing from the command line may cross this wire at all. Even the program's basename is not safe: a leading
`NAME=secret` assignment is the first word.

A template catalog whose agent listing exceeds one reply frame (8 MiB, roughly 130 templates near the per-field cap) is
outside the supported scale. Agent template listing may then fail until templates are removed; the GUI's template
listing is unaffected by that frame limit.

The session list is served WHOLE on this wire (protocol 14). `ListSessions` carries nothing but its request id, and
`SessionList` answers with every session the supervisor has, cut at `LIST_SESSIONS_CAP` (a few hundred rows, one
constant in farhelm-proto that every layer reads) with `truncated` set when the cut applied. There is no cursor, no page
size, and no order contract: the helm sorts every host's rows itself, in memory, for whichever order a client asks, so
the sequence a supervisor chooses is not part of the protocol. Protocol 8 through 13 paginated this list by keyset
cursor with a per-page byte budget; that contract and everything built on it (cursors bound to order and filter,
drain-to-exhaustion with termination bounds, wire-order validation) is gone by decision, recorded in SPEC.md's Session
list section — the fleet this product serves fits in one reply, and the paging machinery was the largest source of
incidental complexity in the codebase. One case is still an explicit error rather than a cut: a reply whose encoding
exceeds the frame limit is refused at the writer's size backstop, so a host with a single record too large to ship
reports a failed listing rather than a silently shortened one.

### Session-host file reads

Protocol version 45 adds helm-only `StatFile`/`FileStat` and `BeginDownload`/`DownloadStarted`, with `DownloadAck`,
`AbortDownload` and `DownloadEnded` for the streamed read. Paths resolve on the supervisor against the session's
recorded cwd and the account home captured at daemon startup. The stat opens the file to prove readability, follows
symlinks, and reports a resolved absolute path, size and typed refusal. Begin opens and checks independently; a hover
reserves nothing. Only regular files up to 100,000,000 bytes are accepted. Reads run to EOF, with the cap enforced
before forwarding each chunk, so growth past the cap fails the whole transfer.

Download data uses the attachment upload's chunk size and credit window reversed. Acknowledgements are cumulative bytes
consumed, monotonic and no greater than bytes sent. Begin, data and end share a bounded bulk writer queue, drained after
terminal and control traffic; the connection reader never waits for a download's consumer. A helm-side guard owns
routing and cancellation from before Begin is sent through EOF. Dropping it retires any pending admission reply entry
and sends Abort, which need not produce a reply; connection death reports failure. Download channel ids stay reserved
for the connection's lifetime, including against attachment and upload reuse: queued frames can outlive an aborted
producer. The production helm already allocates each id only once. There is no upload staging, publication phase,
admission cap or progress timer. Session-authenticated connections remain behind the restricted operation allowlist and
cannot stat or read files.

The helm exposes authenticated JSON `POST /api/sessions/{id}/files/stat` and `/files/download`, with a `path` field.
Both use session-owner routing and the same auth-inside-desktop-CORS boundary as attachments; only OPTIONS preflight is
public. Stat returns the host display name alongside the protocol's resolved path, size and typed status. Download
reopens independently and streams an octet body with an encoded basename attachment disposition. It promises no opening
Content-Length: a file may change within the cap, and a later stream failure invalidates the whole body.

The desktop supplies its platform Downloads directory to the embedded helm. A download request with
`save_to_downloads: true` stages privately on that filesystem and publishes only after successful EOF, returning the
destination path as JSON. Atomic no-clobber publication uses `report.pdf`, `report (1).pdf`, and so on, preserving the
final extension. Failure or cancellation cleans the staging directory without publishing a partial file. A standalone
helm or an unavailable native Downloads directory refuses this save mode; there is no server flag or webview fallback.

The desktop injects a separate file opener alongside its Downloads directory: macOS `open` receives the published path
as one argument; Linux GIO dispatches its file URI without waiting for the default app to exit. Neither uses a shell or
hardened `webbrowser`. An `open: true` download request must also request `save_to_downloads` and have that capability,
otherwise it refuses before routing or transferring. After publication, the helm reads the shared preference row and
checks the saved file's final extension against its stored list, ASCII case-insensitively. A NULL choice uses md, txt
and json; an empty choice opens nothing, and malformed stored text fails closed. The saved file must keep the requested
file's final extension. The helm also reads the list for each stat hint, and re-reads it at open time because the choice
or file can change after hover. The opener sees only that new local save. JSON always reports the saved path after
successful publication, with `opened` and, on refusal or dispatch failure, `open_error`; opening cannot turn a
successful save into a failed download. Stat includes `can_open` only with the native opener, from that same list. This
is an authenticated desktop-helm capability: any signed-in device of that helm can request it, just as it can request a
native save, rather than an extra origin gate. The ordinary server helm supplies no capability. No quarantine flag is
added.

The hover splits the disclosed path for presentation into a file-type Nerd Font icon and name, then host:directory,
without shortening either. All remote fragments still use textContent, peer isolation and visible format-character
escapes. Decimal size (1000 bytes per KB) and exact byte thresholds set the left stripe and size color; refusals
override size with red. Native mode alone advertises the platform modifier for `can_open` files. Printed paths and OSC
file links forward their mouse event to the same transfer path; web modifiers always request a plain browser download.

`terminal-files.js` supplies conservative token recognition and the terminal-owned file links (`FileLinks`), which the
grid hosts directly (terminal.js's `createGridLinks`), after OSC 8 and printed web links in the order a line's links are
offered. Link-line cells map UTF-16 token offsets to wrapped coordinates, preserving wide/combined glyphs. Logical lines
over 8192 characters or 128 rows are not linkified. Whitespace, quotes and brackets delimit ordinary paths; filenames
with spaces require an OSC file link. A deliberate exception reconstructs one isolated pair of rows broken by a program
rather than the terminal: the head's last token has no whitespace, quote or bracket after it apart from space padding,
the tail's first token has only space indentation before it, and that token is longer in terminal columns than the
head's unused columns minus a four-column right-margin allowance. The existing tokenizer must recognize a path at the
head's final token and recognize the joined token; a plain word before an absolute path is not a cut. Both checks read
the full line so URI chunk exclusions and diagnostic suffixes still apply. Both halves share its whole target and mapped
range, with padding and indentation left out of the target. Consecutive possible hard cuts are left unchanged rather
than reconstructing a three-row path. Terminal wrap handling and the 8192-character bound remain in force. Plain schemes
and protocol-relative tokens are excluded. File URIs decode once; their path belongs to the session host regardless of
URI authority. No shell directory tracking or hover cache is introduced.

Grid printed-path hovers track the neighboring rows that can create or remove a cut. When only those outside rows
change, the grid synchronously re-recognizes the path and range: an unchanged match keeps its hover and host check,
without another asynchronous container-admission request. A changed match takes the ordinary leave/requery path.
Covered-row changes always take that path too, preserving the existing display-text and provider-precedence checks.

The grid passes every OSC 8 target on, so the page's OSC link handling owns the explicit scheme boundary (`oscTarget`):
parsed http/https targets go unchanged to the existing opener, valid file targets go to file lookup/download, and every
other scheme does nothing. Fresh hover evidence owns an AbortController and click admission. Covers are declarative:
every surface that draws at or above the hover's layer and can overlap it carries a `data-link-cover` marker in its own
markup (today the approval region, the tunnel-error notice, the quick-launch pill, Farhelm's hover tooltip and the trash
flight animation), and file and disclosed web-link activation alike check the live bounds of every rendered marked
surface for any overlap with the hover. A hand-kept selector list was how the quick-launch pill went uncounted. Overlap
is the whole test, so a tooltip refuses a click only while it actually overlaps the hover, and a link's own hover never
shows one. Modal backdrops are not covers: they span the window and make every terminal inert, so no link under one can
be clicked. A covered hover refuses the click before transfer or opening. Its explanation sits above every cover except
the brief, unfilled trash flight until the pointer leaves the link, then returns to the ordinary failure layer.
app.css's z-index ladder states the rule next to the hover's layer, and a browser test checks the explanation's layer
against every marked surface it can render. The grid synchronously leaves and re-enters a link when its rows redraw
(grid-terminal.js's `refreshHover`), so leaving withdraws click/cursor admission immediately but defers abort and
tooltip removal to the next task. A re-hover in that interval keeps the evidence only for the same displayed text,
target path and snapshotted start/end cell range; a real departure or replacement invalidates it, including raced
replies. A kept ready cursor is restored in a microtask, once the hover callback has returned and the grid has settled
which link is current, and only while that replacement link is still hovered. Teardown never writes cursor decorations:
by then another link may be the hovered one, and the cursor is that link's to set. Plain links underline before lookup
but gain a pointer only when ready. Click reopens the disclosed canonical path independently. Browser bodies become a
bounded Blob only after successful EOF; an object URL is released after the save click. The desktop handoff secret
identifies native mode, which requests `save_to_downloads` and adds `open` only for its platform modifier-click when
that hover's stat reply offered opening; otherwise the modified click saves and explains that opening was not offered.
It never falls back to Blob. Tooltip and transfer outcomes use textContent, peer-value isolation and visible Unicode
format escapes. `FileLinks` is disposed with its grid, cancelling pending work; a grid held across a reconnect or kept
as a takeover snapshot keeps it.

Container environments use these ordinary file rules against their in-container supervisor. The earlier
checkout-confinement browser adapter and supervisor checks belong to the retired supervisor-outside model and are
removed with its terminal access.

## Supervisor internals

### Owned checkout admission and lifetime

Protocol 24 carries validated checkout destinations separately from existing cwd requests. Only full-authority callers
can preview, discover or create them; restricted session clients cannot supply helm-owned configuration. Shared proto
validation constructs the sole HTTPS GitHub URL and validates naming; the supervisor repeats validation at admission.
Existing request fingerprint encodings remain frozen, including the legacy tombstone digest decoders and matchers.
Delete now removes every reservation for the session in the same transaction as the session row, including `Pending`
claims; it neither settles them `Created` nor writes a digest. A retry after Delete allocates a fresh session identity.
Fresh fingerprints still include the original client identity and resolved configuration, while reconciliation looks up
a retained original request before consulting mutable settings.

Refusals with no surviving session row keep their fingerprint and `error_detail` as written for at most 24 hours of
retry protection, measured from the existing `created_at`. Expired refusals are pruned before each keyed create or
reconciliation lookup and on writable store open; there is no timer. An idle store can therefore retain expired rows
until its next writable open or keyed request, but an expired row cannot answer a retry. A retained error session is not
a refusal eligible for this age prune. Writable open also removes legacy `Created` reservations whose sessions are gone
and converts live permanent reservations to session lifetime. This replaces the old startup digest sweep without a
schema change: older supervisors already understand session-lifetime rows. The cleanup is one-way; a downgrade cannot
restore deleted-session answers for records it removed.

Reconciliation defaults to lookup: an unknown key allocates no session and records no refusal. Admission still expires
old refusal rows. When the helm must refuse an unknown request, it asks the supervisor to settle an identity-bound
refusal with the same 24-hour window under intent and directory admission. The supervisor reads back the stored row
before returning proof, so a concurrent winner remains authoritative. Known pending attempts still use their recorded
recovery state; reconciliation is therefore not generally read-only. Storage or authority failure, an identity mismatch
or an unverified installation cannot establish definite refusal. The identity-only refusal uses a versioned fresh
fingerprint variant and the ordinary serialized create-field cap; existing fingerprints stay unchanged.

Schema 18 stores the checkout registry, memberships, origin provenance and preparation snapshot alongside session and
intent state. Directory admission serializes allocation, membership insertion and last-reference teardown. Intent locks
precede directory admission. Discovery uses its own subprocess budget rather than directory admission. Fresh allocation
records its plan before mkdir, then records filesystem identity before exposing preparation. A post-allocation failure
atomically retains an error session and membership instead of losing the only deletion handle. Borrowers join every
applicable canonical managed ancestor; they never inherit the origin's preparation duty.

If identity capture never committed, an existing candidate path remains ambiguous: explicit Delete retires the plan
without adopting or moving that object and names the preserved path in its diagnostic.

The launch shim executes clone, the frozen optional hook, and the real agent invocation in the existing terminal.
Preparation has durable claim/progress/Ready state. The parent-directory durability barrier precedes launch publication;
once publication may have started, missing or ambiguous evidence refuses automatic repetition. Only the original pending
create may recover an unstarted preparation. Ready restart validates the recorded identity and skips clone and hook. The
agent's kind and launch metadata remain its actual values, rather than identifying the preparation wrapper.

Protocol 51 carries a frozen `claude_trust` bool beside `post_clone` in `ResolvedGithubCheckout` and
`CheckoutPreparation`; older stored specs default it to false. Its effect on supervisor behavior requires the ordinary
version-skew hello refusal. Helm schema 53 adds `checkout_config.claude_trust` (false by default) and nullable
`preferences.claude_trust_hint_dismissed`. The protected `GET /api/checkout-claude-trust` returns `enabled` and
`hint_dismissed`; PUT requires only an `enabled` bool. This setting changes no destination, so it does not bump the
checkout preview revision. Its write and the sparse dismissal patch publish ordinary invalidation hints. UI saves use
the configuration ordering funnel; dismissal uses the per-field preference queue and overlays a still-pending dismissal
on fresh reads.

Claude-specific file work lives in `agent_kind/claude/trust.rs`. After recording Ready, while still holding the
preparation flock, the launch process resolves the canonical clone root and Claude's config from its login-shell
environment. Claude 2.1.300 selects `$CLAUDE_CONFIG_DIR/.claude.json` or `~/.claude.json`, with a nonempty legacy
`<config-dir>/.config.json` taking precedence (`<config-dir>` defaults to `~/.claude`). An empty unwritable legacy file
also wins when the modern file is absent. Nonempty custom OAuth selects `.claude-custom-oauth.json`. An absent vendor
file is left absent.

The removal record stores the canonical file, selected lock alias, and canonical project key in the existing preparation
record, durably before the edit. Claude 2.1.300 explicitly uses `<selected-config-path>.lock` even for a symlink, so the
integration shares that mkdir lock while replacing the canonical target and preserving the symlink. Launch and
post-Delete edits retry contention for twelve seconds; startup cleanup tries the lock once without waiting. Neither
steals a lock. The edit reads at most 16 MiB from a regular file under the lock, requires object-shaped
configuration/projects/project, and changes only `hasTrustDialogAccepted`. A missing projects/project object is created
for the opt-in. Removal deletes only that key, preserving even empty objects. A same-directory temporary file receives
the original mode, is written and fsynced, renamed atomically, and followed by directory fsync. Publication refuses a
replaced target/lock or a lock held for four seconds, leaving margin before Claude's ten-second stale-lock threshold.
Temporary files and owned locks are cleaned on error. The advisory lock has the same accepted limit as Claude's own
writers: a writer stalled past Claude's stale-lock threshold can publish an old snapshot after another writer takes
over, losing an update in either direction. Claude's unlocked exit-save race and vendor format changes remain explicit
limits too; none of these checks proves compatibility forever.

Confirmed archive or missing-source retirement schedules removal off the Delete reply path. The worker retains directory
admission until it settles, preventing reuse while an old remover can still touch that path's key; terminal I/O and
listing use neither this guard nor the vendor lock. No-record checkouts retain ordinary metadata cleanup.
Refused/released archives retain trust. Successful removal clears the record before private metadata cleanup, while
failure is logged and retains evidence. Exclusive startup retries retired records only when the original path is absent
and no active registry row claims it. Recovery of an interrupted archive, at startup or on a Delete retry, removes trust
after successful pending-archive reconciliation even if a different folder now occupies the original path. This may
clear that replacement folder's trust record, causing Claude to ask about trust again. A busy vendor lock is logged and
leaves the record for a later startup, without a user-facing notice.

`ClaudeReader::trust_prompt` refines only the captured question, its answer labels and dialog footer, excluding an
ordinary input box. A run-scoped sample bool is false after reload/replacement or failed capture, is shared by renames,
and publishes a sessions-changed hint on transitions. `SessionInfo.claude_trust_prompt` defaults false and omits false;
listing and rename read the same sample cell. The UI visibility predicate also requires managed membership, the agent
tab, a successfully read disabled setting, and neither dismissal. The band stays mounted while hidden so a local × lasts
until the question actually leaves the screen; its flow row lets the existing terminal resize observer fit it.

Helm schema 57 adds independent `codex_trust`, `muse_trust` and `pi_trust` checkout settings, with nullable per-agent
hint dismissals in preferences. Each switch defaults off and leaves the path-preview revision unchanged. The protected
`GET`/`PUT /api/checkout-agent-trust/{harness}` exposes one switch and its dismissal; the original Claude route retains
its exact body and reply. Configuration writes use the existing save-order funnel, and dismissals use the sparse
per-field preference queue, including pending-write overlays on fresh reads.

The helm resolves these settings into an agent selection before compilation. The compiler remains pure so the YOLO guard
can still reproduce a launch exactly. Fresh checkout intent already proves managed ownership and freezes the whole
resolved launch in its existing create fingerprint. For an existing directory, enabled trust uses the pinned target
host's bounded `BrowseDirectory` reply in its default-off `membership_only` mode. Protocol 55 adds that mode and its
required `managed_checkout` reply boolean: the supervisor canonicalizes the accepted cwd and reads allocated registry
ancestors using the same component predicate as admission. Each matching ancestor must also pass the existing recorded
directory-identity comparison; the database lock is released before these filesystem observations. A missing, replaced
or unconfirmed-remount directory supplies no managed membership. The focused mode skips child enumeration and the
picker's printable-label restriction, preserving ordinary launch support for unreadable directory listings and
control-character paths. When trust is granted, the helm carries that canonical cwd through approval, dispatch and
history; a submitted symlink cannot redirect the trusted launch during an approval wait. Disabled and unmanaged
destinations keep their submitted spelling. Plain Restart and Replace retain their stored launch behavior. The session
list is not an ownership inventory, since nested memberships project only their innermost checkout. The read grants no
lease; ordinary create admission still validates the destination. A missing ownership field would silently change trust
behavior, so protocol skew refuses the peer until its ordinary supervisor update rather than treating an old reply as
unmanaged. The normal skew-aware update path remains unchanged.

`SessionInfo.workspace_trust_prompt` is separate from the retained Claude-only flag. The ticker reads Claude and Codex
through their integration readers and Muse through its integration reader or the exhaustive launch-harness seam,
independently of its activity reading. The flag defaults false in older cached snapshots, resets with the run or failed
capture, and shares the existing activity cell across renames. Codex recognition uses its captured 0.159.0 folder-access
menu (`waiting-trust`) and the reported Quit variant (`waiting-derived-trust-quit`, derived from that capture): the Back
choice requires `esc back`, and Quit requires `esc quit`. Muse uses captured 1.4.10-R6427.1 and 1.4.14 startup trust
menus. Both require the dialog's choices and current footer instead of matching isolated transcript words. Pi has no
trust recognizer; its setting still supplies its project-resource approval flag.

Each host caches repository objects at `<state_dir>/repo-cache/<owner>/<name>.git`, derived from the validated,
lowercased repository pair. The supervisor passes that absolute path in the preparation's `repo_cache_path`; it is not
request input and does not change the frozen fingerprints. Old launch specs deserialize with an empty path, then refuse
the clone stage rather than bypassing the cache. A Ready restart skips Git and needs no cache path.

The shim takes the working-copy preparation lock first, then a blocking flock on the permanent sibling `<name>.lock`.
Both descriptors are CLOEXEC. While holding the repository lock it runs these commands in the session terminal, with the
same launch markers, environment scrubbing and `GIT_TERMINAL_PROMPT=1` as the clone:

```text
git init --bare -q <cache>
git -c gc.auto=0 -c maintenance.auto=false --git-dir <cache> fetch --prune <https url> '+refs/heads/*:refs/heads/*' '+refs/tags/*:refs/tags/*'
git -c core.commitGraph=false -c gc.auto=0 -c maintenance.auto=false clone --reference <cache> --dissociate -- <https url> <cwd>
```

The fetch names the bare cache explicitly so Git's `safe.bareRepository=explicit` policy remains in force. Bare init is
idempotent, including after an interrupted initial fetch. Command-line refspecs keep the cache to branches and tags
without writing a remote configuration. Automatic gc and maintenance are disabled for the visible refresh and clone; the
existing background worker consolidates after the clone stage instead. A later launch can still wait while that worker
holds the cache lock. Plain `--reference`, rather than `--reference-if-able`, makes an unavailable cache fail the clone
stage; `--dissociate` copies the borrowed objects so the checkout has no alternates and keeps its GitHub origin. The
clone alone disables commit-graph reads to avoid Git before 2.55.0 losing a cached commit's tree during dissociation;
this command-local workaround can go once Git 2.55 is the minimum (upstream fix `3d8e4004c6`). The lock stays held
through dissociation, then its modification time records successful use. There is no timeout or fallback. Cache errors
name the path and use the existing `Failed{stage: clone}` state; `CloneStarted` covers the whole sequence, including
restart refusal after interruption.

Before cache Git work, under that same exclusive flock, Farhelm removes leftover `packed-refs.lock`, `HEAD.lock`,
`config.lock`, `shallow.lock` and lock files beneath real `refs/` directories. Git children do not inherit the flock and
end with their preparation job, so these locks have no live owner when repair runs; genuine Git failures still fail.

Supervisor startup sweeps `repo-cache/` on a blocking worker. It enumerates the permanent locks and tries each without
waiting, skipping a cache still in use by a shim that survived supervisor restart. Under the lock it removes leftover
`<name>.git.trash`, then renames a cache unused for more than 30 days to that trash path before removing it recursively.
An interrupted eviction therefore leaves only trash to finish at the next startup, never a partially removed live cache.
`remove_dir_all` does not follow directory symlinks. Lock files are never unlinked: a waiting shim still holds their
inode, and replacing it would allow two owners. One empty lock file per repository ever cached is the accepted leftover.
A sweep failure is logged and leaves unused caches for a later startup; it does not prevent sessions from starting.

Spare-checkout recovery uses a sibling `<name>.spare.json` record containing outstanding namespace locations and their
directory identities, plus the intended or retained ready spare's identity. A namespace is the owner directory under
`<checkout root>/.farhelm-spares/<owner>`; both `.farhelm-spares` and the archive directory are reserved in occupancy
and repository discovery. Old locations remain recorded until this repository's ready slot and strictly generated
private names are positively absent, including after a create selects another checkout root. The record contains no Git
facts, credentials, durable build status or per-temporary-tree journal.

The namespace's inode and creation-time evidence uses the working-copy registry's existing remount-tolerant identity
rule. Cleanup opens the canonical checkout root without following a final symlink, confines each reserved child lookup
below it, and verifies the opened namespace. Dedicated ownership of that namespace authorizes recovery of only
`.<name>:building-<uuid>`, `.<name>:claimed-<session uuid>-<generation>` and `.<name>:trash-<uuid>` directories,
including a builder interrupted before recording a per-tree identity. Recovery adopts such a directory at whatever
identity it now has, so a building directory the builder refused to publish because it was replaced is still removed.
Unknown names and other repositories remain untouched. The colon is excluded by repository-name validation, so private
names cannot match another repository's ready slot in the shared namespace. Cleanup removes a ready `<name>` slot only
at its own recorded identity, so cleanup by itself never adopts a replaced ready spare. A take, which never reads the
recovery record, renames whatever occupies the ready slot into its claim, and startup removes a claim left behind after
its launch's clone stage like any other. Both follow SPEC.md's acceptance that anything inside `.farhelm-spares` that
Farhelm did not put there is out of band. Cleanup renames the selected directory to a fresh no-replace trash name,
verifies it, then uses the archive's shared opened-handle confined recursive remover. It never recursively removes by
checkout pathname or crosses links or mounts. The verified namespace is fsynced before a record can forget a removed
ready identity or retire its location. A failed cleanup or namespace barrier retains the original identity and location,
including when removal completed; an absence-only retry still needs that namespace barrier before retiring the evidence.

Each selected spare removal emits one info completion event with its repository, original name, namespace path, outcome,
total and removal times in milliseconds, and counts of files and directories actually removed, including partial counts
after a failure.

Startup collects exact launch-generation, preparation-state and agent-terminal evidence before the blocking cache sweep.
A still-live clone claim is retained, including a live NotStarted preparation that can begin cloning after the sample;
failed or uncertain liveness observations retain possible claims rather than inferring abandonment. Spare cleanup runs
under the cache sweep's existing nonblocking repository lock, skips held locks, and evicts ready spares with the same
thirty-day cache observation. Missing ready content alone never permits forgetting a location with a surviving claim. A
failure is logged and retains the ownership record for later cleanup. Record reads are limited to 16 MiB and
owner-namespace scans to 100,000 iterator calls, including EOF proof. Exhausting either limit refuses cleanup and
retains ownership evidence; neither truncates or silently forgets old locations.

The preparation shim can consume a complete spare when its private `keep_spare` field is true. That field defaults to
false and is omitted when false. Protocol 62 carries the same frozen `keep_spare` field in `ResolvedGithubCheckout`:
ignoring true changes receiver behavior, so the hello refuses older protocols. Omitted false preserves old retained
fingerprint bytes. Helm schema 67 stores the global `checkout_config.keep_spare` choice with a constrained default of
true, for fresh and upgraded databases. The configuration snapshot resolves it with root/hook/trust in one read, and the
create constructor copies it into the retained payload. `GET`/`PUT /api/checkout-spare` exposes only an explicit
`enabled` boolean through the protected owned-mutation router; writes announce invalidation and leave the path-preview
revision unchanged. The Settings row uses the shared synchronous configuration-save fence, so a dismissed save finishes
before a reopened read can seed another edit. No preference, host override or CLI setting is involved. A take runs
inside the existing durable `CloneStarted` stage and preparation lock. It never reads the recovery record or acquires
the repository cache lock. Git below 2.17 or unrecognized version output keeps the ordinary clone path: 2.17 is the
floor for the take's `--prune-tags`, not a minimum for Farhelm. Local metadata queries are silent, limited to 4096
stdout bytes and one second, with stdin closed, stderr discarded and the existing bounded process-group runner owning
kill and reap. The ancestry check below shares that bound although it walks the newly fetched history, so on a very
large fetch it can time out and decline to an ordinary clone after the fetch. Actual fetch and merge inherit the
terminal and have no timeout.

The shim derives the strict claim's session and generation from its actual launch-status filename, checks the session
against the launch spec, and makes one no-replace rename from the ready slot to that claim. It opens both the reserved
namespace and allocated direct-child destination below the canonical checkout root without crossing links or mounts. The
allocation's current device/inode and empty-directory checks remain mandatory. Git's checked-out branch, configured
remote and full upstream tracking ref come from the claimed clone itself, including non-`origin` remotes. Gitfiles, bare
repositories, redirected worktree/common directories and remaining alternates are structural refusal, rather than
metadata the shim rewrites. Ordinary hooks and filters remain configured; a dirty spare is not rejected merely for being
dirty.

A usable claim prints `Farhelm: updating a checkout prepared earlier from GitHub.` and runs
`git -c gc.auto=0 -c maintenance.auto=false -c remote.<remote>.followRemoteHEAD=always fetch --prune --prune-tags <remote>`.
The remote HEAD key is unconditional on every take-capable Git. The saved tracking branch must survive pruning and match
the remote HEAD symref afterwards, and the spare's `HEAD` must be an ancestor of it (`git merge-base --is-ancestor`, any
other answer or a timeout refusing), so a take whose upstream branch was rewound or diverged is refused, rather than
keeping removed commits through a no-op fast-forward or failing after the move. Git 2.48+ refreshes that symref; older
Git intentionally keeps its clone-time default-branch observation. Structural refusal before moving content leaves the
claim for supervisor cleanup and uses ordinary clone into the still-empty allocation. A failed fetch instead fails the
clone stage, with no second clone or credential prompt. A successful take makes one fetch; a successful fetch followed
by structural refusal may subsequently use the explicitly required normal clone.

Before moving, the shim rechecks destination identity and emptiness and preflights all raw top-level entries for mount
or filesystem crossings. It moves `.git` first and then each remaining entry with fd-relative no-replace renames,
preserving the allocated folder itself. A failed first move permits clone only if the destination is still known empty;
every failure after any entry moved retains the partial checkout and fails preparation without rollback. The shim
removes only the emptied claim directory. It then runs
`git -c gc.auto=0 -c maintenance.auto=false merge --ff-only <full tracking ref>` in the final allocated folder, so
changed-file filters and `post-merge` see that path. The existing post-clone command, Ready publication and trust steps
follow unchanged. Interrupted or failed preparations remain non-restartable under the existing state rule.

Each physically accepted fresh managed-checkout launch registers one background cache/spare attempt with its frozen
root, shell and switch. A pending recovery that actually creates a terminal registers too; reconciliation of an already
existing terminal, accepted-key replay and Restart do not. Registration order governs only this supervisor lifetime;
there is no persisted scheduling history, automatic retry or startup rebuild of old Ready creates. One entry per
repository coalesces pending work: a later same-policy create replaces the pending clone gate/token. Once running,
same-policy triggers coalesce without cancellation or a trailing attempt. Off/root changes invalidate either phase.
Successful publication retires its entry under the final rename gate. A physical create arriving while old publication
fsync/cleanup finishes can therefore register the next owed attempt; old cleanup preserves a new same-policy entry and
its ready slot. This is replenishment after a new create, not a retry after failure. One serve-owned worker runs host
cache/spare attempts serially. The ticker only wakes it; clone, cache waits and record fsync are not on the ticker or
create admission path. Runtime cache admission opens the existing permanent lock and uses nonblocking flock probes with
stop/policy cancellation and the same ten-minute attempt deadline as login/Git. It never leaves a blocking flock waiter
holding supervisor ownership. Post-attempt cleanup tries that lock once; a busy cache retains/logs ownership evidence
for startup rather than adding another owed attempt.

The gate reads exact accepted origin/session/generation and the durable preparation state. NotStarted and CloneStarted
wait; missing, corrupt, stale or terminal pre-clone refusal consumes that attempt without poisoning a later create.
HookStarted, Ready, post-clone failure and Failed at clone authorize one attempt: the visible clone command has already
returned and released its cache lock. Off cleanup waits for the same clone evidence before taking the cache lock.
Runtime discard protects every claimed name and retains old locations containing claims; only startup performs exact
claim-liveness cleanup. It removes verified ready/building/trash objects before building at a new root, through the same
confined remover and namespace barrier.

The worker persists namespace identity before creating a strict private building folder. It starts its exact binary's
`internal build-spare-guard` in a new session with a retained stdin owner pipe and nice 10, before login initialization.
The guard verifies it leads that session/group, starts OS owner-EOF and ten-minute deadline watches, then runs the
frozen user's shell with `-l -i -c` and null stdin, execing `internal build-spare`. Session credentials, markers and
reporter authority are scrubbed before startup; an ambient `TMUX` and `TMUX_PANE` (a supervisor started inside the
user's tmux) remain through rc files, unlike a visible launch's, whose terminal host child never gets them, and are
removed from Git afterwards. The helper uses the same session-bin PATH projection, ordinary Git configuration,
credential helpers and URL rewrites. It forces `GIT_TERMINAL_PROMPT=0`, `/bin/false` Git/SSH askpass and
`SSH_ASKPASS_REQUIRE=never` after rc files. No Farhelm post-clone command, trust operation or agent runs in a spare;
ordinary Git hooks and checkout filters remain the user's Git policy.

The shared synchronous cache core takes an explicit cleanup policy. Visible launches use the commands above; background
spare builds retain `gc.autoDetach=false`, `maintenance.autoDetach=false` and `gc.autoPackLimit=10` on their cache
fetch, so Git consolidates accumulated packs sooner and finishes cleanup before releasing the cache lock. The clone
always disables automatic cleanup. A background attempt with spares off or an already-ready spare instead runs
`git -c gc.autoDetach=false -c maintenance.autoDetach=false -c gc.autoPackLimit=10 --git-dir <cache> gc --auto`, without
fetching or cloning. These modes are mutually exclusive for one coalesced attempt. Cache-only work never marks the cache
as used by a checkout or changes the eviction policy. Both modes acquire the cache flock inside the same helper, after
login startup. The supervisor's separate preparation/cleanup critical sections never hold it around the login shell. For
a clone, immediately after the helper's flock and before any Git it verifies the expected building identity without
recreating a missing target. Startup removal under that same lock therefore cannot be undone by a delayed clone helper
whose rc files finished later. A build starts only with an empty ready slot.

After successful clone or cache-only consolidation the helper publishes a fresh private 0600 bool receipt. Only a
successful direct shell exit permits the guard to promote that receipt by collision-refusing hard link to the final
completion name; rc exit zero before executing the helper cannot produce completion. The guard then SIGKILLs its own
retained group on every outcome, as it also does on owner EOF/read error or deadline. The parent retains the pipe writer
outside Tokio Child.stdin, whose wait would otherwise close it. Parent output is limited to one MiB per stream and the
whole run to ten minutes. Only an ordinary bounded Exited(SIGKILL) plus the fresh promoted true receipt authorizes
publication; timeout, overflow and I/O refusal remain failures. Direct child reaping precedes private
spec/receipt/result cleanup. Startup discards strict leftover private artifacts without adopting a job. Failures log
bounded diagnostics and leave ordinary visible clone available.

Before publication the worker durably records intended ready identity and prepares opened handles outside the short
policy gate. Only the final current-token check and identity-checked no-replace rename serialize with registration;
namespace fsync follows outside the gate. Off during record fsync rejects publication; off after rename schedules
discard. A slot occupied at completion is never overwritten. The live supervisor has one worker and alone publishes; the
guard requests termination of ordinary same-group descendants and the parent reaps its direct child. This does not
promise instantaneous global exclusion across supervisor SIGKILL, reaping every grandchild, or containment of user code
which explicitly changes session/process group. The cache-lock/expected-target check supplies the filesystem ordering.

Last-reference Delete journals the source identity and archive destination before one no-replace rename. Linux uses
`renameat2` through its syscall with `RENAME_NOREPLACE`; macOS uses `renameatx_np(RENAME_EXCL)`. Parent fsync barriers
precede journal retirement and atomic metadata settlement. Recovery accepts a matching already-moved destination but
does not adopt a foreign source object. No recursive-copy or delete fallback is permitted. A rename the filesystem
refuses outright (no support for no-replace rename, as on NFS, CIFS and some FUSE mounts; a cross-device, permission or
read-only refusal) moves nothing, so its journal is rolled back and Delete leaves the checkout in place with a notice;
any other rename error keeps the journal, because on a network filesystem the move may have happened even though its
reply was lost. Directory admission also orders ordinary same-path creates against that move, so a new reference either
commits before Delete or observes the directory as unavailable afterward.

Root identity is checked even before accepting an apparently missing source. Common archive entry points refuse
overlapping active registry paths, including during startup recovery. A refused or failed archive step does not fail
Delete (SPEC.md, Managed checkouts): the session and its checkout registry row are retired, the folder is left in place
(after a failed move, possibly at its journaled archive destination, which the notice then names), and the reply carries
a notice naming it. A Replace's reply carries its source Delete's notice as well, since a replacement in another folder
can release the source's last reference. After process teardown and committed final retirement, Delete removes the
private preparation lock and state files. This cleanup is best effort: a crash or unlink failure can leave private
evidence, but cannot authorize another directory move.

### Archived-checkout trash

The archive namespace is `.farhelm-trash`; the legacy `farhelm-archived-working-copies` remains reserved alongside it
and `.farhelm-spares`. Before archive creation/recovery or a trash read/delete can use a root, the supervisor renames
the whole legacy directory without replacement and fsyncs the verified opened root. Registry destinations are bare names
and require no migration. An absent legacy directory or an occupied new name is a no-op; other failures propagate
without creating an empty destination. A non-retired checkout overlapping the legacy path refuses migration and leaves
trash records retained for retry. Passive reads need no directory admission for the one-winner rename. Migration holds
the shared database lock through its overlap check and rename, preventing allocation from committing an overlapping
checkout between them; ordinary archive opens and sizing hold no database lock. The permanent delete guard also compares
session working directories with the legacy spelling after migration.

The trash is built for archive collections well below the 10,000-archive cleanup limit, with ordinary path lengths. A
host beyond that scale may be unlistable in the app; supporting such collections is not a goal.

Protocol 44 adds `ListCheckoutTrash`/`CheckoutTrashListed` and `DeleteCheckoutTrash`/`CheckoutTrashDeleted`. The helm
routes each HTTP request to one connected supervisor and checks the observed host incarnation before dispatch; it keeps
no archive listing cache. IDs select records, never paths supplied by the browser. The session Delete reply includes
`archived: true` only when its completed last-reference teardown moved a checkout; bare successes and notice-only
replies retain their existing JSON shapes.

`checkout_trash` operates only on retired working-copy rows with a recorded archive destination. It derives that path
under the recorded root, opens the root and archive without following their final symlinks, and applies the
allocation-time inode/birth-time rules to those opened objects. Parent and leaf traversal stays relative to those
handles. A passive read prunes only missing archives under a verified root; other failures remain recorded and return a
path and diagnostic on every read. A trash Delete drops unusable identity records without touching the folder, while
uncertain I/O failures keep the record for retry. Recursive deletion lives in this module, preserving `working_copies`'
no-recursive-removal rule. Recursive cleanup removes objects reachable from the verified archive at the time each
directory is read, without crossing symlinks or mounts. Linux uses `openat2` with `RESOLVE_NO_XDEV` (including
same-filesystem bind mounts); macOS uses no-follow `openat` and device checks. Linux without `openat2` can still list
identities, but sizes are unknown and permanent deletion refuses. An encountered mount remains untouched; removal may
already have deleted other confirmed archive contents, so failures name that partial outcome. POSIX final `rmdir` is
name-based and can remove only an empty directory; an opened subtree moved during cleanup remains the object being
traversed. Directory admission stays held through the live-session directory guard and the retirement rename, including
after caller cancellation. Recursive removal never takes that admission, so it cannot delay launches, restarts or
session Deletes on that host. Stopped session rows also protect their folders because they can restart there. The guard
compares raw and admission-time canonical session paths against the archive's recorded and current canonical path; it
does not resolve unrelated session cwds under admission.

Retirement journals `.rm:<original archive name>` on the existing retired working-copy row before a descriptor-relative
no-replace rename in the same trash directory. The colon cannot occur in generated archive names, and its four bytes
leave even a collision-suffixed archive component below 255 bytes. The moved name is reopened and identity-verified
before removal is authorized. A first-time Delete's known no-move rename refusal restores the ordinary destination;
uncertain outcomes retain the journal. Startup and Retry recovery preserve an already accepted journal even after a
definite rename refusal, so the next supervisor start still retries that cleanup. Listings include reserved destinations
as accepted cleanup, distinct from ordinary archives, and a stale passive missing-folder prune compares its observed
destination before forgetting a record. At startup, before accepting new session admissions, reserved records either
verify at their hidden name or retry the journaled rename from the original name with the retained-session guard. Both
branches apply that guard; uncertain recovery keeps the record and warns. The existing create-admission fence also
rejects canonical descendants of both journaled names under the recorded and currently resolved root, including a fresh
checkout's resolved root. Roots are resolved once per admission call, never cached across requests. Retained failed
removals remain fenced, so a later startup cannot reclaim newly admitted working files without holding admission during
the traversal itself.

Each selected retirement emits an info event with its checkout id, archive path, outcome, total and per-step times in
milliseconds, separating journal update and rename from checks and opening. Request-wide admission and read times repeat
on each checkout's event; an archive path unavailable because the shared reads failed is empty. A separate background
completion records actual recursive-removal time and counts of files and directories removed, including partial counts
on failure. Retirement is not logged as reclaimed space.

Passive lists share the two-slot browse worker pool, including a syscall that outlives the caller's reply wait, and
never hold directory admission. Retirement uses a separate blocking worker, so wedged browse slots cannot block session
lifecycle through an empty request. A single serve-owned remover drains newly accepted IDs and the startup recovery set
sequentially, outside directory admission and the browse pool. IDs rather than opened trees wait in its queue,
preserving the descriptor budget for large selections. Failed attempts warn and retain their records; the remover keeps
their short reason and UTC failure time in memory, including failures of startup's interrupted-rename recovery. The
trash listing exposes those rows even when their folder cannot be opened, with unknown size where necessary; a cleanup
failure is an entry state, not a listing issue that makes otherwise known totals partial. An explicit Retry reuses
DeleteCheckoutTrash, rechecks the existing journal and guards under directory admission, and queues one attempt; there
is no timer retry. Active and queued IDs coalesce so repeated requests cannot schedule a second attempt after the one
they observed fails. Successful removal forgets the record only after the traversal finishes. Enqueue happens inside
owned retirement work, so caller cancellation cannot lose its wake. An idle-worker pickup receipt bridges reply and
listing without waiting for recursion; a busy remover accepts more IDs immediately. The optional, default-false
`removing` listing field describes queued and active work, including transitions between folders, and clears when that
run drains. It never follows the mere existence of retained failures. Each accepted-cleanup entry separately reports
whether removal is in progress or stuck, with the latter's reason and time. A default-false `reports_retired` listing
field distinguishes a capable supervisor's empty list from an older peer that hides accepted cleanup. Missing additive
fields retain old decode behavior. An async list whose row snapshot predates a removal's completion must keep the
existing poll alive until a later list can observe the row gone. A short process-local marker spans the admission-owned
retirement closure through enqueue, so a concurrent listing cannot mistake its journal-to-queue handoff for a failed
attempt. Its lifetime is serialized by directory admission; passive reads inspect only remover state. A stable journal
that this run never queued reports unavailable instead of keeping the poll alive forever. Listing without sizes does not
walk checkout trees. Requested disk usage sums allocated 512-byte blocks from no-follow descriptor-relative metadata,
deduplicating hardlinks within each checkout. A mount boundary has unknown size rather than counting its external data.
Traversal holds at most 32 directory levels per operation, leaving process-wide descriptor headroom for the two reads
and one delete that can run together. Deeper trees have unknown sizes and report a retained partial-delete refusal.
Removal retries a nonempty directory through its same opened handle at most three times, accommodating entries skipped
by a directory iterator during unlinking without chasing a concurrent writer forever. Per-record bookkeeping failures
stay in the batch's issues instead of hiding earlier results. One two-second deadline and 100,000-entry budget cover all
sizes on a host; an incomplete measurement is unknown, never a partial checkout total. Archive time is parsed from the
recorded destination after its separately recorded original basename, accepting the archive move's optional 32-hex
collision suffix. Invalid historical dates have no time; directory modification times do not substitute for the rename's
timestamp.

The UI reads each connected host independently, without sizes for the sidebar count and with sizes for the open dialog.
Load, connection changes, its own affirmative archive Delete, a managed row disappearing between complete fleet
listings, dialog open and cleanup completion trigger reads. A host whose last successful same-incarnation listing says
`removing` is re-read every three seconds until a successful listing says otherwise. Only that host is polled, with
sizes while the dialog is open; an unavailable read retains its last removal report. Each host has one owner-scoped poll
task, whose scheduled ticks share that host's single-flight reader and add no demand while a read or retry is already
owed. Reads stay suspended during this page's delete requests. A same-connection count remains visible during
measurement, but an old read generation cannot overwrite this client's deletion result. Host-local request ordering also
prevents an older poll from replacing a newer measurement or completed removal, without invalidating unrelated hosts'
reads. A per-host in-memory Trash epoch advances on accepted nonempty Delete or Retry, on a session Delete or Replace
that archived a checkout, and on a recorded notice change, before the existing fleet wake. Hosts listings carry it; only
its change invalidates an idle host's Trash reader, so another client's cleanup starts the existing removal poll even
after the initiator closes. Ordinary fleet events and fallback ticks do not themselves read Trash. Changes through
another helm are noticed on reconnect, app load or dialog open. The page's existing feed/fallback owner requests shared
notice reads too, with an explicit initial read and the existing SurfaceReader retry/coalescing policy. No additional
timer or channel is mounted. Per-host read children are keyed by host and connection incarnation, retaining independent
demand and retry lifetimes. An explicit delivery counter and the host epoch preserve pending causes across reactive
batching; registry uncertainty retains the owner without dispatching RPCs, and a recovered registry supplies unattended
news. Each attempt samples current measurement mode and mutation generation. A discarded pre-mutation reply is not an
answer unless a replacement actually satisfies that demand. Host-list refresh failures are disclosed rather than treated
as proof of current reachability. Unknown host counts stay unknown; the global cleanup requires successful, settled
listings from every reachable host, and refuses a host above the protocol's 10,000-ID batch limit rather than silently
omitting it.

Confirmations freeze deduplicated IDs and observed host incarnations. Diagnostic IDs are selectable for explicit
forgetting, while every diagnostic remains visible, including several for one ID. Before dispatch the UI rechecks
connection identity and checks the existing shared operation guard, without claiming it. Trash's own pending-host state
rejects overlapping cleanup requests synchronously. An immediate single-entry delete freezes only that checkout's ID and
its host incarnation at click time, requires the full host listing to be eligible, and uses the same dispatch and
completion path without a confirmation. Each host receives one frozen batch, with independent host tasks. Confirmed bulk
submission closes the modal before starting those requests; the persistent `TrashControl` owns their lifetime. An entry
Delete leaves it open, and closing it does not cancel the accepted request. Connection identity is rechecked before
dispatch. The fifteen-minute trash deadline remains for older supervisors that still remove inside the request; the
ordinary mutation deadline stays sixty seconds. A capable host's accepted IDs stay in the local listing, count and size
totals as removing, with any old failure cleared, until a later listing confirms absence. Older hosts keep optimistic
removal on the receipt. A capable receipt starts the existing completion poll immediately, including for an accepted
diagnostic-only ID, so a failed immediate refresh cannot lose its outcome. The row's Retry uses the same single-ID
selection and guards as Delete; queued or running rows offer no second action. Failure text and time participate in the
existing layout-settle fence because they can move other rows' destructive buttons. Only the persistent trash icon turns
upside down and bobs while requests are pending or a connected host's last listing reports removal, with stepped timing
within the looping-indicator cap, the universal inactive-window pause, and no animation under Reduce Motion. Its tooltip
names hosts freeing space and independently discloses retained cleanup issues. Opening the dialog does not clear issues.
Later successful removals clear only their IDs' old issues; new per-ID diagnostics replace older diagnostics for those
same IDs. Host-level uncertain outcomes retain their frozen unresolved IDs privately. A later reply resolves only
selected IDs it confirms removed or answers with a per-ID diagnostic; unrelated successes leave uncertainty intact. A
newer uncertain result replaces only its overlap with an earlier uncertain selection, preserving unrelated IDs and
ID-specific refusals. A healthy host registry prunes notices only for hosts it omits, including while deletes are
pending; disconnected hosts and uncertain registry reads retain them. A pending request remembers an observed removal
until it finishes, so a late reply cannot recreate those notices even if a subsequent registry refresh fails. That fence
ends with the request. A lost reply warns that some folders may already be deleted; other hosts continue, and the UI
refetches after all requests settle. Close and Escape stay available. The archive cue stores row geometry on the
persistent trash button, consumes it on the local Delete reply, and uses finite Web Animations with reduced-motion
refusal. No global geometry cache or new asset file is introduced.

The helm keeps shared non-session condition notices in one host-cascading table. Each notice has a non-reused integer
ID, its host/kind/checkout key, first raise time, read and resolved flags, and a tagged content snapshot. The initial
kind is stuck trash. A repeated failure updates its reason and time without changing its ID, raise time or read state;
unknown disk usage preserves the last known size. A capable successful listing resolves only IDs absent from both its
checkouts and diagnostic IDs, and a host-wide diagnostic prevents resolution. Queued Retry, an old host's listing,
unreachability and a cleanup receipt establish no resolution. Resolution persists read as well as resolved, so reopening
retained history never re-alerts it. Host removal cascades its notices.

`GET /api/notices` returns newest-first history; `POST /api/notices/marks` requires an explicit action and exact IDs,
marking those read or deleting only their resolved rows. A per-host trash gate serializes forwarded reads and Deletes
across clients without holding session/cache or provisioning locks during the RPC. Before recording, the existing brief
registry write fence revalidates the entry, connection incarnation and installation identity; the SQLite transaction
checks the stored binding too. Started blocking work owns both gates and its changed-only fleet invalidation through
commit even if the HTTP reader is cancelled. Successful nonempty cleanup receipts also invalidate the fleet, without
resolving notices. An observation-save failure preserves the host's successful listing and returns a separate optional
`notice_error`; it never fabricates a selectable trash issue or hides actual archives. No extra observer, timer or
channel records these notices.

### Runtime state

- State in SQLite (rusqlite) at `~/.local/state/farhelm/supervisor.db`: sessions and their metadata (SPEC.md's
  supervisor-authoritative list), spawn idempotency keys, captured conversation identities, host identity, and the boot
  id last seen. Comparing the stored boot id against the current one (`/proc/sys/kernel/random/boot_id`;
  `kern.bootsessionuuid` on macOS — a per-boot UUID, chosen over `kern.boottime` because the kernel rewrites boottime on
  clock steps and a boot id must never change mid-boot) is how "interrupted" is classified per SPEC.md. Before schema
  work, an open holding the state-directory claim attempts to convert the file to WAL. A failed conversion, or a result
  other than `wal`, warns and continues in the current mode; the next owning open tries again. Every ordinary read-write
  connection reads the actual mode and sets `synchronous=NORMAL` only for WAL, otherwise retaining FULL. WAL/NORMAL
  preserves database consistency and committed state across process crashes, but an OS crash or power loss can lose the
  last commits. Host-identity creation temporarily uses FULL and commits before the identity can be reported to a helm;
  losing that source identity after a helm recorded it would leave an installation refusing to start. The connection
  returns to its previous sync setting before the identity is returned. If conversion left a rollback journal, this
  critical write uses EXTRA instead, which also synchronizes journal removal. On macOS, FULL and EXTRA make these writes
  durable only up to the drive's cache: SQLite does not request `F_FULLFSYNC`. The maintainer accepted this limit,
  judging the additional protection marginal and expecting most Macs running Farhelm to have batteries. Schema
  migrations are atomic database-only operations: a lost migration leaves a complete earlier schema to migrate again.
  Older builds' SQLite versions can still open a WAL database. The `-wal` journal and `-shm` index live beside each
  database; copying only the `.db` file while Farhelm is running can omit committed changes.
- Host identity: generated once at first run and committed with FULL sync before publication, stored in the db.
- The interrupted-session surface is a centered neutral card in the empty terminal area: it explains that a host restart
  paused the session and keeps the terminal absent until the user intentionally chooses Restart or confirms Replace.
  Restart is an unconfirmed normal action because no process remains; Replace starts as a normal trigger and keeps its
  destructive confirmation inline. Replace reuses the existing create-then-delete endpoint and, on success, hands the
  new session back to the page selection owner so the fresh conversation becomes visible immediately; refusal remains on
  the card with the endpoint's actionable wording. An open Replace confirmation keeps its trigger visibly pressed.
- Sessions launch through the user's shell as an interactive login shell inside the PTY —
  `$SHELL -l -i -c 'exec farhelm internal launch ...'` as the window's command, with the shim doing the final exec of
  the session's invocation (see exited-session semantics) — evaluated per launch. The `-i` is load-bearing, by different
  mechanisms per shell (audited): zsh sources `.zshrc` directly when interactive; bash login shells never source
  `.bashrc` themselves under any flags — only the profile chain — and `-i` matters because it puts `i` in `$-`, so the
  stock Debian/Ubuntu `.bashrc` interactivity guard doesn't bail out when the profile chains it. Either way the sourced
  file set matches an SSH-and-type session, which is the contract. When `$SHELL` is unset (user-manager services on
  systemd older than 255 don't set it), the supervisor falls back to the passwd database, then `/bin/sh`. Command and
  resume text, or raw wrapper text followed by quoted composed agent words, is then interpreted by non-login,
  non-interactive `bash -c` after the shim restores PATH, markers and the working directory. Bash inherits that
  environment; the command text is not reconstructed from the literal argument projection. Ordinary login/interactive rc
  files are not read again, and ordinary aliases and unexported functions are not inherited. Plain Bash still honors
  inherited `BASH_ENV` and exported Bash functions; these are user-configured behavior after the shim supplies its
  restored environment, just as a wrapper can change the environment it receives.
- The supervisor's shared agent-launch builder also writes the intended working directory into the launch spec. After
  checkout preparation, the shim enters it before exec and supplies matching `PWD`; a failed chdir prevents the agent
  from executing and writes the ordinary launch-failure sentinel naming the directory. Entry is separate from exec so an
  absent or non-executable agent is reported as a program failure, rather than a directory problem. Starting the
  terminal in the directory is not enough on its own, because login-shell startup may cd again. Older specs lacking the
  optional field retain their inherited directory. Terminal tabs run `internal tab-shell` inside their existing scope,
  through the app's forwarder in the Mac app's version layout (which claims the version it starts; see "Side-by-side
  versions inside Farhelm.app") and the supervisor's own program elsewhere; the helper enters the folder and removes the
  agent markers and any `TMUX` or `TMUX_PANE` before execing the login shell. A failed tab entry reaches the existing
  dead-before-open refusal with its last output. This keeps tab startup inside containment and preserves a tab shell's
  own startup-file cd, while agent startup restores the session folder after those files have run.
- Status heuristics: periodic sampling of each live agent terminal's visible screen (its emulator's screen text, in
  Farhelm's screen-text format, which the captured agent screens share, "The supervisor's terminal emulator"), read by
  one screen reader per agent kind (`agent_kind::screen_reader`). Sampling must never sit on the attach/input path —
  SPEC.md forbids status from gating interaction. The supervisor's own ticker takes the samples and stores one reading
  per successful capture; replies only map the stored reading, and classification sits BELOW the recorded-error and
  dead-terminal rules in the existing precedence, so a reading only chooses among the live statuses of a terminal its
  terminal host reports alive. The generic reader is observed output alone, counted in a session's OWN samples rather
  than in elapsed time: three consecutive samples showing an unchanged screen reads idle, anything else live reads
  running, and it never reads waiting (a blocked agent and a finished one are equally quiet). Counting samples rather
  than seconds is load-bearing — the sampler works through live terminals on a budgeted round robin, so a session's real
  sampling period grows with the fleet, and any wall-clock window would eventually report a continuously-working agent
  as idle because the HOST was busy. A new launch reads running before its first comparison. A reloaded live terminal
  instead reports provisional `unknown` until a changed screen, a reading from recognized content, or three quiet
  comparisons provide fresh status evidence (a recognized idle prompt therefore ends the gap on its first sample); a
  recognized waiting prompt is reported at once. Dead-terminal and recorded-error outcomes bypass that provisional
  state. The helm retains the prior status from its per-host cache for an `unknown` reply, if it has one, and replaces
  the rest of the row as usual. Its identity-less in-memory list follows the same rule within a connection. Disconnect
  clears those rows by the existing identity-less host rule, so there is no previous status to retain after its
  supervisor restarts.

  Claude Code and Codex have dedicated readers that answer from recognized content ("anchored" readings) rather than
  change counting, because both redraw parts of their screen while idle (Claude's `/clear` hint, Codex's recap block and
  rate-limit footer) and change counting reads every such redraw as work. The anchors are the vendors' own wording and
  layout, preferring text shown to the user as an instruction over decoration: every dialog that asks the user something
  ends in a key-hint footer ("Esc to cancel", "enter to submit answer", "enter continue"), Claude shows a spinner line
  directly above its ruled `❯` input box for the whole of a turn. Its verb can have one or several words, followed by an
  ellipsis and a parenthesized timer beginning with an ASCII digit. The exact prefix `running Stop hook · ` may precede
  that digit, as observed in Claude Code 2.1.310; the existing glyph, nonempty verb, composer and proximity requirements
  still apply. Compaction (`Compacting conversation… (1m 3s)`) counts as working too. Finished-turn prose without that
  ellipsis remains idle. Reply text of the same shape just above the input box (`- Ran the suite… (2 failures)`) also
  reads as working; that cosmetic false positive is accepted. Claude's explicit `Waiting for N background … to finish`
  announcement in that same position also means the turn is still working, while footer hints and a listed task without
  that announcement remain idle. Codex animates a Braille spinner in its terminal title for the whole of a turn (its
  on-screen `Working (…)` widget appears only during tool runs, never while prose streams), so its reader also asks for
  the title the program last set, which the emulator keeps; the other readers do not ask for it. A waiting prompt wins
  over a busy indicator. A recognized screen that carries no state (a model picker, a transcript view) reads "can't
  tell" and the previous reading stands. A screen with none of the anchors falls back to the generic reader for that
  sample, and after a few such samples in a row (so a half-drawn startup frame does not count) the supervisor logs that
  once per run, without screen content, as likely rule drift. The rules are tested against real captures under
  `crates/farhelm-supervisor/tests/fixtures/screens/`, which `scripts/capture-agent-screens.py` re-captures from the
  installed agents, each run in a real terminal host with the environment a supervisor gives an agent and the
  supervisor's emulator answering its queries (the hidden `farhelm internal agent-screen`,
  `docs/agent-screen-fixtures.md`); versions captured before November 2026 come from a tmux pane and stay, because
  agents in terminal hosts started before 0.36.0 still have the environment that draws them
  (`docs/agent-screen-fixtures.md`). The earlier Codex-only masking of its composer out of change comparison is gone:
  whenever it applied, the Codex reader now recognizes the composer and answers from content, so the comparison no
  longer decides anything there.

  Muse also has a dedicated reader, based on manually captured 1.4.14 screens. A ruled `❯` composer followed by its
  model/workspace footer anchors idle; an adjacent `Working (… · esc to interrupt)` or `Thinking (… · esc to interrupt)`
  line above it anchors working. The timer begins with an ASCII digit; the animated glyph, provider, workspace and
  permission label are not fixed. Its current workspace-trust menu anchors waiting and takes precedence over work. No
  in-conversation question or approval dialog has been verified. Unknown screens use generic change counting; neither
  hook events nor the terminal title supplies Muse status. The old trust-only fixtures remain hint evidence, while the
  new version directory also holds working and idle captures.
- Permission-footer evidence: the same sampling pass calls the exhaustive launch-harness reader in
  `agent_kind::screen_reader`, independently of the activity kind. Its optional bool latches
  `ActivitySample::screen_yolo` on the first definite YOLO reading of a run and hints connected helms only when that
  value changes. Later non-YOLO readings, unrecognized modes and covered footers cannot clear it. Rename shares this run
  cell; restart replaces it. After a supervisor restart it is false until the next definite sample. The shared
  `entry_info` builder publishes it as an additive bool omitted when false; helm cache merges take that incoming value
  as given, including a restart's false. The sidebar escalates the drawn launch glyph and puts the vendor wording ahead
  of its existing hover/accessibility description. No admission or inheritance path reads this field. Captured grids
  live separately under `tests/fixtures/status-lines/`, with vendor version and capture date, because these permission
  screens do not satisfy the activity-fixture contract.
- Last-activity timestamp: the same ticker that samples for status also DATES the work it sees, into a
  `last_activity_at` column on the session row and onto the wire. It drives the row's displayed age and the helm's
  seen/unseen comparison, seeded to the session's creation time so one that has never produced output has an honest age,
  and restored verbatim on supervisor restart. A sample dates activity when its reading is working (from recognized
  content on every such sample; from change counting only when the screen actually changed, since change counting's
  working also covers the quiet samples before it decays) and when waiting begins; a question that stays on screen does
  not refresh it, and idle never does. Like work starts below, a baseline sample (a run's first, or the first after a
  failed capture) never dates anything. Persisting it does not contradict the rule that liveness is never persisted: a
  status is a claim about NOW and rots the instant the process it describes moves on, while this is a claim about a past
  instant that the passage of time cannot falsify. The two must not be conflated in the other direction either —
  classification still reads sample COUNTS and never this clock, for the population-dependence reason above. The value
  advances only when the dated sample is at least a minute newer than what is already stored, and the reason is blast
  radius rather than resolution. Two costs, scaling differently: a durable `UPDATE` per session per crossing, which
  without the quantum would be a write per busy session every two seconds; and a fleet-wide UI wake, which is COALESCED
  — the helm detects a changed session by comparing whole serialized `SessionInfo`s, but bumps the invalidation feed at
  most once per host refresh that found anything different, however many sessions moved. So the wake is bounded per
  refresh while the writes are bounded per session, and without a quantum a single busy agent would re-render every
  connected client on every drain. No user distinguishes two sessions whose last output was twenty seconds apart; the
  same quantum applies to the moment waiting begins, so a question asked within a minute of the last working stamp keeps
  that older stamp. Writes are monotonic in SQL as well as in memory, so a backwards clock step cannot make displayed
  activity younger or undo an unseen observation; a lost write costs age precision after restart until another dated
  sample crosses the quantum, and nothing else.
- Work-start ordering: the ticker separately advances `last_work_started_at` when a sample's reading moves a session
  from idle or waiting to working, or from idle or working to waiting. Both sides must be readings of the same run's
  successful samples: the run's first sample, the first successful sample after a capture failure, and a "can't tell"
  screen only establish or keep a baseline. That rule is what stops a question that has been on screen for hours from
  jumping to the top and reading "just now" after every supervisor restart, session restart, or capture recovery.
  Continued work in the same burst, a redraw of the same question, and completion do not advance it. The badge and the
  ordering read the same stored reading, so they cannot drift apart. The key is milliseconds since the epoch, but
  allocation is supervisor-local monotonic state rather than a bare clock read: startup seeds it to the greatest
  effective key across every loaded row (ended rows included), and a start reserves `max(now_ms, previous + 1)` with
  saturation. This orders same-millisecond bursts and survives a backward clock step on one supervisor. Separate
  supervisors still have only their wall clocks and the helm's deterministic creation/id/host tie-breakers; this does
  not claim distributed causality across skewed hosts.

  New sessions start at their creation time in milliseconds. Rename and explicit restart share the same session key,
  while their new run gets fresh transition evidence, so neither operation itself promotes. A start writes immediately,
  outside the last-activity minute throttle. Failed writes keep the exact key and retry it on later visits; rename and
  restart transfer that accepted history into their fresh sampler state under the lifecycle claim, without inheriting
  old screen evidence. A later proven burst replaces an older pending key. The write holds the session lifecycle claim,
  checks that the sampled entry is still the published generation, and updates SQLite only where id and generation both
  match. No activity mutex is held across that write. Schema 17 adds the authoritative supervisor column and backfills
  schema-16 rows from positive `last_activity_at`, otherwise `created_at`, with bounded integer arithmetic so SQLite
  cannot promote overflow to REAL. Old helm cache JSON is not migrated: its serde default and creation fallback apply
  until an authoritative refresh arrives.
- Agent-kind integrations live in the supervisor as a small trait (`AgentIntegration`; `AgentKind` is the wire enum
  naming the kind itself) for conversation-identity capture; status reading is the separate `ScreenReader` above, so a
  kind can have either without the other. New identities come only from accepted reports. Shared bounded readers in
  `agent_kind/records.rs` verify exact reported files; they never discover a conversation. The in-memory state holds
  either no identity or one stored identity with its ownership version. Reload preserves historical identities
  regardless of source, without re-verifying old scan locators. Each refresh takes the per-session capture claim before
  reloading the durable row into its mirror; it performs no Codex transcript or Grok record-pair verification. The
  ticker runs this reconciliation, as do startup, reload and decision-time Restart, without a global pass lock or
  coalescing. The ticker's pass skips a session whose capture claim is busy admitting a report and catches it on a later
  tick; Restart waits for the claim. Ordinary list and rename replies read the state those passes leave. They perform
  one terminal-state read for immediate exits and tab changes, but no capture, launch-artifact reads, cleanup or outcome
  writes. Resume readiness, launch-error classification and notification resolution may lag by one nominal two-second
  tick; the terminal-exit wake commits newly exited agent terminals without sweeping already-stopped sessions.
  Unreadable launch evidence is logged and retried by the ticker and does not fail lists. Found but uncommitted launch
  errors stay in generation-local memory for replies. Successful negative launch-sentinel and preparation reads settle
  for the generation only after a durable terminal outcome and the agent's terminal seen exited, or proof that the
  launch's boot ended. Schema 30 records that proof for all retained launches in the boot-change transaction, without
  altering already-ended outcomes, and restores it on later same-boot supervisor reloads. Older rows default to unknown:
  a reboot an older supervisor already consumed cannot be reconstructed. New launches clear the proof; a definitively
  aborted restart restores it with the prior external run's facts under the advanced generation. A same-boot absence of
  the agent's terminal without such proof keeps retrying. Read errors, uncommitted failures and observers without
  recording authority never settle reads. Rename shares the latches; relaunch and supervisor reload read again. Error
  artifacts are cleaned once after successful accepted-create evidence preservation and removal, with failed cleanup
  retried. Report-backed identities still refresh under their capture claim because accepted reports and decision-time
  Restart can change the row after the agent stops. An injected launch holding no identity warns once after 65 seconds
  from the first input frame delivered to the agent's terminal that holds an Enter (`capture::submits_a_line`): a
  carriage return that is not preceded by ESC (Farhelm's own Shift+Enter sends `ESC CR` to insert a newline), in a frame
  the grid input path queued for the agent's terminal host. A bracketed paste is judged whole and never counts: its
  newlines reach the agent as carriage returns (the supervisor's grid input encoder converts them), inside the paste
  markers. The terminal's automatic replies to the agent TUI's own queries (device attributes, cursor position, colour
  answers, focus reports) never carry a carriage return, so an agent the user opened but has not typed into cannot trip
  it. An Enter while the latest screen reading before delivery is `Waiting` answers a recognized dialog and does not
  start the clock. An outdated waiting reading can defer the clock to the next Enter; no capture runs on the input path.
  With no reading yet, or no dedicated reader, the existing submitted-line rule applies. The spawn records whether its
  argv received the hook before the agent's terminal host starts, fenced by launch generation. Reload restores that
  flag; older rows default unhooked and stay unchecked. The anchor is an in-memory monotonic instant, reset with the
  diagnostic latch on every relaunch and supervisor restart, so a picked-up launch starts its clock at the next
  qualifying Enter after reload rather than recovering the time of an earlier Enter. A Resume carries its identity and
  therefore stays silent even if its new hook never reports. The warning changes no offer or admission rule.

  **Codex attribution and exact-record validation.** The hook records its own process ancestry when it makes a report,
  and the supervisor anchors that chain at the session's agent terminal's root process (see the shared framework below).
  For a Codex report, the innermost native executable whose basename is `codex` is the emitter. Any processes above it
  are launch ancestry, including resident Codex-named launchers, scripts, shells and package managers. The supported
  process chains are documented in [the Codex integration](website/src/content/docs/docs/agents/codex.md). The reporter
  must spell the installed hook invocation (`<farhelm> internal hook …`, matched syntactically so an upgraded supervisor
  still accepts older hooks), and every link between that reporter and the emitter must be a narrow shell trampoline
  directly invoking it (`sh -c '<farhelm> internal hook …'` — the shape vendor hook runners produce) as exactly one
  simple command: any unquoted control operator, redirection, substitution, or other executable shell syntax refuses
  even when the hook comes first, while metacharacters inside quoted paths stay literal; another session-hosting runtime
  or any unclassified intermediary (interactive shell, script, chained command, unreadable argv) refuses. Argv
  classifies honest trampolines only: a descendant forging the exact hook shape is outside the attribution model.
  Unavailable process evidence refuses the report without changing the current identity. This is attribution under
  inherited credentials, not a security boundary against the same Unix user. Renamed native executables are not
  recognized.

  The first admitted selecting report pins its emitter and records the Codex-image ancestors above it, each by PID and
  kernel start token. A nullable JSON `codex_report_pin` column on the session row holds this launch authority
  separately from the conversation locator. The historical SQL name and JSON shape are retained for the shared
  `ReportPin` used by Claude, Codex and Muse; a session has only one agent kind, so no migration or vendor tag is
  needed. Schema 31 defaults existing rows to NULL without backfill. Admission reads the pin under the capture claim and
  writes it through `Store::admit_pinned_conversation` in the same CAS as the conversation, fencing the complete prior
  binding and exact previous pin. The writer takes the replacement ownership version explicitly: Codex passes 1, while
  Claude and Muse pass 0; the previous version remains an independent CAS precondition. No in-memory pin mirror is
  needed. Relaunch clears it, and abort does not restore it. Later reports must come from the pinned emitter, preserving
  the original stored ancestry rather than replacing it with a later snapshot. A recorded Codex ancestor takes
  precedence: its verified record independently selects and corrects both pin and conversation, never inheriting the
  child's file or persistent thread. A correction records the once-per-launch Codex correction notification in the same
  transaction as adoption; notification-write failure rolls back the correction, and reload restores it even if
  publication was interrupted. It does not resolve on capture because it records the prior mistaken target. An
  unverified ancestor changes nothing, and its unready Stop is acknowledged without effect. A pin whose process is gone,
  reused or non-running releases for a fresh emitter only if the pin is absent from the new report's chain; descendants
  cannot inherit authority after its exit. Unavailable process evidence does not release it. Rows already running across
  an upgrade start unpinned: a nested child's first selecting report after upgrade may pin it, and the root's next
  report corrects and notifies. There is no upgrade suppression.

  Process evidence is necessary but not sufficient: threads may share a process. A versioned `codex:` locator binds the
  reported runtime session ID to an absolute transcript path and a separately verified persistent thread ID. The bounded
  no-follow regular-file reader requires the first complete record to be root `session_meta`, with source `cli` or
  `exec` rather than a subagent source. Runtime metadata must match the report; once bound, a different persistent
  thread cannot replace the file's identity. Resume substitutes the persistent thread ID. No directory scan or home
  inference participates, so a configured custom home works without becoming a second source of truth.

  An attributed `SessionStart` with a recognized source selects the conversation. A foreground `clear` may install a
  pending locator before the exact file is available, withdrawing the discarded conversation immediately. A source-less
  `Stop` from the pinned process can confirm only an ownership-proven, unready locator with no established thread, for
  the same runtime and compatible exact path; it may fill an absent path. Its ready or previously withdrawn bindings
  return before vendor I/O. A recorded ancestor's Stop instead verifies its own record as a selection and corrects only
  when resumable. A Stop acknowledged without effect writes no pin. Codex 0.162.0 was run for two actual turns around
  `/clear`: both Stop callbacks named existing root transcripts, clear changed both runtime and path, and the old header
  remained intact. An opt-in real-agent test pins callback-time availability; deterministic fixtures cover a clear whose
  first callback precedes the file.

  Report transactions share a capture-only per-session claim and read the current durable binding while holding it. This
  is separate from the lifecycle claim: a pre-publication hook must not wait for its own launcher. A repeated report
  preserves an established thread binding. Writes compare the generation and complete prior locator, including an
  initially absent capture. A report made before in-memory publication waits on disk and is applied on the first pass
  after publication, once the session's published entry holds the agent's terminal. Historical bare Codex IDs are
  retained but fail closed rather than being guessed into a new locator.

  Admission and pending confirmation verify the exact file; passes, list/info snapshots and replayed creates use the
  stored binding. Restart verifies once before any launch mutation. A missing file persists withdrawal through the
  exact-locator/generation CAS and notifies only when that write wins; inconsistent metadata withdraws without a
  notification. An OS read error refuses with a retry message without changing the binding. Grok uses the same verdict
  policy for its exact pair. The existing final relaunch comparison still fences a conversation report racing Restart.

  **Grok attribution, ordering, and exact-record validation.** Grok uses the same recorded and anchored ancestry,
  capture claim, complete-binding CAS, ownership provenance, and mirror helper as Codex. Its corridor requires exactly
  one native `grok` image with `--no-leader` in the option region, followed only by the hook-shaped reporter and the
  same narrow shell trampoline. A nested Grok, foreign session runtime, missing option, or unreadable argv refuses the
  report. This is the supported native corridor. The terminal's root process is accepted by position, so one resident
  wrapper keeps capture, unless it looks like a Grok Farhelm does not recognise (argv[0] basename `grok`, or an image
  basename `grok-*`): that refuses, so a recognised nested Grok cannot stand in for the session's own.

  The hook parser accepts the observed snake_case and camelCase spellings for session id, event name, and transcript
  path, and requires duplicate spellings to agree after event-name normalization. `SessionStart` alone selects a UUID
  and must carry source `new` or `load` plus an RFC3339 timestamp. `UserPromptSubmit` and `Stop` carry enrichment only;
  they may supply an exact path for the selected UUID but cannot replace it. Only `subagentType` maps into the shared
  raw child marker, so child-marked callbacks are refused at the common doorway before vendor I/O.

  A bounded `grok:` locator stores the UUID, optional absolute `updates.jsonl`, canonical arbitrary-precision event
  timestamp, and current readiness. Its transition function runs under the shared claim after an authoritative row
  reload. A different UUID must have a strictly newer timestamp. The same UUID may advance its timestamp but cannot
  lower it; an equal repeat preserves an established path and may fill an absent one. Enrichment must match the selected
  UUID and inherits its timestamp. This one locator transition supplies restart-stable ordering without a schema field,
  event log, or retired-id set.

  Readiness requires the bounded no-follow first record of the exact `updates.jsonl` to use the observed
  `_x.ai/session/update` method and carry the UUID at `params.sessionId`. Its sibling `summary.json` must be a complete,
  bounded UTF-8 JSON document carrying the same UUID at `info.id`; a valid prefix is not enough. Report admission
  verifies that pair under the shared claim and publishes only the matching generation. Resume rechecks immediately
  before launch and substitutes the verified UUID, never either path. No history scan or path derivation participates.
  Grok's manual hook configuration is the deliberate exception to per-launch injection: Farhelm invokes no editor and
  writes no vendor file.

  **Shared attribution framework and the five-step admission.** Attribution is shared mechanics in two halves, not Codex
  code. Collection runs in the hook, when it makes its report: from the hook itself upwards, at most 64 `Running` edges,
  the hook's own start token verified first, each edge capturing its image (an unreadable one ends collection there) and
  its argv (optional evidence: `/proc` exe plus bounded NUL cmdline on Linux, a bounded `KERN_PROCARGS2` argv reader on
  macOS, 64 KiB per process and 1 MiB per walk, with over-budget or truncated argv recorded as missing rather than
  prefix-matched), plus the working directory of Bun and Node processes, whose relative entry spelling OMP attribution
  resolves. Collection does not know the terminal, so it climbs until the ancestry ends, a process cannot be read, or a
  budget runs out, and notes why it stopped; every edge and image is then re-read, and the chain is cut below the first
  one that changed (an exec between observation and re-read). Anchoring runs in the supervisor, when it applies the
  report: the chain must contain the session's current terminal root process (its terminal host's child), by pid and
  start token pinned while that process ran, and by pid alone for a child that had already exited when its host was
  adopted (`procs::TerminalAnchor`), and is cut there; it must be one unbroken line of parents within the depth and argv
  budgets, re-checked because the chain is now file input. Every launch runs a new root process, so anchoring is also
  what ties a report to its launch, for every vendor. The per-kind step applies its own restrictive corridor to the
  anchored chain — only the reporter plus a narrow trampoline between runtime and reporter; any other session-hosting
  runtime or unclassified intermediary below the emitter refuses. Codex attributes its hook-shaped reporter through
  shell-`-c` trampolines to the innermost native `codex`, accepts arbitrary ancestry above it, and applies its durable
  process pin. Claude and Muse select their emitter by position through the same narrow hook trampolines, accepts
  arbitrary ancestry above it, and uses the shared pin with every ancestor eligible to correct it. Grok retains exactly
  one native image with the required `--no-leader` option. Admission runs five steps: cheap envelope/kind/generation
  gating with no vendor I/O (the report file's discriminator check re-applied against the fenced resolution, plus raw
  event/source/agent-identity validation before diagnostic sanitation); the unbounded capture claim (one shared
  session-keyed `capture_locks` registry for report admission and readiness refresh, never the lifecycle claim; slow
  waits are logged for diagnostics) and a reload comparing kind, generation, and the complete prior binding; the
  mutation-free runtime and vendor-root proofs over the recorded chain (evidence recorded when the report was made
  cannot change during verification, so there is no repeat walk); the atomic generation-plus-complete-binding CAS
  committing identity, locator, provenance, source, readiness reset, and, for pinned kinds, the replacement pin and any
  correction notification together, with the exact prior pin as an additional fence; and the mirror of only the
  committed result into the matching current-generation entry under the same claim. Rejection at any pre-write step
  changes nothing durable, in memory, pending, or offered. Refresh and report-only reconciliation passes take the same
  claim and reload before mirroring, carrying the row's version beside the identity, so memory-derived offers apply the
  same gate as row-derived ones without a second lookup — and a rejected report never triggers a readiness withdrawal
  through them. Restart verification and lifecycle resets retain their durable generation/binding fences; they do not
  all acquire this capture claim. The report claim has no time limit because every operation under it is local and no
  hook waits on it; a slow wait is logged for diagnostics.

  **Ownership provenance and the offer gate.** Migration 20 adds `capture_ownership_version`
  (`INTEGER NOT NULL DEFAULT 0`, identical in fresh DDL and `ALTER`, tables kept `STRICT`): 0 means not established
  under the ownership contract — every historical capture — and only the authoritative admission transaction ever writes
  1, for the kind whose proof ran. Exact Resume requires version 1 for such kinds on every surface (reload, list and
  replay views, direct restart requests, pre-spawn verification), with the stored row's version beside the identity at
  each one; unknown versions preserve their data but refuse Resume and promotion. The deliberate Codex exception keeps
  valid `codex:` v1 tokens admitted before the column existed resumable at 0 through the existing exact-record verifier
  — bare IDs stay excluded, nothing is backfilled, and file existence or a valid header can never upgrade a version.
  Relaunch always resumes (see "Restart only resumes"), so it preserves the capture and its provenance together; the
  restart claim compares the version alongside the identity, so a provenance change under an unchanged conversation
  still invalidates a stale claim. Protocol 28 introduced the required closed-enum report discriminator that the doorway
  gates on; protocol 29 adds Grok to that closed enum together with its agent-kind and launch-harness variants. Protocol
  65 adds Muse's agent kind and report discriminator; its launch harness already existed. Senders predating either
  required variant fail closed at decode and at the missing CLI flag alike. Migration 21 adds `omp_reporter_asset`
  (nullable `TEXT`, identical in fresh DDL and `ALTER`): the gated reporter asset's file name as installed by the
  session's current launch, written pre-spawn — after the launch spec publishes, before its terminal host can start
  anything — from the one place that decides injection and fenced on the launch generation, so no report of the
  generation can arrive ahead of its provenance. Migration 22 adds `omp_launch_program` the same way: the program that
  launch's argv started, retained beside the marker so admission classifies the current launch rather than the resume
  template (a future resume's command). OMP admission requires the marker to name the current binary's asset with the
  file's bytes re-verified, so changing the asset breaks reporting for every OMP session already running (see "What
  running sessions hold across versions"); pre-21 rows adopt `NULL` and fail closed. OMP takes no Codex-style exception:
  every older OMP row has Restart unavailable (`not_captured`) until its first proven report.

  **Interim ownership states.** The discriminator gate applies to every kind now: it is envelope, migrated together.
  Attribution proofs apply to Claude, Codex, Muse, Grok, and OMP. Goose and Pi retain their existing acceptance behind
  the discriminator gate, and new framework entry points default to deny rather than allow. Claude admission runs under
  the shared capture claim, reloading the complete binding and pin before attribution and committing them in one CAS.
  The report's anchored chain (`procs::positional_corridor`), skipping the hook's narrow `sh -c` trampolines, identifies
  the first remaining process as the emitter at any depth below the terminal's root process; that anchor itself ends
  that search. Every process above the emitter through the root process is retained as a correction ancestor. It is
  positional on purpose: no executable, command-line, or environment rule recognizes Claude. Claude's native binary is
  named after its version and npm installs run under `node`, so an image rule would have to track install layouts, and
  the injected `--settings` hook is a vendor detail that may change on its own; the closed attempt in PR #830 shows
  where following either leads. Claude's first admitted selecting report pins the emitter by pid and start token.
  Reports from that emitter preserve the original stored ancestors. A recorded ancestor takes precedence over liveness
  checks and corrects the pin and plain conversation ID, recording `ClaudePinCorrected` once per launch in the same
  transaction. Notification-write failure rolls the correction back; reload restores a committed correction even if
  publication was interrupted. Like `CodexPinCorrected`, it remains unresolved because it records the earlier mistaken
  target. A gone, reused, or non-running pin releases only when its identity is absent from the incoming report's
  complete chain; a descendant cannot inherit authority after the pinned process exits. Unavailable process evidence
  never releases it. Relaunch clears the shared pin, and abort does not restore it. Running sessions start unpinned
  after upgrade: if a parent's report was lost, a nested Claude carrying a reporting hook can report first and be
  followed until an ancestor reports at start, resume, clear, compact, or the end of a turn. That gap is accepted
  without upgrade machinery. Claude adds no transcript verification, stays at ownership version 0, and leaves its
  per-kind offer predicate false: changing it would withdraw existing Restart offers, and the pin needs no versioned
  offer gate. Codex, Grok, and OMP require version 1 through the single per-kind predicate every surface consults; Goose
  and Pi keep their existing offer behavior. There is no general report epoch. Grok's locator carries only its
  vendor-specific selection timestamp; no other kind inherits that ordering rule. OMP uses serial cancellation fences,
  not cross-reporter chronology. Pre-discriminator senders and outdated OMP reporter assets fail closed; neither is
  grandfathered.

  Claude's `Stop` is an ordinary selecting report carrying the current conversation ID, not a confirmation of a prior
  `SessionStart`. It can establish the first pin, select from the pinned process, or correct from a recorded ancestor,
  under the same capture claim and complete-binding/pin CAS. When the pin refuses a rival emitter's `Stop`, admission
  acknowledges it silently without changing the binding, pin, offer or notifications and without a refusal warning.
  Unavailable liveness evidence still never releases a pin. Claude Code 2.1.310 was observed reporting the current
  conversation on `Stop` before and after `/clear`; `/clear` itself emitted `SessionStart`, with no intervening `Stop`
  in that observation. A native background sub-agent completed through `SubagentStop`, while the surrounding `Stop`
  callbacks carried the parent's ID and no `agent_id`. Farhelm subscribes to `SessionStart` and `Stop`, not
  `SubagentStop`; reports carrying a sub-agent marker remain dropped. These observations do not establish behavior for
  other Claude versions or every permission mode.

  **Muse positional ownership.** Muse shares Claude's positional corridor and pinned plain-ID admission, including the
  capture claim, authoritative kind/generation reload, complete-binding-plus-pin CAS, preserved original ancestors,
  dead-pin release only outside the full incoming chain, and unknown-liveness refusal. Its versioned native executable
  can disappear when the vendor launcher updates, so recognizing an executable or install path would make capture depend
  on vendor layout. Arbitrary resident wrapper chains remain admissible. A recorded ancestor corrects pin and
  conversation together with `MusePinCorrected`, unresolved and posted once per launch with Muse wording. Relaunch
  clears the shared pin, even on abort; no Muse-specific store column or pin migration is needed. Muse retains ownership
  version 0 and a false versioned-proof predicate, like Claude. Its `Stop` is an ordinary selecting report; a rival
  `Stop` is acknowledged without mutation or refusal warning, even when pin liveness is unavailable.

  Muse Code 1.4.14 was checked with offline echo-provider and interactive runs. `SessionStart` fires at the first
  prompt, with the current UUID at `session_id` and source `startup` or `resume`; `Stop` carries the current UUID and no
  source. `/clear` and `/new` change the UUID immediately but defer the next `SessionStart` to prompt submission, so old
  capture can remain until that report. The hook runs directly below the stable Muse process in the observed runs.
  Native reminder children in that same process emitted `SubagentStart` and `SubagentStop`, with `subagent_id` and
  `child_session_id`; their UUIDs did not appear in the subscribed parent `SessionStart` or `Stop`. That evidence covers
  the observed reminders, not every native child or side chat. Muse's parser preserves `agent_id`, `subagent_id` and
  `child_session_id` evidence for common child refusal, including malformed-marker rejection. No same-process hazard
  requiring an exact-record check was established, so admission and Resume read neither Muse's session files nor its
  live session index. The callback UUID alone fills `resume <uuid>` under the pin contract.

  **Codex without the shared server.** Codex's `{farhelm_args}` always starts with `--no-daemon`, including when its
  hook is disabled or skipped. Its commands must inherit this Farhelm session's environment and stay under this
  session's process ownership. A shared server instead keeps the environment of the client that started it; in
  particular it can retain another session's `FARHELM_SESSION_ID` and `FARHELM_SESSION_TOKEN`. Its commands, hooks, MCP
  servers and helpers then belong to that server rather than to each attaching terminal client, breaking Farhelm's
  session attribution and stop boundary. These are consequences of sharing execution, not observations from a live
  daemon test. The source inspected was OpenAI Codex
  [`rust-v0.163.0`, commit `83086188`](https://github.com/openai/codex/tree/83086188ae0614bd882818ff1aa23f3194962012):
  `codex-rs/app-server-daemon/README.md` documents the inherited environment and lack of per-client environment
  isolation; `codex-rs/app-server-daemon/src/background_command.rs` preserves the starting environment (making
  process-scoped paths absolute), and `codex-rs/app-server-daemon/src/backend/pid_start.rs` spawns the server with
  `setsid()` on Unix. That detaches its Unix session and process group; it does not establish a separate Farhelm
  ownership boundary for each client.

  The hook flags already force embedded mode in that source, but only as a side effect:
  `codex-rs/tui/src/daemon_startup.rs` excludes both `--dangerously-bypass-hook-trust` and CLI configuration overrides
  outside a narrow allowlist, which excludes `features.hooks` and `hooks.*`. With hooks off, neither exclusion protects
  Farhelm; a future Codex could also widen the allowlist. `--no-daemon` is the explicit supported switch, declared in
  `codex-rs/tui/src/cli.rs`. The inspected version has no equivalent config key or environment variable:
  `features.daemon_auto_start=false` prevents starting a missing server but still attaches to an existing one
  (`codex-rs/tui/src/startup_orchestration.rs`). Auto-start defaults to true (`codex-rs/features/src/lib.rs`). The
  explicit flag disables the auto-start path that constructs the "Running without the shared background server" warning,
  so that warning disappears and the flag adds no line of its own (`codex-rs/tui/src/startup_orchestration.rs`,
  `codex-rs/tui/src/app/startup.rs`). The separate hook-trust warning remains when the hook's bypass is passed.

  Every spawn re-expands the stored `{farhelm_args}` ("Launch kinds: one resolved launch"), so an existing agent or
  declared-Codex command session gains the flag on its next start, restart or resume; an already-running process keeps
  its old command. The flag precedes the hook tail in `CodexIntegration::farhelm_args`, preserving that tail's hooked
  verdict and exact log decision. It is absent from `hook_argv` and from the frozen legacy injection: a legacy session
  can still use the shared server when hooks are off or its hook injection is skipped, including for a bare `--`.

  Farhelm never parses a custom command to make the flag fit. In the inspected Codex, `codex-rs/cli/src/main.rs` tests
  it before and after `resume` and `fork`, but a command that places `{farhelm_args}` after `exec` fails:
  `codex-rs/exec/src/cli.rs` and its flattened `codex-rs/utils/cli/src/shared_options.rs` declare no such flag (exec
  already uses an in-process server, `codex-rs/exec/src/lib.rs`). Commands using `--remote`, `codex agents`, or
  `codex queue` also refuse it (`codex-rs/cli/src/main.rs`, `codex-rs/tui/src/session_queue_commands.rs`, with rejection
  tests in `codex-rs/cli/tests/features.rs`). These are documented limitations, not shape checks added to Farhelm. The
  flag first shipped in stable Codex 0.156.0: `codex-rs/tui/src/cli.rs` has it at
  [`rust-v0.156.0`](https://github.com/openai/codex/blob/rust-v0.156.0/codex-rs/tui/src/cli.rs) and not at
  [`rust-v0.155.1`](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/tui/src/cli.rs). Older Codex fails to
  start; Farhelm always passes the flag without version probing, consistent with SPEC.md, "Supported user environments".

  **The per-launch identity hook.** Claude Code's `/clear`, Codex's `/new`, and Muse's `/clear` or `/new` can replace
  the conversation inside a live process. Farhelm needs the agent's explicit report so Resume does not return to the
  discarded conversation. All three vendors fire a `SessionStart` hook whose payload carries that id and accept a hook
  supplied on the command line for a single launch, so farhelm passes itself as that hook
  (`farhelm internal hook --vendor <adapter>`, which drops its report as a file for the supervisor to apply; see "Report
  files" below) and lets the agent state its own identity. The `--vendor` flag is the report envelope's discriminator,
  sourced from the installed entry point rather than inferred from the payload; the Goose helper supplies its own value
  internally so the persisted declaration keeps invoking the same command, while the Pi/OMP assets pass theirs on the
  spawned command line and keep their JSON `vendor` field purely as a consistency check. Claude takes `SessionStart` and
  `Stop` entries in `--settings <json>`, each with a 60-second timeout; Codex takes
  `--dangerously-bypass-hook-trust -c features.hooks=true -c hooks.SessionStart=… -c hooks.Stop=…`, after the
  independent `--no-daemon` argument described in "Codex without the shared server". Muse takes `-c hooks=<JSON>` with
  Claude-shaped command arrays for `SessionStart` and `Stop`, each with a 60-second timeout. Muse 1.4.14 is the verified
  minimum; Farhelm does not probe versions or add compatibility code for stale harnesses (SPEC.md, "Supported user
  environments"). No Muse plugin installation, approval or configuration write is needed. Per-launch is the whole point:
  nothing is written to `~/.claude` or to Codex's active configuration home (`$CODEX_HOME` when set, `~/.codex`
  otherwise), no trust state is left behind, and flags cannot outlive the process they were passed to — which is what
  keeps SPEC.md's no-agent-configuration rule intact rather than merely bent. The costs are accepted deliberately, and
  both are scoped to the launches that actually carry the injected flags rather than to Codex launches in general: on
  those, Codex prints a hook-trust warning line above its composer, and with trust bypassed any hook the user has in
  that same configuration home but has not trusted runs too. The same bypass covers a trusted project's own `.codex/`
  hooks: Codex loads a project's hooks only once the folder is trusted (Farhelm's managed-checkout Codex trust setting
  or the user's own answer to Codex's trust prompt), and on an injected launch they then run without Codex's per-hook
  review. That is accepted because trusting a workspace already hands its Codex configuration, MCP servers included, the
  ability to run commands, and because skipping injection there would drop conversation identity, and so resume, for
  trusted checkouts. It is accepted only until hook installation becomes an explicit step surfaced to the user, where
  the user is told what is being installed and accepts specific hooks; after that, launches no longer pass the
  per-launch bypass. Codex fires `SessionStart` at the first prompt rather than at process start, so a Codex session's
  identity arrives only once the user has typed something, where Claude's arrives at startup. Muse also reports at the
  first prompt. The flags go where the launch's `{farhelm_args}` stands (see "Launch kinds: one resolved launch");
  nothing reads the rest of the command. Only a legacy session, from before launch kinds, still gets the previous
  release's injection, appended after its argv, and for it these invocation shapes disqualify a launch, which is skipped
  with a logged reason rather than made to work: an argv that already carries `--settings` (Claude honors only the last
  one, so injecting ours would silently drop the user's), an argv already steering Codex's own hook configuration (a
  second bypass flag risks a rejected command line, and the `hooks.`/`features.hooks` tables are the user's once they
  touch them), an argv already configuring Muse's `hooks` or `hooks.*` through `-c` (those settings belong to the user),
  and — for any of these vendors — an argv containing a bare `--` (our flags would become prompt text).
  `FARHELM_AGENT_HOOKS` in the supervisor's environment — `all`, `none`, or a comma list of kinds — turns injection off
  wholesale or per kind, read once at supervisor start and carried as a seam value. Without an accepted report, a new
  session cannot be restarted; no nearby record can supply a substitute identity.
  `website/src/content/docs/docs/how-it-works/agent-hook-injection.md` is the user-facing account of the same mechanism.
  The injected command line outlives the binary that wrote it, so it is part of what newer binaries keep accepting (see
  "What running sessions hold across versions").

  Claude's, Codex's and Muse's Stop hooks carry no instructions announcement: only SessionStart adds that context.
  Sessions launched before Stop was injected retain their old argv until relaunch. For Codex, a pending clear in one of
  them cannot be confirmed; for Claude, ancestor correction waits for another SessionStart rather than the end of a
  turn. No polling or upgrade machinery compensates for those accepted gaps.

  **Report files.** The hook never talks to the supervisor and never waits for it. It reads the vendor's payload
  (bounded at 30 s, for a vendor that holds stdin open, under the 60 s outer timers Farhelm sets or documents; Pi and
  OMP keep their published 2 s child timers), records its process ancestry, writes the report atomically into
  `hook-reports/<session-id>/` under the state directory (the socket's parent), and exits 0. It requires the launch's
  complete credential environment, as before, but carries no token: the file sits in the supervisor's private state
  directory, and attribution, not a secret, is what stops a nested agent's report. Each session has fixed slots, each
  replaced by rename: `latest.json` for the single-report integrations, including Claude and Muse. Their SessionStart
  and Stop both select the current conversation under the pin, so the newest report suffices without a separate
  confirmation slot. Codex and Grok use `selection.json` (`SessionStart`) and `enrichment.json` (Codex Stop; Grok's
  later events), drained selection first. Enrichment is refused without its selection, and the separate slot keeps a
  long outage's enrichments from evicting it. The directory is therefore bounded at two files with no queue, cap, or
  eviction rule. Two hooks firing at once still race for a slot, and the later rename wins. A report that names a
  sub-agent is never written, since it would take the slot of a pending report from the session's own agent and then be
  refused; a nested runtime that carries no such marker (a second Grok started inside a Grok session, whose hooks are
  global) can still take that slot, within one pass while the supervisor runs or across an outage, and is then refused,
  which is an accepted gap.

  The supervisor applies waiting reports on recursive file-watch events and at the start of every reconciliation pass
  (`capture_now`: the 2 s ticker, startup, reload, Restart) and does nothing while it is not recording. It takes a slot
  by renaming it to a private name, so a hook refilling the slot meanwhile is not deleted with it, checks the report
  (the vendor naming the session's durable kind, the conversation's size bound, the sub-agent marker, each vendor's
  source vocabulary), anchors its chain at the current terminal's root process, and runs the five-step admission. An
  accepted or definitively refused report is deleted; one that could not be read from disk, or was refused for an
  `Internal` failure (a store or terminal host that could not be read), or was judged while a relaunch moved the session
  to a new generation, is put back unless the slot was refilled, in which case the newer report wins, and ends that
  session's pass. A report made under an earlier launch, or whose terminal no longer exists, fails the anchor and is
  discarded. Reports of a session that is not published yet wait for the pass after publication. Drains never overlap; a
  taken slot found at the start of a drain belongs to a supervisor that died mid-drain and is put back. A pass that
  finds another drain running skips draining rather than waiting behind a slow admission; Restart is the exception and
  waits for it, then drains again, because a report waiting on disk may name the conversation it is about to resume, and
  after the relaunch that report would be discarded as the old launch's. The watch task also waits for the drain lock,
  so an event is never lost to an already-running drain. Every settled report leaves a verdict line in the session's
  hook log, beside the hook's own `written` line: `acked`, or `refused` with the error kind and reason. An `acked` Stop
  need not change the conversation: Claude's or Muse's pin can silently turn a rival away, and Codex can acknowledge a
  Stop without effect. Delete removes the drop directory; a pass removes one whose session no longer exists, which a
  hook racing the delete can recreate. The supervisor creates the report root with mode 0700 before attaching notify's
  recommended recursive watcher (inotify on Linux, default FSEvents on macOS). An independent task drains the whole
  root; a single stored wake coalesces bursts and retains events during a drain. A 100 ms pause between drains bounds
  the feedback from retried reports linked back into place. Access events are ignored so the drain's own reads cannot
  keep waking it. The timer remains the backstop. Watch setup or backend failure logs once and disables the watch for
  that supervisor lifetime, without changing periodic pickup. The ticker handle owns the watcher's cooperative stop;
  in-flight admission completes before backend drop requests native shutdown (notify's inotify thread finishes
  asynchronously). A closed supervisor lifetime channel ends an idle watch when the last supervisor reference
  disappears. macOS backend batching can add latency; pickup on Linux is normally well below a second. A report made
  while no supervisor runs waits on disk for the next one; a supervisor older than the hook binary (mid-update) may miss
  reports briefly, which is accepted.

  Grok uses the same hook executable and report files but not this injection path. Its native TUI cannot take a
  per-launch hook overlay, so the user installs three matcher groups under `$GROK_HOME/hooks`: one each for
  `SessionStart`, `UserPromptSubmit`, and `Stop`, all invoking `farhelm internal hook --vendor grok` without
  `--announce`. The tracked launch supplies the credential in the inherited environment; the configuration contains no
  Farhelm token or session identity. Missing configuration leaves the Grok session usable but unable to gain a new exact
  resume target.

  **Goose and Pi reporters.** These integrations never scan vendor state. A fresh Goose launch registers one named stdio
  MCP server, `farhelm-reporter`; Goose persists that declaration in its conversation, so resumed launches add no second
  reporter and only supply current-launch enablement, executable, and instruction controls. Those controls, like Pi's
  and OMP's reporter executable, ride in the launch spec's environment (`LaunchSpec.env`), which the shim sets on the
  agent process alone; a legacy session still receives them as an `env NAME=value` prefix on its argv. The persisted
  command contains no credential or session identity and falls back to `farhelm` on `PATH` for a manual Goose resume; it
  is part of the surface newer binaries keep accepting (see "What running sessions hold across versions"). Its empty MCP
  interface reports `AGENT_SESSION_ID` when current Farhelm credentials enable it and carries the instruction pointer in
  the initialize result. Pi loads a versioned TypeScript artifact materialized with private permissions under Farhelm's
  state directory. The extension serializes `session_start` and `agent_end` reports, including the exact absolute
  session file only after Pi has persisted it. The database retains a bounded, versioned Pi locator containing both ID
  and optional file; list/status inspect only that token. Resume alone opens the exact file through the bounded
  no-follow regular-file reader and compares its first `type=session` record's ID. Failure compare-replaces that exact
  locator and generation with a same-ID fileless locator, so a concurrent newer report wins and the stale request gets
  the ordinary offer-changed conflict. Each capture pass reconciles these report-only kinds with their own durable row
  before serving an offer, without opening vendor files. This covers reports accepted before entry publication and
  restart-time withdrawals. The mirror updates only if it still holds the identity observed before the row read,
  protecting a different newer mirrored identity. An identity that changes away and back during the read may briefly
  leave a stale offer until the next pass; restart always checks the durable identity.

  **The OMP reporter and its locator.** OMP (`AgentKind::Omp`, wire `omp`) is a report-only integration with no
  record-root scanning, added beside the Goose/Pi pair rather than inside it. The locator is Pi's shape re-generalized:
  one `SessionLocator` type with a closed `LocatorVendor` enum, encoded and parsed per expected vendor, with Pi's `pi:`
  wire bytes, bounds, and deny-unknown-fields layout preserved exactly and `omp:` added beside it. Parsing is always
  dispatched by the vendor the stored kind expects, never by sniffing the prefix, because accepting a locator under the
  wrong vendor's prefix would let one integration's report become another's resume target — a data-integrity bug, not a
  convenience. The plain-id escape routes (restart offers, template substitution, report acceptance for id-based kinds)
  reject all reserved locator prefixes through one shared check that does not decode JSON first: a malformed locator
  must be refused as a locator, not fall through as an id. Pre-resume verification dispatches on the stored kind and
  gives OMP its own header parser — a bounded no-follow prefix read that skips at most one well-formed leading title
  slot (the fixed-width 256-byte record OMP 18.2.4 rewrites in place) and then requires a `type:"session"` record at
  `version: 3` carrying the reported id, refusing anything else. That parser is deliberately separate from Pi's
  first-record rule, which stays exactly as strict as it was: one vendor's tolerance must not become the other's
  loosening. The cost of the separation is stated in SPEC.md — a title-less OMP version-3 header is
  byte-indistinguishable from a Pi-shaped file, so the vendor boundary is enforced by per-kind locator and report
  acceptance, not by file bytes. An OMP agent launch's resume command is composed by the helm from the same choices as
  its start command, with `--resume <verified-file>` after them; OMP's selector stripping and its end-of-options refusal
  went with resume derivation in protocol 39. For a legacy session, which OMP launches get the extension is still
  decided by an OMP-specific interactive-shape classifier following OMP's own command and flag-consumption tables; Pi's
  classifier is untouched. The gated extension asset is materialized per vendor under Farhelm's state directory
  (`integrations/omp/farhelm-conversation-v2.ts`, sourced from `assets/omp-conversation-v1.ts` — the published name is
  versioned past the gateless `v1` bytes, published beside them never over them) with the same exact-bytes,
  private-file, no-follow rules Pi's had, loaded with `-e <materialized path>` and pointed at the reporter through
  `FARHELM_OMP_REPORTER_EXE` (scrubbed from preparation children like the Goose/Pi reporter variables; its only
  in-support consumer is the asset itself). The extension reports the current conversation id ONLY from the interactive
  context (`hasUI === true && mode === "tui"`, checked before identity, queueing, or any state): a child-shaped callback
  — native task, workpool, revived child, or throwing context — is a complete no-op that never touches parent state, and
  the queued closure re-checks a per-factory serial, advanced only on eligible parent events, as the cancellation fence
  before the stale-id/file reads and subprocess creation. With `session_file: null` when there is no file to name, and a
  null-file report withdraws the old target instead of retaining it — gating the whole report on file existence would
  leave the previous conversation's resume target standing after a fileless transition. Reports serialize so a slow
  report for an old conversation cannot win. OMP 18.2.4's `/new` persists eagerly, so a fresh-but-empty conversation can
  legitimately carry a resume offer immediately; the eventless-relocation and non-file-backend gaps SPEC.md states are
  accepted here rather than papered over. Protocol 22 introduced this kind; protocol 23 added `LaunchHarness::Omp`.
  Protocol 24 also carries the owned-checkout vocabulary described above.

  **The OMP ownership proof.** Admission is one explicit branch (`report_omp_conversation`), wired through the shared
  claim discipline, the atomic compare-and-swap over the complete prior binding, and the shared mirror with version 1 —
  flipped in the same change as the per-kind predicate, since flipping alone would deny every OMP report. The root leg
  is a composition: launch provenance (the session's durable launch record naming the current gated asset, with the
  file's bytes re-verified, so a reload is proven rather than trusted) establishes that the launch installed the gated
  reporter, and process attribution establishes that the reporter descends from that launched runtime — together they
  imply the report passed the asset's context gate. Attribution walks the shared mechanics to the current agent
  terminal's root process and applies OMP's restrictive corridor over the installation descriptor the durable launch
  argv selects — classified at spawn and retained with the generation's provenance, never re-derived from the resume
  template: `Oj` (the selected Bun running the selected bundle `dist/cli.js` or source-tree `src/cli.ts`, entry
  spellings resolved through symlinks because the installed `omp` command is one), `Ob` (the compiled target with TUI
  grammar), `L` (an exact `bun x`/`bunx` or expanded npm/npx argument pattern above the runtime, Bun-resulting only),
  `S` (a known transparent `sh -c 'exec <runtime>'` trampoline, or an exec'd-away shell that leaves no link). Nested or
  additional session-hosting runtimes of any kind, unclassified intermediaries, Node-executed OMP, unknown wrappers,
  ambiguous package scripts, and a Bun or Node root process above the reporting runtime whose arguments cannot be read
  (which would otherwise let a nested runtime below it stand as the emitter) all refuse, for every launch program. Under
  a `bun x` or npm launch, a readable root process keeps its positional exemption without an expected-launcher match,
  because npm rewrites its process arguments and that match would refuse real launchers. An installed `omp` launch has
  no launcher at its terminal's root, so there any Bun or Node root process that is not the reporting runtime refuses,
  readable or not. The live runtime argv is re-read through the same grammar injection uses, so a process that exec'd
  from a TUI launch into a utility or print shape refuses too. Admission checks the report's recorded ancestry once,
  anchored to the current agent terminal's root process. The source vocabulary is the asset's four tags
  (`session_start`, `session_switch` with its opaque upstream reason, `session_branch`, `agent_end`), allowlisted at the
  doorway and re-checked at admission; `agent_id` stays a rejection signal. The session-file header stays a pre-resume
  file↔id check, not ownership evidence, and a parent lineage field never rejects. Supported versions are 18.2.4 and
  18.2.6 with equal gate-semantic pins verified at the pinned sources (static claim, no runtime probe). Compiled and
  package-launcher forms have chain-level shape coverage; live lifecycle evidence covers the Bun-executed entry. The
  npm/npx patterns do not match live npm's rewritten process arguments. On Debian, Ubuntu and other Linux systems whose
  `sh` is dash, npm's resident non-exec `sh -c` link is an unclassified intermediary, so the agent runs but its
  conversation reports are refused and Restart stays unavailable. Bash can exec that shell away, leaving npm as the root
  process whose readable arguments need no launcher match; this path is untested live, including on macOS. Node
  execution and unknown wrapper shapes remain refused, as described in `website/src/content/docs/docs/agents/omp.md`. A
  direct `bun <entry>` launch is classified apart from `bun x` and, like an installed `omp` launch, has no launcher at
  its terminal's root, so a Bun or Node root process that is not the reporting runtime refuses there.

  **The instructions pointer.** The same hook carries a second job, added because it costs nothing extra: with
  `--announce` on its injected command line it prints one line on stdout after writing its report, telling the agent
  that `$farhelm <request>` means the `farhelm agent` CLI and that `farhelm agent instructions` explains it. Both
  vendors were checked before this was built, and both inject a `SessionStart` hook's plain-text stdout into the model's
  context. Claude Code (<https://code.claude.com/docs/en/hooks>): "For most events, Claude Code writes stdout to the
  debug log and doesn't show it in the transcript. The exceptions are `UserPromptSubmit`, `UserPromptExpansion`,
  `SessionStart`, and `PostModelSwitch`, where Claude Code adds plain-text stdout as context that Claude can see and act
  on." Codex (<https://learn.chatgpt.com/docs/hooks>), under `SessionStart`: "Plain text on `stdout` is added as extra
  developer context" — event-specific, since most other Codex hook events say plain stdout is ignored. Because Codex
  does inject it, there is NO Codex fallback: no file under farhelm's state directory and no `model_instructions_file`
  or `instructions` config override, both of which exist but replace the vendor's own base instructions rather than
  adding to them. Muse 1.4.14 also feeds SessionStart stdout into developer context, verified in the offline-provider
  run, so Muse uses that same `--announce` line without a delivery overlay. Its Stop command omits `--announce`.
  Successful and deliberately failing hook commands produced no hook output on the observed Muse terminal. Two shape
  constraints follow from Claude and Codex and are pinned by tests: stdout starting with `{` and ending with `}` is
  parsed as JSON by both (and a failed parse fails the hook run outright on Codex), and the line is one line, ASCII, and
  short, because every session pays for it whether or not anyone ever says `$farhelm`. The full instructions are printed
  only on demand by `farhelm agent instructions`, whose verb list is generated by walking the binary's own clap
  definition rather than transcribed — a verb that exists but goes undocumented is one no agent ever discovers, and
  nothing else would notice. `FARHELM_AGENT_INSTRUCTIONS` in the supervisor's environment — `on` (the default, also what
  unset or empty means) or `off` — controls the flag, read once at supervisor start and carried as a seam value beside
  `FARHELM_AGENT_HOOKS`. Read once for a sharper reason than symmetry: Codex fires `SessionStart` at the user's first
  prompt, arbitrarily long after the launch, so a live environment read would let a session launched under one setting
  announce under another. The two switches are separate because they fail in different directions — no hooks costs
  identity capture, no pointer costs an agent knowing the CLI exists — and because the pointer rides on the hook, every
  skip that suppresses injection suppresses the pointer with it.
- Per-session spawn credential: random token in the session's environment (`FARHELM_SESSION_ID`,
  `FARHELM_SESSION_TOKEN`, socket path), checked by the supervisor on the unix socket. These variables, like the rest of
  what a session holds, stay meaningful to newer binaries (see "What running sessions hold across versions").
- Process-tree ownership (SPEC.md's stop/reap promises): ending a terminal is not enough — its hangup reaches the
  terminal's foreground processes, and daemonized descendants escape it. The portable sweep combines the descendant tree
  of the terminal's root process (its terminal host's child) with environment-marker selection through the platform
  process API. Every marker selection requires the matching `FARHELM_SESSION_ID`. The agent reap shared by Stop and
  Restart selects the agent's `FARHELM_AGENT_ID`; Stop then closes the terminal tabs under its existing lifecycle claim,
  while Restart keeps them. Delete selects the whole session, including tabs; closing one tab selects its exact
  `FARHELM_TAB_ID`. Agent selection also retains the existing legacy case with neither kind marker. These are
  process-ownership hints, not authenticated credentials; across versions they follow "What running sessions hold across
  versions", and nothing older than the markers themselves is promised beyond the legacy case above. Launch boundaries
  scrub the opposite kind's marker so nested supervisors do not misclassify a new agent as an outer tab's process. On
  both platforms the marker is read from the memory where the kernel placed the environment at exec
  (`/proc/<pid>/environ` on Linux, `KERN_PROCARGS2` on macOS), which is live: a program that rewrites its process title
  can overwrite it, and SPEC.md accepts that such a detached process escapes the sweep.

  Stop keeps its existing supervisor-owned mutation and lifecycle claim through the agent reap and every tab close. Its
  tab close borrows that claim instead of acquiring another one; explicit and automatic closes still own their cleanup
  task so caller cancellation cannot strand a sweep. Stop includes dead-but-unreaped tabs, attempts the other tabs after
  a close failure, and returns the unresolved cleanup in `SessionStopped.notice`. The GUI shows that notice through its
  ordinary per-session action result.

  `StopSession.refuse_if_tabs_open` and `refuse_if_agent_running` are default-false fields omitted when false. Before
  recording intent or signalling anything, the tab guard requires a successful listing with no open tabs, and the agent
  guard requires authoritative evidence that the agent has ended, including for an entry without a terminal. Failed
  discovery or uncertain agent liveness refuses too. UI row, header and bulk Stop derive both guards from the same
  displayed snapshot: no tabs guards tabs, an ended agent guards the agent, independently, whether a question was
  answered or skipped. Agent-approved Stop is unrestricted. At the HTTP edge, an omitted tab guard defaults true, so an
  old browser's bodyless or empty-object Stop can still stop the agent but cannot newly close tabs it never warned
  about. An omitted agent guard retains legacy agent-stop authority; the new UI always sends both explicitly. The HTTP
  result is `SessionStopOutcome`, an optional notice with an empty object for complete cleanup. Protocol 70 gates the
  helm-to-supervisor behavior: although the fields have decode defaults and preserve the empty success shape, an older
  helm would send unrestricted Stop without asking about tabs. Treating the change as additive would let that old client
  close tabs it never received permission to close.

  Automatic tab cleanup goes ahead only while the tab's runtime says its shell exited, checked under its lifecycle claim
  before signalling: an old exit observation does not authorize ending a shell that is live now. The tab's host is then
  ended through its runtime or by the verified signal, which checks the host's own identity, so a host that went away
  cannot redirect cleanup to another. Explicit tab closes and failed-open cleanup name their reason. Each process-sweep
  signal logs the target and a capped list of successful syscall recipients. Scope signals log a capped best-effort
  direct-member snapshot, or mark it unavailable; membership can change before systemd delivers the signal, and nested
  cgroup members are not included in that diagnostic snapshot.

  The sweep sends SIGTERM, allows a short grace, quiesces with SIGSTOP, re-enumerates, sends SIGKILL, and polls for
  confirmed disappearance. PID/start-time revalidation narrows reuse races; it does not make the separate identity read
  and signal syscall atomic or guarantee globally unique start timestamps. `systemd-run --user --scope` cgroup scopes
  provide containment on Linux (M3): where a functional systemd user manager exists — probed by actually running a
  trivial transient scope and then showing, killing, and confirming the collection of it, not by `which`, and through
  absolute binary paths so a login shell's `$PATH` cannot substitute what the probe approved — each launch is wrapped in
  its own generation-named scope (audited on systemd 255: the wrapper execs in place, so the terminal's process tree,
  exit codes, and liveness checks see exactly the unwrapped shape), the per-launch SELECTION is recorded durably as a
  boolean while the unit name is re-derived from session id plus generation at every use (a stored name would let a
  tampered row aim a kill at another session's unit), and stop kills through the scope first — SIGTERM, the same grace
  the sweep gives, SIGKILL, then confirming the unit was actually collected, because `systemctl kill` returning only
  proves delivery. The manager is probed once and the answer cached, with two exceptions. A probe that runs out of time
  (15 seconds overall, 5 per query) is not an answer, since a busy manager is not an absent one: that launch runs
  without its own scope, and the first launch or teardown at least a minute later probes again. Teardown waits out that
  minute too (decided 2026-09-30): Stop, Restart, Delete and tab close on a session or tab whose scope is recorded (a
  scoped launch row, or a tab whose host's tag records a scope) refuse until it has passed, because a scope that cannot
  be checked is an unconfirmed cleanup (below), and the periodic reaping of dead tabs warns on each tick for the same
  reason. Probing again for every such teardown would cost up to the probe's 15 seconds each while the manager stays
  slow, which is what the minute exists to avoid; a timed-out probe is rare, and a retry after the minute probes again
  and succeeds once the manager answers. A definite negative stays cached, except that teardown with evidence of a scope
  gets one re-probe: a unit its durable row says was scoped, and a tab whose host's tag records that it was opened in a
  scope.

  Confirmed 2026-11-08: the shared reaper skips the machine-wide marker scan and PPID walk only when containment is
  established for every target, a usable manager handles at least one named scope, every named scope is confirmed
  collected, and no validated explicit root survives afterwards. It checks only those root identities, not the process
  table, before deciding. Confirmed 2026-11-10: a stock systemd-linked tmux the user runs in a session moves its pane
  jobs into sibling `tmux-spawn-*.scope` units. Those jobs may survive scope-only Stop, Restart, Replace, Delete or tab
  close, as intended when running them under tmux; the user's tmux server inside the scope still ends. The reaper does
  no tmux membership inspection.

  When the conditions for scope-only cleanup do not hold, the reaper runs the existing portable sweep unchanged. A named
  scope must be observed live before signalling and confirmed collected afterwards; an already-gone or initially
  unconfirmed scope keeps the sweep. Deleting a session whose agent or a tab already exited, a Stop or Restart after the
  agent exited therefore always sweep. Scope-name provenance and containment completeness are separate inputs: a guessed
  name that is absent does not prove that a target was scoped. An unscoped launch, an unscoped tab or unavailable
  selection keeps the sweep required even when optional scope names are added. Delete combines the agent's and every
  tab's selection, including what each host's tag records, so a scoped agent cannot hide an unscoped tab's cleanup
  requirement. It also keeps the sweep when discovery is incomplete: no agent terminal, or an agent host past generation
  zero, means historical unscoped tab jobs may remain. A systemd glob can supply scope handles in those cases, but
  cannot prove that no such jobs exist. Existing discovery errors still refuse Delete; a successful empty listing does
  not acquire that new meaning.

  A host at generation zero cannot have migrated from tmux. A later-generation host may belong to a session that had old
  tmux tabs before an earlier release's Restart moved it into a host, so Delete conservatively keeps the sweep; host
  records do not retain that history. This also costs the sweep for restarted host-only sessions in that state, avoiding
  a separate persisted history mechanism for the optimization.

  This is the maintainer's decision to trust containment and trade the always-run backstop for cost, with the
  incomplete-discovery exceptions above. The sweep remains the whole mechanism where no user manager exists. A broken
  manager does not silently pass either: for a scoped launch or tab, a scope that cannot be confirmed collected fails
  the operation even when the sweep found nothing (SPEC.md, Lifecycle operations), since the scope is what catches the
  processes the sweep cannot see. A silent Farhelm waypoint step sits between the login shell and wrapper, outside the
  scope, and execs onward without changing the eventual process tree. A wrapper that fails runs before the shim can
  write its exec-failure sentinel; the generation's waypoint and unconsumed spec in an exited agent terminal distinguish
  that **error** from a login shell that exited first.

  Terminal tabs also receive separate scopes, named from the session and tab IDs. A tab has no database row, so whether
  its open selected a scope is recorded in its host's tag, and closing the tab decides from that whether an
  unconfirmable scope fails the close. The session's own launch is no substitute: the agent and a later tab can see
  different verdicts about the user manager. A tab whose host's tag cannot be read (a startup placeholder) counts as
  unscoped, which keeps the portable sweep required. Delete collects those units both from the session's tab hosts and
  independently from the systemd manager using the session-specific tab-unit glob. The second source preserves a cleanup
  handle when no host supplies a tab's ID any more. A manager enumeration failure is distinct from having no usable
  manager; it can refuse Delete before the portable sweep.

  Containment starts at the agent launch, after login-shell initialization: the cgroup wrapper and the shim's
  environment markers deliberately exclude services started by shell startup files. A detached startup-file service may
  belong to the user's ambient login environment and need not be reaped on session teardown. A child that remains in the
  terminal's process tree can still be found by the ordinary descendant walk; this is an exclusion from guaranteed
  cleanup, not a promise that every startup-file child survives.

  Terminal tabs are the deliberate opposite. A tab's session and tab markers are in its host child's environment before
  its login shell starts, and its scope wraps that shell, so everything the shell's startup files start is inside the
  tab's containment and is reaped on close, auto-reap, and Delete, subject to the same residuals as the agent's
  processes (see below; for an unscoped tab, a detached tab-started process whose environment is unreadable escapes the
  sweep). The tab shell is itself what closing the tab promises to kill, and a tab has no shim that could run after
  initialization to apply containment later. The agent launch has normally already run the same startup files outside
  containment, so guarded startup logic usually reuses that instance. A tmux autostart guarded by `TMUX` being unset
  runs during the agent's launch too, since no `TMUX` is set at any point, and one that execs tmux keeps the agent from
  starting (SPEC.md, "Session view"). A shared service dies with a tab only when the tab's startup files are the first
  to start it.

  **What the cgroup does and does not promise.** It targets ACCIDENTAL daemonization — the dev server, MCP server, or
  build watcher that double-forks and execs away its environment marker, which is exactly the shape the sweep provably
  cannot find. It does not contain a descendant that runs `systemd-run --user --scope` on itself: that migrates into a
  sibling unit under the same user manager (reproduced, not theorized). Scope-only cleanup does not chase it even when
  its marker remains, nor a process that merely carries the session's marker without ever having entered its scope.
  Containing a deliberate migration needs a delegation boundary — a parent slice the supervisor owns, with the manager
  refusing migrations out of it — which v1 does not build and SPEC.md does not promise. Agent descendants run with the
  user's own privileges by design, so a descendant determined to outlive its session can always arrange to; the honest
  claim is that stop reaps what a normal program leaves behind. macOS has no /proc, so the three reads the sweep needs —
  the same-euid process table, one pid's parent/start-time/zombie state, and one pid's environment — go through a
  platform seam that answers them with `sysctl` there (`KERN_PROC_ALL`, `KERN_PROC_PID`, and `KERN_PROCARGS2`) and with
  /proc on Linux; the portable sweep's decisions above that seam are one shared implementation. The Mac marker source is
  deliberately narrowed to the environment region of `KERN_PROCARGS2` with argv discarded, so that neither platform can
  claim a process for marker text that merely appears on its command line. One Mac residual on top of the shared ones:
  macOS 26+ withholds that environment region for Apple platform binaries even from a same-uid parent (observed on real
  hardware, pinned by a macOS-only test), so a reparented descendant exec'd into `/bin/sh` or another platform binary
  escapes the marker scan there; the PPID closure still reaps it while it remains in the terminal's tree. The planned
  close is a session-id membership channel — a terminal's root process is a session leader and a SID survives fork,
  exec, and reparenting — deferred until the gap proves to matter in practice. See
  lore/2026-07-27-m2-process-tree-stop.md for the alternatives as they looked when this was decided. Another residual
  applies to unscoped launches and tabs, including every host without a usable user manager: a detached descendant whose
  environment its own user cannot read. `ssh-agent` marks itself non-dumpable, which makes `/proc/<pid>/environ`
  unreadable even to its owner (observed, not theorized), and it calls `setsid`; a setuid program's environment is
  unreadable the same way. The sweep treats an unreadable environment as unmarked, so once such a process has left the
  terminal's tree nothing claims it. Only cgroup containment closes this; the session-id channel would not, because
  `setsid` gives the process a new session id.

  **Process-number reuse inside a short window is accepted, for now.** Decided 2026-10-05. Code that signals or adopts a
  process by its number may lose the race in which that process exits and the kernel hands the number to an unrelated
  process within a short, bounded window: between reading a number and checking it, between reaping a child and
  signalling its group, while a watchdog fires just as its child finishes, or during a polling loop of at most a few
  seconds. That holds for the supervisor and the helm as much as for the repository's tests and maintainer scripts. The
  basis is that reuse inside such a window is vanishingly unlikely (Linux hands numbers out in order up to `pid_max`,
  millions on current systems, and macOS's range is about 100k), and that modern Unix software ordinarily relies on
  exactly that; the start-time revalidation above narrows these windows without closing them. What is not accepted is
  carrying a bare number across unbounded waits or asynchronous work, or storing it for later. Development tooling has a
  looser rule within a single run; see "Development tooling assumes a single-user machine" under Testing. The acceptance
  is deliberately interim. It stops review findings about such windows until two questions in TODO.md are settled:
  whether requiring a systemd user manager on Linux lets the number-based walk go there, and what process cleanup should
  promise on macOS.
- Attachments land in `~/.local/state/farhelm/attachments/<session-id>/`, deleted with the session. There is no size cap
  in v1: the bytes are the user's, on the user's own machine, and every hop streams them under a credit window, so a
  large file costs time rather than memory. A reported write or fsync failure before publication leaves nothing
  published and produces a visible error, never a truncated file at the published path.

  Final publication runs on a blocking thread: fsync the completed staging file, then hard-link it to the first free
  candidate name without clobbering another attachment. Cancellation, disconnection, or a disk-stage timeout may stop
  awaiting that thread without stopping publication. An abandoned publication may therefore leave a complete file with
  no path acknowledged to the client. Its retention is the same as any attachment: until session deletion, not startup
  reconciliation or Stop. A retry may create another copy; there is no rollback, deduplication, or background undo.
  Error responses must not claim nothing was stored when publication completion is unknown. This does not change
  prepublication staging cleanup, no-clobber semantics, prompt desktop Quit, or the healthy-local-filesystem assumption.
- The rest of the state directory: `supervisor.sock` (the unix socket that is the supervisor's only doorway — mode 0600,
  inside a 0700 directory, because reaching it means running commands as the user), `tmux.sock` and `tmux.conf` only
  where an earlier release's private tmux server left them, removed at startup once it is gone ("Ending the old private
  tmux server"), `hosts/` (0700, the terminal hosts' sockets; see "Terminal host"), `repo-cache/` (host-local repository
  objects and permanent locks; see "Owned checkout admission and lifetime"), `hook-log/<session>.log` (each session's
  hook diagnostics) and `hook-reports/<session>/` (its waiting report files; see "Report files"), and `launch/` holding
  one 0600 JSON spec per LAUNCH, named `<session>.<generation>.json`, plus a `.status` failure sentinel and an empty
  `.waypoint` proving a scoped command reached its wrapper step. The waypoint follows the same generation cleanup and
  superseded-launch sweep as the spec; orphan waypoints are removed too. A launch spec carries the agent's command line
  and the session credential, but nothing the session's own database row does not already hold, in the same private
  state directory, for the session's whole lifetime (the plaintext invocation, and the credential kept so a restart can
  inject the same one). Removing specs is therefore tidiness, not a credential boundary: the shim unlinks its spec as
  soon as it has read it, creation removes it if the session never starts, and the supervisor's startup sweep removes
  specs of sessions that no longer exist and of launches a later generation superseded. A current-generation spec whose
  launch never reached the shim (a login shell that exited in its rc files, a reboot, or a stop before launch) may stay
  until the session is deleted, whichever path observed the outcome (startup reconciliation, the runtime observers, or
  Stop); Delete removes it.
- Symlink TOCTOU hardening of the state directory is intentionally absent. The directory create, chmod, lock, socket,
  and sweep operations are plain path-based calls that follow symlinks; making them airtight means `O_NOFOLLOW` opens,
  dir-fd-relative operations, and ownership verification throughout. Exploiting the gap requires write access to a
  parent of the state directory — the user's own home — and an attacker with that already runs arbitrary code as the
  user, so the rewrite buys nothing against any attacker this tool could plausibly face. Decided won't-fix during the M1
  review. Revisit only if the state directory ever moves somewhere group- or world-writable. (The one place symlink
  safety is load-bearing anyway, launch-spec creation, uses `O_EXCL` and is safe.)

## Helm internals

### Checkout configuration and discovery

Schema 27 adds global checkout settings, per-host overrides and a monotonically increasing revision. Effective settings
are read in one SQLite snapshot. The configuration CLI opens only an existing current-schema database; it neither
creates missing state nor migrates a database owned by another running process. Clearing a setting restores inheritance,
whereas an empty hook override explicitly disables it. Configuration stays with the registry row on adoption; history
stays with the installation identity. Host settings exposes the root override only; the effective root continues to
resolve from the current global default at launch rather than being copied when a host is added.

`POST /api/github-checkout-preview` asks the target supervisor to expand/canonicalize the configured root and propose
the exact destination, without writing it. Responses include the configuration revision and installation/incarnation
claim, never the hook. A missing root resolves by canonicalizing its deepest existing directory ancestor and appending
only normal missing components; unresolved parent components, dangling links and non-directory ancestors refuse. This
preserves preview/create agreement through symlinked ancestors such as macOS's temporary directory. Discovery returns
complete-empty for a missing root, and preview has no filesystem occupancy but still reserves the archive name. Naming
scans refuse incomplete results after 100,000 entries. Create verifies this binding, creates the canonical root and its
parents using the ordinary umask, then re-resolves the configured spelling and requires the same canonical path before
capturing the root identity. It may leave an empty root after a later refusal. Atomic mkdir remains the checkout
collision authority. Known intent keys reconcile their original snapshot before current configuration or launch
compilation; replacement identities additionally bind the source session and preserve its veto. The composer reads its
checkout folder field from that accepted preview, or from the original binding while reconciling an ambiguous create. It
leaves the field read-only until an explicit existing-folder action changes the destination draft; the old editable
`cwd` seed is never presented as a fresh-checkout path. Preview failures leave the path empty with an error state rather
than displaying the old seed.

The serving future owns one serial three-second revision observer, initialized before readiness and HTTP serving.
Changed revisions publish existing fleet invalidations; failed reads preserve the last observation. Launch history
carries the authenticated revision. The composer's shared SurfaceReader retains refresh demand through read failures,
obeys build-skew withdrawal, and applies replies only to the current target. While the feed is unavailable, a
component-owned fallback uses the shared polling cadence and scheduled reader trigger; it cannot queue overlapping reads
or interrupt backoff. Its highest observed revision cannot roll back while another history request is pending or fails.

`POST /api/github-repositories` merges installation-scoped recents with supervisor discovery. The scanner owns two
subprocess permits independently of directory admission, considers at most 1024 immediate entries and returns at most
100 identities/64 KiB. It skips the archive directory, symlink children and candidates without their own `.git` entry.
Read-only `git config --local --no-includes --get remote.origin.url` has a 4096-byte stdout cap, a one-second child
deadline and a five-second enclosing budget. Cancellation kills and reaps before releasing permits. Inspection children
discard ambient Git configuration/location overrides, without changing the ordinary credential environment used by
clone. Only validated GitHub HTTPS/scp/SSH origins become canonical pairs; raw origin URLs never reach the UI.
Recent-first deduplication, sorted discovered entries and explicit incomplete status keep a failed scan from disabling
manual repository selection. Browser requests debounce for 150 ms and discard stale host/query generations.

`GET`/`PUT /api/checkout-root` is an ordinary protected route with a required nullable `root` and no host or command
fields. It calls the existing global setter/clearer; the revision watcher is the sole invalidation publisher for HTTP
and CLI writes. `GET`/`PUT /api/hosts/{id}/checkout-root` checks host existence (404 if absent) and reads only its
stored `root` and the global `default`, or sets/clears that host's root with the existing helpers. Neither route
contacts a host; path validation is shape-only.

The all-hosts post-clone command uses its own protected `GET`/`PUT /api/checkout-post-clone` route with a required
nullable `command` and no other fields. Null is unset, an empty string is explicitly disabled, and nonempty text is
stored verbatim; no GUI configuration route exposes a host's command override. The handler checks the shared 16 KiB cap
before calling the existing global setter/clearer, returning a visible 400 refusal without quoting the command or
changing the revision. Existing revision checks and frozen supervisor launch specs keep an accepted preparation and its
retries on the command accepted with it.

### Shared preferences and announcements

NOTE: These rules lived in the Dioxus GUI's section until that app was deleted in the GPUI migration. They describe the
helm, which still enforces them; the old client-side mechanics that surrounded them went with that app.

The helm keeps the GUI's shared preferences in a singleton row of `helm.db` (`preferences`; the `Preferences` type in
`farhelm_proto::ui_channel` lists its fields) behind `GET`/`PUT /api/preferences`, device-authenticated like every other
route. One answer is shared by every client: nothing is kept per client. A write is a sparse patch naming only the
fields it changes, merged per field by the helm: an absent field is untouched and an explicit `null` clears one, so two
writers changing different fields at nearly the same time cannot clobber each other. The remembered permission field and
the per-agent wrapper-prefix map are written by the helm after a successful user agent launch; no client PUTs them.
Remembered permissions describe accepted launches rather than one client's claim. The `remembered_workspace_trust`
column remains for upgrade compatibility but is neither read nor written. The session list's sort travels as the bare
word `?sort=` takes and is validated against that vocabulary at the write; the selection is the full entry-bound session
reference, whose entry UUID keeps a remembered selection from following a removed host's bare session ID to another
entry. A client reads an absent or unrecognized sort word as the default (`activity`), because the row outlives the
build that validated it.

Helm schema 46 adds the nullable `terminal_link_click` preference, the ownership of a click on a terminal web link:
`intercept` is the default, `both` keeps Farhelm's activation plus the program's mouse reporting, and `program` turns
Farhelm's web activation off. File downloads and agent URL forwarding do not consult it.

Helm schema 50 stores `template_shortcuts` as a JSON map from one ASCII digit to one valid template name, initially
empty. A preferences write replaces the whole map; concurrent writers can lose an edit to that same map, which is
accepted. Each template may hold at most one key. Names need not exist at write time, just as the quick-launch mark may
be stale. Both template-deletion paths clear matching bindings and the quick-launch mark inside the deletion
transaction; restoring the template does not restore either shortcut. Older helms refuse this schema.

Helm schema 51 adds `welcome_seen` to the shared preference row. Unset and false mean unseen; both fresh and upgraded
helms start unseen. The dismissal belongs to the helm so closing the welcome card on one client hides it on every
client's next load. A failed or timed-out read is never evidence of unseen. This migration prevents older helms from
opening the database.

Helm schema 56 adds nullable `preferences.name_session_on_quick_launch`. Unset and true enable quick-launch naming; only
false disables it, so upgraded helms have the same default as fresh ones. Null restores the enabled default and omission
preserves the stored choice. Older helms refuse this schema.

Helm schema 59 adds nullable `preferences.color_terminal_frames`. Unset and false leave terminal frames uncolored on
fresh and upgraded helms. A null restores the off default, and an omitted key preserves another client's choice. Older
helms refuse this schema.

Helm schema 60 adds `preferences.remembered_wrapper_prefixes` as an empty JSON map on upgrade. Each accepted user agent
launch replaces or clears its own agent type's entry in the same transaction that records the launch; a launch without a
prefix removes that entry. Agent-origin launches and command launches leave the map alone. An older helm refuses this
schema.

Helm schema 66 adds `preferences.visit_history`, a JSON list of at most 32 full session references, newest first without
duplicates. Each non-null `last_selected` write prepends its reference, removes its previous occurrence and caps the
list in the same database transaction as the sparse preference write. Null clears both selection and visits; omission
changes neither. Clients cannot PUT the history. Deleted references remain until later visits age them out. Migration
adds an empty list without inventing visits from a retained legacy selection; the next selection write establishes the
history-head/last-selection invariant. Older helms refuse the newer schema.

Helm schema 68 adds nullable `preferences.auto_update_remote_hosts`, constrained to booleans. Unset, null and false
authorize nothing; only true enables automatic remote-host updates. Migration leaves it unset and preserves other
preferences. The normal newer-schema guard refuses older helms, and the value survives reopening the store.

Helm schema 69 adds a durable announcement queue and the database's creation build. Fresh creation records the build
inside the schema transaction; migration leaves that row absent, so an older database does not acquire a false
fresh-install baseline. Older helms refuse the new schema. Each announcement stores its stable unique id, title,
plain-text body, optional display-ready detail and labelled link, insertion sequence, and nullable shown timestamp.
Enqueue is atomic and idempotent by id, including after dismissal: original content is never replaced. Pending rows sort
by insertion sequence; shown history sorts by shown time descending, then sequence descending for a batch.

Device-authenticated `GET /api/announcements` reads pending and history from one database snapshot.
`POST /api/announcements/shown` takes an explicit `ids` list and atomically timestamps only those pending rows; unknown
or already shown ids do nothing. A client can therefore dismiss an older window without draining newer unseen content,
and a failed write leaves the whole batch pending.

The compiled `release_announcements.json` table is the queue's first producer and ships empty. Before serving clients,
the helm enqueues entries newer than its creation build and no newer than its running build, using full semver
precedence, including prereleases and ignoring build metadata. An absent or `0.0.0-unreleased` creation build is older
than every entry; a running `0.0.0-unreleased` build enqueues none. Only the producer interprets versions or supplies
release-notes destinations: stable entries link to their version's notes, and prereleases omit the link because curated
notes need not exist. Queue storage, routes and consumers carry display-ready content and know nothing about releases.

Helm schema 85 adds seven nullable preference columns for the settings the old app kept per device: `appearance`
(`system`, `dark` or `light`), `sidebar_width` (the sidebar's resize width in whole logical pixels, 296 to 656, the
collapsed rail included and independent of rail expansion), the booleans `host_rail_open`, `sound_waiting`,
`sound_approval` and `sound_turn_finished`, and `terminal_text_size` (points, 9 to 28). The maintainer decided on
2026-11-13 that these become ordinary shared preferences with the new desktop app and start at their defaults: migration
leaves every column unset and copies nothing from the old app's per-device storage. Unset means System appearance, a
396px sidebar, a collapsed rail, the waiting and approval sounds on with turn finished off, and 14pt terminal text. The
route refuses an unknown appearance word and a width or size outside its range rather than clamping; only the booleans
carry a CHECK, and a stored word or number the route would refuse reads back as unset. Older helms refuse this schema.

### Composer history

`POST /api/browse-directory` accepts a default-false `membership_only` flag. When set, the helm uses the existing
supervisor membership check, returns the canonical cwd with empty listing fields, and skips folder-history refinement.
This provides a target-side existence check without enumerating children; ordinary Browse keeps its existing listing and
refinement behavior. Both modes check the supplied connection incarnation before dispatch and carry the supervisor's
refusal words back through the ordinary HTTP error path.

The launcher's automatic folder check debounces for 150 ms in a resource and has its own `BrowseAuthority` generation,
separate from explicit Browse. An admitted check runs in a component-owned task, so restarting the resource for an edit
does not cancel it and admit a second outstanding check. Completion releases the pending flag and lets the latest
destination run; an unchanged generation is checked once. Both completion and display use `browse_reply_is_current`,
including the raw path, installation and connection, and managed mode or a disconnected host withdraws the authority.
Only missing-folder and non-directory refusals warn; other failures remain inconclusive. Automatic checks share the
supervisor's two browse workers with manual Browse, so a hung mount can stall Browse on that host until it answers.

Composer history is one 100-record, per-host-installation unique-create window. Each accepted supervisor session id
enters it once, whether it was an agent launch or a command launch. Agent-launch suggestions are projections of the
surviving admission rows, so 100 later command launches evict an earlier agent-launch setup and remove its frequency
weight. Folder suggestions are separately bounded at 100 canonical destinations because they answer a different
question: folders remain useful after several sessions in the same place have left the create window.

The admission order is durable. A record with `creation_seq` sorts by its sequence until a sequence-less record enters
the partition. That accepted legacy record switches the whole retained partition and its cutoff to the protocol fallback
`(created_at, session_id)`, where an equal timestamp sorts the lexically smaller session id newer. The switch avoids a
pairwise "sequence when available" comparator that could be non-transitive when clocks and sequence order disagree. The
eviction cutoff stores the complete active key and creation time. While sequence order is active it also keeps a
separate timestamp/ID frontier for every eviction, because sequence order and wall-clock order can disagree; a later
legacy switch uses that monotonic fallback frontier rather than reinterpreting the timestamp on the latest sequence
cutoff. This frontier is deliberately conservative: after a clock-ahead row has been evicted, a newly observed legacy
create that sorts behind it is refused even if it might otherwise fit among the currently retained rows. That refusal is
the price of guaranteeing that an evicted identity cannot reappear when the partition changes order. A replay remains
rejected after its row has been evicted and after the helm reopens. No arrival counter or zero-valued synthetic sequence
is used.

Schema 24 drops schema-23 composer history during upgrade. Schema 23 did not retain the cutoff timestamp needed for a
truthful fallback switch, or whether a folder canonical path came from an accepted create rather than later browsing.
The history is convenience data, so an empty, coherent window is safer than inventing either fact. Adoption also purges
all history partitions for its host that do not match the new recorded identity in the same transaction as the identity
change. Each structured launch stores its accepted canonical destination beside the submitted display spelling. Folder
history remains a bounded suggestion projection: browse may refine legacy unknown paths but never changes an
accepted-create destination.

Schema 28 adds trusted repository provenance to the same bounded create-admission transaction. The helm records it from
the accepted request, not arbitrary supervisor display metadata. Command-launch fresh creates age the same window and
contribute repository recents; agent-launch history additionally projects the saved selection. Fresh requests retain
accepted cwd for diagnostics but skip the folder projection. Repository setups group by destination kind, canonical
repository and complete selection, independently of their previous ephemeral cwd. Adoption purges the old install's
repository suggestions with its other history; ordinary history rows retain their existing semantics.

`LaunchHarness::omitted_permission` is the one per-harness fact for new request defaults. The helm fills omissions
before validation and still refuses unsupported explicit choices; every harness that offers Guarded defaults to it,
including OMP. Draft reconciliation uses `request_permission`. Stored launches use `stored_permission` instead: missing
fields keep their historical no-flag meaning on Claude, Codex, Grok, Muse and Cursor, and their historical YOLO meaning
on Pi, OpenCode, OMP and Goose. Own-setting launches count as YOLO for admission but get a grey dashed shield rather
than a positive bypass claim. Protocol 48 adds the closed `Guarded` permission variant; older supervisors cannot decode
it, and older releases cannot read new stored guarded launches. `LaunchHarness::guarded_flags` supplies both the
compiler's argv and the launcher's help, so the two cannot describe different flags. A resume copies these before its
selector, including before Codex's subcommand. The helm's remembered permission and the browser's local mirror discard a
permission equal to the harness's omitted mode. This prevents default or forced YOLO from becoming another harness's
preselection while preserving explicit YOLO on harnesses with a non-YOLO default. Harness switching applies the same
boundary: passing through a default-YOLO harness loses any earlier explicit YOLO provenance, so switching back selects
the real default. Preferences written before this change retain their value until the next structured launch; the stored
word does not say which harness supplied it. No second default-mode table or preference provenance is kept.

The launcher observes completed agent-switch transactions for permission escalation, including cross-agent model picks
and search. A template's explicit permission is a deliberate choice, not an automatic fallback; compare after its full
replay rather than after each inner edit. Between harnesses, the comparison uses effective request permissions,
including passive memory seeds. Before the first harness, only an explicit permission choice counts; a passive memory
seed or displayed default cannot trigger the notice. Permissions are ordered as Chat/Approve, SmartApprove/Guarded, then
YOLO/Custom/own-setting. The existing reconciliation and remembered-choice rules are unchanged. One transient notice
retains the complete pre-switch draft and raw editor ownership, but never retry intent, connection evidence or cache
contents. Undo invalidates browse/preview work and retires ordinary create/replace authority. An ambiguous
managed-checkout attempt still reconciles through its retained key when the request binding matches; Undo does not
rewind or discard that attempt. Notice generations fence queued Undo, dismiss and expiry callbacks. Undo also checks the
live launcher opening generation, and a new opening or prefill clears the notice, so a mounted Clone/Replace reseed
cannot receive another source's saved draft. CSS supplies the slide, permission outline and a separate visually constant
eight-second lifetime animation; hover, focus within the banner and the existing inactive-window rule pause that
lifetime. Reduced motion keeps only the lifetime animation and a static outline. Removing the notice's controls returns
focus through the existing launcher helper.

Protocol 49 adds the closed `Custom` permission and `LaunchSelection::permission_flags`, an optional vector present
exactly with Custom. Older peers refuse the new protocol and older releases cannot read custom sessions or templates;
the missing downgrade path is accepted. Absent flags remain absent on serialization, so existing create fingerprints and
`agent_create_v1` digests do not change. `split_permission_flags` shares `shell_words::split` between the editors and
the helm's shape check: quotes/backslashes group words, a `#` starting a word begins a comment, and nothing expands. The
compiler refuses all five reserved supervisor placeholders as complete words, then inserts custom words where YOLO flags
would go, before workspace-trust flags and the resume suffix. No custom-flag meaning is interpreted. The shared shape
check requires a nonempty word vector with Custom and refuses flags with any other choice. Templates store the same
words, explicitly paired with Custom; another permission edit clears them. A harness switch drops both even when the
destination also offers Custom, because words belong to the source CLI. The remembered-permission read, write validation
and successful-create projection exclude Custom while recent setups retain the whole selection. Editable UI text is
separate from words so malformed quotes remain repairable without submitting a different command.

Schema 30's nullable `remembered_workspace_trust` column stays in the preference row, but no read or write uses it.
Permission memory and structured launch history still come from the selection the user submitted
(`CreateAcceptance::explicit_selection`), never from the supervisor's reply, per SPEC.md's rule that only explicit GUI
selections shape GUI defaults and suggestions. The launcher derives a host's default folder from the same recent-setups
reply it reads for the target: the `cwd` of the newest launch row without a GitHub repository. It seeds an untouched
folder once per opening or host change, from the first reply for that target (the offered snapshot), so a feed refresh
never reseeds. Launch is refused for an untouched folder until that reply arrives, and a hidden quick launch waits for
it; an unreadable reply falls back to `~`, and a hidden quick launch then shows New with an explanation instead of
launching. The default advances only when the parent-derived target agrees with the form's own host choice, and a
permission Undo restores the default as captured when its warning appeared, reconciled with the restored destination.
The launcher has no trust state, segment or search action. Recent setup grouping ignores historical trust, summaries
omit it, and activating a recent clears it before composing a new launch.

`LaunchSelection.workspace_trust` remains on the wire and in stored selections: supervisors persist selections, and
their strict decoding must still accept older fields. The helm overwrites the field from the agent trust section in
Settings when composing a launch. Compilation remains a function of the selection alone so the YOLO guard can reproduce
stored commands exactly. Codex true and false still compile to whole-argv config markers; the supervisor fills one at
the shared spawn seam, after any GitHub checkout has fixed the final cwd, with a trusted or untrusted project override.
The cwd is quoted as a TOML key and resolved on the target host; omitted trust adds no Codex override. Muse true adds
`--trust-workspace`; Muse false adds no trust flag and cannot undo trust from `--yolo` or vendor settings. Pi true adds
`--approve` and Pi false adds `--no-approve`, independent of Pi's YOLO tool mode. The false branches remain for old
stored launches, including plain Restart and Replace; new composition uses true or omission. Older selection JSON
without the field continues to decode.

Schema 25 resets schema-24 composer history for the same reason. Schema 24 retained only the timestamp attached to its
sequence eviction cutoff, which cannot be converted into a safe timestamp/ID frontier when sequence and clock order
disagree. Schema 25 records both frontiers from the start; resetting bounded suggestion data is the only honest upgrade
because the missing eviction history cannot be reconstructed.

Ordinary recents and search-result recents group a complete selection by the same per-launch canonical destination and
by normalized effective permission, so an omitted YOLO default and an explicit stored YOLO row do not duplicate one
another. Legacy launch rows without that fact keep their own spelling distinct instead of consulting a mutable folder
projection or guessing aliases. Frequency sorts descending and the newest retained occurrence breaks ties. An omitted
model, or effort remains the saved harness default and is part of the complete selection identity, not a wildcard that
merges explicit choices. Permission identity uses the effective mode, including an omitted YOLO default.

The editable folder field offers its bounded history as a grouped combobox. Its optional active index stays empty until
arrow navigation, preserving ordinary form submission on a typed path. Pointer and keyboard choices share one guarded
callback; each recent choice carries the installation that offered it. Labels escape peer text before retaining the last
35 characters with a start ellipsis; narrower layout clips the leading edge while retaining LTR direction and the full
accessible name and tooltip. The list floats below its input like model/search suggestions, with its own bounded scroll
region and keyboard scrolling of the active row. Removing an in-flow list on pointer blur would resize and recenter the
dialog between mousedown and mouseup, moving Launch or another pressed control away before its click.

The composer retains the installation claim when a recent setup or saved folder is applied, not only while offering the
suggestion. A replacement under the same registry row disables Launch and requires an explicit host or folder choice.
The same installation reconnecting keeps the claim valid. Captured history callbacks check the current destination
before changing the draft; submission checks it before and after key minting. New carries the selected session's folder
beside its installation snapshot from AppBody, independently of the filtered sidebar projection.

- State in SQLite at `~/.local/state/farhelm/helm.db`: host registry (SSH destinations, host identities, and optional
  aliases), last-known session cache (survives helm restarts per SPEC.md), recoverable web token, hashed browser device
  sessions, and the one client preference (list order, last-selected session, compact rows, and host setup/removal
  confirmation choices) every client shares. Its ordinary store opens share the supervisor's journal setup: owning opens
  attempt WAL before migrations, warning and continuing in the current mode if conversion fails or does not return
  `wal`. Non-owning opens never convert; every ordinary read-write connection selects `synchronous=NORMAL` only after
  reading an actual WAL mode and otherwise retains FULL. This accepts losing the last commits on an OS crash or power
  loss without weakening database consistency; process crashes preserve commits. Host removal, host command, YOLO and
  both browser-link permission setters, and patches that set or clear tunnel consent, temporarily use FULL (EXTRA if the
  file still uses a rollback journal) so a power loss cannot restore acknowledged trust revocations, and restore the
  connection's prior sync setting afterwards. Unrelated preference patches remain NORMAL. On macOS, FULL and EXTRA make
  these writes durable only up to the drive's cache: SQLite does not request `F_FULLFSYNC`. The maintainer accepted this
  limit, judging the additional protection marginal and expecting most Macs running Farhelm to have batteries.
  First-contact identity recording can remain NORMAL: a forgotten consumer copy is learned again from the supervisor's
  durable source identity. Schema and local-row setup are atomic or repeatable database-only work. The separate
  checkout-config CLI opener is unchanged and keeps FULL. Backup and restore must keep each database with the `-wal` and
  `-shm` files its backup includes and remove current sidecars absent from that backup before restoring, so unrelated
  journal data is not replayed.
- Helm schema 36 drops the `profiles` catalog and its remembered default with no conversion (SPEC.md, the launch-kinds
  upgrade paragraph). Cached session rows may still carry the removed `source_profile` member; serde ignores it, so they
  keep listing without a rewrite. Helm schema 37 converts every cached session for launch kinds (see "Launch kinds: one
  resolved launch"). Nothing derives a resume command since protocol 39: the helm composes an agent launch's start and
  resume commands, a command launch carries its own, and a legacy session keeps what it stored. OpenCode is a structured
  harness only: its release catalog holds verified Zen model IDs, accepts an omitted model for OpenCode's configured
  default, passes explicit bare custom Zen names as `opencode/<model>`, and maps YOLO to OpenCode's `--auto` flag. Its
  empty effort vocabulary, generic activity classifier, and absent resume template deliberately avoid claiming a
  provider-specific effort or conversation lifecycle contract. Cursor likewise maps its structured harness to the
  existing Generic kind, with no resume template or capture machinery. The structured harness launches `cursor-agent`.
  Models use `--model`, with no separate effort flag. The UI preserves its harness in launch intent while explicitly
  disclosing the lack of tracking and Resume. Protocol 27 adds the Cursor harness variant, not a new runtime integration
  kind. Protocol 29 adds Grok as both a structured harness and a durable agent kind. Its compiler now emits
  `grok --no-leader --permission-mode auto` for guarded and `grok --no-leader --always-approve` for YOLO, refuses model
  and effort choices, and retains those flags before `--resume {conversation} {farhelm_args}` as argv elements. The
  dedicated kind preserves the ownership policy across helm and supervisor storage. It uses the generic activity
  classifier while the manually configured reporter supplies ownership-proven conversation capture. OMP is also a
  structured harness only at launch time: its release catalog holds the same OpenRouter model IDs as Pi's, omits model
  and provider flags for the harness default, and compiles an explicit model as `omp --provider openrouter --model <id>`
  (provider intent explicit; a literal custom id stays one argv element and is stored verbatim — provider qualification
  is not a promise of literal upstream routing for unknown ids; OMP's own resolution still runs alias, fuzzy, and
  `:suffix` interpretations on the id it receives, as documented in `website/src/content/docs/docs/agents/omp.md`).
  `--thinking <effort>` carries the seven-level list (`off` through `max`; OMP's `auto` is not offered), and
  `--approval-mode yolo|always-ask|write` carries the YOLO/Approve/Guarded choices. An omitted permission normalizes to
  YOLO, as for Pi, OpenCode and Goose; explicit Approve and Guarded remain explicit. SmartApprove and Chat are refused
  for OMP; the row glyph is the Greek capital omega, chosen so it cannot read as Pi's "P" at sidebar size.
- The launch composer's model field is a bounded combobox: it lists the selected harness's catalog filtered by the typed
  text, can reveal every harness's models with each foreign row suffixed by its harness, and accepts a custom id only
  after an explicit harness selection. Enter applies an arrow-navigated row over the typed text, so a half-typed filter
  can never become a custom id behind a highlighted model. This keeps known ownership visible while preserving the
  custom-model contract without a separate disclosure. Its rows are not sequential tab stops: Arrow and Enter reach them
  through the input's active descendant, and Tab leaves the field for the next control. Blur closes the list, so a
  tabbable row would unmount under the keyboard and drop focus out of the surrounding dialog.
- The host registry (PLAN_M6.md item 3) reserves one row for the machine running the helm itself: auto-created at `open`
  if absent, never registered, retargeted, or removed through the ssh-host management API, so its destination and its
  existence are not user management surface — but its alias is user-editable on the same terms as any other host's. It
  exists specifically so the local host has a cache row to serve stale sessions from when its own supervisor is down —
  the plan's first draft made this row optional, and review caught that a row-less local host would have nowhere to
  cache into, breaking the very promise (stale sessions survive a down host) the cache exists to keep. An SSH row also
  carries optional `remote_farhelm`/`remote_state_dir` fields (the argv fields M1's
  `--remote-farhelm`/`--remote-state-dir` carried), `None` meaning "use the remote's own default", not "unset for now".
  Two distinct SQL mechanisms enforce two distinct invariants here, not one: the `hosts` table's own `CHECK` constraint
  is what pins the local row's NULL destination/remote-field shape, while separate partial unique indexes enforce at
  most one local row, uniqueness of an SSH row's destination among SSH rows, and — see below — at most one row claiming
  any given host identity.
- Each host registry entry also has a helm-minted canonical UUID (`hosts.entry_uuid`, schema 62), independent of the
  supervisor's reported installation identity. The local entry and every new SSH entry receive one; a registration that
  converges on an existing row, an edit, a retarget and an adoption preserve it. Removing an entry and adding it again
  mints a different UUID. Migration backfills existing entries in the same transaction as the schema change and adds a
  unique index. It appends the column without rebuilding `hosts`, whose deletion would cascade into host-owned state.
  Reads refuse a missing or noncanonical UUID instead of inventing a replacement identity.
- Session identity inside the helm is `SessionKey { host, session_id }`: the never-reused integer entry number and the
  supervisor's bare ID. Public REST paths and replies, agent replies, selection and approval descriptions use
  `SessionRef`, formatted as the entry UUID plus `:` plus the bare ID. Parsing splits at byte 36, never the first colon.
  The shared admission predicate permits at most 40 bare UTF-8 bytes (77 full), rejects empty, control-containing and
  dot-segment suffixes, and also validates parent and checkout-origin IDs. Every list/create/mutation ingress uses it;
  old unrepresentable cache payloads are skipped from public rows and counts without deleting raw membership. Supervisor
  wire IDs and credentials remain bare under their existing larger relay limit. Host snapshots and mutation claims carry
  the entry UUID with their connection provenance. Projection uses that captured prefix even if removal wins after a
  successful reply, and always qualifies a peer suffix rather than parsing it as full. Parent filters compare the
  host/session pair alongside any independently supplied host filter.
- A host alias is nullable display data: when set it replaces the derived SSH destination or local `this machine` label.
  Input is trimmed; an empty result clears the alias. Nonempty aliases reject control characters and are limited to 64
  Unicode characters. The browser mirrors these syntax checks, while the helm owns collision checks and remains the
  authority for accepting the write. Setting one is checked against every OTHER host's current display name, alias or
  derived — a stored alias is arbitrary text with no uniqueness index of its own, so this is the only check that can
  catch it colliding with anything. Every write that instead touches a DESTINATION (registering, retargeting, or
  clearing an alias to restore the derived name) is checked only against other hosts' current ALIASES:
  destination-versus-destination collisions are already the `hosts_ssh_destination` partial unique index's job, so
  checking against every display name there would just re-detect what the index already refuses, under the wrong error.
  Skipping the destination-side check entirely, though, would let a registration, a retarget, or a clear reintroduce the
  exact ambiguous name an alias write had just been refused for. The alias rides the manager's host snapshot with the
  destination and connection state, so session rows, single-session reads, and agent relay views derive one coherent
  name without each joining the registry.
- The add host dialog marks its `POST /api/hosts/probe` as a new entry (`new_entry`, carrying the optional alias); no
  other caller sends it. Without the marker the probe keeps converging onto an existing row for the destination, which
  is how a host row's "set up again", path repair, a confirmed add's own second registration and update confirmation
  work. With it, the alias passes the alias syntax rules and the helm refuses a destination that already has a row
  (naming the row's display name) or an alias equal to any host's current display name, at three points: a read before
  the probe contacts ssh, again when a pending setup confirmation is confirmed (before the run claims the host), and
  inside the registration transaction, which is the only race-free one and writes the alias with the new row. The marker
  rides the pending confirmation, and the registration derived from the stored row after the insert carries none, so the
  run's later registration converges as before. Spellings are not normalized.
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
- SPEC.md's install-bound create default rides on a denormalization of that recorded identity: every session listing and
  detail row carries `host_identity` (the registry's value for the row's host), the UI snapshots it together with the
  host id at selection time, and the default holds only while the row still reports the same identity — the identity,
  not the client-side fingerprint or the connection token, because only it changes exactly when the install does (an
  address-only retarget or a reconnect must not evict the default). The key is serialized even when `null` so a client
  can tell "records no identity" from "predates the field", and only the latter degrades to the row-id-only comparison.
  The identity is a second read beside the session snapshot and cannot be atomic with it, so both assembly paths read
  the registry FIRST: a retarget straddling the reads then yields a stale identity on fresh content — mismatch, safe
  fallback — rather than a fresh identity on stale content, which would be a false match onto the wrong machine. The
  identity-less residual SPEC.md accepts shows up here as `null == null` passing; a host frozen in the identity-mismatch
  phase is disqualified outright, since its recorded identity still names the predecessor.
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
  tables disagree on purpose: a cache row that no longer decodes is dropped from the read with a warning naming the host
  and session rather than failing the read (it is last-known display data, not authority) — dropped from the counts too,
  so the served list never describes a row nobody can render — while a corrupt registry row still fails `list_hosts`
  loudly (the registry is authority for which hosts exist at all).
- One connection actor per registry row, the local row included (PLAN_M6.md item 4), each owning its transport
  connection, its reconnect state machine, and its slice of the session cache. A row's connection is always in exactly
  one of the connection states below, with additional provider states for environment hosts. The refusal states exist
  because folding them into "unreachable" would throw away the only information that makes the situation fixable:
  **connecting** (active retries in progress), **unreachable-reprobing** (the active window is spent, background probes
  continue forever, no give-up), **connected**, **version-skew** (the hello was answered and refused; carries both
  protocol versions, the peer's build, and the remediation text, since SPEC.md demands actionable rather than merely
  diagnostic errors), **identity-mismatch** (frozen, carrying both identities, connecting nothing until the user adopts
  or fixes the destination), **identity-unverified** (the peer cannot confirm the recorded identity, so nothing is
  connected), and **duplicate** (this entry's identity is already another entry's; frozen like identity-mismatch,
  connecting nothing until a Retry, an edit of the entry, or a helm restart asks again, while the entry stays visible
  naming the other one), and, for environment hosts, **suspended** (the provider reports suspension or failed resume,
  with nothing dialed), a reason-bearing changing state during a lifecycle transition, and Podman's distinct
  stopped/start-failed state (see "Disposable environments" and "Tensorlake hosts"). First contact checks whether
  another entry holds the reported identity before comparing it with the one this entry recorded, so an entry that
  recorded one install and now reaches another entry's machine is a duplicate; and an adopt prompt shown before the
  other entry recorded the identity turns into the duplicate state when its adoption is refused, because the refusal
  starts the entry's next attempt. The local row's unreachable state additionally distinguishes "no supervisor is
  running on this machine" from a generic transport failure, because that is the one case whose remedy is a command on
  the machine the user is already sitting at — a manual-path hint, never an offer to install (provisioning is M7's). A
  separate state exists that is not about the host at all — **retired** — for an entry whose actor has stopped: a
  panicked task, or one that outlived its own registry row. Without it, an actor's last published status stands forever
  after the actor is gone, so a task that died mid-connection would leave the entry reading connected, with a routable
  client, and nothing left running to ever correct it. Each actor is therefore supervised by the task the manager
  actually holds, which publishes the retired state (client dropped) when the actor it wraps finishes for any reason
  other than being cancelled on purpose.
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
  seconds. Version-skewed entries ride the same 45-second cadence, so an upgraded host resurfaces by itself.
  Identity-mismatch and duplicate are deliberately the states with no timer at all, because no amount of waiting answers
  a question only a user can.
- Two DEADLINES bound what a peer can do to an actor by saying nothing, both injectable alongside the cadences. One
  connection attempt shares a single 20-second budget across opening the transport, exchanging hellos, recording the
  host's identity and applying the browser-link setting. Expiry names the last observed stage and is an ordinary failed
  attempt, so the ladder and re-probe cadence carry on unchanged. The actor retains stage evidence outside the cancelled
  attempt future: local socket connection, waiting for hello and final setup are separate boundaries; spawning SSH alone
  establishes only that it is connecting to the host. A remote connected marker establishes waiting for hello; without
  one, timeout wording follows the marker-capability evidence described in the transport section. Active attempts
  publish stage changes through the existing changed-only host snapshot, guarded against retargeting; backoff is
  explicitly waiting to connect, and background re-probes never publish Connecting. One cache refresh is bounded at 30
  seconds, and expiry drops the connection so the actor re-enters its normal loss handling. Without these deadlines a
  transport that accepts and then goes silent parks the whole state machine indefinitely while every layer below looks
  healthy — no error, no EOF, and a host that reads as connecting or connected forever.
- Editing a registry row's connection-defining fields RECONNECTS the host rather than waiting for its current connection
  to end on its own: the connection is torn down, a non-connected state is published together with the new row (so a
  hosts list can never pair an edited destination with the old connection's state), and the actor gets a fresh active
  window, which is the same treatment resolving a freeze earns. An explicit retry is the same restart without the fresh
  window — a user clicking retry is not evidence that a down host is back, so it makes one attempt and returns to the
  re-probe cadence, while a connected host's retry is a genuine reconnect rather than an early poll.
- A cache write refused because this connection's identity is no longer the row's also ends the connection. Every later
  refresh on it would be refused identically, so keeping it up would show a host as healthily connected while its stale
  list silently stopped advancing; dropping it re-asks the identity question against the row as it now stands.
- A connected host's cache refresh is one request and one replacement: a single `ListSessions`, whose reply carries the
  host's whole list, then that host's whole cache slice replaced in one identity-bound write. One request per refresh
  bounds round trips and terminal-state reads; report reconciliation stays on the supervisor's timer rather than
  multiplying with polls and hints. A failed refresh records the failure and keeps the previous cache, never wiping it:
  the cache's whole job is to answer "what did this host have, last we knew" while the host is unavailable, so clearing
  it on failure would destroy the answer exactly when it becomes the only one available, and would make a transient
  failure look identical to "this host genuinely has no sessions". A host whose supervisor reports no identity at all
  connects and serves live but writes no cache, since the identity binding has nothing to bind to. The reply is checked
  at ingress and refused whole — an ordinary failed refresh that keeps the previous cache — when it is longer than
  `LIST_SESSIONS_CAP` (a peer ignoring the one bound on what this side retains), when a session id exceeds the id length
  cap, or when an id appears twice; a reply cut AT the cap is accepted and remembered as truncated, which is what the
  served list's own `truncated` flag carries forward.
- The served session list is a MERGE, and it is served from what the helm has already recorded rather than from the
  hosts. Every connected host's actor records its supervisor's whole list into helm.db; the list endpoint then merges
  what is there — live hosts' latest refresh and down hosts' last-known entries alike — into one order, tagging each row
  with its host and marking it stale unless that host is connected right now. A host being connected changes only that
  flag, never where its rows are read from.
- The list is served in one of THREE orders, chosen by the request (`?sort=`, defaulting to `created` when absent so
  every client written before there was a choice keeps its behavior; an unrecognized word is a 400, like an unknown
  status). `created` is `created_at` DESCENDING, then session id ascending, then HOST ID ascending — the first two are
  the order supervisors have always listed in and the third is what keeps the merged order total across hosts.
  `activity` leads with an active-first grouping — rows from connected hosts reported Running or Waiting sort above
  every other row — then the session's effective work-start key descending, and `title` with its collated title
  ascending; all three then fall into that same creation-order tail, so every order is total and a stable sort of the
  same rows always yields the same sequence. The effective work-start key is `last_work_started_at` when positive and
  `created_at * 1000` otherwise, so an older sender or a session with no observed burst keeps a stable creation position
  rather than moving with ordinary activity or piling up at the epoch. The title collation is Rust's `str::to_lowercase`
  compared as code points: Unicode's locale-independent FULL lowercase mapping (it can lengthen a string, as `İ` does),
  which is neither SIMPLE lowercasing nor case FOLDING — so `ß` and `SS` stay distinct keys. The result is
  case-insensitive and otherwise ordinal, deliberately not locale-aware (that is an ICU dependency and a per-user
  setting this product does not have). The sort happens in Rust, in memory, over the whole merged fleet, on every
  request; nothing about the order is stored, indexed, or cut.
- Ordering is not filtering, and the two are kept apart: a sort changes the sequence, never the membership, so neither
  `total` nor `matching` moves with it. Past the cap the sequence does decide which rows the reply reaches — the counts
  describe the whole filtered view, while the array is what fits — so "membership unchanged" is about the underlying
  view rather than identical returned rows under truncation.
- One host does not fit the cache rule and cannot be made to: a supervisor reporting NO identity, against a registry row
  that has none on record either, has nothing for the identity-bound cache write to bind to. Its refreshes are kept in
  the connection manager's memory and merged into the list and the owner lookup from there; they serve while it is
  connected and vanish when it is not, because with no durable copy there is nothing to stand behind. A row that HAS a
  recorded identity meeting an identity-less hello is a different situation entirely and fails closed — see below.
- The REST list is served WHOLE, and this is a decision rather than a default (SPEC.md's Session list section states the
  scale assumption and the prohibition). `GET /api/sessions` reads every cached row for every host plus the in-memory
  rows of identity-less hosts, decodes them, filters, sorts, and counts in Rust, and answers with one array cut at
  `LIST_SESSIONS_CAP` with `truncated` set when the cut applied or when any host's own list was cut at the wire's cap.
  The reply is a snapshot: rows and both counts come from the same in-memory view, so nothing can move between them. A
  host's cap flag is kept WITH its cache (`hosts.cache_truncated`, written in the same transaction as the rows it
  describes) rather than with its connection, because the cut rows go on being served stale in every non-connected state
  and across a helm restart, and SPEC.md forbids presenting a cut list as the whole one in any of them; the one host
  with no cache (an identity-less supervisor, serving from the actor's memory) carries its flag on the snapshot beside
  its rows, and both vanish with the connection. Versions 8 through 13 of this list were cursor-paginated at the REST
  layer on top of a cursor-paginated wire, with per-order ordering columns and host-leading indexes in helm.db (schema
  versions 11 and earlier), a Rust-side title fold cut to 128 characters with a batched backfill, a byte budget as a
  second page cut, a matching-count cache keyed to a store generation, and a client-side "underfilled listing" predicate
  — all of it existing because sorting by mutable keys under pagination lets a row cross the cursor between two pages.
  Schema version 13 drops the ordering columns and their indexes and adds the per-host cap flag; `session_cache` keeps
  `created_at` as the identity cross-check a read applies to a decoded payload. Schema version 29 removes the retired
  archive column and strips the retired member from valid object payloads; malformed and non-object payload bytes stay
  untouched so the migration does not turn pre-existing corruption into invented data. What the whole-list reply gives
  up is a per-request cost that grows with the fleet — decoding every cached payload on every poll — and that is
  accepted outright: the decode work is bounded per HOST (up to `LIST_SESSIONS_CAP` rows from every cache-serving host,
  before the merged output is cut back to one cap), which at the spec's scale of a few hosts is a few hundred JSON
  decodes per request. The one deliberate second cut anywhere in the listing paths is the agent verb's encoded-byte
  allowance (6 MiB, `agent_requests`): that reply must fit a single frame and its rows carry unbounded caller text, so
  the helm stops projecting rows before the reply could exceed a frame and reports it through the same `truncated` flag.
- A listing reply carries two counts, and they answer different questions. `matching` is how many rows satisfy the
  caller's filter across the whole merged view, and it is present exactly when a predicate is active. `total` is how big
  the VIEW is — the denominator the UI's "N matching of M sessions" prints — and it deliberately does not move when a
  query narrows membership, because a denominator that tracked the query would compare a number against itself. A row
  whose payload no longer decodes is in NEITHER count: it is dropped at the read with a warning, so `total` and
  `matching` describe rows a client can see and "Showing 4 of 5" never appears over a row nobody can render (the
  corruption is for the log). The ordinary list reads as unfiltered — "M sessions" — and the filtered wording belongs to
  filters a person applied.
- Helm schema 65 removes the global bare-ID unique index. The cache's `(host_id, session_id)` primary key is its only
  index: equal suffixes on different entries are independent rows, with independent counts, mutation seeds and viewer
  state. Identity-less live rows follow the same rule when merged. No contested-ID state, global owner lookup or
  cross-entry list deduplication remains. Full references choose the entry, and raw paired cache membership or that
  entry's coherent identity-less live list proves membership. A host listing one ID twice still contradicts itself and
  its whole refresh is refused, keeping the previous cache; direct duplicate-row writes roll their transaction back.
- The order a supervisor lists in is neither validated nor relied on. The helm sorts the union of every host's rows —
  cached and in-memory alike — per request, so an unsorted or differently sorted reply costs nothing and the one host
  that serves from memory rather than from the cache needs no special handling. Session ids are still bounded at every
  peer ingress by the shared small reference bound above, independently of the unchanged supervisor relay allowance.
- Every mutation whose result changes what the helm has RECORDED records it before answering: a create seeds its new
  session, a restart and a rename store the reply's fresh `SessionInfo`, and a delete forgets the row. The merged list
  and the owner lookup are both served from those records, so a mutation that recorded nothing leaves the list
  contradicting the answer the caller just got — a session that cannot be operated on, a restart that still reads
  `exited`, a deleted row sitting beside its replacement. All of it is best effort and none of it can fail the mutation:
  the operation succeeded, and reporting a success as a failure is the one outcome SPEC.md's creation contract rules
  out. Another entry's equal suffix cannot refuse or redirect a create: output is always qualified with its own
  immutable operation claim, including identity-less and reply-then-disconnect paths. Each write carries the CLAIM its
  operation was routed under — a manager-wide connection token that is never reused, plus the identity — and is dropped
  if the connection has changed since, so a delayed reply cannot file one install's session under another's name. Writes
  are serialized against the host's own refresh, and a refresh whose drain predates one of them declines to commit
  rather than erasing it.
- One field of such a reply is NOT taken as given: a status of `unknown` never overwrites a definite one. The protocol
  is explicit that list and rename replies compute fresh terminal liveness, while create and restart replies' `unknown`
  means "not yet known" rather than "not running" — a create's and a restart's replies carry it deliberately, because at
  the instant they are built the terminal exists but the agent's own exec inside it has not been observed. Recording
  that verbatim answered a successful restart with a badge saying the helm had no idea, for a session it had definite
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
- Session operations parse the full reference, resolve its entry UUID and check only that entry's membership — from the
  cache's COLUMNS, never from the stored metadata, so a row whose payload no longer decodes still routes and a live
  session is never made unreachable by a corrupt copy of its own details. A session whose host is in any non-connected
  state is refused with the state named and nothing queued. Unreachable is not special-cased; a version-skewed,
  identity- mismatched, duplicate, or retired host refuses identically, because a caller that handled four of six would
  silently mis-handle the rest. The routing decision reads the host's state and its live connection from a SINGLE borrow
  of the actor's published status — split across two reads it could pair a fresh `Connected` with a dead connection,
  which is precisely how an operation gets routed onto a corpse. Creation takes the target host in the body (defaulting
  to the local row, the tail of SPEC.md's own creation default) and refuses a non-connected one as a precondition
  failure. Reading a session's DETAIL is the one route a non-connected host does not refuse: SPEC.md requires a stale
  session's metadata to remain available beside the calm card naming the cause, so that read is served from the cache
  and marked stale, while a reachable host's detail is always fetched live — the cache exists for the stale list, not as
  a general serving layer. The live path reads the owner's whole list, which is the only list the wire serves.
- `POST /api/sessions/{id}/replace` (SPEC.md's "Replace") is a composition of the create and delete this section already
  describes, not a third code path: it routes the source by owner lookup exactly like any other lifecycle operation,
  reads the source's row live from the owning host (the same live read `clone_for_agent`'s agent-CLI clone performs, and
  for the same reason — the cache is for the stale list, never a source for a fresh mutation), copies that row's stored
  launch through a function (`sessions::mode_from_source`, which refuses a legacy session) shared with
  `clone_for_agent`, then calls `do_create_session` followed by a delete, reusing the ONE `(claim, client)` pair the
  owner lookup produced for both calls rather than re-routing before the delete. The create half carries the same
  idempotency-replay veto `clone_for_agent` gives its own create: a same-host replace with no overrides can reconstruct
  the source's own creation fingerprint, so a caller reusing the source's key would otherwise receive the source row
  BACK as the "replacement" — this route refuses that reply with `Conflict` before any bookkeeping runs, rather than
  deleting the session it was just handed back. Create runs before delete so a failed create leaves the source
  untouched. A delete that fails AFTER a successful create is reported one of two ways depending on whether the failure
  is a DEFINITE answer: an explicit supervisor refusal, or a delete that never reached the wire at all, means the source
  was not removed, and the reply names both ids and says both sessions still exist; a delete that reached the wire and
  then lost its answer (the connection dying after the frame was sent, or any other post-send ending this client cannot
  interpret) is NOT definite — the supervisor may have completed it — so the reply instead says the replacement exists
  and that the source's fate is unknown and must be checked before deleting it again or retrying. Neither shape ever
  rolls the create back (killing an agent the caller just asked for) or claims success (hiding a session, or an
  uncertainty, the caller needs to see).
- "Replace with" (SPEC.md's bullet of that name) reuses the same endpoint through `ReplaceReq`'s optional `with` field —
  a whole `CreateReq`, the same type an ordinary create's body decodes into — rather than a second route or a second
  override type: present, its cwd/title/dimensions/mode selector are resolved exactly as an ordinary create's own body
  is (including that body's own mutual-exclusivity refusals), and used in place of the source's live row; absent,
  behavior is byte-for-byte unqualified replace. Replace never changes machine, with or without an override, so a
  `with.host` naming anything but the source's own host (compared against the `claim` `route_session` resolved for the
  source, not a fresh registry read) is refused with `Conflict` before either the create or the delete runs — the same
  refusal shape `precondition::incarnation_holds` already gives an ordinary create, applied here to `with`'s own
  `expected_incarnation` too. The client's idempotency binding folds the replace-with source id in alongside the
  ordinary create fields (`create_form.rs`'s `IntentBinding::replace_source`), so retrying a replace-with reuses its own
  key and an ordinary create can never collide with one — and the wire body's own two `intent_key` fields (the
  endpoint's own, and the one `with` carries because it is built from the identical create-body function) must agree, a
  caller sending two different keys for what claims to be one intended create being refused as a 400 rather than
  silently resolved by picking one.
- Host management commits first and converges the live actors after; ordinary WAL/NORMAL commits survive process crashes
  but may be lost on an OS crash or power loss. Each verb states how it fails closed: add rolls its row back if no actor
  could be started (a registered host with no actor is invisible and un-dialed, while its destination is taken);
  retarget converges instead of rolling back, because the durable write is what the user asked for and the actor can be
  told to reconnect (not guaranteed: with an unreadable registry, reviving a dead actor fails and is reported, and a
  live actor keeps its old row); remove tears the actor down by the id it just committed, needing no registry read that
  could fail. Retry reports whether it found a host, and a RETIRED host's retry respawns its actor from the current row
  — nothing else ever restarts one, so without that an actor that panicked left its host permanently dark. Adopting
  names the identity the user was shown and is refused if the host has since started reporting a different one, because
  a re-probe between the decision and the request would otherwise adopt something nobody approved.
- `--ensure-hosts <file>` is a JSON5 floor under the registry, applied through the same registration path as a REST add
  before serving begins and never consulted again. It adds what is missing and touches nothing else: an already
  registered destination keeps its fields and its learned identity, because helm.db is the durable authority and a
  startup file that overwrote user edits every boot would make the two fight. Validation is all-or-nothing — a malformed
  file, an unusable destination, or a destination listed twice fails startup with the entry named and nothing written,
  since a helm that came up with three of five guaranteed hosts looks healthy and is not.
- axum serving: REST for CRUD (sessions, hosts), a WebSocket event stream for live session-list updates, and a WebSocket
  per attached terminal. The helm serves no page; the desktop's embedded helm uses a fresh kernel-selected loopback
  port. Loopback bind enforced — refuses non-loopback per SPEC.md.
- The desktop app's credential is the only credential the helm accepts in production; there is no browser sign-in. The
  one other credential is the test hook `farhelm helm run --test-credential-file PATH` (hidden): a standalone helm mints
  one in memory the same way and writes it to a new 0600 file, so end-to-end tests can reach the real binary's API. The
  embedded helm mints the desktop's in memory at startup and hands it to the desktop process through `run_embedded`'s
  readiness channel; it keeps only its SHA-256 digest, in memory, so only the current launch can authenticate. No HTTP
  route mints one. It dies with the process, so nothing persists or accumulates across launches, and the app keeps none
  on disk. With nothing to revoke, the desktop has no re-authentication path: a 401 is reported as an error.
- The loopback guard accepts a request only when its `Host` is the IPv4 literal `127.0.0.1:<port>` (bare `127.0.0.1` on
  port 80, where clients omit the default port), it carries no `Origin` header, and its `Sec-Fetch-Site`, if any, is
  neither `cross-site` nor `same-site`. Farhelm's own clients never send `Origin`, so any request that carries one came
  from a web page and is refused whatever its value; the custom-scheme origins the Dioxus webview needed left with it.
  `Sec-Fetch-Site` catches browser requests without an `Origin`, such as a top-level navigation from a hostile page or
  from a page on another loopback port. `localhost` and `[::1]` are refused because the helm binds only IPv4 loopback,
  leaving `[::1]:<port>` free for another local account to bind, and a client resolving `localhost` to `::1` would talk
  to that account's server. Binding `[::1]` as well was rejected: IPv6 loopback can be enabled after the helm starts, so
  the helm cannot hold that address reliably. Every refusal is a plain 403; the helm serves no page, so there is nothing
  to redirect to.
- The native app embeds farhelm-helm in-process; the Linux helm is the same code behind `farhelm helm run`. The local
  supervisor is a separate process either way — the app discovers one that already answers and leaves it alone (unless
  it is the app's own outdated or wedged one), or starts `farhelm supervisor run` detached, and it keeps running after
  the app quits (see "Side-by-side versions inside Farhelm.app").
- Schema 29 adds the supervisor session column `launch_hooked` (integer, default 0). New spawns record the argv decision
  before the agent's terminal starts; relaunch clears it, and a pending-create retry inserts a fresh unhooked decision
  before spawn decides again. Older supervisors refuse this upgraded database on downgrade; upgrades preserve stored
  conversations and notifications. No hook clock or diagnostic latch is persisted.
- Schema 30 adds the supervisor session column `launch_boot_ended` (integer, default 0), the boot-finality proof
  described under Runtime state. Older supervisors refuse this upgraded database on downgrade.
- Schema 32 adds the supervisor session column `terminal_kind` (text, `tmux` or `host`, held to those two by a `CHECK`,
  default `tmux`): whether the session's agent terminal is a tmux pane or a terminal host, so a restarted supervisor
  looks for it in the right place. For a host row `pane` (`terminal_address` since schema 34) holds the host's socket
  name, so an empty `pane` still means "launch not confirmed" for both kinds. The row's insert writes the kind, never
  the confirm, so a crash before confirm is reconciled against the right kind, and a create retry whose kind differs
  from the crashed attempt's replays instead of taking the row over (the evidence that the attempt started nothing
  covers its own kind only). A relaunch writes the kind of its new launch as it opens the generation (a Restart that
  moves an old tmux session into a terminal host writes `host` there), saves the previous kind with the rest of its
  prior state, and an abort restores it with the pane, so a restored `pane` is read the way it was written. Existing
  rows become `tmux`. Older supervisors refuse this upgraded database on downgrade, which is why the kind got a schema
  bump instead of riding on an existing column: a supervisor from before terminal hosts would otherwise take a host's
  socket name for a tmux pane, find no such pane, and record a live session as exited (SPEC.md, "Upgrade compatibility
  and client scale"). Schema 32 alone did not fence every such build: builds at 32 decode the kind but do not act on it
  everywhere, so the change that let a supervisor be started with host terminals on bumped the schema again, to 33, so
  that every one of them refuses a database that may hold host rows.
- Schema 33 changes no table or row. It came with the since-removed `farhelm supervisor run --new-terminals`, so that
  every build at 32 (which stores the kind but does not handle a host row everywhere) refuses a database that a
  supervisor able to create host terminals has opened.
- Schema 34 drops `sessions.tmux_name` and `create_reservations.tmux_name` and renames `sessions.pane` to
  `terminal_address`, so that a row of a terminal that is not a tmux one carries no tmux session name and code reading a
  host row reads no tmux pane field (SPEC.md, "Upgrade compatibility and client scale"). Every stored tmux name was
  `fh-` and the session's id, as the tmux code derived it, and had been since sessions were first persisted; the step
  checks that before it drops a column and refuses the upgrade, naming the row, for one that is not (only a hand edit
  makes one), rather than derive a name the row never had and lose an old tmux session. Of the reservations it checks
  only pending ones, the only ones whose name was ever read again. One address column serves both kinds rather than one
  column per kind because every rule about it is the same for both: written with the confirmation, emptied by a
  relaunch, restored by an abort together with the kind, empty meaning "launch not confirmed". `sessions` is rebuilt
  (SQLite's `DROP COLUMN` refuses a `UNIQUE` column): the old table is renamed aside and copied by column name, the
  notification trigger dropped first and recreated on the new table, all in one transaction with the version bump. The
  rebuilt `terminal_kind` has no default (schema 32 gave it `tmux`): every writer names the kind, so the default changed
  nothing, and without it a writer that forgot the kind fails at its insert instead of quietly writing a tmux row. Like
  every step it is one-way: a build at 33 refuses the database.
- Schema 35 adds `supervisor_meta.sealed` (integer, 0 or 1, default 0), the environment seal described under
  "Environment Delete and the seal". It is a schema step rather than a file beside the database so that a build from
  before the seal, which would accept creates on a sealed supervisor, refuses the database instead.
- Session notifications (SPEC.md, Status) are recorded by the supervisor, which holds the specifics their wording
  depends on (which agent, which reporter, why a hook was not added), in a `session_notifications` table of its own
  database: a per-session sequence number that only grows, a kind naming the problem, the launch generation it belongs
  to, the time it was recorded, its user-facing text and a nullable `resolved_at`. `(session, generation, kind)` is
  unique: resolution retains that row, and recurrence reopens it with a new sequence, time and text and clears
  `resolved_at`. An unresolved duplicate stays ignored. This survives a supervisor restart with no separate latch; only
  an insert beyond a session's 10th drops its oldest, and deleting the session deletes its rows. The kinds are the hook
  that never reported (the 65-second tripwire above, recorded by the capture pass that runs it, only for an agent whose
  report is due by its first prompt, `RestartReadiness::due_by_first_prompt`, and not for a launch already told its OMP
  reporter does not match; its text uses `AgentKind::display_name` and `RestartReadiness::clause(ToTheUser)` to name the
  agent and its usual timing, then explains the loss of Resume and suggests checking `{farhelm_args}` for custom
  commands or sending feedback from the help menu), a hook that could not be added (recorded by the spawn that decided
  it, except when `FARHELM_AGENT_HOOKS` turned hooks off for that agent, which is the user's own choice, and except for
  a kind that adds no hook at all), a Codex or Grok record whose verification withdrew the resume offer (recorded only
  after the withdrawal is committed, and only on a clean verdict, not a read error; the background refresh and Grok's
  final check before a Restart both record it), an OMP launch whose recorded reporter is older than this build's or
  whose installed reporter file is there with different contents (a missing or unreadable file records nothing), and a
  Claude, Codex or Muse process pin corrected after a recorded ancestor replaces a child-first conversation (verified
  against the exact record for Codex). These are separate `ClaudePinCorrected`, `CodexPinCorrected` and
  `MusePinCorrected` kinds. Each correction records the consequence for Restart before adoption and remains unresolved,
  giving one notification per launch. The old-terminal kind (`old_terminal`, `NotificationKind::OldTerminal`) was the
  one about the session rather than a launch, recorded by earlier releases while something in the session ran in an old
  terminal; every supervisor start now resolves every unresolved row of it in one update fenced by neither session,
  generation nor conversation (`SessionStore::resolve_notifications_of_kind`). Its successor, `old_terminal_ended`
  (`NotificationKind::OldTerminalEnded`), is recorded at startup, at the converted launch's generation, for each session
  whose agent the startup's ending of the old terminals converted to interrupted ("Ending the old private tmux server");
  nothing resolves it, so a later start, which resolves `old_terminal` rows, leaves it as it is. The capture pass
  touches neither. The first two say Restart cannot resume, so they are refused whenever the launch's row holds a
  captured conversation, checked in the same statement as the insert, which is also what closes the race between a
  report's commit and the tripwire. The listing carries the last 10 on `SessionInfo::notifications` as sequence number,
  time, text and an additive `resolved` bool (false omitted and the decode default): the kind and generation stay
  private to the store and supervisor's in-memory notification snapshot, because an enum on `SessionInfo` that a newer
  supervisor extended would make the whole record fail to decode in an older helm, which drops a session it cannot
  decode. The field is additive with a decode default, so an older helm shows no bell and an older supervisor sends
  none, and needed no protocol bump. Replies are built from immutable entries, so the list lives in a session-scoped
  cell beside the activity time, seeded from the store when an entry is built from a row and replaced after each
  recording or resolution, which then sends the `SessionsChanged` hint. Each capture pass resolves the silent-hook and
  resume-withdrawn kinds only when the generic restart offer is Resume, rather than merely when a captured conversation
  exists. This includes reports admitted before startup publishes the entry. An unresolved non-resolving kind or an
  earlier launch's warning remains history; clearing in the helm does not remove it from the store. The in-memory
  snapshot retains kind and generation beside the wire fields, so a pass with no unresolved, resolving kind for the
  current launch skips the store call entirely. An eligible pass uses two indexed updates in one transaction. The
  resolve SQL compares the exact captured conversation and generation on the session row, so an earlier Resume
  observation cannot erase a later withdrawal. Notification reloads order snapshots by newest sequence and then resolved
  count: with the newest sequence fixed, only resolution changes, and it only grows; reopening raises the sequence.
  Schema 28 adds `resolved_at` on the forward ladder, preserving old unresolved history; older supervisors refuse the
  upgraded database on downgrade, as accepted for this change. The helm keeps read and cleared state as two per-session
  sequence marks ("read through N", "cleared through N") in a `session_notification_marks` table beside `session_seen`,
  keyed and cleaned up the same way, set by the two endpoints `PUT /api/sessions/{id}/notifications/read` and
  `PUT /api/sessions/{id}/notifications/cleared`, each taking `{"through": N}`. A mark never moves backwards and is
  clamped to the newest sequence number the helm's cached row holds, so a mark can never cover an entry that arrives
  later; the fleet-events revision is bumped only when a mark moved. Building rows for the UI, the helm keeps at most
  the 10 newest entries whatever a supervisor sent, drops cleared ones, and reports the read mark so the UI can tell
  unread from read. Clearing deletes nothing in the supervisor; the cap ages records out there. The UI renders each text
  as peer text, marks resolved entries grey with a "Resolved" word, and excludes them from unread counts and "new"
  styling without moving the helm's read mark.
- Per-session "seen" state (the idle dot's grey/blue split and the read/unread toggle; SPEC.md, Status) lives in its own
  `session_seen` table keyed by `(host_id, session_id)`, not as a `session_cache` column: that cache is replaced
  WHOLESALE by each host's refresh, so a column there would need every refresh to carry forward a viewer fact the
  supervisor never reported. The host number names the durable registry entry, so retargeting and adoption preserve the
  state; a foreign key to `hosts` with `ON DELETE CASCADE` removes it with that entry. The stored value is the ACTIVITY
  STAMP that was current the last time some client had the session open, never a wall-clock "seen at" time — comparing a
  later activity stamp against an earlier one from the SAME host involves no clock at all, where a wall-clock comparison
  would pit the session's host's clock against the helm's own, on a remote host a different machine's clock entirely
  (the same reason the relative-age column reads an activity stamp rather than a "last seen" timestamp). One caveat
  follows directly: the supervisor quantizes `last_activity_at` at the source, advancing it only when what it observes
  is at least a minute newer than what it already holds, so work landing within that minute after a mark-read does not
  yet register as unseen — cosmetic, and not worth a second, finer-grained stamp. The table is a helm-local write with
  nothing to refuse: `PUT /api/sessions/{id}/seen` does not route through a session's owning host at all (unlike every
  lifecycle verb above), so a session on an unreachable host can still be marked read or unread, and the write bumps the
  fleet-events revision only when the stored value actually changed — a client re-marking the SAME stamp it already
  recorded — which happens when a session is reopened with no new activity behind it, or when a client retries a PUT
  whose response it missed — must not wake every other connected client to redraw a dot that has not moved. Deleting a
  session drops its `session_seen` row explicitly, since the viewer row is independent of the cache. A session that
  disappears from its host's listing without being deleted through the helm leaves its row behind until its host entry
  is removed, which is accepted as garbage bounded by the number of sessions that ever existed, at a few dozen bytes
  each.

- Helm schema 63 re-keys `session_seen`, `session_pins` and `session_notification_marks` from bare IDs to
  `(host_id, session_id)`. Migration retains a legacy row only when exactly one cached host claims its ID; ownerless or
  multiply claimed rows are dropped. Existing live contested markers were never persisted, so migration cannot recover a
  contest forgotten at restart. Each table is rebuilt in the schema transaction; hosts and their cache are retained.
  Store APIs carry the pair from the caller's resolved or snapshotted host, and batch results use the pair as their key.
  Equal bare IDs on different entries cannot share viewer state.

- Helm schema 64 migrates `preferences.last_selected` alongside the public ID switch. Every old value is bare, including
  text that happens to parse as a full ID. Exactly one raw cached membership and a representable suffix qualify it with
  that entry's UUID; unowned, ambiguous or unrepresentable values clear. Spelling alone never supplies provenance. New
  public writes require full-reference syntax but permit stale references; null and sparse updates keep their existing
  meanings. A version-only synthetic rewind may clear an already-qualified selection rather than guess.

- Shared pins live in `session_pins`, keyed by `(host_id, session_id)` since helm schema 63, with a NOT NULL signed
  `position` since schema 74. The 73→74 step rebuilds the presence-only table using the same DDL as fresh creation,
  seeding positions from oldest positive creation time recorded in the cache, then unknown times, with
  `(host_id, session_id)` breaking ties. Missing cache rows and nonpositive times are unknown; the separately recorded
  time does not depend on decoding the cached JSON. A version-only rewind with the position column already present
  preserves it. Like `session_seen`, it cascades on host removal, survives retarget/adoption, is cleared explicitly on
  deletion, and accepts bounded stray rows from other deletion paths without a reaper. `PUT /api/sessions/{id}/pin`
  requires a `{pinned: bool}` body, verifies helm-local ownership even for an unreachable host, and bumps the event
  revision only when membership changes. A new pin appends after the stored maximum; retries retain its position, and
  unpin/re-pin appends again. Only an overflowing maximum requires compaction before appending, in the same transaction.
  Listing and detail rows carry `pinned: bool` and `pin_position: integer | null`; the list reads positions in one batch
  for both cached and identity-less live rows. The helm sort stays unchanged for the quick switcher. The sidebar stably
  partitions accepted replies, orders the pinned partition by that total key before applying its reorder hold, and
  computes the separator from the resulting display. A hold retains displayed IDs and original pin membership across
  reply reconciliation: original pins still pinned now sort within their held slots; other rows keep their slots,
  disappearing IDs are removed, and new IDs append. Reconciliation adopts the displayed IDs without recapturing pin
  membership. Move neighbors come from the displayed leading run of original, still-pinned rows, excluding pins added
  during the hold even when contiguous. Missing or mixed pin positions retain the older stable partition with no Move
  controls. Row menus use their existing action-identity focus reconciliation; accepting Move returns focus to the keyed
  row's toggle before the request, then uses the ordinary row refusal and explicit listing refresh paths. The page's
  menu-focus helper observes direct list-child mutations only while a session toggle owns focus. If its keyed row is
  removed and reinserted, the helper restores focus lost to the document body after that actual DOM mutation, provided
  the same node remains connected, the document still has focus, and no independent focus intervened. A later focus
  choice or row removal retires that observer; focusout without replacement focus retires it at the next animation frame
  if reinsertion has not restored focus. An old mutation or frame callback cannot affect its successor. No optimistic
  order or client write queue is introduced. Pointer dragging uses `pin-order.js`, an unconditional asset on web and
  desktop, with its eval bridge owned by the component that renders the actual list root. Listing error/recovery can
  replace that root without replacing ListView; the bridge owner follows the root so late predecessor cleanup cannot
  release its successor. Setup waits for the asynchronous asset while the root's mount marker remains current, and the
  eval awaits its subscription lifetime. The grip shares the existing displayed eligibility projection and stays out of
  sequential Tab order; menu moves supply keyboard reordering. An armed pointer owns temporary document capture and
  Escape listeners, removed on drop, cancellation or teardown. Geometry is sampled from current eligible rows and
  clipped to the visible pinned group. Only transient lift/drop-line paint changes before the ordinary relative move
  callback; receipt rechecks current source admission and displayed membership without recapturing adjacency. Hiding the
  list cancels its gesture. No HTML drag-and-drop, optimistic DOM order, separate rank or drag focus-retention mechanism
  is introduced. Replace copies a source pin and its position to the new id before requesting the source's deletion, so
  a pin write failure leaves the source intact; if deletion fails both surviving sessions can be pinned at the same
  position. An already-pinned replacement keeps its position. Positions deliberately are not unique, and their total
  order is `(position, host_id, session_id)`. Confirmed deletion clears the old pin best effort, like the other viewer
  metadata.

  `PUT /api/sessions/{id}/pin-order` requires `{neighbor: full_session_reference, placement: "before" | "after"}`. Both
  sessions must be known to the helm, including cached sessions on unreachable hosts, and both must still be pinned; a
  missing pin is a 409 refusal. A neighbor rather than an index makes the request meaningful when filters or a display
  hold hide or rearrange rows. One store call under the existing connection mutex reads the full stored set, including
  invisible pins, moves the requested pin beside its neighbor and renumbers the set from zero in a transaction. Ties and
  gaps normalize even when the requested adjacency already holds; a present self-move does nothing. Later concurrent
  moves act on the earlier result, with no revision check or additional locking. The fleet revision advances only when
  stored positions change. Router ownership keeps that event attached to the write after a client disconnects, and a
  failed write rolls the transaction back.

## Ntfy waiting alerts

The helm observes fresh accepted session listings inside each host connection's refresh loop. Its connection-local
policy takes a small projection of session id, title and raw status, with an injected monotonic time; it never derives
transitions from the restart-persistent session cache. The first successful list baselines waiting sessions. Subsequent
waiting spells settle for 10 seconds on successful refresh observations and emit once. Unknown does not advance a spell,
resolved non-waiting ends it, and removed sessions discard it. A discarded walk, failed cache write or withdrawn
connection contributes no evidence. Equal suffixes on different entries belong to their separate connection-local
policies. The raw projection precedes Unknown retention in the cache, including for identity-less hosts, so a cached
Waiting value cannot fabricate new evidence. Publication and enqueue are checked under the connection's status read lock
after the guarded publish, preventing a retarget during publication from alerting for the withdrawn connection.

The manager owns one bounded 32-item queue and one sender task, also shared with AppState. Refresh uses try-send only.
The sender reads the current dedicated settings and host display name; title and host characters that
`farhelm_proto::text::is_presentation_unsafe` names are stripped and each field is capped at 200 Unicode characters. A
five-attempt fixed 60-second fleet window includes failed sends, drops excess, logs once per capped window and carries
its dropped count into the next permitted alert. No timer sends a summary. Each HTTP request has a five-second timeout,
no redirects and no retries. Failure reasons are fixed words or an HTTP status; reqwest errors have their URL stripped
and their cause chains are never displayed. The ntfy response body is ignored. No diagnostic includes the topic, token
or title. Dropping the final sender handle aborts the owned task; it holds no handle back to itself or the manager.
Test-built helms default offline and inject a loopback client only for alert tests.

Protected GET/PUT `/api/ntfy-alerts` read/save an independent schema-47 singleton, not the queued shared preferences.
The JSON stored in helm.db includes the token; the response contains only `token_set`. Saves atomically preserve an
omitted/null token only for the same server origin (scheme, host and effective port, with default ports normalized),
clear it on an origin change, clear an empty token or replace a non-empty one. The comparison uses stored settings in
the same save operation, so stale forms cannot carry a saved token across origins. While a saved token would be cleared
by the entered server change, the form explains that before saving and does not offer to keep it by leaving the field
blank. URL/topic/token validation uses fixed refusals, including malformed JSON, so the usual decoder diagnostic cannot
echo secret input. Server URLs are http(s), at most 2048 bytes, without credentials, query or fragment; topics use
ntfy's 64-character ASCII letter/digit/underscore/dash grammar and may be empty only while disabled. Tokens contain at
most 4096 visible ASCII bytes. The form saves explicitly and shows its outcome; it never receives the stored token. POST
`/api/ntfy-alerts/test` posts with saved settings even while disabled, bypasses queue/policy/cap, and waits on one owned
send under the same timeout. Modal dismissal does not retract a save or test; component state is updated only while
mounted.

## Named credential catalog

Schema 75 adds the strict `credentials` table, keyed by the immutable user-chosen name. It stores the immutable kind,
the plaintext secret, creation/replacement timestamps and a positive signed-integer revision. Replacements increment the
revision even within one timestamp second; blank saves keep the existing row. Overflow refuses the replacement without
changing the value. Each creation also receives a hidden immutable UUID, distinct even when a deleted name is reused;
replacement keeps it. Delivery can use that identity alongside the revision without putting it in public metadata. These
values are separate from shared preferences and the Tensorlake account singleton.

Protected `GET/POST /api/credentials` and `PUT/DELETE /api/credentials/{name}` distinguish creation (409 on a collision)
from replacement (404 after deletion). Read queries select metadata alone. Save acknowledgements read that metadata
inside the same immediate transaction as the write, so another window's replacement cannot change the acknowledged
revision. JSON decoder and database diagnostics never enter responses or logs. Secret-bearing request types have no
Debug implementation. Tokens reuse the visible-ASCII 4096-byte bound; Codex accepts only a JSON object of at most 64
KiB, preserving vendor fields. The 512 KiB request cap permits JSON escaping of the bounded pasted file.

Credential metadata includes the names of recipes referring to it. The same reference query protects deletion inside its
immediate transaction. Both recipe create and update check existence and the required endpoint-token, Claude-login,
Codex-login or GitHub-token kind inside their own immediate write transactions; this excludes a competing delete even
across independent SQLite connections. Saving a recipe checks metadata alone. Delivery resolves values into a transient
`ResolvedEndpoint` with neither Debug nor Serialize; persisted `StoredEndpoint` contains only `base_url` and optional
`token_credential`.

Schema 77 adds nullable `hosts.tensorlake_credential_delivery`. The provisioning delivery component alone reads and
writes this nonsecret JSON: known absolute home and independent successful Claude/Codex endpoint fingerprints (URL,
credential name, revision and creation UUID), a separate last-successful Codex auth name/revision/creation UUID, plus a
GitHub absent/name/revision/creation-UUID stamp. Claude subscription settings use a name/revision/creation-UUID
fingerprint. Unknown slots retry. Explicit None for either agent and Codex Subscription use a distinct Cleared
fingerprint, so old endpoint-only Absent records cannot bypass their first cleanup. Clean or missing config files need
no write, but still commit Cleared. Each successful file's stamp is committed before the next file is attempted. The
revision and value come from the same credential read; no secret is cached in the record or runtime binding.

Recipes persist optional `claude_login_credential` and `codex_login_credential` names, each mutually exclusive with that
agent's optional endpoint. Neither present means None. The endpoint and login drafts stay separate in the editor; only
the selected mode is saved. Delivery resolves concrete slots in order: Claude settings, Codex config, Codex auth,
GitHub. Each name is resolved at its own slot so a later missing retained selection cannot withhold an earlier
successful replacement. Codex auth goes through the existing private stdin uploader into the ordinary
`.codex/auth.json`, only when the selected name/revision/creation UUID differs from the last successful auth write. Its
stamp commits before GitHub and is retained across Endpoint/None. Neither a reconnect nor a later-slot failure can cause
an unchanged seed to overwrite Codex's refreshed file. Delivery never reads auth.json, tests no remote ownership marker
and creates no separate agent home or retry task. Remote-success with a lost response or a process crash before the
local stamp remains ambiguous; this is not exactly-once installation.

Recipes persist optional `github_token_credential` names. The GitHub slot atomically installs 0600 bytes through stdin
at `github_token::host_file(remote_state_dir)`, the shared `github-token` basename, or unlinks only that file for None.
The supervisor's path-injected reader trims and rejects empty/NUL or unreadable values with fixed errors; NotFound keeps
the ordinary child environment. Host agent and tab paths inject after `host_child_env` into `ChildSpec.env`, before the
login shell, managed clone and launch shim. Restart reuses the agent seam. LaunchSpec, launch records, host
argv/environment never carry this value. Container environments use the same in-container host file. The background
spare-clone guard reads the same current file into only its child command's environment before its login shell; its
persisted `BuildSpec` stays secret-free. No protocol or terminal-host change is needed.

Without a previous successful GitHub token stamp, delivery runs the token-free
`gh auth setup-git --force --hostname github.com`; absence is a fixed warning after installing the file, and failure
leaves the slot unknown or at its previous successful stamp. Endpoint stamps committed earlier stay successful. A
successful GitHub stamp also means helper setup succeeded, so ordinary token replacements do not rerun it. None does not
edit Git configuration. There is no additional helper journal or credential cache.

Production backend and payload selection happen before manager startup. Provisioning and environment providers share one
delivery object and backend, including when explicitly injected. Setup records preparation's home and reconciles after
install coordinates are admitted but before releasing the actor hold. Legacy hosts recover their home with a bounded
read-only backend query, without rerunning user-manager preparation. A per-host mutex serializes setup, connect and
replacement. Its owned guard moves through each noncancellable `Db::call` write, so cancelling the caller cannot let
later work overtake a queued SQLite update. Writes require the same host entry identity and environment binding.

The real connect hook registers a counted lifetime lease before any await or credential read. Its returned resource
bundle retains that lease and the tunnel lease through Connected publication and serving; the actor's existing
Suspend/Remove cancellation path drops both before acknowledging parking. Replacement selects these active leases,
including hooks that have not published Connected, rather than filtering the manager's public phase. It uses the
connection's nonsecret selection snapshot; pending recipe edits wait for a later connection. A hook that has not yet
loaded a binding reads the committed replacement itself or reports its own connection-delivery failure; a skipped push
does not supersede that warning. Dropping the final lease cancels in-progress pushes before their next transport poll.
Host removal forgets retained runtime state.

A nonblank replacement uses the existing owned-handler task around commit and push, so an HTTP disconnect cannot abandon
delivery after saving. One absolute SSH timeout bounds all candidates, mutex waits and remote operations, with four
hosts processed concurrently. `SavedCredential` acknowledges the committed metadata plus per-host fixed failure reasons;
create still returns metadata directly. Runtime warning attempt ordering prevents an older timeout replacing a newer
result. Warning changes invalidate the host feed and project through host details, rail color and attention grouping
independently of connection phase. No timer or separate retry worker is added.

The Settings dialog owns one Tensorlake account context above both mounted panels. Key editors share only acknowledged
metadata, key/test discovery revisions and serialization of key requests. Their secret drafts and outcomes stay local.
Accepted API saves survive editor dismissal, with mounted fences preventing late responses from touching retired
signals. A native file selection fills the Codex draft and never submits it; a generation check discards reads
superseded by paste, agent selection or submission. The metadata-only catalog reader also populates endpoint pickers in
Settings and Add host, independently of whether Credentials is mounted. Failed refreshes keep the last good list. Feed
updates refresh an open credential editor's use metadata without replacing its secret draft.

## Disposable environments

Tensorlake and Podman implement one helm-owned environment-provider abstraction. Every environment runs an ordinary
supervisor, and session-owned rows reuse the host actor, session aggregation, provisioning, credentials and tunnel
lifecycle. Provider-specific work stays at the provider boundary. The supervisor has no provider-specific launch,
terminal or lifecycle mechanism. `crates/farhelm-helm/src/environment/mod.rs` maps the implementation responsibilities
for adding another provider, as the agent-kind module does for harnesses.

A shared provider-kind discriminator carries exhaustive static decisions consumed by the helm and UI. Place, binding,
recipe base and state, host kind and the shared recipe-state projection are also named exhaustive decision homes. Each
provider implements the I/O trait for places and cached probe reasons, two-stage preparation with progress, create,
status, suspend, resume, confirmed idempotent destruction, owned-resource discovery, Rebuild admission and recipe
cleanup. Its required connected hook acquires connection-scoped credential and tunnel leases. The provider registry has
an exhaustive selection method. Per-provider wording, marks and help live in one exhaustive UI module. Shared code must
not compare individual providers, match subsets or hide a new provider under a wildcard arm. A new provider requires an
implementation and a decision at every exhaustive match. Add static methods only when a caller consumes them.

A place carries its provider, either the Tensorlake account or an ordinary local/SSH host holding rootless Podman's
storage. A launch target carries recipe plus place as one choice. An environment binding records its chosen reserved
name, ownership and provider-specific handle. Tensorlake retains its existing sandbox/create facts. Podman records its
place host and canonical container identity, with the login user and uid needed for exec. Registration precedes lazy
preparation, so a pending binding may lack all login coordinates while it has no container ID or admitted install paths.
Preparation observes and atomically records the full login identity, prepared facts and pending-send marker before
container creation is sent. Exec requires the canonical ID and complete login identity. Ownership is independent of
session membership. User-owned environment hosts remain Tensorlake-only.

### Environment rows and provider state

Add nullable provider and place-host columns without rebuilding `hosts`. Keep the on-disk `tensorlake_sandbox` name as
the environment-name column, renaming it only in code. NULL provider means Tensorlake for existing rows. Decode kind,
environment name and provider together through one function, refusing unknown provider spellings. Podman keeps
`kind = 'ssh'` and a unique synthetic destination the ordinary SSH-host add path refuses. Nothing dials that string. The
internal host kind has a flat Podman variant next to Tensorlake, with exhaustive named predicates, including the
environment provider and whether a host can hold environments. Only Local and ordinary SSH may hold them.

The place-host foreign key has no cascade. Removing or retargeting a host is refused while any environment row refers to
it, including stopped resources and cleanup leftovers, or recipe state records artifacts there. The refusal names the
environments or recipes to clean up first, retaining access for confirmed cleanup. The existing credential SQL guard on
a non-null `tensorlake_sandbox` continues to admit environment delivery for both providers. Host views retain
session-owned rows in Hosts, update tracking and ordinary attention. Only permanent rail icons and ordinary
launch/template/agent host choices omit them. Destruction returns provider-confirmed cleanup evidence before the store
forgets a resource row. A missing resource counts as destroyed, and Tensorlake also removes its snapshots.

Replace the Tensorlake-only status source with the provider trait while retaining its scripted fake for actor tests.
Status returns a successful environment status separately from an unanswered lookup, represented as
`Result<EnvironmentStatus, StatusUnanswered>`. Status distinguishes Running, reason-bearing Changing, Suspended (noticed
time and resume-failure detail), Stopped, Missing and an unknown provider state with its reason. Tensorlake's Suspended,
Suspending and resume-failed states retain their existing Suspended projection. Missing includes a terminated sandbox.
An unknown vendor state is not usable and must not dial. A consumed exhaustive provider-kind method chooses what an
unanswered lookup does, `DialAnyway` for Tensorlake and `RetryLater` for Podman. Tensorlake attaches the API error only
if that dial fails. This preserves its outage behavior without treating a successful unknown state as an API failure.
Podman Stopped triggers one automatic start while the place host is connected. Failure publishes its reason and an
explicit Resume remedy, without a timed start retry. Suspended never auto-resumes.

### One route for transport and provisioning

Local IPC, SSH and Podman exec carry the same framed supervisor protocol. Both transport and provisioning resolve
through one host-access representation. Its remote route is SSH destination/access or Podman exec through a local or SSH
base route. Provisioning's remote target retains the existing serialized SSH-case spelling in confirmation plans. Host
routes are resolved from the store on each attempt, joining the current place row. Builders and preparers obtain their
route from the provider, since those machines have no registered host row. Credential delivery, installation, builder
shells and preparation all use the same remote-command constructor.

The constructor builds argv layers with quoting once per parsing shell. Podman exec uses `-i`, the recorded login user,
home working directory, explicit HOME/USER/LOGNAME and `XDG_RUNTIME_DIR=/run/user/<uid>`, then `sh -c` for the script. A
remote base wraps that invocation in the ordinary SSH command. The protocol's innermost command is
`farhelm internal stdio`. Do not paste Podman argv into the configured remote Farhelm program. The child-process
connection owns stderr drainage, the connected marker and exit diagnostics for SSH and local/remote Podman exec. Local
Podman therefore uses a process pipe even though the ordinary local host uses its Unix socket directly.

All environment stdio routes pass `--stdin-idle-limit-secs 120` unconditionally and emit the existing 30-second
heartbeat. The supported environment helper has that option. Killing a Podman exec client does not reliably end its
in-container process, so the helper's own stdin watchdog supplies the bound. Ordinary SSH hosts retain no input-idle
deadline. Releasing a route's shared connections must never close the container host's masters on behalf of a nested
environment. Exec channels ride the place host's supervisor master, preserving logins that need interactive second
factor. sshd's default `MaxSessions` of ten shares that connection among the ordinary supervisor and environment
channels. After a full master refuses a channel, OpenSSH can open another noninteractive login. Hosts that require
interactive approval for each login therefore allow about ten concurrent container connections, with actual capacity
depending on their configuration and other channels. When the fallback login fails, launch refusal or connection
unavailability names the container host's SSH session limit. No supervisor relays another's protocol.

### Shared recipes and prepared artifacts

Generalize the existing recipe catalog, preserving its separate definition and build-state writers. One recipe base
supports providers through an exhaustive decision. Managed Ubuntu supports both, Tensorlake image and snapshot bases
support Tensorlake, and container image reference plus login user supports Podman. Keep the stored spellings
`tensorlake_ubuntu`, `image` and `snapshot` for existing bases. Size stays Tensorlake-only. Recipe templates, Clone,
quick launch and recent setups carry a recipe/place target separately from installed-host identity, never both. Missing
recipes or place hosts are inapplicable. Ordinary creation cannot target a session-owned row. Save as template from an
environment launch and bulk lifecycle conveniences remain deferred.

Reusable environment history is a bounded projection admitted in the existing successful-create transaction from
captured request intent. It retains recipe/place, explicit structured selection and accepted repository-or-home intent
without a disposable-host foreign key. A shared admission order compares different supervisors, whose clocks and
creation sequences are independent. Replayed creates cannot change the captured intent or increase its frequency.

Prepared artifacts belong to (recipe, place). Tensorlake retains built and prepared snapshots, while Podman stores built
and prepared images in the place account's Podman storage. Keep per-place state in the existing `state_json`, with
Podman entries keyed by place-host id. Recheck Podman artifacts with `podman image exists` before trusting records,
since a host may have been reinstalled or its storage removed. Preparation becomes stale after strictly more than 86,400
seconds, a different Farhelm version or a rebuilt source. Concurrent requests for the same recipe/place join its build
or preparation. A Podman session launch builds a missing software image. Tensorlake session launch retains its
built-snapshot prerequisite because builds bill. The Tensorlake build and preparation details below remain applicable to
that provider.

Rebuild is a required provider operation that returns after admission or joining. It reads the current definition. A
Podman Rebuild refuses as busy when a preparation-only job holds that recipe/place, so joining cannot claim a rebuild
that never ran. Readiness is a synchronous projection over shared recipe state and the one build registry, with the same
freshness rule for both providers and an exhaustive place decision.

Recipe deletion first takes the shared registry's deletion admission, excluding new builds, preparations and source
selection for every provider. Each provider cleans all its recorded and live places under that guard. Cleanup waits for
active work and requests cancellation only between commands, preserving recorded ownership of partial resources. Every
installed provider is attempted and failures retain their records. The final store transaction rechecks recipe use and
aggregate cleanup state before deleting the catalog row. Both engines share the same registry instance. Tensorlake
cleanup checks only Tensorlake state before requiring its key.

Containers leftovers use a provider-owned operation. It rechecks canonical identity, this helm's ownership labels and
the absence of row, recipe or active-job references before removal. Row-backed leftovers use the ordinary confirmed
whole-host Delete path with its actor hold and provisioning lock. Image removal uses non-forcing Podman removal after
checking ownership labels, leaving an in-use image recorded for later cleanup. User-owned base images remain separate
from Farhelm's committed layers.

Settings → Recipes owns the shared catalog/editor and per-place readiness and Rebuild. Settings → Tensorlake retains
account/key/create-confirmation settings and sandbox leftovers. Settings → Containers exposes cached probe results and
container leftovers. The launcher and template editor share Existing host, New container and New sandbox placement, with
a recipe/place pair for either environment segment. Provider UI differences remain exhaustive decisions in one module.
Launch progress and errors reuse the existing operation surfaces, with no placeholder session.

### Podman places, creation and images

Probe only local and ordinary SSH hosts through their base route. Cache timestamped Linux, architecture, absolute Podman
path and the results of `podman info` and a working transient user scope. Require Podman 4.9.3 or later, rootless,
cgroup v2 and `cgroupManager: systemd`. Refresh in the background when the launcher opens, keeping unusable hosts with
their reasons. No synchronous probe fan-out gates opening the launcher.

Lifecycle commands run on the place host. Wrap `podman run` and `podman start` in `systemd-run --user --scope --collect`
with a reserved environment name and a per-attempt nonce. This keeps conmon and the container's network helper outside
the helm's cgroup and a remote SSH login's lifetime. Resolve Podman to an absolute path and strip all `FARHELM_*`
variables and `TMUX`, `TMUX_PANE` and `TMUX_TMPDIR` from its environment before it can initialize shared Podman
processes. Session and helper container names and all owned image tags carry the helm tag. Labels include
`farhelm.helm=<tag>` and `farhelm.role=session|build|prepare`. Podman storage is per Unix account, so two helms or test
runs must never reuse each other's builder, preparer or image names.

Managed Ubuntu bootstraps a digest-pinned fully qualified Ubuntu image in a non-systemd container, installing systemd,
sudo, curl, git and bash, creating its login user with passwordless sudo and enabling linger before committing. The
shared readiness checks then require systemd running or degraded, apt and the build tools. Every systemd container run
passes `--systemd=always`, since committing does not preserve that run flag. Preparation uses the common installer and
login-manager/linger setup. It stops the enabled supervisor, removes and verifies absence of its entire state directory,
and clears `/etc/machine-id`. Stop the preparer cleanly before `podman commit`. A clone starts with no supervisor
identity and learns it through the ordinary hello. A custom image supplies its reference and login user and must meet
the same readiness contract. The concrete managed Ubuntu digest and login user are implementation choices.

Read proxy variables from `systemctl --user show-environment` on the place host during its probe. Use
`--http-proxy=false` and write the selected variables into the container user's `environment.d` at creation. PID 1's
container environment alone does not reliably reach its user manager. This makes each session's proxy inheritance
deterministic and independent of the helm's or SSH command's incidental environment.

After successful build/preparation replacement, remove a superseded owned image once no container uses it. Rebuild
cleans the place's replaced images, and recipe deletion cleans owned images at every place before forgetting their
records. Never delete a user's base image. Reject Farhelm-managed checkpoint images as recipe bases. Discover leftovers
with Podman's label-filtered listing at usable cached places and every place referenced by an environment row. Failed
explicit cleanup keeps its resource and reason visible for retry. No startup cleanup worker is introduced.

### Environment ownership and refusal

Use the landed sandbox-session contract for both providers. Record ownership and a reserved resource name before
creation. User Delete of a session goes to the environment's supervisor and removes the resource only with its last
session ("Environment Delete and the seal" below); the warned whole-environment Delete destroys the whole resource,
including unlisted sessions and files, without membership proof. Replace stays inside it and uses ordinary supervisor
session deletion for its source, preserving the resource and existing files on every failed or ambiguous outcome. A
failed normal initial launch may destroy only its new environment. Retain failed cleanup and interrupted resources for
explicit deletion. The supervisor's seal and the one request that sets it are the only session-finality state and the
only protocol change for these cases: add no helm-side record of which sessions an environment holds, no membership RPC,
no Resume to count sessions, no connection handshake change, no transparent recovery of failed Replaces and no startup
continuation protocol.

### Environment Delete and the seal

A user's Delete of a session in an environment is `ControlMsg::DeleteSessionOrSeal` (protocol 76), sent only by the
helm's full-authority connection; a session-authenticated one is refused like a create. The supervisor handles it like
`DeleteSession` (agent fence, so the session's in-flight agent requests finish first; admission; directory admission;
lifecycle claim), with each wait before the decision bounded so that a busy environment gets a definite "nothing was
deleted" `Conflict` rather than a decision taken after the helm gave up. A supervisor already sealed answers `Sealed` to
any such request, whether or not the session still exists; an unsealed one that does not find the session answers
`NotFound`, which the helm reads as an earlier attempt having deleted only that session. Otherwise teardown runs its
read-only preflight, then one store transaction counts the other rows in `sessions` (any state: a `Launching` row is
written before its terminal starts, and a Restart keeps its row while its entry is off the in-memory map; a pending
reservation with no row is not counted) and, when there are none, sets `supervisor_meta.sealed`. A failure to write the
seal is an error before anything destructive. The in-memory seal is set after the commit, and the supervisor hints a
listing change. Counting under directory admission means a create, which holds it until its launch settles, is either
counted or refused. A non-final Delete then finishes the ordinary teardown, except that the checkout's last reference is
discarded (renamed into the trash folder for immediate removal and never listed) instead of archived, and answers
`Deleted { remaining }`; a checkout another session still uses stays, as it does for any Delete. A final Delete skips
the archive move, finishes the teardown and answers `Sealed`, with any later teardown failure as its notice: the
environment's destruction ends what remains, and the sealed supervisor refuses to restart it. Teardown is split at its
preflight boundary so the seal sits between the two halves without the shared teardown knowing about it.

Schema 35 stores the seal (below). It is loaded at startup and checked with `ErrorKind::Sealed`, which on its own says
nothing was created (a sealed refusal records nothing, so `create_unaccepted`, which needs a durable refusal record,
stays false), at the top of `create_session_admitted`, after reservation resolution, which every create passes through:
the helm's own, Replace's replacement, agent spawns and creates, a keyed retry that would launch, and managed-checkout
recovery, whose inspect-only path offers no recovery on a sealed supervisor; at Restart and Restart with after their
lifecycle claim; and at a tab open after its lifecycle claim. A tab open takes no directory admission, which is still
sound: a seal happens only when no other row exists, and a tab of the deleted session is serialized by its lifecycle
claim, which the seal holds. A replay that only answers an existing session still answers. The seal is never cleared.
`SessionList.sealed` carries it on every listing, since the hello is sent once per connection.

On the helm, the new path is opted into by a query only new pages send; a plain DELETE of a session on a session-owned
host is still refused, so an older page cannot reach it, and `delete_sandbox=true` keeps meaning the whole-environment
Delete. The helm denies the session's pending approval cards first (an unanswered spawn card would otherwise hold the
agent fence), queues behind any other environment session Delete of the same host, then takes the host's provisioning
lock without waiting (a setup, update, suspend or whole-environment removal answers "try again"), re-reads the row and
requires it session-owned and machine-deleting, and sends the request only while the host is connected, with a reply
deadline derived from the supervisor's wait bound and teardown's own bounds. Not connected, not sent, sent without an
answer, or past the deadline: the helm answers with a marker that nothing was decided, and the page offers the warned
whole-environment Delete. `Deleted` runs the ordinary local forgetting and reports the supervisor's count; `Sealed`
destroys the environment through the same removal path as host removal, under the lock already held.

Whenever a connection publishes a listing with `sealed` set for a session-owned, machine-deleting host, a helm-owned
finisher destroys it through that same function, at most one at a time per host. A sealed listing from any other host is
shown as a problem (a failed refresh naming the seal) and never acted on. Every machine-deleting removal shows the host
as `HostState::Deleting` through the connection hold, with the provider's wording from the UI. A sealed host is also
kept in an in-memory seal record, pending or with its last destruction's failure, and the manager's status reads report
every host in that record as `HostState::Deleting` with that failure and no client, whatever its connection does
meanwhile (held, released, reconnected, retired), so the failure and its Retry cannot be lost and nothing is routed to a
supervisor that takes no new sessions. Retry runs the destruction again only for a host whose record holds a failure.
Nothing about it is stored on the helm: after a restart the seal arrives with the next listing. While a host is
`Deleting`, its sessions show as being deleted with their actions unavailable. Replace counts `NotFound` from its source
removal as success and turns `ErrorKind::Sealed` on its create into "the environment is being deleted".

Suspend closes the exec connection before Podman pause, and Resume unpauses a Suspended container or starts a Stopped
one. Stopped auto-start is attempted once after observation with a connected place, then failures wait for explicit
Resume. A successful start returns the ordinary supervisor's classification, with no agent respawn. Its existing
kernel-boot comparison interrupts sessions last known live across a host reboot and preserves known exits. Same-host
container stop/start uses ordinary exit reconciliation. Plain Stop and Stop and suspend retain their ordinary
chosen-session semantics followed, for the latter, by provider suspension.

### Agent requests and environments

Everything SPEC.md's "Agents and environments" asks of the helm lives in `agent_requests.rs`, keyed on one fact: whether
the origin's host row is session-owned, read from the same manager status the origin-liveness check uses, after the
existing prompts-off and liveness gates. That makes the origin _confined_, and each verb arm applies the confinement
where it resolves its host or target, before any card: listings filter to the origin host, lifecycle verbs compare the
resolved target's host with the origin's, create resolves an absent or own-named host to the origin and refuses any
other host or any template or flag that sets one or a recipe target, and clone, the template verbs and the host verbs
refuse outright. The check names no provider: a confined origin is any session-owned row, so a third provider inherits
the rule without a branch. The refusal is an ordinary `Unauthorized` agent error with one fixed message; no
notification, supervisor metadata or acknowledgement is involved, and the ordinary relay still fails closed without an
owning helm.

A confined Delete of another session in the same environment goes to that host's supervisor as a session-only delete,
the same supervisor operation Replace uses for its source, which never seals. It does not pass through the user's
environment Delete (`sessions::sandbox::delete_if_owned`), which only an explicit user confirmation reaches. From
elsewhere, Delete of a session-owned target is still refused, and session-owned rows stay out of `resolve_host` and
template host identities.

An environment launch for an agent is `AgentVerb::CreateEnvironment`, naming a recipe and, for a container, the
container host's display name. It is a verb of its own (protocol 75) rather than a field on `AgentVerb::Create`, so a
helm that predates it fails to decode the request instead of ignoring the placement and creating an ordinary session. A
plain create with no `--host` whose named template targets a recipe takes the same path. Clone of a session-owned source
builds the same plan from the source's row (recipe and place) and its launch and repository intent, read live from its
owning supervisor as ordinary clone reads them, since a cached launch may no longer be the session's; a suspended or
unreachable source is refused until it is resumed. Before asking, the helm checks the place's cached availability and
the launcher's own environment-launch validation, so the user is never asked to approve a launch that is then refused
before anything starts. It asks with `ApprovalAction::LaunchEnvironment`, which `requires_direct_allow` and which an
older UI fails to decode with the rest of its approval list. Its Allow stands in for the launcher's creation
confirmation: the helm calls `ProvisioningService::start_session_environment` directly, which is what the
confirmation-checking REST route calls after its own check. The agent YOLO rule needs no host row here: a new
environment row is registered with `yolo_without_asking` on (Topology), which vouches for every launch, so the rule is
satisfied by construction, and the direct-Allow card already covers an agent-supplied wrapper.

The answer waits for that task's result. The supervisor relay's answer budget starts when it forwards the request and
knows nothing of the approval, and only the helm knows whether a create's template or a clone's source leads to a new
environment, so `AgentVerb::may_launch_environment` (the new verb, create and clone) adds `ENVIRONMENT_LAUNCH_WAIT`
(thirty minutes) to the ordinary budget plus the approval wait, in one span. The helm bounds its own wait for the task
to the same allowance after the Allow and then answers `Timeout`, saying the launch carries on, so the agent normally
hears that before its relay gives up. Neither expiry cancels the task, which is helm-owned. Keyed environment launches
are refused before anything else: the key would have to be held by a supervisor that does not exist until the launch has
run.

Keep local help, conversation reporting and ordinary disconnected-helm relay behavior. No supervisor restriction
metadata, acknowledgement, membership or notification bookkeeping is added.

## Tensorlake account

`crates/farhelm-helm/src/tensorlake/` holds the account side of Tensorlake sandbox hosts. Protected GET/PUT
`/api/tensorlake` read and save an independent schema-70 singleton, `tensorlake_account`, whose JSON holds the API key
and the create-without-asking choice; like ntfy's row it is outside the queued shared preferences. Replies carry
`api_key_set`, never the key. A save names only the fields its control owns: an absent or blank `api_key` keeps the
stored key, a non-blank one is trimmed and replaces it (visible ASCII, at most 4096 bytes), `clear_api_key` removes it,
asking for both is refused, and an absent `create_without_asking` keeps the stored choice. The save is resolved against
the stored row inside one database operation, so two windows saving different fields cannot undo each other. Refusals,
malformed JSON included, use fixed messages. POST `/api/tensorlake/test` lists every page of sandboxes with the stored
key and answers the count, a 400 without a key, or a 502 with the classified failure.

The REST client is a `TensorlakeApi` trait with one implementation per caller: reqwest against
`https://api.tensorlake.ai` in production, the same client pointed at a loopback stub in unit tests, an offline
implementation for test-built helms, and a filesystem-controlled fake for unit tests. Every call takes the key
explicitly; `ApiKey` has no `Debug` or `Display`. The trait carries sandbox listing, lookup, creation from an image or
snapshot, suspend, resume and termination, plus taking, looking up, listing and deleting snapshots. Requests follow no
redirects and are not retried; a call has a 30-second bound and a create ten minutes. A create that times out may still
be waiting for capacity on Tensorlake's side and start later, so a timeout does not mean "not created": callers
reconcile by the sandbox name they chose. The create request always states `auto_resume`, because Tensorlake's default
lets proxied traffic, a gateway ssh connection included, wake a suspended sandbox; with it off such traffic gets a
prompt 409 instead, which keeps resuming the user's decision. Path arguments are single percent-encoded segments.
Replies decode leniently: unknown fields are ignored, most fields are optional, and an unknown status decodes as
`Unknown` (so an unknown snapshot status cannot tell "still working" from a new kind of failure, and pollers bound their
wait). Failures are classified as auth (401, 403), not found (404), conflict (409), refused (400, 422), other status,
transport and undecodable reply. Server messages are kept for display, bounded, whitespace-collapsed, stripped of
presentation-unsafe characters, and have the key replaced should a server echo it.

The helm tag is the first eight bytes of SHA-256 over a fixed domain prefix and the registry namespace, written as four
base-36 digits; a unit test pins its value for a fixed namespace, because changing the derivation would rename every
helm's sandboxes. Farhelm's SSH key lives in `<state>/tensorlake/ssh-key/` (`id_ed25519` and `id_ed25519.pub`).
`GET /api/tensorlake` only reports it (ready, not created, or unreadable); Settings mounts every section when it opens,
so a read that generated the key would leave one on every helm. POST `/api/tensorlake/ssh-key`, the section's **Create
SSH key**, runs `ssh-keygen -q -t ed25519 -N "" -C farhelm-<tag>` without a shell into a fresh temporary directory
beside it, sets the private key to 0600, and renames the directory into place, so a crash leaves either no key or a
whole one; leftover temporary directories are removed at the next generation. A key directory without a readable pair is
reported, never regenerated, and a create keeps an existing key. A key that cannot be read is a state in the settings
reply, not a failed read; a failed create answers its reason. The registration command carries the public key line,
quoted as one POSIX shell word, rather than the file's path, which would only mean something on the helm's machine. A
production helm whose HTTPS client cannot be built (no CA certificates) answers every Tensorlake call with a reason that
says so.

## Adding Tensorlake hosts

`POST /api/hosts/probe` accepts `tensorlake` (name and recipe) and `tensorlake-host` (an existing host id for rerun).
These targets refuse remote path overrides and `new_entry`. A probe checks the saved key, recipe and alias locally,
ensures Farhelm's SSH key exists, and freezes the recipe definition, size and generated sandbox name in a bounded,
one-use outer plan. It makes no Tensorlake API call. `tensorlake-provisionable` returns the confirmation and the same
step descriptions later used for progress. Confirming through the existing provision route writes the host row before
starting billed work, then uses the ordinary host provisioning lock and fleet-wide run limit. A registry handoff failure
rolls the new row back with `SandboxDeleted::never_created`; no create has been sent at that boundary.

Ordinary setup recovery is an explicit exception to provisioning's frozen-inner-plan rule. A sandbox does not exist at
confirmation, so its home, architecture and install paths cannot yet be inspected. The confirmation authorizes creation
and ordinary Farhelm setup on that sandbox; the run computes the inner plan after preparation and replaces its final
placeholder with those concrete steps, without a second question. The existing executor runs those actions with a
progress offset.

All recipes call the snapshot builder's single-flight `ensure_snapshot`, which reports its watch progress and joins an
existing build. A ready snapshot is reused (base and package edits still require explicit Rebuild). A build started by
the add uses the confirmed definition. A snapshot whose recorded disk is larger than the confirmed host disk is refused.
Setup skips the build when the host's sandbox is visible, when its ID is recorded (a recorded, deleted sandbox is never
replaced), or while a pending create is inside the settle window. Only a settled absence with no recorded ID builds. The
rerun probe describes these steps from local records only. This keeps reruns of hosts created from old reuse recipes
from starting a newly required build. Host creation adopts an existing sandbox by the pre-chosen name only when this row
records an outstanding create, including after a lost create response; a saved sandbox ID is looked up directly. Without
either record, a name collision refuses setup before preparation or credential delivery. The refusal names the sandbox
and tells the user to delete this host entry first; that removal leaves the existing sandbox alone, and it then appears
under Sandboxes not on this helm. Host rows reserve their names until removed. Creation records the sandbox id before
preparation and never replaces a recorded, deleted sandbox with a new one. Creates set `timeout_secs: 0`,
`auto_resume: false` and `max_pending_secs: 900`.

Schema 73 adds `hosts.tensorlake_pending_create_at` (unix seconds). Before sending create, the run records this
outstanding request durably. An observed sandbox id settles it in the same write that saves the id; a definite API
refusal clears it. After an ambiguous failure or restart, setup and deletion reconcile by the pre-chosen name. A 404
settles absence only after the snapshot builder's shared 15-minute create settle window, and before then both refuse
without replacing the outstanding request or forgetting its ownership row. Deletion saves an observed id before
terminating, so snapshot cleanup remains retryable even once the name stops resolving. A row with neither a saved ID nor
an outstanding create is removed without looking up its name or deleting any sandbox or snapshots. The API-key
requirement still applies. A legacy row or a power loss that lost both records cannot recover by name: remove that host
entry, then handle the leftover explicitly.

For a genuinely new host, the run ensures a prepared snapshot after ensuring the built snapshot, then selects it again
under the recipe's source-use exclusion before creating the host. It omits ordinary inspection and installation on the
clone. Schema 79 adds nullable `hosts.tensorlake_prepared_install_json`, read on demand rather than included in the
ordinary host view. The exact selected home, binary path, state directory and installed version are recorded atomically
with the pending-create timestamp before sending create. They do not populate the admitted coordinates, so an actor
cannot dial a name collision or an unfinished clone after restart. Recording the sandbox ID retains these facts;
admission, once the sandbox is Running and answers over SSH, records both coordinates and clears them. A definite
refusal clears them with the pending marker. An interrupted create recovers its own saved facts, not the recipe's latest
prepared record. Later Rerun and Update retain ordinary installation, and the supervisor's hello remains the authority
for observed host version and identity. The API's Running state can precede gateway readiness. Setup waits for SSH with
the ordinary bounded transport retry, without starting logind or modifying the clone, before admitting paths or
delivering credentials. This wait belongs to the admission portion of the debug spawn timings. Credentials are delivered
with the saved home before the actor's hold is released, and the ordinary fresh-window attachment wait establishes the
connection. A pending ordinary create that appears between lookups retains ordinary inspection/install recovery rather
than assuming it used the newly selected prepared snapshot.

The free probe omits **Prepare snapshot** when the local prepared record is fresh. Execution rechecks and inserts that
step if preparation became necessary, retaining completed common steps. Its first-time/24-hour/update explanation is
part of `PlanStep`, copied into plain confirmation and live progress; both expanded and folded views render it as peer
text, and its displayed geometry participates in movement equality. Only the `prepare-snapshot` waiting explanation
enters live progress; ordinary setup, Update and Uninstall explanations remain confirmation detail. No new approval is
needed when freshness changes.

The run holds the host connection during preparation and while it records install coordinates. Ordinary preparation
waits for SSH, fails promptly with the key-registration hint on public-key refusal, checks PID 1, starts logind with
passwordless sudo and enables linger for the actual login user, then waits for a usable user manager. Every host's
provisioning SSH scripts also supply `XDG_RUNTIME_DIR=/run/user/<uid>` when unset and that directory exists; local
scripts are unchanged. Before a first install, an existing `supervisor.db` at the planned state directory refuses setup,
so copied identities cannot silently become Duplicate. Reruns with recorded install paths retain their own state. This
deliberately refuses any pre-existing supervisor database, including one whose identity has not yet been minted; there
is no identity reset of captured state. Prepared snapshots satisfy the rule by carrying no state at all. Connection
actors refuse to dial a Tensorlake row until both install coordinates are present. Initial setup records them only after
that copied-state check, so the gate survives a restart; an initial failure keeps the explicit hold too. Ordinary SSH
discovery and Update refuse such unfinished rows. The store also checks this inside the ordinary SSH convergence
transaction, before writing coordinates or identity, so a probe or plan started before the sandbox row existed cannot
adopt copied state later. The Tensorlake executor instead uses an explicit admission write after its copied-state check;
that write records both coordinates together and imports no discovered identity.

Agent settings are read with a 1 MiB bound, merged on the helm, and streamed through backend stdin into mode-0600 atomic
installs. Claude's JSON and any existing `env` must be objects. Farhelm manages `ANTHROPIC_BASE_URL`,
`ANTHROPIC_AUTH_TOKEN` and `CLAUDE_CODE_OAUTH_TOKEN`. Endpoint mode removes the OAuth token, Subscription removes the
endpoint keys and writes the selected setup-token, and None removes all three. Missing or already clean settings need no
write. No onboarding or first-run metadata is initialized, so Claude's own setup can appear once on a fresh host.
`toml_edit` preserves Codex's root settings and other providers while replacing its entire `model_providers.farhelm`.
Codex None and Subscription remove only a `farhelm` model_provider selection and that table's experimental_bearer_token,
retaining its table, unrelated keys and comments. Setup always includes credential configuration because cleanup can do
work even without selected credentials; the copied-state check and install-coordinate admission still precede it.
Parse/read failures omit contents and parser details so tokens cannot enter progress. The shared delivery component
writes at Add, connection and credential replacement; Update does not independently reapply settings. Failures become
warnings and do not abort supervisor installation or connection. The shared SSH uploader sets `umask 077` before
creating its temporary, so even an existing mode-0755 agent directory cannot expose tokens during upload or after
interruption. Its finishing command applies the requested final mode. Before replacing agent settings it compares the
destination with the bytes read (or expected absence); a mismatch re-reads and merges once, then warns without recording
delivery if it conflicts again, leaving the latest file for the next connection's retry.

This credential cleanup is best effort beyond SPEC.md's undefined result for credentials already present when Farhelm
supplies an agent's credentials. It does not establish support for mixing credential owners. In Codex API endpoint mode,
the entire Farhelm provider table is emptied before its keys are written, dropping `env_key`, header settings and other
leftovers. Comments outside that table remain intact. For Claude API endpoint and Subscription modes, it also removes
`ANTHROPIC_CUSTOM_HEADERS` and `ANTHROPIC_API_KEY` from the settings environment and the root `apiKeyHelper`. Other
settings remain intact. A gateway needing extra headers belongs under None, with the image providing its complete
credentials; None's existing three-key cleanup is unchanged.

The add dialog embeds the full recipe editor with deletion disabled and a `DialogCloseGuard` for Escape and Close. It
selects the editor outside its own body wrapper: the editor supplies a body and `DialogFooter` as direct siblings of the
existing header, which is the frame's pinned-footer contract. An explicit presentation prop selects that layout;
deletion permission does not infer the embedding context. The editor still owns the draft, requests and close guard. Its
Add host mount focuses Name; its Settings mount aligns the editor's top, including Back, to the existing body's top and
focuses its heading without a second native scroll. Deferred focus captures the mounted editor and never targets a later
replacement; both focus and scroll yield when a usable editor field or modal control already owns focus, so opening
cannot redirect typing that began before its animation frame. Settings keeps normal, leave and delete answers and their
refusal messages in one opaque bottom-sticky wrapper inside the editor, so hiding the Tensorlake section hides every
answer while the global Close footer stays unchanged. It uses Tensorlake's acknowledged `create_without_asking` setting,
never the general setup preference, and refreshes the host list after an accepted or ambiguous submission. An inline
Settings entry point opens its Tensorlake section. Rerun retains the Tensorlake plan kind, consults only that
acknowledged setting, and offers no answer that writes the general SSH setup preference. The add dialog binds
confirmation to a `ConfirmSlot` opening; accepted close retires it synchronously through the recipe close guard, before
notifying its parent. Automatic submission consumes only the opening it just created, so a cancelled or replaced opening
cannot authorize a later billed POST.

The add form reuses the recipe list's summary helper and associates separate Name and Recipe help with their controls,
keeping both accessible names exact. A displayed creation offer renders `SetupPlanConfirmation` directly as the existing
frame's header, body and footer. Its optional summary override carries the confirmed size; its optional Back callback
retires only the captured current `ConfirmToken`, keeps the fields and returns focus to Name. Continue must probe again.
The existing dialog focus facility hands off to Create when an offer appears inside the mounted modal; ordinary field
and busy renders do not repeat that handoff. Creation authority remains in `ConfirmSlot` and the existing operation
claim. The shared Tensorlake mode combines billing, pricing and the idle limit in one callout for initial add and rerun;
SSH callers retain their default subtitle and omit Back. Copy plan remains in the body with the helm's exact raw text.

## Tensorlake recipes

The shared catalog extends the existing recipe storage and validation described here. Existing table names and stored
base spellings remain compatible. The provider and place determine which artifact is built, as specified under
Disposable environments. Tensorlake-specific build operations below keep their own billing and ownership rules.

Protected recipe routes store recipes in the schema-71 `tensorlake_recipes` table, one row per name, with the user's
definition in `spec_json` and the helm's snapshot-build bookkeeping in `state_json`. The two are separate columns
because they have separate writers: a save replaces only `spec_json`, and builds write only `state_json`, so a recipe
edit racing a build cannot drop a recorded snapshot or builder ID, each of which is the only record of a billed
Tensorlake resource. Changing a base or packages keeps the current snapshot and its ownership record; only a successful
build replaces it and moves it to the retired list. This is why recipes are not launch templates' last-write-wins "PUT
creates or replaces": `POST` creates and answers 409 for an existing name, `PUT /{name}` updates and answers 404 for a
missing one, and no route renames. `DELETE /{name}` checks host use inside its own transaction, counting the hosts rows
whose `tensorlake_recipe` names the recipe. The store also refuses the delete while `state_json` records a snapshot, a
builder, a retired snapshot or a build start, so no path forgets a billed resource; the route cleans those up first (see
below).

The wire types and validation live in `farhelm_proto::tensorlake::recipes`, shared with the UI, which runs the same
checks before sending. `RecipeBase` is tagged by `kind`: `tensorlake_ubuntu`, `image` with `image`, or `snapshot` with
`snapshot_id`, extended with a container-image reference and login user. The managed Ubuntu base retains its existing
spelling and supports both providers. Save and view payloads carry this `base` separately from `packages`. The snapshot
view records the base and packages actually built, so staleness compares both. Schema 78 converts old package sources to
the Tensorlake Ubuntu base with their packages and current snapshot preserved. It leaves `state_json` byte-for-byte
unchanged; an old built snapshot without `base` reads as Tensorlake Ubuntu, the only base old builds used. Old image and
snapshot sources become the corresponding base with empty packages and no built snapshot. Their existing hosts are
unchanged. The old source payload shape is not accepted by the recipe API after migration.

The name grammar is narrower than a template's because the name is embedded verbatim in the snapshot builder's sandbox
name (`farhelm-build-<helm tag>-<recipe>`), so it must already be a valid part of a Tensorlake name, and mapping it
without sanitizing keeps two recipes from sharing a builder. Package words must start with a lowercase letter or digit:
the build shell-quotes them, but apt-get still reads a quoted `-o...` word as an option. Tunnel hosts are DNS names,
IPv4 literals or bracketed IPv6 literals, the shape an ssh `-R` forward carries without quoting. Endpoint base URLs are
http(s) without credentials, query or fragment. Endpoints name an optional endpoint-token credential; None is an
explicit selection, and a URL change retains the named choice. Schema 76 removes the old `claude.token` and
`codex.token` JSON fields without importing values or changing `state_json`, so users re-enter their endpoint tokens in
Credentials. Request decoding rejects the old token fields. A request body that does not decode is refused with a fixed
message, because serde's diagnostic can quote submitted text, and stored rows that do not decode fail the listing
without quoting their content. Successful writes bump the invalidation feed, and the Settings list re-reads on each
notice. Write bodies are capped at 64 KiB.

## Tensorlake snapshot builds

`crates/farhelm-helm/src/tensorlake/build.rs` builds every recipe's snapshot;
`POST /api/tensorlake/recipes/{name}/build` starts a build or joins the running one and answers 202 with the recipe's
view (400 for a missing API key or a missing ssh key; 409 while the recipe is being deleted). `RecipeBuilds` is an
in-memory registry per helm: at most one build per recipe, each a task the registry owns rather than the request,
publishing its step through a `watch` channel that the recipes GET reads and bumping the feed at every step. The GET
reports `snapshot`, `build` (idle, building with its step, failed with step and message, or interrupted) and
`cleanup_pending`. Interrupted is derived: `build_started_at` persisted with no live task, since nothing reconciles at
helm start.

`state_json` (`RecipeState`) is the record of everything the build bills for. `build_started_at` is written first.
`pending_create_at` is written before the create request goes out and cleared in the same write that records the
builder's ID, which happens right after create returns: once terminated, a sandbox no longer answers to its name, and
its snapshots are attributable only by its source ID. The create and that write are not cancellable, so a delete waits
for them. A create answer that is not a refusal (a transport failure, a reply that does not decode, a server error)
leaves the outcome open: the builder is looked up by name and adopted if found, and otherwise the pending record stays,
counts as awaiting cleanup and blocks the store from deleting the row. Reconciliation, shared by the next build's
cleaning step and the recipe's deletion, first sweeps a recorded builder by its ID. Only an outstanding pending create
then permits looking up the builder's name and recording a sandbox found there before terminating it. A saved ID
authorizes only that sandbox; neither an interrupted build start nor the current build's own start authorizes adopting a
different sandbox by name. Without a pending create, an unrelated name occupant and its snapshots are left alone; the
normal name-free wait refuses a new build if the name stays occupied. Such a sandbox stays hidden from Sandboxes not on
this helm while its recipe exists, so it must finish or be removed in Tensorlake before retrying. Ordinary write
durability is retained: a legacy row or power loss that loses both the ID and pending marker requires explicit cleanup.
A lookup that finds nothing settles a pending create only 15 minutes after it was sent (the client's ten-minute create
bound plus margin; Tensorlake makes a sandbox durable, and so findable by name, before it waits for capacity), and until
then the delete is refused with "try again in a few minutes". The new snapshot's ID is written in the same write that
moves the old one to `retired_snapshots`, and a builder ID or retired snapshot is removed only once Tensorlake confirms
the deletion or answers 404. The steps are: cleaning (reconcile, retry retired deletions, wait up to two minutes for the
name to be free), creating (the recipe's image or snapshot base, with Tensorlake Ubuntu resolved through the one
`tensorlake/ubuntu-systemd` constant; the recipe's CPUs, memory and disk; a 900-second idle timeout, so a builder a
crashed helm left suspends and stops billing for compute; `auto_resume` off), starting (poll every 2 s for up to 15
minutes; terminated fails with Tensorlake's reason), connecting and checking the base (`Connecting`, step 4 of 9),
packages, Claude Code, Codex, snapshotting (`filesystem`, polled until `completed`, not merely `local_ready`, for up to
30 minutes; `failed` fails the build) and finishing. Sweeping a builder terminates it, lists every snapshot and deletes
those whose `sandbox_id` is the builder's, except any snapshot a recipe records as current; a failure leaves the builder
recorded. After a failed step the builder, if its ID is recorded, is swept, bounded to two minutes; a builder whose ID
could not be recorded is left running under the pending create, for the next build or the delete to find by name and
record before terminating it. The failure is recorded with its step, and the previous snapshot is untouched. A build
records the frozen base and packages it used even if the recipe was edited while it ran; the row then warns that the
current definition differs. Deleting a recipe marks it as being deleted (no build may start), cancels a running build
and waits for its task, reconciles, deletes the snapshot and retired snapshots, and only then deletes the row.
Cancellation is cooperative: only waits on Tensorlake, the sandbox and poll sleeps are cancelled, never the create or a
state write, so everything a cancelled build created is recorded before the delete reads the state. A recipe cannot
select as its snapshot base a snapshot Farhelm built (any recipe's current or retired one): Farhelm deletes those when
their recipe moves on, so the save is refused naming the owning recipe. Not covered: a snapshot a builder took that no
recipe records yet (the helm stopped between taking it and recording it) can be named as a snapshot base, and the next
sweep of that builder then deletes it; finding its ID takes listing the account's snapshots by hand. The list shows a
running or interrupted build and pending cleanup for every recipe. Build, Rebuild and Retry are available for every
base.

Tensorlake builds and hosts share an identity policy that offers Farhelm's generated key first during public-key
authentication. `IdentityAgent=none` prevents agent-key promotion, `PubkeyAcceptedKeyTypes=ssh-ed25519` excludes
certificates, and `PKCS11Provider=none` prevents a token key from preceding the file key. Other configured file keys may
follow it. The older accepted-types spelling works on the supported OpenSSH 7.6 floor and remains an alias on newer
clients. OpenSSH 7.6–7.9 treats the provider value as a library name and emits a nonfatal `dlopen none failed`
diagnostic; the type filter excludes those versions' RSA-only token keys. Proxy and jump configuration still applies,
and a ProxyJump hop's separate ssh child keeps its own agent and identity policy.

The build's scripts (`build_script.rs`) run as the sandbox user under `sh -c`, through the narrow `SandboxShell` trait.
Its production implementation runs `ssh` with `ssh_oneshot_args` (the connection overrides, `BatchMode=yes`,
`ControlMaster=no`, `ControlPath=none`): a builder's name is reused, so a `ControlPersist` master to an earlier builder
would carry commands to a terminated sandbox, where a new channel hangs. The Tensorlake ssh options
(`tensorlake/ssh.rs`, shared with Tensorlake hosts) are Farhelm's key with `IdentitiesOnly=yes`, `IdentityAgent=none`,
`PubkeyAcceptedKeyTypes=ssh-ed25519` and `PKCS11Provider=none`; `UserKnownHostsFile` in `<state>/tensorlake/known_hosts`
with `GlobalKnownHostsFile=/dev/null`, `HostKeyAlias=sandbox.tensorlake.ai` (every sandbox name presents the gateway's
one host key), `StrictHostKeyChecking=accept-new` and `UpdateHostKeys=no`; and `ServerAliveInterval=30` with
`ServerAliveCountMax=3`, inside the gateway's measured 300 to 400 second silent-drop window. The builder alone adds
`LogLevel=ERROR`, which drops the gateway's login banner from every command's stderr; a host's connections leave it out,
because their failure text is ssh's stderr tail and OpenSSH logs a server's disconnect reason below that level. Paths
are quoted for ssh_config. Tensorlake access refuses a path containing `%`, `"` or `\`, which supported older OpenSSH
versions cannot carry safely, or `${`, which ssh would expand, with a message naming the path and asking the user to
move the helm's state directory. The Tensorlake directory both use is computed in one place
(`tensorlake::tensorlake_dir`). The child runs in its own process group and is captured through provisioning's bounded
`capture_child`, which fails past 64 KiB of output, so every script logs to `/var/tmp/farhelm-build-<step>.log` on the
sandbox and sends back only the log's last 8 KiB on failure; the helm keeps the last 12 lines, at most 2000 characters,
stripped of presentation-unsafe characters. Exit 255 with OpenSSH's `Permission denied (publickey` is a refused key and
fails at once with the registration hint; any other 255 is unreachable, retried for up to 60 seconds while connecting,
since sshd can lag the sandbox's running state. The readiness script refuses a PID 1 other than systemd, polls
`systemctl is-system-running` every 2 seconds for up to 180 until it says (right after boot it answers nothing, even
with `--wait`) `running` or `degraded`, and requires passwordless `sudo -n`, apt, bash, curl and git before any
installation. It stops at the first missing requirement with a message naming it, reported through the existing build
failure line. The package script runs `dpkg --configure -a`, `apt-get update` and
`apt-get install ca-certificates curl gh <packages>` through
`sudo -n env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a` with `DPkg::Lock::Timeout=600` and `confdef`/`confold`,
each package shell-quoted. Each agent script downloads its installer with `curl --proto '=https' --tlsv1.2 -fsSL` into a
file and runs that file (`bash` for `https://claude.ai/install.sh`, `CODEX_NON_INTERACTIVE=1 sh` for
`https://chatgpt.com/codex/install.sh`), finds the command with `~/.local/bin` first on `PATH`, symlinks it into
`/usr/local/bin` (on the default `PATH` of non-login ssh commands and the systemd user manager alike, and following the
agent's updater when it re-points its own entry), and prints the bare command's `--version`, which the build records.
The build's final script runs `apt-get clean`, truncates `/etc/machine-id` to empty (so each clone generates its own ID
at boot) and removes the build logs. Logind, linger and Farhelm installation belong to the prepared layer below;
ordinary setup recovery on an existing host still performs them directly.

## Tensorlake prepared snapshots

Preparation is a separate per-recipe single-flight task in the existing build registry, with its own watch progress,
cancellation, durable preparer ID, pending-create time, start and failure records in `RecipeState`. An explicit Build or
Rebuild starts or joins a build, never a preparation. No preparation endpoint, timer or startup reconciliation is added.
`ensure_prepared_snapshot` is the shared entry point for Add and later consumers. It ensures the built layer, joins an
existing preparation and rechecks freshness after completion. If Rebuild changed the built source during installation,
the completed result records its actual lineage and a later preparation replaces that now-stale result before use.

The current prepared record contains ID, preparation time, Farhelm build version, source built-snapshot ID, source disk
size and installation facts (home, absolute binary and state paths, installed version). A record is stale exactly when
its age is strictly greater than 86,400 seconds, its Farhelm version differs from the helm's, or its source
built-snapshot ID differs from the recipe's current one. Display precedence when several apply is version, source, age.
Recipe definition, size, credentials and tunnels do not add invalidation rules. The recipes view adds defaulted
`prepared` and `preparation` fields alongside `snapshot` and `build`; the latter keep their existing meaning.
Preparation reports Creating, Starting, Installing, Snapshotting and Finishing. The list uses the shared activity clock
only for displayed ages, qualifying idle readiness as last checked. The helm decides reuse on every call.

The preparer is `farhelm-prepare-<tag>-<recipe>`, with the confirmed recipe CPUs and memory, the built snapshot's
recorded disk, a 900-second idle timeout and `auto_resume: false`. It follows the builder's recorded-before-acting
ownership contract: only its own pending-create marker permits name recovery, observed IDs are saved before further
work, create and durable writes are not cancelled, and absence settles only after the shared 15-minute window. Current
built and prepared snapshot IDs are protected from sweeps and refused as recipe bases, as are retired IDs. The new
prepared ID and retirement of the old one land in one state write. Cleanup terminates the preparer, deletes its other
snapshots and retries retired deletions; failures retain ownership. Recipe deletion marks deleting, cancels and awaits
both task kinds without holding their source mutex, cleans both resource roles, then lets the store forget the row.
Orphan recognition and reserved names include preparers.

One per-recipe async mutex excludes source selection/create through the first observed Running state from snapshot
publication and deletion. Installation and supervisor attachment run outside that exclusion. Consumers reread freshness
while holding it and release it before waiting for a build or preparation that needs to publish. This is live-process
exclusion, not a durable dependency registry: an interrupted or ambiguous create overlapping retirement retains the
ordinary vendor failure/retry behavior. Ownership recovery remains durable independently of the mutex.

Production constructs a narrow `SnapshotPreparer` from the already selected provisioning backend, payload source and
layout before starting the connection manager. It owns no manager, host row, delivery service or tunnel. A separate
one-shot `SystemBackend` instance governs every preparer SSH operation, including inspection and uploads; ordinary host
provisioning retains its shared connection. Preparation reuses ordinary payload resolution/staging and host-independent
Add actions. Host attachment is expressly excluded. The preparer requires boot-capable linger and persistent unit
enablement; ordinary degraded login-only setup is insufficient for a reusable snapshot.

After installation, the preparer stops the supervisor while retaining its enabled unit, verifies that no unit processes
can recreate state, removes the whole planned state directory using the existing symlink/mount refusal semantics, and
verifies absence including `supervisor.db` and dangling links. It clears and verifies `/etc/machine-id` before returning
facts to the engine; any failure prevents checkpointing. The engine takes a filesystem checkpoint and waits for
Completed, never only LocalReady. Each clone therefore opens an empty supervisor state directory and mints its own
identity through normal startup; no supervisor identity-reset operation is introduced.

Preparation invokes no credential delivery or gh setup and creates no tunnels. Authentication inherited from a custom
base is left intact, with the base author responsible for it; this is not a claim that prepared snapshots are free of
inherited credentials. Farhelm's selected authentication and tunnels remain per-host connection/setup work.

## Tensorlake sandbox sessions

Session ownership is one durable marker on the existing Tensorlake host row, added without rebuilding `hosts`. Existing
columns retain the recipe, chosen reserved-prefix name, canonical sandbox ID and recorded create facts. The ordinary
host actor supplies reconnect, credentials, tunnels, updates and session aggregation. HostView retains the row; only
permanent rail icons and ordinary launch/template/agent host choices filter it. Hosts and the existing sidebar attention
list use their ordinary rows and remedies. Do not create a second diagnostic projection or management path.

A recipe launch has a distinct protected route. Older ordinary-create decoders ignore unknown fields and default an
absent host to local, so a recipe request must never fall through to ordinary creation on an older helm. The helm
composes prepared-snapshot, Tensorlake add-run and session-create facilities with existing progress and run limits. It
records the owned row and `farhelm-sess-<tag>-…` name before billed work. API key, readable local SSH pair and a built
snapshot are prerequisites; local key presence does not prove account registration. Reuse the registration remedy for
SSH public-key refusal. Missing software snapshots are not built by session launch. Set `timeout_secs: 1800` and
`auto_resume: false`; credentials and tunnels keep ordinary Add warning semantics.

For managed checkouts, carry repository and naming intent until the new supervisor connects, then use its existing
preview, allocation and create admission. Confirmed creation keeps ordinary terminal clone, hook and error behavior.
Before a confirmed create reply, a failed normal launch attempts whole-sandbox cleanup through the existing bounded
Tensorlake deletion calls and resource proof. The row is retained if cleanup cannot complete. Lost create replies need
no special reconciliation: the newly created resource may be destroyed. Interrupted owned resources remain visible in
Hosts, and resources without a row are discoverable by the reserved prefix. There is no new launch journal, retained
request fingerprint, exactly-once launch guarantee or startup continuation worker.

Explicit session Delete asks the sandbox's supervisor ("Environment Delete and the seal"); on `Sealed`, and for the
warned whole-sandbox Delete when the supervisor cannot be asked, it calls the existing whole-sandbox removal path, which
validates the host/resource binding and deletion consent, not the number of sessions. Any names displayed come from the
ordinary cache and make no completeness claim. The whole-sandbox deletion works while the supervisor is unreachable or
suspended; it never asks for Resume to inspect membership. Existing bounded termination and snapshot cleanup precede
forgetting the actor and host-owned rows. Failure retains the resource and reason. Host Delete sandbox always uses that
path and its own confirmation. Bulk Delete sends one supervisor request per marked session of a connected sandbox and
coalesces only the rows of sandboxes it could not ask by host ID, without requiring a complete member set or a
designated last session.

Replace stays in the same sandbox and retains ordinary create-before-source-removal ordering. Its internal source
removal calls ordinary supervisor session deletion, which never seals and never destroys the sandbox. Failures preserve
the resource and report known outcomes and its name; rare unresolved cases can require manual recovery. The warned
whole-sandbox Delete may subsequently destroy any leftover session, listed or unlisted. This is accepted product
behavior; the seal is the only finality state, and nothing else proves which session remains.

Suspend and Resume reuse host lifecycle tasks. Stop and suspend completes ordinary Stop first, then applies the host
Suspend rule to the whole resource; failure or incomplete Stop withholds Suspend. No path automatically resumes it. Bulk
lifecycle conveniences remain deferred. Recipe recent setups carry recipe and place, never the disposable host's install
identity. The ordinary per-host provisioning/lifecycle guard protects conflicting resource work; do not introduce new
membership-specific locks or ordering.

Templates represent a recipe separately from the existing opaque install-identity host field; both together are an
inapplicable Host field. Shared template application treats them as one placement choice. Clone resolves an owned source
to its recipe and creates a new sandbox. Restart/Replace retain the actual host internally; ordinary creates reject
owned host destinations, except an agent create from that same environment. Agents cannot list, read or write recipe
templates; agent template verbs naming one are refused, and only `farhelm agent create --template` from an ordinary host
launches one. Changes to the shared template encoding must preserve the existing strict-decoder and old-page failure
behavior; a protocol bump, if needed for that encoding, is separate from the removed supervisor policy and membership
protocol.

Agent requests from and about sandbox sessions follow "Agent requests and environments" under Disposable environments.
Do not add a notification kind, generation bookkeeping, supervisor metadata or acknowledged connection policy for them.
All policy belongs to the owning helm; the ordinary relay already refuses when no owning helm connection exists.

## Tensorlake hosts

A Tensorlake host is a `hosts` row with `kind = 'ssh'`, a non-null `tensorlake_sandbox`, and a NULL or Tensorlake
environment-provider discriminator (schema 72 added the sandbox column and four more: the sandbox's id once a create
answers, the recipe's name, the size as recipe-size JSON, and the time Farhelm's own Suspend finished). The kind CHECK
is not widened, because that needs a rebuild of `hosts`, and about ten tables cascade-delete from it; only the sandbox
column has a CHECK (`kind = 'ssh' OR tensorlake_sandbox IS NULL`), since relaxing a column CHECK later costs a rebuild
too (schema 58). Rust derives a distinct `HostKind::Tensorlake` through the common kind/name/provider decode in every
reader, so each exhaustive match and named predicate decides what a sandbox does: it has a remote install and is updated
like an ssh host, is remote for browser links and tunnels, has no panel uninstall, no editable destination and no manual
2FA connections, and its removal deletes the machine. A reader that decoded `kind` alone would let a sandbox row be
retargeted or forgotten as an ssh row.

Connections to a sandbox use the ordinary ssh argv, with its shared connections, plus the snapshot builder's Tensorlake
ssh options (see "Tensorlake snapshot builds") before the option terminator, so builders and hosts share Farhelm's key,
one known-hosts file and one host key alias. The user's ssh config is still read, since a network may need its
`ProxyCommand`; command-line options win where they overlap. A state directory whose path contains `${`, `%`, `"` or `\`
is refused with an error naming it: OpenSSH expands `${NAME}` in these options even inside quotes and has no escape for
it, and supported older OpenSSH versions cannot carry the other three safely (see "Tensorlake snapshot builds").
Provisioning resolves the common SSH remote route with its Tensorlake access options. The serialized SSH plan spelling
and omitted ordinary-access field remain compatible, including payload uploads.

Before each connection attempt on a Tensorlake host, the connection actor asks the API for the sandbox's status through
a provider implementation the manager owns, inside the attempt's 20-second budget and bounded at 10 seconds itself.
Running dials. Suspended, suspending and resume-failed publish the `suspended` state without dialing; the first time,
the actor also runs `ssh -O exit` on both of the host's shared masters (5 seconds each, best effort), because a master
survives the suspension and a new session on it hangs. A starting, snapshotting or unknown status fails the attempt so
the ladder retries; a terminated or missing sandbox fails it with the advice to delete the host. When the API cannot
answer, the actor dials anyway and adds the reason to a failed dial's text: a new connection to a suspended sandbox is
refused promptly by the gateway, while refusing to dial would cut off every running sandbox whenever the key is cleared
or the API is down. The check is made per attempt and never while connected, so a connected host costs no API traffic
and a suspended one one call per re-probe (45 seconds). A sandbox suspended under a live connection is noticed when the
30-second refresh timeout tears the connection down. `suspended` carries the time this helm first saw it; the host list
adds the row's own suspend time, so the UI can tell Farhelm's suspension from Tensorlake's idle timeout across helm
restarts. Provisioning does not go through the actor's gate, so it checks the same state itself: Update and setting a
host up again are refused for a host the manager shows suspended, before anything is probed and again before a confirmed
run's preflight probe, with a resume-first message, and the panel offers neither while the host is suspended.

Removal (`DELETE /api/hosts/{id}`, the GUI's only removal) first holds the host's connection actor: the manager
publishes a not-connected state with no client, interrupts whatever the actor was doing, and waits until it parks, after
which neither a nudge nor retry time makes it dial or publish (the hold flag is checked inside the same `send_modify`
every actor publication takes). A dead actor's replacement inherits the hold and its displayed state under the actor-map
lock before its start gate opens, including a hold acquired while Retry is reading the registry; its parking
acknowledgement starts empty because it belongs to the new task. Release after a failure and stop after success address
the current entry, so replacing the actor cannot end the host's hold. Only then does removal close the host's shared
masters, which the actor can no longer reopen, and terminate a recorded sandbox. Without a saved id, an outstanding
create permits recovery by name; the recovered id is saved before terminating, so a retry still finds snapshots after
the name stops resolving. A missing sandbox counts as terminated only once that outstanding create has settled. With
neither an id nor an outstanding create, removal forgets only the entry and makes no sandbox API calls. For a recorded
sandbox it polls until Tensorlake reports it terminated (at most 30 seconds, so a snapshot a suspend was still writing
exists before the sweep), lists the namespace's snapshots and deletes those whose `sandbox_id` is the sandbox's (a
missing one counts as deleted), and only then forgets the row. Each API call is bounded at 20 seconds. Every step
accepts that its work was done, so removing again after a failure finishes the job. The store enforces the order:
`remove_host_row` forgets a Tensorlake row only when handed a `SandboxDeleted` proof for that row, and refuses
otherwise. The proof establishes cleanup of recorded resources or absence of any create record; it does not establish
that an account has no legacy or power-loss leftover. Teardown constructs it, as does registration rollback before a
create request; a retarget of such a row is refused the same way. The hold lasts until the actor stops with the row; a
failed removal releases it with a fresh retry. Without an API key the removal is refused (409) rather than falling back
to forgetting. The panel's uninstall is refused with a "delete the sandbox instead" remedy, and an agent's host remove
is refused before any card.

Explicit Tensorlake Suspend and Resume use `POST /api/hosts/{id}/suspend` and `/resume`, on helm-owned tasks so a closed
tab cannot strand a hold. Both take the host's provisioning lock before reading its binding, excluding setup, update and
removal. `HostState::TensorlakeChanging` and its UI projection carry `LifecycleDirection` (suspending or resuming) and
the request's unix-ms time under wire phase `tensorlake-changing`. Suspend holds the actor in that transient state,
releases both shared SSH masters, asks Tensorlake to suspend and waits at most 60 seconds. Only confirmed suspension
records the completion time and publishes `Suspended` under the hold, so the displayed timestamp cannot momentarily
attribute Farhelm's own suspension to an outside action. Resume holds a disconnected actor as Resuming around the
Tensorlake call, waits at most 120 seconds for running, clears the timestamp and releases the hold with a fresh retry.
The transient phase ends at hold release rather than covering reconnection. The confirmed Suspended snapshot also
carries Farhelm's completion time: a host listing may have read an older registry row before taking that snapshot, so
the snapshot's attribution wins over the row's. The stored value remains the fallback after a restart. Already-at-target
states succeed; conflicts are reconciled by reading status. An actual resume withdraws any pre-suspension connection and
releases both masters before retrying, even if the actor has not yet detected an external suspension. Only an
already-running sandbox with an already-connected actor skips recovery, preserving its viewers on repeated Resume. Every
failure releases the hold. The launcher's existing-folder path binds the fresh connection token after resume. Fresh
checkouts arm the form's one-shot pending submission, bound to the returned connection. The old connection is only a
waiting state while the form's independent host read catches up; its preview never authorizes creation. The form's own
Ready preview on the returned connection submits once, spending the pending submission before dispatch. Draft or opening
changes cancel it. That waiting state ends after 60 seconds (`WOKEN_CONNECTION_WAIT_MS` in `list/create_form.rs`): a
press still short of the returned connection then is discarded through the same path as a draft change, with its own
error, while a press already waiting for its preview is left alone. A per-press counter keeps an earlier attempt's timer
from discarding a later attempt in the same opening. Automatic template/Clone name candidates may advance under the
unchanged naming policy, while typed and fixed names remain exact. Resumed auto-allocation does not pin the old checkout
configuration epoch, which the host's history may not have revealed when a shortcut pressed; the current Ready preview
still matches the full live authority and the server validates its configuration. Ordinary guarded answers keep their
exact epoch comparison. A lost create race refreshes the preview but never submits again automatically.

`resume_host_connected` tolerates the Suspend and Resume other surfaces may run beside a launch. The helm marks the one
lifecycle refusal that waiting can clear, another operation holding the host's provisioning lock, with the helm-only
`x-farhelm-host-busy` header, the same for every provider; the helper sends Resume again each second while it gets that
refusal, stops as soon as the host connects through another Resume, and fails at once on any other refusal. All its
waiting, busy refusals included, shares one 60-second budget that only the accepted Resume request does not spend, so a
slow resume is never cut off. It reads the host's phase before the request, and a suspension stamped later than that one
(`observed_at` or `suspended_by_farhelm_at`), any suspension when it did not start suspended, or a suspension after the
connection published a phase of its own fails the wake at once. A suspension with the starting marks is not one: it is
the starting reading, which keeps showing while the wake's Resume is refused as busy behind an operation that publishes
no hold of its own (an update run, an edit), and which comes back after the hold of another Suspend of a host Farhelm
had already suspended, since that republishes the stored marks. A failed-resume mark that reading already carried
belongs to an earlier Resume, so only one that appears later fails the wake.

The launcher's preparation signal belongs to its own submit and names the host being resumed. It surrounds
`resume_host_connected`, including that helper's bounded reconnection wait; only that form treats its own pending host
as available and replaces the unavailable note with preparation progress. Both destination paths say the session starts
when the host is ready. Quick launch renders the same preparation in its existing progress surface. Preparation failure
sends no create and has its own refusal wording; session-create progress and uncertain-outcome recovery begin only at
their existing dispatch boundary. The attention classifier treats a routine lifecycle transition as neither a Problem
nor an Update, while independent provisioning progress and unseen setup warnings keep their existing precedence. Tunnel
failures retain their connected-host eligibility; stopped forwards during a lifecycle hold do not report a live tunnel
failure.

A successful helper read can precede the form's independent host read. A display-only receipt suppresses the obsolete
unavailable note for that exact pending target and connection token until the displayed snapshot changes. It grants no
launch or checkout-preview authority, and retires on a changed token, target, or settled state, so it cannot hide a
later connection failure.

The Suspend question uses the shared framed-dialog header, scroll body and footer as direct siblings; its existing
lifecycle callback still owns acceptance and dispatch. Dismissal releases that modal before returning focus to the
host's control. Sandbox metadata reuses the ordinary `host-detail` style rather than a separate type scale.

The remote `internal stdio` proxy has no input deadline by default. Every environment route passes
`--stdin-idle-limit-secs 120` unconditionally, using the common host-access decision. The option shipped in the same
release as Tensorlake host support, and new Podman environments carry a prepared helper. Ordinary SSH argv retains its
default without an input deadline.

For environment hosts the client's writer emits an empty data frame on channel zero every 30 seconds, giving due
heartbeats priority between queued frames. No channel can be allocated at zero, so the supervisor's data router ignores
it, including older supervisors; there is no new control variant or protocol bump. The timer belongs to the writer,
independently of refresh/cache/database completion. Only byte activity on the helper's stdin resets its watchdog. The
watchdog runs outside the copy pump, so blocked supervisor writes or stdout backpressure cannot defeat expiry; expiry
exits the process with status 1 and closes the socket even while the gateway retains its TCP peer. Clean stdin EOF
disables the watchdog, half-closes the socket and drains pending replies. Process tests pass a short budget directly to
their child, without changing the test process's environment.

## Tensorlake recipe tunnels and unowned sandboxes

The connection actor gives its environment connected-resource hook the actual `SupervisorClient`. The hook snapshots the
recipe once and acquires a `TunnelLease` that owns a claim worker on that connection. Leaving the connected phase,
entering a hold, removing the row or aborting the actor drops the lease: it aborts the worker, retires the claiming
connection and withdraws the tunnel view. There is no release message or separate SSH process. The actor also races hook
acquisition against its nudge, so a hold cancels a recipe read still waiting on storage instead of starting listeners
after teardown. Tunnel health invalidates the fleet feed independently of supervisor health.

The worker claims all configured ports and publishes Up only after the correlated successful reply. A failed bind
retries after 1, 2, 4 seconds, capped at 30, while the same host connection remains live. An unanswered claim times out
after 30 seconds and retires the connection, because cancellation cannot establish whether the supervisor installed
listeners. A successful claim remains owned by that connection until it ends; recipe edits apply on the next connection,
never midway through a retry.

For each accepted event, the worker resolves the reported port against its own snapshot. An unknown port is refused
without dialing; otherwise the helm dials that configured host and port with a ten-second bound, removing IPv6 display
brackets before resolution. An unreachable destination fails only that accepted connection. The shared relay pump
provides credit, directional EOF and cancellation; the worker's owned task set keeps pending dials and relays within the
client's ingress bound and aborts them together when the lease ends.

These relays inherit the supervisor connection's transport and teardown. They therefore use the host's ordinary SSH
configuration, including ProxyJump, without interpreting any SSH forwarding directive or adding another connection.
Through Tensorlake's gateway, a dead helm's stdin EOF ends its stdio session and the supervisor drops the connection's
listeners and sockets before unrelated teardown waits. A silent helm is handled by the existing 120-second stdio stdin
watchdog, not a tunnel-specific heartbeat or watcher. Disconnecting leaves no independent tunnel session to keep the
sandbox awake. The prior separate-forward design could leave a local SSH forwarded channel open after the gateway's
sandbox session ended; carrying bytes inside the existing supervisor protocol avoids that extra channel lifetime.

Standalone SIGINT/SIGTERM enter the same serving teardown as an embedded owner's shutdown receiver. Teardown withdraws
clients, cancels connection actors and joins their supervisors, which await the actors, before returning; connection
leases therefore drop while the runtime still exists. Sessions continue on their supervisors. No tunnel child,
parent-death signal or platform-specific lifetime helper remains.

Protected `/api/tensorlake/orphans` recognizes `farhelm-sess-` alongside the ordinary host, builder and preparer
prefixes. It lists reserved-name sandboxes not owned by a host row or a deterministic builder/preparer name of a saved
recipe. A session-owned launch or cleanup leftover with a host row stays visible in Hosts; no session membership
evidence is needed to make it manageable. `/api/tensorlake/orphans/{id}` DELETE rechecks the prefix and current
ownership under the host provisioning lock. An owned resource uses host Delete sandbox instead. It terminates the
sandbox, waits for termination, and deletes snapshots by source sandbox ID using the existing bounded removal calls. The
delete runs on a helm-owned task so closing Settings cannot cancel cleanup. The service retains in-flight and failed
explicit cleanup operations in memory, including canonical sandbox ID, display facts and the failure reason; the list
overlays those operations on vendor discovery because terminated sandboxes disappear from that discovery. Settings shows
running cleanup and offers Retry cleanup after failure, including after closing/reopening Settings or losing the
vendor's terminated sandbox record. A retry still rechecks local ownership. Only a successful snapshot sweep removes the
operation; nothing retries automatically. A helm restart loses this in-memory operation state; a session-owned leftover
still has its durable host resource record and remains visible in Hosts. Other unowned sandboxes gain no persistent
ownership ledger, and neither path adds automatic startup reconciliation. Settings re-reads on invalidations and offers
Refresh for changes outside this helm. A failed discovery pauses automatic reads until Refresh, a key save/clear in this
dialog, a successful Test of the saved key, or remounting Settings; unrelated fleet changes must not keep resending a
rejected key. Key replacement in this dialog triggers discovery even when a key was already saved. A successful Test
resumes failed discovery with one reload. Another tab's key save is not observed automatically: this tab needs Refresh,
Test or reopening Settings.

`OrphanList` keeps local checking and last-successful-check feedback beside its existing rows, discovery error and
generation. A saved key keeps its section mounted when discovery is empty; Refresh sits beside the heading and is
disabled during discovery or a deletion request. Only the current successful read updates rows and the viewer-local
HH:MM label, obtained from the renderer's Date; the generation is checked again after that bridge await. A bridge
failure labels the time unavailable without changing a successful discovery into an API failure. Failed reads keep
earlier rows and hide the empty-success message. An earlier discovery error remains while explicit recovery runs so
unrelated feed notices remain paused. Clearing the key invalidates pending reads and clears checking/time feedback.
These are presentation facts, not new backend operation state or a timer.

## Feedback forwarding

SPEC.md's Feedback section is implemented as one protected UI route on the helm, `POST /api/feedback`, behind the same
device authentication as every other UI route and absent from the agent request verbs. The UI composes the whole
submission: the message, the optional contact, the version the sidebar already shows, `desktop` or `web`, and the
operating system the browser or webview reports. The helm validates the submission against the shared request type and
forwards that type as JSON: it adds nothing, trims nothing, and stores nothing. Forwarding through the helm rather than
posting from the page keeps the webview and the browser off the internet and gives one place where the feedback
connection lives. After a successful send the dialog separately remembers or clears the contact through the shared
preferences row, using the authenticated client's snapshot to seed later dialogs and the existing preference queue to
write every successful choice; the feedback route still stores nothing. Preference validation uses the same contact cap
and never includes the contact in a refusal or log.

The request type and its caps live in `farhelm-proto`, shared by the UI and the helm, so both enforce the same numbers:
a message of at most 4,000 characters that is not empty or whitespace-only, a contact of at most 200 characters,
machine-written version and operating-system strings that are not empty, hold no control characters, and are at most 64
and 128 characters, and a surface of exactly `desktop` or `web`. Limits count the text as sent; nothing is trimmed in
transit. Whitespace means the Unicode `White_Space` property (Rust's `char::is_whitespace`, JavaScript's
`/^\p{White_Space}*$/u`), which `str::trim` in Rust and `String.prototype.trim` in JavaScript do not agree on. The user
cannot edit the version or the operating system, so the UI makes them fit before it displays them (shortening an
over-long value, replacing control characters, and using `unknown` for an empty one), which keeps the displayed value
the sent value and keeps a cap from refusing a submission the user could not fix. Characters are Unicode code points
(Rust's `chars().count()`, JavaScript's `[...text].length`), so the UI, the helm and the endpoint count alike. The helm
posts to `https://farhelm.io/api/feedback` with a 15-second timeout, and the endpoint bounds its own call to GitHub at
10 seconds so it answers first. The dialog's Send is disabled while a send is in flight. A send that times out at the
helm after GitHub did create the issue can still be retried into a duplicate issue; that is accepted. That URL is a
parameter of the helm's state construction rather than a constant, so Rust tests point it at a stand-in server they
start themselves; there is no user-facing flag, configuration key, or environment variable for it. A failure
(unreachable, timeout, any non-success status) comes back as one plain-text route error the dialog shows; the helm logs
that the send failed without the message or the contact.

The endpoint is a Vercel function in the docs website's project, `website/api/feedback.js`, which Vercel deploys from
the project's `api/` directory alongside the static site. It is plain JavaScript with no runtime dependencies, split
into a pure handler (request, configuration and a GitHub client in; response out) and a thin entry that reads
`process.env` and passes the real client, so `node --test` tests the shipped handler with injected configuration. It
accepts only an `application/json` body, because a browser posts a "simple" content type such as text/plain cross-origin
without a preflight, which would let any web page make its visitors send feedback from as many IP addresses as it has
visitors. It caps the raw request body at 64 KiB, which is above the largest submission those caps allow even fully
JSON-escaped (about 52 KiB), so a submission the app accepts is never refused for size; it enforces the same field caps
again because it is the public boundary, and creates one issue per accepted submission in a private GitHub repository:
the title is `Feedback:` and the message's first non-blank line, trimmed and shortened, as plain text; the body holds
the message, the contact and the version, surface and operating system each inside a code fence longer than any run of
backticks in its text, so no field can close its fence early and none of them renders as Markdown or pings anyone with
an `@` mention. The repository (`FEEDBACK_GITHUB_REPO`, as `owner/name`) and a token with Issues write on that
repository only (`FEEDBACK_GITHUB_TOKEN`) are Vercel environment configuration, never source; when either is missing the
function refuses, and no response or log line carries either value, nor the message or the contact. The per-IP rate
limit is a Vercel firewall rule on `/api/feedback` rather than code in the function, so refused traffic never runs it,
and the kill switch is revoking the token or adding a firewall deny rule, both immediate. The endpoint accepts these
fields for as long as any released app sends them and ignores fields it does not know, so later releases can add fields
without breaking older ones. Two deployment facts this relies on and the repository cannot show: the Vercel project's
root directory is `website/` (where `website/vercel.json` lives), and the site keeps building as plain static Astro
output with no Vercel adapter, as it does today; the setup document repeats both. `docs/feedback-endpoint.md` is the
maintainer's setup and operations procedure.

## Standalone uninstall

`farhelm uninstall` establishes ownership before confirmation, differently per platform. Deletion targets always come
from fixed names and the layout the installer writes, never from arbitrary record fields, and every target must be a
regular file or real directory owned by the current user; records must not be group or world writable.

On macOS (`crates/farhelm/src/uninstall/app.rs`) the installation is the app. The running CLI must be a versioned
program inside it (`Contents/Versions/<version>/farhelm`, which is what `~/.local/bin/farhelm uninstall` reaches through
the forwarder); anything else refuses and names that command. The app's record (`farhelm-app-v2` and the terminal link's
path) is the ownership evidence, since it carries no checksums: every other file changes on every update. In their place
the layout is checked exactly, at every level, so a file the installer never writes refuses the whole uninstall rather
than being removed or skipped. The link is removed only while it is a symlink to this app's forwarder (its folder
resolved, because the installer spells the target with `$HOME` as its shell saw it); otherwise it is retained and
reported. The Running records of the state directories whose runtime locks uninstall holds are removed too, first, so
that the forwarder sends any retry to the Installed version, which is removed late. The link's folder need not be
spelled as recorded, and an installer update accepts any record of this layout, whatever link it names (the app's
location is fixed, so it can only be this installation's), and rewrites it, so a moved `~/.local/bin` or renamed home
does not strand the app.

Removal is ordered to keep `farhelm uninstall` runnable until the other required work succeeds. On macOS the retry
command is the link, the forwarder, `Versions/installed` and the folders of the version uninstall runs as and the
Installed one, so other versions, the main program, the icon and Info.plist go first; a failure among them leaves the
command and a record that still plans the rest. A failure after that point can leave no runnable CLI; what is left of
the app then holds no data, and the report says so. On Linux the flat CLI goes last, and receipt cleanup after it is
nonfatal and reports retained metadata. Already absent payloads are accepted as completed work. No recursive deletion or
rollback is needed.

On macOS, once the locks below are held, uninstall opens the app's folder without following a link (`removal::HeldFs`),
re-checks the plan, and refuses if the folder it holds is no longer the one at the app's path (a Finder move between the
two); the plan itself carries the app folder's device and inode and the record's installation id, so an app put at the
place while the confirmation waited counts as a change too. Through that folder it takes every version's lock
exclusively without waiting (see "Side-by-side versions inside Farhelm.app"): its own version by converting the one
shared lock it holds as a running `farhelm` (only when that lock is on the held app's program; otherwise it refuses as a
change), every other version that still has its `farhelm` by opening and locking it; a version whose program is already
gone is removal already done. A version that cannot be locked refuses with nothing removed, naming the version and
`lsof` on its program, since the lock cannot say which process holds it and it need not be Farhelm. It then moves the
ownership record to its next `removing` generation and flushes it, so an update already on its way refuses and the app
and its updater can tell an interrupted uninstall. The removal goes through the held folder, never following a link;
paths outside the app (the Running records, the link, the app's own name) keep their path-based removal. Leftover
staging and trash folders an interrupted install left beside the app go first, and after the early removals every folder
they changed (`Versions/`, `MacOS/`, `Resources/`, `Contents/` and `~/Applications`) is flushed before the retry
command's own pieces go. The record of either shape is accepted, and so are the installer's own temporary files (regular
`.farhelm-new.*` files, which an install or an uninstall killed mid-write leaves in the app's folders); they are removed
with the early pieces.

On Linux, setup's existing managed-unit parser and removal machinery select services whose recorded executable resolves
to this installation. Custom and other-installation units, drop-ins and linger remain untouched. Services are disabled
and stopped before their unit files or executables are removed. The operation does not examine effective overrides or
processes. Manual shutdown remains an operator prerequisite. Concurrent install, update and setup (and on macOS desktop
startup) are excluded by locks (`crates/farhelm/src/uninstall/locks.rs` and `lock_and_recheck` in uninstall.rs), taken
after the confirmation rather than before it: some of the installers' locks are directories, which nothing removes when
a process dies, so holding them across the prompt would turn a Ctrl-C there into stale locks. On macOS that is the
installation lock on `~/Applications` (a kernel lock, waited for a few seconds; the folder is created if missing, so the
lock is always taken) and the lock folder older installers use (see "Side-by-side versions inside Farhelm.app"); a
volume where `~/Applications` cannot be locked refuses, saying how to remove Farhelm by hand, because without locks
uninstall cannot tell what still uses a version. On Linux the install-directory lock earlier installers used, shaped
like theirs (a 0700 directory holding only `pid`), so an installer that finds it refuses while uninstall runs and clears
it as stale after a crash. On Linux uninstall checks no runtime locks: it stops setup's services itself, after which
only a process the user started by hand could hold them, which stopping is already the operator's job.

On macOS the runtime locks are taken in two steps, because a supervisor the desktop app started can outlive the app and
uninstall stops it (SPEC.md, "Operator prerequisites and failure behavior"). The helm locks of both state directories
(the one the shell selects and the default one, which the app uses whatever the shell says) come first, before the
re-check: an open app holds its helm's, so uninstall refuses there with nothing stopped. After the re-check, uninstall
stops each supervisor the plan listed: the one named by the app's record in that state directory
(`farhelm_supervisor::managed_supervisor`, the file `desktop-supervisor.json`), counted only when the record matches a
live process by pid, start identity and running image (the executable's path, and on Linux the running image's device
and inode, which macOS does not expose cheaply), and the recorded program file (by device and inode, so neither a `..`
spelling nor another app put at the same place counts) is one of the version programs of the app being removed. The
record is re-read first and must still equal the planned one, so a record replaced after confirmation is not followed to
a process the user was not shown. The stop is SIGTERM with a grace longer than the supervisor's own output-client
shutdown budget, never a kill; a supervisor still running after it, one whose last threads are still exiting a few
seconds after it ended, or a stop that cannot be confirmed because the process could not be read or signalled, refuses
with nothing removed and keeps the record. The record of a supervisor confirmed gone is removed. Only then does
uninstall take the supervisor locks; a holder at that point is reported as usually a supervisor uninstall did not stop,
with an `lsof` command that shows which. A refusal after the stop says the supervisor was stopped and that opening
Farhelm starts it again.

Uninstall ends no terminal. Terminal hosts (see "Terminal host") outlive the supervisor by design, including the
supervisor uninstall stops on macOS, and keep running with their sessions; stopping sessions first is the operator's
prerequisite (SPEC.md, "Operator prerequisites and failure behavior"). The preview says so: sessions and their terminals
keep running without the desktop app or the supervisor, and uninstall does not end them.

The confirmation preview is flushed before mutation, and subsequent progress is buffered so an output-pipe failure does
not interrupt removal midway. Filesystem and service failures retain the CLI, report concrete paths and operation
errors, and ask the user to resolve the failure and retry. Tests inject failures at deletion boundaries and run the
actual installer and compiled CLI in private homes; native macOS executes the same filesystem acceptance harness
headlessly in on-demand CI and the release gate.

## Logging

`tracing` everywhere, with `tracing-subscriber` env-filter semantics. Processes log to stderr; where stderr goes is the
starter's choice: the systemd journal for a setup-managed supervisor, the terminal for one started by hand, and for the
desktop app's detached supervisor `supervisor.log` in its state directory (see "Side-by-side versions inside
Farhelm.app"), because that supervisor outlives the app whose stderr it used to share. A terminal host has nowhere to
log: once ready, its standard streams are on `/dev/null` (`crates/terminal-host/SPEC.md`, "Start and readiness"), so
what happens to a host is logged by the supervisor that drives it. The intended mature shape uses spans carrying session
and host ids, so SPEC.md's required diagnostic trails (creation, PTY lifecycle, attachment transfer, reconnection,
resume) fall out of structured context rather than ad-hoc log lines.

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

- `farhelm helm checkout-config show`, `set-root <path>`, `clear-root`, `set-post-clone <command>` and
  `clear-post-clone` — inspect/edit existing helm checkout settings. Each accepts `--state-dir`; `--host <id>` selects a
  registered host override instead of the global value. An empty post-clone command disables inheritance explicitly. The
  default folder and all-hosts command can also be saved in Settings; host settings exposes each host's folder, while
  per-host command overrides remain CLI-only. The command neither initializes missing state nor migrates an incompatible
  database, and never creates target directories or runs the hook.
- `farhelm helm run` — run the helm (flags: `--port`, `--state-dir`, `--ensure-hosts <file>`, `--payload-dir <dir>` (env
  `FARHELM_HELM_PAYLOAD_DIR`), `--release-base-url <url>` (env `FARHELM_RELEASE_BASE_URL`)). The last two select where
  "Add host" provisioning payloads come from — an operator-staged directory (verified not at all, D3) or a download
  source other than the default, this build's own release on get.farhelm.io (D2); `--payload-dir` wins if both are given
  (D18). It takes no session or transport flags: M1's `--ssh`, `--cwd`, `--agent`, `--title`, `--remote-farhelm`, and
  `--remote-state-dir` were dropped with M6's registry (user decision 2026-08-04). A helm drives every registered host
  at once, so a flag naming one of them could only ever have meant the wrong thing; the last two live on as per-host
  registry fields, and creation is `POST /api/sessions`, which is where the host selection belongs.
- `farhelm helm setup [--state-dir DIR] [--port N] [--no-supervisor] [--dry-run]`, and
  `farhelm helm setup --uninstall [--dry-run]` — write, enable, and remove this machine's systemd user units for the
  helm and its supervisor. Linux only (it exits 2 elsewhere, pointing macOS at the desktop app). One state directory is
  resolved from setup's own environment and pinned into both units, so a shell-only `XDG_STATE_HOME` cannot leave the
  two services on different trees, and both installing and uninstalling refuse when the running user manager reads units
  from a different directory than the one this environment selects. The units are rendered from the templates in
  `crates/farhelm-helm/units/`, the same ones remote provisioning fills in, and every file setup writes starts with
  `# managed-by: farhelm helm setup`: setup overwrites or removes only files carrying that marker, and refuses anything
  else rather than replacing it. The supervisor unit's `PATH` is the installed `farhelm`'s directory plus a fixed base
  ("Provisioning"). The matching rule on the helm side is that the hosts panel never installs or updates a supervisor on
  the helm's own machine at all: an absent one is answered with "run `farhelm helm setup` here", a unit already running
  that same binary is named as setup's or its author's and left alone, and a supervisor that ANSWERS is discovered and
  registered exactly as a remote one is.
- `farhelm supervisor run` — run the supervisor in the foreground; this is SPEC.md's "run the binary with arguments in a
  terminal" path.
- `farhelm spawn [--cwd <dir>] (--inherit-agent | <launch flags>) [--title ...] [--parent ...] [--idempotency-key ...]`
  — the in-session spawn CLI from SPEC.md. Agents are taught it, so it is part of what newer binaries keep accepting
  (see "What running sessions hold across versions"), as are the `farhelm agent` verbs below. Every spawn goes through
  the helm and waits for the user's approval (see "Permission prompts for agent actions"). `--inherit-agent` takes the
  asking session's own stored launch, filled in by its supervisor, and needs `--cwd`; the launch flags are the ones
  `farhelm agent create` takes (see "Agent launches from the CLI"). clap refuses `--inherit-agent` beside any launch
  flag, and a spawn with neither is refused before anything is dialed. `--confirm-yolo` is still parsed, hidden, only so
  it can be refused with a message saying an agent cannot confirm a YOLO launch for the user. `--agent` took a profile
  name before profiles were removed and is now the agent type flag; `--profile-id <id>` is still parsed, hidden, only so
  it can be refused with a message naming the launch flags and `--template`.
- `farhelm agent hosts|sessions|templates [--json]` — the in-session ASKING CLI from SPEC.md, on the same injected
  credential spawn uses. It prints an aligned table on stdout, `*` marking the asking session and its host, and puts a
  refusal on stderr with a non-zero exit exactly as spawn does. Human output is a table because the reader is usually a
  model quoting its own shell output. The JSON form uses schema version 7, includes exact IDs, caller identity, and
  completeness fields, and omits invocation arguments, credentials, resume templates, and provider configuration.
  Version 7 makes session IDs and `caller.session_id` full entry-bound references, and host IDs and `caller_host_id`
  canonical entry UUIDs. The supervisor still relays the same typed fields; the CLI formats caller identity from the
  existing caller-host field and its bare credential. Version 6 added the templates listing. The templates listing
  carries each template's fields except a command line or resume command, which appear only as `sets_command` and
  `sets_resume_command` (in the table, `command (set)`), and names the host a template's install identity currently
  resolves to. Named templates show their effective `name_kind` explicitly in JSON, including `base` when the wire reply
  omits it; the table labels the quoted title as a base or fixed name. This additive discovery default does not change
  the schema version. It does not change the normalized storage or write replies, which omit Base. Version 3 is the
  restart-only-resumes change: `restart_offer` lost `fresh_only` and `fallback_template`, while `resume` kept its
  spelling and meaning. Version 4 is the profile removal: a session's `agent` is always its agent type's word or
  `custom`, never a profile name, and the profiles listing is gone. Version 5 is launch kinds: `restart_offer` gained
  `no_resume_command`, for a command launch with no resume command. `farhelm agent profiles` is still parsed, hidden,
  only so it can be refused with a message saying profiles were removed and naming `farhelm agent templates`. A session
  row's non-secret `OFFER` cell is `resume` when restart can resume the session's conversation, otherwise the reason it
  cannot (`not-captured`, `no-reporting`, `no-resume-command`; `not_captured`, `no_conversation_reporting` and
  `no_resume_command` in JSON); it does not disclose the template, captured conversation locator, or a live-stop
  recommendation. Every dynamic table cell is escaped to one printable line and non-final descriptive columns are capped
  at 48 characters: these values are fleet-wide user text printed straight to a terminal, so a raw newline forges a row,
  an ESC drives the terminal, and one long title would otherwise be padded onto every other row. Exact host, session and
  template selector columns print whole; a session reference is bounded at 77 bytes before display escaping. A cut
  listing prints its rows on stdout and one warning on stderr, so a script capturing stdout still gets nothing but the
  table. It has no timeout of its own: the supervisor bounds the relay and is the only party that can distinguish its
  two failures (see the transport section's version-20 paragraph).
- `farhelm agent delete --session <id> [--stop-if-running]` — removes an explicitly named session, refusing self-delete.
  Success prints `deleted <id>` on stdout and any checkout notice on stderr. The relay and helm both refuse self-delete;
  the helm derives a delete guard from the target state shown on its approval card and passes it unchanged to the
  supervisor. Without the flag, anything alive is refused. UI and agent deletes share the helm's approval denial,
  teardown and best-effort local cleanup; Replace shares the successful local cleanup after inheriting its pin.
- `farhelm agent create` and ordinary `farhelm spawn` accept mutually exclusive `--prompt` and `--prompt-file` (`-`
  reads stdin). Shared flag parsing materializes at most one launch budget of UTF-8 text, allowing newline and tab but
  refusing other controls. Template writes use that same field, and listings include its text. The prompt travels beside
  the reusable launch, through `CreateExtras` and `CreateSession` to `CreateInputs`. The supervisor appends its
  per-harness literal arguments only after cwd and Farhelm-argument expansion, in the shim spec for this create.
- `farhelm agent rename --session <id> --expected-title=<old> -- <new>`, `farhelm agent stop --session <id>`, and
  `farhelm agent restart --session <id> [--stop-if-running]` — the in-session ACTING CLI, on the same relay and
  credential. Every target is explicit, including a deliberate self-action. Rename compares the observed title and
  changes it atomically in the owning supervisor; a mismatch is a conflict with no mutation. Success prints one plain
  confirmation line on stdout (`renamed <id> to "<title>"`, `stopped <id>`, `restarted <id>`), its dynamic cells run
  through the same escaping the listing tables use, so a scripted caller gets exactly one line rather than a table with
  one row. Restart always resumes and forwards the consent unchanged to the owning supervisor, which rechecks both that
  the session can still resume and whether the agent is working; the CLI never infers consent from discovery. The
  `--mode` flag earlier releases required is still parsed, hidden, only so it can be refused with a message saying every
  restart resumes and to drop the flag. An explicit self-stop or self-restart may terminate the CLI before its line is
  printed because it belongs to the process tree being ended. Self restart prints its interruption/outcome-unknown
  warning before dispatch and treats a lost reply as unknown rather than success. Exact own-bare equality wins before
  parsing; for a full target with the caller's bare suffix, a read-only Hosts discovery distinguishes the caller's entry
  before the mutation. A discovery failure stops before dispatch and retains read-only failure semantics.
- `farhelm agent create [--host <name>] [--cwd <dir>] <launch flags> [--title ...] [--idempotency-key ...]` and
  `farhelm agent clone --source-session <id> --host <name> [--cwd <dir>] [--title ...] [--idempotency-key ...]` — the
  in-session CREATING CLI, on the same relay and credential. These invert the stream convention the lifecycle verbs
  follow: stdout is the new session's id and nothing else, matching `farhelm spawn`'s contract, with one confirmation
  line on stderr (`created <id> "<title>" on <host> in <cwd>`, escaped the way the listing tables escape their cells).
  `agent_launch::Resolution` carries the template name kind; applying request flags sets a request's own title to Fixed
  in the helm, including for older CLIs. For a managed destination, `create_checkout_for_agent` first completes keyed
  recovery, then previews a Base through the shared `CheckoutNameSequence` before approval. Only a supervisor-reply
  Conflict advances: this endpoint maps Occupied to Conflict, while helm conflicts such as a missing install identity
  refuse immediately. Other errors and the cap refuse too. Approval and create carry the resolved title and accepted
  path; no create is retried automatically, and the keyed request identity stays the original intent.

  The id is the one agent output meant to be captured as a SINGLE VALUE — an agent takes it and hands it back as
  `--session` — so a confirmation on stdout would make the two verbs that need parsing the two that cannot be. The
  listings are parsed too (the relay fixture reads the hosts table, and `docs/old_readme.md` shows the same), but they
  are parsed as TABLES: a table that grew a column is still a table, while an id that grew a sentence beside it is not
  an id. The stderr line is written through a fallible `write!` whose result is discarded rather than through
  `eprintln!`, because the macro panics on an unwritable stderr and the session already exists by then — turning a
  create that succeeded into a command that failed would tell a caller holding the id to retry a create it must not
  repeat.

  `--host` takes a NAME from `farhelm agent hosts`, printed there WHOLE: the NAME column is exempt from the truncation
  non-final descriptive columns take, because that column is a selector rather than a description and a name cut at 48
  characters is a host an agent can see and can never target. Duplicate names remain separate rows and are refused as
  ambiguous targets. What `create` needs (a folder, a host, a launch, and for a command launch its YOLO assertion) may
  come from a template, so clap requires none of it and the helm refuses what is still missing, naming the flag;
  `--source-session` and `--host` are required on clone. The removed `--profile` and `--profile-id` selectors are still
  parsed, hidden, only so they can be refused naming the launch flags and `--template`, and the retired `--invocation`
  is parsed, hidden, only to be refused naming `--command` and the YOLO flags. Every free-text value option on both
  verbs carries `allow_hyphen_values` (`--agent`, `--effort` and `--permissions` do not: each takes a word from a closed
  set, and an unknown one is refused listing them all), because every one of these values is judged downstream — by the
  registry, by the target filesystem — and every one of them may legally begin with `-`; refusing such a value locally
  would be this CLI declining to carry a name the far end would have explained.
- `farhelm agent template create <name> [--cwd <dir>] [--title ...] [--title-kind base|fixed] [--host <name>] <launch flags>`,
  `farhelm agent template edit <name> ...` with the same flags, and `farhelm agent template delete <name>` — an agent's
  template writes (SPEC.md, Agent-spawned sessions). The launch flags are `farhelm agent create`'s; `--template` is
  parsed, hidden, only so it can be refused, since a template written from other templates would copy command text the
  agent never saw. `--host` names a host by the name `farhelm agent hosts` prints and is resolved to that host's install
  when the template is written. Create records the agent launch kind for agent-launch choices, including an agent type
  alone; command fields record the command kind. Placement and title alone leave the kind absent, and existing kindless
  templates are not migrated. An edit sends only the fields given: omitted name kind keeps the saved kind, while
  explicit Base replaces Fixed before the shared writer normalizes Base out. A kind-only edit merges before shape
  validation so it can use an existing name; a kind without a name after merging is refused. Each write is relayed to
  the helm and waits for the user's approval (see "Permission prompts for agent actions"); stdout gets one confirmation
  line naming the template, as the lifecycle verbs confirm on stdout.
- `farhelm agent instructions`, and its alias `farhelm agent help` — print the agent-facing manual described above ("The
  instructions pointer") locally, generated by walking this same `AgentCmd` definition. Neither spelling touches the
  supervisor, the helm, or the session credential: both must work for an agent that has just been handed the pointer
  line and has no way yet to know whether anything is attached. The generated verb list is padded into two columns only
  up to a 52-character usage width; past it a verb carries its description right behind itself, because alignment pads
  every row to the widest one and `create`'s full command line would otherwise spend a slice of the manual's context
  budget on whitespace. The manual distinguishes conversational `$farhelm help` from an acting request: the agent
  summarizes the generated verbs as user-level actions and gives natural-language examples, without forwarding CLI
  usage, restart caveats, or internal credential/relay details. There is no separate hand-maintained action catalog,
  installed skill, fleet lookup, or help-specific network request.

Internal commands live under a hidden-from-help `internal` namespace — `farhelm internal stdio` is the ssh-exec stdio
proxy. (An underscore prefix like `_stdio` was considered; it is not a recognized convention, while an explicit
`internal` namespace is self-describing and gives future internal commands a home.)

Motivation: one binary is one provisioning artifact and guarantees the spawn CLI exists inside every session (the
supervisor puts its own binary on the session PATH, or in the Mac app's versioned layout the forwarder that reaches the
running version; see "Side-by-side versions inside Farhelm.app"). clap-derive because it is the standard and keeps the
grammar declared next to the types.

## Native app packaging

During the GPUI migration this section describes release machinery that no longer builds: the browser UI and the dx
build it relies on are gone from main, and the release files are deliberately left as they were until the GPUI app
replaces them at cutover. Main ships no releases in the meantime.

Dioxus desktop (wry) wrapping farhelm-ui, released as a BARE BINARY rather than a `.app` bundle: `farhelm-desktop`, a
thin crate (`crates/farhelm-desktop`) whose `main` is one call into farhelm-ui's desktop module, built by cargo-dist
alongside `farhelm` and published as its own archive. The two are one artifact pair from one workspace version: the
shell embeds farhelm-helm and reaches supervisor code through the CLI, discovering a local supervisor that already
answers or spawning `farhelm supervisor run` when none does. Where it finds the CLI is the desktop's sibling, except in
the side-by-side layout below. Replacing that pair never re-issues what running sessions already hold, which is why
newer binaries keep accepting those values (see "What running sessions hold across versions").

`install.sh`, which is macOS-only, puts the pair into `~/Applications/Farhelm.app`, which is the whole installation: the
side-by-side layout below, an Info.plist (bundle identifier `org.scode.farhelm.desktop`) and the icon shipped in the
desktop archive, with `~/.local/bin/farhelm` a symlink to the forwarder for terminal use. The app's executable keeps the
name `farhelm-desktop`, because the default case-insensitive APFS would make an executable named `Farhelm` the same
directory entry as the forwarder `farhelm`. The bundle gives Farhelm its launcher identity (Spotlight/Alfred
launchability, a Dock icon, a Cmd-Tab name, and Launch Services' single-instance activation, so relaunching activates
the running app instead of racing it for the embedded helm's state); asset serving stays the embedded tree below. A
fresh install builds the bundle in a private directory beside the public name and renames it into place. An update
changes the bundle in place, as the next section describes, because a running app survives that and does not survive
having its bundle replaced. The installer changes an existing `Farhelm.app` only when its ownership record
(`Contents/.farhelm-installation`, `farhelm-app-v2` and the terminal link's path) names this installation's link; the
one-time move from the layout that copied both binaries into `~/.local/bin` and the bundle also accepts that layout's
record naming this bin directory, or the recordless bundle installers built before records existed, and is done with
Farhelm quit. Anything else is refused before anything changes. Only one run changes the bundle at a time: a run that
finds `~/Applications/.farhelm-app.lock` held, or left by an interrupted run, refuses. Old releases are refused before
anything changes: those without the icon, and those whose desktop program lacks a fixed piece of the text it prints when
its own version folder is missing (`grep` on the downloaded binary; a test in `desktop/bundle.rs` ties the two
together), which marks every desktop build that understands the side-by-side layout.

Installing a supported target over a newer Installed version remains part of the installer contract: there is no
version-order refusal for a Terminal install whose existing version meets the target's upgrade floor, and the target's
own published installer restores the main program, Info.plist and forwarder to that target while retaining the previous
Installed and Running folders. The ownership record must remain readable by supported older installers; changing its
representation requires a compatible path, not just a new writer. Tagged stable installers from v0.23.0 through v0.28.2
accept the `farhelm-app-v2` NUL-separated link record and this replacement. These are mechanical compatibility
properties, not proof that downgrading local databases is safe, and the updater does not use them to replace withdrawn
waiting releases.

`farhelm_proto::version::UPGRADE_FLOOR` holds the release's minimum supported update source, initially `0.23.0`. The
shared comparison takes both the source and floor as inputs, uses SemVer precedence without build metadata, and never
treats a development build or an unparseable version as below the floor. The floor controls update mechanics, not
data-schema compatibility. An agent raises it in the change that first relies on an earlier release's upgrade logic; the
release procedure surfaces the raise for the maintainer's decision before proceeding.

The dx-produced bundle went away because a bare binary has nowhere to put a `Resources/` directory, and Dioxus's
`asset!()` files were the only thing that needed one. They are served instead from the UI tree compiled into
`farhelm-helm`, through a `dioxus-desktop` asset handler registered on `/assets/*`, handed to the webview over the
`dioxus://` scheme. The helm does not serve this tree over HTTP. Registering a handler for a path prefix takes
precedence over dioxus's own filesystem resolver, so there is no bundle-directory fallback at all; the price is that the
desktop build's asset set and the web bundle's must be identical, which `scripts/check-desktop-assets.sh` enforces on
every change. The desktop page declares UTF-8 in its head, and every `text/*` asset response declares `charset=utf-8`.
Without these declarations WebKit can use a locale-dependent legacy encoding, garbling literal UTF-8 in the stylesheet
and scripts.

A releasable `farhelm-desktop` must be produced by `dx`, not by Cargo alone. The `asset!()` macro emits a placeholder
into a `__ASSETS__` link section and dx rewrites those symbols with content-hashed names after linking; a plain
`cargo build` links and launches but requests placeholder paths, so every asset 404s even with the web bundle embedded.
Release production therefore has to run or consume `dx build --package farhelm-desktop --platform desktop --release`
with `FARHELM_UI_DIST` set; that it must happen is not negotiable, because the failure is invisible to a build that
succeeds.

How that is wired: cargo-dist cannot be asked to run dx over a package it built, and it has no post-build hook — so the
shell is declared to it TWICE, and exactly one of the two descriptions ships. `crates/farhelm-desktop` (the Cargo
package) carries `[package.metadata.dist] dist = false`, which is what stops cargo-dist from publishing its own
placeholder-bearing Cargo output. `packaging/farhelm-desktop/dist.toml` declares a generic (non-Cargo) dist package of
the same name in the hybrid workspace `dist-workspace.toml` defines, and its `build-command` is
`scripts/build-desktop-binary.sh`, which runs dx, refuses a binary still carrying placeholder asset names, checks the
requested set against the bundle it embedded, and hands the result back for archiving. Neither half is redundant: delete
the generic package and the Mac loses its window; delete the `dist = false` and the release grows a second
`farhelm-desktop` archive that renders nothing. The cost of the arrangement is a version number duplicated into that
`dist.toml`, which a unit test in `assets.rs` holds to the workspace's.

Native glue (dock/menu integration, plus whatever proves genuinely necessary — see the clipboard note in the Dioxus
risks) lives behind a feature flag in farhelm-ui, kept deliberately thin.

The native desktop stores a versioned physical-pixel outer-frame rectangle and maximized flag separately from client
credentials. Bootstrap's already resolved state directory is reused, so persistence cannot silently move to the current
working directory if path resolution fails again. A missing file keeps the first-run window builder defaults; corrupt,
unsupported, or display-unsafe state gets a monitor-fitting centered fallback. The geometry check requires the complete
saved frame to fit one connected monitor and avoids arithmetic overflow. Tao provides monitor rectangles rather than
work areas, so the fallback caps its size and a real macOS display/decorations check remains necessary. Wayland's raw
window handle marks positions as unreliable: its move events do not replace the remembered position, and restore keeps a
usable saved size while preserving maximization while the compositor chooses placement. Restore converts the saved outer
size through Tao's inner-size API using the current outer-minus-inner decoration extent; a real macOS
display/decorations check remains necessary. Fullscreen observations never replace the last ordinary frame or maximize
state. The final state is written through a private sibling file and rename on close, with one best-effort retry at
event-loop teardown; the rename is atomic for readers but parent-directory durability across a machine crash is best
effort. Failures warn without preventing startup.

Every pre-window desktop refusal prints one plain `farhelm-desktop: ...` stderr line and exits 1, and on macOS also
sends it to Console and a critical native alert so a Finder or Spotlight launch has a visible diagnostic.

### Side-by-side versions inside Farhelm.app

On the Mac the app bundle holds every kept version of the command-line program side by side, so that an update never
replaces a program a running Farhelm still starts. The layout, with `<v>` a release version such as `0.21.0`:

- `Contents/MacOS/farhelm-desktop`, the app's main program, replaced by rename when an update switches versions.
- `Contents/Versions/<v>/farhelm`, one folder per kept version, written completely before anything points at it.
- `Contents/Versions/installed`, the Installed record: one line naming the version the next start of Farhelm uses.
- `Contents/MacOS/farhelm`, the forwarder: a short `sh` script that runs one of the versioned programs.

Two records decide which version runs. Installed belongs to the installer, which may change it while an older Farhelm is
running; this record, the version folders and the forwarder are all written by the installer. Running belongs to
Farhelm: a supervisor whose own program is `…/Contents/Versions/<v>/farhelm` writes `<v>` to `running-version` in its
state directory, beside `supervisor.sock`: one line holding `<v>`, written to a temporary file and renamed into place,
once it holds the right to serve and before it binds its socket. The version comes from the program's path rather than
from the version compiled into it, because the path is what the forwarder and the installer's cleanup can see. A
supervisor that serves from anywhere else (a development build, a Linux install) removes a `running-version` it finds,
since it is not the version that record would name. Recognizing the layout is by path alone
(`crates/farhelm-supervisor/src/app_bundle.rs`) and works the same on every platform; only the Mac installer ever builds
the shape, so elsewhere nothing changes.

Inside that layout the supervisor hands sessions two different programs. Its own launch shim (`internal launch`, run in
each session's agent terminal, a terminal host's child) is its own versioned program, because the launch description it
writes is read only by its own version. A terminal tab's helper (`internal tab-shell`) starts through the forwarder
instead: a terminal host's first stage holding its spec can start a tab's shell after the supervisor that asked for it
has stopped, and the forwarder claims the version it starts, which is this supervisor's own while it is there.
Everything a session keeps beyond its supervisor's life names the forwarder instead: the hook command lines, the
reporter variables, and the directory put first on the session's `PATH`. Desktop-managed supervisors before this layout
ran from `Contents/MacOS/farhelm`, so sessions they started already hold that path, and putting the forwarder exactly
there is what keeps them working without migration; it must never move. Outside the layout the supervisor uses its own
program for all of these, as it always has.

A target release older than the first build with this installer (`HANDOFF_FLOOR` in `install.sh`, the lowest prerelease
of the release that brought it, so its dev builds and release candidates are never handed off) cannot do the locked work
below, so `install.sh` hands it to that release's own installer: once it has resolved the target (from
`FARHELM_VERSION`, or from `/latest`, which can be rolled back) and read the target's `SHA256SUMS`, and before it
creates or changes anything, it downloads `/<tag>/install.sh`, checks it against that file's `install.sh` entry, and
runs it with the same environment and `FARHELM_VERSION` pinned to the resolved release (so a `/latest` that moves
meanwhile cannot make it install a newer release the old way), exiting with its status. Version numbers are compared as
digit strings, so their length is unbounded. The comparison is semantic-version precedence written in POSIX `sh` (a
release sorts above its prereleases, `dev` below `rc`), and the development version `0.0.0-unreleased` is exempt. Older
installers accept the extended ownership record below by its first two fields and rewrite it in the two-field form, so a
downgrade leaves the older release the record it expects.

An install's locked work is done by the release's own `farhelm`, which `install.sh` runs as `farhelm internal install`
once it has downloaded, checked and staged the release (its two programs, the icon, and the forwarder and `Info.plist`
texts the script writes); the script writes nothing inside the app and reads the outcome from an `install-result` or
`install-failure` file the command leaves in its staging folder
(`crates/farhelm-supervisor/src/installation/install.rs`). That command takes the installation lock: an exclusive
`flock` on `~/Applications` itself (created first if missing), held for exactly as long as it works and dropped by the
kernel when it exits or is killed, with a fixed lock file in `~/Applications` that is never deleted standing in where a
folder cannot be locked, and a refusal saying the installation stays usable on its current version where neither can be.
A busy lock refuses after a five-second wait; nothing ever says to delete it. It then takes the lock folder older
installers use, `~/Applications/.farhelm-app.lock`: created under a temporary name, marked with an extended attribute,
renamed into place only if nothing is there (`RENAME_NOREPLACE`), and removed by renaming it away before deleting it, so
it stays an empty folder older tools' "remove it by hand" advice still applies to. A marked leftover is removed (the
kernel lock proves no newer tool holds it); an unmarked one is an older tool's and refuses as before, including for an
update the app's updater starts, where it shows as a failed check rather than a quiet retry because it may be a stuck
leftover only the user can remove (accepted 2026-10-24).

Under the lock it opens `~/Applications`, the app and its folders without following links and reaches everything inside
the app through those open folders, so if Finder moves the app on the same disk meanwhile the work follows the app it
checked. An update writes the ownership record's next "installing" generation, durably, before anything else; repairs
what a killed earlier run left (a missing `MacOS/` or `Resources/`, regular `.farhelm-new.*` files, and its own staging
and trash folders beside the app); stages the new version's folder beside the app and renames it into `Versions/` (an
existing folder whose `farhelm` is byte for byte the verified download is reused, never rewritten, because its users
hold locks on that file; any other folder for that version, one without a `farhelm` or with a damaged one, is replaced,
a damaged program only under its exclusive version lock, so a copy something still runs refuses the install instead; a
`farhelm` that is a link or a folder counts as missing. The replacement takes the name in one rename that swaps the two
folders (`RENAME_SWAP`), so the version's name never goes empty; only a volume that cannot swap moves the old folder to
trash first); then renames a complete copy of `farhelm-desktop`, the icon and the forwarder (only when its text changed)
over the old ones, flushes `MacOS/`, then the Installed record, then `Info.plist`, flushes the folders whose names
changed, and writes the record's "complete" generation. Every file is written as a `.farhelm-new.*` file beside its
destination and flushed before the rename; folders are flushed at each point a later step depends on (`F_FULLFSYNC` on a
Mac, `fsync` where that is unsupported). The flush of `MacOS/` before the Installed record is what an older app's
updater depends on: it judges success by that record, so the app program must be on disk before the record names the new
version. Stopping after any step leaves an app that launches the old version or the new one, the record saying the
install is unfinished until it is, and the next run finishes the job. Nothing is re-signed (re-signing could rewrite the
running program's file in place), and nothing is quarantined. Pruning keeps the new version, the one it replaced, the
one named by the Running record in the default state directory or in the one the installer's shell's absolute
`XDG_STATE_HOME` names, and any version whose lock someone holds (an exclusive no-wait attempt fails); every other
version folder is renamed, while that exclusive lock is held, into a trash folder beside the app, which is flushed and
then deleted. A trash or staging folder an interrupted run left is deleted only after the folders its moves touched (the
app's `Versions/`, `~/Applications` and the leftover itself) are flushed, since the run may have died before its own
flush. Clearing a killed run's staging, trash and marked lock folder at the next run, told apart from a live run's by
the installation lock, is the kind of start-up sweep "Cleanup on quit" says needs the maintainer's approval for the
specific case; it was approved as part of the race-free installer design (2026-10-11), because an install that is killed
or loses power has no quit path to clean up on. A Farhelm running with an overridden state directory is protected by its
version lock for as long as it runs. The Terminal link is placed last by creating a symlink at the public name, which
refuses an entry that arrived after inspection. The refusal leaves the app installed, names the untouched entry and asks
for another installer run; it also reports any earlier foreign file kept aside. Both recorded old-layout command copies
move to visible replaced names before hashing: only matching contents are deleted, while a replacement stays under its
visible name and is reported. The link's folder is flushed. The script then touches the bundle and re-registers it with
`lsregister -f`, because Spotlight and Launch Services otherwise keep showing the old version.

Before update or rebuild, after the updater's expectation comparison, the locked installer checks the existing
installation against that floor. Both update-only and Terminal invocations use this gate; a fresh install is allowed. A
pre-`Versions/` installation is known to predate the floor and is refused. A `Versions/` folder whose Installed record
is missing or unreadable is refused conservatively because the source version cannot be established; a readable
unparseable version remains exempt. Refusal uses status 76, distinct from the busy and changed-installation retries, and
tells the user that the data stays on disk, to run `farhelm uninstall`, then the installer, without promising the new
release can use every feature's old data. The documented manual removal fallback covers releases without uninstall, an
old uninstall that retains a recordless app, and a missing or unreadable Installed record that prevents the Terminal
command from starting uninstall. The gate precedes new update/rebuild writes; existing locked recovery of an interrupted
run's leftovers still happens before inspection, so the gate sees the recovered installation. The shell installer and
its permanent interface are unchanged.

The retained rebuild code builds an app at the usual place without `Versions/`, or no app at all, whole under a
temporary name beside its place, flushed, and renamed into place (moving any existing app aside first, and putting it
back if the rename fails); `~/Applications` is flushed after. An existing installation's id carries over; otherwise a
new one is minted. The script decides beforehand whether an app without this layout's record is one an older installer
built (its old record, or the recordless shape), which is the only foreign-looking app that is replaced. Because that
check runs before the lock, its approval names the folder it checked by inode (`ls -di`, read before and after the
check), and the locked work replaces only a folder with that inode: an app Finder puts at the place in the meantime is
refused, and a file there never counts as an older app. The rebuild moves the app aside before checking the moved folder
is the approved one, then puts back anything else. One narrow case stays open, accepted by the maintainer on 2026-10-24
rather than answered with durable records of what was approved: if Finder puts another app at the place in that instant
and the run is then killed before its check (or Finder fills the place again before the app can be put back), the next
run's cleanup deletes the folder moved aside, because only a moved-aside Farhelm is ever expected there.

The ownership record, `Contents/.farhelm-installation` (mode 0600), keeps the format name and the first two
NUL-terminated fields older installers read (`farhelm-app-v2`, then the Terminal link) and appends three: a random
installation id (32 hex digits), a generation, and a state, `complete`, `installing` or `removing`. The generation is
odd exactly while an install or uninstall is running or was interrupted. A two-field record, as every older installer
writes, means complete with no id; an install gives it an id when it first advances it. Older installers recognize the
extended record by its first two fields and rewrite it in the two-field form when they finish; older uninstallers, which
require exactly two fields, refuse it with their own "rerun the installer" advice. A run started by an older app's
updater, recognized by the checksum file it passes (`FARHELM_INSTALL_SUMS_FILE`), only updates: it refuses when the app
is missing, when the record says an uninstall was interrupted, or when the Installed version is newer than its target,
and it finishes an interrupted install, because an older app cannot show a notice telling its user to run the installer.

Version locks keep a version from being deleted while anything uses it
(`crates/farhelm-supervisor/src/version_lock.rs`). Every `farhelm` that runs from `Contents/Versions/<v>/farhelm` holds
a shared `flock` on that file from its first step until it exits, through one open file per process; whatever deletes a
version first takes an exclusive lock on the same file without waiting, and keeps the version if that fails. The kernel
drops a lock when the last descriptor sharing it closes, so a killed holder never leaves one behind. Each acquisition
checks afterwards that the locked file is still the one at the path (same device and inode), because a deletion renames
the version's folder away while it holds the exclusive lock; a program that finds its version gone, or still exclusively
held after a five-second wait, refuses with one line saying to run the command again (agent hooks refuse too, silently
and with status 0, since their stderr is the agent's terminal). Where the filesystem cannot lock, programs run without
one, as before. Programs outside the layout take no lock. One program in the layout takes none either: a terminal host
(`internal terminal-host`, "Terminal host"), started from the supervisor's own versioned program, lives for weeks, so
holding its version would keep pruning and uninstall waiting on every terminal ever opened. It clears
`FARHELM_VERSION_LOCK_FD` and closes every descriptor it inherited, a forwarder's lock included, and it never runs its
own program again; like a running agent, it needs nothing from its version once it has started. The program it runs
holds its version itself: an agent's through the hold step, a tab's through the forwarder (below).

The forwarder's contract, which a later installer may only replace by rename and only with a script that forwards every
older invocation the same way: it resolves its own real path (it is reached through `~/.local/bin/farhelm` from a
terminal) to find the bundle; when `FARHELM_SUPERVISOR_SOCK` is set and a `running-version` exists beside that socket,
it runs that version, and otherwise the Installed one; if the version the Running record names is missing, the
supervisor that wrote it is not running any more, and it uses the Installed version instead. Before starting the version
it claims it: it opens that version's `farhelm` on descriptor 8 (written `{ command exec 8<"$path"; } 2>/dev/null`, so a
failed open neither ends the shell nor prints anything, and the forwarder's own error output is untouched) and runs that
`farhelm` once as `internal version-lock 8` with all three standard streams on `/dev/null`. The lock that run takes
belongs to the open file, so it stays after the run exits. Exit status 0 means the version is held: the forwarder
exports `FARHELM_VERSION_LOCK_FD=8` and `exec`s the version with descriptor 8 still open, and the program adopts that
descriptor as its own lock (after checking it is its own file), makes it close-on-exec and removes the variable before
starting anything. Status 2 (a `farhelm` from before version locks rejecting the unknown command) and 69 (the filesystem
cannot lock) start the version without a lock, with descriptor 8 closed and no variable. Anything else (75: the version
is gone or still exclusively held) closes descriptor 8 and chooses again, up to three times, then prints one line and
exits 127. It `exec`s `Contents/Versions/<version>/farhelm` with the arguments, the caller's environment variables and
the standard streams as they are (the shell itself may add `PWD`, `SHLVL` and `_`, and a claimed start adds
`FARHELM_VERSION_LOCK_FD`). A missing Installed version also counts as a failed choice, since an install may have moved
Installed on and pruned that version after it was read: the forwarder reads Installed again, within the same three
tries, and prints one line and exits non-zero only when the version it read last is still missing. It never takes or
tests `supervisor.lock`.

The desktop app starts its managed supervisor from `Contents/Versions/<its own compiled version>/farhelm` when its
bundle has a `Contents/Versions/` folder, and refuses with an error naming the missing folder rather than falling back
to its sibling, which in that layout is the forwarder. `FARHELM_DESKTOP_FARHELM` still overrides the choice, and an app
without the folder still uses its sibling. The app is not itself a program in a version folder, so it holds that
version's lock (above) on the `farhelm` it chose from its own version folder, from before it probes for or spawns a
supervisor until it exits; that covers the time before its supervisor exists and every supervisor restart. A program
named by `FARHELM_DESKTOP_FARHELM` is never held, even inside a bundle. A version that is already gone, whether found
missing or removed before the lock was taken, refuses startup with one sentence saying to open Farhelm again, and so
does one still held exclusively after the five-second wait. The lock's descriptor is close-on-exec, so neither the
supervisor nor the updater's installer inherits it.

The app starts that supervisor detached, so it outlives the app, as a Linux supervisor runs under systemd: in a session
of its own (`setsid`), with stdin and stdout on `/dev/null` and stderr appended to `supervisor.log` in the state
directory (0600; a launch that starts a supervisor first moves a log of 16 MiB or more aside as `supervisor.log.1`).
Nothing stops it when the app quits, refuses at startup or panics. `--exit-on-stdin-close` remains a supervisor option,
used by tests; the app no longer passes it. The app records the supervisor it started in `desktop-supervisor.json` in
the state directory (`farhelm_supervisor::managed_supervisor`): pid, kernel start identity, and the resolved program
path with that file's device and inode, read before the spawn. This record is a deliberate ownership marker between
processes, which "Cleanup on quit" otherwise rules out without approval; the maintainer chose it on 2026-10-31 (the Mac
supervisor outliving the app), and it is the only thing that lets anything stop that supervisor. The record authorizes
an action only after the live process is read again and matches it: same pid and start identity, running the recorded
executable (on Linux also the running image's device and inode; macOS exposes only the path). A supervisor the app did
not start is never stopped by it.

At startup the app decides from the record and from discovery (`plan_local_supervisor` in
`crates/farhelm-desktop/src/lib.rs`). Without a matching record, an answering supervisor is reused as it is and only
absence starts one, as before. With one, the app's own supervisor is replaced when it was started from another program
file than the one this launch would start (by resolved path, device and inode, not version: every untagged build reports
the same version), and when it does not answer at all (holding the state directory without serving); otherwise it is
reused. The whole decision, and any stop or start, happens under the helm's `helm-token.lock`, taken before the record
is read or anything is discovered (waiting up to five seconds for a brief holder, as the helm does) and released just
before the launch's own embedded helm takes it: a launch beside an open app refuses without touching anything, and two
launches cannot act on what they saw before the other changed it. Every writer and remover of the record holds that
lock, uninstall included. A replacement does everything that could refuse first: the new supervisor's prerequisites (a
session program path the shell can use), a `--version` run of the program it would start (a file that cannot run would
otherwise look like a predecessor that does not answer), and opening its log. A discovery that fails outright counts as
not answering only for the app's own supervisor. One that runs the current program and does not answer, whether nothing
listens or its hello fails, may still be starting: before binding its socket it waits for the state directory or
migrates its database, and after binding it runs startup sweeps (the launch directory, the repository cache, attachment
reconciliation) before it accepts. So the app first gives it `MANAGED_SUPERVISOR_SERVING_WAIT` to answer, reuses it if
it does, and only then treats it as wedged. Interrupting it after the wait is acceptable because those sweeps survive
interruption (the repository-cache sweep renames before it removes); post-bind startup work that could not survive a
kill would change that. Then the app stops the old one with SIGTERM, waits `ORDERLY_STOP_GRACE` (the output-client
shutdown budget plus five seconds), kills it only after that, removes its record once it is confirmed gone, and starts
the new one, with `STATE_DIR_CLAIM_WAIT` covering the handover. Confirmed gone means that no thread of it can run any
more, not that its main thread reads as exited: on Linux an exiting process's main thread can read as a zombie while its
other threads are still exiting, after a kill or an orderly exit alike, and until the last of them has finished the
process holds the state directory's lock and its socket (connections to which the kernel then resets unanswered); a
successor started in that window came up read-only and never served. A stop that finds the process already gone, or that
sees it go on its own, waits for that too, and a launch whose record names a process that reads as gone waits a few
seconds for it to finish before it looks for a supervisor; a supervisor still exiting after a stop's few seconds refuses
the launch. The new supervisor's claim also treats a connection dropped before the hello as a holder on its way out, not
as one serving, for whatever else starts a supervisor without that wait. The log is rotated only then, when a supervisor
is about to start, and a rotation that fails falls back to appending. A supervisor started by hand in that window races
the new one for the state directory; if it wins, this launch fails to start, and the next launch reuses it. The record
is written right after the spawn, so a crash in between leaves a supervisor the app treats as not its own: reused while
it answers, and not replaced on an update until it exits. A long-lived supervisor keeps its version's lock, so pruning
keeps that version until the supervisor is replaced.

A session's agent terminal (its terminal host's child) runs the supervisor's own versioned `farhelm`, not the forwarder,
so in this layout the supervisor starts it as `<farhelm> internal hold <spec> -- <shell argv>`
(`crates/farhelm-supervisor/src/session_hold.rs`) to hold the version through the login shell's startup. The hold step
takes the version lock, starts a keeper (`internal keep-version`) and waits up to ten seconds for it to report ready,
writes `<id>.<generation>.held` beside the launch spec with its own pid, and replaces itself with the shell, which
inherits neither the lock's descriptor nor its variable. The keeper adopts the hold step's lock through
`FARHELM_VERSION_LOCK_FD`; it is detached by a double fork into a session of its own, with its streams on `/dev/null`,
ignoring SIGINT, SIGQUIT, SIGTSTP and SIGHUP, and carries the session's `FARHELM_SESSION_ID` and `FARHELM_AGENT_ID` so
stopping the session finds it. It watches the hold step's pid (kqueue `NOTE_EXIT` on macOS, registered before it reports
ready) and exits when that process exits or when `<id>.<generation>.launched` names that pid, which the launch shim
writes with its own pid once it holds its own lock. Hold step, shell and shim replace one another in place, so they
share that pid. A keeper that cannot start fails the session's start, recorded in the launch's status file like any
launch failure, and so does a hold step or shim that cannot hold its version. The marks follow the spec through the
startup sweep, a retaken create and delete; the startup sweep's removal of an unknown or superseded launch's marks is a
start-up cleanup the maintainer approved for this case (2026-10-24), as "Cleanup on quit" asks. On an orderly stop the
supervisor waits up to three seconds for the agent terminals it started to show a `held` mark naming their host child's
pid, including ones still being created when the stop begins. A terminal that has not got there by then, or whose keeper
is killed before the shim holds its own lock, is unprotected until it does; if an install removes its version in that
moment the session fails to start, visibly, and starting it again works. A shell whose startup files `exec` another
shell never reaches the shim, and its keeper then holds the version until the terminal closes. A terminal tab needs no
hold step: its host starts the tab helper through the forwarder, which claims the version before the helper runs (the
tab's environment names this supervisor's socket, so it picks this supervisor's version while that is there), and the
helper replaces itself with the tab's shell, which needs no version afterwards.

### The desktop app's updater

The updater is native code in the desktop app (`crates/farhelm-desktop/src/updater.rs`), in the same process as the
window, which reaches it through its `UpdaterHandle`. It has no helm part: no endpoint, no helm state, nothing a
standalone helm could see. Its network requests are the app's own reqwest calls. `update_readout.rs` beside it turns its
state into the words the window shows.

It is active only when the running program is the main program of `$HOME/Applications/Farhelm.app` (both sides
canonicalized; `app_bundle::bundle_contents_of_main_program` recognizes the layout), that bundle has
`Contents/Versions/`, and the compiled version is a release rather than a development build. `install.sh` only ever
writes that one path, so an app running from anywhere else would install every day and never see its own Installed
record change. The same gate keeps the desktop smoke test and Linux CI off the network. Outside it nothing is registered
in the context, and every surface behaves as before.

The latest stable version is found with a plain GET of `https://get.farhelm.io/latest` (https only, at most 256 bytes,
the probe's 30-second timeout): exactly one line, `v` and a stable release version, a final newline allowed. A
prerelease there is refused rather than installed, since `/latest` never names one, and so is anything else that is not
exactly that; either is a failed check. The probe is one function, so a later channel setting can replace it.

After every probe attempt, including a failed one, the worker fetches `/advisories` and `/advisories.minisig` under the
same HTTPS-only streaming cap boundary. Each request has a 15-second total deadline; the body cap is 64 KiB and the
signature cap is 4 KiB. Only an authenticated UTF-8 body with the fixed header replaces the last good snapshot in this
run. Body 404 is quiet absence; a missing signature beside a body, every other HTTP status or transport/body failure,
and authentication or content refusal are logged as no new information. None blocks an ordinary allowed update, starts
an offline streak or sets `needs_reinstall`. There is no advisory disk cache.

The snapshot maps bare stable versions to bounded plain-text withdrawal reasons. The worker refuses a withdrawn
`/latest` target before installation on either trigger. It warns independently when the running version was withdrawn,
including when an unwithdrawn update waits. A withdrawn installed version newer than the running one is a different
case: readiness and Restart to update are suppressed, the hover requires manual recovery before restarting, and neither
automatic nor on-demand checks replace it. The restart entry point also rechecks current eligibility, so a stale menu
cannot restart into a withdrawal learned since it rendered. Ordinary quit/relaunch can still open that version.

This deliberately does not infer safe downgrade from `Contents/Versions/running`. A supervisor can migrate its database
and then fail before publishing that record; an old or missing record is not evidence that no newer program touched
state. Automatic replacement needs stronger attempted-version evidence and is deferred. Supervisor startup, the meaning
of Running, the standalone installer, the helm, remote hosts and additional advisory directives are unchanged.

Every check starts with a locked read of the installation (`farhelm_supervisor::installation::snapshot`): the
installation lock on `~/Applications` taken without waiting and without the older installers' lock folder, the ownership
record and the Installed record read through the app's folder, and the lock released. Nothing is installed without a
readable snapshot of a complete installation. A busy lock ends the check as "busy": nothing installed, no failure notice
for an automatic check (which runs again at the next one-minute tick instead of the next day), and the reason for a
user's check; an installer that finds the lock busy past its own wait exits 74 (`BUSY`) and is treated the same. A
snapshot that cannot be read is a failed check, and so is a record that is there but cannot be read or is in a shape
this release does not read (it is not taken for "no installation", which would stop every update without a word); no
installation at the app's place installs nothing. Every way a check stops before installing still refreshes the release
advisories first. A record in the `installing` or `removing` state (an odd generation found with the lock free) means an
interrupted install or uninstall: the check installs nothing, still refreshes the release advisories, and publishes
`UpdaterState::interrupted`, which the readout shows with the reinstall warning's mark and the command to run; only a
later check that reads a complete record clears it. Otherwise the record's id (none for a two-field record), its
generation and the Installed version become the expectation the installer is run with, as
`FARHELM_INSTALL_EXPECT=<id or ->:<generation>:<installed or ->`. The installer's locked work compares it through the
same `Contents/` folder it then updates, before changing anything, and, if the installation changed, changes nothing and
exits 75 (`STATE_CHANGED`), which the updater treats like a busy lock. A two-field record matched by a "no id"
expectation is given its id by that run, so the id is never minted before the comparison. Installers from before this
variable ignore it.

At startup the desktop app reads its installation's record through its own app folder before it resolves its version
(`desktop/bundle.rs`). If its own version's `farhelm` is gone, it compares that with what is at the same place now: the
same installation id, a different Installed version, and that version's program present mean the version moved on while
the app was starting, and the app runs a helper that waits for it to exit and then runs
`open -n --env FARHELM_DESKTOP_REOPENED=<installation id>` on the bundle, then exits. A reopened app whose own
installation id is not the marker's (another installation was put at the place meanwhile) refuses, and one that is never
reopens again. The marker is for that app alone: it is removed from the environment of the supervisor the app starts
(and so from every session) and of the next instance Restart to update opens. A record that is gone or says `removing`
refuses with "Farhelm was uninstalled" and the install command, one that says `installing` refuses with "an install was
interrupted" and the command that finishes it, and anything else refuses with the plain missing-version sentence, which
keeps the marker text the installer checks.

Release tests in `releasing/mac-vm-test/` can select a stable candidate before `/latest` names it by launching the app
with `FARHELM_DESKTOP_UPDATE_LATEST=v<version>`. The updater captures it once as an `Option<OsString>` when building its
dependencies; automatic and on-demand checks share that probe. A present value goes through the same stable-tag parser
as `/latest`, and an invalid value (including non-UTF-8 or a prerelease) fails every check without falling back to the
site. Startup logs that the override is active; the UI wording is unchanged. This selects only a version: the
newer-version comparison, real download origin and verification chain below all remain in force. The installer already
strips every `FARHELM_*` variable, and the relaunch helper removes this override from its environment before spawning
the opener, so Restart to update returns the app to the ordinary `/latest` probe.

To install version X, nothing is trusted on TLS alone. The updater downloads `https://get.farhelm.io/vX/SHA256SUMS` and
its `.minisig` and verifies them with `farhelm_helm::verify_signed_sums`, the rules the helm applies to its own
downloads: a signature by any key in the compiled-in ring, and the trusted comment `farhelm vX`. It then downloads
`https://get.farhelm.io/vX/install.sh` and refuses unless its SHA-256 equals the signed `install.sh` entry (a missing
entry is a refusal too). Only then does it write the script and the verified checksum bytes into a private temporary
directory and run the script with `/bin/sh`, rather than piping a `curl` into `sh`, so verification comes first and a
failed download is an error before anything runs. The installer starts from the app's own environment with every
`FARHELM_*` variable removed (an inherited `FARHELM_INSTALL_TEST_BASE_URL` would redirect the download) and exactly two
set: `FARHELM_VERSION` to X, and `FARHELM_INSTALL_SUMS_FILE` to the verified checksum file, which the installer then
uses instead of fetching its own, so every archive it downloads is checked against signed hashes (and, from the
race-free installer on, `FARHELM_INSTALL_EXPECT`, below). `HOME`, `PATH` and proxy settings stay what the user's own run
would see. The installer's interface to the updater is a permanent contract, part of the site layout's contract
("Verification chain (D3)"): every shipped updater runs future releases' `install.sh` with exactly `FARHELM_VERSION` and
`FARHELM_INSTALL_SUMS_FILE`, and with a checksum file that may list more entries than the installer needs. A later
installer that renames or drops either variable, or rejects extra entries, breaks every installed app's updates. The
install step is one function that takes the downloader and the runner (`install_release`), which is the seam its tests
drive with signed fixtures. Its output goes to the app's log. Success is judged only by `Contents/Versions/installed`
naming the target version once the installer has exited, never by its exit status alone. The installer runs in its own
process group, with stdin from `/dev/null` and its output written to a file beside the script rather than to a pipe the
app reads, so that quitting the app mid-install neither kills nor signals it: an installer killed by a broken pipe would
skip its own cleanup and leave its lock behind, and every later install, automatic or by hand, would refuse until
someone removed it. The worker copies that file into the app's log once the installer exits.

An installer exit of 76 means the installed source cannot meet the target's upgrade floor. The updater maps it to the
existing lasting reinstall warning, with an uninstall-first remedy followed by the installer command; it is not a busy
retry. Older app releases keep their ordinary install-failure message and installer details in the log. Verification
failures use the same warning mechanism. Its ordinary clearing rules apply to either cause, and no new persisted updater
state or pre-download metadata check is added.

Version order uses the helm's SemVer precedence (`farhelm_helm::build_is_newer`), the same comparison the host list uses
for a host's build, so there is one rule for which build is newer.

One worker thread does all checks and installs, so they are single-flight across every trigger: an on-demand request
that arrives while a run is in progress joins it, and that run's outcome is then shown as the user's. The thread wakes
about once a minute, re-reads the Installed record (cheap, and how a terminal install shows up), and runs the automatic
check when automatic updates are on and 24 hours of wall-clock time have passed since the last check, or the clock now
reads earlier than the last check, so a corrected clock cannot postpone checks. A plain 24-hour timer would not do: the
timers on macOS stop counting while the Mac sleeps. The scheduling last-check time is kept in memory only, distinct from
the persisted probe-failure streak below; the check shortly after startup covers restarts. While Installed is newer than
the running version the automatic check still refreshes `/latest` and advisories, but installs nothing. Holding back
avoids a third version replacing the folder of the one still running (the installer keeps only Installed, the version it
replaced, and Running, and Running is recorded by a supervisor this app may not own). A check the user asked for still
installs over an unwithdrawn wait, accepting that rare case because the user asked; with the default state directory the
Running record still protects the version that is running. A withdrawn wait never takes this path.

The automatic-updates setting is `install_updates_automatically` in `desktop-client.json`, through that file's locked
read-modify-write. It is optional, and absence means on, so every existing file reads as on.

The same file holds optional `automatic_probe_failure_since`, serde's `SystemTime` object with UTC `secs_since_epoch`
and `nanos_since_epoch`. Absence means no streak; exact precision avoids showing a notice early at the three-day
boundary. The updater reads it once at startup, retaining the in-memory value as authoritative and writing changes best
effort through the existing atomic writer. A missing, unreadable or malformed file supplies no known streak; no
per-field tolerance is added. Failed writes are logged without stopping checks or undoing memory. Old clients ignore the
added field. Shared status holds the streak under its lock, and every write takes that lock before the file's lock,
including settings saves. Successfully changing automatic updates from off to on clears both memory and disk; an
in-flight failure cannot restore a stale start over that reset. A future-dated start or a clock moving backwards from
the preceding probe sample resets the start to now on a failed check. Gaps without probes do not start a streak.

The updater publishes one state, read by the app bar: idle, checking, installing a version, up to date, or a failed
check with its reason, plus the version Installed names. Only a run the user started, or an automatic run that a user's
request joined, shows its progress there; an unjoined automatic run does not publish its working activity. It can retain
an earlier failure notice while it works. The worker distinguishes probe, install and verification failures. A failed
automatic probe starts the streak if none exists and shows a notice only at or after three days. Any successful probe,
automatic or on demand, ends the streak, even if the subsequent install fails. A failed automatic install shows at once,
including downloads after a successful probe. The next check tries again on the unchanged daily schedule. A release that
fails verification (no key in the ring verifies its signature, its trusted comment names another version, or its
`install.sh` does not match the signed checksum): nothing is run, and the app cannot update itself, which the user must
not discover by accident. That sets a lasting notice in the published state, whether the check was automatic or the
user's; the readout carries a warning mark and its hover says this Farhelm can no longer verify its updates and must be
reinstalled with `curl -fsSL https://get.farhelm.io/install.sh | sh`. It stays while the app runs, until a later check
verifies and installs or finds nothing newer, or the Installed record changes to a version newer than the running one
(the user followed it, and the update marker now asks for the restart). It is held in memory only, so a relaunched app
shows it again once a check fails the same way, which with automatic updates on is the startup check. Network and HTTP
failures never set it. It is what an app more than one key rotation behind sees (see "Release signing key"). A
verification refusal creates no automatic inline failure notice. The outcome of a run the user saw (up to date, or
failed with its reason) stays in the hover until the next run starts. An automatically shown failure is retained across
later failed runs until a check succeeds; UI dismissal hides its inline words without losing the hover reason. Manual
progress and outcome take precedence over it. While an unwithdrawn update waits, the hover names the waiting version and
that a restart finishes the update; a failed check the user started then names both. The readout renders it through pure
functions (its class, glyph, hover text and inline status). "Update ready" is not a state of its own: it is Installed
being newer than the running version and not known to be withdrawn, whatever the updater is doing.

The version readout also shows checking/installing inline, spinning its trailing refresh glyph when it is a check
button, or the separate spinner when an update waits and the readout opens a menu. Both use the existing stepped
animation and stop moving under reduced motion; the progress words remain. It then shows up to date for about five
seconds or check failed until dismissed or another check starts; an automatic-origin failure instead survives
unsuccessful runs until a successful check or dismissal. An up-to-date answer yields to a waiting update; a failed
restart has no inline check outcome. The activity type is shared with presentation without changing the worker's
published transitions. The published state retains a notice identity, incremented when a request starts or first joins a
run or when an unjoined automatic failure qualifies, and whether a user's successful completion found an update waiting.
Repeated requests joining that visible run keep the identity; its completion does not increment it again. Nonqualifying
automatic failures and Installed-record changes do not create one or replace a retained failure's reason. A later
qualifying automatic failure gets a new identity, even for the same reason, so it reappears after dismissal. The
captured success clears with a new request, the next run, a restart failure, or a changed Installed record; it is
announcement provenance, not update readiness itself. Expiry and dismissal belong to the component, keyed by activity
and request identity rather than every published change: Installed-record or reinstall-warning changes must not stretch
the timer or revive hidden text. The same pair rejects a callback or dismissal from an earlier request, even when watch
delivery skipped checking between identical answers. Successful readiness has hidden "Update ready" words in the live
region while the red toggle owns the visible outcome. The live-region child is keyed by request so an identical later
phrase still replaces its text-bearing node. The status is a persistently mounted polite live region beside either
version rendering, with bounded, ellipsized text, the existing reduced-motion-aware spinner, and a keyboard-accessible
button for failure dismissal. Hover words and version marks are independent of this visibility state, and the web build
has no updater to mount it.

Restart to update spawns a detached helper in its own process group: a short `/bin/sh` script that waits, bounded at
about a minute, for the app's process to exit, then runs `/usr/bin/open` on the bundle, and gives up without opening
anything if the app has not exited by then. The app then quits the way closing its window does, which leaves its
supervisor running. The new main program finds the previous version's supervisor by its record, running another program
file than its own, and replaces it in order (see "Side-by-side versions inside Farhelm.app"); sessions keep running.
Terminals in terminal hosts keep running too: the old supervisor's orderly stop checkpoints them, and the new one adopts
them from their hosts ("Terminal host", "Across supervisor restarts"). The real update handover is exercised by the
release brick test. Opening the releases page uses the same external-link path as the Documentation item.

What it trusts over TLS alone is the answer to "which release is the latest". A wrong answer cannot make it run anything
unsigned, but it can withhold updates (by naming an old release), or, by naming one that fails verification, put up the
reinstall notice, whose recovery command trusts get.farhelm.io over TLS alone. Anyone who controls the site can
therefore get updating apps to ask their users to run that site's installer unverified, so the site's own integrity
still matters for installed apps, not only for fresh installs.

## Provisioning

Implemented in the helm over the common remote route, using system SSH or local/SSH-nested Podman exec. Stream the
target's `farhelm` binary, the one payload a host gets, into `~/.local/lib/farhelm/` through the same route constructor
every other step uses (`cat` into a nonce temporary, then a digest check before the rename; not sftp, whose own
destination grammar reads an IPv6 literal or an `ssh://` URI as a different host than ssh does). On Linux, write
user-level systemd units, `systemctl --user enable --now`, `loginctl enable-linger` as the optional-step
(proceed-without-if-privileged per SPEC.md). Discovery-first: probe for a running supervisor via
`farhelm internal stdio` before proposing any of this, and show the full concrete action list before touching the host
for initial setup.

Inspection chooses the OS with `uname -s` before any systemd command. The combined payload target is Linux x86_64, Linux
aarch64 or macOS aarch64; its serialized Linux architecture names remain `x86_64` and `aarch64`. Apple-silicon Macs
select the existing `farhelm-aarch64-apple-darwin.tar.gz` from either signed releases or staged payloads. Intel Macs
have no supported payload. Mac inspection requires the commands the native path uses, including a working `readlink -f`,
and refuses with the missing command's name rather than maintaining a macOS version floor or fallback implementations.
It also checks for `~/Applications/Farhelm.app` and `/Applications/Farhelm.app`: setup, Update and uninstall refuse when
either is present. Discovery still runs first, so an answering supervisor is adopted unchanged even on such a Mac.

The remote Mac's background service is `~/Library/LaunchAgents/io.farhelm.supervisor.plist`, loaded into `user/<uid>`
for the inspected SSH user, never a system daemon or a `gui/<uid>` job. Its `LimitLoadToSessionType=Background` permits
starting it from an SSH-only login. `RunAtLoad` starts it immediately; `KeepAlive={SuccessfulExit=false}` restarts it
after failure without restarting a clean exit. On the tested SSH-only Mac, the job was absent at the first post-reboot
SSH observation and throughout a further 30-second observation window; explicit setup recovery started it again.
Automatic loading at screen login is unverified, so setup tells users to choose Update in the host menu after reboot.
The plist's `ProgramArguments` launch the same foreground `farhelm supervisor run --state-dir <state>` with
`/usr/bin/env -u` removing `FARHELM_SESSION_ID`, `FARHELM_AGENT_ID` and `FARHELM_TAB_ID`, as the systemd unit removes
them below. Arguments are XML strings, never shell command text. Its PATH starts with the binary's directory, followed
by the Homebrew and system binary directories.

`AbandonProcessGroup=true` keeps terminal hosts and their agents and tab shells alive when launchd replaces or restarts
the supervisor. Setup bootstraps an absent job; an already loaded job must name the planned plist and is kickstarted
without killing a running process. Update removes the loaded registration with `bootout`, confirms it is absent and
bootstraps the new plist. Rewriting a plist alone would leave launchd's old loaded configuration in use. Neither path
uses `launchctl disable`, whose persistent override would prevent a later setup from starting the job. The plist has no
setup-owned marker because `farhelm helm setup` does not install a Mac job. Launchd actions are explicit members of the
same frozen action list as Linux actions, without a separate service-manager abstraction.

Every remote Darwin `stat` call uses the system `/usr/bin/stat` regardless of PATH, including uninstall.

Shared file transfers report their temporary file size with BSD `stat` on Darwin and GNU `stat` on Linux. Artifact mode
inspection makes the same platform choice. Mac uninstall uses the native mount table as best-effort protection: it
rejects only a mount read unambiguously on or inside the lib directory. Ambiguous, malformed or non-absolute lines are
silently skipped; an empty table or a failed read reports no mounts. An ambiguously named nested mount therefore goes
undetected. Linux retains its `/proc/self/mountinfo` check and read-failure refusal. The ownership, canonical-path and
shared-directory permission rules below still apply.

ADD and UPDATE both retain that concrete plan behind an opaque, one-use confirmation id. Planning is inspection-only;
consuming the id revalidates the host and registry facts the plan relied on, and only then admits the host-scoped run.
Setup shows the helm's ordered action descriptions in a numbered dialog and consumes the id on explicit confirmation.
Each path chip uses the helm's home-relative spelling where available, with the full host path on hover; Copy plan
copies the helm's exact plain-text confirmation, since selecting separate names and chips cannot produce that text. The
helm derives the dialog entries and progress entries from the same frozen action list, so the UI never supplies its own
action wording. A remote update consumes its plan automatically after the user's Update click, which is the
authorization — the wire keeps the same single-consumption authority either way, and the UI never replays a spent token.
Discovery records the resolved supervisor binary, state directory, and identity together so a later helm dials the same
installation that answered the probe. UPDATE starts from those recorded coordinates rather than assuming the standard
layout.

Update also checks the probed supervisor version against the shared upgrade floor at planning and at execution, on both
connected and protocol-skewed paths, beside the downgrade and newer-schema refusals. A single refusal helper takes the
floor as an input and uses the probed program and data paths in its manual recovery advice: stop the host's Farhelm
supervisor service, remove its dedicated Farhelm installation directory, optionally remove the data directory if losing
that data is accepted, then remove the host from Farhelm and set it up again. The reported executable can live in a
shared or custom binary directory: its parent is not assumed to be Farhelm-owned, and the advice says to remove only
Farhelm-owned files there, preserving the shared parent. A missing data path stays unknown rather than being inferred
from the executable. Development builds and unparseable versions stay exempt. No protocol field, host state or
automatic-update scheduling rule changes.

The supervisor-unit ownership rule in SPEC.md is enforced twice on remote Linux hosts. The reach check reports whether
the user-unit directory already holds a `farhelm-supervisor.service` whose first line is setup's managed-by marker (the
same test `units::is_managed` applies), and such a host gets the `SetupManaged` outcome every operation refuses on,
before any plan exists, each in its own words (installing or updating there is the host's installer and setup; removing
is its `farhelm uninstall`). The remote unit write repeats the test in the same shell command that renames the new unit
into place, and uninstall's unit removal repeats it in the same shell command as the `rm`, because a retained plan is
confirmed later and setup may have run on the host in between. The local executor branch has no such check: the panel
never installs a unit on the helm's own machine.

UNINSTALL is the third operation on the same machinery: `POST /api/hosts/{id}/uninstall` plans without a body and
consumes the plan's id from a body, like update's route, but the user sees and confirms the same numbered plan as setup,
with the kept-data note outside the action rail. On Linux its actions disable the supervisor unit, remove the unit file,
reload the user manager, stop the unit with its terminal hosts, remove the lib directory, and finally forget the host.
Removing the file before stopping keeps the supervisor answering until nothing can start it again: up to the unit file's
removal the host stays connected and the ordinary checks apply, and after it a retry may proceed without a connection.
Reloading after the file is gone makes systemd forget the unit's `KillMode=process` and use its default `control-group`:
the stop ends the supervisor and everything else in the unit's group (this default behavior was verified on a real user
manager when uninstall was built), which is the terminal hosts (see "Terminal host") and, on a host whose supervisor
never ended it, the private tmux server of an earlier release. The same stop action then ends the terminal hosts of the
host's state directory, which covers an inactive supervisor whose process-only policy left hosts alive before systemd
forgot its removed unit. Uninstall has no tmux step: a private tmux server that outlived a supervisor stopped before
uninstall (a host never updated to a release that ends it at startup, or one whose startup ending failed and whose
supervisor was later stopped) keeps running. The maintainer accepted that residual on 2026-11-11 when the step was
removed. The hosts are ended by the installed `farhelm internal stop-terminal-hosts --state-dir <state dir>`, run with
the binary the plan names, which is still in place because the lib directory goes after this step. The command runs
whenever the state directory's `hosts` exists, whatever it holds (the command's contract has callers decide by that, and
judges what is there) and whatever kind of entry it is (the command refuses one that is not a real directory); only a
missing `hosts`, an install from before terminal hosts, skips it. Only its exit status 0 counts; any other status,
including the 2 of a `farhelm` from before the command, fails the step with the command's lines and leaves the host
listed for a retry. After a 0 the step removes the socket files left in `hosts` (the command only notes one it could not
remove) and then `hosts` itself, so that the retry after the lib directory's removal, which has no `farhelm` to run,
finds no `hosts` and skips. Anything else in `hosts` (a file, a symlink, a directory), or a removal or listing that
fails, fails the step while the `farhelm` is still there, naming what is left. A `hosts` whose `farhelm` is already
gone, which only a removal from outside uninstall leaves, fails the step with how to end the hosts by hand. This helm
allows the command 30 seconds, plus room for starting it, on top of the bound of the step's other commands; that figure
is this build's own bound for the command, not a promise every release's `farhelm` keeps. The command accepts only an
absolute state directory, so planning refuses a host whose state directory is not one. The `farhelm` binary and the
state directory are frozen in the plan before confirmation. The disable command may reload implicitly; the explicit
reload after file removal ensures the same stop behavior on every retry, including one whose preceding stop failed. Only
the removals still outstanding are planned (the unit file's two steps when it exists, the lib directory's when it
exists), and each host command treats work already done as a skip, so a failed run continues from wherever it stopped
with no process-local memory of how far it got, which would not survive a helm restart. Every path comes from the plan,
frozen from the same layout and overrides an install uses, never re-derived at run time. A retry without a connection
also needs the unit no longer running (`inactive` or `failed` in the uninstall inspection): the probe reads a custom
command name that stopped resolving as no supervisor answering, so its evidence alone would let a retry stop a
supervisor whose sessions were never checked.

On a remote Mac, uninstall instead removes the plist, removes the exact frozen job with `launchctl bootout` and confirms
it is no longer loaded, then runs the same installed `stop-terminal-hosts` command before removing the lib directory and
forgetting the host. Bootout deliberately leaves the job's children alone, so explicit terminal-host termination is
required. A loaded job, even one with no running PID, is treated as active because launchd could start it; an offline
retry needs both the plist gone and the job absent. Inspection first verifies that the user's launchd domain is
available, and accepts only launchctl's specific missing-job answer as absence. Other command failures refuse rather
than passing for an already stopped job. A loaded job's plist path must match the frozen planned path, using the same
canonical-path and home-normalization guards as the Linux unit fragment. A stop failure or an unconfirmed terminal host
keeps the lib directory and host listed. The kept-data note and retry behavior are the same as Linux.

The checks run at planning and again under the run's lock at confirmation, by planning again and requiring the same
plan, so one piece of code decides every fact the plan rests on. A connected host needs a fresh session list (a live
round trip, never the cache) with no session that has not ended and none with a live tab, a list the supervisor did not
truncate, its supervisor service running (on Linux its unit, on a Mac its loaded launchd job; a connected supervisor
whose service is absent was started some other way, and removing its lib directory would leave it running from deleted
files), and the fresh probe reporting exactly the recorded identity, a missing one included, since the probe dials the
destination again and uninstall must not act on a machine it has not verified. The unit systemd loaded (`FragmentPath`),
or the plist launchd loaded, must be the planned service file after the home-only normalization below, including its
full-path same-file guard, so neither "no unit file left" nor the removal can miss a unit loaded from elsewhere. A host
that is not connected is refused when a supervisor answers there but cannot be used (version skew, an identity problem),
and otherwise needs the probe's positive evidence that no supervisor answers and its unit file or plist already gone;
when its lib directory is gone too, the registered binary is no longer checked, since there is nothing left to protect
and the check would strand the retry. The binary the probe dialed must lie inside the lib directory and the host's state
directory must not, compared as canonical paths the host resolves (`readlink -f`), since the lib directory is removed
with `rm -rf` and a state directory symlinked into it would otherwise go with it; a path the host cannot resolve
refuses. For every inspected path, including the unit fragment, the inspection walks from that resolved path toward the
root to find an ancestor that is the same folder (`-ef`) as the SSH login's resolved home. It replaces only that prefix
with the login home's spelling, and keeps the replacement only when both full paths name the same file (`-ef`). That
final guard matters for a non-recursive home bind with different nested mounts. An absent, empty, unresolvable or root
home leaves the resolved spelling unchanged; so does an unavailable identity comparison or a failed same-file guard. No
other folder aliases are normalized. The three guards still compare the resulting paths as text, preserving refusals for
shared unit folders, shared installs and hard-linked unit copies at a different path. Those comparisons cannot see a lib
directory that is itself a symlink, since the binary resolves inside the link's target, while `rm -rf` of the link
removes only the link; so the inspection also reports, per path, whether the path itself is a symlink (`[ -L ]`, which
sees a dangling link too), and planning refuses a symlinked lib directory before any other check of the inspection, the
resolution check included, so that a dangling link gets the refusal that points to Remove rather than "cannot tell where
it leads". The removal command repeats the `-L` test in the same shell as the `rm`, as the unit removal does for setup's
marker, and refuses a lib directory that became a link after confirmation as a failed step. A row with no recorded state
directory uses the supervisor's default as the host's own environment resolves it, `XDG_STATE_HOME` included. A session
started between the confirmation check and the stop is not locked out and may be ended with the unit. The
confirmation-time check is sufficient; nothing requires another check during the removal. Local `farhelm uninstall`
keeps its separate scope and behavior. The state check also compares the spellings from before home normalization. Each
path is respelled on its own, so the lib directory can come back in the home's spelling while the state directory inside
it keeps the alias spelling, because its same-file guard failed (below a nested mount the bind lacks, or for a state
directory that does not exist). The inspection therefore also reports each resolved path before normalization, and the
state check refuses when the state directory is inside the lib directory in either spelling. The state check also
refuses on the host's same-folder test, an identity answer it never uses to accept a path: the inspection reports, per
path, whether its unnormalized resolution or a folder above it is the same folder (`-ef`) as the lib directory, which
catches the lib and the state reached through two binds of the home with a mount over a folder above the lib visible
through only one, where no spelling shows the containment and no mount lies inside the lib.

The last action deletes the row from inside the run, which already holds the host's provisioning lock and is the host's
own run task, so it cannot go through Remove's entry point, which takes that lock and aborts the host's task. Both share
`ProvisioningService::delete_registered_host`, which takes the cache-write lock, deletes the row, purges the host's
progress, plans and busy marker (aborting the task only for Remove), stops the actor and forgets its lock. The helm
keeps no result for a deleted host, so the confirming client builds its success notice from the plan it already holds.
Uninstall's host actions have only an SSH implementation: the local row is refused at planning, and the direct-local
executor answers them with an error rather than an implementation nothing can reach.

The Hosts header's `Update all` button reads the current host snapshot and enqueues the same binding-captured UPDATE
request that each available SSH row's menu would send. The row's permanently mounted provisioning panel retains its own
intent, one-use plan, submission claim, and progress. Rows with no current Update offer are skipped, so an ADD in flight
cannot turn the header click into a delayed update after setup finishes. All eligible rows begin planning without
waiting for one another; a failed plan or run stays on its own row and cannot suppress another host's result. There is
no fleet endpoint or page-wide update result to reconcile against those host-scoped authorities.

The attention list's Update rows use the rail's narrower available-update classification, not the Hosts header's Update
all list: an offered remote Update, no live provisioning work, and either a compatible older build or a lower peer
protocol. The Hosts toggle still summarizes that update count and highest urgency. Each Update action sends the existing
binding-captured request for its displayed host; the attention header's Update all uses exactly its displayed Update
rows and appears only for more than one. No fleet endpoint or second provisioning owner is added. The header's Always
update requests those same displayed rows, then saves automatic remote-host updating through the same shared preference
queue and field identity as the Settings switch, so a later choice on either surface supersedes it. Its alert belongs to
the session list rather than to the attention list, which unmounts while Hosts is open, and shows only while the
preference still reads off.

The pure `hosts::attention_list` classifier reads the retained host snapshot, that update classification, per-host
`HostUpdateProgress`, `HostAttention::unseen()`, Tensorlake tunnel health, and the existing optional-update dismissal.
Progress wins over a problem, which wins over an update; the resulting rows are grouped as Problems, Working, Updates,
preserving registry order within each group. Pending and running setup/update/uninstall use their actual verb. Problems
cover the phases SPEC.md names, protocol skew without a usable update offer, resume and tunnel failures, and unseen
setup/update outcomes. Connecting, usable current/newer connections and ordinarily suspended hosts do not contribute a
row on their own. The classifier does not invent an unreachable-since time or change the attention owner's
acknowledgement rules.

ListView owns the progress and attention maps alongside the existing provisioning offer, request and busy maps. The
permanently mounted host panels still publish progress, consume requests and reconcile warnings and initial Add
receipts. Both visible surfaces read those same projections. Opening the attention row reveals the mounted Hosts panel,
then uses one animation-frame handoff to focus its existing menu button without scrolling before bringing the row into
view. It changes no host filter and adds no persistent scroll or highlight signal.

The required-update banner and optional-update card, their components, display helpers and CSS are removed. Their
page-local `UpdateDismissal` remains: it captures update host ids and the helm build, and applies only to optional
Update rows. A host outside the dismissed set gaining an update or a changed build invalidates it; required, Problem and
Working rows remain visible. Reminder identity uses the latest nonempty response build observation, separately from the
first-mismatch skew verdict. A late old reply can renew a reminder but cannot clear that verdict or restore unattended
reconnection.

The attention region is a bounded sibling above the session-content scroller; only its row region scrolls, including
from the keyboard, leaving its heading and header actions reachable. Hosts management conceals it without ending its
owner's lifetime. Its layout key joins the existing pre-render movement capture for session anchors: row identities,
groups, names and wrapped state copy retire only openings measured against the old layout. Progress counts, step text,
run identity and elapsed time do not enter that key, so routine progress polls cannot close an open session menu.

A supervisor that answers the hello but speaks a DIFFERENT protocol version is a distinct discovery outcome, not a probe
failure: presence is proven (only a live supervisor sends a hello) and the skew payload names its build, but nothing
after the refusal — in particular no identity — was exchanged. ADD registers such a host as discovered (identity
unknown) so it surfaces with the manager's version-skew state and the update action; UPDATE proceeds against it with the
recorded identity carried forward unverified, because the alternative — demanding an identity the old peer can never
transmit — would make exactly the host UPDATE exists for permanently un-updatable. The desktop's local discovery treats
it as answering (an ownership boundary, whatever its version). This outcome was originally misclassified as a transport
failure, which is how update-on-skew shipped broken until the first real cross-protocol update attempt (2026-09-01).

Be honest about what the relaxation costs: nothing in a skewed hello proves the peer is OLD — any process at the ssh
destination can send a wrong-protocol hello, and doing so exempts it from the identity checks a healthy peer faces at
update planning and confirmation. What such a peer gains is bounded: the update pushes payloads TO it (it learns nothing
from the helm beyond the binaries) at paths it named, ssh host-key verification is not weakened anywhere in this flow,
and adoption after the update still runs the full hello — a wrong identity freezes the host as identity-mismatch and
fails the run at attach rather than being taken over. The alternative — demanding identity proof an old peer cannot give
— is not a mitigation, it is the bug this outcome exists to fix.

Artifacts land under temporary names in their final flat directories and are atomically renamed into place. There are no
version directories or `current` symlinks: a failed transfer leaves the installed file intact, while a running binary
keeps its old inode until the explicit supervisor restart. Sessions on the host keep whatever the old binary handed
them, so the new one must accept it (see "What running sessions hold across versions"). Hash checks skip identical
payloads and service files are written only when their content differs, so rerunning provisioning converges from
wherever an earlier run stopped. Remote plans report binary upload and installation as separate actions. Upload verifies
the nonce temporary's digest; installation checks it again before the atomic rename. The upload's stall timeout (SPEC.md
"Provisioning transfers time out only on stalls") starts once the host's temporary file first exists and renews only on
its verified growth. The upload command reports that size on its own ssh stdout every two seconds and once at EOF; no
second command runs while the upload is active. Local writes into ssh are not progress evidence, including while the
last buffered bytes drain to the host. Before that file appears the upload has no deadline of its own: a slow ssh
connection setup has no byte-progress signal to tell it from a stall, and ssh runs without a connect or keepalive
timeout of its own, so a host that stops answering in that window holds the upload until the connection itself fails.
SPEC.md accepts that window (same section): ordinary ssh and TCP behavior bounds it, and Farhelm adds no deadline or ssh
timeout of its own for it. Both actions use the same staged payload snapshot, and local plans retain a single install
action because they do not transfer over the network. Matching content also repairs installed-file mode drift.
Provisioning may create directories with explicit modes and repair permissions on directories dedicated to Farhelm; the
supervisor state directory is private to its user (`0700`). Existing shared directories, including a shared executable
directory, the systemd user-unit directory or the Mac's `Library/LaunchAgents`, must retain their permissions. If those
permissions prevent installation, report the obstacle rather than changing them. Each plan's `EnsureDirectories` step
marks every directory dedicated or shared: a dedicated one converges on its mode with `install -d -m` (which chmods an
existing directory, and that is the point), while a shared one is handed to `install -d` only when it is missing, and
the confirmation text says an existing one keeps its permissions.

The supervisor runs no tmux. The supervisor unit, whether provisioning or `farhelm helm setup` renders it, names no tmux
and puts no tmux directory on `PATH`: its `PATH` is the directory of the Farhelm program it runs, then
`/usr/local/bin:/usr/bin:/bin` (`farhelm_helm::units::render_supervisor_unit`). The Mac app bundles none and needs none,
and hands its managed supervisor no tmux. `FARHELM_TMUX` is no longer passed to a terminal host's child (`host_launch`'s
`NOT_INHERITED`). Nothing this release starts or renders sets it, but a supervisor unit an earlier release wrote keeps
setting it until its next setup or Update; the supervisor ignores it.

Earlier releases' Linux provisioning required a host tmux at or above a version floor (3.7c at the end) and, on most
hosts, installed a private static build as `~/.local/lib/farhelm/tmux` to meet it. That directory holds the host's
`farhelm` too, so it comes first on the unit's `PATH` and on the `PATH` a session starts with
(`LaunchSpec.farhelm_bin_dir`), and a `tmux` typed in a session ran that copy rather than the host's own. Provisioning
no longer inspects, downloads, uploads or installs any tmux, and every Linux Add and Update plan removes that file
instead, as the step `remove-bundled-tmux` ("Remove the bundled tmux") between starting or restarting the supervisor and
connecting to it, so that it never deletes a program an earlier supervisor that is still running may use. The step is in
every Linux plan whether or not the file exists, because planning does not inspect for it and a missing file is a skip;
it always names the private lib directory's `tmux`, never a file beside a registered binary that lives elsewhere (a
shared bin directory may hold the user's own `tmux`). A remote Mac never had one and its plans have no such step. A
personal tmux server the user started from the bundled copy keeps running from the deleted file. The remote removal
looks the file up with `stat` in the C locale, like uninstall's stop step, and only the known missing-path diagnostics
count as absence, so a parent directory the user cannot search fails the lookup rather than reading as already gone. A
failed removal degrades the step instead of failing the run: the new supervisor already runs and the install does not
depend on the file, so the run completes with a warning that the file may have been left in place (a command that timed
out or lost its connection may still have removed it). Snapshot preparation skips the step, as it skips attachment and
lingering: the sandbox image it installs into has no Farhelm installation (preparation refuses setup-managed installs
and copied state), so it is not expected to carry an earlier tmux.

The Linux supervisor unit uses `KillMode=process`. Sessions started through Farhelm belong to their terminals, which
live in the unit's control group: each terminal's terminal host. systemd's default `control-group` policy would kill
them and every session whenever an explicit UPDATE restarts the supervisor. Limiting the unit stop to its main process
preserves the same ownership model as running `farhelm supervisor run` manually: stopping the supervisor detaches
management, while the terminals continue to own the session processes until the user deletes them, the host reboots, or
Farhelm is uninstalled from the host, which ends the terminal hosts on purpose.

The Linux supervisor unit also sets `UnsetEnvironment=FARHELM_SESSION_ID FARHELM_AGENT_ID FARHELM_TAB_ID`. The kill
sweep claims every process carrying a session's markers and has no exemption for the supervisor. A tab's startup file
that runs a bare `systemctl --user import-environment`, or `dbus-update-activation-environment --systemd --all`, copies
that tab's markers into the user manager; a supervisor started afterwards would inherit them, and deleting that session
or closing that tab would make it signal itself. A systemd-managed supervisor never legitimately belongs to a session,
so stripping the markers is free. This covers supervisors started through the generated unit (local setup and
provisioning share it); one started by hand from an environment carrying markers, and other user services that inherited
them, are not protected, and the sweep deliberately has no protected-process set.

The provisioning payloads — linux-musl `farhelm` binaries for both architectures and the Apple-silicon Darwin `farhelm`,
the only program a host is sent — are no longer embedded in the helm's own distribution (D2). This REVERSES the earlier
"provisioning must work with no third-party downloads" posture: a release-shaped build (D13 — one that embedded the web
UI) downloads them, on demand, from its own version's release on get.farhelm.io (`https://get.farhelm.io/v<version>/`,
whose archive links redirect to where the archives are hosted, followed over https only), verifies them, and caches them
under helm state before pushing them over SSH exactly as before. A release-shaped build of main, whose version is the
development sentinel (see "Version and skew"), has no release to download from and refuses by default, naming
`--payload-dir` and payloads built from the same commit (`UnreleasedPayloads`). A developer build defaults to no
payloads at all (`NoPayloads`, D13) rather than to a download, and `--payload-dir <dir>` (env
`FARHELM_HELM_PAYLOAD_DIR`) selects an operator-staged directory instead — files in an explicitly selected local
directory are treated as operator-trusted and are not verified, on ANY build, developer or release. The desktop app's
embedded helm takes the same directory from the same environment variable, so a desktop build from a working copy can
update hosts with target payloads built from that working copy (`scripts/devbuild-mac.sh`). The downloading source is
`ReleasePayloadSource` (`provisioning/release_payloads.rs`), which caches one release's assets, their extracted
binaries, and the signed checksum file under `<state_dir>/payloads/v{version}-<first 12 hex of sha256(base_url)>/` —
keyed by version so an upgraded helm never reuses the previous release's binaries, and by base URL so a mirror or test
server can neither read nor poison what the real release wrote. Motivation for reversing the embedding: bundling every
target's binaries inside every platform's own artifact cost tens of MB per download and coupled a payload's presence to
whichever platform happened to embed it; a download keeps the size cost where it belongs (paid once, by the host
actually being provisioned) while keeping the same "provisioned host runs exactly what the provisioning helm expects"
property, because the default download always names the provisioning helm's own version.

Verification chain (D3): every release is published on one origin, `https://get.farhelm.io`, in a fixed layout that is a
permanent contract with every client: `/latest` is one line naming the latest stable tag (never a prerelease; 404 before
any release is published); `/<tag>/SHA256SUMS` lists, in `sha256sum` format sorted by name, the four payload archives
(`farhelm` for x86_64 Linux, arm64 Linux and Apple-silicon macOS, and the macOS `farhelm-desktop`) and that release's
`install.sh` (releases before this one also list `tmux-x86_64-unknown-linux-musl` and `tmux-aarch64-unknown-linux-musl`;
clients look entries up by name); `/<tag>/SHA256SUMS.minisig` is the minisign signature over it, with trusted comment
exactly `farhelm <tag>`; `/<tag>/install.sh` is the installer signed with the release; and `/<tag>/<payload>` for each
payload is a temporary redirect to wherever the archive is hosted (today the GitHub release). `<tag>` is `v` plus the
version, prereleases included. The installer's two-variable interface to the desktop updater is part of this contract
(see "The desktop app's updater"). Installed software trusts a ring of public keys compiled into both shipped binaries
(`RELEASE_KEY_RING` in `release_payloads.rs`): a `SHA256SUMS` is accepted when its signature verifies under ANY key in
the ring and its trusted comment is exactly `farhelm v{version}` for the version being installed, and then each payload
must match its listed SHA-256. The helm checks its own version's release this way before provisioning a host;
`farhelm_helm::verify_signed_sums` is the one entry point for these rules outside the helm's own downloads. The trusted
comment is not decoration: the signature otherwise authenticates only the CONTENTS of `SHA256SUMS`, which name no
version, so whoever serves the release URL could replay an older release's valid manifest and assets at a newer
version's URL and downgrade every host that helm provisions. A release tag is `vX.Y.Z` and already carries the `v`, so
the comment is `farhelm` plus the tag verbatim. `--payload-dir` is the one path that skips all of this — nothing there
is downloaded, so nothing there is checked. Integrity never depends on where a payload redirect points, so hosting can
move without touching clients. Releases from before get.farhelm.io are not published there (their checksums were signed
by a key CI could read), and nothing falls back to GitHub for a release the site does not have. SPEC.md's "no public
relay, no third-party rendezvous service" line still holds: get.farhelm.io, and GitHub behind its redirects, are
download sources the helm's own machine reaches directly, never a relay or rendezvous point sessions or connections pass
through.

`/advisories` and `/advisories.minisig` extend that permanent site contract without changing `/latest` or any release's
checksum files. The signature is prehashed minisign, authenticated by any key in the release ring, with trusted comment
exactly `farhelm advisories`. `farhelm_helm::verify_signed_metadata` shares the checksum verifier's whole-ring and
purpose authentication; a release's `farhelm <tag>` signature cannot be replayed as advisories. The helm continues to
fetch only its own version's files and does not read advisories.

The UTF-8 file's first line is exactly `farhelm advisories`. CRLF and a final line without newline are accepted. There
is no format-version negotiation: unknown directives, empty lines and unknown content are ignored, so newer publishers
can extend it without disabling old clients. This release understands `revoked vX.Y.Z reason`: one ASCII space at each
field boundary, then the rest of the line a free-text reason. Only stable SemVer tags are accepted; prereleases, build
metadata and the development build are ignored. A nonexistent stable tag is allowed because the client has no release
registry. Known malformed directives are logged and ignored individually; a line over 4096 bytes is malformed. Last
valid duplicate wins. Reason control characters are removed and the result is capped at 240 Unicode characters; a blank
result is malformed. The 64 KiB whole-file cap bounds lines and allocation. The signature covers every byte, including
unknown lines, so truncation cannot turn into a valid partial snapshot. Invalid UTF-8 or a wrong header refuses the
entire replacement, while a verified header-only file lifts all known withdrawals.

Advisories retain D3's existing replay boundary. A site operator can withhold newer information or replay an older
authenticated snapshot; no sequence storage or freshness guarantee is added. Withdrawals guard against publishing
mistakes, not an attacker controlling the download site. The unsigned standalone installer is outside this mechanism.

Release signing key. CI never signs and no workflow step reads a signing key: agents working on this repository can push
to main and run workflows, so any key CI could read would be a path from an agent's push to code running on users'
machines. The release workflow builds and validates a release and publishes its archives on GitHub (`sign-sums.yml`,
which keeps its name but only validates); the maintainer then signs its `SHA256SUMS` and publishes the release on
get.farhelm.io from a trusted host, with tooling that deliberately lives outside this repository so that agents cannot
author it. That tooling refuses to sign a release unless every ring key string appears verbatim in every archive, which
is why each key stays one verbatim string literal in the source. The key previous releases were signed with was held as
the `MINISIGN_SECRET_KEY` repository secret; that secret has been deleted, so no workflow can read it. The public keys
are committed twice — `RELEASE_KEY_RING` and one `.pub` file per key beside `release_payloads.rs` — with a test that the
two agree in both directions.

The ring holds two keys, a primary and a backup, generated on the maintainer's trusted host; `RELEASE_KEY_RING` lists
the primary first. The key CI held is in no ring: releases built from then on do not trust it, while helms of earlier
releases keep trusting it for their own version's payloads. Releases are signed with the primary; the backup is compiled
in but kept offline. Rotation follows from two facts: the helm verifies its own release with its own ring, so a
release's signing key must be in that release's own ring, or the release cannot provision hosts; and an app updates by
verifying the next release with the ring it already has, so the signing key must also be in the ring of the release
before it. A rotation therefore replaces at most one key per release: to retire the primary A of ring {A, B}, the next
release carries {B, C} and is signed with B, and B becomes the primary. A compromised key is retired the same way,
promptly. An app more than one rotation behind cannot verify the latest release and must be reinstalled with
`curl -fsSL https://get.farhelm.io/install.sh | sh`, which trusts get.farhelm.io over TLS and needs no key; that is also
the recovery if both keys are lost. Consecutive rotations are therefore spaced so that daily automatic updates have time
to carry apps across each one. Note also what the keys do not protect: `install.sh` run by hand on a fresh machine has
nothing to pin a key in, so it trusts get.farhelm.io over TLS and the `SHA256SUMS` it serves; the signature guards what
a running helm provisions onto other hosts and what an installed app updates to, not the first download of Farhelm
itself.

A release also carries cargo-dist's own metadata on GitHub, none of which is signed and none of which Farhelm reads:
`dist-manifest.json`, a `<archive>.tar.gz.sha256` beside each of the four archives, and a lowercase `sha256.sum` over
what dist built. That last one is worth naming explicitly because it looks like the file that matters and is not it:
`SHA256SUMS` — uppercase, on get.farhelm.io, the one `SHA256SUMS.minisig` authenticates — is what the helm, the updater
and `install.sh` verify against. The metadata is nonetheless part of the release contract rather than incidental: the
validation job REQUIRES the manifest and the four per-archive checksums to be present as stable, machine-readable
metadata for downstream tooling, and treats `sha256.sum` as optional. Homebrew distribution remains deferred, with no
publishing model selected. Anything else appearing on the GitHub release, a `SHA256SUMS` or its signature included,
fails it.

## Cross-compilation and targets

Linux supervisor artifacts: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, static, built by cargo-dist on
a native runner per architecture with `musl-tools` for the C half of the link (audited: rusqlite-bundled static musl
builds work for both). Cross-compiling them with cargo-zigbuild, which the retired hand-written release workflow did, is
no longer necessary now that GitHub's arm64 Linux runners are generally available; zig remains the toolchain for
devbuild's Linux cross-builds on a Mac. The Mac artifacts (`aarch64-apple-darwin`: `farhelm` and `farhelm-desktop`) are
built on a macOS runner with the native toolchain. The Darwin `farhelm` is also the remote Mac supervisor payload;
`farhelm-desktop` remains local to the desktop installation. Cross-building them from Linux was audited and rejected:
zig cannot link the Apple frameworks wry needs (WebKit, AppKit) without a licensed macOS SDK, and cross ships no darwin
images. Dropping the `.app` bundle removed a third reason (`dx bundle` produces bundles only on the native platform) but
not those two, and it added one: `farhelm-desktop` is built through `dx build`, which patches the asset names into the
linked binary and so has to run where that binary is linked. The macOS runner stays. Signing and notarization are
deferred (D11) and the artifacts ship unsigned, so signing is not among today's reasons — it would become one if that
changes.

Motivation: musl-static sidesteps glibc version skew across Ubuntu releases, which matters because provisioning drops
binaries on machines we do not control; "predicated on whatever cross-compiled binaries are available" in SPEC.md
becomes exactly this target list.

## Testing

The testing story is a first-class requirement — agents verifying GUI behavior without a human is a standing project
constraint (see the GUI section's motivation), not an afterthought:

- **A fake agent** — `farhelm-fixtures fake-agent --script basic|altscreen|mouse-modes|spawn|agent-relay`, from the
  `farhelm-fixtures` package, a test-only binary that is never shipped (the product binary carries no fixtures) — stands
  in for Claude Code/Codex across this suite's integration and e2e tests. Its deterministic scripts cover prompt/echo
  input, terminal modes, alternate-screen rendering, and mouse-mode reporting, without vendor auth. Later milestones
  extend this fixture with fake on-disk records for status heuristics, conversation capture, and resume. The
  `agent-relay` script goes further than the rest: it reads the `SessionStart` hook out of its own launch's injected
  `--settings` argv, runs it, obeys the pointer line that hook prints by running `farhelm agent instructions`, and then
  serves `$farhelm ...` requests typed into its terminal by running the shipped `farhelm agent` verbs. Everything on
  either side of one link is the real product; the single stand-in is what DECIDES a conversation started, which is the
  part CI cannot have. The spawn suite also has an automated real-Claude leg that creates a jj workspace and spawns into
  it; CI leaves it gated because vendor credentials and network access are absent, and a developer enables it manually
  with `FARHELM_REAL_AGENT=1`.
- Rust integration tests exercise the supervisor with real terminal hosts and the framing protocol with golden cases;
  farhelm-proto keeps wire compatibility testable.
- **Terminal hosts are tested against real hosts on real ptys.** The host crate has a test bar of its own
  (`crates/terminal-host/TESTING.md`: real-pty tests of the built binary, golden fixtures of every frame of every
  protocol version, property tests, at least 95% line coverage and mutation testing; the suite also runs in the focused
  macOS CI job), higher than the rest of the workspace's because a running host outlives every release that could fix
  it. The supervisor's side starts real hosts from the built `farhelm`, never inside a test process, since a host forks;
  the emulator is held to Farhelm's answer table, screen-text format and modes by its own tests (the recorded agents'
  startup probes among them), the status readers by the captured agent screens painted into it, and its snapshot round
  trip by property tests. `.agents/validation.md` names the selections.
- **A CentOS Stream 9 container stands in for a host that is not the CI runner.** Provisioning accepts any Linux host
  with a usable systemd user manager, but every provisioning integration test dials `localhost`, and both CI and the
  release gate run on Ubuntu — so the case the retired `ID=ubuntu` gate existed to forbid, a helm installing onto a
  different distribution, was the one case nothing covered. `scripts/test-provision-centos.sh` boots
  `quay.io/centos/centos:stream9` with systemd as PID 1, publishes its sshd on loopback, and points the existing ssh
  provisioning test at it through an alias in the user's own ssh config, so the transport, the PAM stack, the user
  manager, and `/etc/os-release` under test are all the container's. What it pushes is what a release publishes and the
  helm pushes to a host: the musl-static `farhelm` for `x86_64-unknown-linux-musl`, put through
  `scripts/check-static-elf.sh` first. That choice is forced — the workspace's glibc debug binary cannot exec on CentOS
  9's older glibc — and it is also the point, since it makes this the one test of the artifacts users actually receive
  on a machine that is not the one that built them. Known limit: the container runs under the host's SELinux, and the
  host is an Ubuntu runner where it is not enforcing, so RHEL-family hosts with SELinux enforcing remain untested.
- The desktop shell's native glue is the acknowledged manual-test gap (see GUI risks); everything else must be coverable
  without a human.

**Development tooling assumes a single-user machine.** Decided 2026-10-10. The test harnesses, the capture and recording
scripts, deflake and the docs preview run on a development machine with no hostile other local accounts, and need not
defend against one: a secret in a process's arguments, an X display without authentication, or a race with another
account's symlink are accepted. Within one run, tooling may signal a process number it recorded earlier in that run
without rechecking it; keeping a number across runs (a daemon's process-number file, for example) is still not accepted.
Tooling may recognise its own files and processes by naming convention, such as a run's private path in a command line
or the demo recorder's still-image names, and may act on whatever matches. The agent-screen capture tool keeps skipping
author-name parts shorter than three characters to avoid false matches, with the documented manual read of the diff as
backstop.

**A checkout runs one harness or tooling command at a time.** Decided 2026-10-11. This includes browser tests,
screenshot and video capture and publishing, deflake and similar tooling. The docs preview server, which one agent
leaves running beside its own commands, is not a second run in this sense. Keeping concurrent agents in separate
checkouts is the maintainer's responsibility; tooling need not tolerate overlapping runs in one checkout.

## Version and skew

One version number across the workspace; the protocol hello carries protocol and build versions; incompatibility refuses
with a clear error at the edge (helm↔supervisor connect, client↔helm load) per SPEC.md. Once a hello is compatible, the
helm compares the peer and its own build strings as semantic versions for an advisory age signal: prerelease ordering
applies, build metadata does not change precedence, and an unparsable value leaves age unknown and false. That signal
adds `old_version` and `newer_version` to the connected REST state, without changing the `connected` phase or
operational routing; the incompatible protocol case is displayed as `Needs update`, or `Too new` when the peer's
protocol is the higher one. A development helm (version `0.0.0` with a prerelease, what every build from source reports)
never calls a connected peer newer and never refuses Update as a downgrade, since it sorts below every release; a skew
is judged by protocol versions alone, so a skewed host with the higher protocol reads `Too new` there too. Protocol
version bumps with any incompatible change — which includes a field whose omission changes what the receiver DOES, not
only changes to frames and message sets. A serde-additive field can still be semantically load-bearing: the
non-displacing attach is the worked example (a peer that ignores it displaces a client it was asked to leave alone,
silently, on both ends), and decode tolerance is why such a bump is required rather than why it is unnecessary.

Outside the release commit a tag points at, that number is the fixed sentinel `0.0.0-unreleased`, and release commits
never land on main. An untagged build therefore never claims to be a release: its version readout says so, and a
release-shaped build refuses its default payload download before any request instead of provisioning hosts with some
real release's binaries. The refusal says it is an unreleased build and to pass `--payload-dir` with payloads built from
the same commit, because a 404 for a release that will never exist would tell the user to retry. Any `0.0.0` version
with a prerelease counts as such a build. `--release-base-url` is still honored on these builds (D18), since it names a
source explicitly rather than this build's own release. The cost is that two untagged builds carry the same build stamp,
so the client↔helm check below cannot tell them apart; installs come from tags, which always differ.

The client↔helm edge has no hello to refuse at, so the helm stamps its build on every reply and the UI compares it
against the one compiled into its bundle. A mismatch — including a helm that reports no build at all — surfaces a reload
prompt and, more importantly, withdraws every UNATTENDED behavior that depends on the helm honoring this milestone's
vocabulary: the terminal heartbeat and automatic reconnect both stop, while anything the user explicitly asks for keeps
working. The sidebar's app bar shows the helm's build at all times: the client's own compiled build until a mismatching
stamp is reported (agreement means the two are the same string), and the reported stamp from then on. The bar leads with
the Farhelm wordmark, inlined at compile time from `packaging/farhelm-desktop/wordmark-dark.svg` (the brand file every
use of the name as a mark shares) rather than served as an asset, so both the web bundle and the desktop build carry it
without a desktop asset-parity entry. Settings and help live at the host rail's foot, while AppBar retains their state
and dialogs. The top band spans the full sidebar and preserves macOS traffic-light clearance and drag handling; the rail
starts below it. Its avatars read the existing retained host snapshot and provisioning progress, and send context
requests with the actual opener to the existing keyed HostRow menu. No second action set or host reader is mounted. The
content column scrolls independently of the rail and footer. Long versions ellipsize before the wordmark shrinks. In a
desktop app whose updater is active, the version is also the update marker: grey as always, or red with a one-character
up-arrow before it while Installed is newer than the running version, and then a button that opens the update menu
(Restart to update, What's new). Otherwise the active updater's readout is a native keyboard-reachable check button
calling the same `check_now` as the help menu and host row. Its dim refresh glyph brightens with the version and turns a
quarter on hover or keyboard focus; during a visible check that glyph owns the spin, so the inline status suppresses its
separate ring. The web/no-updater branch stays a plain span. A verification warning keeps its leading mark and check
action. Its hover text carries the updater's state, so a check the user started ends visibly there. The update menu and
the `?` menu are one bar-menu component given different items.

The rail's shared notification bell occupies one 40px row above Hosts; footer height and the rail's bottom offset are
both 208px. It reuses BellIcon at 16px and the existing danger-strong token for unread fill and count. ListView owns its
notice reader and close-time exact shown IDs. Toggle and panel render through separate AppBar slots so the panel sits
outside the footer's stacking context and rail-button styles. Existing popover registration, measured placement, outside
dismissal and guarded focus handback own its opening. Successful read marks fence older GETs before updating only
acknowledged IDs; failure leaves shared read state unchanged until another explicit close. Clear re-reads actual server
history instead of hiding rows optimistically. A narrow callback published by permanently mounted TrashControl opens
that same dialog or freezes a current stuck single-ID Retry through its existing dispatch. It supplies no second cleanup
owner or generic command bus. Open Trash first reveals session content and remembers the visible bell for modal return
focus, including while marking conceals the ordinary Trash trigger. Retry callbacks outlive the panel because ListView
owns them; acceptance is not notice resolution. A helm-only notice-save warning is displayed separately from archive
rows and selectable diagnostics, preserving known totals and ordinary completion polling.

The bar's hover is selected by a pure function of the window build, any reported helm build and the updater's readout. A
mismatch names both builds; a development window identifies itself as a development build rather than a release. Without
an updater a release window only names its version. A check button's hover leads with its action before the updater's
status, so a reinstall command still ends the text. An idle installed app also explains that the readout turns red once
a newer version is installed and offers the restart menu then. The development-build predicate lives in `farhelm-proto`,
shared by web wording and the helm's native release decisions, so their classification cannot drift.

Terminal hosts add version surfaces that the protocol version above does not govern, because a running host outlives
every release: the host's own protocol, which only grows and which every later supervisor must speak with every host
ever shipped (`crates/terminal-host/SPEC.md`, "Change policy"); the client tag's encoding and every snapshot format,
which every later supervisor reads for good, with golden fixtures per version ("Terminal host", "Terminal snapshots");
and `farhelm internal stop-terminal-hosts`, whose command line and exit status 0 a newer helm relies on from an older
installed `farhelm`. A supervisor that cannot read a host on one of these counts (an older supervisor after a downgrade
meeting a newer host, tag or snapshot) leaves the host running and its terminal unavailable, never gone. A downgrade of
the supervisor itself is fenced by its database instead: schema 32 stores each session's terminal kind, and schema 33
came with the flag that turns hosts on, so every build that could misread a host row refuses the database (SPEC.md,
"Upgrade compatibility and client scale"). The grid stream is not one of these surfaces: it is part of the
helm↔supervisor protocol and changes with its ordinary bumps (protocol 68 added it), in the directions above. An older
helm shows a supervisor at protocol 68 as `Too new` and must itself be upgraded, never the supervisor downgraded, and a
helm at 68 shows an older supervisor as `Needs update`.

### Rootless Podman sessions

New rootless Podman environments follow [Disposable environments](#disposable-environments). Their supervisor runs
inside the container, and every session and terminal uses its ordinary host paths.

The earlier supervisor-outside model is retired. Keep decoding stored `ContainerConfig`, including legacy `image_file`,
so sessions, templates, recent setups and history remain readable. A retired session supports Delete only. Refuse all
other lifecycle, terminal and exec paths, including plain Replace. Drop or refuse retired container selections when
using a stored setup, with an explanation. No path creates another old-style session.

Retain only the legacy `podman rm -f` removal, confirmed-absence check and lock/ticket handling still required to
serialize deletion with an already-started creator. Resolve Podman lazily. A host without Podman has nothing left to
remove. Other unconfirmed cleanup keeps the session for retry. Remove creation helpers, account selection, launch fields
and switches, hooks-off branches, special tab/attach and file-confinement paths once nothing can create or open these
sessions. Normal container environments require no supervisor-side Podman implementation.

### What running sessions hold across versions

A newer `farhelm` must accept everything a running or resumable session was handed and will hand back to whatever
`farhelm` it reaches. Sessions outlive the binary that started them: an update replaces the installed `farhelm` while
sessions keep running, and once Farhelm restarts on the new version, the sessions it adopts and the conversations it
resumes reach the new binary with whatever the old one handed them. Nothing re-issues those values, and the protocol
hello does not help, since no handshake stands between a session's command line and the program it ends up running.
(State the supervisor keeps for itself, such as its database, its launch records and its scope unit names, follows its
own migration and adoption rules instead. A terminal host is the exception that outlives every binary: its protocol, its
client tag and the snapshots it keeps are permanent surfaces with rules of their own, in "Terminal host" and "Version
and skew".) So the following are a compatibility surface, held to the same care as a wire format:

- The hook command lines and their flags: `farhelm internal hook --vendor <adapter>`, with or without `--announce`, as
  injected into Claude's `--settings`, Codex's `-c hooks.…` overrides and Muse's `-c hooks=…`, and as the user installs
  it for Grok (see "The per-launch identity hook" under Supervisor internals), and the outer timeouts those declarations
  carry, which bound how long a newer hook, and anything in front of it, may take. Sessions launched before an earlier
  upgrade still hold Claude's or Codex's old five-second limit, which a hook that only writes a file fits easily; only a
  vendor holding stdin open could push one past it. The supervisor's recognition of those shapes counts too: hook
  attribution for Claude, Codex, Muse, Grok and OMP matches the reporter's `<farhelm> internal hook …` argv and its
  `sh -c` trampoline syntactically, never by path, so that an upgraded supervisor still accepts an older hook. Those
  command lines run whatever binary is installed when they fire, so an already-running session's hooks start writing
  report files after an update with nothing re-issued; the report file format is versioned, and a supervisor refuses a
  version it does not read (the brief update gap described under "Report files").
- Goose's stored reporter declaration, in full: the extension name `farhelm-reporter`, its
  `sh -c 'exec "${FARHELM_GOOSE_REPORTER_EXE:-farhelm}" internal goose-hook'` command, and the fallback to `farhelm` on
  `PATH`. Goose replays it on every resume, including a manual one long after the Farhelm session is gone: Farhelm adds
  no reporter to a resumed launch and relies on that stored declaration, which reads Goose's own `AGENT_SESSION_ID` as
  the conversation it reports.
- The Pi and OMP reporters: the `internal hook --vendor pi|omp` invocations their assets spawn, the JSON those assets
  write to the hook's stdin, and the materialized asset files a running launch loaded with `-e`, which is why a new
  asset version is published beside the old ones, never over them.
- The environment a launch sets: `FARHELM_SUPERVISOR_SOCK`, `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, the reporter
  variables (`FARHELM_GOOSE_REPORTER_EXE`, `FARHELM_GOOSE_REPORTER_ENABLED`, `FARHELM_GOOSE_INSTRUCTIONS`,
  `FARHELM_PI_REPORTER_EXE`, `FARHELM_OMP_REPORTER_EXE`), the process markers (`FARHELM_AGENT_ID`, `FARHELM_TAB_ID`)
  that a restarted supervisor uses to decide what stopping, restarting, deleting or closing a tab cleans up, and the
  directory holding `farhelm` that a launch puts first on the session's `PATH`. Names, accepted values and meanings all
  count, and so does any `FARHELM_*` variable a launch sets later. This binds Farhelm's own binaries to each other; it
  does not turn these variables into a contract for users, which SPEC.md limits to the session id and credential.
- What agents were taught: `farhelm spawn` and the `farhelm agent` verbs, with their flags and aliases, and the output
  the instructions tell agents to read (for example the `--json` fields and values they name, the `*` that marks the
  agent's own session or host, the session id that spawn, create and clone print, and the flags a refusal tells them to
  add). The instructions pointer and `farhelm agent instructions` put that text in the agent's context, and in a resumed
  conversation's, so the agent keeps typing and parsing what it was told after the binary behind it has changed.
- In the Mac app's side-by-side layout (see "Side-by-side versions inside Farhelm.app"): the forwarder at
  `Contents/MacOS/farhelm`, which every session's paths above lead to, and the Running record `running-version` beside
  the supervisor socket, one line naming the running version. A forwarder written by a newer installer reads records
  written by older supervisors that are still running, so the record's name, place and format stay as they are. The
  forwarder's version claim binds versions to each other the same way: every later forwarder may run an older or newer
  `farhelm` as `internal version-lock <fd>`, so that command line, its exit statuses (0 held, 75 choose again, 69 no
  lock support) and `FARHELM_VERSION_LOCK_FD` keep their meaning, and status 2 keeps meaning a `farhelm` without version
  locks.

Every path among those values (the hook's program, the reporter variables, the `PATH` entry, the `-e` asset, the
supervisor socket) must keep naming a working `farhelm` or file after an update. An installer, a change of layout, or a
cleanup of old versions may not move or remove one while a session can still reach it.

In practice: add new spellings beside old ones rather than in place of them, never make an optional argument required,
and never give an existing spelling a different meaning. An old spelling may become an alias or a no-op, but it keeps
being accepted; retiring one is a decision to surface, not a cleanup. Anything placed between a session and the binary
it reaches, such as a launcher that picks which installed version to run, passes arguments, environment, stdin, stdout,
stderr and exit status through unchanged, prints nothing of its own when it succeeds, and replaces itself with that
binary (`exec`) rather than running it as a child: hook attribution walks the reporter's process ancestry to the
session's agent terminal's root process, and an extra process in that chain is an unclassified intermediary that gets
the report refused. Reviewers check every change that touches any of the above against this section. The launch
description a supervisor writes for its launch shim (the `LaunchSpec` JSON under `launch/` and its status file) is
outside this surface: in the Mac app's versioned layout the shim is the supervisor's own versioned program, which an
update never removes while that supervisor runs (see "Side-by-side versions inside Farhelm.app"). On a Linux host,
provisioning renames a new binary over the running one before it restarts the supervisor, and a session launched in
between runs the new shim against the old supervisor's description; this section does not cover that window.

SPEC.md (Durability and resume) has compatibility decided with the maintainer feature by feature; this section is that
decision for what sessions hold, and it applies from here on. Breaks already decided stay as they were: hooks that
predate the required `--vendor` discriminator fail closed (see "Ownership provenance and the offer gate"), sessions
launched before the spawn credential existed must be restarted before an agent in them can use `farhelm spawn` or
`farhelm agent` (the refusal says so), `farhelm agent restart --mode` is refused because every restart now resumes
(SPEC.md, Agent-spawned sessions; the refusal says to drop the flag), the `restart_offer` values `fresh_only` and
`fallback_template` are gone from `farhelm agent sessions` (its JSON envelope moved to schema version 3; `resume` is
unchanged), agent profiles are removed with no conversion (SPEC.md's launch-kinds upgrade paragraph;
`farhelm agent profiles`, `farhelm agent create --profile` and `--profile-id`, and `farhelm spawn --agent <name>` and
`--profile-id` are refused with a message naming what to use instead, a session's `agent` in `farhelm agent sessions` no
longer carries a profile name and the JSON envelope moved to schema version 4, and a profile-backed create's idempotency
key from before the removal is refused rather than replayed, see "Launch-kinds reservations"), launch kinds retire
`farhelm agent create --invocation` and the REST create body's `invocation`, `agent_kind` and `resume_template` (each is
refused naming `--command` or `command` and its fields; `farhelm agent sessions` may report the new offer
`no_resume_command`, and its JSON envelope moved to schema version 5), plain Replace, `farhelm agent clone`,
`farhelm spawn --inherit-agent` and Restart with refuse a session from before launch kinds with a remedy, and every
create-idempotency key from before launch kinds is refused rather than replayed (see "Launch-kinds reservations"), and
OMP admission accepts only the current binary's reporter asset. The last one keeps biting: any change to the OMP asset's
bytes or name makes every OMP session started before it lose conversation tracking until relaunched, so such a change is
exactly the kind of retirement this section asks to be surfaced. Asking the user before agent actions (protocol 41)
retired three more. `--confirm-yolo`, and its alias `--allow-yolo-on-sensitive-host`, on `farhelm agent create`, `clone`
and `farhelm spawn` are refused with a message saying why (SPEC.md, Agent-spawned sessions, has the agent YOLO rule that
replaced them). `farhelm spawn --inherit-agent` no longer works without the helm, because every spawn now waits for the
user. And a keyed `--inherit-agent` spawn retried across the update is not replayed: its key used to be reserved by the
session's own supervisor and is now reserved through the helm like every other agent create, so the retry is a new
request, shown on a card, which creates a second child if allowed.

Entry-bound session identity is one further explicit exception. Spawn, create and clone now print full session IDs;
other bare targets learned before the update are refused with `farhelm agent sessions` as the remedy. The exact
authenticated own-bare ID remains accepted on its requesting entry, with equality checked before full parsing.
Self-Delete and self-Prompt compare resolved pairs. Discovery host IDs become entry UUIDs, the JSON envelope becomes
schema version 7 with a full caller ID, and own-session and own-host markers derive from entry-bound identity rather
than global bare ownership. This exception covers those taught fields, values and markers together. Supervisors,
credentials and their typed relay remain unchanged; sessions keep running across the helm update. Approval targets,
asking keys and deletion counters carry the internal pair through waits, so removed entries cannot redirect consent to a
re-added entry. A card waiting during the update may get no answer.

### Restart only resumes

SPEC.md makes plain Restart mean resuming the session's own conversation. Named Restart with is the explicit exception
described below; the agent relay still has no selector for a different conversation. `RestartOffer` is `resume` when the
supervisor can fill the session's resume command with a captured conversation it accepts, and otherwise names why it
cannot: `not_captured` for an agent type that reports conversations but has no usable identity (nothing reported yet, an
identity from before ownership proofs, a Pi or OMP file that no longer matches), `no_conversation_reporting` for a
session with no integration at all, and `no_resume_command` (protocol 39) for a command launch that declared an agent
type without opting into Resume. `resume` kept its spelling because agents in running sessions read it; the reasons
replaced `fresh_only` and `fallback_template`. Protocol 37 removed the `mode` field from `RestartSession` and from the
agent relay's `Restart` verb, with `RestartMode` itself: a restart whose current offer is not `resume` is refused with a
`Conflict` naming the reason, before anything is stopped, and the fresh relaunch of the stored invocation and the
verbatim run of a placeholder-free resume command are gone. Since launch kinds (protocol 39) a resume command must
contain `{conversation}`, so no new session can carry one; a legacy session that stored one keeps it, but it is never
run.

Ordinary restarts keep the captured identity, its source, and its ownership version; only the OMP launch-provenance
columns clear, as they describe the launch rather than the conversation. The helm caches each host's last listing, so
helm.db schema 35 rewrites cached `fresh_only` to `not_captured` and `fallback_template` to `no_conversation_reporting`;
without it the cache reader would skip those rows and a down host's sessions would drop out of the list. Tests of
restart mechanics that once restarted a plain command fresh now bind a conversation through the supervisor's
`record_conversation_for_test` seam, which exists only under `cfg(test)` and the `test-seams` feature.

### Launch-kinds reservations

Removing profiles (protocol 38, supervisor schema 25, helm schema 36) changes what a create request can be, and the
create-idempotency reservations already on disk were written for the old shapes. They are kept as they are, with no
compatibility added for them. A key is matched by plain string equality between the stored fingerprint and the
fingerprint of the retry, so a retry that crosses the upgrade with an old key whose shape changed is refused as key
reuse (`Conflict`) and never runs a second launch, while one whose shape did not (a raw create into an existing
directory, whose fingerprint is a frozen tuple) still replayed until launch kinds. These rows follow the same retention
rules as other reservations; removing a live claim merely because its encoding is old would let one intended create
start two sessions, which SPEC.md forbids. The frozen fingerprint encoder for a helm-resolved profile bundle
(`"resolved_profile"`, added in protocol 15) was deleted with profiles: nothing produces that shape any more, and its
stored rows need no encoder to stay unmatchable.

Launch kinds (protocol 39, supervisor schema 26) change every create's shape: each now fingerprints as
`("session_launch_v1", parent, cwd, title, launch)` with the resolved launch whole, and the raw, parented and structured
encoders were deleted. So every key stored before launch kinds, raw ones included, is refused as key reuse on a retry
that crosses the upgrade, never replayed and never run twice; a pending claim from before the upgrade stays pending, its
row not launched, until the session is deleted. The fresh-checkout snapshot moved to `github_checkout_v4`, carrying the
resolved launch; a `github_checkout_v3` row no longer decodes and is refused with the explicit message below. The helm's
own fresh-create request identity moved to `github_create_request_v2` for the same reason, so a lost-reply
fresh-checkout retry across the upgrade is refused as a different request.

A fresh-checkout reservation stores a serialized create mode so recovery can relaunch the accepted request after a lost
reply. Under profile removal alone (protocol 38) a row stored before it still decoded, because the decoder ignored the
removed `source_profile` member, and the helm's fresh-create retry reused the stored string, so such a lost reply was
still recovered. Launch kinds end that: every row from before them is `github_checkout_v3`, and a stored mode that fails
to decode under the current types is refused explicitly ("no compatible fresh-checkout recovery snapshot") rather than
treated as an unknown key, which would otherwise allocate a second checkout. Legacy tombstone identity digests remain
decodable for both encodings; this compatibility does not extend retention. Writable open removes legacy `Created`
records whose sessions are gone, and Delete removes the session's record regardless of its fingerprint encoding, so a
retry after Delete is a fresh request.

The 64 KiB create bound counts the launch as the JSON it is stored and fingerprinted as, so quotes and backslashes in a
command count twice and the field names count too; this is slightly stricter than counting the text the user typed.
Restart with applies the same bound to its replacement launch.

### Restart-with backend wire and persistence

`RestartSession` optionally carries `with`, the replacement launch (protocol 39; protocol 30 carried a compiled
structured bundle instead), and the exact-version handshake refuses an older peer, because one that ignored it would
relaunch with the old settings and still report success. The helm resolves what the caller sent before routing: an agent
type and its choices composed by the catalog (a refused choice is a 400 that never reaches the supervisor), or a command
launch checked by the command-launch rules; the REST body names one of `with` (choices) or `with_command`.
Attached-session relay calls omit it. The supervisor validates the launch exactly as create does before it looks the
session up, then, before destructive work, checks it against the stored launch (same launch kind and agent type, a
resume command, no legacy session) and the current `Resume` offer, and fills the new resume command with the captured
conversation. After the new process spawns, one generation-fenced store write replaces the stored launch while the
working directory stays fixed. A spawn failure leaves the prior launch untouched. If the post-spawn write fails after an
otherwise successful relaunch, the restart still reports success because the new process is already running; the failure
is logged, and the reply and live session retain the old stored launch. A relaunch that published its new process but
hit an independent cleanup or reply error still attempts the write and retains that error reply. A later restart uses
the saved launch, or the old one if the write failed, as it would after a crash between spawn and the write. This is the
only exception to the ordinary create-time immutability of a session's launch.

Protocol 50 adds a transient `NamedConversationStart { conversation, argv }` beside `with` and ordinary create inputs.
The browser's raw `conversation` stays outside `LaunchSelection`, history and templates. The helm validates it with the
supervisor's existing id/token checks and fills the six existing ordinary resume templates through `fill_slots`, without
parsing captured locator provenance. Muse now uses its ordinary `resume <uuid>` template too. Cursor and OpenCode get
one-time suffixes in the helm only (`--resume` and `--session` respectively); their ordinary `resume` stays absent. A
Muse fallback suffix remains for older stored launches that have no resume command. Exhaustive harness facts state
whether a named start is offered, whether it takes a session file, and whether it can report afterwards; labels stay in
the UI composer.

The supervisor validates agent-only eligibility, raw selector and bounded executable argv before admission. Its shared
start-shape check requires one whole Farhelm argument slot, no unfilled conversation slot and no placeholder program.
Named Restart with requires a stored resume command, but bypasses only the captured-target offer, legacy Codex refusal,
locator filling and report-only target verification. Launch kind/harness, directory identity, stop consent and lifecycle
claims still apply. A named create uses the existing Resume hook phase and runs the transient argv while saving the
ordinary launch. The `named_session_launch_v1` fingerprint binds both raw selector and argv; absent selectors retain the
frozen `session_launch_v1` bytes. Same-fingerprint pending retries restore the transient argv beside the recorded
ordinary launch. New managed checkouts refuse it before their separate recovery encodings become involved. Both launch
and transient serialized inputs count toward the existing request bounds.

Reporting harnesses retain old capture until an accepted report replaces it. Goose's named restart clears capture,
source and ownership version in the generation-opening transaction because its resume has no reporter. The same clear
applies to any kind `AgentHooks::silences` reports as having no reporter under the supervisor's opt-out, decided from
the seam value the spawn uses, or when the named start's reporter asset cannot be prepared. Asset-backed reporters are
materialized or verified before deciding whether to retain the old capture; spawn still verifies the artifact for
injection through its ordinary path. `PriorRun` retains those columns only for the existing definitive-abort
restoration, fenced by the new generation. Success and ambiguous failures publish a cleared mirror; an abort restores
that mirror only if its durable restoration succeeded. No capture is seeded from request data and no new report
admission rule or database schema is needed.

### Launch kinds: one resolved launch

SPEC.md's launch kinds are one type end to end, `farhelm_proto::SessionLaunch`: an agent launch (the agent type's
selection with the start and resume argv the helm composed from it), a command launch (`CommandLaunch`: the command,
YOLO assertion, optional declared agent type and resume command, as written), or a legacy launch (a pre-existing
session's stored invocation, agent kind and resume template, untouched). The helm resolves every create to one before
contacting a supervisor; the supervisor validates it (`SessionLaunch::validate_new`, which refuses a legacy launch),
stores it whole in the `sessions.session_launch` JSON column, and every later operation reads that value: restart and
Restart with, clone and replace, inheritance, the YOLO guard and mark, and the integration snapshot capture uses
(`IntegrationSnapshot::of`, the kind and resume argv, derived and never re-guessed). An agent's create carries a
`LaunchRequest` instead (see the agent relay above), which never holds composed argv.

The helm composes an agent launch's start command with `{farhelm_args}` last, and for an agent type with conversation
reporting a resume command that adds the type's resume selector after the start command's choices and then
`{farhelm_args}`; Cursor and OpenCode get none. Muse adds `resume {conversation}`, retaining the already-composed
permission flags and wrapper prefix. Older stored Muse agent launches derive the Muse kind from their saved harness and
gain hooks through their existing `{farhelm_args}` at their next start, but retain their missing stored resume command
and offer `NoResumeCommand` until recreated; there is no resume backfill or migration. `{farhelm_args}` sits where the
previous release appended Farhelm's arguments, so an upgraded session and a new one spawn alike.
`LaunchSelection.wrapper_prefix` stores optional raw shell words, quotes and escapes included, before both composed
commands, including Goose's leading `env` words. The field is omitted when absent; the protocol bump refuses older peers
rather than translating the new launch shape. OMP attribution skips exactly that many words before classifying the agent
program, without relaxing its process proof. At spawn the supervisor fills `{cwd}` (and the compiler's Codex trust
markers `{codex:trusted-cwd}` and `{codex:untrusted-cwd}`, which predate launch kinds and are filled in any launch that
contains one, a command launch included, so a command copied from a composed Codex launch keeps working), then replaces
`{farhelm_args}` with the declared kind's own arguments (`AgentIntegration::farhelm_args`, asked for a start or a resume
and never shown the command) and puts that kind's reporter settings in `LaunchSpec.env`, which the shim sets on the
agent process alone, after scrubbing inherited reporter variables. Goose is the one kind whose integration the
environment still switches: its reporter is persisted in the resumed session, so a resume carries no argument and only
`FARHELM_GOOSE_REPORTER_ENABLED` turns it on or off. A legacy session goes through the previous release's injection
(`AgentIntegration::inject_hooks`, with its shape checks and `env` prefix). Its only other reader of a command line is
OMP's process attribution (`classify_omp_launch`), which reads the program word of every OMP launch, new ones included,
to choose how the running process is matched to the session; it decides nothing about YOLO, agent type or resume. The
`env`-prefix helpers in `farhelm_proto::yolo` stay for these two. The command-line YOLO classifier, resume derivation
(`default_resume_template`, the ambiguous-selector refusals, the selector strippers) and kind derivation from the
program's basename are gone.

Supervisor schema 26 converts each row in one transaction by `SessionLaunch::from_pre_launch_kinds`: a row with a stored
structured selection becomes an agent launch whose stored start and resume commands gain `{farhelm_args}` at the end,
and every other row, or a structured one that would not make a valid agent launch, becomes legacy. A row whose old
columns no longer decode fails the upgrade rather than being guessed at, since the load path already refused it. The
`invocation`, `agent_kind`, `resume_template` and `launch` columns are dropped; `SessionInfo` keeps `invocation` (the
launch's display command) and `agent_kind` (its integration kind) as derived conveniences for its readers. Helm schema
37 rewrites each cached session row by the same rule, adding `launch` and dropping `resume_template`, so a host that is
down at the upgrade keeps its sessions listed until it returns; a cached row with no string `invocation` is left for the
reader's skip-and-log policy.

### Launch templates

Direct quick-launch shortcuts resolve their digit/bolt references from fresh preferences. A completed sequence first
captures the names in its indicator's preference snapshot at Enter; an unbound reference stays unbound even if a later
read finds a binding. Typed references retain the entered word, or Tab's full name, separately from exact-name
references. The dispatcher resolves every typed reference against its fresh template read before resolving captured
bindings, so an unknown or ambiguous word wins over an unbound key. Matching prefers case-sensitive exact, then unique
Unicode-lowercase exact, then unique lowercase prefix; ambiguity reports every competitor in the winning tier. Existing
exact-name references keep their missing-template refusal: a renamed or deleted captured binding cannot select a
different template. Typed references, including Tab's completed spelling, still use the matching rules at Enter. Both
paths resolve current template definitions as one all-or-none request. Dispatch takes a shared keyboard-context and
modal-opening-epoch snapshot before and after that read check, so a dialog that appears during the read cannot receive
its result after closing. It hands the captured definitions to ordinary New, replaying one through the existing
template/search acceptance path per effect pass. A refusal stops the remaining replay. Automatic submission waits for a
render after the final replay, so its captured readiness and refusal describe the resulting draft. A coarse fold over
launch kind, agent and command decides whether to attempt direct launch; the mounted form retains full validation,
catalog, host and admission authority. That fold lives in the protocol crate and also judges a single template for ⚡
marking, its hints, and agent writes. Only the active kind counts: Agent needs an agent, Command a nonblank command, and
omitted kind starts on Agent.

The same dispatch serializes fresh reads when shortcuts arrive in the visible session launcher, including restart mode.
Each press or completed sequence becomes its own captured batch in a form-owned FIFO, fenced by the opening generation
and the shared modal context and epoch. The form validates the whole batch against one draft snapshot, then replays its
templates synchronously through the draft owner's search acceptance operation and private template replay. A refusal
drops only that batch before it changes the draft, leaving later presses queued. Shortcut batches do not enter the
opening replay's loading or auto-submit predicates, and their acceptance keeps the query and focus in place. If a
template unmounts the focused control, a deferred check restores the launcher's normal search target only while that
same launcher is still the sole visible modal. Busy launch and template-save states consume and drop a press.

The shared template-shortcut asset owns the synchronous armed flag, a pure key classifier and the context rule. Rust
holds the ordered references, unfinished word and optional advisory catalog in app-owned transient context. One hidden
input carries tagged action JSON from JS through synchronous `oninput`, preserving keystroke order without a second word
buffer in JS. Printable text, bare digit plus its printed character, number/bolt chords, Space, Tab and Backspace all
mutate that Rust state. A bare digit extends a nonempty word; otherwise it queues its binding. Chords commit the word
first, Space commits it alone, and Backspace removes one Unicode scalar only from the unfinished word. Numeric keypad
digits retain cancellation and pass-through. Tab uses the same match precedence against advisory names and queues a
unique full name as one typed reference, including a historical spaced name. No catalog or no unique match is a no-op.
The sidebar's hidden relays mutate that signal synchronously, and the main area renders its non-modal status pill, using
"apply" while the launcher is open. No storage or general keymap is involved. CSS centers the amber pill in the main
area, using a fixed 9px downward correction for an open Mac session's taller top bar. Its launcher context keeps the old
bottom placement. Existing warning tokens provide text, border, fill and a soft halo; a single 200ms fade/scale entrance
is disabled for reduced motion. Prefix and number modifiers follow the build's Mac-desktop choice; the bolt keeps its
existing operating-system chord, including Cmd+P in a Mac browser. The shared listener installs after the composition
guard and before other command listeners, consuming with stopImmediatePropagation because same-window capture listeners
otherwise still run. Pointer/click, blur, visibility, modal focus and the existing modal-opening hook cancel both state
owners. A launcher-owned text field admits the prefix only without selected text; other editors keep their usual key
action. Hidden quick-launch preparation consumes recognized keys without enqueuing a relay. The relays stay mounted and
enabled across render-delayed armed state. Enter always pairs one press snapshot with one captured batch, including an
empty batch that drains without opening anything. Starting the prefix independently reads template names alongside
bindings, with the same sequence generation fence; failed or late advisory reads cannot gate Enter or provide its launch
definitions. The indicator renders queued chips, the unfinished word and at most five matching candidates below the
pill. The display cap never limits ambiguity detection, and all visible peer names and refusal text are escaped without
changing lookup spellings. The indicator publishes its raw binding projection beside the visible names in the same
render, retaining a hidden projection while idle for rapid input before the first pill render. Physical Enter copies
that DOM projection into the existing press snapshot; the worker pairs it with the raw reference queue before starting
the existing generation-fenced quick-launch read. Starting the prefix refreshes shared bindings once; a cancelled or
superseded sequence cannot publish its late preference read. Enter never refreshes the captured bindings. Missing
captured names reveal New with its existing named-template refusal, including within multi-template sequences.

Number bindings accept partial templates; the bolt keeps its runnable-template affordance. A single partial-template key
passes a number-key banner to New for a prominent alert explaining the prefix. Dispatch carries an explicit origin:
number key, bolt chord, template list, quick switcher, or sequence. Incomplete bolt/list/switcher choices instead say
the template needs more choices; any incomplete sequence says its combined setup is incomplete. The form starts one
component-owned six-second expiry only after initial application finishes, so a slow catalog read cannot spend the
viewing interval. Form input, changes and deliberate search choices dismiss the hint; Clone/Replace reseeding withdraws
it. Nothing stores the hint or changes launch admission.

A template is stored by the helm in `helm.db` (schema 38, table `launch_templates`): its unique name, and its fields as
the JSON of `farhelm_proto::launcher::TemplateFields`. The fields stay one JSON value rather than a column each because
a template is applied whole and never queried by field, and because the set of launcher fields is the wire crate's to
grow. A field a template leaves out is absent from the JSON; for the agent-launch choices that have a default (model,
effort, permissions) and for the resume command, an explicit `null` means "reset to the default", which is how a
template can clear a choice rather than only set one. The wrapper prefix follows the same absent/set/null shape, but its
`null` means no prefix, overriding a remembered one. The agent type, host, destination and session name have no reset: a
template sets them or leaves them alone. A session name's optional `name_kind` is `base` or `fixed`; absent means base.
Writers validate that a kind has a name, then omit base from both storage and the save reply, preserving existing JSON
without a database migration. Only fixed adds a stored field. The shared application function carries name and kind
together, and a name-only agent edit keeps the previous kind. Protocol 61 adds this strict field: an older same-protocol
supervisor would refuse a fixed-template reply during control-frame decoding and drop its helm connection, so the bump
refuses it at hello instead. The normal update path remains available. A host is named by its recorded install identity;
a sandbox recipe instead uses the separate optional recipe field described under Tensorlake sandbox sessions. Unknown
fields are refused on decode, so a misspelled field is an error rather than a silently ignored edit. The retired
`workspace_trust` field remains readable in `TemplateFields`, but application ignores it and every save drops it. The
editor clears it in its draft and on delete undo; Save as template never offers it. This keeps old records usable
without letting their trust choices bypass the agent trust section in Settings. The GUI's writes go through
`PUT /api/templates/{name}` (create or replace, last write wins, no version check, as SPEC.md wants) and
`DELETE /api/templates/{name}`; `GET /api/templates` lists them by name. The helm checks only the shape of a template
the GUI writes (a non-empty name of at most 128 bytes with no surrounding spaces or control characters, and fields
within the 64 KiB a create is held to); whether its fields apply is decided only when it is applied. An agent's template
writes share the same store path but carry a precondition and a few refusals of their own (see "Permission prompts for
agent actions"). New-name whitespace validation is separate from stored shape: the Templates editor checks only when the
name changes, Save as template always checks, and the agent template-create branch checks before approval. The shared
store and collision checks remain permissive so historical spaced names still save unchanged and delete undo can
recreate them. Duplicate normalizes whitespace to hyphens in its suggested name and uses `-copy` / `-copy-N` suffixes,
retaining the byte cap and collision search.

An agent's quick-launch and number-key edits travel separately from template fields in protocol 60: those bindings
belong to fleet preferences, not to a template's composable choices. The helm and Templates editor share the ⚡
eligibility check. An agent write that changes a binding uses `TemplateWriteWithShortcuts`, a distinct approval kind
whose card shows each change and the previous template for a move. An older UI cannot decode that kind and therefore
cannot approve a move it cannot show. As with `LaunchWithPrompt` below, its approval-list decoder fails the whole list
while such a card waits and keeps showing its last successfully decoded list; this is the accepted older-UI behavior,
consistent with SPEC.md's "Upgrade compatibility and client scale" update rule rather than a supported downgrade path.
At approval, the store compares the named template and the entire ⚡/number-key snapshot from the card under one
immediate transaction, then writes the template and both binding columns together. The GUI's template and preference
writes keep their existing last-write-wins behavior, which SPEC.md's "One GUI at a time" accepts for templates and
number-key bindings.

Application is one pure function, `farhelm_proto::launcher::apply_template`, compiled into both the UI and the helm: a
launcher state plus a template gives a new launcher state or a refusal naming the field, in SPEC.md's order (launch
kind, then agent type, then the rest). Choosing an agent type on the agent launch kind runs
`reconcile_harness_selection`, which moved from the UI into the same module together with the model-catalog row type, so
a click, a search result and a template reconcile alike: setting a model runs the same reconciliation choosing it by
hand does (an effort the new model does not offer goes), the custom model's owning agent type is tracked so a later
change of agent type clears it, and with no agent type chosen a model only one agent type offers picks that type. Models
are compared in the agent type's own catalog spelling, as the launcher and the helm's compiler compare them. The
function refuses a field that does not apply to the launch kind then active, an agent-launch choice with no agent type
(other than that single-owner model), a model another agent type owns, an effort or permission the agent type does not
offer, a host the caller holds fixed, and a host no known install has (a host row in the identity-mismatch phase does
not count, exactly as for the create default); any refusal leaves the launcher unchanged. Before that fold, the UI
removes template assignments equal to the opening's held values: host in Replace with, and kind, active-kind agent,
host, folder and raw title in Restart with. Host equality needs a usable recorded identity; folder equality ignores
trailing slashes without expanding or resolving paths. Equal name and name kind are removed together. Remaining held
assignments refuse in template order. An equal agent must never reach reconciliation, which can change historical unset
permissions or clear model and effort. Prompt and command-resume removal remain refused; search results and recents
retain their presence-based checks. The helm's `farhelm spawn` keeps the strict host flag.

The launcher's "Save as template" panel shares the snapshot reader used for application, including raw relayed text,
destination and name. A pure mapping offers only the active tab's values and marks explicit choices separately from
remembered defaults and Clone's inherited placement. A folder-explicit flag is separate from its raw-text ownership:
picking a directory retains a raw seed while still pre-checking it. The inline panel removes unchecked fields through
the editor's field identities, validates through its compatibility rules and `check_template_shape`, and reads the name
list before the existing last-write-wins PUT. This refuses ordinary collisions, not a concurrent write after the read;
there is no new endpoint or version check. The parent guards cancellation and session submission while saving, and a
successful save hands the actual template to the Templates dialog's initial editor state without waiting for its list.

The wrapper input is a raw shell-style draft beside the agent choices, while `LauncherState` and template snapshots hold
parsed words. Invalid text remains in the input and prevents a launch or save instead of silently becoming no prefix.
Switching agent types loads that type's remembered words; Clone, Replace with and recents use the prefix stored in their
selection. A template with no prefix edit leaves the current draft alone, while explicit null empties it. Save as
template offers the current prefix but checks it only when the user explicitly changed or cleared it, so a passive
remembered value does not become a template edit. Quick launch uses the same snapshot and submit path as New. Restart
with keeps its own raw input and updates its selection only after parsing succeeds; an invalid edit cannot be sent.

The launcher's initial prompt is a separate optional field in its draft. The agent tab's template snapshot retains it
when the agent is unset or supported; a request requires a selected supported agent. Recent setups leave the draft alone
and Clone/Replace prefills clear it. Templates replay the shared application result, preserving an empty prompt edit as
distinct from absence. Permission-switch Undo captures it with the other draft values. The full raw prompt is bound to
the create key and fresh-checkout retry identity, re-read after asynchronous key minting, and sent by the same API body
builder for ordinary, named, fresh-checkout and Replace-with creates. It never enters `LaunchSelection`, `SessionLaunch`
or recent history. Restart controls omit the prompt callback.

In the launcher, `tl:name` offers templates (the only search scope that does). Accepting one first removes matching held
assignments and runs `apply_template` on a snapshot of the launcher, so a refusal applies nothing; then it replays the
exact cleaned template returned by validation as the launcher's own search actions (agent type, model, effort,
permissions, host, folder, repository, name) and sets directly only what has no such action (the launch kind, the
command fields and the command launch's declared agent type, the approve, smart approve and chat permissions, and a
choice reset to its default). That replay is what makes remembered defaults, recent setups and a pending checkout
preview react exactly as they do to the same edits by hand, with no provenance tracking for templates. The Templates
dialog beside New has a bounded list column and a selected-template editor; at phone width only the list or editor is
shown, with a back link. A draft holds the wire fields directly, so absence means no visible field, while explicit null
resets and false approval assertions survive editing. Add/remove actions change presence; defaults are values inside the
controls. The editor's agent/command/don't-switch selector writes the kind, with don't-switch offering only placement
and name. A kindless stored template is inferred in the draft only, marked unsaved against its original baseline; saving
explicitly stores that switch. Per-harness choices use the shared capability answers and release catalog, not another
agent table. Model text is unrestricted and suggestions use the existing catalog filter; other incompatible choices stay
visible and refuse save until corrected or removed. Fields need not form a complete launch, because another stacked
template can supply what they omit.

Opening another row, new, duplicate, mobile back, close and Escape share a pending departure and the same inline
save/discard/keep-editing prompt. Save keeps the selected editor and refreshes its baseline; duplicate opens an unsaved
copy with a free name. A rename saves the new name before deleting the old one, so a failure in between leaves both;
saving under a name another template already has is refused in the panel rather than overwriting that template. A
pending or failed name-list read cannot prove a new name is free. Delete checks the stored snapshot, not the unsaved
draft, against the existing template shape rules. If it fails, the control says "Delete permanently" and deletion clears
any Undo; the ordinary save path could not restore that snapshot. Otherwise Delete retains the stored template
client-side for about ten seconds; undo freshly reads the names before restoring and refuses a taken name. The two
delete kinds have separate, always-mounted buttons, switched with `hidden`, whose handlers each carry a fixed kind.
Native listener updates can precede a webview paint, so reusing one button could change the Undo promise before its new
label appears. A Delete with no usable reply reports that deletion may have happened, clears Undo, and refreshes only
the template list; helm refusals retain their existing error. This is a local stopgap, not lost-reply reconciliation.
The notice and its timer belong to this mounted dialog, with a generation guarding against an earlier timer expiring a
later deletion. Writes are serialized and disable navigation until their outcome is known. API last-write-wins semantics
remain unchanged; name checks do not acquire an atomic cross-client reservation. The launcher reads templates when it
opens and again whenever the Templates dialog closes, using the existing revision bump and focus return.

### Agent launches from the CLI

`farhelm agent create` and `farhelm spawn` without `--inherit-agent` send the helm launcher edits, not a launch:
`AgentVerb::Create` (protocol 40) carries the template names in order, the flags as one more
`farhelm_proto::launcher::TemplateFields` (`--cwd` as the destination folder, `--title` as the name), an optional host
NAME, and for a spawn a placement naming the parent. The helm's `agent_launch::resolve` applies the templates and then
the flags to an empty launcher with the same `apply_templates` the GUI uses, so the CLI and `tl:` cannot disagree about
what a template means; a refusal of the flags names the flag (`--model applies to an agent launch ...`) rather than an
unnamed template. The result must have a folder or, for agent create, a managed checkout (`--gh`, aliased as `--github`,
sets `TemplateDestination::Github` and conflicts with `--cwd`). Spawn still refuses a managed destination. Either
requires an agent type or a command and a YOLO answer; the launch is then validated by the same
`sessions::resolve_launch_request` a REST create goes through. Only `--command` sets the launch kind: the agent-launch
flags leave it alone, so on a command template they are refused naming the flag, as the same edit is refused in the
launcher. `--model`, `--effort` and `--permissions` accept `default`, sent as `null`, which resets a choice a template
made, and `--no-resume-command` resets the resume command; an omitted flag sends nothing. The CLI has no way to clear a
command launch's declared agent type that a template set. clap refuses a repeated flag (other than `--template`) and
`--yolo` with `--no-yolo`. The supervisor's relay only bounds the request (host name, at most 64 template names of at
most 128 bytes without control characters, the flags' JSON within the 64 KiB create cap); whether it says enough to
launch is the helm's to decide.

The host is the asking session's own for a spawn (which also makes a template that sets a host a refusal, through the
launcher's host-fixed rule); otherwise an explicit `--host` name, then the install a template named, matched among host
rows not in the identity-mismatch phase. With an explicit `--host` the templates' host fields are dropped before they
are applied, so the flag wins even over a template whose host was since reinstalled or removed, which the launcher's
`tl:` would refuse. A spawn's parent must be the asking session; the relay holds the asking session's delete fence for
the whole request, which keeps the parent from being deleted under the create. An inheriting spawn is placed the same
way, with the launch its supervisor filled in, and a host row that has gone missing during the request refuses it rather
than letting it fall back to another host. The helm still sends the spawn's create with `key_lives_with_session`
(protocol 40). The field remains accepted on the wire, but the supervisor ignores it: every authorized keyed create gets
session-lifetime scope, whether the marker is present or absent.

A keyed agent create or clone is matched on its target by the request as the agent sent it (protocol 43). The helm
digests a dedicated, versioned input (`agent_requests::AgentRequestDigest`): `agent_create_v1` holds the `--host` name,
the template names in order, the flags as `TemplateFields`, and whether it is a spawn with its parent; `agent_clone_v1`
holds the source session, the host name and the `--cwd`/`--title` overrides as given. Neither holds the `confirm_yolo`
field an older CLI may still send. The SHA-256 of its JSON travels beside the asker-scoped key as
`CreateSession::request_fingerprint`, and the supervisor stores `("agent_request_v1", digest)` as the reservation's
fingerprint in place of the resolved fields, so a retry that resolves differently after a template or source edit still
replays while its retry record is retained. Delete prunes that record like any other reservation. The supervisor honors
the field only beside a key and never beside a managed checkout, and only the helm's full-authority connection can send
it, since a session-authenticated create is refused before any field is read (protocol 41). Every attempt resolves
afresh and the helm keeps no record of a key; a create the first attempt left pending is still relaunched from its
stored row (`validate_retry`), never from the retry's resolution, and only when the launches, folders and titles agree:
under a request fingerprint the helm approved and YOLO-checked the retry's launch, folder and title, so a stored one
(the folder compared after `~` expansion, a title the retry leaves out not compared) that differs is refused as a
`Conflict` rather than run unseen. If recovery removes the stranded row, the key retains that refusal for 24 hours for
every create, spawn included; after expiry the next retry creates afresh from the request as it resolves then. Explicit
Delete frees the key immediately. The digest input's JSON is frozen, because it lands in reservations that outlive
builds: a new shape gets a new tag. `TemplateFields` is the one protocol type inside it, which is safe because its JSON
is already the stored template format and omits unset fields, so a launcher field added later leaves existing digests
alone. An inheriting spawn sends no request fingerprint and keeps the resolved-launch one. A keyed
`farhelm agent create` without `--host` is refused at the helm (SPEC.md gives the reason). Reservations written before
protocol 43 hold resolved-launch fingerprints, so a retry spanning the upgrade is refused as key reuse and never
duplicated, the rule "Launch-kinds reservations" already applies. Helm schema 39 kept each keyed create's first
resolution (`agent_create_bindings`, for 30 days) to spare retries from template edits; schema 42 drops that table, and
the 38→39 step no longer creates it. A keyed create, clone or spawn re-run after its created session was deleted creates
a new session from the request as it resolves now, with the ordinary approval rules, because its reservation is pruned
with that session.

Managed-checkout agent creates use that same frozen `agent_create_v1` digest as the checkout's `client_identity`,
without a `request_fingerprint`. The helm resolves the request and host, checks the agent YOLO rule, and performs
lookup-only reconciliation before preview or approval. An unknown key proceeds to the shared claim-bound preview, card
and fresh-create pipeline. A known key exposes its original launch and checkout for the wrapper guard; a pending
agent-supplied prefix without a retained Allow gets a card for those original facts if the host now asks before YOLO.
The exact inspected fingerprint is required by the later launch-capable reconciliation, so another accepted snapshot
cannot replace the one the card showed. An Allow is recorded with that reservation under the same admission lock before
recovery, and its approval bit does not change the identity of the original create. A completed attempt returns its
recorded result without a card. Agent fresh dispatch does not perform another launch-capable lookup after approval;
supervisor create admission compares the complete checkout fingerprint when concurrent requests reach it. It revalidates
configuration and the accepted installation, and uses `CreateOrigin::Agent`, including the dispatch-time YOLO guard and
no remembered GUI selection. A local failure leaves an agent's key for an exact guarded retry instead of using the
browser's launch-capable durable refusal settlement. A template that flips between a folder and checkout encounters an
incompatible reservation and is refused as key reuse. Agent template writes store managed destinations through the same
editor path as folder destinations.

### Permission prompts for agent actions

SPEC.md (Agent-spawned sessions) has the helm ask the user before it carries out any acting `farhelm` command from
inside a session. The decision lives in the helm because the requesting host and its supervisor are untrusted: the
per-host setting, the check that a GUI is there to ask, and the wait for the answer are all keyed by the host connection
the request arrived on (`agent_requests::AgentOrigin`), never by anything the request claims. Each acting verb handler
first resolves exactly what it would do (the launch, the target session, the whole template), then asks
(`approvals::ask`), then rechecks that the request's connection is still the one serving its host before acting, so an
approval never carries over to whatever replaced that connection during the wait.

NOTE: running-agent prompts are turned off (SPEC.md, Agent-spawned sessions). The helm's agent request handler refuses
every `AgentVerb::Prompt` first, ahead of its liveness and validation checks, while
`agent_requests::AGENT_PROMPTS_ENABLED` is false; flipping that constant is the helm side of a re-enable. This and the
next paragraph describe the dormant path that the re-enable restores. It still compiles and ships unchanged, protocol
included; the helm's unit tests reach it through a test-only handler, and its end-to-end delivery tests are ignored
until then.

Running-agent prompts use their own `AgentVerb::Prompt`, `AgentReply::Prompted` and approval kind. The helm resolves the
target before asking, then reads `PromptTargetInfo` from that routed supervisor. Its `PromptTarget` reply carries the
current `SessionInfo` and an attachment snapshot across every terminal belonging to that session, terminal tabs
included. The supervisor's attachment table is the authority on which terminals are held; the helm's channel registry
cannot answer that question. The card renders the full message through the same escaped, scrollable peer block as an
initial prompt and reuses the session list's permission-shield classification. The requesting and target connections are
revalidated after approval; this path never composes a launch or applies the launch-only YOLO guard.

`PromptSession` is an accepted supervisor-owned mutation, so losing the requesting connection does not cancel a paste
between its opening marker and submit. It takes the target lifecycle claim through the final confirmed send, reads its
stored activity synchronously, and requires a live agent terminal reading Running or Idle. No activity leaf lock or
attachment lock survives an await, and it never takes the target's agent-request fence: crossing prompt requests must
not wait for each other's relay fences. Delivery writes to the session's agent terminal through its runtime, without
taking over a viewer: one bracketed paste, then the per-kind submit bytes, as two confirmed sends. Its internal bounded
chunking preserves one opening and one closing paste marker around the whole message. The terminal's emulator must have
caught up with its host and record the program's bracketed paste mode as on; otherwise delivery is refused. Claude uses
CR and Codex Tab, at both accepted statuses. A later native delivery route belongs behind that same per-kind strategy
seam. There is no Farhelm prompt queue, receipt store or idempotency key. A lost or failed reply may follow partial or
complete input, so retries are new sends. Ordinary attached-client typing stays ungated; the status check is specific to
this agent-requested operation.

Pending approvals are an in-memory table in the helm (`approvals::Approvals`). Nothing is persisted: a helm restart
drops the table and the parked requests with it, and the asking CLI gets the relay's ordinary "helm went away" ending.
The GUI reads the table through `GET /api/approvals` and learns of changes from the fleet invalidation feed, which every
insert and removal bumps; it answers with `POST /api/approvals/{id}`, which returns 410 once the request no longer
waits. An entry leaves the table exactly once: the user's answer, the asking session being deleted, the requesting
connection going away (checked on every feed change and at least every 5 seconds), or the wait expiring after 9 minutes
(`farhelm_proto::approvals::APPROVAL_WAIT`). Every ending passes through one drop guard that removes the entry if it is
still there and bumps the feed, so no card outlives the request it describes. Card ids are random, not sequential.

The GUI keeps one expanded request id locally, preserves it across listing refreshes, and falls back to the oldest
remaining request when it leaves. The other requests are header buttons below it, including requests from other hosts.
The listing's id sequence and the expanded id together trigger the 700ms arm delay; a generation check prevents an older
delay from arming buttons after a more recent switch. The common shell separates its pinned header and answer bar from
an aligned fact list and individually scrollable long-text boxes. Requester host, quoted title and shortened ID remain
separate peer isolates with accessible labels. ID shortening uses complete `peer::display_identity` escape spellings
from the bare ID, while hover and accessible text retain the complete reference. Target and copied-source IDs stay in
full. Copy buttons call the existing clipboard writer on the unmodified raw reference. Escaped or shortened display text
is never copied or used as an action target; the card's answer handlers retain its random approval ID. Launch and
restart permission chips reuse `list::session_permission` and its description rather than classifying command strings
again; stored workspace trust and permission flags keep their separate meaning.

The approval region stays beside the shell so modal isolation leaves it live. While mounted, its small geometry observer
follows the actual main-pane rectangle and the terminal area's top, falling back to the top row's bottom when no
terminal surface is shown. Anchoring below the top row and conditional bands keeps approval cards from covering those
in-flow notices. Floating tunnel surfaces independently declare terminal-link coverage. This accounts for sidebar width,
narrow-window shell scrolling, native top chrome, and conditional notices without fixed offsets. Width is capped at
660px with 12px margins; the region's height is capped at 75% of the pane and at the space remaining below its top. CSS
flex sizing keeps the card header and answer bar fixed while shrinking long-text boxes toward their two-line floor
before the intervening body scrolls. Waiting headers have their own bounded scrolling list, and a late-answer notice is
a fixed item. The fallback minimum measures the header, requester and answer bar at the actual card width, including
their spacing, plus the notice and waiting list. Those fixed siblings consume the same cap; comparing only the card's
minimum with the whole region could clip its answers. When that combined minimum cannot fit below the anchor, the
geometry helper uses the whole viewport. Below the minimum even there, CSS lets the whole region scroll instead of
clipping answers. The existing size observer follows only those fixed inputs, alongside the pane, sidebar and chrome;
direct-child observation rebinds them when the request, notice or waiting list changes. Body and text-box heights are
outputs of CSS shrinking, never inputs to this measurement. Terminal output mutations remain unobserved. Listeners and
observers are released when the region leaves the DOM. Sidebar size is an input even at the pane's minimum width, where
a resize moves the pane without changing its size.

"A GUI is connected" means a window holds the helm's UI seat (`gui_connected` in the helm's approvals module reads it;
see "The seat: one UI per helm"): the seat is what a window holds for its whole connected life, and the UI channel that
holds it is what pushes the card to it. A displaced window, or one whose channel dropped, has given the seat up, so a
request then is refused as having no window to ask in rather than waiting on a card nobody can see. The helm's own
internal subscriptions to the fleet feed never count as a window. For now a connected window in the desktop app's helm
gets no card either: its requests are refused at once with an interim message of their own (see the NOTE in "The seat:
one UI per helm").

Waiting needs time budgets that outlast the wait. `AgentVerb::may_wait_for_user` names the verbs that can wait (every
acting verb, no listing); the supervisor's relay gives those an answer budget of its ordinary agent upcall timeout plus
`APPROVAL_WAIT`, and the CLI, which has no deadline of its own, prints a line saying it is waiting for the user once a
request has waited two seconds. The relay's extra 30 seconds is an allowance for the work the helm does before and after
the wait, not a guaranteed ordering: the approval wait starts only once the helm has resolved the request, so a request
whose resolution is slow (a clone reading its source from a busy supervisor, say) can still time out at the relay, and
the CLI then reports the outcome as unknown rather than as unanswered. The supervisor still serializes a session's
acting requests behind its delete fence, so a second change from one session while the first waits is refused once that
fence's ordinary wait runs out. `OpenUrl` is non-mutating but may wait for the user: neither the supervisor's delete
fence nor the helm's per-session change lock is held while its card waits. Protocol version 46 adds the tagged
`AgentVerb::OpenUrl` and `AgentReply::UrlOpened` variants.

The helm admits at most four agent answer tasks per host connection (`client::AGENT_ANSWER_SLOTS`), and a waiting
request holds one. A host therefore has at most four cards up at once, and while it does, every other agent request from
that host, listings included, is refused with the slots' ordinary "too many in flight" message. That is accepted: four
waiting requests from one host is already more than a user answers at once, and a separate cap would only move the
refusal.

The "Run farhelm commands from this host without asking" setting is `hosts.commands_without_asking` in `helm.db` (schema
40), off by default, and reset to off whenever the row adopts a different install, exactly as the YOLO setting is.
"Always allow" on a card stores it only if the card's connection is still live when the answer arrives, and stores it
only while the host row still records the install identity it had when the request arrived
(`allow_commands_for_identity`, under the host write lock), so a card raised by a replaced installation cannot grant its
successor the setting. The setting and the request can race: when the request expires between the setting being stored
and the request being taken, the answer gets 410 and the setting stays on, and the GUI's notice says the answer did not
take effect and, after an "Always allow", that the host's setting may still have been turned on. The liveness check is
not repeated once the host write lock is held, so a connection that ends while the answer waits for that lock is the
same race: the setting the user chose is stored for that install, the request is refused, and the same notice applies.
This holds for every setting a card's "Always allow" stores, including the own-host and link grants below.

The independent **Manage sessions on this host without asking** permission is `hosts.own_sessions_without_asking`
(schema 80), default off for existing and new hosts, Tensorlake hosts included, and reset on identity adoption in the
same write as the other permissions. Each covered verb passes the shared approval decision a fact computed from
helm-resolved host IDs: the destination is the origin, and for clone its source also belongs to the origin. Prompt,
templates, host changes and links pass false. A non-direct-Allow command bypasses its card when either fleet-wide
command authority is on or this fact and the own-host permission are both true. The launch-only YOLO guard stays
separate; plain restart does not acquire a YOLO check.

The pending entry retains that eligibility, masked off for direct-Allow actions, and exposes it through a
serde-defaulted card field. The GUI shows **Always allow on <host> only** beside the existing fleet-wide button only
when eligible, using escaped peer text for the host. Its distinct answer variant cannot be mistaken for the existing
fleet-wide answer by an older helm. The answer handler refuses an ineligible grant without taking the card, then checks
the live origin and writes only for the recorded install identity under the host write lock before approving. The
settings switch and fleet-wide switch write independent stored values through their own per-field routes; the own-host
switch remains editable while the fleet-wide grant is on.

Protocol 64 adds `AgentVerb::HostAdd` and `AgentReply::HostAddAccepted`, plus the distinct `host_add` approval tag,
which requires direct Allow. Its answer carries only the edited destination; that payload is required on HostAdd Allow
and refused on every other action or answer, with bounded nonempty text checked before the entry is taken. Alias stays
in the dialog's ordinary add binding. The existing approval table's take elects the sole answering window. A HostAdd
Allow carries a oneshot receipt with its destination: the answer POST returns 204 only after the waiting host-add
handler selected that answer and passed its final origin check. A failed send, expiry, or dropped receipt returns Gone
and permits no browser probe. Sending the positive receipt is the last fallible approval step, with no intervening await
or later origin veto. Other actions keep their existing answer semantics.

The UI keeps a page-local opening between approval cards and the hosts panel, without another pending registry or
installer. Newly observed requests show their cards in every window without opening a dialog or moving focus. Only the
card's explicit open action queues the ordinary add dialog. `HostModal` admission waits for existing dialogs, and the
hosts panel keeps a manual add draft or pending submission ahead of an agent opening. Explicit opens are presented in
click order after the current form closes; successful reads drop queued ids that no longer wait. Admission also
preserves a setup submission's draft until its POST has resolved and any definite refusal has reopened it. One shared
synchronous answer guard and the existing 700ms arm pause cover the card and dialog. The pause restarts only when the
card list or selection changes, so a dialog admitted after waiting behind another dialog, draft or submission opens
already armed. Successful listing absence can retire a pending dialog, but failed reads prove nothing and an answering
dialog waits for its own reply. Only the successful receipt continues the same form's ordinary probe with captured
edited fields. An immutable agent-origin bit travels through setup submission and definite refusal/reopen; the
provisionable branch skips setup only for a user-originated add whose preference is true. The receipt attests to
approval, not discovery or installation: browser or reply loss can interrupt the continuation before a probe reaches the
helm. There is no correlation, persistence, or automatic replay across that gap; once a probe or setup request itself
accepts work, its ordinary server-owned lifetime applies.

Protocol 63 adds `AgentVerb::HostEdit` and `HostRemove`, answered by `AgentReply::HostEdited` and `HostRemoved`, with
the approval kinds `ApprovalAction::HostEdit` and `HostRemove`. Both kinds return true from
`ApprovalAction::requires_direct_allow`, the mechanism agent-supplied wrappers already used: `approvals::ask` cards them
even when the host's command setting is on, the GUI offers no Always allow on them, and the answer route refuses one.
Their own tags make a UI from before them fail to decode the card rather than offer Allow on a change it cannot show.
The helm resolves the host by display name with the creating verbs' `resolve_host`, refuses this machine with the
store's own `LocalHostImmutable` refusal and the asking session's own host before any card, and canonicalizes a new
alias with the store's `validate_alias`, so the card shows the value that will be stored. An edit whose fields already
have the requested values is refused before any card rather than answered with success, so no edit succeeds without an
explicit Allow. After the Allow, `hosts::edit_identity_checked` and `hosts::remove_host_checked` read the row again
under the locks the GUI's writers of the same fields take, refuse if the destination, or a field the card showed,
changed during the wait, and otherwise write while still holding them, through the cores the settings dialog's and the
hosts panel's routes share (`write_alias_locked`, `write_appearance_locked`, `forget_host_locked`). An edit holds the
host's write lock, which the alias, retarget and appearance routes all write under, plus the provisioning lock first
when it changes the alias, as the alias route does; a remove holds the provisioning lock that every retarget holds,
taken without waiting so a busy host still gets the GUI remove's prompt refusal. A comparison made before taking the
locks would leave a window in which a GUI retarget or appearance change lands after the check and is then overwritten or
acted on. The appearance write takes icon and color together, so an edit of one writes the other back with its value
from the protected read, and an alias and appearance edit is not split by another writer. A test-only gate in the
approvals table (`HostWriteGate`) parks an approved change just before it takes the locks, which is how the helm tests
land a GUI write in that window.

Link requests use the same approval table, presence check, wait and answers, with `ApprovalAction::OpenUrl` carrying the
full URL, its local-network category and whether the card is preapproved. `browser::permission_for_url` applies the
terminal link predicate's raw-string exclusions before WHATWG parsing, with a 4096-byte cap. It keeps the original
string; the parsed host is used only for classification. No DNS lookup is performed. An IPv6 literal in `::/96`,
`64:ff9b::/96` or `2002::/16` is classified by the IPv4 address it embeds, so a public NAT64 or 6to4 address stays
public; `64:ff9b:1::/48` (local-use NAT64, whose embedding position varies) and site-local `fec0::/10` are local
whatever follows the prefix. The two default-off schema-45 columns, `hosts.links_without_asking` and
`hosts.local_links_without_asking`, are independent of command authority and each other. Their settings routes accept an
explicit `allowed` boolean. Always allow selects only the card's category through `allow_browser_for_identity`, using
the live connection and identity preconditions of command grants; adoption clears both columns.

Only `run_embedded` receives `LinkOpener`, registered by the desktop shell through `webbrowser::open`; no HTTP route
exposes that callback. The helm validates and approves before calling it on a blocking worker, then returns the native
opener's result without exposing its error details. A matching grant skips the card only with this native capability. A
standalone helm instead offers a preapproved Open/Dismiss card, because a browser needs a click. The web Rust click
handler checks the live arm state and answer-in-flight guard, then calls the shared `isPlainWebUrl` and
`window.open(url, '_blank', 'noopener')` synchronously through wasm bindings, before spawning the answer request.
`noopener` can return null even on success, so that return value cannot establish whether a pop-up was blocked. A native
UI answers the card without opening in its page. The approval region installs the shared URL predicate when mounted.

Protocol 47 adds `SetBrowserForwarding` and its operation-specific `BrowserForwardingSet` acknowledgement. The
connection manager sends true for SSH rows and false for Local rows only after identity acceptance, including accepted
identity-less peers, and awaits the reply before publishing Connected. Mismatched and duplicate identities never
configure. A refusal or missing acknowledgement follows the existing connection attempt deadline and retry path. The
supervisor applies a process-local boolean before acknowledging; disconnect leaves it in place, while each accepted
reconnect sets it explicitly. Actual link requests still require the owning helm's connection. No schema or per-session
placement field is needed: every next launch and new tab reads the current policy.

`LaunchSpec::browser_command` is optional with a serde default for private specs already written on disk. It carries the
absolute `session_farhelm_exe` plus `agent open-url -- %s`, separate from harness reporter environment. The shim adds it
only in final agent assembly after login-shell startup, preserving any inherited BROWSER, even empty; git and the
post-clone command use the common child assembly and get no fallback. Tabs pass it in their host child's environment
without a PATH change; explicit launch seams and the supervisor's inherited BROWSER win, then shell startup can override
it. Tests pass inherited browser state explicitly rather than editing or depending on their process environment.

Common BROWSER readers split on colon and whitespace; Python also removes shell quotes/backslashes, and xdg-open's word
expansion and printf interpret glob characters and backslashes. The generator therefore rejects relative paths,
whitespace, colon, percent, quotes, backslash and glob metacharacters, logging a generic reason without exposing the
path. Python's command-template controller and xdg-open wait for the child, whereas Rust webbrowser runs non-text
browsers in the background (Codex's login server uses it). Inspection of Claude Code 2.1.296's Linux bundle found its
native opener passes BROWSER as one executable to an argv-based child helper, not as a template; automatic forwarding
does not work there. The explicit Farhelm verb remains usable. Login callbacks to a remote localhost server are not
forwarded automatically by the explicit localhost-tunnel feature.

Deleting a session refuses its waiting requests first and keeps refusing new ones until the delete finishes
(`Approvals::deny_session`, held for the delete and for a replacement's teardown), so a card for a session that is going
away cannot be allowed into acting for it.

Protocol 52 adds `AgentVerb::Delete`, `AgentReply::Deleted` and `DeleteSession::refuse_if_agent_request_in_flight`. An
agent delete sets the flag: the target supervisor tries its existing agent-request fence without waiting, refusing with
Conflict when held. This prevents two sessions' crossing deletes from each waiting on the other's retained relay fence.
UI deletion and Replace keep their ordinary waiting behavior. Self-delete is refused at both relay and helm before
destructive dispatch, because the relay holds that same fence for its own request and deletion would revoke its
credential before returning.

The same protocol bump adds the optional one-time create prompt. Prompt-free retry encodings stay byte-for-byte
unchanged. Prompt-bearing ordinary and named creates bind their text separately from launch; fresh-checkout recovery
retains its accepted prompt in its versioned reservation snapshot. An agent's frozen request digest includes explicit
prompt edits through template-field serialization. A new `agent_prompt_request_v1` reservation holds that digest plus
the resolved prompt's hash: completed replay compares request digests, pending recovery also compares prompt hashes.
Delete removes these reservations too; it writes no prompt tombstone. Legacy tombstone normalization remains readable. A
prompt-bearing approval uses `LaunchWithPrompt`, so an older UI refuses its unknown kind rather than approving an unseen
first message. Template fields already refuse unknown fields. Both cards render the full message through the escaped
multiline peer block.

What a card shows is what an approval does. Launches are resolved fully before the card, and the dispatched launch is
the one shown. The agent YOLO rule (`yolo_guard::check_agent`) runs before the card, after it, and again at dispatch: on
a host that asks before YOLO launches it refuses every agent launch except one it can vouch for, a structured agent
launch whose effective permission is not YOLO and whose start and resume argv equal what `launches::compile` makes of
its selection. That last condition is what keeps a clone honest: a clone copies the source session's launch from the
source supervisor, which is untrusted, and without it a source could pair non-YOLO choices with a YOLO command line. A
command launch can never be vouched for, since Farhelm does not read commands, and `confirm_yolo` in a request is
ignored. A launch is also refused if the target host's connection changed while the card waited, since the approval was
for the machine the card named. A lifecycle verb routes its target before asking, so a session no host knows is refused
without a card. A plain restart re-runs the session's stored launch; the card shows the helm's cached copy of it (a
restart whose copy the helm cannot read is refused before any card, since there would be nothing to show or to hold it
to), and the restart carries that copy to the supervisor as `RestartSession::expected_launch` (protocol 41). The
supervisor refuses, before stopping anything, unless the stored launch is still exactly that, comparing under the
session's lifecycle claim, which every Restart with also takes, so a Restart with from the GUI while the card waited
cannot make the approval resume a launch the card did not show. The helm also refuses up front when its cache already
shows the change, which only gives a clearer message.

Template writes (`TemplateCreate`, `TemplateEdit`, `TemplateDelete`, protocol 42) go through the same card. An edit is
merged with the stored template before the card, so the card shows the whole result, command text the listing withholds
included. The write then lands only on the template the card was built from: the conditional store operations
(`put_launch_template_if`, `delete_launch_template_if`) compare and write in one immediate transaction, and a mismatch
is refused as "changed while this request waited". This is the agent path's exception to SPEC.md's last-write-wins; the
GUI's own writes still carry no precondition. The helm also refuses, before any card, an edit with nothing to change, a
`--command` without a YOLO assertion, and a write whose result would hold both agent-launch and command-launch choices,
which `launcher::apply_template` could never apply. A create stores the launch kind its choices imply: agent for
agent-launch choices or an agent type alone, command for any command field. The template therefore means what the flags
meant.

An agent-supplied wrapper is marked when its own flags set the prefix, or when clone or inheritance takes a prefix from
a source session. The resolved launch still undergoes exact recomposition, but that comparison cannot establish who
chose the prefix. On a host that asks before YOLO, the guard therefore returns an approval-dependent verdict; the direct
Allow card satisfies it at the later checks. A template prefix counts as user-approved only after its write card was
shown, including on a host that normally skips command cards. The ordinary host command shortcut cannot waive either
card.

The prompts are a check on agents that act through the `farhelm` CLI, not a sandbox. Any process running as the
session's user, an agent included, can open the supervisor's socket with full authority and act on that host without
asking, and a compromised supervisor can act on its own host; SPEC.md (Local authority and trust between hosts) leaves
same-account isolation out of scope. Reaching another host, or the helm's own state, still goes through the helm.

## Host identity appearance

The helm stores each host's decorative identity beside its alias: nullable `hosts.icon` and `hosts.color` text columns
added in schema 44. Schema 58 removes their SSH-only checks. Null means `laptop` and `red` for the local host, `cloud`
and `default` for an SSH host; resetting to that kind's defaults restores null. Stable lowercase words are shared by the
helm and UI through `farhelm-proto::host_appearance`, so picker reordering never reinterprets saved values, and unknown
words are refused at the HTTP boundary. An untouched local row keeps the red laptop without a backfill.

`POST /api/hosts/{id}/appearance` requires both words and refuses extra fields and an unknown host. A helm-owned task
serializes the atomic write on the ordinary host write lock and bumps the existing feed only when the stored choice
changes. No actor reconcile is needed: connection actors do not consume decorative identity. The hosts listing carries
both choices; the existing UI hosts read supplies session-row state as well as the hosts panel, so a feed refresh
redraws memoized rows without adding another fetch.

`HostMark` draws the sixteen Farhelm-owned paths in the existing 16-unit box at stroke weight 1.3; `data-glyph` carries
the icon word (`laptop` for an untouched local host, `cloud` for an untouched SSH host). The seven `--host-icon-*` CSS
tokens are identity colors with no state meaning, and the contrast harness binds them to the chrome, selected, dialog
and picker surfaces. Red has its own identity token with the same value as `--danger`. Only the SVG receives the tint,
apart from the optional terminal frame; its hover text still describes the host's locality even when a remote host
chooses a red laptop. The unknown-locality slot stays blank.

Appearance controls live for both known host kinds in the host settings dialog's own refresh classes, between Connection
and Permissions. Each choice is a named button with `aria-pressed`, in a labelled icon or color group, and saves through
the panel's existing operation token and per-section outcome. They are disabled while a field editor or any write holds
that token. Buttons keep arrow navigation from triggering intermediate saves, as a radio group would; the existing focus
recovery returns to the chosen control after its save. The panel submits only the explicitly changed word from an event
and reads the other word from the live hosts snapshot at handler entry. A successful appearance write installs its known
pair behind the existing read-generation barrier before releasing the write token; an older in-flight GET cannot restore
the preceding pair, and the next explicit choice cannot undo it while the refresh is pending. The ordinary refresh still
supplies connection facts and subsequent changes from other clients. A sidebar-surface preview shows the icon without
changing a session.

The quick switcher reuses ListView's existing hosts signal for marks, while retaining its independent fleet session
listing. The launcher reduces the same host snapshot to `HostOption` display facts, including identity words. Its host
field is a launcher-only button combobox with one listbox, active-descendant navigation and non-tabbable options,
following the model picker's focus ownership. Arrow keys, Home and End browse without committing; Enter or Space commits
an open option, Escape or blur discards browsing, and closed Enter retains `enter_choice` launch behavior. Typeahead
uses a case-insensitive prefix, restarting after a one-second pause and cycling repeated letters. The menu closes during
busy work and unmounts with its launcher; an outside pointer press blurs the button so even a click on unfocusable
padding closes the menu without changing the committed host. The committed id stays empty until hosts arrive. The parent
retains the old native select's synchronous destination/history fence, browse invalidation, clone takeover and intent
reset. Picking the already committed host does not reset the draft. Template editing retains its native host select.

`SessionView` reuses `AppBody`'s `Signal<HostsRead>` context that the sidebar refreshes from the hosts feed; its
separate stale-only host read remains limited to outage details. The shared preferences signal controls whether a
colored frame is drawn. The host color word becomes one CSS custom property on the terminal panes container, inherited
by each terminal, using `--fg-0` for `default` and the matching `--host-icon-*` token otherwise. The current web and
WebKit render has no distinct dark margin: pane, terminal and the terminal's own surface (the grid) all paint the
terminal theme's background (`--terminal-bg`) by default. With coloring enabled, a 4px inset shadow fills the terminal
element's existing padding. The shadow leaves the terminal background and the terminal theme unchanged and adds no
layout space; the setting-off render has no shadow. A host appearance refresh recolors an open session without fetching
hosts from the session view, and a Settings write updates this client's preference signal immediately.
