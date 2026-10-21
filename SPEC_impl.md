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

## Who owns an accepted action

Once the helm or a supervisor accepts a request that changes state, the server owns carrying it out. The work runs on a
task the server owns, not on the task serving the client's connection, and that connection's handler only waits for the
result. A client that disconnects, reloads, has its credential rotated, or is cancelled loses the reply, never part of
the work. This applies wherever a connection's task can be cancelled: helm HTTP and WebSocket handlers, which the web
framework drops when the client goes away, supervisor request handlers, which are aborted after a grace period when the
helm's connection closes, and the helm's answers to agent requests relayed by a supervisor, whose read-only answers are
aborted when that supervisor's connection ends.

For authenticated helm HTTP routes, ownership is the router's default for methods other than GET and HEAD. It begins
after authentication and before typed request parsing, so a valid request can finish parsing and take effect after its
caller disconnects. Invalid requests still receive their existing rejection responses. Routes whose cancellation is
intentional live outside that layer: read-only POSTs, Stop and tab operations, transfers, and handlers with narrower
admission or owned-work boundaries documented at their registrations. GET, its implicit HEAD companion and WebSocket
upgrades keep their existing lifetimes. Desktop preflight and the standalone token exchange remain outside ownership.

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

The helm owns its runtime and retains its state-directory claim until that runtime has dropped, including on unwind, so
a successor cannot share its state with blocking work still finishing during teardown.

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
- `crates/farhelm-supervisor` — session management, tmux driver, agent-kind integrations, SQLite state.
- `crates/farhelm-helm` — the host registry (SPEC.md's term: the helm's record of registered hosts and their SSH
  destinations), SSH transport, aggregation, axum API, static UI serving.
- `crates/farhelm-proto` — wire types and protocol version, shared by both ends and by tests; also the few HTTP tokens
  the UI branches on in the helm's replies.
- `crates/farhelm-ui` — the Dioxus application, built for web (wasm32) and desktop from the same crate.
- `crates/farhelm-desktop` — the macOS webview shell (D6): a `main` that calls farhelm-ui's desktop entry point and
  nothing else. It exists as its own package because only a package can enable farhelm-ui's `desktop` feature
  unconditionally, and it is excluded from the workspace's `default-members` so that no ordinary
  `cargo build`/`test`/`clippy` compiles WebKit. `-p farhelm-desktop` is consequently the only thing that compiles it.

Motivation: the proto crate is the seam that keeps helm and supervisor honestly decoupled (they meet only over the wire,
even in-process). `farhelm` remains the single multi-call artifact provisioning ever has to move — farhelm-desktop is a
second release binary for the Mac desktop alone, never provisioned to a host.

## GUI: Dioxus

Dioxus, version pinned at the workspace level, rendering the same component tree in two targets: web (wasm32, real DOM,
served by the helm) and desktop (wry webview wrapping the identical DOM). No dioxus-fullstack / server functions — the
UI is a pure client of the helm's HTTP/WS API.

The launcher's managed type also has an unselected repository draft, which cannot carry preview authority or fall back
to the ordinary folder seed. Its field reuses the existing installation-bound repository discovery worker; explicit
`gh:` search owns that worker's query while scoped search is active, otherwise the repository field owns it. Selecting a
repository invalidates the preview generation even on same-repository reselection. The launcher and template editor
share one destination type control; template JSON retains its existing folder/github variants.

The composer keeps destination choice separate from agent mode. `github_checkout` owns the preview authority and
retained-attempt state; `launch_composer` owns pure scope parsing and history grouping. An async preview is usable only
for the same destination generation, host/incarnation/install claim, repository, title, agent choice and observed
configuration epoch. Key minting snapshots and rechecks that authority. A transport-ambiguous attempt retains its entire
body and key across configuration changes and ordinary conflicts. Authenticated reconciliation on the original
installation either returns the recorded result or durably refuses that same key before any allocation. Only that
refusal permits retiring a dispatched binding, including after a lost reply; the composer then refreshes its preview and
waits for another explicit submission. It never automatically substitutes a new key.

Motivation: the project's standing constraints — recorded here, because SPEC.md deliberately stays
implementation-neutral and does not contain them: as much Rust as possible, one implementation for web and native, and a
GUI that agents can test visually without a human in the loop. Those force a DOM-based Rust framework. Canvas-rendering
toolkits (egui, Iced, Slint) fail the testing constraint: Playwright against a canvas is blind screenshot-diffing with
no semantic selectors. Among DOM-based Rust options, Dioxus is the most active and has a first-party desktop story;
Tauri+Leptos would mean gluing two frameworks for no clear gain. Skipping dioxus-fullstack keeps the API a first-class
tested surface (the spawn CLI and test fixtures need it anyway) and avoids the framework's most churn-prone part.

Helm schema 50 stores `template_shortcuts` as a JSON map from one ASCII digit to one valid template name, initially
empty. A preferences write replaces the whole map; concurrent windows can lose an edit to that same map, which is
accepted. Each template may hold at most one key. Names need not exist at write time, just as the quick-launch mark may
be stale. Both template-deletion paths clear matching bindings and the quick-launch mark inside the deletion
transaction; restoring the template does not restore either shortcut. Older helms refuse this schema.

Helm schema 51 adds `welcome_seen` to the shared preference row. Unset and false mean unseen; both fresh and upgraded
helms start unseen. The dismissal belongs to the helm so closing the welcome card on one client hides it on every
client's next load. This migration prevents older helms from opening the database.

The preferences gate exposes its successful-read, unseen decision before mounting the authenticated UI. AppBar seeds the
welcome's open state once from it, separately from later preference changes; defaults after a failed or timed-out read
never count as unseen evidence. A page-owned consumed-decision flag survives authentication remounts; an initial
authentication refusal does not consume it before sign-in finishes. First-open dismissal explicitly restores the
selected, revealed terminal's focus, because its reveal behind a modal deliberately withholds that handoff. The card
uses the existing framed modal and isolation helper, with every close path available regardless of persistence.
First-open dismissal writes through the normal per-field preference queue; help-menu reopening writes nothing. AppBar
owns both welcome and feedback so their handoff releases isolation and replaces one modal with the other without a
competing focus-return task.

Helm schema 56 adds nullable `preferences.name_session_on_quick_launch`. Unset and true enable quick-launch naming; only
false disables it, so upgraded helms have the same default as fresh ones. The Settings switch under sessions, beside
session confirmations, writes an explicit boolean through the ordinary sparse preference queue. Null restores the
enabled default and omission preserves the stored choice. Older helms refuse this schema.

Helm schema 59 adds nullable `preferences.color_terminal_frames`. Unset and false leave terminal frames uncolored on
fresh and upgraded helms. The Settings switch under Sessions writes through the same sparse preference queue: a null
restores the off default, and an omitted key preserves another client's choice. Older helms refuse this schema.

Helm schema 60 adds `preferences.remembered_wrapper_prefixes` as an empty JSON map on upgrade. Each accepted user agent
launch replaces or clears its own agent type's entry in the same transaction that records the launch; agent requests do
not change it. An older helm refuses this schema.

The session list's chosen ORDER, last-selected session, compact-row choice, and the launch composer's remembered
permissions, host confirmation choices, and remembered feedback contact are one preference the HELM keeps, in a
singleton row of `helm.db` (`preferences`: `list_sort`, `last_selected`, `visit_history`, `compact`,
`remembered_permissions`, `remembered_workspace_trust`, `skip_host_remove_confirmation`, `skip_host_setup_confirmation`,
`feedback_contact`, `remembered_wrapper_prefixes`, `terminal_link_click`, `skip_session_confirmations`,
`name_session_on_quick_launch`, `color_terminal_frames`, `quick_launch_template`, `template_shortcuts`, `welcome_seen`)
behind `GET`/`PUT /api/preferences`, device-authenticated like every other route. The remembered permission field and
per-agent wrapper-prefix map are written by the helm after a successful user agent launch; no client PUTs them. A launch
without a prefix removes that agent type's entry. Agent-origin launches and command launches leave the map alone. The
`remembered_workspace_trust` column remains for upgrade compatibility but is neither read nor written. Remembered
permissions describe accepted launches rather than one client's claim. Both clients read the row once after
authentication — `PreferencesGate` holds the authenticated tree, rendering nothing, until the read lands, so the sort
control and the auto-select effect see the remembered values on their first run and no frame shows a default that is
then corrected. On desktop the IPC authentication gate already holds the tree and the read is one loopback hop, so
nothing is visible; in the browser the first paint deliberately waits on that one round trip to the helm — a page with a
valid credential used to paint its sidebar synchronously from localStorage — so the list never appears in an order that
then changes. A write is a sparse patch naming only the field the user changed, merged per-field by the helm (an absent
field is untouched, an explicit `null` clears one), so two clients changing different fields at nearly the same time
cannot clobber each other; the signal in the page is updated before the request leaves, which is what keeps the choice
in force when the write fails. Same-field writes are serialized latest-wins in the client, so a burst of changes cannot
land on the helm in reverse order; the write queue is process state outside the remounted tree, and after the browser's
credential recovery the gate overlays and replays any local choice whose write never got through, so reauthentication
cannot roll the current client back to the helm's older row. The desktop app never signs in again, so nothing remounts
its tree. The seed read runs under a seconds-scale deadline of its own and expiry reads as "nothing remembered", so a
stalled preference endpoint cannot blank the page for the funnel's full sixty seconds. Before each automatic session
choice the list makes one more GET under that same short deadline. Only selection/history inputs refresh; unrelated
settings retain their reload contract. A failed GET uses the current optimistic preference mirror. Selection writes
update both values in that mirror before queueing their existing single-field PUT, including recovery overlays.

The GUI's HTTP funnel orders reads and actions after this client's earlier configuration saves: quick-launch and
number-key bindings, template save/delete, checkout folder, post-clone command, and ntfy settings. A save registers
synchronously before its task is polled. Template name-validation reads capture their place before registering the save
they validate, so validation cannot wait on itself. Saves to different settings remain parallel; same-field preference
writes retain their serialized latest-wins queue. A dependent request snapshots the preceding tickets, including queued
preference choices, and waits only for those batches, so newer edits cannot starve it. Its existing deadline includes
that wait; only the remaining budget reaches HTTP. Completion, refusal, timeout and cancellation release tickets. Dirty
idle preferences are not pending work: failure or writer loss releases readers, including the reauthentication seed
whose completion restarts replay. A lost reply still has an unknown remote outcome: later requests can proceed while the
helm may still apply that save, with no new retry or remote-order guarantee.

Read/unread and notification marks, every preference field other than the template bindings, and host settings register
no tickets; host mutations can wait on remote operations and must not hold every read behind them. These requests still
wait for preceding tracked configuration saves. Reads and actions remain concurrent with each other. Terminals, the
event feed and the page JavaScript's own HTTP calls (attachments, file stat/download, client log and clipboard) bypass
this Rust funnel and are unaffected. No helm or protocol change is involved, and separate clients remain
last-writer-wins.

The sort travels as the bare word `?sort=` takes and is validated against that vocabulary at the write; the selection is
the full entry-bound session reference. Its entry UUID prevents a remembered selection from following a removed host's
bare session ID to another entry. Helm schema 66 adds `preferences.visit_history`, a JSON list of at most 32 full
references, newest first without duplicates. Each non-null `last_selected` write prepends its reference, removes its
previous occurrence and caps the list in the same database transaction as the sparse preference write. Null clears both
selection and visits; omission changes neither. Clients cannot PUT the history. Deleted references remain until later
visits age them out. Migration adds an empty list without inventing visits from a retained legacy selection; the next
selection write establishes the history-head/last-selection invariant. Older helms refuse the newer schema. An absent or
unrecognized sort word still reads as the UI default (`activity`) on the client, because the row outlives the build that
validated it. Nothing is kept per client: no localStorage key, no field in `desktop-client.json` (which holds no
credentials either, only the webview readiness counter the desktop smoke reads and the desktop app's automatic-updates
setting, which is a setting of that app installation rather than one of these shared preferences), no eval round trip.
The visible consequences are the ones SPEC.md names — one answer shared by every client, and a second client attaching
to its first listed surviving visit.

Keeping the order out of `SessionFilter` mirrors the helm's own split, and on this side the argument is about
reconciliation rather than about caches: what a reply COVERS is keyed to the filter — whether the banner may say the
list is filtered, whether a session's absence means it left the fleet, whether an optimistic rename may be retired — and
not one of those answers can change because the same rows arrived in a different sequence. Reply ADMISSION is the one
question that does depend on both: a listing answered under the previous order is a correct list of the wrong sequence
arriving under a control that names another one, so it is refused exactly as a listing answering a stale filter is. The
UI names the order on every read instead of leaning on the helm's `created` default, which is what keeps the control on
screen and the rows beneath it from disagreeing. Changing it is one more read, since every read is one request for the
whole list.

The listing is ONE request and one reply (SPEC.md's Session list section): `GET /api/sessions` answers with the entire
view — merged, filtered, sorted — up to the helm's cap, plus both counts and a `truncated` flag that is set exactly when
the cap cut it. The UI follows no cursor, keeps no ceilings of its own, and never asks a second time to reconcile what
it got. Because the rows and both counts come from one snapshot on the helm's side — with a cached row nothing can
render dropped from all three at the source — there is nothing for the list to change under and no count contradiction
to detect: the flag is the whole of what the UI reads off a reply about completeness, and the rows are never compared
against the counts (that comparison was the paged design's "underfilled listing" detector, and it went with the pages).
"Short" is one predicate with two readers rather than a rule each place restates: it is exactly the condition the count
banner prints "showing N of M" for, the same answer decides whether an absence may be read as a departure (otherwise the
missing row's optimistic rename is retired, its editor is left usable, and — if it is the selected one — its pane is
left alone). Automatic selection uses only listed candidates even on a short reply; it never resolves missing visits
directly. A complete authoritative absence changes a ListView-owned rename editor to an unavailable-target state instead
of closing it, preserving its draft for copying or cancellation; seeing the exact source again clears only that state. A
UI that tells the user its list is incomplete and then reasons as though it were complete would be disagreeing with the
one line whose job is to be believed.

One consequence is worth recording because nothing on screen shows it: the auto-select fallback (SPEC.md's
"newest-created session", when no eligible listed visit remains) cannot be assumed to be the first row of the listing,
because the first row is whatever the chosen order put there. It picks by the session's `created_at` instead, which is
why that field is decoded by the UI at all, and treats a missing stamp (an older helm) as unknown rather than as 1970 —
a fleet with no stamps degrades to the listing's own first row. It picks from the rows in hand even when the cap cut the
listing, where under a non-creation order the newest session may sit past the cut: a cut listing is a fleet of hundreds,
the fallback exists to keep the pane from sitting empty rather than to be exact, and the alternative was a second
request shape for that corner.

One pure chooser serves startup and removal: first listed history entry, else that newest-created fallback. When stored
history is empty, the retained last selection is one effective visit under the same eligibility and exclusion rules; it
is never appended behind nonempty history. This preserves the known choice after the empty-history migration, and the
ordinary automatic save establishes stored history. The list retains its most recent actually-open reference as a
transient exclusion when the pane clears, because a successful local delete can precede the listing refresh. The
exclusion applies to both history and fallback. One in-flight guard holds the pre-pick preference read; completion
checks the current selection, operation gates and latest admitted rows, so a manual selection during the read wins. No
eligible rows means no read and no pick. Automatic selections use the same selection writer as manual ones, so reload
remembers the resulting visit. This listed-only rule replaces the old direct restoration of a remembered session beyond
a truncated listing; absence still cannot retire an already-open session without the separate complete/unfiltered
disappearance gate.

The sidebar uses an identity line and, outside compact mode, a host/directory line plus a detail line when there is
ended status or a stale qualifier to explain. Its identity line is status, locality, title, a one- or two-glyph agent
badge, and a right-aligned activity-time column before the narrow menu gutter. The host line is the host name and
directory, joined by `:` when the helm supplied a name. This restores host visibility for confirmed local sessions too:
a host name is an identity fact, while the locality icon answers a separate question. The status and locality tracks
keep fixed icon-sized widths: live status uses the first slot's dot, compact ended status replaces that dot with a
distinct stopped, exit, interrupted, or error glyph, and an absent status or unknown locality leaves its slot blank. A
session with notifications adds a bell between the agent glyph (and its permission mark) and the activity-time column,
in both densities; it is absent, taking no width, when the list is empty, and its hover help and accessible name carry
the unread count. Compact rows therefore remain one visual line; host unavailability survives there as a neutral glyph
with the same cause phrase the regular row and session card show, in its accessible text and tooltip. The full status,
annotation, exit code, and qualifier meaning remain in accessible text and tooltips, and ended glyphs never acquire the
live dot's mark-read action. Noncompact ended details and qualifier words occupy their own full-width line under the
title through activity and above host/directory. Detail wraps unbroken peer text at any boundary without ellipsis,
clamping, or widening the menu gutter. The activity track has a four-character minimum and grows for unbounded ages such
as `1000d`. Agent glyphs are max-content rather than a text-badge allowance: an agent launch's agent type and a command
launch's declared agent type are authoritative, a legacy row gets its stored agent kind's glyph and the unclassified
permission mark, and a command with no declared agent type gets the terminal glyph; nothing is read off a command line.
C/M/L/G/P are Farhelm letter paths for Codex, Muse, Claude, Goose, and Pi; OpenCode uses its attributed inline mark, and
an unknown command uses the neutral terminal glyph. A legacy row with no name leaves that fact absent.
`list::shared::session_locality` decides among three answers rather than two — `Local` when the session's host id
matches the registry's `HostKind::Local` row (never by name; see that function's own doc for why), `Remote` when both
ids are known and differ, and `Unknown` when either is missing (an old helm sending no host id, or a hosts read that has
not landed). A confirmed local glyph uses the semantic red caution color, including selected, stale, and compact rows,
to keep local execution conspicuous. The row draws the LOCAL glyph only for a confirmed `Local` verdict — an `Unknown`
row draws no glyph at all, never the local one, because a glyph is a positive claim `session_locality` has no evidence
to back. The 2026-08-23 rule's weaker promise survives underneath: unknown locality still never SUPPRESSES an available
host label, it only ever leaves the row free to show one it already has, and the glyph rule adds a second promise on top
rather than replacing the first. Legacy rows without a host name at all necessarily show none regardless — locality
answers whether a name would be shown, not whether one exists to show. The agent track is rendered as glyphs: structured
launch metadata decides the harness: an agent launch's agent type, a command launch's declared one, or a legacy
session's stored kind, and the terminal glyph otherwise. The sidebar's YOLO mark and the helm's YOLO confirmation must
agree on every launch by reading the same verdict, `SessionLaunch::yolo`: `farhelm_proto::yolo::selection_is_yolo` for
an agent launch and the user's (or agent's) assertion for a command launch. Nothing reads a command line for it. A
legacy session has no verdict and carries the unclassified mark; nothing that can create a launch holds one, so the
guard never asks about it. An omitted Pi, OpenCode, OMP or Goose permission is rendered as YOLO for compatibility with
older snapshots. The full invocation remains in its accessible text and tooltip. The working directory is tilde-folded
against the `/home/<user>` and `/Users/<user>` shapes, since no home directory is on the wire to fold against properly.
The full invocation stays on the agent mark; the untouched directory and other row facts ride in a hidden summary card
source beside the row. The open button names that source with `data-tooltip-card`; the shared tooltip clones its DOM
children on hover or keyboard focus. The source is a sibling, not a row descendant, so row-scoped controls and locators
cannot match a second, hidden copy. Rust renders the host's own mark, status classes and isolated peer text. The card
gives the full title, host, managed checkout, directory and timestamps without changing the open button's accessible
name; marks keep their nearer hover targets and the complete ended-status detail stays on the row. The source and cloned
tooltip remain outside the accessibility tree.

Each live dot carries its status word on the dot itself, with the optional mark read / mark unread action following it.
Permission marks use hand-drawn stroke silhouettes in the 12-unit viewBox: a slashed shield (`data-glyph="yolo"`) and a
question mark (`unknown`) use `--warn`; a plain shield (`shielded`) uses `--ok`. The badge's permission is non-optional.
Agent-launch non-YOLO modes share the plain shield but retain their own mode-specific descriptions; a command launch
gets the plain shield only from its own not-YOLO assertion, and a legacy command gets the question mark. An own-setting
launch gets a dashed shield (`own-setting`) in `--fg-1`, while still counting as YOLO for the host guard. Both slots
remain fixed-width.

The agent and permission SVGs sit in separate tooltip targets, so hovering the permission mark explains its mode instead
of returning only the combined agent summary. The combined summary remains on the agent track for provenance and the
full invocation.

`status::status_badge` supplies the status wording; the row chooses its presentation according to compact mode. Live
states keep their text for screen readers alongside the colored dot. Ended states use a distinct icon in compact mode,
with the complete wording in accessible text and a tooltip. Outside compact mode the wording gets a full-width wrapping
line. Keeping that detail beside the title used to truncate it despite unused space elsewhere in the row, and keeping it
visible in compact mode made stopped rows taller than live ones. The two presentations preserve the same facts while
honoring the user's density choice.

The relative age beside it needs a `now`, and there is no honest one on the wire. `last_activity_at` is written by the
session's HOST and compared against the VIEWER's wall clock, which on a remote helm is a different machine and in a
multi-host fleet is several of them at once. The UI corrects for none of that. The one reference a client could obtain —
the helm's own clock, off an HTTP `Date` header — would fix at most one of the N edges involved and would lend the rest
a precision they do not have, so the code instead refuses to print nonsense (a stamp in the future reads `now` rather
than a negative age) and keeps the raw stamp one hover away. The `Session` mirror decodes `last_activity_at` for this
and applies the proto's own fallback rule, `last_activity_at` when positive and `created_at` otherwise, by calling
`farhelm_proto::effective_activity` rather than keeping a copy. The UI depends on farhelm-proto with its tokio-based
frame I/O feature turned off, so it builds for wasm, and shares the leaf wire types the helm forwards verbatim (session
status, restart offer, tab, launch selection). It keeps its own decoders for what the helm shapes for HTTP (session
rows, hosts and the reply envelopes), because those tolerate words a newer helm may send to a browser tab still running
older code; shared golden files under `crates/farhelm-helm/http-contract/`, serialized by the helm's tests and decoded
by the UI's, keep the two sides from drifting. That rule governs the displayed age and the seen/unseen comparison only —
the helm orders an activity-sorted list by reported status first and the work-start key inside each group, so the age
column is deliberately not a rank column: a row above another can show an older age, and that is the contract rather
than a contradiction. A zero means "this helm predates the field" and renders no age at all rather than an age counted
from 1970. The viewer's end of the subtraction can go missing too — a platform clock that will not answer, or one
sitting at or before the epoch — and that is carried as an absent value rather than as a zero, because subtracting a
good host stamp from a zero "now" would clamp every session in the fleet to `now` and paint a dormant fleet as a busy
one.

Ages advance on a dedicated 30-second tick — one page-wide signal, written by a component mounted beside the
invalidation feed and read by the list and the open session's header. The listing's fallback poll was the obvious thing
to reuse and is the wrong one: it runs only while the feed is DOWN, so on a healthy page it never fires at all. The
signal outlives the component that writes it, so the component republishes the current time at MOUNT before starting its
loop: it is unmounted and remounted whenever the authenticated tree is rebuilt, and without that first write the page
would spend a full tick — or, after a reauthentication that followed a long idle, much longer — rendering ages against a
reading from before the gap. The list formats each row's age itself and hands the row a finished string, which is what
keeps the tick from re-rendering rows whose displayed age has not moved — an `8h` row survives sixty ticks comparing
equal.

Styling is the single hand-written application stylesheet, `crates/farhelm-ui/assets/app.css`: plain CSS, with no
preprocessing and no framework transformation, though Dioxus still registers it as a packaged asset and decides its
served path the same way it does every other asset. xterm.js's own look is a separate vendored stylesheet
(`vendor/xterm.css`), loaded alongside app.css rather than folded into it. Its colors, font stacks, and non-zero corner
radii are declared once as CSS custom properties in a `:root` block at the top of the file and referenced as
`var(--token)` everywhere else; the rule is that no use site holds a literal, with one carve-out — a structural `0`,
where a corner has to stay square because it joins a neighboring control, names no design value and stays a literal. The
tokens are named for the role a value plays — surface levels (`--bg-*`), foreground levels (`--fg-*`), one accent
family, `--ok`/`--warn`/`--danger` with their fill and border variants, `--radius-*`, `--font-ui`/`--font-mono` — rather
than for the color it happens to be, so a restyle is an edit to one block instead of to every rule that mentioned the
same hex. The palette is dark-only today; a light theme lands as a second `:root` block redefining the same names, which
is the arrangement the no-literals rule exists to protect.

Two of those roles are design constraints and not merely names. The first is the surface ladder: exactly three levels
are in use — the ground (the page), the chrome one step above it (sidebar, main header, tab strip), and the floating
level one step above that (menus, dialogs, forms, bands that interrupt a pane). All three lean cool by the same small
amount, the ground included: it is a near-black and not `#000`, because pure black would be the one untinted surface in
a set meant to read as one material at three depths. The terminal's own `--terminal-bg` surface, Ghostty's default
background for an out-of-the-box readable terminal, is not a rung on that ladder: it is the color xterm.js paints,
mirrored so the pane's gutter and viewport agree. Which level an element sits on is recorded in the `:root` comments; a
`--control-hover-bg` and a `--chip-bg` token fill a bordered control's hover state and a small chip respectively, named
for that specific role rather than folded into the `--bg-*` surface family, so neither one reads as a fourth and fifth
level to lay something out on. `--well` is a third fill of that kind: the inside of an input, select, or code box,
darker than any surface a control can sit on so that a field reads as cut into its panel, and lighter than the ground so
that a field on a dialog does not read as a hole through to the page. The second is that there is ONE accent, and what
it may be spent on is a closed list rather than a palette to decorate with: selection, `:focus-visible`, normal primary
actions, any PRESSED disclosure control, and the wait after a confirmed delete (the spinner and words on the session's
row and over its terminal, and the dot and words in its header). That last entry is the maintainer's choice
(2026-09-30): the wait is the outcome the user just confirmed, so the danger red it used to be read as a failure; grey
is what an earlier indicator used, and it went unnoticed; and `--info`, the only other blue, already means idle with
unseen output. It is the one entry that does not mean "this is where you are" (see below), so it is kept narrow enough
not to compete with selection: it colors only the spinner or dot, the words, and the terminal overlay card's edge, and
the deleting row takes no tint or edge of its own, which would look exactly like the selection it usually already is.
Action buttons use three deliberate tiers on the shared ghost `.btn` base: `.btn-primary` is the normal blue
affirmative, `.btn-neutral` is the quiet secondary treatment, and `.btn-danger` is reserved for destructive
confirmations. A permanent answer uses the outlined form of its one-off tier: the YOLO confirmation's "start, and don't
ask again on this host" and host removal's "remove, don't ask again" keep the danger color, while host setup's "set up,
don't ask again" keeps the primary color. Each leaves its one-off action as the filled choice. The maintainer chose
(2026-09-30) not to make a permanent answer the filled one that draws the reflex click. The host row's inline update
action is another deliberate outlined treatment: amber (`--warn`) for an optional update and red (`--danger-strong`) for
a required one. Its outline signals urgency without presenting an update as a destructive confirmation. Being outlined,
it preserves the sidebar's one-filled-control rule. Pressed disclosures use `--accent-fill-hover` so an open trigger is
distinct from a resting primary. The normal-primary entry is scoped per SURFACE, not per screen: the sidebar's resting
chrome carries exactly one filled control (`new session`), and each dialog or popup that floats over it may supply its
own affirmative primary. The sidebar's secondary actions use the neutral tier; menu items, tabs, composer selections,
relays, and other explicit exemptions retain their ghost or purpose-built styling. Destructive menu items remain red
text, while their confirmation buttons use the danger tier. SPEC.md requires the sidebar to mark the selected session's
row readably at a glance, so anything else joining that list has to be a place where the accent means "this is where you
are" — the same thing every entry but the delete wait says — because an accent spread across ordinary decoration would
leave nothing to make the selection readable. Both constraints have a contrast floor under them: the quiet foreground
tokens are set so that metadata stays at WCAG AA against the brightest surface it lands on, which is what caps how light
the selected row's fill may go.

Selection is one construct wherever it appears — the sidebar's selected row, the selected tab, and the launch composer's
chosen harness, segment, folder, and list option: the accent-tinted `--accent-fill`, an accent bar along one edge, and
`--fg-bright` text. The bar takes the leading edge of a thing chosen from a vertical list and the underside of a thing
chosen from a horizontal set. The fill is deliberately quiet, only as chromatic as it takes to stay apart from the
neutral hover tint beside it, and the bar is what announces the selection. The reason is what else is on screen: status
colors are meant to be the loudest thing in the sidebar, and each dot-only status has a shape of its own as well as a
color (filled circle running, triangle waiting, diamond idle with unseen output, hollow ring idle), so the session that
needs a person outranks the one already open, and the sidebar can be read without telling hues apart. The launch
composer used to carry a palette, corner radii, and a periwinkle selection of its own, a second accent that the closed
list above rules out; a dialog now uses the shared tokens and adds only what being modal needs (a veil, a panel edge one
step brighter than a control outline, a shadow), on the same one corner scale as everything else.

Nothing on screen may redraw on its own more than 10 times per second. "On its own" means an indicator or decoration
that moves, fades, spins, or blinks while nothing about the state it shows has changed; drawing new content because new
data arrived (terminal output, a list update) is not animation, and this rule does not cover it. In this codebase that
means a CSS animation that repeats indefinitely (`infinite`, or any iteration count that keeps it running with no end in
sight) uses a stepped timing function, `steps(n)`, `step-start`, or `step-end`, at no more than 10 steps per second
counted across the whole cycle (the timing function applies to each interval between keyframes, so `steps(4)` over a
three-keyframe pulse is eight changes per cycle). Interpolating timing functions on such an animation (`linear`, `ease`,
`ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(...)`) are forbidden, and so are `requestAnimationFrame` loops and
JS timers that change what is drawn more often than 10 times per second. That list explains the rule; it is not a
whitelist, and a technique it does not name that redraws every frame is just as forbidden. One-shot transitions and
animations started by a user action or a state change, and done drawing within about a second, are allowed (a delay
before one starts draws nothing and does not count): the 100 ms hover tints, or xterm.js's 800 ms scrollbar fade. Every
looping animation also stands still while the window is inactive, meaning the window or tab lacks keyboard focus or is
hidden or minimized, which includes a Farhelm window visible beside another app that has focus; one universal
`animation-play-state` rule in app.css, keyed on an attribute the UI root keeps in step with focus and visibility,
covers present and future animations alike. That rule freezes one-shot CSS animations as well, so one that may start
while the window is inactive has to read correctly at its first keyframe, or be written as a CSS transition, which the
pause does not touch. Under Reduce Motion (`prefers-reduced-motion: reduce`) every looping indicator is static.
`js-tests/app-css-animations.test.js` enforces the stepped-timing half against app.css.

The reason is CPU and battery, not taste. An interpolated animation produces a new frame on every display refresh for as
long as it runs, so on macOS the compositor (WindowServer) redraws the window at the display's full rate, up to 120 Hz,
and the display cannot drop to its low-power idle refresh. A single running-status pulse, `2s ease-in-out infinite` on
opacity, was observed holding WindowServer at roughly 40-50% CPU on a 120 Hz MacBook, even with the window hidden; that
is one observation on one machine (2026-10-02), not a constant, but turning Reduce Motion on made the load go away and
turning it off brought it back. Animating only opacity or transform does not get around this: it keeps the page's own
layout and paint idle, but the compositor still redraws every frame the value changes, so "compositor-only" does not
make a looping animation cheap.

`--font-ui` and `--font-mono` name the same vendored face — JetBrains Mono Nerd Font, described below in the xterm.js
island section — rather than two different ones. The chrome (`--font-ui`) and the terminal (`--font-mono`, the stack
terminal.js hands xterm.js) used to differ, chrome sitting on the platform's `system-ui` face; unifying them means
chrome-only pages that never open a terminal (the auth screen, an empty session list) now load the face too, and chrome
text reads as the same typeface as whatever the agent prints instead of pairing a generic UI font against a distinctive
monospace one. That extra load is a WOFF2 fetch rather than the vendored TTF's — a lossless re-encoding at roughly 40%
of the TTF's size — and once either surface has fetched it, the browser serves the other from cache rather than fetching
it a second time. Form controls get the face by default rather than by opting in: user-agent stylesheets give `button`,
`input`, `select`, and `textarea` a platform face instead of letting them inherit, so one zero-specificity rule in
app.css makes them inherit `font-family`, and a control added later cannot fall back to the platform sans-serif the way
most of the launch composer's buttons once did. The rule leaves font size alone, and it excludes xterm.js's own helper
textarea, which belongs to the vendored widget.

The open session's chrome has one top row with terminal tabs on the left and Restart, Restart with, Replace, Clone,
Replace with, and Delete on the right, and a slim details bar below the terminal. The bottom bar is ordered status,
title, host, age, managed-checkout branch mark when present, copyable `{cwd}`, and copyable invocation. The host uses
the same label component as the session list, reading the shared `Signal<HostsRead>` context rather than starting
another host request; its name remains visible when locality is unknown, and only a missing name omits it. Identity
moved below the terminal to make the active session and host easier to see while keeping the bottom bar small and
leaving room for the tabs and actions together at the top. The terminal keeps at least its previous steady-state height.
Earlier, four stacked bands cost roughly 170px before the terminal started, on a surface whose entire point is the
terminal; they were consolidated into one roughly 40px header above the tabs before this split. Two of those bands had
to go somewhere rather than merely shrink. The restart offer's explanation became the restart button's tooltip and its
`aria-describedby` target: SPEC.md's rule that Restart always resumes and, when it cannot, is greyed out with the
specific reason is carried by the button's accessible name (`aria-label` and, alongside the further elaboration, its
tooltip) — naming the offer (`resume conversation` or `restart unavailable`) rather than the action. An unavailable
Restart, like an unavailable Restart with, is `aria-disabled` rather than natively disabled, so its tooltip stays
hoverable in webviews and its click handler is what refuses. The directory and invocation buttons carry their full
values in their tooltips, shrink before the session title and host, and reveal a clipboard affordance on hover or
keyboard focus. Their hidden-characters copy warning opens upward from the bottom bar so the window edge cannot clip it.

Copy feedback snapshots that warning from the copied value alongside its click id. A metadata refresh leaves the warning
and its existing expiry unchanged; an older click's timer cannot clear newer feedback.

The six action buttons remain fully visible and in DOM order from a 650px main pane; the tab strip shrinks into the
remaining width and scrolls horizontally. The app's 320px main-pane floor is unchanged; between those widths the row may
clip its trailing actions rather than wrapping or hiding them. The restart confirmation became a popover anchored under
the button that opened it, still confirm-in-place with focus on the enabled confirming answer (Cancel while disabled);
the consequence sentence they lead with is the one line standing between a click and a killed process tree, and a header
that kept it in flow would have to either wrap or truncate it. Header Replace has a separate anchored confirmation state
so it cannot accidentally open the interrupted card's confirmation. The top row owns their Escape handler alongside the
action buttons; keyboard order is top row, terminal, then bottom bar. Everything conditional — a refused restart's
prose, the last-known-status band and the "helm stopped listing this session" line — is still a full-width band, because
a band that only appears when it has something to say costs the steady state nothing. These notices, the interrupted
card, trust hint and tab errors remain between the top row and terminal, followed by the desktop tunnel strip. A
classified status renders in at most one place: the bottom bar normally, the stale notice's own metadata band for a
stale session (where SPEC.md's title/directory/last-known-status triple is assembled), and nowhere at all for a session
nothing has classified yet.

An unavailable session uses one accent card over the terminal scrim, sharing Delete's card geometry and spinner.
`hosts::session_unavailable_cause` maps the existing host read to a stable phrase for both row densities and the card;
retry counts and escaped peer diagnostics stay in smaller print. The remedy is preserved verbatim. A pending registry
read says checking, a failed read names that failure, and a connected host under a stale session says refreshing. The
browser's reconnect card uses the same CSS with its own counter and neutral retry button. Recoverable detach notices
never paint the red banner; takeover, stall and terminal refusals still do. A session-owned resize observer gives the
card the viewport when less than 80px of terminal height remains, so conditional chrome cannot hide its explanation.

The session view remembers whether it has offered terminals. Staleness retains those islands and makes their panes and
tab strip inert; a freshly opened stale view offers no terminals. The existing recovery snapshot's xterm element moves
beside the replacement mount so hidden replay does not hide the old screen. It is disposed only at successful replay
reveal, or restored into its mount for a final refusal. Pane visibility still hides background tabs. Availability in the
synced terminal specs nudges waiting recovery controllers once when the host returns, through the existing automatic
`if_unowned` path, without changing the retry ladder or disturbing attempts already in flight. Takeover tombstones and
the build-mismatch capability veto remain authoritative. Interruption after a reboot still empties the desired set.
Replay callbacks from a socket that died meanwhile cannot retire its new recovery. If replay reveals while the pane is
still inert, the availability edge completes the focus handoff through the same guard used by reveal, preserving
deliberate focus in fields, dialogs and other keyboard owners. Inline session questions share a keyboard-owner marker:
header Restart, Replace and Delete, sidebar Delete and Replace, interrupted-card Replace and tab Close keep focus on
their answers across replay reveal, availability recovery and automatic selected-tab fallback. The guard follows focus
inside the question, so explicitly selecting a terminal still allows typing there. Header Escape cancellation keeps its
existing post-unmount return to the selected terminal.

Restart with uses a separate dialog because it relaunches the current session rather than creating one. For an agent
launch it renders the same `LaunchControls` component as the session launcher, with the harness fixed to the session's
stored selection. For a command launch that declared an agent type and a resume command it shows the command, the resume
command and the YOLO answer instead, checks the edit in the browser with the same `CommandLaunch::validate` the helm and
supervisor apply, and sends it under `with_command` rather than `with`. Its two command fields follow the launcher's
peer-text rule: they show the stored commands escaped and send the stored bytes back exactly when untouched. One
component serves both kinds, so the focus rules, modal isolation, YOLO question and footer below exist once. The dialog
owns its draft and comparison baseline; the launcher keeps its own create-only state and effects. Only edited fields get
changed markers; a marker's old model is a stored string and renders as an escaped, direction-isolated peer value. The
dialog's submit uses Restart's stop-first consent and handles the reply through the same terminal reattachment path as
an ordinary restart. That path reattaches the terminal even when the restart is refused and the dialog stays open,
beneath a modal whose keystrokes must never reach the agent.

The dialog owns keyboard focus structurally. While it is mounted, every sibling of every element on its path up to
`body` is `inert`, including siblings rendered after it opened, except the agent approval cards (`data-modal-exempt`),
which stay live so that an agent waiting on a card does not also wait on the dialog, so no other code can focus anything
behind it and Tab from `body` can only reach the dialog. Only elements that were not already inert are marked, and
exactly those are restored when it closes, before focus returns to the header action. A capture-phase keydown handler
covers focus that still ends up outside the dialog (a click on the scrim leaves it on `body`): it swallows that key and
puts focus back on the dialog, and Escape still cancels unless a request is in flight. The earlier per-mechanism guards
stay as a second layer for engines without `inert`: a terminal whose output becomes visible, or that becomes the
selected terminal (as when the selected tab exits and the view falls back to the agent), does not take focus while the
dialog is mounted, and during a request the primary action stays focusable (unavailable through `aria-disabled`) while
the other controls are disabled, because a focused control that is natively disabled or unmounted drops focus to `body`.

Hosts use one permanently mounted list beside the session list, not a compact summary plus a second management panel.
Its one-row header gives the known host count, an unpersisted global details checkbox, and the secondary add control.
Every row always shows its name, phase dot, and a muted actions toggle that appears on hover, keyboard focus, or a
coarse pointer, in the same narrow trailing gutter as the session-row actions toggle; connected spends no visible word
unless the helm marks a compatible older build, in which case the amber `old version` advisory is shown, or a newer one
(`newer_version`), in which case the amber `too new` advisory is shown. A skew whose peer protocol is the higher one
also reads `too new`, but keeps the red styling of every skew, since that host cannot be used until one side is updated.
Either way the `too new` label's hover names both versions, and Update (and a rerun of a failed Update) is not offered.
Other phases use humanized prose and retain the stable wire token in their data attribute. A protocol-incompatible
supervisor remains the red `needs update` case; an unparseable build leaves a connected host's age unknown and keeps the
ordinary connected label. When an older SSH host has Update available, a compact outlined `↑ update` button replaces
`old version` or `needs update`, amber for the former and red for a lower-protocol peer. It shares the fleet update
eligibility predicate (remote kind, not newer, menu offer present, no live provisioning work) and sends the same bound
Update request as the menu. The button sits outside the status region; that region retains its phase words through an
accessible label. Its hover uses escaped peer builds and explains urgency and installing the helm's version. Current,
newer, local, busy, and not-yet-loaded rows keep their existing presentation. Each row's effective disclosure is the
global checkbox OR that row's automatic provisioning disclosure: the checkbox is the user's preference and no run writes
it. Update stays folded during planning and execution, shows a pending status until a snapshot is available, and then
publishes compact step/count/elapsed progress beside the row status. Setup and uninstall show their own pending verbs;
their first running snapshot opens the row once, with a fold choice kept for that run id. Failure opens the row for its
diagnostic, and completion clears the one-off expansion. A failed Update or unresolved diagnostic also opens its row;
authoritative Update success clears the automatic half for the tracked run. Provisioning commands live in the row menu,
with the older-host update shortcut also inline, but active or retained progress stays under the row because that
lifecycle owns more context than a floating menu can safely hold. Setup and uninstall confirmation dialogs render
independently of row details, while a failed retained run leaves one short trace when details are closed. A run whose
status is showing inline leaves that trace out because it would only repeat the progress.

The app bar's gear opens a modal `settings` dialog as a sibling of the sticky bar, so the bar's stacking context cannot
cap the backdrop below main-pane surfaces. Its sessions section holds session confirmations, quick-launch naming and
terminal-frame coloring; hosts holds the two app-wide host-confirmation choices, with state-specific help and an
explanation that other open clients see changes on reload, and, only in a desktop app whose updater is active, the app
section holds the automatic-updates switch, which reads and writes the desktop state file instead (see The desktop app's
updater). The desktop file-opening field lives in the shared preference row but bypasses the best-effort preference
queue: it sends one acknowledged sparse `PUT /api/preferences`, keeps a refused draft editable, and updates the shared
client mirror only after success. Its warnings use the same parser and type classifier as the helm's stat and open
checks. The switches retain native checkbox inputs for label activation and Space handling. Each host-confirmation
switch reads `Some(true)` as without asking, updates `SharedPreferences` immediately and sends an explicit boolean
through the existing sparse preference queue. Host-confirmation preference failures retain that queue's silent behavior.
The session confirmation switch also updates locally immediately, but restoring confirmations observes the queue's
completion: a failed save shows an adjacent Settings error because a reload could restore the stored opt-out. Each click
advances a component-local generation and clears the error; only the newest restoring click can show a failure. The
enqueue happens before spawning its waiter so a later silent opt-out cannot overtake it. Closing Settings cancels the
waiter, never the process-owned preference writer. The dialog uses the host dialogs' framed header and footer, with a
gear tile and a separately named × close control. Only the body scrolls, containing a section-button sidebar and the
selected panel. The order is sessions, hosts, terminal, checkouts, agent trust, notifications and, when the updater is
active, app. Terminal holds web-link choices and desktop tunnel consent and file types; checkouts holds the
explicit-save folder and command; agent trust holds all four agents' blocks; notifications groups device sounds and
helm-wide ntfy alerts. Inactive panels stay mounted with hidden, preserving field drafts, outcomes and pending saves.
The sound asset's document-wide input query initializes even hidden inputs. No section choice is persisted. At 620px and
below, a presentation-only signal reveals either the selected panel or the section list; the back button reads '‹
settings'. Section selection focuses its heading, while back focuses the selected section button after the rendering
frame. Neither adds a normal Tab stop or overrides platform button navigation. Both close controls stay visible. It uses
the shared focus/isolation helper with its own selector; gear opening selects sessions and focuses its first switch.
Existing trust props instead select agent trust before mount, focus the agent's switch and scroll its block into view.
Escape or either close control releases isolation and returns focus to the gear. The host setup and removal permanent
answers each name that gear as the way to turn confirmation back on.

The all-hosts checkout folder uses a shared explicit-save field rather than the preference queue: invalid path text must
have a visible refusal, and a draft is not configuration. `GET`/`PUT /api/checkout-root` is an ordinary protected route
with a required nullable `root` and no host or command fields. It calls the existing global setter/clearer; the revision
watcher is the sole invalidation publisher for HTTP and CLI writes. The saving field refetches on success and invokes
its caller's callback so dependent reads need not wait for the watcher. A submitted save is app-owned and survives
dismissal; its outcome and callback touch component-owned state only while the field remains mounted. The browser reads
only the global folder through that route. The field keeps its refused draft and reports success.

The all-hosts post-clone command uses its own protected `GET`/`PUT /api/checkout-post-clone` route with a required
nullable `command` and no other fields. Null is unset, an empty string is explicitly disabled, and nonempty text is
stored verbatim; neither configuration route exposes host overrides. The handler checks the shared 16 KiB cap before
calling the existing global setter/clearer, returning a visible 400 refusal without quoting the command or changing the
revision. Its Settings-only multi-line field follows the folder field's draft, refusal, app-owned save and refetch
contract; Enter inserts a newline. Only an explicit save maps an all-whitespace draft to null. Opening the dialog
displays an explicit disable as an empty draft without writing it. Existing revision checks and frozen supervisor launch
specs keep an accepted preparation and its retries on the command accepted with it.

The repository-discovery REST reply carries `needs_checkout_root`, derived from the selected host's effective
configuration, without changing the supervisor protocol. The launcher retains an accepted setup observation, including
either answer, for that host connection and installation so selecting a repository cannot hide inline setup or prevent
its first check. It continues discovery while choosing, awaiting that first answer, or needing setup; a successful save
restarts discovery and advances preview authority explicitly. Once a reply confirms a folder is in effect, selected
repositories return to preview-only behavior. Global-root reads and message text never decide host setup eligibility, so
command-line overrides remain authoritative.

The host actions menu follows the session menu's anchor, pointer, raised surface, header, grouped inset commands, line
icons, muted descriptions, roving keyboard focus, and one-menu-at-a-time dismissal rules. The add-host fields, probe
outcomes, and setup confirmation live in a modal dialog; a permanent setup answer is shared through the helm preference
and lets later setup-needed adds submit after probing. The dialog never holds the user behind a pending request: its
cancel and Escape stay live while discovery or another operation's request is out, and confirming setup closes it at
once. The submission then continues from the hosts panel, which keeps the page token until the helm answers; the new row
tracks an accepted run. A definite refusal reopens the add dialog with its destination and reason but without the
consumed plan; a lost reply stays on the panel because the run may have started. Remote setup rerun, update and
uninstall planning failures, and definite confirmed setup rerun or uninstall refusals, use one host-keyed refusal
dialog. Retry and adopt refusals use that dialog too. Remove opens the modal `HostRemoveDialog`; the modal is isolated
from the page, starts focus on Remove when enabled (Cancel otherwise), and returns focus to the row toggle when
cancelled. It stays open through its request and shows a refusal there; when the remembered preference skips
confirmation, a refused Remove opens the short refusal dialog instead. The dialog's permanent answer writes the shared
helm preference best-effort before removing the registry row.

Host Settings waits while any host prepares or holds an ADD/Uninstall confirmation, so independent modal isolation
cannot lock both dialogs. Another host's one-click Update or accepted run does not block it; the host's own planning or
busy state and the page operation lock still do.

Every per-session action lives in one floating actions menu behind the row's `⋯`, and four decisions about it are
contract rather than styling. **Anchor:** the panel opens just beyond the sidebar's right edge, with its top aligned to
the row and a small pointer toward it. It stays beside the session list instead of covering neighbouring rows and their
action toggles. One side placement is clamped to the viewport, including when there is too little room to keep the
sidebar fully uncovered or to keep the panel top aligned with a row near the bottom. The toggle holds a pressed accent
state, its row holds a tint, and the panel is a raised surface with a shadow. The action list starts with the session
title and a muted summary of its stored launch selection, followed by the concise state. The summary uses the same
launch-choice wording as the session launcher; a legacy session uses its command's program name, and an unclassified
session omits the state word. The pointer is hidden when horizontal clamping makes the panel overlap the sidebar, where
it could no longer indicate the opening row. Commands have small decorative line icons and form groups separated by
non-focusable rules: rename and mark read/unread; clone, replace with, and replace; stop; delete. Only groups with
available commands contribute rules. Clone, replace with, replace, stop, and delete each have a visible muted
description exposed as an accessible description, so the accessible command name remains the action word. Hover and
focus fill each command inside the panel with rounded inset corners. **One at a time:** at most one row's menu is open,
and it closes on any layout change that could have moved the row it was measured against (a sidebar scroll or resize,
the host list's shape changing, the create form opening, the row reordering under a refresh), because the panel's
coordinates are a one-time snapshot. **Keyboard:** it is a real `role="menu"` and behaves like one — opening it
(pointer, Enter, Space, ArrowDown) lands focus on the first command and ArrowUp opens onto the last; arrows step and
wrap, Home/End jump; the whole menu is a single tab stop via roving `tabindex`, so Tab leaves rather than walking the
commands; Escape closes; and every close that took the menu away from a focused item hands focus back to the toggle
rather than dropping it on the document body — except the two transfers, Rename and a clone/replace-with acceptance,
whose newly mounted dialogs own focus instead, so the teardown retires its return rather than racing their mount
handoff. An item made inert by an in-flight operation stays focusable and refuses on activation (`aria-disabled`) rather
than going natively `disabled`, because a browser cannot focus a disabled control and a menu that went busy under the
user would otherwise swallow every navigation key. Arrows step from whichever item actually has focus, including one the
user focused by pointer or screen reader in the middle of an arrow burst. A page-side check rejects navigation focus
moves after independent focus, including requests already sent across the desktop webview. Each request retains the
page's independent-focus epoch from its key press; a synchronous input relay updates Rust with the current focus event's
source before the original event, so a user landing at the in-flight target also retires older requested destinations.
Initial menu focus uses the same function without that check and does not invalidate navigation. Nothing activates
without Enter or Space on the visibly focused item. The row tint is owned by the menu's open state, not by
`:focus-within`: a dismissal may return focus to the toggle while the pointer is elsewhere, and that focused toggle must
not make the row look as though its menu is still open. The focused toggle or menu item retains the normal
`:focus-visible` indicator, so pointer-return focus and keyboard focus remain visually distinct without changing the
dismissal or focus-return contract. **Confirm in place:** a destructive item swaps the panel's own contents for the
consequence line and a confirm/cancel pair with focus on the enabled confirming answer, or Cancel while busy, rather
than opening a second surface; that sub-state is a `role="dialog"` inside the same positioned box, and it survives the
panel closing, which is why it deliberately does not answer Escape. The one exception is a pointer going down outside
the open menu: that closes the menu and answers a showing confirmation with cancel, since a click elsewhere is the user
leaving the question, and a prompt kept for the next open would greet them with a question they already walked away
from. Every other dismissal still preserves it. Outside dismissal also never takes focus back from the control the
pointer went to: the toggle handback only reclaims focus that is on nothing, on the document body, or still inside a row
menu.

Each session confirmation mount schedules focus on its enabled confirming answer, falling back to Cancel if disabled.
The script supplements native autofocus because browsers can ignore it after a document's first prompt, including when
an existing question remounts. Confirm buttons consume repeated or composing Enter before native click synthesis: a held
opening key cannot also answer the question. A shared header key handler cancels any still-open Restart, Replace or
Delete question on Escape, releases its lifecycle claim and returns focus to the selected terminal after unmount. Host
dialogs retain quiet scripted focus and independent Escape selectors; trash focuses and scrolls its confirming answer
into view. YOLO and Restart with keep their Cancel default.

The dialog installers share a pointer rule: primary mouse-down on unused space cancels the browser's focus change, but
controls and text keep their native behavior. An Enter listener runs only when the dialog container itself has focus; it
submits the launcher form or clicks the dialog's named main button. The selector is resolved at keypress time so a
button mounted by a later confirmation or disabled during a request keeps its own rule. Inline session confirmations use
only the gap rule. Modal dialog containers have no focus ring; controls and the nonmodal notification panel retain their
keyboard rings.

Mark read/unread and stop close the menu as soon as the handler accepts the choice. Their asynchronous failures still
appear in the row's error line; completion does not close a subsequently opened menu or reclaim focus. In-place
confirmations retain their panel so the user can finish the interaction. Rename does not: opening it closes the menu and
mounts the list-owned dialog SPEC.md specifies, whose lifetime belongs to the list rather than to the popup — the edit
survives the panel, the row, and listing failures.

Clone reuses the create form rather than a second submit path: the click builds a `CreatePrefill` snapshot of the row's
`Session` and hands it to the SAME `CreateSessionForm`, tagged with a monotonic generation the list view mints per
click. A `use_effect` inside the form compares that generation against the last one it applied and reseeds the form
whenever the two disagree; comparing generations rather than mere presence is what makes cloning the SAME row twice in a
row reseed a second time, since an unrelated rerender of that effect (a host reconnect, a catalog refresh) must not
overwrite an edit in progress. An agent-launch source seeds the agent tab from its stored declarative selection,
resolving new-request permissions through `LaunchHarness::request_permission` rather than parsing the composed command,
and also fills the command tab's command with the composed start command, YOLO unanswered, as a starting point for
turning the setup into a command launch; the launch still comes from whichever tab is selected at submit. A
command-launch source opens the command tab with its command, YOLO answer, declared agent type and resume command, and a
legacy source opens the command tab with only its stored command. The two tabs are one `LaunchTab` value over two sets
of signals, so switching shows and submits the other draft without copying anything between them. Destination, folder
browser, optional name, search, and submission remain shared; only the tab's own controls differ.

The prefill keeps the source's raw directory and title while separately carrying its repository, preferring checkout
membership over launch provenance. Clone seeds that repository as a fresh destination; Replace with keeps the folder.
The untouched Clone name is computed at every launch-related read, so display, preview, retry binding and submission
agree without destroying the copied title needed when returning to a folder. One naming source is active at a time: the
untouched Clone default or a template's base/fixed name. Applying a name template replaces the Clone series, and typing
clears the template source. Occupied-name previews advance a Clone default through `-clone`, then `-clone-2` through
`-clone-50`, or a template base through its raw base, then `-2` through `-50`; fixed names never advance. The pure
`CheckoutNameSequence` in `farhelm-proto::github_checkout` owns that shared series and cap; GUI source/target state
stays in the form. Snapshots retain the template's unnumbered base and kind while display, preview and submission use
its resolved name. Folder destinations use the base unchanged. At the cap the conflict stays visible. Changing host
installation or repository resets the search. The preview API classifies an unmarked 409 as occupied, separately from
the helm's stale-precondition 409, transport errors and other refusals; new conflict responses must revisit that
classification. The existing preview-authority generation checks reject late results before they can advance a name. A
retained attempt matching the current intent and installation freezes its name for reconciliation; the occupied
directory may be its own accepted allocation. Editing the launch while an ambiguous attempt is retained can advance the
default away from that attempt, even if the edit is later undone; reselecting the repository restarts the search and
restores the opportunity to reconcile its name. Re-preview never submits a launch by itself.

Search is the composer's one initial and post-selection focus target on both tabs. Its command-tab result set is built
without the retained structured harness or model, so it can expose globally owned models but cannot offer an effort that
would edit only a hidden draft. A harness, known model, or recent setup switches to the agent tab; a folder, host, or
name changes the shared field and leaves the active tab alone. There is no search result for the command tab itself:
SPEC.md's list of what search matches has no launch kind, and the tab strip is that choice. The focus handoff runs only
at dialog mount, the tab buttons, and accepted search results. Catalog, history, and operation rerenders cannot take
focus back from another field. A clone or replace-with acceptance transfers that initial handoff from the originating
row menu: removing the activated item can leave the row's inside-focus bookkeeping populated — the teardown reclaims the
item's element identity in the same pass, so its `onfocusout` never runs to clear it — and the dismissal must retire its
toggle return for the transfer instead of issuing it after the composer's own search focus and stealing it back.

The leading `name:` label offers one action carrying its whole value, including later colons. `host:` filters the same
registry rows as the GUI selector; `host:local` uses the row's local kind rather than its mutable display name.
Accepting either action does not launch and keeps search as the focus target. A host action also takes over a clone's
inferred host and retires the old history destination and idempotency key, just as changing the host selector does. The
GUI labels an unaliased local row `local (this machine)` in the selector, Hosts page, and confirmed-local session rows,
while preserving aliases and the registry's own `this machine` name. A session row without confirmed locality retains
the name supplied by the helm; the GUI does not infer locality from its text.

The `perms:` scope offers guarded, YOLO, custom, own-setting and approval-mode choices only where the active harness
supports them. Bare `yolo` and `guarded` join the ordinary combined search and get exact-word preselection; own setting,
custom and the approval modes require `perms:`. Bare `own` and `custom` would match too many unrelated queries. Applying
a permission row updates the same explicit-choice bookkeeping as the segmented control and leaves the other draft fields
intact. Command mode has no active structured harness, so it offers no permission actions.

The `effort:` scope offers `default` whenever the chosen harness offers effort, even while the model catalog is empty.
Bare `default` does not offer an effort action. Accepting it clears the explicit effort, as the default button does.

The clone's host is put through the SAME install-identity comparison SPEC.md's ordinary creation default uses (a
`HostId` is a registry row that outlives a retarget or an adopt) before the selector trusts it. A row whose install this
client cannot currently confirm is left at the ordinary host default with a note explaining why, rather than risking a
stale command landing on a successor install. That identity check is not a one-shot gate: `CloneHostState`
(`list::create_form`) tracks it across renders so a clone opened before the FIRST hosts read lands keeps retrying once
the registry answers, instead of giving up permanently because the form's separate text-field reseed only ever runs once
per clone generation. A clone whose host DID pass the check is re-checked on every later pass, withdrawing only the host
selection back to the ordinary default the instant a retarget or adopt changes the installation behind it while the form
stays open. A row that names no host at all (a session from a helm too old to report one) is permanently unconfirmable
rather than retried. An explicit host interaction takes the host decision away from automatic reconciliation for the
rest of that clone generation.

Launch seeding is separate from host selection. An agent-launch source seeds its stored selection, a command-launch
source its command fields, and a legacy source its stored command line, each once per clone generation. A delayed or
unconfirmable host does not suppress that seed, and later host binding or withdrawal does not change it, so a late host
read cannot overwrite an edit.

A clone's working directory, invocation and title are peer-relayed text (SPEC.md's clone rule copies them off another
session, and a remote supervisor under `--ssh` is the one this client does not control) going into editable controls, so
they get an escaped-display / raw-seed / edited-flag treatment (`create_form::submitted_field`) rather than being
written in raw: shown escaped while untouched, so a directional override or an invisible character cannot make the field
say something different from the bytes a submit would send, and an untouched submit still sends those ORIGINAL bytes
rather than the escaped spelling on screen.

The title is the exception once a managed checkout is the destination: an untouched copied title is submitted empty and
not displayed (SPEC.md, Managed checkouts). The composer applies that rule wherever it reads the title for a launch
(`create_form::submitted_title`) instead of clearing the field when a repository is picked, because the checkout
preview's title must equal the create's, and choosing an existing folder again must bring the copied title back.

The session list and open-session details bar mark current `working_copy` membership with the shared inline branch SVG,
including sessions sharing another session's checkout whose launch has no repository provenance. Full rows display
`working_copy.repo` after a spaced dot separating it from the host; compact rows retain only the mark beside the title.
The escaped tooltip exposes the recorded canonical path, the session directory when its spelling differs from the root,
and checkout lifetime. A visually hidden word pairs with the decorative SVG without repeating a visible repository.
Ordinary directories keep their existing clipping and display.

The macOS desktop WindowBuilder retains native decorations while making the titlebar transparent, hiding its visible
title text, and extending the webview into the full content area. Tao positions the native traffic lights in logical
coordinates, and the root-mounted Wry webview retains the same inset because its content view replaces Tao's. The
desktop macOS shell class reserves matching space in the sticky sidebar app bar and aligns the session top row's height
with it at 48px. In narrow windows, a fixed app row sits above both scrolling panes; the current sidebar width,
main-pane floor, and horizontal scrolling remain intact without moving controls under the native buttons. The native
window still has a title for system menus. The title band's drag surfaces are the sidebar's app bar, the session top
row, and, with no session open, an empty strip laid over the top of the main pane; each is a Dioxus element carrying the
`window-drag-surface` class and one shared press handler, and surfaces never nest, so each press reaches exactly one
handler. Because a surface also contains controls and text, and its handler receives their presses by bubbling, the
handler never decides from the event alone: a page script (`assets/click-detail.js`) classifies every press inside a
surface as empty background or not (controls, anything focusable or with a role, and text keep their own interactions;
the wordmark counts as background) and reports that verdict with the press, and only background presses drag or zoom.
Three cases need the press's position, not just its target. A press outside the surface's own box is never background,
because a surface can lay descendants out beyond it, as a session's confirmation panels hang below the top row. A press
on anything absolutely or fixed positioned between the target and the surface is not background either, because such a
popover can overlap the band: those panels' top edges do. And a press over an `inert` descendant (a stale session's tab
strip) is not background, because hit testing skips inert elements and would otherwise hand the press to the row behind
a visible tab. The classifier is one exported function, which the browser suite runs over the whole band. Double-click
zoom/restore is decided on the press's own mousedown, before any dragging for that press begins: the script reads the
mousedown's DOM `detail` — which WebKit sets from the incoming native click count (`NSEvent.clickCount`, copied
statelessly through `WebEventFactory::createWebMouseEvent`, `WebEventConversion`, and
`EventHandler::handleMousePressEvent`'s per-press assignment) — and synchronously POSTs it, with an eligibility bit
saying whether the preceding primary press landed on the same connected surface's background and the background verdict,
to a Rust asset-handler route ahead of the interpreter's own event send for the same press. That chain proves
propagation of an incoming native count, not AppKit's classification itself: that the press after a drag-consumed
release carries count 2 is an assumption, unobserved here, and native zoom/restore awaits the manual Mac checklist. A
document-level capture listener runs strictly before the interpreter's delegated bubble listener, and both sends are
synchronous XHRs on the page's one JS thread, so each POST is answered before its press's event send starts and the Rust
side pairs the two by order. Every press inside a surface POSTs, background or not, so the handler always consumes the
report for its own press. The handler then zooms exactly once for an eligible repeat press and never drags it, drags an
ordinary background press, and does nothing at all for a cross-target repeat (no zoom AND no drag), a press that is not
on background, or a press whose report is missing: without a verdict a press cannot be told apart from a button press.
There is no `dblclick` handler. The first press's mouse-up may never reach the view once native dragging starts, and
that is harmless here: the count is assigned fresh from each press's own platform event rather than accumulated across
the release, and the release-dependent click/dblclick synthesis is on a path this design never uses. Fullscreen
suppresses both actions explicitly (`toggle_maximized` has no fullscreen check of its own, and Tao defers a fullscreen
maximize change until exit). The route is registered from the window root, which never unmounts, and the page listener
survives component remounts. Complementary loss in both directions at once — report A arrives, event A never consumes
it, report B fails to replace it, event B consumes A — borrows the wrong press's report and stays a documented residual.

The drag itself is started with `performWindowDragWithEvent:` on the window's own NSWindow, not through Tao's
`drag_window`. Tao hands AppKit `NSApp.currentEvent`, assuming the press is still current when the Dioxus handler runs.
It often is not: on a Force Touch trackpad AppKit delivers pressure events while the button is down, and by the time a
press has crossed the webview and the bridge, a pressure or mouse-dragged event is frequently current. AppKit silently
ignores a drag started from anything but a mouse-down, so before this, many presses on the band did nothing (observed
2026-10: in one logged session three of eight presses found such an event current). When the current event is not a left
mouse-down, the handler builds one at the pointer's present location in the window, as Tao itself does for one other
event type, and drags from that. Only the event's type is read from an arbitrary current event; AppKit throws when a
pressure event is asked for a click count. Other builds keep the surfaces' handlers inert and receive no macOS shell
class, and the script never POSTs outside that class. Browser tests can apply the class to production markup to verify
geometry and the press classification, without enabling native actions. Fullscreen, resizing, and button behavior remain
AppKit-owned; actual native appearance and interactions require the manual Mac checklist. The window root installs
native layout before authentication completes. Bootstrap and error pages reserve a top band without requiring the
sidebar to mount. A build-mismatch notice stays below that band and above the scrolling shell, with the app bar pinned
above it so the warning remains readable.

Hover help is Farhelm's own tooltip, `assets/tooltip.js`, not the browser's `title` tooltip. The native tooltip's delay
belongs to the engine and cannot be shortened by any attribute, style or script, and WebKit, which the macOS desktop app
embeds, also ignores the macOS tooltip-delay default; at a second or more, nobody discovered the hover texts that
existed. So no element keeps a `title` as hover help, because one would bring the slow native box back on top. Elements
opt in with a `data-tooltip` attribute for text, or `data-tooltip-card` naming a hidden DOM source for a styled card,
set like any other attribute by components; scripts can also add targets (terminal.js does for its reconnect and
take-control buttons). A card clones the source's child nodes into the tooltip, rather than parsing markup from a
string, so peer text remains DOM text rendered by Rust. One delegated listener set on `document` and one body-level
`role="tooltip"` element serve the whole page, keeping the mechanism out of every component's props. The tooltip appears
300 ms after the pointer comes to rest on a target (each movement inside it restarts the wait) or keyboard focus
(`:focus-visible`) reaches one. Host confirmations suppress hover help during their automatic initial focus on their
enabled confirming answer (Cancel while disabled), preserving its visible focus ring; later pointer entry or moving
focus away restores ordinary help. Other programmatic focus keeps its existing behavior. The tooltip appears at once
when another target is entered while one is showing or within 300 ms of the pointer or focus leaving one (a press,
Escape, a scroll or window blur does not open that window, so the next tooltip after a dismissal waits the full delay).
It hides on leaving, any press (which keeps that element's tooltip down until the pointer leaves it or it loses focus),
Escape, losing focus, window blur, the target leaving the document (checked on a short timer rather than with a DOM
observer, which would run on every live status update), and a scroll of the document or of a container holding the
target, but not a scroll elsewhere such as the terminal under live output. Touch never shows it. It is placed above the
target with a 6 px gap and, only without room there, below with a 28 px gap: a page cannot know the cursor's size, and
standard cursors extend downward from their hot spot, so space above a control is never under the cursor while a box
just below a small icon would be. It is `position: fixed` from the target's measured rectangle so no scroll container
clips it, wears the row menu panels' background, border and shadow, wraps at about 280 px and clamps to eight lines.
Peer text still goes through `display_peer` before it becomes tooltip text, which the script sets with `textContent`;
the card copies nodes already rendered by Rust and never parses its serialized change signature as markup. The tooltip
element is a visual aid kept out of the accessibility tree (`aria-hidden`); icon-only controls and status marks keep
accessible text of their own (an `aria-label` or a `.visually-hidden` copy), and the hover texts that carry information
a screen-reader user needs (the header copy buttons' full values, a host's update urgency, a too-new host's remedy) are
also exposed as accessible descriptions. Plain timestamps and the client build string are hover-only. Two hover displays
keep their own shape and carry no `data-tooltip`, so two popups never stack: a host's update-progress popup and the
terminal's link-target display. Coverage is guarded by a browser sweep (`e2e/tests/tooltip-coverage.spec.ts`) that
visits the states the suite's fixtures reach and fails on any visible `button`, `[role=button]`, `[role=tab]`,
`[role=menuitem]`, `a[href]`, `select`, `summary`, checkbox or radio button without hover text (for the last two, on the
`label` that wraps them), exempting only described items in the session and host rows' menus. The local and remote host
marks carry their hover text inside the icon component itself, so the session rows and the host rows both get it without
either row's layout changing.

Known risks, accepted deliberately:

- API churn between Dioxus 0.x releases. Mitigation: pin, avoid internals, budget for migrations.
- Desktop is WKWebView on macOS while tests drive Chromium (no usable WebDriver exists for WKWebView on macOS).
  Mitigation: the tested surface is the web build; desktop-only glue is kept as small as possible and is the one
  manually-verified path.
- Clipboard and drag-drop are where WKWebView diverges from Chromium, and paste interception is a headline feature. The
  default is the same DOM paste/drop event path on both targets — WebKit does deliver file/image data on those events,
  but with documented engine-specific restrictions (pasted-HTML sanitization, gesture gating on the async clipboard
  API), so the honest framing is "same event model, engine differences expected", not "same as Chromium". The predicted
  real deficiency arrived (2026-09, dogfooding): clipboard WRITES never worked in the desktop app at all, because
  WKWebView does not treat the `dioxus://` page as a secure context and `navigator.clipboard` is simply absent there
  (wry registers custom schemes as secure on webkit2gtk, and has no way to on WKWebView — which is why Linux never
  showed it). The built solution is the native-side fallback this entry reserved: the webview POSTs copy text to the
  embedded helm's `POST /api/clipboard` (device-session authenticated, enabled only when the desktop registered a
  `ClipboardSink` — farhelm-helm's clipboard.rs) and the shell writes the real pasteboard via arboard. At most four such
  writes run at once; a write arriving while all four are still waiting on the native clipboard is dropped with the same
  success reply, so a hung OS clipboard cannot pile up threads in the helm's blocking pool. One correction to the
  parenthetical this entry used to carry: loopback HTTP is a secure context in Chromium but NOT in WebKit — Safari
  against `http://127.0.0.1` has no `navigator.clipboard` either, so browser-tab copies work in Chromium-family browsers
  and stay silently refused in Safari, within SPEC.md's best-effort clipboard contract. One concrete thing to check
  early rather than debug late: wry's own file-drop handling swallows DOM drop events unless configured not to. Also
  established the hard way during M2 dogfooding: wry implements NO native JS dialogs on macOS — `window.confirm()`
  silently does nothing — so any confirmation or prompt the UI needs must be in-page DOM, never a browser dialog.
  SPEC.md's confirmation language is deliberately mechanism-agnostic; this is the constraint that picks the mechanism.
- Blitz (Dioxus's native renderer) is not production-ready. The plan assumes webview desktop indefinitely; nothing may
  depend on Blitz landing.

**Input-method keys.** `composition-guard.js` installs before AppBody's other window capture listeners. It marks an
otherwise noncomposing keydown as composing when the same input just ended a composition, so existing Rust guards,
desktop event serialization and JS shortcuts see the same value. This works around
[WebKit's end-before-keydown order](https://bugs.webkit.org/show_bug.cgi?id=165004); Dioxus's desktop keyboard API does
not expose the legacy key code, and 229 alone would also suppress ordinary Pinyin Enter. The compatibility hint expires
after 16ms and is consumed by the next keydown or keyup; 229 does not extend it because it can also label an ordinary
Enter. An unreleased native composing key on the target makes the end ambiguous: it could be Chromium's ending key, or
earlier WebKit typing or a modifier still held. The listener declines a hint in both cases, deliberately leaving WebKit
rollover unprotected so Chromium's separate next key works even before the ending key is released. Only a keyup of the
same physical key (`event.code`) releases the recorded composing key; any keyup retires a pending end conservatively.
Browser replay of the outlined 229-or-window rule exposes the conflict at equal elapsed time; a threshold cannot select
the two different desired outcomes. This is the agreed spare-real-key fallback, not an inference that a held key caused
the end. The 16ms bound covers the immediate replay sequence with a scheduling margin, not measured native Mac timing;
delayed ending keys remain unprotected. Installation is idempotent across authenticated-tree remounts, and xterm's
helper textarea retains its own input-method handling. Browser tests replay both orders and pin the overlap omission;
they do not establish real Mac IME or desktop-shell behavior.

## Terminal widget: xterm.js island

The terminal is xterm.js, vendored as a static asset (no CDN — the UI must be fully self-contained, consistent with
SPEC.md's no-public-relay, no-third-party-services posture and the loopback deployment), mounted as a JS island inside
the Dioxus tree. PTY bytes flow WebSocket → `term.write()` directly, bypassing Dioxus state entirely. Dioxus owns
everything around the terminal (tabs, status, dialogs), not the terminal's content path.

Vendored xterm.js 6.0.0 has an observed, reproduced defect (`terminal-scroll-freeze.spec.ts` pins it): a scrolled-back
viewport can paint stale rows during sustained output, both once scrollback is already full and when output is confined
to a DECSTBM scroll region. What is established from the bundle: `BufferService.scroll` decrements `ydisp` by one per
evicted line while scrolled back with the buffer full, and the parser's per-write repaint maps dirty screen rows to
viewport rows via `(ybase - ydisp)`, skipping the refresh once that offset reaches the row count. On their own both are
correct behavior (the viewport's lines do not change under either), so they rule out the per-write path as the repainter
rather than explain the stale DOM; the exact xterm-internal step that leaves stale content is not established from
source. `terminal.js` compensates with a throttled, unconditional `term.refresh()` while the viewport sits away from the
tail, rather than patching the vendored bundle. The bundle is deliberately never patched: keeping the vendored file
byte-identical to upstream is what makes its provenance checkable and a future version bump a plain swap. Decided
2026-10-10: non-security bugs inside the bundle are fixed by upgrading it, not worked around in Farhelm, unless a
workaround is already recorded here (the refresh above is one).

JetBrains Mono Nerd Font is vendored alongside xterm.js for the same self-contained reason, and terminal.js sets it as
xterm's `fontFamily` — but it is no longer terminal-only: `app.css`'s `--font-ui` token (see the design-tokens paragraph
above) applies the identical vendored face to the rest of the chrome, so the whole app reads as one typeface. Chrome and
terminal share the same two cached `.woff2` files rather than each vendoring its own copy; whichever surface asks first
pays the fetch, and the other reads it back from the browser's cache.

Sidebar width lives in `sidebar-width.js`, shared by web and desktop. It reads localStorage once at script load
(`farhelm.sidebar-width`): plain digits only, clamped to 240–600px, with 340px for absent, malformed or unreadable
storage. The root's `--sidebar-width` survives authenticated-tree remounts. A shell-level separator owns pointer
capture, double-click reset and 10px arrow steps; completed gestures store the width. Width-derived root classes replace
the fixed narrow-window and row-menu-pointer media cutoffs. The existing terminal ResizeObserver sends each resulting
grid change without debounce. A focused separator prevents terminal reveal from taking keyboard focus. Desktop
persistence has the same webview-localStorage assumptions as terminal text size below.

Terminal reveal also yields keyboard focus to an open menu, including its expanded toggle while the menu is mounting.
Those controls remain the destination for menu keys even if the selected terminal finishes its initial replay at the
same time; tab-strip buttons still allow reveal to move focus into the terminal after selection.

Device sounds live in `sounds.js`, which generates the three approved note sequences with Web Audio at master gain 0.3.
Its `farhelm.sound.waiting`, `farhelm.sound.approval` and `farhelm.sound.finished` localStorage keys accept only `true`
or `false`; missing, malformed or inaccessible values use their event defaults. One context serves the page, resumed on
trusted pointer-release, click or key gestures, with no replay queue. Touch release is required for mobile browser
activation. Desktop persistence has the same webview-storage assumption as terminal text size below.

The authenticated `SessionSounds` component lives under the persistent sidebar and observes its accepted listing and
view scope signals, without another session-list request. The scope advances when an accepted reply changes the filter,
including a return to a previously shown filter while an approval read is pending. That first reply records the view's
silent status baseline, so a later status change is still detected when approval replies lag. Approval history starts
with its own first successful reply. It keeps prior statuses for that view and all seen approval ids. Its
serialized/retrying surface reader reads approval data on feed notices, accepted listing changes and the
disconnected-feed fallback. A different accepted filter silently reseeds status history; sort and selection changes and
reconnects preserve it. A new authenticated mount starts silently. Status coverage has the sidebar's listing cap.
Successful approval reads pair with current sidebar evidence for pure JS transition and priority rules; device switches
and `data-window-active` quiet the open session before priority is chosen. Priority is per observed snapshot, not a time
window across independent replies. Failed reads leave history intact; a silent audio refusal still consumes the event.
Approval-card markup is not an input to detection, and approval requests remain audible when their requesting session is
outside the sidebar's view.

Terminal text size lives entirely in `terminal.js`. It reads the remembered size once, at script load, from the page's
localStorage (`farhelm.terminal-font-size`; 9 to 28 in steps of 1, default 14; a missing or unreadable value, or
anything but plain digits, means the default, and an out-of-range number is clamped) and constructs every terminal at
it. `farhelmTerm.stepFontSize` clamps, stores, then sets `term.options.fontSize` on every mounted terminal and calls its
fit: setting the option only makes xterm re-measure the font, and the fit is what recomputes rows and columns and sends
the resize to the pty. Hidden tabs are mounted and sized like visible ones, so they take the change at once. A screen
held aside during a reconnect or kept after a takeover keeps its old size: it is frozen, with no program to tell. The
shortcut is one capture-phase `keydown` listener on `window`, matched on `event.code` (`Equal`, `Minus`, so the
US-layout key positions) with Shift and Meta on macOS or Ctrl elsewhere, by a platform-string test that agrees with
xterm.js's own Mac check; capture on `window` runs before xterm's handler, so its custom key handler is untouched, and
preventDefault keeps the browser out. With no terminal mounted the listener lets the keys through. The tab strip's A− /
A+ buttons are stateless Rust buttons that call the same function, which hands focus back to the terminal the view last
focused when the click left it on a button; nothing about the size flows back to Rust, so they have no disabled state at
the ends of the range. In the desktop app the size persists as far as the webview keeps its local storage across
restarts; that was verified on Linux, not on macOS, where a webview that does not keep it would fall back to the default
rather than earn a native state file.

The quick switcher installs an idempotent capture-phase window keydown listener through inline `document::eval` in both
renderers. It matches `KeyK` with Meta alone on macOS or Ctrl+Shift elsewhere, using terminal.js's platform test;
composition and repeat do not open it. Cmd+J is matched by the shared template-shortcut listener from the Rust
definition, and uses the same focus-capturing opener with a templates-only relay. An existing modal or an absent
list-owned trigger yields the chord without preventing its default. Otherwise it prevents default and propagation before
xterm handles input, records the active element and clicks the appropriate hidden trigger. In sessions mode, each
mounted dialog fetches one unfiltered activity-ordered session snapshot and partitions it into title and metadata-only
subsequence matches, retaining helm order inside each tier. It never adds a result cap or subscribes to listing updates.
Peer text is escaped and isolated before highlighting. Closing releases modal isolation and waits for actual DOM removal
before letting ListView's ordinary navigation callback open a session. Cancellation, a refused pick and picking the
current session restore previous focus; an accepted different session leaves focus free for its terminal. The trigger
stays mounted but disabled through that handoff, swallowing repeat chords without replacing the saved opener;
terminal.js's modal focus veto must no longer see the switcher when the new selection arrives.

The switcher also reads one template snapshot per open and delegates `tl:` parsing and matching to the launcher's own
search helpers. It reuses the Templates panel's summary formatter. New/name and template choices close through the same
DOM-removal handoff, then use the ordinary New-open callback with an optional `ComposerSearchResult`, not a clone
prefill. The launcher keeps its normal remembered permissions and destination seeds. A one-shot pending action uses
names after mount seeding; templates also wait for the templates and model-catalog resources to settle and the hosts
read to succeed. While waiting, the launcher shows a cancellable loading state so late acceptance cannot overwrite user
edits. Clone/Replace revokes any pending choice, and closing clears the parent opening intent. Both kinds use the same
search-accept callback as Enter and a clicked search result. Completed read failures follow that callback's ordinary
refusal behavior; resource refreshes do not reapply the initial choice. Name and template picks do not submit the form.
The templates-only mode passes its plain query through that same template matcher and refreshes shortcut labels when
opened. Its separate preference read overlays unconfirmed local bindings and updates only the binding fields; a failed
read leaves cached labels intact and does not hide templates. Later binding moves need not refresh an already-open list.
An exact case-sensitive name is selected over other substring matches unless the user arrows away; the same rule applies
if the template snapshot arrives after typing. A row pick carries only its name across the modal removal handoff. The
shared shortcut worker reads preferences and templates afresh and resolves that name before using the existing
quick-launch admission, fallback and naming behavior. A name missing from the fresh catalog opens New with a preparation
error rather than launching a stale captured definition.

Session-action shortcuts use an idempotent capture listener and a pure key matcher from `session-action-shortcuts.js`.
The build's Mac-desktop condition selects Cmd+W for Delete; browsers use Ctrl+Shift+Backspace. A visible modal yields,
and browser Delete leaves ordinary editors alone while consuming xterm's input proxy. Matching keys stop before xterm;
repeat and disabled controls never click. Delete clicks the current header button, sharing admission, confirmation and
the skip preference. Cmd+W is consumed even without a header. The Mac desktop installs an explicit native menu without
muda's predefined Close Window, whose fixed Cmd+W accelerator would win before the page. Its renderer-free description
is tested on Linux; native construction is type-checked by Linux desktop builds, while only macOS installs it.

The same matcher maps Cmd+L on Mac desktop or Ctrl+Shift+L in browsers to a list-owned hidden Rename trigger. The open
view publishes its accepted raw title through a separate signal, keyed by the selected session ID and independent of
sidebar filtering. Navigation and successful local renames seed it synchronously; detail refreshes update it without
writing the selection, whose observer would fence and refetch those same reads. The trigger uses the ordinary rename
opener and guards. An editor retains whether it came from that shortcut: row edits return to their menu, keyboard edits
to the selected terminal, through the same post-unmount generation and focus-ownership checks. The listener records the
exact active element before its hidden click so repeated dialog mounts can take focus from that opener; a delayed mount
still yields to an unrelated control chosen meanwhile.

Template shortcuts use one capture listener with a pure classifier fed by a Rust definition of physical keys and
modifier masks. Rust also derives the visible chords from those masks. The build's Mac-desktop condition selects Meta
alone for number keys and the sequence prefix; every other build selects Ctrl+Shift alone for those, regardless of
`navigator.platform`. The bolt's chord follows the operating system instead. The listener's one context rule
distinguishes the page, a hidden launcher preparing quick launch, the visible launcher and unrelated visible modals; the
launcher alone accepts template actions while unrelated visible modals yield. Ten hidden `.template-number-trigger`
buttons hand the digit to the same list callback that handles quick launch; the existing `.quick-launch-trigger` remains
unique. Cmd+J uses the definition's platform chord in this listener and admits only the page context; a hidden
template-switcher relay shares Cmd-K's opener and close handoff. Both direct-launch relays read fresh shortcut
preferences and template definitions, resolve a name and use the same New-open, template application and submission
path. The row's key popup uses `menu_panel` keyboard/focus/placement helpers inside modal isolation; Escape closes it
without closing Templates. The open toggle owns pointerdown until click. Dialog scroll/resize, backdrop resize and the
app's layout epoch dismiss the popup when its fixed coordinates become stale; the popup's own scrolling does not dismiss
it. Focus returns only while still owned by a closing menu, with `preventScroll`, so a delayed dismissal cannot steal
another popup's focus or undo the dialog's scroll. Re-delivery to an already closed popup does nothing. The two shortcut
preference fields share acknowledgment, deletion retirement and fresh-read overlay helpers. A rename repoints both
before deleting the old name, and removes a stale new-name key when retaining the old name's key would otherwise violate
one-key-per-template. Failed repoints keep both names and dirty intent for recovery.

Quick launch uses that shared capture listener and the same list-owned New-open callback, followed by the ordinary
template search action and form requestSubmit. The mark is one nullable shared preference rather than template data, so
a stale editor save cannot move it. Both template delete paths clear only a matching mark in their existing immediate
transaction; recreating a template does not restore the mark. The editor repoints between save-new-name and
delete-old-name through the existing preference writer, awaiting its own acknowledgment. Rename reads the mark fresh,
respecting an unconfirmed explicit local choice; its separate writes retain ordinary cross-client last-writer-wins
semantics. An unconfirmed repoint keeps both template names and its dirty local choice for recovery. Delete waits for
any owned mark request and retires matching dirty intent before releasing that wait, so credential recovery cannot
restore a deleted mark. This sends no unconditional clear that could erase another client's different mark. Each
shortcut fetches the saved mark and templates rather than trusting its startup preference snapshot, overlaying only an
explicit local mark still awaiting persistence. A pending or failed mark write cannot silently launch the previous mark.

The form's quick-launch flag changes visibility and schedules one ordinary submission after mount seeding,
template/catalog reads, host binding and the current managed-checkout preview settle. Its transparent modal backdrop and
existing modal isolation prevent page clicks or keyboard input from replacing the pending intent, while hidden fields
take no focus. Parent opening generations revoke late reads and scheduled submissions when another opening takes over;
the modal install epoch also notices an intervening modal that already closed. Session Rename uses its own focus trap
and explicitly revokes a held quick read when opened. Clone/Replace reveals a superseding draft and spends the automatic
intent. Incomplete or offline prerequisites, request errors and YOLO refusal reveal that same form. The auto-submit
intent is consumed before requestSubmit and stays consumed through retry reconciliation or a new path preview. There is
no headless composition, second create path or timer-based readiness. The YOLO question mounts only after reveal so its
usual Cancel focus still applies. A completed initial host-read failure reveals the cancellable draft, focuses its
loading Cancel button, and cannot launch later merely because the host read recovers. Preparation errors survive first
binding; every unsuccessful create reveals the form even when its old-target refusal is fenced out after a target
change. The quick-switcher row closes its modal through the existing DOM-removal handoff before using this shortcut
path.

Successful creation carries one local fact to the list: whether this quick-launch opening never revealed New, read from
its original quick-launch flag and current hidden state at success. The list selects the accepted session and opens its
existing rename editor in naming mode in that same completion event, omitting New-button focus. The create operation is
already released; no rename or user answer gates agent startup. An unset naming preference enables it. The editor keeps
one naming flag alongside its existing shortcut origin. Naming's generation-checked mount explicitly focuses and selects
the field once, including later openings in the same document. An unchanged raw title closes without calling rename;
edited titles keep ordinary validation and refusal behavior. Naming and shortcut editors use the existing
generation-fenced, post-unmount terminal focus return. The terminal's existing modal guard withholds focus while naming
is mounted; an unrevealed terminal retains its ordinary reveal-time handoff after early dismissal.

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

Outbound key delivery has one non-obvious mechanic: for input small enough to fit one protocol frame and one `send-keys`
command, message boundaries are pty write boundaries. The transport frames input at 32KiB and the supervisor chunks each
frame into `send-keys` commands of at most 256 bytes, so larger input is deliberately split — nothing may depend on
atomic delivery of an arbitrary message. But a message at or under one command IS one command, and tmux flushes each
command to the pane as its own write — measured on the pinned 3.7b: two commands landed as two reads ~7ms apart, never
coalesced. For almost all input none of this matters, but SPEC.md's Shift+Enter chord (ESC CR, two bytes, always one
frame and one command) is exactly the sequence where the boundary is meaning: split across two writes it reads as
Escape-then-Enter to boundary-sensitive line editors, which is how the chord shipped broken for Codex while bash's 500ms
readline timeout hid the split. The implementation therefore merges the pair into ONE message at the source
(`shift-enter-key.js`, whose header owns the full design rationale): the chord's keydown arms the merge and returns
`true` so xterm's own Enter path — scroll-on-input, selection dismissal, hidden-textarea cleanup, accessibility — runs
untouched, and the `term.onData` wrapper prepends the pending ESC onto the `\r` xterm emits synchronously for that same
keystroke. The arm's lifetime is that one synchronous dispatch (expired in a microtask queued at arm time), so a fizzled
chord leaves no stale arm behind and the ESC is never flushed alone: a stray lone ESC is the byte shape the merge exists
to never emit.

Clipboard writes — SPEC.md's terminal contract is that select-to-copy and a program's OSC 52 WRITE land on the system
clipboard while an OSC 52 READ is never answered — are implemented per surface, because the web platform only delivers
that contract on some engines. Both of terminal.js's write paths (the copy-on-select mouseup callback and the OSC 52
`ClipboardAddon` provider) prefer `window.__farhelmNativeClipboardWrite` whenever it exists and fall back to
`navigator.clipboard.writeText` otherwise, with every failure swallowed per the contract's best-effort clause. The
global exists ONLY in the desktop app, installed by desktop authentication's success path (auth.rs's
`arm_native_clipboard`, re-armed whenever that authentication runs again from the failure page's Retry): it POSTs the
text to the embedded helm's `POST /api/clipboard`, and the desktop shell — the only construction able to register a
`ClipboardSink` (`run_embedded`'s parameter; `farhelm helm run` hardcodes none, deliberately without a flag) — writes
the real pasteboard via arboard. The reason the desktop cannot use the web API at all: WKWebView does not treat the
`dioxus://` page as a secure context, so `navigator.clipboard` is absent there — not denied, absent — which shipped as
copy-never-works until 2026-09 (the Dioxus-risks bullet above records the diagnosis; farhelm-helm's clipboard.rs owns
the endpoint's contract: device-session auth with the desktop-webview CORS layering, one bounded text field, 404 on any
helm without a sink so a remote browser can never write a server machine's clipboard, and a silent 204 whether the
native write succeeded or not). A browser tab keeps the web API path and inherits its engine's policy: Chromium-family
engines treat loopback HTTP as a secure context and work; Safari does not, and stays silently refused. OSC 52 reads are
refused identically on every surface — the native route is write-only by construction, not by policy that could drift.

The "this drag did not copy" notice is decided in `assets/copy-on-select.js` (`dragMayHaveCopiedNothing`,
`dragCopyNoticeText`) and driven from terminal.js's existing copy-on-select mousedown/mouseup pair: the press records
the pointer position, whether the pane's program had mouse tracking on (`term.modes.mouseTrackingMode`), and the pane's
OSC 52 count, kept by a fall-through `registerOscHandler(52, …)` registered after the clipboard addon so the addon still
performs the write; the release checks for a plain drag under press-time mouse tracking without the forcing modifier,
and 1.5 seconds later whether any OSC 52 arrived since the press. A retained local selection does not suppress this
notice: that program-owned gesture does not copy it. The forcing modifier it names copies the vendored xterm's own
`isMac` platform list. Agent-specific wording comes from the session's agent kind (`SessionAgentKind::drag_copy_hint` in
the UI crate, an exhaustive per-kind function per the harness map), passed on the agent terminal's spec only; tabs
always get the generic text. The persistent Rust-rendered live region holds an initially empty text span and a dismiss
button. `createDragCopyNotice` owns its placement and thirty-second timer per mount; every qualifying drag replaces its
position and deadline. Horizontal placement reuses `placeTooltip`; vertical placement clamps six pixels above the
release pointer rather than flipping below it. CSS caps the box to the viewport and hides its entire subtree when
dismissed. Only × takes pointer events; its mousedown prevents the native focus change, and its click hides locally
outside xterm's element. A terminal press cancels both the visible notice and any pending OSC 52 grace check. Resize
repositions it; mount teardown removes listeners and cancels timers.

Terminal web-link click ownership is the nullable `terminal_link_click` shared preference added by helm schema 46:
`intercept` is the default, `both` preserves Farhelm activation plus program mouse reporting, and `program` disables
Farhelm's web activation. The settings radio buttons use the ordinary sparse preference queue. `PreferencesGate`
publishes the current choice into the page-wide terminal asset state; every terminal reads it at press/activation time,
without a remount. Other clients seed again on reload, matching host confirmation choices. File downloads and agent URL
forwarding do not consult this choice.

OSC 8 and plain-web hover callbacks identify only targets their existing activation checks would open. In interception
mode a primary non-forced press on such a target is stopped by a screen-element listener installed after xterm opens.
The vendored Linkifier's screen listener must see it first to record the owning link; capture-phase suppression would
leave a stale pressed-link record and can duplicate a later activation. Suppression occurs before xterm's enclosing
terminal-element mouse reporter, so that press installs neither the program's release reporter nor its drag reporter.
Another held button can already have installed them, or can install them during the withheld gesture. Independent
primary-button ownership therefore lasts through release. Document capture listeners record each mouse event and its
ownership decision; the public `onData` and `onBinary` callbacks discard only complete mouse-report chunks emitted
synchronously while that event's `eventPhase` is nonzero. DOM supplies the end-of-dispatch boundary: a microtask can run
between native capture and bubble callbacks and would clear suppression too early. The reporters still run their
all-buttons-up cleanup, and other buttons' reports pass through. The always-on move reporter ignores motion while a
button is down; the drag reporter prioritizes the primary bit when buttons overlap. Normal release link callbacks still
open the target. The withheld press explicitly runs drag-notice dismissal bookkeeping, marks its gesture as withheld,
clears an earlier local selection, prevents the native focus change and focuses the terminal. Forced-selection presses
keep xterm's own path; file links, other buttons, wheel and unowned moves remain unchanged. Teardown removes the screen
and document listeners with the mount's copy-on-select listeners, so reconnects cannot accumulate owners. A restored
recovery snapshot reinstalls its retained terminal's link-gesture observer beside local copy-on-select. Burial removes
both; input and transport listeners remain detached.

An OSC 8 link's hover compares the link's underlined text with its target (`linkTextMismatch` in
`assets/terminal-links.js`) to decide whether the display is the quiet one or the loud mismatch warning. The text is
joined by `linkHoverText`, which reads the undocumented `extended.urlId` on the cell returned by the public
`term.buffer.active.getLine(y).getCell(x)` API. Only consecutive rows carrying the same tag join, with exactly one
contiguous tagged run per row, at most 128 rows and 8192 characters. Each run is read without trimming; only the ends of
the joined string are trimmed. One synthetic empty padding cell before a terminal-wrapped wide character is omitted; an
actual printed space is preserved. Agent cursor moves do not set `isWrapped`, so that flag is not a join condition.
Within a complete bounded, unambiguous extent, rows whose addresses each independently match the target compare as
separate occurrences. Otherwise the whole text is compared normally. An adjacent continuation that exceeds a bound or
has ambiguous runs yields Fragment: its hovered piece warns whenever it looks like an address, even if that piece
happens to equal the target. A single row with repeated same-tag links keeps its existing comparison. An unavailable tag
yields Unknown and restores the previous row-only comparison unchanged, including its false alarms and gaps.

Decided 2026-10-15: accept this read-only private field access, superseding the 2026-09-30 decision to accept
wrapped-link false alarms. The bundle stays byte-identical to upstream. The field can change at a manual xterm upgrade;
a browser test against the vendored bundle pins its presence and shared tag for both agent layouts. Losing it falls back
to today's row comparison, never a quieter partial-match rule. That old rule is incomplete: `https://git` followed by
`hub.com/login` targeting `https://hub.com/login` was quiet on the second row, which alone equals the target.

The tagged cells, read row by row, must spell the target for a quiet URL-like comparison; this does not certify all text
the reader sees. Untagged decoy text is outside the comparison. Nor is the join complete against a hostile program, and
the reason must not be overstated: pieces that are not joined are still judged one by one, so `https:/` and
`/github.com/login` emitted as two hyperlinks with different tags (or on non-adjacent rows, or as a join refused for an
ambiguous run) are each not URL-like and both stay quiet. `https://good.example` followed by `.evil.test/` targeting
`https://good.example.evil.test/` is quiet, just as the same tagged text on one row is. Position proves nothing because
agents can break before the edge. The quiet display still names the real host, which is the safeguard the warning adds
to. The URL-like gate is deliberately generous, and strict parsing then decides: it accepts any scheme spelling, looks
past leading non-ASCII characters that are not letters or digits and past the Hangul fillers (letters that draw as
blanks), and reads lookalike colons, slashes and dots as their ASCII originals. So a lookalike scheme (`httрs` with a
Cyrillic letter), invisible leading characters, or a `ː` posing as `:` warn rather than pass. Only the start of the text
is examined: link text that is prose containing an address is not judged. A dotted host counts only when its last label
is two or more letters, which keeps version and directory text such as `v1.2/` or `changelog.d/` quiet, at the cost of
not judging scheme-less IP text. The warning shows the link's text with Unicode format characters (bidi controls,
zero-width characters) as visible escapes, because the terminal stores them unapplied and HTML would apply them.

## Terminal substrate: private tmux server

Each supervisor runs a dedicated tmux server on a private socket (`~/.local/state/farhelm/tmux.sock`) with a locked-down
generated config: status bar off, `history-limit` sized to SPEC.md's replay floor, `remain-on-exit on`. Every start,
fresh or adopting a running server, also sets a global `pane-died` hook that signals a `wait-for` channel; the
supervisor's ticker keeps one `tmux wait-for` client blocked on it and runs its dead-tab reap as soon as it fires, so a
tab whose shell exits is closed right away instead of on the next tick, which remains the fallback. One tmux session per
Farhelm session; window 0 is the agent terminal in practice, additional windows are the terminal tabs. Neither is
identified by position: the supervisor stamps each window it creates with a tmux user option — the agent's window with
the session id, a tab's window with a minted tab id that is also that tab's whole record. The agent terminal is
identified by its durable pane record first, with the marker as the recovery aid for a session whose record is empty;
tabs have no durable record at all and are rediscovered from their markers alone, because anything that reaches the
private server can conjure windows a positional scan would adopt: a program pointed at its socket, a process the agent
login shell's startup files started before the launch removed `TMUX`, or a session or tab launched by a version that did
not remove it yet. Agents and tabs no longer inherit `TMUX`/`TMUX_PANE` (removed by the launch shim for agents, after
the login shell's startup files, and by the tab helper before starting its shell; see `PRIVATE_TMUX_ENV_VARS` in
launch.rs). That was checked statically against Claude Code 2.1.286 and Codex 0.159.3, not by running them: both wrap
clipboard and similar escapes in tmux passthrough when `TMUX` is set, and the supervisor unwraps passthrough itself, so
either form reaches xterm.js; Claude Code also keeps truecolor without `TMUX` instead of dropping to 256 colors (the
private tmux advertises RGB) and stops showing its tmux scroll hint. The user's own tmux usage and config are untouched.

The private server is an implementation detail. Direct interaction with it, by the user or by a program running in a
session, is unsupported (SPEC.md, "Ownership during cleanup and provisioning"): Farhelm handles its own objects going
missing, but does not track, reap, or restore windows, panes, or configuration created or changed that way. This does
not relax the helm/GUI's obligation to tolerate remote failures.

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

Farhelm requires tmux at or above a version FLOOR that is, by policy, the exact release the output-client teardown
regression suite (`scripts/test-tmux-pinned-shutdown.sh`) runs against — 3.7c as of this writing, pinned in
`.github/release/source-pins.env`, with the supervisor's floor constant tested to equal that pin so the two cannot
drift. This replaced the original "any tmux ≥ 3.3" policy on 2026-08-22 (the decision record lived in TODO.md until the
floor shipped). The original policy treated versions above 3.3 as interchangeable, and experience said otherwise: the
supervisor's driver is full of behavior audited per version (3.3a, 3.4, and 3.7b each differ in ways that shaped real
code), a production distro 3.4 server died in BUGS.md's `fatal()` abort shape on 2026-08-19, and BUGS.md records the
same abort class reproduced on distro 3.6. The floor is therefore DESIGNED to exclude many current distro packages
(Ubuntu 24.04 ships 3.4, 26.04 about 3.6, Debian 13 3.5a; some Fedora releases already ship 3.7c): on Linux this is
close to always-bundled in practice. Always bundling was considered and rejected all the same: it loses distro security
patching of tmux and libevent (where the 2026-08-16 3.7b segfault lived), concentrates a bad build's blast radius on
every host at once, needs a static build per platform (the darwin one has never completed), and a from-source install
has no bundle anyway — so the version check is the policy and the bundle is one way of meeting it. Versions above the
floor are accepted in tmux's own release spelling (`major.minor` plus at most one patch letter); a version newer than
the pinned one earns a one-time warning that it is unaudited, never a refusal. Anything the parser cannot read exactly —
`next-3.8`, release candidates, distro-decorated strings — is refused rather than guessed about: a version Farhelm
cannot name is one nobody audited it against.

How the binary is chosen: the supervisor selects its tmux program once at startup — `--tmux <path>` on
`farhelm supervisor run`, else `FARHELM_TMUX` from its environment, else the bare name `tmux` — and every invocation
goes through that one value. A bare name is resolved against `PATH` by the operating system at each spawn, as it always
was; only the spelling is fixed, and the refusal message reports the `PATH` entry it would resolve to. Whatever was
chosen is version-checked and refused by name (binary path, version found, floor) when too old. The check is applied
TWICE, because a tmux server outlives the supervisor by design: once to the client executable before any server is
started, and once more to the server the supervisor adopts on a socket that already has one — the server is the
component that holds sessions and the component that has crashed, so a 3.7c client driving a pre-floor server left over
from before an upgrade is exactly the case the floor exists for. A below-floor adopted server is refused without being
killed: the message names the socket, both versions, and the floor, and the operator drains or kills that server
deliberately. The override is "you own the substrate": a way to run something newer or differently built, not a
supported configuration. Linux releases bundle a private musl tmux build per architecture, which provisioning installs
only when the host has nothing acceptable (Linuxbrew's tmux is the documented alternative); either way the unit
provisioning writes names the accepted binary through `FARHELM_TMUX`, so a private build left behind by an earlier
installation cannot shadow a host tmux that was accepted later. The Mac app bundles none: the desktop app takes an
ambient `FARHELM_TMUX` as given, otherwise probes `/opt/homebrew/bin`, `/usr/local/bin` (Homebrew on the two
architectures) and `/opt/local/bin` (MacPorts) in that order — because GUI apps do not inherit the shell `PATH` — and
hands a hit to its managed supervisor through `FARHELM_TMUX`; with no hit it sets nothing and the supervisor's ordinary
`PATH` lookup applies. Homebrew's tmux is the recommended way to meet the floor on a Mac, not the only one the code
accepts.

The desktop app additionally owns a PREFLIGHT of its own: before it spawns its managed supervisor — never before, and
never for a supervisor it merely discovers already answering — it probes the SAME candidate it is about to hand that
child through `FARHELM_TMUX` and applies the version floor itself (`farhelm_ui::desktop::run_tmux_preflight_or_exit`). A
missing or below-floor result prints one plain stderr message naming what was tried and how to fix it, then exits 1,
before the state directory holds anything beyond the bare private directory itself (created up front so discovery's own
probe has a valid path to operate against), before the embedded helm starts, and before the managed supervisor is ever
spawned. Any OTHER probe failure this preflight has no tailored wording for (a permission-denied spawn, a nonzero `-V`,
unparseable `-V` output) falls through to the desktop's ordinary bootstrap-error path instead. Discovery runs FIRST
specifically because an answering supervisor is an ownership boundary: it may be driving a perfectly good tmux selected
by its own `--tmux`, its own `FARHELM_TMUX`, or a login-shell `PATH` this Finder-launched process never sees, and
refusing startup over a dependency this process does not need would reject a supported setup. The supervisor still
performs its OWN two floor checks after being handed a tmux this way (the client-executable probe and the adopted-server
check described above) — the desktop's preflight is a user-experience improvement specific to the one path where this
app starts the substrate itself, not a replacement for the supervisor's own checks, which remain the authority for the
case this preflight cannot see: a private server already running on the socket from before an upgrade.

Every pre-window desktop refusal keeps that exact stderr message and exit status, and on macOS also sends it to Console
and a critical native alert so a Finder or Spotlight launch has a visible diagnostic.

Historical note on what the floor made moot: below 3.7 the supervisor warned once at first attach and lost bracketed
paste restoration (`bracket_paste_flag` arrived in 3.7), and on 3.3a `capture-pane -N` dropped trailing styled padding
from a dead pane's captured frame (found 2026-07-29 during M2.5's 3.3a validation, in a since-removed stop-time
capture). The first is a fallback the driver still carries (a missing `bracket_paste_flag` format) that the floor makes
unreachable; the second was old tmux's own behavior under `capture-pane -N`, not a Farhelm code path. Both are recorded
here so nobody rediscovers them as bugs.

tmux is a headless PTY holder and history store. The supervisor's only client is a non-rendering control-mode client
(`tmux -C`, the interface iTerm2's tmux integration is built on; `pipe-pane` is the fallback shape). Sizing (audited on
tmux 3.7): a control-mode client is an attached client, but tmux ignores it for window sizing until it declares a size
via `refresh-client -C` — the supervisor never declares one, so tmux ignores it for sizing entirely, and geometry comes
from explicit `resize-window` calls tracking the attached GUI client's dimensions (`resize-window` sets
`window-size manual` on the window it touches, which is where that setting comes from). NOTE: setting
`window-size manual` globally in the config crashes the tmux 3.4 server outright — the version Ubuntu 24.04 ships — so
it must stay out of the generated config; the two mechanisms above make it redundant anyway. The supervisor streams raw
pane output to the client; input goes in as `send-keys` commands written to a dedicated no-output control client's
stdin. InputClient owns that client's request/reply stream, separately from the attachment's replay/live OutputStream
and the session's always-drained SessionSink. Input success means tmux confirmed execution, not merely that a pipe
accepted the command: ignoring an input error or killing a shared output client during takeover must not silently lose
input already reported as delivered. An earlier design tried `load-buffer -` over stdin followed by `paste-buffer -d -r`
instead, specifically to keep input bytes off a process's argv (see below) — and had to be abandoned: verified
empirically against tmux 3.7b, `paste-buffer` caret-escapes control bytes on the way into the pane (DEL arrives as the
two literal characters `^?`, ESC as `^[`, ctrl-C as `^C`), silently breaking backspace, arrow keys, and ctrl-C.
Keystrokes are not pastes, and no `paste-buffer` flag changes that. `send-keys -H` delivers bytes verbatim instead (also
verified against 3.7b) and keeps the security property that motivated stdin delivery in the first place: hex-encoded
input never touches a process's argv, because it rides an _already-running_ process's stdin rather than a freshly
spawned `tmux send-keys` command's arguments — the earlier concern was a spawned process's argv being world-readable via
`/proc/<pid>/cmdline`, which matters because input includes credentials typed at agent prompts, and that risk never
applied to bytes written to a pipe. Each `send-keys` command is chunked at 256 bytes and commands are pipelined in
bounded batches before their ordered replies are read. Entirely printable ASCII chunks use one quoted `send-keys -l`
argument to avoid per-byte argument parsing during large pastes. Quotes, backslashes, dollar signs and tildes are
escaped for tmux's parser; an explicit `--` prevents leading hyphens from becoming options or overriding the target.
Control and non-ASCII chunks use `-H`, with one hex argument per byte. Both forms use the same input client and consume
command acknowledgements. Command size and batching must stay bounded, including enough room to drain replies without a
pipe-capacity deadlock. The current constants are not a claim about tmux's exact parser ceiling. Every command's
`%begin`/`%end` or `%error` reply is consumed by InputClient so command failures reach the caller. Passthrough sequences
(audited): the control-mode pane-output stream carries `\ePtmux;...\e\\`-wrapped payloads still wrapped, regardless of
the `allow-passthrough` option — that option only gates forwarding to rendering clients, which Farhelm has none of — so
the supervisor unwraps passthrough payloads itself before they reach xterm.js. Reconnect replay prefills xterm.js from
`capture-pane -e` history, then continues with live bytes from the same control client — that is how the 10,000-line
floor is met without a gap between the two. The live stream also removes only the literal terminal queries that the
pinned tmux answers itself, retaining possible split prefixes briefly and flushing them after idle; a query split longer
than that idle window is deliberately forwarded and answered twice. This is a bounded byte transform, not a VT parser.
The handoff ordering is load-bearing: a separate tmux command process targets the incumbent control client by its
tmux-assigned name and switches it back to `no-output`; only after that process succeeds is the incumbent's stdin closed
and the process reaped. The acknowledgement cannot share the output client's protocol stream because cancellation may
leave older positional command replies unread there. tmux applies `no-output` by discarding all pending pane blocks for
that client and refusing new ones, so this is a client-wide boundary rather than a racy list of panes that existed when
teardown began. Closing or killing tmux 3.7b's client while one of those blocks remains can abort the whole private
server with `fatal: not enough data`; the existing handoff therefore retains the acknowledged no-output transition. Pane
modes, a history snapshot, a visible-screen snapshot, and a final `refresh-client -f !no-output,pause-after=N` are
submitted as one semicolon-separated command group through that replacement. The matching `%end` for the final refresh
block is the cutover: earlier pane bytes are represented by the snapshot, later ones arrive as live output, and
`no-output` advances rather than queueing a second copy for delivery. Normal-screen replay selects the history snapshot;
alternate-screen replay selects the visible snapshot so normal history is not mixed into a full-screen app. Known
limitation, accepted: an alternate-screen replay carries only that screen, so when the full-screen program later exits,
the browser's normal buffer behind it is empty until new output arrives, where tmux's own grid still held the
pre-program scrollback. Replaying both would add two captures to every attach for a cosmetic gain, and was declined
(review finding A7-C8).

Planned supervisor stops give orderly output-client teardown ten seconds, including waiting for the attachment lock. On
expiry, one best-effort attempt lists the private server's control clients and switches them to no-output, within two
additional seconds. It neither kills clients nor retries or verifies the roster; failure is logged and the process still
exits. This can quiet existing clients even when the attachment lock spent the original budget, but does not guarantee
that every client closes safely.

Further defenses against a tmux abort on abrupt closure of an output-bearing control client are not worth significant
complexity without an observed abort on tmux 3.7c or later. The recorded aborts are on distro tmux 3.6 and tmux 3.7b;
applicability to 3.7c remains unverified. Until that evidence exists, a rare path bypassing the existing safe-teardown
discipline is not a reason for further design or code. The existing discipline stays in place; an observed abort on 3.7c
or later reopens this decision.

The initial foreign-pane filters must be arguments of the cutover's final `refresh-client` invocation, described above.
Clearing `no-output` resets the client's per-pane state, so sending the filters as a separate earlier command silently
loses them. The first bounded batch rides the cutover; any overflow is explicitly filtered afterwards, when no
subsequent `no-output` transition will erase it. Late panes use the live filtering path, with a bounded memo to avoid
issuing a filter command for every output notification. The session sink must keep those filtered panes readable; local
dropping of foreign bytes remains separate from this reduction in tmux notification traffic.

Setting `pause-after` on that final cutover refresh (M2.5) changes the dialect the client then reads, which the parser
must handle rather than discard: pane bytes arrive as `%extended-output <pane-id> <age> ... : <data>` instead of
`%output`, and `%pause`/`%continue` notifications appear. Both output dialects are accepted unconditionally and decoded
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
pane's status. What a dead pane shows is exactly what tmux holds for it and nothing more: the supervisor takes no screen
snapshot before stopping a live process, stores no frame, and appends nothing to a dead pane's replay. A restart into a
surviving pane goes straight to `respawn-pane`, which keeps history and reinitializes the visible grid; the supervisor
neither shrinks the window to push the grid into history first nor hands the launch shim a frame to paint before `exec`.
SPEC.md forbids all three (the restart rules under Lifecycle operations, the no-snapshot rule under Terminal experience)
because a painted frame with no process behind it is worse than a blank pane; do not reintroduce them. The one trace
left is a startup sweep that deletes a `<state>/snapshots/` directory an older build wrote
(`sweep_legacy_snapshots_dir`), so an upgrade does not strand captured frames on disk. Exec failure versus ran-and-died
cannot be told apart by exit code alone (a missing command yields 127 and a non-executable file 126 — both
indistinguishable from a program exiting with that code), so classification does not rely on exit codes: the shell execs
`farhelm internal launch`, a shim that always exists, which resolves and execs the session's invocation and, on exec
failure, writes a sentinel with the errno detail to a per-launch status file (named by session and launch generation, so
a sentinel left by a failed earlier launch can never describe a later relaunch) before exiting. The supervisor
classifies **error** on that sentinel; the one sentinel-less error path is a launch whose login shell or `systemd-run`
scope wrapper died before the shim ever ran, recognized only by its full evidence shape (owned dead pane, launch spec
still unconsumed, no sentinel). An absent or unattributed pane does not qualify, and a completed user Stop keeps its
annotation. A scoped window command first execs `farhelm internal launch-wrapped`, which creates an empty
generation-named `.waypoint` file without following symlinks and then execs the wrapper with the ordinary shim command
appended. No waypoint means the login shell exited before Farhelm's command; a waypoint means the systemd scope wrapper
exited before starting the agent. The error names that step and includes the pane's exit code when known. New specs
carry `scope_waypoint_expected`; an older scoped spec without that bit retains the two-suspect explanation because it
never promised a waypoint. An unscoped command has no wrapper, so its unconsumed spec already identifies the shell,
including for older launches. A failed wrapper exec records the ordinary sentinel with the wrapper and errno. NOTE: a
sentinel written by the shell after a failed `exec` was audited and rejected — interactive bash survives a failed exec,
but zsh terminates on it in every mode, so shell-side code after `exec` never runs for zsh users; the shim works
identically under any `$SHELL`.

Never-started errors may retain the failed terminal's last five nonblank physical lines, at most 500 UTF-8 bytes after
presentation-unsafe characters are escaped. The excerpt follows Farhelm's diagnosis in the existing durable error
detail, with `|` between lines so the existing peer-text escaping in the badge and tooltip does not turn framing
newlines into visible control-character escapes. The separators count toward the byte bound. It is not a replay snapshot
or a conversation history store. A fresh pane starts its process-local fence at history position zero. Pane reuse
samples the old history size, stamps a random launch token and a separate validity token, then respawns in one
synchronous tmux command group. Respawn clears the old visible grid and preserves history. Every resize empties the
validity token immediately before resizing in the same group, so even a queued resize from an old attachment invalidates
a reused fence when it actually executes. A resize executed after pane death also clears generation eligibility: the
death banner can move away from the final row when the dead grid grows. Fresh resizes while the process is alive keep
generation eligibility because their later death draws the banner at the new bottom row. Reload and ambiguous launches
have no fence and omit the optional quote.

Capture uses `if-shell -F` to check exact session and pane identity, deadness, launch token and, for reuse, validity,
then captures in that same non-waiting command group. Its start is the recorded history position minus current
`history_size`, expanded by tmux at capture time. A history count below the recorded boundary omits the quote; history
trimming otherwise only skips additional new text. The final visible row is excluded conservatively because tmux draws
its death banner there. A one-row pane qualifies only when history extends beyond the boundary, preventing tmux from
swapping an empty numeric interval. The driver streams and reduces nonblank lines without retaining the full grid;
rendered attributes are omitted and the shared presentation-unsafe predicate escapes controls and invisible formatting.
These groups contain no waiting commands or hooks. Window tokens, like the existing window markers, describe Farhelm's
own private-server operations; arbitrary direct tmux commands are not an authenticated lifecycle interface.

Only the background observer and an already-dead Stop classification request this quote, after proving the unconsumed
spec/no-sentinel shape. Session lists never capture. Each capture has the existing tmux-driver timeout; the first
capture error disables optional quotes for the rest of that observer pass while ordinary errors still persist. A live
Stop never captures, and reboot and shim-reached classifications are unchanged.

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
sequences) get debugged at the tmux layer first; the generated config is the knob. Its truecolor capability and
`window-style` colors are what make OSC 10/11 answers agree with the browser's terminal theme.

## Helm ↔ supervisor transport: system ssh + stdio protocol

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
`farhelm internal stdio` on the remote side, which proxies stdio to the supervisor's unix socket. Supervisors listen on
that unix socket only — no network port, exactly as SPEC.md requires.

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
budget while still draining, and Debug-escapes what it does log — the same treatment the supervisor gives tmux's exit
reasons, for the same reason. Peer-supplied error text is normalized the same way wherever it is logged or retained in a
host's state. Host-manager diagnostics may be noisy and may repeat indefinitely: every failed attempt and refresh
remains in the stderr trail, including a peer that returns the same error on every refresh. A collision check still
emits one summary event for that refresh, rather than one event per colliding row. The SSH stderr relay's separate
per-connection budget still bounds which child lines it logs while it continues draining the pipe.

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
could not be executed/found, and 75 identifies Farhelm with no supervisor running and supplies the start command. Other
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
control messages are serde_json, terminal data channels are raw bytes. The same protocol runs over the unix socket
locally, so "local host" and "remote host" differ only in transport. Connection setup exchanges protocol and build
versions; it refuses protocol-version incompatibility per SPEC.md's version-skew rule, while build versions travel for
diagnostics only — mixed builds with a compatible protocol are the normal steady state SPEC.md describes.

Motivation: SPEC.md promises provisioning and transport ride "the user's keys, agent, and config" — only the real ssh
binary honors ~/.ssh/config fully (ProxyJump, Match blocks, the agent used to authenticate, ControlMaster). russh was
rejected for exactly that: partial config support would quietly break the promise. What the config does not get is what
rides Farhelm's connections: every ssh invocation carries `-o` overrides (`CONNECTION_OVERRIDES` in
`crates/farhelm-helm/src/ssh.rs`) for agent, X11 and port forwarding, `RemoteCommand`, `RequestTTY` and
`PermitLocalCommand`, which win over the config file. They set an OpenSSH 7.6 floor on the helm machine, and settings
whose override keywords are newer (`SessionType`, `StdinNull`, `ForkAfterAuthentication`, all 8.7) are left alone rather
than break older ssh. A port forward set up by a shared connection's master that predates an upgrade survives if the new
helm reuses that master within its 60 seconds of persistence (the ordinary path for the macOS desktop app: quit,
install, reopen), and lasts until the master exits (left unused for over a minute, its connection to the host dropped,
or `ssh -O exit` on its socket); see the `CONNECTION_OVERRIDES` docs. `ClearAllForwardings` leaves ProxyJump working,
because the jump runs as its own `ssh -W` child; that was checked with a real connection through a jump on localhost.
JSON control frames keep the protocol debuggable by eye; raw binary data channels keep PTY throughput off the JSON path.

`SessionInfo` carries a `last_activity_at` (unix seconds) beside `created_at`: the last time the supervisor observed
that session's agent pane change. It was added WITHIN protocol version 11 rather than bumping it, per the running rule
every version since 3 has followed — a new optional field with a decode default, whose omission a receiver can ignore
harmlessly, is additive; a new tagged variant or a required field is not. Absent means the sender predates the field and
decodes to 0, which a receiver reads as "unknown, fall back to `created_at`" and never as an instant in 1970. This is
recorded here because the per-version changelog in `lore/` is frozen at the moment it was written and is not maintained
as the protocol grows. The identity hook's pair — `ControlMsg::ReportConversation` and `ConversationReported` — went the
other way and took the protocol to version 12, since two new tagged variants are exactly what an older decoder refuses
outright instead of ignoring. The pair was later removed without a bump, when reports moved to files (see "The
per-launch identity hook"): only a hook ever sent it, a helm and a supervisor sharing a version never exchange it, and a
hook older than its supervisor during an upgrade is the accepted gap that move describes.

Protocol version 31 gives every terminal detach a code beside its reason. `ControlMsg::Detached` carries a `DetachCode`
(`taken_over`, `stalled`, `tab_closed`, `replaced`, `other`), a non-displacing attach refused because another client
holds the terminal is `ErrorKind::TakenOver` rather than a `Conflict` recognized by its message, and the helm forwards
the code in the browser's `detached` notice. Clients decide behavior from the code only (the browser latches a takeover,
holds a stall, and hides a closed tab) and show the reason as text, so reason wording is free to change. Before 31 the
helm and `terminal.js` compared English sentences they each kept a copy of, and only the disabled browser suite would
have noticed a rewording.

The helm also uses `tab_closed` for a tab attach refused as `NotFound`. That refusal describes current state, not a
passing condition: the supervisor looks the tab up in tmux's live panes, and a session or host that cannot be found has
taken its tabs with it. Reported as `other`, it sent the browser's reconnect ladder through every attempt for every
listed tab, rebuilding a terminal each time. The browser stops retrying on it, but paints the reason as a banner rather
than hiding the pane, because silence is the reap's promise for a tab this view had working. The agent terminal's
`NotFound` stays `other`: its reason is the supervisor's account of the session, such as a reboot and what restart would
do.

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
ends parked work; EOF ends both directions, with no half-close. Successful remote EOF drains preceding bytes before
closing the local socket, and a failure cannot be overwritten by a later success. There is no per-relay idle timer: an
idle WebSocket is healthy, and task ownership plus transport teardown supply cancellation. Existing shared-writer stall
limits still apply to actual transport writes. Session-authenticated peers cannot open, acknowledge or close these
channels; the restricted operation allowlist does not admit them.

The embedded helm owns IPv4 loopback listeners by session, canonical target and remote port, preferring that port and
falling back to a kernel-selected one. Listeners live in memory through agent stop/restart and host disconnect. Every
accepted socket resolves the current coherent session route again; the original host and any known installation identity
must still match. Successful deletion or replacement closes the old session's listeners and admitted relays; failed
deletion preserves them. Shutdown closes them before host connections, and weak application references keep an idle
listener from retaining the helm.

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

### What the helm believes from a supervisor

The helm's trust in an attached supervisor is scoped by effect, not by connection. A supervisor is believed about things
that affect only its own host: which of its sessions sent an upcall, that one of its sessions exited, what its panes
show. A lie about those damages only that host, where the supervisor already has full authority. Anything whose effect
reaches beyond its own host (another host's sessions, another supervisor, the helm's machine, or the helm's own state
such as settings and credentials) is allowed only where the spec grants it, and a request's arrival on a supervisor
connection adds nothing to that grant. The grants are the agent verbs any agent may use, each carried out only once the
user has approved it or the requesting host's "run farhelm commands from this host without asking" setting is on
(SPEC.md, Agent-spawned sessions; see "Permission prompts for agent actions"). A supervisor cannot, for example, ask the
helm to change a host's settings.

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
every descendant of the failed launch stopped: the rollback after a failed tmux creation accepts a systemd scope kill it
could not confirm once the portable sweep finds nothing, and after a failed database confirmation a sweep error only
adds to the returned error. A survivor of either can then keep running beside the fresh attempt this proof allows, which
needs two coinciding faults and falls under SPEC.md's two-fault exception in "Lifecycle operations". Keeping the session
when such cleanup is unconfirmed is deferred in TODO.md's entry on requiring a systemd user manager on Linux.
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
held for everything but this (version 33's change hint, below, carries no session data either). The supervisor picks the
helm that holds an attachment to the asking session — well defined by the one-attachment-per-session rule, and by
construction the helm the user is looking at, which is also the rule that stays correct if several helms per supervisor
are ever supported. Request ids are per connection on this protocol and stay that way: the asking process numbers its
own leg, the supervisor numbers the upcall from a counter it keeps per helm connection, and the relay holds the mapping
for one round trip.

Version 33 adds a second message in that direction, `ControlMsg::SessionsChanged`: a content-free hint that something a
user can see about the host's sessions changed. The helm still learns WHAT changed by drain; the hint only moves the
drain earlier. The supervisor owes a hint whenever a value it would list actually changes (a pane dying, a status
transition, the activity and work-start stamps advancing, a restart offer changing, and every lifecycle mutation), never
merely because a listing was served, so hints and the refreshes they cause cannot feed each other. It coalesces owed
hints into at most one per 200 ms and sends each on every full-authority connection's ordinary writer queue, dropping it
rather than blocking when that queue is full. Replies never run a capture sweep or commit outcomes; a pane-death wake
hints before observation and an outcome change hints again. The helm refreshes the hinted host at once, unless a refresh
that started with a hint pending began less than that same 200 ms ago (`SESSIONS_CHANGED_MIN_GAP` in the protocol crate,
which both ends use); then it waits out the rest of the gap, and the one refresh that follows answers every hint that
arrived meanwhile. Supervisor messages are untrusted, so the helm enforces the gap itself rather than relying on the
supervisor's spacing. Only a hint's own wake waits: the poll, a user's refresh and a nudge do not. A refresh a hint
caused always raises a feed event, even when the cache compares equal: a tab opened and exited between two refreshes
leaves the cache unchanged but a client's optimistic tab behind. The three-second poll stays as the backstop and keeps
its changed-only rule. A hint stays pending until a refresh that STARTED after it completes; a refresh the helm
discarded because one of its own seeded writes overtook it is retried as soon as the gap allows rather than left to the
poll.

The helm's client closes its transport when its final owning handle drops, even with an unanswered upcall. Answer tasks
hold writer senders of their own, so closing the client's sender alone cannot make a quiet connection reach EOF. The
destructor aborts its registered answer tasks and signals the existing reader and writer cancellation paths.
Cancellation also interrupts a frame already being written: the connection is closing, so preserving frame
synchronization cannot justify retaining its transport until a live-peer stall timeout. The manager also retires a
withdrawn connection explicitly: it must close while callers still retain obsolete handles, rather than waiting for
final-owner cleanup. Neither path cancels a mutation that has started: only read-only answers are registered for abort
(see "Who owns an accepted action").

The failure vocabulary is two kinds, split by whether a retry is free. `ErrorKind::Unavailable` means nothing is holding
the request: no helm is attached, its connection died before or during the request, the request could not be delivered
onto that connection at all, or the helm itself is not in a state to answer — still starting up, shutting down, or
serving a connection with no fleet behind it. `ErrorKind::Timeout` means the opposite and only that: the request was
QUEUED for delivery on the helm's connection and no answer arrived within the supervisor's budget. It may or may not
have reached the helm — the supervisor observes its own writer queue accepting the frame, never the writer transmitting
it — so a retry is neither provably free nor provably duplicative, and the message says exactly that rather than
claiming a delivery nobody watched. Closing that residual gap needs a per-frame transmission receipt from the
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
`agent-v3-<SHA-256 of the registry namespace, asking host row, session and key>`, with length-delimited strings. Each
helm database has a durable random namespace, minted at creation or migration, so a fresh registry cannot reuse an
earlier registry's approved retry when numeric host rows coincide. The host row keeps approvals tied to the logical host
across adoption. Existing databases also gain a namespace: an in-flight keyed retry across this upgrade needs approval
again and, if approved, creates a new session or checkout instead of returning the earlier one. The namespace is read
only for keyed agent requests, so offline commands can still open older schemas without migrating them. The fixed-length
hash keeps any agent key within the target's intent-key limit. The helm draws the one distinction that matters from it —
a clone whose result is the asking session or the named source is refused rather than reported, before anything durable
is recorded for the replayed row. The two creating verbs name their target host by display name, matching
`AgentHost::name`. Registry IDs join host and session discovery and distinguish duplicate labels; they are not
destination selectors. A name matching two registered hosts is refused as a `Conflict` naming the collision, never
resolved to whichever row the listing ordered first: display names are not unique by construction (the local row renders
as `this machine`, and an ssh destination may be spelled the same), and guessing between them would put a session on a
machine nobody chose. A registered name carrying a control character can never be typed back at all, since the relay
refuses one in `--host`; the not-found refusal says so by count, because the fix is a rename and an agent has no rename
verb for hosts.

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
override size with red. Native mode alone advertises the platform modifier for `can_open` files. Both plain and OSC
adapters forward their mouse event to the same transfer path; web modifiers always request a plain browser download.

`terminal-files.js` supplies conservative token recognition and a terminal-owned xterm addon below the existing OSC 8
and web providers. Public buffer cells map UTF-16 token offsets to wrapped coordinates, preserving wide/combined glyphs.
Logical lines over 8192 characters or 128 rows are not linkified. Whitespace, quotes and brackets delimit ordinary
paths; filenames with spaces require an OSC file link. Plain schemes and protocol-relative tokens are excluded. File
URIs decode once; their path belongs to the session host regardless of URI authority. No shell directory tracking or
hover cache is introduced.

The OSC handler enables `allowNonHttpProtocols` and owns the explicit scheme boundary: parsed http/https targets go
unchanged to the existing opener, valid file targets go to file lookup/download, and every other scheme does nothing.
Fresh hover evidence owns an AbortController and click admission. File and disclosed web-link activation checks the live
bounds of the approval region and tunnel-error notice for any overlap with the hover. A covered hover refuses the click
before transfer or opening. Its explanation sits above both covering surfaces until the pointer leaves the link, then
returns to the ordinary failure layer. xterm synchronously leaves and re-enters a link when its rows redraw, so leaving
withdraws click/cursor admission immediately but defers abort and tooltip removal to the next task. A re-hover in that
interval keeps the evidence only for the same displayed text, target path and snapshotted start/end cell range; a real
departure or replacement invalidates it, including raced replies. A kept ready cursor is restored in a microtask, after
xterm installs its decoration accessors, and only while that replacement link is still hovered. Teardown never writes
cursor decorations: xterm's accessors target whichever link is current, which may already be unrelated. Plain links
underline before lookup but gain a pointer only when ready. Click reopens the disclosed canonical path independently.
Browser bodies become a bounded Blob only after successful EOF; an object URL is released after the save click. The
desktop handoff secret identifies native mode, which requests `save_to_downloads` and adds `open` only for its platform
modifier-click when that hover's stat reply offered opening; otherwise the modified click saves and explains that
opening was not offered. It never falls back to Blob. Tooltip and transfer outcomes use textContent, peer-value
isolation and visible Unicode format escapes. The addon is disposed with its terminal, cancelling pending work; retained
reconnect terminals retain it.

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
the configuration ordering funnel; dismissal uses the per-field preference queue and overlays unacknowledged intent on
fresh reads.

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
per-field preference queue, including dirty-intent overlays and credential-recovery replay.

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
through their integration readers and Muse through the exhaustive launch-harness seam, because Muse shares Generic's
activity kind. The flag defaults false in older cached snapshots, resets with the run or failed capture, and shares the
existing activity cell across renames. Codex recognition uses its captured 0.159.0 folder-access menu; Muse uses a
captured 1.4.10-R6427.1 startup trust menu. Both require the dialog's choices and current footer instead of matching
isolated transcript words. Pi has no trust recognizer; its setting still supplies its project-resource approval flag.

Each host caches repository objects at `<state_dir>/repo-cache/<owner>/<name>.git`, derived from the validated,
lowercased repository pair. The supervisor passes that absolute path in the preparation's `repo_cache_path`; it is not
request input and does not change the frozen fingerprints. Old launch specs deserialize with an empty path, then refuse
the clone stage rather than bypassing the cache. A Ready restart skips Git and needs no cache path.

The shim takes the working-copy preparation lock first, then a blocking flock on the permanent sibling `<name>.lock`.
Both descriptors are CLOEXEC. While holding the repository lock it runs these commands in the session terminal, with the
same launch markers, environment scrubbing and `GIT_TERMINAL_PROMPT=1` as the clone:

```text
git init --bare -q <cache>
git -c gc.autoDetach=false -c maintenance.autoDetach=false -c gc.autoPackLimit=10 --git-dir <cache> fetch --prune <https url> '+refs/heads/*:refs/heads/*' '+refs/tags/*:refs/tags/*'
git -c core.commitGraph=false clone --reference <cache> --dissociate -- <https url> <cwd>
```

The fetch names the bare cache explicitly so Git's `safe.bareRepository=explicit` policy remains in force. Bare init is
idempotent, including after an interrupted initial fetch. Command-line refspecs keep the cache to branches and tags
without writing a remote configuration. Git's automatic gc remains enabled, with `gc.autoPackLimit=10` so accumulated
fetch packs consolidate sooner instead of making each dissociating clone repeat their delta search. It runs in the
foreground so it cannot outlive the lock; an occasional launch is slower while the cache consolidates. Plain
`--reference`, rather than `--reference-if-able`, makes an unavailable cache fail the clone stage; `--dissociate` copies
the borrowed objects so the checkout has no alternates and keeps its GitHub origin. The clone alone disables
commit-graph reads to avoid Git before 2.55.0 losing a cached commit's tree during dissociation; this command-local
workaround can go once Git 2.55 is the minimum (upstream fix `3d8e4004c6`). The lock stays held through dissociation,
then its modification time records successful use. There is no timeout or fallback. Cache errors name the path and use
the existing `Failed{stage: clone}` state; `CloneStarted` covers the whole sequence, including restart refusal after
interruption.

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
including a builder interrupted before recording a per-tree identity. Unknown names and other repositories remain
untouched. The colon is excluded by repository-name validation, so private names cannot match another repository's ready
slot in the shared namespace. A ready `<name>` slot requires its own recorded identity; namespace ownership never adopts
a replaced ready spare. Cleanup renames the selected directory to a fresh no-replace trash name, verifies it, then uses
the archive's shared opened-handle confined recursive remover. It never recursively removes by checkout pathname or
crosses links or mounts. The verified namespace is fsynced before a record can forget a removed ready identity or retire
its location. A failed cleanup or namespace barrier retains the original identity and location, including when removal
completed; an absence-only retry still needs that namespace barrier before retiring the evidence.

Startup collects exact launch-generation, preparation-state and owned-pane evidence before the blocking cache sweep. A
still-live clone claim is retained, including a live NotStarted preparation that can begin cloning after the sample;
failed or uncertain liveness observations retain possible claims rather than inferring abandonment. Spare cleanup runs
under the cache sweep's existing nonblocking repository lock, skips held locks, and evicts ready spares with the same
thirty-day cache observation. Missing ready content alone never permits forgetting a location with a surviving claim. A
failure is logged and retains the ownership record for later cleanup. Record reads are limited to 16 MiB and
owner-namespace scans to 100,000 iterator calls, including EOF proof. Exhausting either limit refuses cleanup and
retains ownership evidence; neither truncates or silently forgets old locations. This recovery layer does not itself
create, publish or take spares.

The preparation shim can consume a complete spare when its private `keep_spare` field is true. That field defaults to
false and is omitted when false; the create path currently leaves it off, and nothing produces spares yet. A take runs
inside the existing durable `CloneStarted` stage and preparation lock. It never reads the recovery record or acquires
the repository cache lock. Git below 2.17 or unrecognized version output keeps the ordinary clone path: 2.17 is the
floor for the take's `--prune-tags`, not a minimum for Farhelm. Local metadata queries are silent, limited to 4096
stdout bytes and one second, with stdin closed, stderr discarded and the existing bounded process-group runner owning
kill and reap. Actual fetch and merge inherit the terminal and have no timeout.

The shim derives the strict claim's session and generation from its actual launch-status filename, checks the session
against the launch spec, and makes one no-replace rename from the ready slot to that claim. It opens both the reserved
namespace and allocated direct-child destination below the canonical checkout root without crossing links or mounts. The
allocation's current device/inode and empty-directory checks remain mandatory. Git's checked-out branch, configured
remote and full upstream tracking ref come from the claimed clone itself, including non-`origin` remotes. Gitfiles, bare
repositories, redirected worktree/common directories and remaining alternates are structural refusal, rather than
metadata the shim rewrites. Ordinary hooks and filters remain configured; a dirty spare is not rejected merely for being
dirty.

A usable claim prints `Farhelm: updating a checkout prepared earlier from GitHub.` and runs
`git -c remote.<remote>.followRemoteHEAD=always fetch --prune --prune-tags <remote>`. That key is unconditional on every
take-capable Git. The saved tracking branch must survive pruning and match the remote HEAD symref afterwards. Git 2.48+
refreshes that symref; older Git intentionally keeps its clone-time default-branch observation. Structural refusal
before moving content leaves the claim for supervisor cleanup and uses ordinary clone into the still-empty allocation. A
failed fetch instead fails the clone stage, with no second clone or credential prompt. A successful take makes one
fetch; a successful fetch followed by structural refusal may subsequently use the explicitly required normal clone.

Before moving, the shim rechecks destination identity and emptiness and preflights all raw top-level entries for mount
or filesystem crossings. It moves `.git` first and then each remaining entry with fd-relative no-replace renames,
preserving the allocated folder itself. A failed first move permits clone only if the destination is still known empty;
every failure after any entry moved retains the partial checkout and fails preparation without rollback. The shim
removes only the emptied claim directory. It then runs `git merge --ff-only <full tracking ref>` in the final allocated
folder, so changed-file filters and `post-merge` see that path. The existing post-clone command, Ready publication and
trust steps follow unchanged. Interrupted or failed preparations remain non-restartable under the existing state rule.

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
traversed. Directory admission stays held through the blocking operation and its live-session directory guard, including
after caller cancellation; large deletes can therefore delay launches, restarts and session Deletes on that host.
Stopped session rows also protect their folders because they can restart there. The guard compares raw and
admission-time canonical session paths against the archive's recorded and current canonical path; it does not resolve
unrelated session cwds under admission.

Passive lists share the two-slot browse worker pool, including a syscall that outlives the caller's reply wait, and
never hold directory admission. Permanent deletion runs on a separate blocking worker serialized by directory admission,
so wedged browse slots cannot block session lifecycle through an empty request. Listing without sizes does not walk
checkout trees. Requested disk usage sums allocated 512-byte blocks from no-follow descriptor-relative metadata,
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
listings, dialog open and cleanup completion trigger reads; no new poll runs. A same-connection count remains visible
during measurement, but an old read generation cannot overwrite this client's deletion result. Other clients' cleanup
can leave the count stale until the dialog opens. Host-list refresh failures are disclosed rather than treated as proof
of current reachability. Unknown host counts stay unknown; the global cleanup requires successful, settled listings from
every reachable host, and refuses a host above the protocol's 10,000-ID batch limit rather than silently omitting it.

Confirmations freeze deduplicated IDs and observed host incarnations. Diagnostic IDs are selectable for explicit
forgetting, while every diagnostic remains visible, including several for one ID. Before dispatch the UI rechecks
connection identity and takes the existing shared operation guard. An immediate single-entry delete freezes only that
checkout's ID and its host incarnation at click time, requires the full host listing to be eligible, and uses the same
dispatch and completion path without a confirmation. Separate host requests run concurrently; the guard lasts until all
replies arrive. Successful record removals are folded into the displayed listing, all issues retained, and every
completion refetches. A lost cleanup reply warns that some contents may already be deleted and refetches; it never
claims that a folder is intact. The archive cue stores row geometry on the persistent trash button, consumes it on the
local Delete reply, and uses finite Web Animations with reduced-motion refusal. No global geometry cache or new asset
file is introduced.

### Runtime state

- State in SQLite (rusqlite) at `~/.local/state/farhelm/supervisor.db`: sessions and their metadata (SPEC.md's
  supervisor-authoritative list), spawn idempotency keys, captured conversation identities, host identity, and the boot
  id last seen. Comparing the stored boot id against the current one (`/proc/sys/kernel/random/boot_id`;
  `kern.bootsessionuuid` on macOS — a per-boot UUID, chosen over `kern.boottime` because the kernel rewrites boottime on
  clock steps and a boot id must never change mid-boot) is how "interrupted" is classified per SPEC.md.
- Host identity: generated once at first run, stored in the db.
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
  systemd older than 255 don't set it), the supervisor falls back to the passwd database, then `/bin/sh`.
- The supervisor's shared agent-launch builder also writes the intended working directory into the launch spec. After
  checkout preparation, the shim enters it before exec and supplies matching `PWD`; a failed chdir prevents the agent
  from executing and writes the ordinary launch-failure sentinel naming the directory. Entry is separate from exec so an
  absent or non-executable agent is reported as a program failure, rather than a directory problem. tmux's `-c` alone is
  insufficient because tmux falls back to HOME when it cannot enter the folder, and login-shell startup may cd again.
  Older specs lacking the optional field retain their inherited directory. Terminal tabs run `internal tab-shell` inside
  their existing scope, through the supervisor's own program rather than the app forwarder; the helper enters the folder
  and scrubs the agent/private-tmux markers before execing the login shell. A failed tab entry reaches the existing
  dead-before-open refusal with its last output. This keeps tab startup inside containment and preserves a tab shell's
  own startup-file cd, while agent startup restores the session folder after those files have run.
- Status heuristics: periodic sampling of each live agent pane's visible grid, read by one screen reader per agent kind
  (`agent_kind::screen_reader`). Sampling must never sit on the attach/input path — SPEC.md forbids status from gating
  interaction. The supervisor's own ticker takes the samples and stores one reading per successful capture; replies only
  map the stored reading, and classification sits BELOW the recorded-error and dead-pane rules in the existing
  precedence, so a reading only chooses among the live statuses of a pane tmux says is alive. The generic reader is
  observed output alone, counted in a session's OWN samples rather than in elapsed time: three consecutive samples
  showing an unchanged screen reads idle, anything else live reads running, and it never reads waiting (a blocked agent
  and a finished one are equally quiet). Counting samples rather than seconds is load-bearing — the sampler works
  through live panes on a budgeted round robin, so a session's real sampling period grows with the fleet, and any
  wall-clock window would eventually report a continuously-working agent as idle because the HOST was busy. A new launch
  reads running before its first comparison. A reloaded live pane instead reports provisional `unknown` until a changed
  screen, a reading from recognized content, or three quiet comparisons provide fresh status evidence (a recognized idle
  prompt therefore ends the gap on its first sample); a recognized waiting prompt is reported at once. Dead-pane and
  recorded-error outcomes bypass that provisional state. The helm retains the prior status from its per-host cache for
  an `unknown` reply, if it has one, and replaces the rest of the row as usual. Its identity-less in-memory list follows
  the same rule within a connection. Disconnect clears those rows by the existing identity-less host rule, so there is
  no previous status to retain after its supervisor restarts.

  Claude Code and Codex have dedicated readers that answer from recognized content ("anchored" readings) rather than
  change counting, because both redraw parts of their screen while idle (Claude's `/clear` hint, Codex's recap block and
  rate-limit footer) and change counting reads every such redraw as work. The anchors are the vendors' own wording and
  layout, preferring text shown to the user as an instruction over decoration: every dialog that asks the user something
  ends in a key-hint footer ("Esc to cancel", "enter to submit answer", "enter continue"), Claude shows a spinner line
  directly above its ruled `❯` input box for the whole of a turn. Its verb can have one or several words, followed by an
  ellipsis and a parenthesized timer beginning with a digit; compaction (`Compacting conversation… (1m 3s)`) counts as
  working too. Finished-turn prose without that ellipsis remains idle. Reply text of the same shape just above the input
  box (`- Ran the suite… (2 failures)`) also reads as working; that cosmetic false positive is accepted. Claude's
  explicit `Waiting for N background … to finish` announcement in that same position also means the turn is still
  working, while footer hints and a listed task without that announcement remain idle. Codex animates a Braille spinner
  in its pane title for the whole of a turn (its on-screen `Working (…)` widget appears only during tool runs, never
  while prose streams, so the title is fetched with a separate `display-message` for Codex sessions only — the title is
  written by the pane, so it never rides the authoritative pane-fact query). A waiting prompt wins over a busy
  indicator. A recognized screen that carries no state (a model picker, a transcript view) reads "can't tell" and the
  previous reading stands. A screen with none of the anchors falls back to the generic reader for that sample, and after
  a few such samples in a row (so a half-drawn startup frame does not count) the supervisor logs that once per run,
  without screen content, as likely rule drift. The rules are tested against real captures under
  `crates/farhelm-supervisor/tests/fixtures/screens/`, which `scripts/capture-agent-screens.py` re-captures from the
  installed agents (`docs/agent-screen-fixtures.md`). The earlier Codex-only masking of its composer out of change
  comparison is gone: whenever it applied, the Codex reader now recognizes the composer and answers from content, so the
  comparison no longer decides anything there.
- Permission-footer evidence: the same sampling pass calls the exhaustive launch-harness reader in
  `agent_kind::screen_reader`, independently of the activity kind (Muse and Cursor use generic activity). Its optional
  bool latches `ActivitySample::screen_yolo` on the first definite YOLO reading of a run and hints connected helms only
  when that value changes. Later non-YOLO readings, unrecognized modes and covered footers cannot clear it. Rename
  shares this run cell; restart replaces it. After a supervisor restart it is false until the next definite sample. The
  shared `entry_info` builder publishes it as an additive bool omitted when false; helm cache merges take that incoming
  value as given, including a restart's false. The sidebar escalates the drawn launch glyph and puts the vendor wording
  ahead of its existing hover/accessibility description. No admission or inheritance path reads this field. Captured
  grids live separately under `tests/fixtures/status-lines/`, with vendor version and capture date, because these
  permission screens do not satisfy the activity-fixture contract.
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
  one pane-state query for immediate exits and tab changes, but no capture, launch-artifact reads, cleanup or outcome
  writes. Resume readiness, launch-error classification and notification resolution may lag by one nominal two-second
  tick; the pane-death wake commits newly dead owned agent panes without sweeping already-stopped sessions. Unreadable
  launch evidence is logged and retried by the ticker and does not fail lists. Found but uncommitted launch errors stay
  in generation-local memory for replies. Successful negative launch-sentinel and preparation reads settle for the
  generation only after a durable terminal outcome and an owned pane seen dead, or proof that the launch's boot ended.
  Schema 30 records that proof for all retained launches in the boot-change transaction, without altering already-ended
  outcomes, and restores it on later same-boot supervisor reloads. Older rows default to unknown: a reboot an older
  supervisor already consumed cannot be reconstructed. New launches clear the proof; a definitively aborted restart
  restores it with the prior external run's facts under the advanced generation. Same-boot pane absence without such
  proof keeps retrying: an empty tmux query can be transient while the shim still writes. Read errors, uncommitted
  failures and observers without recording authority never settle reads. Rename shares the latches; relaunch and
  supervisor reload read again. Error artifacts are cleaned once after successful accepted-create evidence preservation
  and removal, with failed cleanup retried. Report-backed identities still refresh under their capture claim because
  accepted reports and decision-time Restart can change the row after the pane stops. An injected launch holding no
  identity warns once after 65 seconds from the first input frame delivered to the agent pane that holds an Enter
  (`capture::submits_a_line`): a carriage return that is not preceded by ESC (Farhelm's own Shift+Enter sends `ESC CR`
  to insert a newline) and not inside a bracketed paste of the same frame (xterm.js turns pasted newlines into carriage
  returns), in a frame whose every chunk tmux confirmed. The terminal's automatic replies to the agent TUI's own queries
  (device attributes, cursor position, colour answers, focus reports) never carry a carriage return, so an agent the
  user opened but has not typed into cannot trip it. An Enter while the latest screen reading before delivery is
  `Waiting` answers a recognized dialog and does not start the clock. An outdated waiting reading can defer the clock to
  the next Enter; no capture runs on the input path. With no reading yet, or no dedicated reader, the existing
  submitted-line rule applies. A paste large enough to span frames still has its middle frames judged without their
  markers. The spawn records whether its argv received the hook before tmux starts, fenced by launch generation. Reload
  restores that flag; older rows default unhooked and stay unchecked. The anchor is an in-memory monotonic instant,
  reset with the diagnostic latch on every relaunch and supervisor restart, so a picked-up launch starts its clock at
  the next qualifying Enter after reload rather than recovering the time of an earlier Enter. A Resume carries its
  identity and therefore stays silent even if its new hook never reports. The warning changes no offer or admission
  rule.

  **Codex attribution and exact-record validation.** The hook records its own process ancestry when it makes a report,
  and the supervisor anchors that chain at the session's owned pane process (see the shared framework below). For a
  Codex report, the innermost native executable whose basename is `codex` is the emitter. Any processes above it are
  launch ancestry, including resident Codex-named launchers, scripts, shells and package managers. The supported process
  chains are documented in [the Codex integration](website/src/content/docs/docs/agents/codex.md). The reporter must
  spell the installed hook invocation (`<farhelm> internal hook …`, matched syntactically so an upgraded supervisor
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
  separately from the conversation locator. Schema 31 defaults existing rows to NULL without backfill; admission reads
  the pin under the capture claim and writes it in the same CAS as the conversation, fencing the complete prior binding
  and exact previous pin. No in-memory pin mirror is needed. Relaunch clears it, and abort does not restore it. Later
  reports must come from the pinned emitter. A recorded Codex ancestor takes precedence: its verified record
  independently selects and corrects both pin and conversation, never inheriting the child's file or persistent thread.
  A correction records the once-per-launch Codex correction notification in the same transaction as adoption;
  notification-write failure rolls back the correction, and reload restores it even if publication was interrupted. It
  does not resolve on capture because it records the prior mistaken target. An unverified ancestor changes nothing, and
  its unready Stop is acknowledged without effect. A pin whose process is gone, reused or non-running releases for a
  fresh emitter only if the pin is absent from the new report's chain; descendants cannot inherit authority after its
  exit. Unavailable process evidence does not release it. Rows already running across an upgrade start unpinned: a
  nested child's first selecting report after upgrade may pin it, and the root's next report corrects and notifies.
  There is no upgrade suppression.

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
  after publication, which discovers the owned pane from tmux when the row has not recorded it yet. Historical bare
  Codex IDs are retained but fail closed rather than being guessed into a new locator.

  Admission and pending confirmation verify the exact file; passes, list/info snapshots and replayed creates use the
  stored binding. Restart verifies once before any launch mutation. A missing file persists withdrawal through the
  exact-locator/generation CAS and notifies only when that write wins; inconsistent metadata withdraws without a
  notification. An OS read error refuses with a retry message without changing the binding. Grok uses the same verdict
  policy for its exact pair. The existing final relaunch comparison still fences a conversation report racing Restart.

  **Grok attribution, ordering, and exact-record validation.** Grok uses the same recorded and anchored ancestry,
  capture claim, complete-binding CAS, ownership provenance, and mirror helper as Codex. Its corridor requires exactly
  one native `grok` image with `--no-leader` in the option region, followed only by the hook-shaped reporter and the
  same narrow shell trampoline. A nested Grok, foreign session runtime, missing option, or unreadable argv refuses the
  report. This is the supported native corridor; wrappers may still launch but cannot establish capture ownership.

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
  resolves. Collection does not know the pane, so it climbs until the ancestry ends, a process cannot be read, or a
  budget runs out, and notes why it stopped; every edge and image is then re-read, and the chain is cut below the first
  one that changed (an exec between observation and re-read). Anchoring runs in the supervisor, when it applies the
  report: the chain must contain the session's current pane process, by pid and start token while that process is alive
  and by pid alone once it has exited and tmux still lists the pane, and is cut there; it must be one unbroken line of
  parents within the depth and argv budgets, re-checked because the chain is now file input. Every launch runs a new
  pane process, so anchoring is also what ties a report to its launch, for every vendor. The per-kind step applies its
  own restrictive corridor to the anchored chain — only the reporter plus a narrow trampoline between runtime and
  reporter; any other session-hosting runtime or unclassified intermediary below the emitter refuses. Codex attributes
  its hook-shaped reporter through shell-`-c` trampolines to the innermost native `codex`, accepts arbitrary ancestry
  above it, and applies its durable process pin. Grok retains exactly one native image with the required `--no-leader`
  option. Admission runs five steps: cheap envelope/kind/generation gating with no vendor I/O (the report file's
  discriminator check re-applied against the fenced resolution, plus raw event/source/agent-identity validation before
  diagnostic sanitation); the unbounded capture claim (one shared session-keyed `capture_locks` registry for report
  admission and readiness refresh, never the lifecycle claim; slow waits are logged for diagnostics) and a reload
  comparing kind, generation, and the complete prior binding; the mutation-free runtime and vendor-root proofs over the
  recorded chain (evidence recorded when the report was made cannot change during verification, so there is no repeat
  walk); the atomic generation-plus-complete-binding CAS committing identity, locator, provenance, source, and readiness
  reset together; and the mirror of only the committed result into the matching current-generation entry under the same
  claim. Rejection at any pre-write step changes nothing durable, in memory, pending, or offered. Refresh and
  report-only reconciliation passes take the same claim and reload before mirroring, carrying the row's version beside
  the identity, so memory-derived offers apply the same gate as row-derived ones without a second lookup — and a
  rejected report never triggers a readiness withdrawal through them. Restart verification and lifecycle resets retain
  their durable generation/binding fences; they do not all acquire this capture claim. The report claim has no time
  limit because every operation under it is local and no hook waits on it; a slow wait is logged for diagnostics.

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
  gates on; protocol 29 adds Grok to that closed enum together with its agent-kind and launch-harness variants. Senders
  predating either required variant fail closed at decode and at the missing CLI flag alike. Migration 21 adds
  `omp_reporter_asset` (nullable `TEXT`, identical in fresh DDL and `ALTER`): the gated reporter asset's file name as
  installed by the session's current launch, written pre-spawn — after the launch spec publishes, before tmux can start
  anything — from the one place that decides injection and fenced on the launch generation, so no report of the
  generation can arrive ahead of its provenance. Migration 22 adds `omp_launch_program` the same way: the program that
  launch's argv started, retained beside the marker so admission classifies the current launch rather than the resume
  template (a future resume's command). OMP admission requires the marker to name the current binary's asset with the
  file's bytes re-verified, so changing the asset breaks reporting for every OMP session already running (see "What
  running sessions hold across versions"); pre-21 rows adopt `NULL` and fail closed. OMP takes no Codex-style exception:
  every older OMP row has Restart unavailable (`not_captured`) until its first proven report.

  **Interim ownership states.** The discriminator gate applies to every kind now: it is envelope, migrated together.
  Attribution proofs apply to Codex, Grok, and OMP. Goose and Pi retain their existing acceptance behind the
  discriminator gate, and new framework entry points default to deny rather than allow. Claude stays on that legacy path
  with one added check before the capture claim: the report's anchored chain (`procs::claude_corridor`), skipping the
  hook's narrow `sh -c` trampolines, must end at the pane process or its direct child. It is positional on purpose.
  Claude's native binary is named after its version and npm installs run under `node`, so an image rule would have to
  track install layouts, and the injected `--settings` hook is a vendor detail that may change on its own; the closed
  attempt in PR #830 shows where following either leads. A shelled-out child is always at least two links below the
  pane, because the foreground's Bash tool runs it through a shell that does not `exec` it. Accepted costs: a wrapper
  chain deeper than one level loses hook capture and, without a report, cannot be restarted, and a child the foreground
  Claude spawned with no shell between them in a wrapperless launch would be admitted (not observed; the Bash tool
  always interposes a shell). Reports reach admission one at a time, from one drain of the session's report files, so
  two Claude reports never attribute concurrently; the generation fence keeps a check made before a relaunch from
  committing into the new launch. The check writes no provenance and does not flip Claude's predicate, because flipping
  it would make every existing Claude capture unresumable until its next proven report, and stopping replacement needs
  no version. The offer gate has its final shape but flips per kind: Codex, Grok, and OMP require version 1, while the
  other kinds keep today's offer behavior until their proof lands, writes 1, and flips the single per-kind predicate
  every surface consults. There is no general report epoch. Grok's locator carries only its vendor-specific selection
  timestamp; no other kind inherits that ordering rule. OMP uses serial cancellation fences, not cross-reporter
  chronology. Old processes and assets fail closed after the upgrade; nothing is grandfathered.

  **The per-launch identity hook.** Claude Code's `/clear` and Codex's `/new` can replace the conversation inside a live
  process. Farhelm needs the agent's explicit report so Resume does not return to the discarded conversation. Both
  vendors fire a `SessionStart` hook whose payload carries that id, and both accept a hook supplied on the command line
  for a single launch, so farhelm passes itself as that hook (`farhelm internal hook --vendor <adapter>`, which drops
  its report as a file for the supervisor to apply; see "Report files" below) and lets the agent state its own identity.
  The `--vendor` flag is the report envelope's discriminator, sourced from the installed entry point rather than
  inferred from the payload; the Goose helper supplies its own value internally so the persisted declaration keeps
  invoking the same command, while the Pi/OMP assets pass theirs on the spawned command line and keep their JSON
  `vendor` field purely as a consistency check. Claude takes it as `--settings <json>`; Codex takes
  `--dangerously-bypass-hook-trust -c features.hooks=true -c hooks.SessionStart=… -c hooks.Stop=…`. Per-launch is the
  whole point: nothing is written to `~/.claude` or to Codex's active configuration home (`$CODEX_HOME` when set,
  `~/.codex` otherwise), no trust state is left behind, and flags cannot outlive the process they were passed to — which
  is what keeps SPEC.md's no-agent-configuration rule intact rather than merely bent. The costs are accepted
  deliberately, and both are scoped to the launches that actually carry the injected flags rather than to Codex launches
  in general: on those, Codex prints a hook-trust warning line above its composer, and with trust bypassed any hook the
  user has in that same configuration home but has not trusted runs too. The same bypass covers a trusted project's own
  `.codex/` hooks: Codex loads a project's hooks only once the folder is trusted (Farhelm's managed-checkout Codex trust
  setting or the user's own answer to Codex's trust prompt), and on an injected launch they then run without Codex's
  per-hook review. That is accepted because trusting a workspace already hands its Codex configuration, MCP servers
  included, the ability to run commands, and because skipping injection there would drop conversation identity, and so
  resume, for trusted checkouts. It is accepted only until hook installation becomes an explicit step surfaced to the
  user, where the user is told what is being installed and accepts specific hooks; after that, launches no longer pass
  the per-launch bypass. Codex fires `SessionStart` at the first prompt rather than at process start, so a Codex
  session's identity arrives only once the user has typed something, where Claude's arrives at startup. The flags go
  where the launch's `{farhelm_args}` stands (see "Launch kinds: one resolved launch"); nothing reads the rest of the
  command. Only a legacy session, from before launch kinds, still gets the previous release's injection, appended after
  its argv, and for it three invocation shapes disqualify a launch, which is skipped with a logged reason rather than
  made to work: an argv that already carries `--settings` (Claude honors only the last one, so injecting ours would
  silently drop the user's), an argv already steering Codex's own hook configuration (a second bypass flag risks a
  rejected command line, and the `hooks.`/`features.hooks` tables are the user's once they touch them), and — for either
  vendor — an argv containing a bare `--` (our flags would become prompt text). `FARHELM_AGENT_HOOKS` in the
  supervisor's environment — `all`, `none`, or a comma list of kinds — turns injection off wholesale or per kind, read
  once at supervisor start and carried as a seam value. Without an accepted report, a new session cannot be restarted;
  no nearby record can supply a substitute identity.
  `website/src/content/docs/docs/how-it-works/agent-hook-injection.md` is the user-facing account of the same mechanism.
  The injected command line outlives the binary that wrote it, so it is part of what newer binaries keep accepting (see
  "What running sessions hold across versions").

  Codex's Stop hook carries no instructions announcement: only SessionStart adds that context. Sessions launched before
  Stop was injected retain their old argv, so a pending clear in one of them cannot be confirmed until relaunch. No
  polling or upgrade machinery compensates for that accepted breaking gap.

  **Report files.** The hook never talks to the supervisor and never waits for it. It reads the vendor's payload
  (bounded at 30 s, for a vendor that holds stdin open, under the 60 s outer timers Farhelm sets or documents; Pi and
  OMP keep their published 2 s child timers), records its process ancestry, writes the report atomically into
  `hook-reports/<session-id>/` under the state directory (the socket's parent), and exits 0. It requires the launch's
  complete credential environment, as before, but carries no token: the file sits in the supervisor's private state
  directory, and attribution, not a secret, is what stops a nested agent's report. Each session has fixed slots, each
  replaced by rename: `latest.json` for the single-report integrations; Codex and Grok use `selection.json`
  (`SessionStart`) and `enrichment.json` (Codex Stop; Grok's later events), drained selection first. Enrichment is
  refused without its selection, and the separate slot keeps a long outage's enrichments from evicting it. The directory
  is therefore bounded at two files with no queue, cap, or eviction rule. Two hooks firing at once still race for a
  slot, and the later rename wins. A report that names a sub-agent is never written, since it would take the slot of a
  pending report from the session's own agent and then be refused; a nested runtime that carries no such marker (a
  second Grok started inside a Grok session, whose hooks are global) can still take that slot, within one pass while the
  supervisor runs or across an outage, and is then refused, which is an accepted gap.

  The supervisor applies waiting reports on recursive file-watch events and at the start of every reconciliation pass
  (`capture_now`: the 2 s ticker, startup, reload, Restart) and does nothing while it is not recording. It takes a slot
  by renaming it to a private name, so a hook refilling the slot meanwhile is not deleted with it, checks the report
  (the vendor naming the session's durable kind, the conversation's size bound, the sub-agent marker, each vendor's
  source vocabulary), anchors its chain at the current pane, and runs the five-step admission. An accepted or
  definitively refused report is deleted; one that could not be read from disk, or was refused for an `Internal` failure
  (a store or tmux that could not be read), or was judged while a relaunch moved the session to a new generation, is put
  back unless the slot was refilled, in which case the newer report wins, and ends that session's pass. A report made
  under an earlier launch, or whose pane no longer exists, fails the anchor and is discarded. Reports of a session that
  is not published yet wait for the pass after publication. Drains never overlap; a taken slot found at the start of a
  drain belongs to a supervisor that died mid-drain and is put back. A pass that finds another drain running skips
  draining rather than waiting behind a slow admission; Restart is the exception and waits for it, then drains again,
  because a report waiting on disk may name the conversation it is about to resume, and after the relaunch that report
  would be discarded as the old launch's. The watch task also waits for the drain lock, so an event is never lost to an
  already-running drain. Every settled report leaves a verdict line in the session's hook log, beside the hook's own
  `written` line: `acked`, or `refused` with the error kind and reason. Delete removes the drop directory; a pass
  removes one whose session no longer exists, which a hook racing the delete can recreate. The supervisor creates the
  report root with mode 0700 before attaching notify's recommended recursive watcher (inotify on Linux, default FSEvents
  on macOS). An independent task drains the whole root; a single stored wake coalesces bursts and retains events during
  a drain. A 100 ms pause between drains bounds the feedback from retried reports linked back into place. Access events
  are ignored so the drain's own reads cannot keep waking it. The timer remains the backstop. Watch setup or backend
  failure logs once and disables the watch for that supervisor lifetime, without changing periodic pickup. The ticker
  handle owns the watcher's cooperative stop; in-flight admission completes before backend drop requests native shutdown
  (notify's inotify thread finishes asynchronously). A closed supervisor lifetime channel ends an idle watch when the
  last supervisor reference disappears. macOS backend batching can add latency; pickup on Linux is normally well below a
  second. A report made while no supervisor runs waits on disk for the next one; a supervisor older than the hook binary
  (mid-update) may miss reports briefly, which is accepted.

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
  imply the report passed the asset's context gate. Attribution walks the shared mechanics to the current owned pane and
  applies OMP's restrictive corridor over the installation descriptor the durable launch argv selects — classified at
  spawn and retained with the generation's provenance, never re-derived from the resume template: `Oj` (the selected Bun
  running the selected bundle `dist/cli.js` or source-tree `src/cli.ts`, entry spellings resolved through symlinks
  because the installed `omp` command is one), `Ob` (the compiled target with TUI grammar), `L` (an exact `bun x`/`bunx`
  or npm/npx package selection above the runtime, Bun-resulting only), `S` (a known transparent `sh -c 'exec <runtime>'`
  trampoline, or an exec'd-away shell that leaves no link). Nested or additional session-hosting runtimes of any kind,
  unclassified intermediaries, Node-executed OMP, unknown wrappers, ambiguous package scripts, and a Bun or Node pane
  process above the reporting runtime whose arguments cannot be read (which would otherwise let a nested runtime below
  it stand as the emitter) all refuse, for every launch program. Under a `bun x` or npm launch, a readable pane keeps
  its positional exemption without an expected-launcher match, because npm rewrites its process arguments and that match
  would refuse real launchers. An installed `omp` launch has no launcher at its pane, so there any Bun or Node pane that
  is not the reporting runtime refuses, readable or not. The live runtime argv is re-read through the same grammar
  injection uses, so a process that exec'd from a TUI launch into a utility or print shape refuses too. Admission checks
  the report's recorded ancestry once, anchored to the current owned pane. The source vocabulary is the asset's four
  tags (`session_start`, `session_switch` with its opaque upstream reason, `session_branch`, `agent_end`), allowlisted
  at the doorway and re-checked at admission; `agent_id` stays a rejection signal. The session-file header stays a
  pre-resume file↔id check, not ownership evidence, and a parent lineage field never rejects. Supported versions are
  18.2.4 and 18.2.6 with equal gate-semantic pins verified at the pinned sources (static claim, no runtime probe).
  Compiled and package-launcher forms have chain-level shape coverage; live lifecycle evidence covers the Bun-executed
  entry. Node execution and unknown wrapper shapes remain refused, as described in
  `website/src/content/docs/docs/agents/omp.md`.

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
  adding to them. One mechanism, both vendors. Two shape constraints follow from the vendors and are pinned by tests:
  stdout starting with `{` and ending with `}` is parsed as JSON by both (and a failed parse fails the hook run outright
  on Codex), and the line is one line, ASCII, and short, because every session pays for it whether or not anyone ever
  says `$farhelm`. The full instructions are printed only on demand by `farhelm agent instructions`, whose verb list is
  generated by walking the binary's own clap definition rather than transcribed — a verb that exists but goes
  undocumented is one no agent ever discovers, and nothing else would notice. `FARHELM_AGENT_INSTRUCTIONS` in the
  supervisor's environment — `on` (the default, also what unset or empty means) or `off` — controls the flag, read once
  at supervisor start and carried as a seam value beside `FARHELM_AGENT_HOOKS`. Read once for a sharper reason than
  symmetry: Codex fires `SessionStart` at the user's first prompt, arbitrarily long after the launch, so a live
  environment read would let a session launched under one setting announce under another. The two switches are separate
  because they fail in different directions — no hooks costs identity capture, no pointer costs an agent knowing the CLI
  exists — and because the pointer rides on the hook, every skip that suppresses injection suppresses the pointer with
  it.
- Per-session spawn credential: random token in the session's environment (`FARHELM_SESSION_ID`,
  `FARHELM_SESSION_TOKEN`, socket path), checked by the supervisor on the unix socket. These variables, like the rest of
  what a session holds, stay meaningful to newer binaries (see "What running sessions hold across versions").
- Process-tree ownership (SPEC.md's stop/reap promises): killing the tmux pane is not enough — tmux signals the
  foreground process group, and daemonized descendants escape it. The portable sweep combines the pane's descendant tree
  with environment-marker selection through the platform process API. Every marker selection requires the matching
  `FARHELM_SESSION_ID`. Stop and restart select the agent's `FARHELM_AGENT_ID`; delete selects the whole session,
  including tabs; closing one tab selects its exact `FARHELM_TAB_ID`. Agent selection also retains the existing legacy
  case with neither kind marker. These are process-ownership hints, not authenticated credentials; across versions they
  follow "What running sessions hold across versions", and nothing older than the markers themselves is promised beyond
  the legacy case above. Launch boundaries scrub the opposite kind's marker so nested supervisors do not misclassify a
  new agent as an outer tab's process. On both platforms the marker is read from the memory where the kernel placed the
  environment at exec (`/proc/<pid>/environ` on Linux, `KERN_PROCARGS2` on macOS), which is live: a program that
  rewrites its process title can overwrite it, and SPEC.md accepts that such a detached process escapes the sweep.

  The sweep sends SIGTERM, allows a short grace, quiesces with SIGSTOP, re-enumerates, sends SIGKILL, and polls for
  confirmed disappearance. PID/start-time revalidation narrows reuse races; it does not make the separate identity read
  and signal syscall atomic or guarantee globally unique start timestamps. `systemd-run --user --scope` cgroup scopes
  layer on top as the Linux hardening (M3): where a functional systemd user manager exists — probed by actually running
  a trivial transient scope and then showing, killing, and confirming the collection of it, not by `which`, and through
  absolute binary paths so a login shell's `$PATH` cannot substitute what the probe approved — each launch is wrapped in
  its own generation-named scope (audited on systemd 255: the wrapper execs in place, so the pane's process tree, exit
  codes, and liveness checks see exactly the unwrapped shape), the per-launch SELECTION is recorded durably as a boolean
  while the unit name is re-derived from session id plus generation at every use (a stored name would let a tampered row
  aim a kill at another session's unit), and stop kills through the scope first — SIGTERM, the same grace the sweep
  gives, SIGKILL, then confirming the unit was actually collected, because `systemctl kill` returning only proves
  delivery. The manager is probed once and the answer cached, with two exceptions. A probe that runs out of time (15
  seconds overall, 5 per query) is not an answer, since a busy manager is not an absent one: that launch runs without
  its own scope, and the first launch or teardown at least a minute later probes again. Teardown waits out that minute
  too (decided 2026-09-30): Stop, Restart, Delete and tab close on a session or tab whose scope is recorded (a scoped
  launch row, a tab window marked as opened in a scope, or an unmarked tab of a scoped session) refuse until it has
  passed, because a scope that cannot be checked is an unconfirmed cleanup (below), and the periodic reaping of dead
  tabs warns on each tick for the same reason. Probing again for every such teardown would cost up to the probe's 15
  seconds each while the manager stays slow, which is what the minute exists to avoid; a timed-out probe is rare, and a
  retry after the minute probes again and succeeds once the manager answers. A definite negative stays cached, except
  that teardown with evidence of a scope gets one re-probe: a unit its durable row says was scoped, a tab whose window
  records that it was opened in a scope, an unmarked tab of a scoped session, and an unmarked tab of an unscoped session
  (skipped quietly if the manager is still unusable, since nothing shows it ever had a scope). The sweep ALWAYS runs
  afterwards as the backstop, and is the whole mechanism where no user manager exists — a missing manager never degrades
  stop below the sweep's guarantees. A broken one does not silently pass either: for a scoped launch or tab, a scope
  that cannot be confirmed collected fails the operation even when the sweep found nothing (SPEC.md, Lifecycle
  operations), since the scope is what catches the processes the sweep cannot see. A silent Farhelm waypoint step sits
  between the login shell and wrapper, outside the scope, and execs onward without changing the eventual process tree. A
  wrapper that fails runs before the shim can write its exec-failure sentinel; the generation's waypoint and unconsumed
  spec on an owned dead pane distinguish that **error** from a login shell that exited first.

  Terminal tabs also receive separate scopes, named from the session and tab IDs. A tab has no database row, so whether
  its open selected a scope is recorded as an option on its tmux window, and closing the tab decides from that whether
  an unconfirmable scope fails the close. The session's own launch is no substitute: the agent and a later tab can see
  different verdicts about the user manager. A window opened before that option existed is treated as scoped when the
  session's launch was, and otherwise still gets the one re-check of the manager, its scope skipped only if the manager
  stays unusable. Delete collects those units both from tmux-discovered tabs and independently from the systemd manager
  using the session-specific tab-unit glob. The second source preserves a cleanup handle when tmux no longer supplies
  tab IDs. A manager enumeration failure is distinct from having no usable manager; it can refuse Delete before the
  portable sweep.

  Containment starts at the agent launch, after login-shell initialization: the cgroup wrapper and the shim's
  environment markers deliberately exclude services started by shell startup files. A detached startup-file service may
  belong to the user's ambient login environment and need not be reaped on session teardown. A child that remains in the
  pane's process tree can still be found by the ordinary descendant walk; this is an exclusion from guaranteed cleanup,
  not a promise that every startup-file child survives.

  Terminal tabs are the deliberate opposite. A tab's session and tab markers are set on the tmux window before its login
  shell starts, and its scope wraps that shell, so everything the shell's startup files start is inside the tab's
  containment and is reaped on close, auto-reap, and Delete, subject to the same residuals as the agent's processes (see
  below; on a host without a usable user manager, a detached tab-started process whose environment is unreadable escapes
  the sweep). The tab shell is itself what closing the tab promises to kill, and a tab has no shim that could run after
  initialization to apply containment later. The agent launch has normally already run the same startup files outside
  containment, so guarded startup logic usually reuses that instance; a shared service dies with a tab only when the
  tab's startup files are the first to start it.

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
  Another residual applies to every host without a usable user manager, for agent and tab cleanup alike: a detached
  descendant whose environment its own user cannot read. `ssh-agent` marks itself non-dumpable, which makes
  `/proc/<pid>/environ` unreadable even to its owner (observed, not theorized), and it calls `setsid`; a setuid
  program's environment is unreadable the same way. The sweep treats an unreadable environment as unmarked, so once such
  a process has left the pane's tree nothing claims it. Only cgroup containment closes this; the session-id channel
  would not, because `setsid` gives the process a new session id.

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
  inside a 0700 directory, because reaching it means running commands as the user), `tmux.sock` and `tmux.conf` for the
  private tmux server, `repo-cache/` (host-local repository objects and permanent locks; see "Owned checkout admission
  and lifetime"), `hook-log/<session>.log` (each session's hook diagnostics) and `hook-reports/<session>/` (its waiting
  report files; see "Report files"), and `launch/` holding one 0600 JSON spec per LAUNCH, named
  `<session>.<generation>.json`, plus a `.status` failure sentinel and an empty `.waypoint` proving a scoped command
  reached its wrapper step. The waypoint follows the same generation cleanup and superseded-launch sweep as the spec;
  orphan waypoints are removed too. A launch spec carries the agent's command line and the session credential, but
  nothing the session's own database row does not already hold, in the same private state directory, for the session's
  whole lifetime (the plaintext invocation, and the credential kept so a restart can inject the same one). Removing
  specs is therefore tidiness, not a credential boundary: the shim unlinks its spec as soon as it has read it, creation
  removes it if the session never starts, and the supervisor's startup sweep removes specs of sessions that no longer
  exist and of launches a later generation superseded. A current-generation spec whose launch never reached the shim (a
  login shell that exited in its rc files, a reboot, or a stop before launch) may stay until the session is deleted,
  whichever path observed the outcome (startup reconciliation, the runtime observers, or Stop); Delete removes it.
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
stays with the installation identity.

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

### Composer history

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
replay rather than after each inner edit. The comparison uses effective request permissions, including passive memory
seeds, ordered as Chat/Approve, SmartApprove/Guarded, then YOLO/Custom/own-setting. The existing reconciliation and
remembered-choice rules are unchanged. One transient notice retains the complete pre-switch draft and raw editor
ownership, but never retry intent, connection evidence or cache contents. Undo invalidates browse/preview work and
retires ordinary create/replace authority. An ambiguous managed-checkout attempt still reconciles through its retained
key when the request binding matches; Undo does not rewind or discard that attempt. Notice generations fence queued
Undo, dismiss and expiry callbacks. Undo also checks the live launcher opening generation, and a new opening or prefill
clears the notice, so a mounted Clone/Replace reseed cannot receive another source's saved draft. CSS supplies the
slide, permission outline and a separate visually constant eight-second lifetime animation; hover, focus within the
banner and the existing inactive-window rule pause that lifetime. Reduced motion keeps only the lifetime animation and a
static outline. Removing the notice's controls returns focus through the existing launcher helper.

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
selections shape GUI defaults and suggestions. The launcher has no trust state, segment or search action. Recent setup
grouping ignores historical trust, summaries omit it, and activating a recent clears it before composing a new launch.

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
  confirmation choices) every client shares.
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
  one of six states, and the last three exist because folding them into "unreachable" would throw away the only
  information that makes the situation fixable: **connecting** (active retries in progress), **unreachable-reprobing**
  (the active window is spent, background probes continue forever, no give-up), **connected**, **version-skew** (the
  hello was answered and refused; carries both protocol versions, the peer's build, and the remediation text, since
  SPEC.md demands actionable rather than merely diagnostic errors), **identity-mismatch** (frozen, carrying both
  identities, connecting nothing until the user adopts or fixes the destination), and **duplicate** (this entry's
  identity is already another entry's; frozen like identity-mismatch, connecting nothing until a Retry, an edit of the
  entry, or a helm restart asks again, while the entry stays visible naming the other one). First contact checks whether
  another entry holds the reported identity before comparing it with the one this entry recorded, so an entry that
  recorded one install and now reaches another entry's machine is a duplicate; and an adopt prompt shown before the
  other entry recorded the identity turns into the duplicate state when its adoption is refused, because the refusal
  starts the entry's next attempt. The local row's unreachable state additionally distinguishes "no supervisor is
  running on this machine" from a generic transport failure, because that is the one case whose remedy is a command on
  the machine the user is already sitting at — a manual-path hint, never an offer to install (provisioning is M7's). A
  seventh state exists that is not about the host at all — **retired** — for an entry whose actor has stopped: a
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
  bounds round trips and pane-state queries; report reconciliation stays on the supervisor's timer rather than
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
  `matching` describe rows a client can see and "showing 4 of 5" never appears over a row nobody can render (the
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
  is explicit that list and rename replies compute fresh pane liveness, while create and restart replies' `unknown`
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
- `POST /api/sessions/{id}/replace` (SPEC.md's "replace") is a composition of the create and delete this section already
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
- Host management commits durably first and converges the live actors after, so each verb states how it fails closed:
  add rolls its row back if no actor could be started (a registered host with no actor is invisible and un-dialed, while
  its destination is taken); retarget converges instead of rolling back, because the durable write is what the user
  asked for and the actor can be told to reconnect (not guaranteed: with an unreadable registry, reviving a dead actor
  fails and is reported, and a live actor keeps its old row); remove tears the actor down by the id it just committed,
  needing no registry read that could fail. Retry reports whether it found a host, and a RETIRED host's retry respawns
  its actor from the current row — nothing else ever restarts one, so without that an actor that panicked left its host
  permanently dark. Adopting names the identity the user was shown and is refused if the host has since started
  reporting a different one, because a re-probe between the decision and the request would otherwise adopt something
  nobody approved.
- `--ensure-hosts <file>` is a JSON5 floor under the registry, applied through the same registration path as a REST add
  before serving begins and never consulted again. It adds what is missing and touches nothing else: an already
  registered destination keeps its fields and its learned identity, because helm.db is the durable authority and a
  startup file that overwrote user edits every boot would make the two fight. Validation is all-or-nothing — a malformed
  file, an unusable destination, or a destination listed twice fails startup with the entry named and nothing written,
  since a helm that came up with three of five guaranteed hosts looks healthy and is not.
- axum serving: REST for CRUD (sessions, hosts), a WebSocket event stream for live session-list updates, and a WebSocket
  per attached terminal. A standalone helm also serves the static UI bundle; the desktop's embedded helm uses the same
  API routes with no static fallback, no token-exchange route, and a fresh kernel-selected loopback port. Loopback bind
  enforced — refuses non-loopback per SPEC.md.
- Web token: random 128-bit value minted on the helm's first run and stored recoverably in helm.db so `token show` can
  print it. Browser auth exchanges it once for a random 128-bit device secret returned in the response body; the browser
  keeps that secret in origin-scoped localStorage, whose origin includes the loopback port, and sends it explicitly as a
  Bearer credential on REST requests and a credential-bearing WebSocket subprotocol during upgrades. The helm stores
  only the device secret's SHA-256 hash, and rotation deletes all device credential rows, rejecting their use on new
  requests. Already-admitted HTTP requests may finish. The current implementation also closes browser terminal and
  event-feed sockets on rotation; SPEC.md permits either closing or retaining those existing connections, so preserving
  that behavior is not a reason to add cancellation machinery elsewhere. This deliberately gives up HttpOnly: script
  execution in the authenticated origin can read the secret, but such a script can already drive the same API, while
  port scoping prevents an unrelated loopback service from receiving an ambient host-scoped credential. The loopback
  Origin guard remains defense in depth; no ambient browser credential remains, so this flow has no CSRF edge. The
  `dioxus://` and `wry://` Origin exemption exists only on the embedded helm; a standalone helm has no custom-scheme
  page to authorize and refuses those origins.
- The desktop app's two credentials (one for native REST, one for the webview's page memory and WebSocket subprotocols)
  bypass that exchange. The embedded helm mints them in memory at startup and hands them to the desktop process through
  `run_embedded`'s readiness channel; it keeps only their SHA-256 digests, in memory, and checks them before any stored
  rows. It rejects stored browser rows from the shared state directory, so only the current launch can authenticate.
  They are never device rows, so rotation's delete and the 64-row eviction cannot reach them, and sockets they
  authenticate are not closed by rotation. No HTTP route mints one, and the embedded helm serves no browser UI. They die
  with the process, so nothing persists or accumulates across launches, and the app keeps none on disk. With nothing to
  revoke, the desktop has no re-authentication path: a native 401 is reported as an error, and the window's failure page
  with its Retry button is left for genuinely broken states, such as a webview credential that fails validation or the
  event-socket probe. The page keeps its secret in memory for the life of the window, and removes any legacy
  `farhelm.device-secret` key on each credentialed authentication attempt as best effort. A debug page reload discards
  that page copy; nothing hands the page its credential again, so its terminals, attachment uploads and event feed
  cannot authenticate until the app is relaunched. The shipped release desktop configuration does not expose the debug
  context-menu reload control, so that debug-only behavior is outside the supported product lifetime. Like a browser's,
  the webview's secret is readable by script in the window; unlike a browser's, a stolen one survives rotation and ends
  only when the app quits.
- The loopback guard accepts `Host` and `Origin` only as the IPv4 literal `127.0.0.1:<port>` (bare `127.0.0.1` on port
  80, where browsers omit the default port), or (in embedded mode only) the native webview's custom schemes as Origin.
  That exemption is a scheme prefix and cannot be narrower in a useful way: dioxus-desktop hardcodes the page URL
  `dioxus://index.html/` on Linux and macOS, so every Dioxus desktop app sends the same Origin, and `wry://` is open to
  any app built on wry directly. A standalone helm has no custom-scheme page to authorize and refuses those origins.
  Another such app's content passing the embedded guard is the residual SPEC.md "Client to helm" accepts for this
  browser-facing check. `localhost` and `[::1]` are refused because the helm binds only IPv4 loopback, leaving
  `[::1]:<port>` free for another local account to bind; a `localhost` origin could then be served by that account and
  read whatever the UI stored there. Binding `[::1]` as well was rejected: IPv6 loopback can be enabled after the helm
  starts, so the helm cannot hold that address reliably. A `GET` or `HEAD` without an `Upgrade` header whose Host is
  exactly one of the refused names gets a `307` to the fixed `http://127.0.0.1:<port>/`, built from the literal rather
  than from the request; API calls and WebSocket upgrades under those names get the ordinary 403.
- The native app embeds farhelm-helm in-process; the Linux helm is the same code behind `farhelm helm run`. The local
  supervisor is a separate process either way — the app discovers one that already answers and leaves it alone, or
  starts `farhelm supervisor run` from its sibling binary and owns that child for its own lifetime.
- Schema 29 adds the supervisor session column `launch_hooked` (integer, default 0). New spawns record the argv decision
  before tmux starts; relaunch clears it, and a pending-create retry inserts a fresh unhooked decision before spawn
  decides again. Older supervisors refuse this upgraded database on downgrade; upgrades preserve stored conversations
  and notifications. No hook clock or diagnostic latch is persisted.
- Schema 30 adds the supervisor session column `launch_boot_ended` (integer, default 0), the boot-finality proof
  described under Runtime state. Older supervisors refuse this upgraded database on downgrade.
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
  Codex process pin corrected after a verified recorded ancestor replaces a child-first conversation. The correction
  records the consequence for Restart before adoption and remains unresolved, giving one notification per launch. The
  first two say Restart cannot resume, so they are refused whenever the launch's row holds a captured conversation,
  checked in the same statement as the insert, which is also what closes the race between a report's commit and the
  tripwire. The listing carries the last 10 on `SessionInfo::notifications` as sequence number, time, text and an
  additive `resolved` bool (false omitted and the decode default): the kind and generation stay private to the store and
  supervisor's in-memory notification snapshot, because an enum on `SessionInfo` that a newer supervisor extended would
  make the whole record fail to decode in an older helm, which drops a session it cannot decode. The field is additive
  with a decode default, so an older helm shows no bell and an older supervisor sends none, and needed no protocol bump.
  Replies are built from immutable entries, so the list lives in a session-scoped cell beside the activity time, seeded
  from the store when an entry is built from a row and replaced after each recording or resolution, which then sends the
  `SessionsChanged` hint. Each capture pass resolves the silent-hook and resume-withdrawn kinds only when the generic
  restart offer is Resume, rather than merely when a captured conversation exists. This includes reports admitted before
  startup publishes the entry. An unresolved non-resolving kind or an earlier launch's warning remains history; clearing
  in the helm does not remove it from the store. The in-memory snapshot retains kind and generation beside the wire
  fields, so a pass with no unresolved, resolving kind for the current launch skips the store call entirely. An eligible
  pass uses two indexed updates in one transaction. The resolve SQL compares the exact captured conversation and
  generation on the session row, so an earlier Resume observation cannot erase a later withdrawal. Notification reloads
  order snapshots by newest sequence and then resolved count: with the newest sequence fixed, only resolution changes,
  and it only grows; reopening raises the sequence. Schema 28 adds `resolved_at` on the forward ladder, preserving old
  unresolved history; older supervisors refuse the upgraded database on downgrade, as accepted for this change. The helm
  keeps read and cleared state as two per-session sequence marks ("read through N", "cleared through N") in a
  `session_notification_marks` table beside `session_seen`, keyed and cleaned up the same way, set by the two endpoints
  `PUT /api/sessions/{id}/notifications/read` and `PUT /api/sessions/{id}/notifications/cleared`, each taking
  `{"through": N}`. A mark never moves backwards and is clamped to the newest sequence number the helm's cached row
  holds, so a mark can never cover an entry that arrives later; the fleet-events revision is bumped only when a mark
  moved. Building rows for the UI, the helm keeps at most the 10 newest entries whatever a supervisor sent, drops
  cleared ones, and reports the read mark so the UI can tell unread from read. Clearing deletes nothing in the
  supervisor; the cap ages records out there. The UI renders each text as peer text, marks resolved entries grey with a
  "resolved" word, and excludes them from unread counts and "new" styling without moving the helm's read mark.
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
  session drops its `session_seen` row explicitly, since the viewer row is independent of the cache; a session deleted
  through ANOTHER helm can leave a row behind until its host entry is removed, which is accepted as garbage bounded by
  the number of sessions that ever existed, at a few dozen bytes each.

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

- Shared pins live in the presence-only `session_pins` table, keyed by `(host_id, session_id)` since helm schema 63.
  Like `session_seen`, it cascades on host removal, survives retarget/adoption, is cleared explicitly on deletion, and
  accepts bounded stray rows from other deletion paths without a reaper. `PUT /api/sessions/{id}/pin` requires a
  `{pinned: bool}` body, verifies helm-local ownership even for an unreachable host, and bumps the event revision only
  when membership changes. Listing and detail rows carry `pinned: bool`; the list reads membership in one batch for both
  cached and identity-less live rows. The helm sort stays unchanged for the quick switcher. The sidebar stably
  partitions accepted replies before applying its reorder hold, and computes the separator from the resulting display.
  Replace copies a source pin to the new id before requesting the source's deletion, so a pin write failure leaves the
  source intact; if deletion fails both surviving sessions can be pinned. Confirmed deletion clears the old pin best
  effort, like the other viewer metadata.

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

Protected GET/PUT `/api/ntfy-alerts` read/save an independent schema-47 singleton, not replayed shared preferences. The
JSON stored in helm.db includes the token; the response contains only `token_set`. Saves atomically preserve an
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

On Linux, setup's existing managed-unit parser and removal machinery select services whose recorded executable resolves
to this installation. Custom and other-installation units, drop-ins and linger remain untouched. Services are disabled
and stopped before their unit files or executables are removed. The operation does not examine effective overrides or
processes. Manual shutdown remains an operator prerequisite. Concurrent install, update and setup (and on macOS desktop
startup) are excluded by locks (`crates/farhelm/src/uninstall/locks.rs` and `lock_and_recheck` in uninstall.rs), taken
after the confirmation rather than before it: the installer's locks are directories, which nothing removes when a
process dies, so holding them across the prompt would turn a Ctrl-C there into stale locks. On macOS that is the app
lock; on Linux the install-directory lock earlier installers used, shaped like theirs (a 0700 directory holding only
`pid`), so an installer that finds it refuses while uninstall runs and clears it as stale after a crash. On Linux
uninstall checks no runtime locks: it stops setup's services itself, after which only a process the user started by hand
could hold them, which stopping is already the operator's job.

The confirmation preview is flushed before mutation, and subsequent progress is buffered so an output-pipe failure does
not interrupt removal midway. Filesystem and service failures retain the CLI, report concrete paths and operation
errors, and ask the user to resolve the failure and retry. Tests inject failures at deletion boundaries and run the
actual installer and compiled CLI in private homes; native macOS executes the same filesystem acceptance harness
headlessly in on-demand CI and the release gate.

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

One log source is not native code: `POST /api/client-log` (PLAN_desktop_web_bug_triage.md) lets the desktop webview's
console shim forward its errors and warnings into native tracing under the `webview_console` target, because the webview
is the one layer with neither tracing nor devtools and a dead eval bridge (MT-5) otherwise erases its own evidence. The
route is device-session-authenticated with the desktop-webview CORS layering, and treats the page as a peer, not a
friend: an envelope-shaped body with unknown fields refused, a route body limit, per-field byte caps with the same
bound-and-escape treatment as every other peer string, a shared fixed-window accept budget, and at most one
dropped-count warn per window so the endpoint's own reporting cannot amplify the failure loop it exists to observe. Only
the desktop build ships a sender; browsers have devtools and forward nothing. The desktop app also runs a native
eval-bridge watchdog (`webview_watchdog` target): a 15-second one-shot-eval heartbeat that logs exactly one error line
per continuous outage when the bridge stops answering (the MT-5 class — the shim cannot report a failure of the very
bridge that armed it) and one recovery line if it resumes; log-only by explicit decision, never a reload or an exit.
Both target names are grep contracts: `docs/desktop-web-triage.md` is the triage recipe built on them, and
`scripts/desktop-smoke.sh` asserts the whole pipeline (a marker through the real capture path, for the first launch and
the restarted process alike) plus watchdog silence on every non-skipped run.

Motivation: tracing is the ecosystem standard, and span context is the cheap way to make "logs are available for X" a
property of the architecture instead of a discipline.

## CLI

clap (derive), one multi-call binary named `farhelm`, clean subcommand grammar. The user-facing surface:

- `farhelm helm checkout-config show`, `set-root <path>`, `clear-root`, `set-post-clone <command>` and
  `clear-post-clone` — inspect/edit existing helm checkout settings. Each accepts `--state-dir`; `--host <id>` selects a
  registered host override instead of the global value. An empty post-clone command disables inheritance explicitly. The
  all-hosts folder and command can also be saved in Settings; per-host overrides remain CLI-only. The command neither
  initializes missing state nor migrates an incompatible database, and never creates target directories or runs the
  hook.
- `farhelm helm run` — run the helm (flags: `--port`, `--state-dir`, `--ui-dist`, `--ensure-hosts <file>`,
  `--payload-dir <dir>` (env `FARHELM_HELM_PAYLOAD_DIR`), `--release-base-url <url>` (env `FARHELM_RELEASE_BASE_URL`)).
  The last two select where "add host" provisioning payloads come from — an operator-staged directory (verified not at
  all, D3) or a download source other than the default, this build's own release on get.farhelm.io (D2); `--payload-dir`
  wins if both are given (D18). It takes no session or transport flags: M1's `--ssh`, `--cwd`, `--agent`, `--title`,
  `--remote-farhelm`, and `--remote-state-dir` were dropped with M6's registry (user decision 2026-08-04). A helm drives
  every registered host at once, so a flag naming one of them could only ever have meant the wrong thing; the last two
  live on as per-host registry fields, and creation is `POST /api/sessions`, which is where the host selection belongs.
  A release build compiles its own web UI in (`FARHELM_UI_DIST` at build time); `--ui-dist` still overrides it at
  runtime, and an ordinary developer build with neither serves the API alone.
- `farhelm helm setup [--state-dir DIR] [--port N] [--tmux PATH] [--no-supervisor] [--dry-run]`, and
  `farhelm helm setup --uninstall [--dry-run]` — write, enable, and remove this machine's systemd user units for the
  helm and its supervisor. Linux only (it exits 2 elsewhere, pointing macOS at the desktop app). One state directory is
  resolved from setup's own environment and pinned into both units, so a shell-only `XDG_STATE_HOME` cannot leave the
  two services on different trees, and both installing and uninstalling refuse when the running user manager reads units
  from a different directory than the one this environment selects. The units are rendered from the templates in
  `crates/farhelm-helm/units/`, the same ones remote provisioning fills in, and every file setup writes starts with
  `# managed-by: farhelm helm setup`: setup overwrites or removes only files carrying that marker, and refuses anything
  else rather than replacing it. It never installs tmux — a tmux below the floor, or none at all, is a refusal naming
  what it found. The matching rule on the helm side is that the hosts panel never installs or updates a supervisor on
  the helm's own machine at all: an absent one is answered with "run `farhelm helm setup` here", a unit already running
  that same binary is named as setup's or its author's and left alone, and a supervisor that ANSWERS is discovered and
  registered exactly as a remote one is.
- `farhelm helm token show|rotate` — web-token bootstrap and rotation.
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
version-order refusal, and the target's own published installer restores the main program, Info.plist and forwarder to
that target while retaining the previous Installed and Running folders. The ownership record must remain readable by
supported older installers; changing its representation requires a compatible path, not just a new writer. Tagged stable
installers from v0.23.0 through v0.28.2 accept the `farhelm-app-v2` NUL-separated link record and this replacement.
These are mechanical compatibility properties, not proof that downgrading local databases is safe, and the updater does
not use them to replace withdrawn waiting releases.

The dx-produced bundle went away because a bare binary has nowhere to put a `Resources/` directory, and Dioxus's
`asset!()` files were the only thing that needed one. They are served instead from the UI tree compiled into
`farhelm-helm`, through a `dioxus-desktop` asset handler registered on `/assets/*`, handed to the webview over the
`dioxus://` scheme. The embedded helm does not serve this tree to browsers; standalone helm builds may serve the
compiled-in tree or an explicit `--ui-dist` directory. Registering a handler for a path prefix takes precedence over
dioxus's own filesystem resolver, so there is no bundle-directory fallback at all; the price is that the desktop build's
asset set and the web bundle's must be identical, which `scripts/check-desktop-assets.sh` enforces on every change. The
desktop page declares UTF-8 in its head, and every `text/*` asset response declares `charset=utf-8`. Without these
declarations WebKit can use a locale-dependent legacy encoding, garbling literal UTF-8 in the stylesheet and scripts.

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

### Side-by-side versions inside Farhelm.app

On the Mac the app bundle holds every kept version of the command-line program side by side, so that an update never
replaces a program a running Farhelm still starts. The layout, with `<v>` a release version such as `0.21.0`:

- `Contents/MacOS/farhelm-desktop`, the app's main program, replaced by rename when an update switches versions.
- `Contents/Versions/<v>/farhelm`, one folder per kept version, written completely before anything points at it.
- `Contents/Versions/installed`, the Installed record: one line naming the version the next start of Farhelm uses.
- `Contents/MacOS/farhelm`, the forwarder: a short `sh` script that runs one of the versioned programs.

Two records decide which version runs. Installed belongs to the installer, which may change it while an older Farhelm is
running; this record, the version folders and the forwarder are all written by `install.sh`. Running belongs to Farhelm:
a supervisor whose own program is `…/Contents/Versions/<v>/farhelm` writes `<v>` to `running-version` in its state
directory, beside `supervisor.sock`: one line holding `<v>`, written to a temporary file and renamed into place, once it
holds the right to serve and before it binds its socket. The version comes from the program's path rather than from the
version compiled into it, because the path is what the forwarder and the installer's cleanup can see. A supervisor that
serves from anywhere else (a development build, a Linux install) removes a `running-version` it finds, since it is not
the version that record would name. Recognizing the layout is by path alone
(`crates/farhelm-supervisor/src/app_bundle.rs`) and works the same on every platform; only the Mac installer ever builds
the shape, so elsewhere nothing changes.

Inside that layout the supervisor hands sessions two different programs. Its own launch shim (`internal launch`, run in
each session's tmux window) is its own versioned program, because the launch description it writes is read only by its
own version. A terminal tab's helper (`internal tab-shell`) is its own versioned program too: it only enters the tab's
folder and then replaces itself with the shell, so it never outlives the supervisor that started it. Everything a
session keeps beyond its supervisor's life names the forwarder instead: the hook command lines, the reporter variables,
and the directory put first on the session's `PATH`. Desktop-managed supervisors before this layout ran from
`Contents/MacOS/farhelm`, so sessions they started already hold that path, and putting the forwarder exactly there is
what keeps them working without migration; it must never move. Outside the layout the supervisor uses its own program
for all of these, as it always has.

An update in place, as `install.sh` performs it: the new version's folder is staged privately and renamed into
`Versions/`, then a complete copy of the new `farhelm-desktop` is renamed over the old one, then the icon, the forwarder
(only when its text changed), `Versions/installed` and `Info.plist` the same way, each copied beside its destination
first so the rename is atomic. Stopping after any step leaves an app that launches the old version or the new one, and
the next run finishes the job. Nothing is re-signed (re-signing could rewrite the running program's file in place), and
nothing is quarantined. The bundle is then touched and re-registered with `lsregister -f`, because Spotlight and Launch
Services otherwise keep showing the old version. Version folders other than the new one, the one it replaced, and the
one named by the Running record in the default state directory or in the one the installer's shell's absolute
`XDG_STATE_HOME` names are removed; a Farhelm running with an overridden state directory is protected by the "replaced"
rule for one update only.

The forwarder's contract, which a later installer may only replace by rename and only with a script that forwards every
older invocation the same way: it resolves its own real path (it is reached through `~/.local/bin/farhelm` from a
terminal) to find the bundle; when `FARHELM_SUPERVISOR_SOCK` is set and a `running-version` exists beside that socket,
it runs that version, and otherwise the Installed one; it `exec`s `Contents/Versions/<version>/farhelm` with the
arguments, the caller's environment variables and the standard streams as they are (the shell itself may add `PWD`,
`SHLVL` and `_`), and prints one line and exits non-zero if that version's folder is missing. It never takes or tests
`supervisor.lock`.

The desktop app starts its managed supervisor from `Contents/Versions/<its own compiled version>/farhelm` when its
bundle has a `Contents/Versions/` folder, and refuses with an error naming the missing folder rather than falling back
to its sibling, which in that layout is the forwarder. `FARHELM_DESKTOP_FARHELM` still overrides the choice, and an app
without the folder still uses its sibling.

### The desktop app's updater

The updater is native code in the desktop build of `farhelm-ui` (`desktop/updater.rs`), in the same process as the
component tree, which reaches it through the Dioxus context the way it reaches `WebviewBootstrap`. It has no helm part:
no endpoint, no helm state, nothing a standalone helm or a browser could see. Its network requests are native reqwest
calls, so the webview still never talks to the internet; feedback goes through the helm because it starts in page
JavaScript, which nothing here does.

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
uses instead of fetching its own, so every archive it downloads is checked against signed hashes. `HOME`, `PATH` and
proxy settings stay what the user's own run would see. The installer's interface to the updater is a permanent contract,
part of the site layout's contract ("Verification chain (D3)"): every shipped updater runs future releases' `install.sh`
with exactly `FARHELM_VERSION` and `FARHELM_INSTALL_SUMS_FILE`, and with a checksum file that may list more entries than
the installer needs. A later installer that renames or drops either variable, or rejects extra entries, breaks every
installed app's updates. The install step is one function that takes the downloader and the runner (`install_release`),
which is the seam its tests drive with signed fixtures. Its output goes to the app's log. A Finder-launched app has no
tmux on its `PATH`, so that log shows the installer's tmux advice, harmlessly. Success is judged only by
`Contents/Versions/installed` naming the target version once the installer has exited, never by its exit status alone.
The installer runs in its own process group, with stdin from `/dev/null` and its output written to a file beside the
script rather than to a pipe the app reads, so that quitting the app mid-install neither kills nor signals it: an
installer killed by a broken pipe would skip its own cleanup and leave its lock behind, and every later install,
automatic or by hand, would refuse until someone removed it. The worker copies that file into the app's log once the
installer exits.

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
delivery skipped checking between identical answers. Successful readiness has hidden "update ready" words in the live
region while the red toggle owns the visible outcome. The live-region child is keyed by request so an identical later
phrase still replaces its text-bearing node. The status is a persistently mounted polite live region beside either
version rendering, with bounded, ellipsized text, the existing reduced-motion-aware spinner, and a keyboard-accessible
button for failure dismissal. Hover words and version marks are independent of this visibility state, and the web build
has no updater to mount it.

Restart to update spawns a detached helper in its own process group: a short `/bin/sh` script that waits, bounded at
about a minute, for the app's process to exit, then runs `/usr/bin/open` on the bundle, and gives up without opening
anything if the app has not exited by then. The app then quits the way closing its window does, so the managed
supervisor's stdin tether and its 20-second wait for the state directory cover the handover, and sessions keep running.
The new main program starts its own version's supervisor from its folder, as any launch does; nothing else is needed.
Opening the releases page uses the same external-link path as the Documentation item.

What it trusts over TLS alone is the answer to "which release is the latest". A wrong answer cannot make it run anything
unsigned, but it can withhold updates (by naming an old release), or, by naming one that fails verification, put up the
reinstall notice, whose recovery command trusts get.farhelm.io over TLS alone. Anyone who controls the site can
therefore get updating apps to ask their users to run that site's installer unverified, so the site's own integrity
still matters for installed apps, not only for fresh installs.

## Provisioning

Implemented in the helm over the same system-ssh access: stream the cross-compiled `farhelm` binary (plus a private
static tmux build when the host has no tmux) into `~/.local/lib/farhelm/` through the same `ssh` command every other
step uses (`cat` into a nonce temporary, then a digest check before the rename; not sftp, whose own destination grammar
reads an IPv6 literal or an `ssh://` URI as a different host than ssh does), write user-level systemd units,
`systemctl --user enable --now`, `loginctl enable-linger` as the optional-step (proceed-without-if-privileged per
SPEC.md). Discovery-first: probe for a running supervisor via `farhelm internal stdio` before proposing any of this, and
show the full concrete action list before touching the host for initial setup.

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

The supervisor-unit ownership rule in SPEC.md is enforced twice on remote hosts. The reach check reports whether the
user-unit directory already holds a `farhelm-supervisor.service` whose first line is setup's managed-by marker (the same
test `units::is_managed` applies), and such a host gets the `SetupManaged` outcome every operation refuses on, before
any plan exists, each in its own words (installing or updating there is the host's installer and setup; removing is its
`farhelm uninstall`). The remote unit write repeats the test in the same shell command that renames the new unit into
place, and uninstall's unit removal repeats it in the same shell command as the `rm`, because a retained plan is
confirmed later and setup may have run on the host in between. The local executor branch has no such check: the panel
never installs a unit on the helm's own machine.

UNINSTALL is the third operation on the same machinery: `POST /api/hosts/{id}/uninstall` plans without a body and
consumes the plan's id from a body, like update's route, but the user sees and confirms the same numbered plan as setup,
with the kept-data note outside the action rail. Its actions disable the supervisor unit, remove the unit file, reload
the user manager, stop the unit, remove the lib directory, and finally forget the host. Removing the file before
stopping keeps the supervisor answering until nothing can start it again: up to the unit file's removal the host stays
connected and the ordinary checks apply, and after it a retry may proceed without a connection. Reloading after the file
is gone makes systemd forget the unit's `KillMode=process` and use its default `control-group`: the stop ends the
supervisor and the private tmux server, including any ended sessions' retained panes (this default behavior was verified
on a real user manager when uninstall was built). The same stop action then ends any server still reachable at this
host's private state-directory socket. That covers an inactive supervisor whose process-only policy left tmux alive
before systemd forgot its removed unit. The tmux client is the accepted host executable when available, otherwise the
private lib-directory executable; both it and the socket are frozen in the plan before confirmation. An absent server
skips this cleanup, including a retry after the lib directory was removed; any other tmux failure leaves the host listed
so uninstall can be retried. The socket cleanup reaches the named server even outside the unit's control group; it does
not scan for other processes or servers. The disable command may reload implicitly; the explicit reload after file
removal ensures the same stop behavior on every retry, including one whose preceding stop failed. Only the removals
still outstanding are planned (the unit file's two steps when it exists, the lib directory's when it exists), and each
host command treats work already done as a skip, so a failed run continues from wherever it stopped with no
process-local memory of how far it got, which would not survive a helm restart. Every path comes from the plan, frozen
from the same layout and overrides an install uses, never re-derived at run time.

The checks run at planning and again under the run's lock at confirmation, by planning again and requiring the same
plan, so one piece of code decides every fact the plan rests on. A connected host needs a fresh session list (a live
round trip, never the cache) with no session that has not ended and none with a live tab, a list the supervisor did not
truncate, its supervisor unit running (a connected supervisor whose unit is not running was started some other way, and
removing its lib directory would leave it running from deleted files), and the fresh probe reporting exactly the
recorded identity, a missing one included, since the probe dials the destination again and uninstall must not act on a
machine it has not verified. The unit systemd loaded (`FragmentPath`) must be the planned unit file, so neither "no unit
file left" nor the removal can miss a unit loaded from elsewhere. A host that is not connected is refused when a
supervisor answers there but cannot be used (version skew, an identity problem), and otherwise needs the probe's
positive evidence that no supervisor answers and its unit file already gone; when its lib directory is gone too, the
registered binary is no longer checked, since there is nothing left to protect and the check would strand the retry. The
binary the probe dialed must lie inside the lib directory and the host's state directory must not, compared as canonical
paths the host resolves (`readlink -f`), since the lib directory is removed with `rm -rf` and a state directory
symlinked into it would otherwise go with it; a path the host cannot resolve refuses. Those comparisons cannot see a lib
directory that is itself a symlink, since the binary resolves inside the link's target, while `rm -rf` of the link
removes only the link; so the inspection also reports, per path, whether the path itself is a symlink (`[ -L ]`, which
sees a dangling link too), and planning refuses a symlinked lib directory before any other check of the inspection, the
resolution check included, so that a dangling link gets the refusal that points to Remove rather than "cannot tell where
it leads". The removal command repeats the `-L` test in the same shell as the `rm`, as the unit removal does for setup's
marker, and refuses a lib directory that became a link after confirmation as a failed step. A row with no recorded state
directory uses the supervisor's default as the host's own environment resolves it, `XDG_STATE_HOME` included. A session
started between the confirmation check and the stop is not locked out and may be ended with the unit. The
confirmation-time check is sufficient; nothing requires another check during the removal. Local `farhelm uninstall`
keeps its separate scope and behavior.

The last action deletes the row from inside the run, which already holds the host's provisioning lock and is the host's
own run task, so it cannot go through Remove's entry point, which takes that lock and aborts the host's task. Both share
`ProvisioningService::delete_registered_host`, which takes the cache-write lock, deletes the row, purges the host's
progress, plans and busy marker (aborting the task only for Remove), stops the actor and forgets its lock. The helm
keeps no result for a deleted host, so the confirming client builds its success notice from the plan it already holds.
Uninstall's host actions have only an SSH implementation: the local row is refused at planning, and the direct-local
executor answers them with an error rather than an implementation nothing can reach.

The Hosts header's `update all` button reads the current host snapshot and enqueues the same binding-captured UPDATE
request that each available SSH row's menu would send. The row's permanently mounted provisioning panel retains its own
intent, one-use plan, submission claim, and progress. Rows with no current Update offer are skipped, so an ADD in flight
cannot turn the header click into a delayed update after setup finishes. All eligible rows begin planning without
waiting for one another; a failed plan or run stays on its own row and cannot suppress another host's result. There is
no fleet endpoint or page-wide update result to reconcile against those host-scoped authorities.

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
payloads and unit files are written only when their content differs, so rerunning provisioning converges from wherever
an earlier run stopped. Remote plans report binary upload and installation as separate actions. Upload verifies the
nonce temporary's digest; installation checks it again before the atomic rename. The upload's stall timeout (SPEC.md
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
directory or the systemd user-unit directory, must retain their permissions. If those permissions prevent installation,
report the obstacle rather than changing them. Each plan's `EnsureDirectories` step marks every directory dedicated or
shared: a dedicated one converges on its mode with `install -d -m` (which chmods an existing directory, and that is the
point), while a shared one is handed to `install -d` only when it is missing, and the confirmation text says an existing
one keeps its permissions.

The supervisor unit uses `KillMode=process`. Sessions started through Farhelm belong to the private tmux server that the
supervisor launches, so systemd's default `control-group` policy would kill that server and every session whenever an
explicit UPDATE restarts the supervisor. Limiting the unit stop to its main process preserves the same ownership model
as running `farhelm supervisor run` manually: stopping the supervisor detaches management, while tmux continues to own
the session processes and terminals until the user deletes them, the host reboots, or Farhelm is uninstalled from the
host, which ends the private tmux server on purpose.

The supervisor unit also sets `UnsetEnvironment=FARHELM_SESSION_ID FARHELM_AGENT_ID FARHELM_TAB_ID`. The kill sweep
claims every process carrying a session's markers and has no exemption for the supervisor or its private tmux server. A
tab's startup file that runs a bare `systemctl --user import-environment`, or
`dbus-update-activation-environment --systemd --all`, copies that tab's markers into the user manager; a supervisor
started afterwards would inherit them, and deleting that session or closing that tab would make it signal itself, and a
tmux server it started would take every session's panes with it. A systemd-managed supervisor never legitimately belongs
to a session, so stripping the markers is free. This covers supervisors started through the generated unit (local setup
and provisioning share it); one started by hand from an environment carrying markers, and other user services that
inherited them, are not protected, and the sweep deliberately has no protected-process set.

Motivation for shipping tmux ourselves when absent or too old: apt needs root, and SPEC.md forbids requiring it; a
static tmux under our own lib dir keeps the no-root promise without asking the user to install anything.

The provisioning payloads — linux-musl `farhelm` binaries for both architectures plus the static tmux builds — are no
longer embedded in the helm's own distribution (D2). This REVERSES the earlier "provisioning must work with no
third-party downloads" posture: a release-shaped build (D13 — one that embedded the web UI) downloads them, on demand,
from its own version's release on get.farhelm.io (`https://get.farhelm.io/v<version>/`, whose archive links redirect to
where the archives are hosted, followed over https only), verifies them, and caches them under helm state before pushing
them over SSH exactly as before. A release-shaped build of main, whose version is the development sentinel (see "Version
and skew"), has no release to download from and refuses by default, naming `--payload-dir` and payloads built from the
same commit (`UnreleasedPayloads`). A developer build defaults to no payloads at all (`NoPayloads`, D13) rather than to
a download, and `--payload-dir <dir>` (env `FARHELM_HELM_PAYLOAD_DIR`) selects an operator-staged directory instead —
files in an explicitly selected local directory are treated as operator-trusted and are not verified, on ANY build,
developer or release. The desktop app's embedded helm takes the same directory from the same environment variable, so a
desktop build from a working copy can update hosts with Linux payloads built from that working copy
(`scripts/devbuild-mac.sh`). The downloading source is `ReleasePayloadSource` (`provisioning/release_payloads.rs`),
which caches one release's assets, their extracted binaries, and the signed checksum file under
`<state_dir>/payloads/v{version}-<first 12 hex of sha256(base_url)>/` — keyed by version so an upgraded helm never
reuses the previous release's binaries, and by base URL so a mirror or test server can neither read nor poison what the
real release wrote. Motivation for reversing the embedding: bundling every target's binaries inside every platform's own
artifact cost tens of MB per download and coupled a payload's presence to whichever platform happened to embed it; a
download keeps the size cost where it belongs (paid once, by the host actually being provisioned) while keeping the same
"provisioned host runs exactly what the provisioning helm expects" property, because the default download always names
the provisioning helm's own version.

Verification chain (D3): every release is published on one origin, `https://get.farhelm.io`, in a fixed layout that is a
permanent contract with every client: `/latest` is one line naming the latest stable tag (never a prerelease; 404 before
any release is published); `/<tag>/SHA256SUMS` lists, in `sha256sum` format sorted by name, the six payloads and that
release's `install.sh`; `/<tag>/SHA256SUMS.minisig` is the minisign signature over it, with trusted comment exactly
`farhelm <tag>`; `/<tag>/install.sh` is the installer signed with the release; and `/<tag>/<payload>` for each payload
is a temporary redirect to wherever the archive is hosted (today the GitHub release). `<tag>` is `v` plus the version,
prereleases included. The installer's two-variable interface to the desktop updater is part of this contract (see "The
desktop app's updater"). Installed software trusts a ring of public keys compiled into both shipped binaries
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

Supervisor-side artifacts: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, static, built by cargo-dist on
a native runner per architecture with `musl-tools` for the C half of the link (audited: rusqlite-bundled static musl
builds work for both). Cross-compiling them with cargo-zigbuild, which the retired hand-written release workflow did, is
no longer necessary now that GitHub's arm64 Linux runners are generally available; zig remains the toolchain for the
private tmux builds, which are C and cross-compile cleanly. The Mac artifacts (`aarch64-apple-darwin`: `farhelm` and
`farhelm-desktop`) are built on a macOS runner with the native toolchain — cross-building them from Linux was audited
and rejected: zig cannot link the Apple frameworks wry needs (WebKit, AppKit) without a licensed macOS SDK, and cross
ships no darwin images. Dropping the `.app` bundle removed a third reason (`dx bundle` produces bundles only on the
native platform) but not those two, and it added one: `farhelm-desktop` is built through `dx build`, which patches the
asset names into the linked binary and so has to run where that binary is linked. The macOS runner stays. Signing and
notarization are deferred (D11) and the artifacts ship unsigned, so signing is not among today's reasons — it would
become one if that changes.

Motivation: musl-static sidesteps glibc version skew across Ubuntu releases, which matters because provisioning drops
binaries on machines we do not control; "predicated on whatever cross-compiled binaries are available" in SPEC.md
becomes exactly this target list.

## Testing

The testing story is a first-class requirement — agents verifying GUI behavior without a human is a standing project
constraint (see the GUI section's motivation), not an afterthought:

- **Playwright (TypeScript) drives the web build in headless Chromium** against a real helm and real supervisor on
  Linux. DOM assertions and screenshots both work because the UI is real DOM. This is the canonical GUI verification
  path for agents.
- **A fake agent** — `farhelm-fixtures fake-agent --script basic|altscreen|binary|mouse-modes|spawn|agent-relay`, from
  the `farhelm-fixtures` package, a test-only binary that is never shipped (the product binary carries no fixtures) —
  stands in for Claude Code/Codex across this suite's integration and e2e tests. Its deterministic scripts cover
  prompt/echo input, terminal modes, alternate-screen rendering, byte-clean live output, and mouse-mode reporting,
  without vendor auth. Later milestones extend this fixture with fake on-disk records for status heuristics,
  conversation capture, and resume. The `agent-relay` script goes further than the rest: it reads the `SessionStart`
  hook out of its own launch's injected `--settings` argv, runs it, obeys the pointer line that hook prints by running
  `farhelm agent instructions`, and then serves `$farhelm ...` requests typed into its terminal by running the shipped
  `farhelm agent` verbs. Everything on either side of one link is the real product; the single stand-in is what DECIDES
  a conversation started, which is the part CI cannot have. That script is what makes the whole agent-relay chain
  testable end to end (`e2e/tests/agent-relay.spec.ts`). The spawn suite also has an automated real-Claude leg that
  creates a jj workspace and spawns into it; CI leaves it gated because vendor credentials and network access are
  absent, and a developer enables it manually with `FARHELM_REAL_AGENT=1`.
- Rust integration tests exercise supervisor+tmux directly (CI provides tmux) and the framing protocol with golden
  cases; farhelm-proto keeps wire compatibility testable.
- **`node --test` unit-tests the asset-JS layer's pure functions**, under `crates/farhelm-ui/js-tests/` (outside
  `assets/` for source/test separation — bundling itself is by explicit `asset!` registration, so placement alone
  neither includes nor excludes a file). It currently covers `term-bytes.js`'s byte-domain conversion for
  `term.onBinary` — the byte-for-byte contract pinned at the boundaries (0x00, 0x7f, 0x80, 0xff), empty input, and a
  mouse-report-shaped sequence. Node's built-in runner over vitest/jest: node is already a CI requirement for
  Playwright, so this is zero new dependencies, and the asset-JS layer has no bundler for a module-tooling-heavy runner
  to pay for (PLAN_M6_5.md item 1).
- **A CentOS Stream 9 container stands in for a host that is not the CI runner.** Provisioning accepts any Linux host
  with a usable systemd user manager, but every provisioning integration test dials `localhost`, and both CI and the
  release gate run on Ubuntu — so the case the retired `ID=ubuntu` gate existed to forbid, a helm installing onto a
  different distribution, was the one case nothing covered. `scripts/test-provision-centos.sh` boots
  `quay.io/centos/centos:stream9` with systemd as PID 1, publishes its sshd on loopback, and points the existing ssh
  provisioning test at it through an alias in the user's own ssh config, so the transport, the PAM stack, the user
  manager, and `/etc/os-release` under test are all the container's. What it pushes is what a release publishes: the
  musl-static `farhelm` for `x86_64-unknown-linux-musl` and the pinned static tmux, both put through
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
operational routing; the incompatible protocol case is displayed as `needs update`, or `too new` when the peer's
protocol is the higher one. A development helm (version `0.0.0` with a prerelease, what every build from source reports)
never calls a connected peer newer and never refuses Update as a downgrade, since it sorts below every release; a skew
is judged by protocol versions alone, so a skewed host with the higher protocol reads `too new` there too. Protocol
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
without a desktop asset-parity entry. A nonshrinking settings gear follows the version, outside the macOS drag region;
long versions ellipsize before it or the wordmark shrinks. In a desktop app whose updater is active, the version is also
the update marker: grey as always, or red with a one-character up-arrow before it while Installed is newer than the
running version, and then a button that opens the update menu (Restart to update, What's new). Otherwise the active
updater's readout is a native keyboard-reachable check button calling the same `check_now` as the help menu and host
row. Its dim refresh glyph brightens with the version and turns a quarter on hover or keyboard focus; during a visible
check that glyph owns the spin, so the inline status suppresses its separate ring. The web/no-updater branch stays a
plain span. A verification warning keeps its leading mark and check action. Its hover text carries the updater's state,
so a check the user started ends visibly there. The update menu and the `?` menu are one bar-menu component given
different items.

The bar's hover is selected by a pure function of the window build, any reported helm build and the updater's readout. A
mismatch names both builds; a development window identifies itself as a development build rather than a release. Without
an updater a release window only names its version. A check button's hover leads with its action before the updater's
status, so a reinstall command still ends the text. An idle installed app also explains that the readout turns red once
a newer version is installed and offers the restart menu then. The development-build predicate lives in `farhelm-proto`,
shared by web wording and the helm's native release decisions, so their classification cannot drift.

### What running sessions hold across versions

A newer `farhelm` must accept everything a running or resumable session was handed and will hand back to whatever
`farhelm` it reaches. Sessions outlive the binary that started them: an update replaces the installed `farhelm` while
sessions keep running, and once Farhelm restarts on the new version, the sessions it adopts and the conversations it
resumes reach the new binary with whatever the old one handed them. Nothing re-issues those values, and the protocol
hello does not help, since no handshake stands between a session's command line and the program it ends up running.
(State the supervisor keeps for itself, such as its database, its tmux window options and its scope unit names, follows
its own migration and adoption rules instead.) So the following are a compatibility surface, held to the same care as a
wire format:

- The hook command lines and their flags: `farhelm internal hook --vendor <adapter>`, with or without `--announce`, as
  injected into Claude's `--settings` and Codex's `-c hooks.…` overrides and as the user installs it for Grok (see "The
  per-launch identity hook" under Supervisor internals), and the outer timeouts those declarations carry, which bound
  how long a newer hook, and anything in front of it, may take. Sessions launched before an earlier upgrade still hold
  Claude's or Codex's old five-second limit, which a hook that only writes a file fits easily; only a vendor holding
  stdin open could push one past it. The supervisor's recognition of those shapes counts too: hook attribution for
  Claude, Codex, Grok and OMP matches the reporter's `<farhelm> internal hook …` argv and its `sh -c` trampoline
  syntactically, never by path, so that an upgraded supervisor still accepts an older hook. Those command lines run
  whatever binary is installed when they fire, so an already-running session's hooks start writing report files after an
  update with nothing re-issued; the report file format is versioned, and a supervisor refuses a version it does not
  read (the brief update gap described under "Report files").
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
  written by older supervisors that are still running, so the record's name, place and format stay as they are.

Every path among those values (the hook's program, the reporter variables, the `PATH` entry, the `-e` asset, the
supervisor socket) must keep naming a working `farhelm` or file after an update. An installer, a change of layout, or a
cleanup of old versions may not move or remove one while a session can still reach it.

In practice: add new spellings beside old ones rather than in place of them, never make an optional argument required,
and never give an existing spelling a different meaning. An old spelling may become an alias or a no-op, but it keeps
being accepted; retiring one is a decision to surface, not a cleanup. Anything placed between a session and the binary
it reaches, such as a launcher that picks which installed version to run, passes arguments, environment, stdin, stdout,
stderr and exit status through unchanged, prints nothing of its own when it succeeds, and replaces itself with that
binary (`exec`) rather than running it as a child: hook attribution walks the reporter's process ancestry to the
session's pane, and an extra process in that chain is an unclassified intermediary that gets the report refused.
Reviewers check every change that touches any of the above against this section. The launch description a supervisor
writes for its launch shim (the `LaunchSpec` JSON under `launch/` and its status file) is outside this surface: in the
Mac app's versioned layout the shim is the supervisor's own versioned program, which an update never removes while that
supervisor runs (see "Side-by-side versions inside Farhelm.app"). On a Linux host, provisioning renames a new binary
over the running one before it restarts the supervisor, and a session launched in between runs the new shim against the
old supervisor's description; this section does not cover that window.

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
parsing captured locator provenance. Muse, Cursor and OpenCode get one-time suffixes in the helm only (`resume`,
`--resume`, `--session` respectively); their ordinary `resume` stays absent. Exhaustive harness facts state whether a
named start is offered, whether it takes a session file, and whether it can report afterwards; labels stay in the UI
composer.

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
source and ownership version in the generation-opening transaction because its resume has no reporter. `PriorRun`
retains those columns only for the existing definitive-abort restoration, fenced by the new generation. Success and
ambiguous failures publish a cleared mirror; an abort restores that mirror only if its durable restoration succeeded. No
capture is seeded from request data and no new report admission rule or database schema is needed.

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
`{farhelm_args}`; Muse, Cursor and OpenCode get none. `{farhelm_args}` sits where the previous release appended
Farhelm's arguments, so an upgraded session and a new one spawn alike. `LaunchSelection.wrapper_prefix` stores optional
literal words before both composed commands, including Goose's leading `env` words. The field is omitted when absent;
the protocol bump refuses older peers rather than translating the new launch shape. OMP attribution skips exactly that
many words before classifying the agent program, without relaxing its process proof. At spawn the supervisor fills
`{cwd}` (and the compiler's Codex trust markers `{codex:trusted-cwd}` and `{codex:untrusted-cwd}`, which predate launch
kinds and are filled in any launch that contains one, a command launch included, so a command copied from a composed
Codex launch keeps working; SPEC.md states this as the one exception to a command launch running as written), then
replaces `{farhelm_args}` with the declared kind's own arguments (`AgentIntegration::farhelm_args`, asked for a start or
a resume and never shown the command) and puts that kind's reporter settings in `LaunchSpec.env`, which the shim sets on
the agent process alone, after scrubbing inherited reporter variables. Goose is the one kind whose integration the
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
read finds a binding. Both resolve current template definitions as one all-or-none request, so a renamed or deleted name
refuses the sequence. Dispatch takes a shared keyboard-context and modal-opening-epoch snapshot before and after that
read check, so a dialog that appears during the read cannot receive its result after closing. It hands the captured
definitions to ordinary New, replaying one through the existing template/search acceptance path per effect pass. A
refusal stops the remaining replay. Automatic submission waits for a render after the final replay, so its captured
readiness and refusal describe the resulting draft. A coarse fold over launch kind, agent and command decides whether to
attempt direct launch; the mounted form retains full validation, catalog, host and admission authority. That fold lives
in the protocol crate and also judges a single template for ⚡ marking, its hints, and agent writes. Only the active
kind counts: Agent needs an agent, Command a nonblank command, and omitted kind starts on Agent.

The same dispatch serializes fresh reads when shortcuts arrive in a visible New, Clone or Replace with launcher. Each
press or completed sequence becomes its own captured batch in a form-owned FIFO, fenced by the opening generation and
the shared modal context and epoch. The form validates the whole batch against one draft snapshot, then replays its
templates synchronously through the same search acceptance and `template_edits` path. A refusal drops only that batch
before it changes the draft, leaving later presses queued. Shortcut batches do not enter the opening replay's loading or
auto-submit predicates, and their acceptance keeps the query and focus in place. If a template unmounts the focused
control, a deferred check restores the launcher's normal search target only while that same launcher is still the sole
visible modal. Busy launch and template-save states consume and drop a press.

The shared template-shortcut asset owns the synchronous armed flag, a pure key classifier and the context rule. Rust
holds the ordered references in app-owned transient context; the sidebar's hidden relays mutate that signal
synchronously, and the main area renders its non-modal status pill, using "apply" while the launcher is open. No storage
or general keymap is involved. CSS centers the amber pill in the main area, using a fixed 9px downward correction for an
open Mac session's taller top bar. Its launcher context keeps the old bottom placement. Existing warning tokens provide
text, border, fill and a soft halo; a single 200ms fade/scale entrance is disabled for reduced motion. Prefix and number
modifiers follow the build's Mac-desktop choice; the bolt keeps its existing operating-system chord, including Cmd+P in
a Mac browser. The shared listener installs after the composition guard and before other command listeners, consuming
with stopImmediatePropagation because same-window capture listeners otherwise still run. Pointer/click, blur,
visibility, modal focus and the existing modal-opening hook cancel both state owners. A launcher-owned text field admits
the prefix only without selected text; other editors keep their usual key action. Hidden quick-launch preparation
consumes recognized keys without enqueuing a relay. The relays stay mounted and enabled across render-delayed armed
state. The indicator publishes its raw binding projection beside the visible names in the same render, retaining a
hidden projection while idle for rapid input before the first pill render. Physical Enter copies that DOM projection
into the existing press snapshot; the worker pairs it with the raw reference queue before starting the existing
generation-fenced quick-launch read. Starting the prefix refreshes shared bindings once; a cancelled or superseded
sequence cannot publish its late preference read. Enter never refreshes the captured bindings. Missing captured names
reveal New with its existing named-template refusal, including within multi-template sequences.

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
refuses it at hello instead. The normal update path remains available. A host is named by its recorded install identity.
Unknown fields are refused on decode, so a misspelled field is an error rather than a silently ignored edit. The retired
`workspace_trust` field remains readable in `TemplateFields`, but application ignores it and every save drops it. The
editor clears it in its draft and on delete undo; Save as template never offers it. This keeps old records usable
without letting their trust choices bypass the agent trust section in Settings. The GUI's writes go through
`PUT /api/templates/{name}` (create or replace, last write wins, no version check, as SPEC.md wants) and
`DELETE /api/templates/{name}`; `GET /api/templates` lists them by name. The helm checks only the shape of a template
the GUI writes (a non-empty name of at most 128 bytes with no surrounding spaces or control characters, and fields
within the 64 KiB a create is held to); whether its fields apply is decided only when it is applied. An agent's template
writes share the same store path but carry a precondition and a few refusals of their own (see "Permission prompts for
agent actions").

An agent's quick-launch and number-key edits travel separately from template fields in protocol 60: those bindings
belong to fleet preferences, not to a template's composable choices. The helm and Templates editor share the ⚡
eligibility check. An agent write that changes a binding uses `TemplateWriteWithShortcuts`, a distinct approval kind
whose card shows each change and the previous template for a move. An older UI cannot decode that kind and therefore
cannot approve a move it cannot show. As with `LaunchWithPrompt` below, its approval-list decoder fails the whole list
while such a card waits and keeps showing its last successfully decoded list; this is the accepted older-UI behavior,
consistent with SPEC.md's "Upgrade compatibility and client scale" update rule rather than a supported downgrade path.
At approval, the store compares the named template and the entire ⚡/number-key snapshot from the card under one
immediate transaction, then writes the template and both binding columns together. The GUI's template and preference
writes keep their existing last-write-wins behavior.

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
offer, a host the dialog holds fixed (Replace with), and a host no known install has (a host row in the
identity-mismatch phase does not count, exactly as for the create default); any refusal leaves the launcher unchanged.

The launcher's "save as template" panel shares the snapshot reader used for application, including raw relayed text,
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

The launcher's initial prompt is a separate optional draft signal. The agent tab's template snapshot retains it when the
agent is unset or supported; a request requires a selected supported agent. Recent setups leave the draft alone and
Clone/Replace prefills clear it. Templates replay the shared application result, preserving an empty prompt edit as
distinct from absence. Permission-switch Undo captures it with the other draft values. The full raw prompt is bound to
the create key and fresh-checkout retry identity, re-read after asynchronous key minting, and sent by the same API body
builder for ordinary, named, fresh-checkout and Replace-with creates. It never enters `LaunchSelection`, `SessionLaunch`
or recent history. Restart controls omit the prompt callback.

In the launcher, `tl:name` offers templates (the only search scope that does). Accepting one first runs `apply_template`
on a snapshot of the launcher, so a refusal applies nothing; then it replays the template as the launcher's own search
actions (agent type, model, effort, permissions, host, folder, repository, name) and sets directly only what has no such
action (the launch kind, the command fields and the command launch's declared agent type, the approve, smart approve and
chat permissions, and a choice reset to its default). That replay is what makes remembered defaults, recent setups and a
pending checkout preview react exactly as they do to the same edits by hand, with no provenance tracking for templates.
The Templates dialog beside New has a bounded list column and a selected-template editor; at phone width only the list
or editor is shown, with a back link. A draft holds the wire fields directly, so absence means no visible field, while
explicit null resets and false approval assertions survive editing. Add/remove actions change presence; defaults are
values inside the controls. The editor's agent/command/don't-switch selector writes the kind, with don't-switch offering
only placement and name. A kindless stored template is inferred in the draft only, marked unsaved against its original
baseline; saving explicitly stores that switch. Per-harness choices use the shared capability answers and release
catalog, not another agent table. Model text is unrestricted and suggestions use the existing catalog filter; other
incompatible choices stay visible and refuse save until corrected or removed. Fields need not form a complete launch,
because another stacked template can supply what they omit.

Opening another row, new, duplicate, mobile back, close and Escape share a pending departure and the same inline
save/discard/keep-editing prompt. Save keeps the selected editor and refreshes its baseline; duplicate opens an unsaved
copy with a free name. A rename saves the new name before deleting the old one, so a failure in between leaves both;
saving under a name another template already has is refused in the panel rather than overwriting that template. A
pending or failed name-list read cannot prove a new name is free. Delete checks the stored snapshot, not the unsaved
draft, against the existing template shape rules. If it fails, the control says "delete permanently" and deletion clears
any Undo; the ordinary save path could not restore that snapshot. Otherwise Delete retains the stored template
client-side for about ten seconds; undo freshly reads the names before restoring and refuses a taken name. The notice
and its timer belong to this mounted dialog, with a generation guarding against an earlier timer expiring a later
deletion. Writes are serialized and disable navigation until their outcome is known. API last-write-wins semantics
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
current `SessionInfo` and an attachment snapshot across every terminal belonging to that session, including views on
another helm connection. The helm's channel registry cannot answer that question. The card renders the full message
through the same escaped, scrollable peer block as an initial prompt and reuses the session list's permission-shield
classification. The requesting and target connections are revalidated after approval; this path never composes a launch
or applies the launch-only YOLO guard.

`PromptSession` is an accepted supervisor-owned mutation, so losing the requesting connection does not cancel a paste
between its opening marker and submit. It takes the target lifecycle claim through the final confirmed send, reads its
stored activity synchronously, and requires a live owned agent pane reading Running or Idle. No activity leaf lock or
attachment lock survives an await, and it never takes the target's agent-request fence: crossing prompt requests must
not wait for each other's relay fences. The existing input client opens on the exact session and pane, without taking
over a viewer, and sends one bracketed paste then the per-kind submit bytes as two confirmed sends. Its internal bounded
chunking preserves one opening and one closing paste marker around the whole message. A one-shot session-paired tmux
query requires `bracket_paste_flag` to be exactly on; a missing, off or malformed flag refuses delivery. Claude uses CR
and Codex Tab, at both accepted statuses. A later native delivery route belongs behind that same per-kind strategy seam.
There is no Farhelm prompt queue, receipt store or idempotency key. A lost or failed reply may follow partial or
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
delay from arming buttons after a more recent switch. Existing decision rows are regrouped into requester, short facts,
and full-width text, preserving the peer-text distinction and showing real commands in full. Session-reference rows
reuse `peer::display_identity` and the existing full-width identity layout, with a Copy button calling the existing
clipboard writer on the unmodified full reference. Escaped display text is never copied or used as an action target; the
card's answer handlers retain its random approval ID.

The approval region stays beside the shell so modal isolation leaves it live. While mounted, its small geometry observer
follows the actual main-pane rectangle and the terminal area's top, falling back to the top row's bottom when no
terminal surface is shown. Anchoring below the conditional bands and tunnel strip keeps approval cards from covering
notices or tunnel chips. This accounts for sidebar width, narrow-window shell scrolling, native top chrome, and
conditional notices without fixed offsets. Width is capped at 660px with 12px margins; the whole region's height is
capped at 55% of the pane and at the space remaining below its top. Below 44px of available height it instead uses the
whole viewport, keeping answer controls scrollable even when session chrome exhausts the pane. Pane, sidebar and chrome
sizes and direct chrome structure are observed, not terminal output mutations; listeners and observers are released when
the region leaves the DOM. Sidebar size is an input even at the pane's minimum width, where a resize moves the pane
without changing its size.

"A GUI is connected" means at least one subscriber to the fleet invalidation feed, which every GUI holds open for its
whole life and which the cards rely on to appear without a reload. The desktop app's process ends when its window closes
(dioxus-desktop's default, which Farhelm keeps), so a closed app holds no subscription. A browser whose feed socket
failed and fell back to polling does not count, and its requests are refused as having no window to ask in; that is the
accepted cost of not adding a second presence signal.

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

The "run farhelm commands from this host without asking" setting is `hosts.commands_without_asking` in `helm.db` (schema
40), off by default, and reset to off whenever the row adopts a different install, exactly as the YOLO setting is.
"Always allow" on a card stores it only if the card's connection is still live, and stores it only while the host row
still records the install identity it had when the request arrived (`allow_commands_for_identity`, under the host write
lock), so a card raised by a replaced installation cannot grant its successor the setting. The setting and the request
can race: when the request expires between the setting being stored and the request being taken, the answer gets 410 and
the setting stays on, and the GUI's notice says the answer did not take effect and, after an "Always allow", that the
host's setting may still have been turned on.

Link requests use the same approval table, presence check, wait and answers, with `ApprovalAction::OpenUrl` carrying the
full URL, its local-network category and whether the card is preapproved. `browser::permission_for_url` applies the
terminal link predicate's raw-string exclusions before WHATWG parsing, with a 4096-byte cap. It keeps the original
string; the parsed host is used only for classification. No DNS lookup is performed. The two default-off schema-45
columns, `hosts.links_without_asking` and `hosts.local_links_without_asking`, are independent of command authority and
each other. Their settings routes accept an explicit `allowed` boolean. Always allow selects only the card's category
through `allow_browser_for_identity`, using the live connection and identity preconditions of command grants; adoption
clears both columns.

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
reconnect sets it explicitly. Actual link requests still require the ordinary attached helm. No schema or per-session
placement field is needed: every next launch and new tab reads the current policy.

`LaunchSpec::browser_command` is optional with a serde default for private specs already written on disk. It carries the
absolute `session_farhelm_exe` plus `agent open-url -- %s`, separate from harness reporter environment. The shim adds it
only in final agent assembly after login-shell startup, preserving any inherited BROWSER, even empty; git and the
post-clone command use the common child assembly and get no fallback. Tabs pass it through tmux `-e` without a PATH
change; explicit launch seams and the supervisor's inherited BROWSER win, then shell startup can override it. Tests pass
inherited browser state explicitly rather than editing or depending on their process environment.

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
Conflict when held. This prevents two sessions' crossing deletes from each waiting on the other's retained relay fence,
including requests carried by different helms. UI deletion and Replace keep their ordinary waiting behavior. Self-delete
is refused at both relay and helm before destructive dispatch, because the relay holds that same fence for its own
request and deletion would revoke its credential before returning.

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
WebKit render has no distinct dark margin: pane, terminal and xterm viewport all paint `--terminal-bg`. With coloring
enabled, a 4px inset shadow fills the terminal element's existing padding. The shadow leaves the terminal background and
xterm theme unchanged and adds no layout space; the setting-off render has no shadow. A host appearance refresh recolors
an open session without fetching hosts from the session view, and a Settings write updates this client's preference
signal immediately.
