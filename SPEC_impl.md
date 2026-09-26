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
  `cargo
  build`/`test`/`clippy` compiles WebKit. `-p farhelm-desktop` is consequently the only thing that compiles it.

Motivation: the proto crate is the seam that keeps helm and supervisor honestly decoupled (they meet only over the wire,
even in-process). `farhelm` remains the single multi-call artifact provisioning ever has to move — farhelm-desktop is a
second release binary for the Mac desktop alone, never provisioned to a host.

## GUI: Dioxus

Dioxus, version pinned at the workspace level, rendering the same component tree in two targets: web (wasm32, real DOM,
served by the helm) and desktop (wry webview wrapping the identical DOM). No dioxus-fullstack / server functions — the
UI is a pure client of the helm's HTTP/WS API.

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

The session list's chosen ORDER, last-selected session, compact-row choice, and the launch composer's remembered
permissions and workspace-trust choices are one preference the HELM keeps, in a singleton row of `helm.db`
(`preferences`: `list_sort`, `last_selected`, `compact`, `remembered_permissions`, `remembered_workspace_trust`) behind
`GET`/`PUT /api/preferences`, device-authenticated like every other route. The two remembered launch fields are written
by the helm after a successful user structured launch; no shipped client PUTs them. Workspace trust changes only after
an explicit Codex, Muse, or Pi choice. An agent-originated create and an unsupported harness leave it alone. This makes
the remembered values facts of accepted launches rather than claims from one client. Both clients read the row once
after authentication — `PreferencesGate` holds the authenticated tree, rendering nothing, until the read lands, so the
sort control and the auto-select effect see the remembered values on their first run and no frame shows a default that
is then corrected. On desktop the IPC authentication gate already holds the tree and the read is one loopback hop, so
nothing is visible; in the browser the first paint deliberately waits on that one round trip to the helm — a page with a
valid credential used to paint its sidebar synchronously from localStorage — so the list never appears in an order that
then changes. A write is a sparse patch naming only the field the user changed, merged per-field by the helm (an absent
field is untouched, an explicit `null` clears one), so two clients changing different fields at nearly the same time
cannot clobber each other; the signal in the page is updated before the request leaves, which is what keeps the choice
in force when the write fails. Same-field writes are serialized latest-wins in the client, so a burst of changes cannot
land on the helm in reverse order; the write queue is process state outside the remounted tree, and after credential
recovery the gate overlays and replays any local choice whose write never got through, so reauthentication cannot roll
the current client back to the helm's older row. The seed read runs under a seconds-scale deadline of its own and expiry
reads as "nothing remembered", so a stalled preference endpoint cannot blank the page for the funnel's full sixty
seconds. The sort travels as the bare word `?sort=` takes and is validated against that vocabulary at the write; the
selection is a bare session id (the browser's old `{helm, id}` record was keyed by helm identity only because
origin-scoped storage could outlive a state-directory swap, and a row in the helm's own database cannot describe another
helm's fleet). An absent or unrecognized sort word still reads as the UI default (`activity`) on the client, because the
row outlives the build that validated it. Nothing is kept per client: no localStorage key, no field in
`desktop-client.json` (which now holds credentials only), no eval round trip. The visible consequences are the ones
SPEC.md names — one answer shared by every client, and a second client attaching to whatever was selected most recently
anywhere.

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
"Short" is one predicate with three readers rather than a rule each place restates: it is exactly the condition the
count banner prints "showing N of M" for, the same answer decides whether an absence may be read as a departure
(otherwise the missing row's optimistic rename is retired, its editor is left usable, and — if it is the selected one —
its pane is left alone), and the same answer decides whether a remembered selection missing from the list is resolved
directly rather than treated as gone. A complete authoritative absence changes a ListView-owned rename editor to an
unavailable-target state instead of closing it, preserving its draft for copying or cancellation; seeing the exact
source again clears only that state. A UI that tells the user its list is incomplete and then reasons as though it were
complete would be disagreeing with the one line whose job is to be believed.

One consequence is worth recording because nothing on screen shows it: the auto-select fallback (SPEC.md's
"newest-created session", for a client with no remembered selection) cannot be assumed to be the first row of the
listing, because the first row is whatever the chosen order put there. It picks by the session's `created_at` instead,
which is why that field is decoded by the UI at all, and treats a missing stamp (an older helm) as unknown rather than
as 1970 — a fleet with no stamps degrades to the listing's own first row. It picks from the rows in hand even when the
cap cut the listing, where under a non-creation order the newest session may sit past the cut: a cut listing is a fleet
of hundreds, the fallback exists to keep the pane from sitting empty rather than to be exact, and the alternative was a
second request shape for that corner.

The sidebar uses an identity line and, outside compact mode, a host/directory line plus a detail line when there is
ended status or a stale qualifier to explain. Its identity line is status, locality, title, a one- or two-glyph agent
badge, and a right-aligned activity-time column before the narrow menu gutter. The host line is the host name and
directory, joined by `:` when the helm supplied a name. This restores host visibility for confirmed local sessions too:
a host name is an identity fact, while the locality icon answers a separate question. The status and locality tracks
keep fixed icon-sized widths: live status uses the first slot's dot, compact ended status replaces that dot with a
distinct stopped, exit, interrupted, or error glyph, and an absent status or unknown locality leaves its slot blank.
Compact rows therefore remain one visual line; stale survives there as a small labelled glyph. The full status,
annotation, exit code, and qualifier meaning remain in accessible text and tooltips, and ended glyphs never acquire the
live dot's mark-read action. Noncompact ended details and qualifier words occupy their own full-width line under the
title through activity and above host/directory. Detail wraps unbroken peer text at any boundary without ellipsis,
clamping, or widening the menu gutter. The activity track has a four-character minimum and grows for unbounded ages such
as `1000d`. Agent glyphs are max-content rather than a text-badge allowance: declared structured launch metadata is
authoritative, while a legacy row receives only conservative shell-word executable/flag recognition; a profile name is
not proof of either. C/M/L/G/P are Farhelm letter paths for Codex, Muse, Claude, Goose, and Pi; OpenCode uses its
attributed inline mark, and an unknown command uses the neutral terminal glyph. A legacy row with no name leaves that
fact absent. `list::shared::session_locality` decides among three answers rather than two — `Local` when the session's
host id matches the registry's `HostKind::Local` row (never by name; see that function's own doc for why), `Remote` when
both ids are known and differ, and `Unknown` when either is missing (an old helm sending no host id, or a hosts read
that has not landed). A confirmed local glyph uses the semantic red caution color, including selected, stale, and
compact rows, to keep local execution conspicuous. The row draws the LOCAL glyph only for a confirmed `Local` verdict —
an `Unknown` row draws no glyph at all, never the local one, because a glyph is a positive claim `session_locality` has
no evidence to back. The 2026-08-23 rule's weaker promise survives underneath: unknown locality still never SUPPRESSES
an available host label, it only ever leaves the row free to show one it already has, and the glyph rule adds a second
promise on top rather than replacing the first. Legacy rows without a host name at all necessarily show none regardless
— locality answers whether a name would be shown, not whether one exists to show. The agent track is rendered as glyphs:
structured launch metadata decides the harness and permission mark when present, otherwise conservative recognition uses
the program basename plus a permission glyph. Legacy recognition skips known option values and stops at unknown syntax,
subcommands, or `--`, so argument data cannot earn a permission glyph. The closed approval glyph distinguishes Goose's
`approve`, `smart approve`, and `chat` metadata from the open YOLO warning; an omitted Pi permission is rendered as YOLO
for compatibility with older snapshots, while an omitted OMP permission is rendered as an honest absence — OMP has no
rewrite-to-default rule for the raw-invocation marker to inherit. The full invocation and a profile's snapshotted name
remain in its accessible text and tooltip. The working directory is tilde-folded against the `/home/<user>` and
`/Users/<user>` shapes, since no home directory is on the wire to fold against properly. Every one of those
abbreviations is lossy, so the untouched string rides along in a `title` attribute — the row is a summary, and the full
truth stays one hover away.

Each live dot carries its status word on the dot itself, with the optional mark read / mark unread action following it.
The agent and permission SVGs sit in separate `title` targets, so hovering the open lock explains its permission mode
instead of returning only the combined agent summary. The combined summary remains on the agent track for provenance and
the full invocation.

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
rows, hosts, profiles and the reply envelopes), because those tolerate words a newer helm may send to a browser tab
still running older code; shared golden files under `crates/farhelm-helm/http-contract/`, serialized by the helm's tests
and decoded by the UI's, keep the two sides from drifting. That rule governs the displayed age and the seen/unseen
comparison only — the helm orders an activity-sorted list by reported status first and the work-start key inside each
group, so the age column is deliberately not a rank column: a row above another can show an older age, and that is the
contract rather than a contradiction. A zero means "this helm predates the field" and renders no age at all rather than
an age counted from 1970. The viewer's end of the subtraction can go missing too — a platform clock that will not
answer, or one sitting at or before the epoch — and that is carried as an absent value rather than as a zero, because
subtracting a good host stamp from a zero "now" would clamp every session in the fleet to `now` and paint a dormant
fleet as a busy one.

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
actions, and any PRESSED disclosure control. Action buttons use three deliberate tiers on the shared ghost `.btn` base:
`.btn-primary` is the normal blue affirmative, `.btn-neutral` is the quiet secondary treatment, and `.btn-danger` is
reserved for destructive confirmations. Pressed disclosures use `--accent-fill-hover` so an open trigger is distinct
from a resting primary. The normal-primary entry is scoped per SURFACE, not per screen: the sidebar's resting chrome
carries exactly one filled control (`new session`), and each dialog or popup that floats over it may supply its own
affirmative primary. The sidebar's secondary actions and profile-row edit/delete controls use the neutral tier; menu
items, tabs, composer selections, relays, and other explicit exemptions retain their ghost or purpose-built styling.
Destructive menu items remain red text, while their confirmation buttons use the danger tier. SPEC.md requires the
sidebar to mark the selected session's row readably at a glance, so anything joining that list has to be a place where
the accent means "this is where you are" — the same thing the other entries say — because an accent spread across
ordinary decoration would leave nothing to make the selection readable. Both constraints have a contrast floor under
them: the quiet foreground tokens are set so that metadata stays at WCAG AA against the brightest surface it lands on,
which is what caps how light the selected row's fill may go.

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

The open session's chrome is ONE header row ordered status, title, age, copyable `{cwd}`, copyable invocation, then
Restart, Restart with, Replace, Clone, and Replace with — sized at about 40px, with the tab strip beneath it and nothing
else in the steady state. It used to be four stacked bands costing roughly 170px before the terminal started, on a
surface whose entire point is the terminal. Two of those bands had to go somewhere rather than merely shrink. The
restart offer's explanation became the restart button's tooltip and its `aria-describedby` target: SPEC.md's "restart
says so and offers that same fallback or a fresh launch" is carried by the button's accessible name (`aria-label` and,
alongside the further elaboration, `title`) — naming the offer (`resume conversation`, `restart (fresh launch)`,
`restart with the configured resume command`) rather than the action. The directory and invocation buttons carry their
full values in `title`, shrink before the session title, and reveal a clipboard affordance on hover or keyboard focus.
The five action buttons remain fully visible and in DOM order from a 580px main pane. The app's 320px main-pane floor is
unchanged; between those widths the row may clip its trailing actions rather than wrapping or hiding them. The restart
confirmation became a popover anchored under the button that opened it, still confirm-in-place with focus on cancel; the
consequence sentence they lead with is the one line standing between a click and a killed process tree, and a header
that kept it in flow would have to either wrap or truncate it. Header Replace has a separate anchored confirmation state
so it cannot accidentally open the interrupted card's confirmation. Everything conditional — a refused restart's prose,
the host-unreachable notice and its last-known-status band, the "helm stopped listing this session" line — is still a
full-width band, because a band that only appears when it has something to say costs the steady state nothing. A
classified status renders in at most one place: the header normally, the stale notice's own metadata band for a stale
session (where SPEC.md's title/directory/last-known-status triple is assembled), and nowhere at all for a session
nothing has classified yet.

Restart with uses a separate dialog because it relaunches the current session rather than creating one. It renders the
same `LaunchControls` component as the session launcher, with the harness fixed to the session's stored structured
selection. The dialog owns its draft and comparison baseline; the launcher keeps its own create-only state and effects.
Only edited fields get changed markers; a marker's old model is a stored string and renders as an escaped,
direction-isolated peer value. The dialog's submit uses Restart's stop-first consent and handles the reply through the
same terminal reattachment path as an ordinary restart. That path reattaches the terminal even when the restart is
refused and the dialog stays open, beneath a modal whose keystrokes must never reach the agent.

The dialog owns keyboard focus structurally. While it is mounted, every sibling of every element on its path up to
`body` is `inert`, including siblings rendered after it opened, so no other code can focus anything behind it and Tab
from `body` can only reach the dialog. Only elements that were not already inert are marked, and exactly those are
restored when it closes, before focus returns to the header action. A capture-phase keydown handler covers focus that
still ends up outside the dialog (a click on the scrim leaves it on `body`): it swallows that key and puts focus back on
the dialog, and Escape still cancels unless a request is in flight. The earlier per-mechanism guards stay as a second
layer for engines without `inert`: a terminal whose output becomes visible, or that becomes the selected terminal (as
when the selected tab exits and the view falls back to the agent), does not take focus while the dialog is mounted, and
during a request the primary action stays focusable (unavailable through `aria-disabled`) while the other controls are
disabled, because a focused control that is natively disabled or unmounted drops focus to `body`.

The session-list profiles popup has one explicit focus request at a time. Opening lands on `new profile`; opening an
editor lands on its name field; closing a form returns to its row's edit control or to `new profile`; opening a delete
prompt lands on cancel; and completing a save or delete chooses the surviving row control described by the catalog
transition. Escape and layout invalidation close the popup and restore its toggle. Focus-out instead preserves the
outside destination the user chose. Document/body focus after an internal control replacement is transit, not an outside
destination, even when the bounded replacement-focus request cannot place focus. The confirmation stays mounted and
reachable; a recorded trusted outside pointer or Tab choice still dismisses it. A page operation may defer either
dismissal while it keeps the popup mounted, but it never consumes the obligation: the popup closes once the operation is
idle if focus or layout still requires it. A catalog refresh patches unchanged keyed profile rows in place, so it does
not replay focus after locally absorbed mutations. A terminal whose retained output becomes visible while the popup is
mounted does not take focus. Closing the popup does not hand focus to that terminal; the user can click it when they
want to type there.

A trusted outside pointer or Tab destination supersedes pending opening and completion focus. The popup DOM node records
that choice synchronously, so even a focus commit already sent across the renderer bridge must yield before moving
focus. The Rust request worker observes the same obligation; internal form transitions still express newer in-popup
intent. Unknown classification and unowned body-focus transit supply no dismissal evidence and leave the obligation
pending. Escape also reaches the current popup while failed placement leaves focus on body; it does not move focus from
an unrelated outside control. A subsequent outside focus event or window focus return reconsiders it with a new
observation revision once its current observation has finished, preserving the original intent's identity and
provenance. Notifications during that observation coalesce into one queued recheck if it returns Unknown; old classifier
completions cannot clear a newer obligation. There is no outer timer retry chain; each reconsideration retains the
existing bounded classification and pending-focus settlement.

Hosts use one permanently mounted list beside the session list, not a compact summary plus a second management panel.
Its one-row header gives the known host count, an unpersisted global details checkbox, and the secondary add control.
Every row always shows its name, phase dot, and muted actions toggle in the same narrow trailing gutter as the
session-row actions toggle; connected spends no visible word unless the helm marks a compatible older build, in which
case the amber `old version` advisory is shown. Other phases use humanized prose and retain the stable wire token in
their data attribute. A protocol-incompatible supervisor remains the red `needs update` case; an unparseable build
leaves a connected host's age unknown and keeps the ordinary connected label. Each row's effective disclosure is the
global checkbox OR that row's automatic update disclosure: the checkbox is the user's preference and no update writes
it, while an update keeps its row folded during planning and execution, shows a pending status until a progress snapshot
is available, and then publishes compact step/count/elapsed progress beside the row status. A failed run or unresolved
diagnostic opens that row; authoritative success clears the automatic half for the exact tracked run. Provisioning
commands live in the row menu, but setup's confirmation and active or retained progress stay under the row because that
lifecycle owns more context than a floating menu can safely hold. Starting setup opens details before planning, while a
running or failed retained run leaves one short trace when details are closed. The one exception is an update whose
status is showing inline: the trace would only repeat it, so it is left out until that status clears.

Every per-session action lives in one floating actions menu behind the row's `⋯`, and four decisions about it are
contract rather than styling. **Anchor:** the panel opens just beyond the sidebar's right edge, with its top aligned to
the row and a small pointer toward it. It stays beside the session list instead of covering neighbouring rows and their
action toggles. One side placement is clamped to the viewport, including when there is too little room to keep the
sidebar fully uncovered or to keep the panel top aligned with a row near the bottom. The toggle holds a pressed accent
state, its row holds a tint, and the panel is a raised surface with a shadow. The action list starts with the session
title and a muted summary of its stored launch selection or legacy profile snapshot, followed by the concise state. The
summary uses the same launch-choice wording as the session launcher; a legacy session without a profile uses its agent
label. A structured session also keeps any source-profile snapshot in that line, and an unclassified session omits the
state word. There is no profile footer. The pointer is hidden when horizontal clamping makes the panel overlap the
sidebar, where it could no longer indicate the opening row. Commands have small decorative line icons and form groups
separated by non-focusable rules: rename and mark read/unread; clone, replace with, and replace; stop; delete. Only
groups with available commands contribute rules. Clone, replace with, replace, stop, and delete each have a visible
muted description exposed as an accessible description, so the accessible command name remains the action word. Hover
and focus fill each command inside the panel with rounded inset corners. **One at a time:** at most one row's menu is
open, and it closes on any layout change that could have moved the row it was measured against (a sidebar scroll or
resize, the host list's shape changing, the create form opening, the row reordering under a refresh), because the
panel's coordinates are a one-time snapshot. **Keyboard:** it is a real `role="menu"` and behaves like one — opening it
(pointer, Enter, Space, ArrowDown) lands focus on the first command and ArrowUp opens onto the last; arrows step and
wrap, Home/End jump; the whole menu is a single tab stop via roving `tabindex`, so Tab leaves rather than walking the
commands; Escape closes; and every close that took the menu away from a focused item hands focus back to the toggle
rather than dropping it on the document body — except the two transfers, Rename and a clone/replace-with acceptance,
whose newly mounted dialogs own focus instead, so the teardown retires its return rather than racing their mount
handoff. An item made inert by an in-flight operation stays focusable and refuses on activation (`aria-disabled`) rather
than going natively `disabled`, because a browser cannot focus a disabled control and a menu that went busy under the
user would otherwise swallow every navigation key. The row tint is owned by the menu's open state, not by
`:focus-within`: a dismissal may return focus to the toggle while the pointer is elsewhere, and that focused toggle must
not make the row look as though its menu is still open. The focused toggle or menu item retains the normal
`:focus-visible` indicator, so pointer-return focus and keyboard focus remain visually distinct without changing the
dismissal or focus-return contract. **Confirm in place:** a destructive item swaps the panel's own contents for the
consequence line and a confirm/cancel pair with focus on cancel, rather than opening a second surface; that sub-state is
a `role="dialog"` inside the same positioned box, and it survives the panel closing, which is why it deliberately does
not answer Escape.

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
overwrite an edit in progress. A structured source seeds the shared composer from its stored declarative selection,
preserving omitted harness defaults rather than parsing the compiled invocation. A legacy source selects
`other / command` in that same composer and seeds the raw invocation there, including when profile mode is selected and
displays the selected profile's invocation. Destination, folder browser, optional name, search, and submission remain
shared; only the structured model, effort, and permission controls are replaced by the profile picker and raw command
field. For legacy sources, the profile choice is used only when the row's own profile snapshot is `Present` — the
catalog still holds that id under the SAME name — which is deliberately STRICTER than an ordinary create's
remembered-default rule (an id that merely still exists, under a new name, is not evidence that cloning it again is what
today's catalog would still offer); every other answer falls back to the raw command. Trusting the id at all is still a
snapshot decision, not a live one: submitting a profile-backed clone resolves that id against whatever definition the
catalog holds at that moment, exactly like any other profile-backed create.

Search is the composer's one initial and post-selection focus target in both modes. Its command-mode result set is built
without the retained structured harness or model, so it can expose globally owned models but cannot offer an effort that
would edit only a hidden draft. A harness, known model, or recent setup explicitly returns to structured mode; a folder
changes the shared destination and leaves the active mode alone. The focus handoff runs only at dialog mount, explicit
mode buttons, and accepted search results. Catalog, history, and operation rerenders cannot take focus back from another
field. A clone or replace-with acceptance transfers that initial handoff from the originating row menu: removing the
activated item can leave the row's inside-focus bookkeeping populated — the teardown reclaims the item's element
identity in the same pass, so its `onfocusout` never runs to clear it — and the dismissal must retire its toggle return
for the transfer instead of issuing it after the composer's own search focus and stealing it back.

The leading `name:` label offers one action carrying its whole value, including later colons. `host:` filters the same
registry rows as the GUI selector; `host:local` uses the row's local kind rather than its mutable display name.
Accepting either action does not launch and keeps search as the focus target. A host action also takes over a clone's
inferred host and retires the old history destination and idempotency key, just as changing the host selector does. The
GUI labels an unaliased local row `local (this machine)` in the selector, Hosts page, and confirmed-local session rows,
while preserving aliases and the registry's own `this machine` name. A session row without confirmed locality retains
the name supplied by the helm; the GUI does not infer locality from its text.

The `perms:` scope offers only the default and YOLO choices that the active harness can normalize. Bare `yolo` joins the
ordinary combined search and gets exact-word preselection; bare `default` stays ordinary text. Applying a permission row
updates the same explicit-choice bookkeeping as the segmented control and leaves the other draft fields intact. Command
mode has no active structured harness, so it offers no permission actions.

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

Agent seeding is separate from host selection. Structured sources retain their stored launch selection. For legacy
sources, the form reads the source choice against the one helm-owned catalog, which applies to every host: a `Present`
profile is selected once the live catalog confirms its id, while every other source state falls back to the source
session's raw invocation. A delayed or unconfirmable host does not suppress that choice, and later host binding or
withdrawal does not change it. An explicit agent or command interaction is authoritative for the rest of the clone
generation, so neither a late catalog read nor a late host read can overwrite it.

A clone's working directory, invocation and title are peer-relayed text (SPEC.md's clone rule copies them off another
session, and a remote supervisor under `--ssh` is the one this client does not control) going into editable controls, so
they get the profile editor's escaped-display / raw-seed / edited-flag treatment (`profiles::submitted_field`) rather
than being written in raw: shown escaped while untouched, so a directional override or an invisible character cannot
make the field say something different from the bytes a submit would send, and an untouched submit still sends those
ORIGINAL bytes rather than the escaped spelling on screen.

The macOS desktop WindowBuilder retains native decorations while making the titlebar transparent, hiding its visible
title text, and extending the webview into the full content area. Tao positions the native traffic lights in logical
coordinates, and the root-mounted Wry webview retains the same inset because its content view replaces Tao's. The
desktop macOS shell class reserves matching space in the sticky sidebar app bar and aligns the session header's height.
In narrow windows, a fixed app row sits above both scrolling panes; the existing sidebar width, main-pane floor, and
horizontal scrolling remain intact without moving controls under the native buttons. The native window still has a title
for system menus. A dedicated empty Dioxus element owns native window actions for primary-button presses; the press
handler is not attached to a parent containing controls or text. Double-click zoom/restore is decided on the press's own
mousedown, before any dragging for that press begins: a page script (`assets/click-detail.js`) reads the mousedown's DOM
`detail` — which WebKit sets from the incoming native click count (`NSEvent.clickCount`, copied statelessly through
`WebEventFactory::createWebMouseEvent`, `WebEventConversion`, and `EventHandler::handleMousePressEvent`'s per-press
assignment) — and synchronously POSTs it, with an eligibility bit saying whether the preceding primary press landed on
the same connected spacer, to a Rust asset-handler route ahead of the interpreter's own event send for the same press.
That chain proves propagation of an incoming native count, not AppKit's classification itself: that the press after a
drag-consumed release carries count 2 is an assumption, unobserved here, and native zoom/restore awaits the manual Mac
checklist. A document-level capture listener runs strictly before the interpreter's delegated bubble listener, and both
sends are synchronous XHRs on the page's one JS thread, so each POST is answered before its press's event send starts
and the Rust side pairs the two by order. The spacer's handler then zooms exactly once for an eligible repeat press and
never drags it, drags an ordinary press as before, or does nothing at all for a cross-target repeat (no zoom AND no
drag); there is no `dblclick` handler. The first press's mouse-up may never reach the view once native dragging starts,
and that is harmless here: the count is assigned fresh from each press's own platform event rather than accumulated
across the release, and the release-dependent click/dblclick synthesis is on a path this design never uses. Fullscreen
suppresses the toggle explicitly (`toggle_maximized`, unlike `drag`, has no fullscreen check of its own, and Tao defers
a fullscreen maximize change until exit). The route is registered from the window root, which never unmounts, and the
page listener survives component remounts; a missing report against an empty slot, or a count stuck at 1, degrades to
ordinary dragging. That fail-safe does not cover complementary loss in both directions at once — report A arrives, event
A never consumes it, report B fails to replace it, event B consumes A — which borrows the wrong press's report and stays
a documented residual. Other builds keep that spacer hidden and inert, and receive no macOS shell class. Browser tests
can apply the class to production markup to verify geometry, without enabling native actions. Fullscreen, resizing, and
button behavior remain AppKit-owned; actual native appearance and interactions require the manual Mac checklist. The
window root installs native layout before authentication completes. Bootstrap and error pages reserve a top band without
requiring the sidebar to mount. A build-mismatch notice stays below that band and above the scrolling shell, with the
app bar pinned above it so the warning remains readable.

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
  `ClipboardSink` — farhelm-helm's clipboard.rs) and the shell writes the real pasteboard via arboard. One correction to
  the parenthetical this entry used to carry: loopback HTTP is a secure context in Chromium but NOT in WebKit — Safari
  against `http://127.0.0.1` has no `navigator.clipboard` either, so browser-tab copies work in Chromium-family browsers
  and stay silently refused in Safari, within SPEC.md's best-effort clipboard contract. One concrete thing to check
  early rather than debug late: wry's own file-drop handling swallows DOM drop events unless configured not to. Also
  established the hard way during M2 dogfooding: wry implements NO native JS dialogs on macOS — `window.confirm()`
  silently does nothing — so any confirmation or prompt the UI needs must be in-page DOM, never a browser dialog.
  SPEC.md's confirmation language is deliberately mechanism-agnostic; this is the constraint that picks the mechanism.
- Blitz (Dioxus's native renderer) is not production-ready. The plan assumes webview desktop indefinitely; nothing may
  depend on Blitz landing.

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
byte-identical to upstream is what makes its provenance checkable and a future version bump a plain swap.

JetBrains Mono Nerd Font is vendored alongside xterm.js for the same self-contained reason, and terminal.js sets it as
xterm's `fontFamily` — but it is no longer terminal-only: `app.css`'s `--font-ui` token (see the design-tokens paragraph
above) applies the identical vendored face to the rest of the chrome, so the whole app reads as one typeface. Chrome and
terminal share the same two cached `.woff2` files rather than each vendoring its own copy; whichever surface asks first
pays the fetch, and the other reads it back from the browser's cache.

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
`arm_native_clipboard`, re-armed with fresh credentials on every reauthentication): it POSTs the text to the embedded
helm's `POST /api/clipboard`, and the desktop shell — the only construction able to register a `ClipboardSink`
(`run_embedded`'s parameter; `farhelm helm run` hardcodes none, deliberately without a flag) — writes the real
pasteboard via arboard. The reason the desktop cannot use the web API at all: WKWebView does not treat the `dioxus://`
page as a secure context, so `navigator.clipboard` is absent there — not denied, absent — which shipped as
copy-never-works until 2026-09 (the Dioxus-risks bullet above records the diagnosis; farhelm-helm's clipboard.rs owns
the endpoint's contract: device-session auth with the desktop-webview CORS layering, one bounded text field, 404 on any
helm without a sink so a remote browser can never write a server machine's clipboard, and a silent 204 whether the
native write succeeded or not). A browser tab keeps the web API path and inherits its engine's policy: Chromium-family
engines treat loopback HTTP as a secure context and work; Safari does not, and stays silently refused. OSC 52 reads are
refused identically on every surface — the native route is write-only by construction, not by policy that could drift.

## Terminal substrate: private tmux server

Each supervisor runs a dedicated tmux server on a private socket (`~/.local/state/farhelm/tmux.sock`) with a locked-down
generated config: status bar off, `history-limit` sized to SPEC.md's replay floor, `remain-on-exit on`. One tmux session
per Farhelm session; window 0 is the agent terminal in practice, additional windows are the terminal tabs. Neither is
identified by position: the supervisor stamps each window it creates with a tmux user option — the agent's window with
the session id, a tab's window with a minted tab id that is also that tab's whole record. The agent terminal is
identified by its durable pane record first, with the marker as the recovery aid for a session whose record is empty;
tabs have no durable record at all and are rediscovered from their markers alone, because a pane's own processes inherit
`TMUX` and can conjure windows a positional scan would adopt. The user's own tmux usage and config are untouched.

Arbitrary changes to this private server's configuration are the local operator's responsibility. Farhelm supports its
documented pane/window interactions and handles missing objects safely, but does not promise to restore configuration
after arbitrary same-account changes. This does not relax exact targeting of operations or the helm/GUI's obligation to
tolerate remote failures; see SPEC.md's maintainer-confirmed decisions.

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
`farhelm
supervisor run`, else `FARHELM_TMUX` from its environment, else the bare name `tmux` — and every invocation
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
server with `fatal: not enough data`; the acknowledged transition is therefore part of the handoff contract, not cleanup
polish. Pane modes, a history snapshot, a visible-screen snapshot, and a final
`refresh-client -f !no-output,pause-after=N` are submitted as one semicolon-separated command group through that
replacement. The matching `%end` for the final refresh block is the cutover: earlier pane bytes are represented by the
snapshot, later ones arrive as live output, and `no-output` advances rather than queueing a second copy for delivery.
Normal-screen replay selects the history snapshot; alternate-screen replay selects the visible snapshot so normal
history is not mixed into a full-screen app. Known limitation, accepted: an alternate-screen replay carries only that
screen, so when the full-screen program later exits, the browser's normal buffer behind it is empty until new output
arrives, where tmux's own grid still held the pre-program scrollback. Replaying both would add two captures to every
attach for a cosmetic gain, and was declined (review finding A7-C8).

The initial foreign-pane filters must be arguments of that same cutover `refresh-client` invocation. Clearing
`no-output` resets the client's per-pane state, so sending the filters as a separate earlier command silently loses
them. The first bounded batch rides the cutover; any overflow is explicitly filtered afterwards, when no subsequent
`no-output` transition will erase it. Late panes use the live filtering path, with a bounded memo to avoid issuing a
filter command for every output notification. The session sink must keep those filtered panes readable; local dropping
of foreign bytes remains separate from this reduction in tmux notification traffic.

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
pane's status. What a dead pane shows is exactly what tmux holds for it and nothing more: the supervisor takes no screen
capture at stop, stores none, and appends nothing to a dead pane's replay. A restart into a surviving pane goes straight
to `respawn-pane`, which keeps history and reinitializes the visible grid; the supervisor neither shrinks the window to
push the grid into history first nor hands the launch shim a frame to paint before `exec`. SPEC.md forbids all three
(the restart rules under Lifecycle operations, the no-snapshot rule under Terminal experience) because a painted frame
with no process behind it is worse than a blank pane; do not reintroduce them. The one trace left is a startup sweep
that deletes a `<state>/snapshots/` directory an older build wrote (`sweep_legacy_snapshots_dir`), so an upgrade does
not strand captured frames on disk. Exec failure versus ran-and-died cannot be told apart by exit code alone (a missing
command yields 127 and a non-executable file 126 — both indistinguishable from a program exiting with that code), so
classification does not rely on exit codes: the shell execs `farhelm internal launch`, a shim that always exists, which
resolves and execs the profile invocation and, on exec failure, writes a sentinel with the errno detail to a per-launch
status file (named by session and launch generation, so a sentinel left by a failed earlier launch can never describe a
later relaunch) before exiting. The supervisor classifies **error** on that sentinel; the one sentinel-less error path
is a cgroup-scoped launch whose `systemd-run` wrapper died before the shim ever ran, recognized only by its full
evidence shape (dead pane, launch spec still unconsumed, no sentinel) so it can never claim an agent that actually
started. NOTE: a sentinel written by the shell after a failed `exec` was audited and rejected — interactive bash
survives a failed exec, but zsh terminates on it in every mode, so shell-side code after `exec` never runs for zsh
users; the shim works identically under any `$SHELL`.

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
host's state. Host-manager diagnostics may be noisy and may repeat indefinitely: every failed attempt and refresh
remains in the stderr trail, including a peer that returns the same error on every refresh. A collision check still
emits one summary event for that refresh, rather than one event per colliding row. The SSH stderr relay's separate
per-connection budget still bounds which child lines it logs while it continues draining the pipe.

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

`SessionInfo` carries a `last_activity_at` (unix seconds) beside `created_at`: the last time the supervisor observed
that session's agent pane change. It was added WITHIN protocol version 11 rather than bumping it, per the running rule
every version since 3 has followed — a new optional field with a decode default, whose omission a receiver can ignore
harmlessly, is additive; a new tagged variant or a required field is not. Absent means the sender predates the field and
decodes to 0, which a receiver reads as "unknown, fall back to `created_at`" and never as an instant in 1970. This is
recorded here because the per-version changelog in `lore/` is frozen at the moment it was written and is not maintained
as the protocol grows. The identity hook's pair — `ControlMsg::ReportConversation` and `ConversationReported` — went the
other way and took the protocol to version 12, since two new tagged variants are exactly what an older decoder refuses
outright instead of ignoring.

Protocol version 31 gives every terminal detach a code beside its reason. `ControlMsg::Detached` carries a `DetachCode`
(`taken_over`, `stalled`, `tab_closed`, `replaced`, `other`), a non-displacing attach refused because another client
holds the terminal is `ErrorKind::TakenOver` rather than a `Conflict` recognized by its message, and the helm forwards
the code in the browser's `detached` notice. Clients decide behavior from the code only (the browser latches a takeover,
holds a stall, and hides a closed tab) and show the reason as text, so reason wording is free to change. Before 31 the
helm and `terminal.js` compared English sentences they each kept a copy of, and only the disabled browser suite would
have noticed a rewording.

`SessionInfo::last_work_started_at` is the millisecond ordering key for the session list's stable work bursts. It was
added within protocol version 20 under the same additive rule: absent decodes to zero, and zero falls back to
`created_at * 1000` with saturating integer arithmetic. It never falls back to `last_activity_at`, because continued
output would then undo the stable ordering. A stale mutation reply merges this field by maximum in both the helm's
in-memory and durable caches, so delayed request traffic cannot move a later supervisor observation backward.

Version 13 adds the one shape on this wire that travels UPWARD as a request: `ControlMsg::AgentRequest`, answered by
`AgentResponse`. Both legs of its journey carry the same pair. An agent inside a session dials its own supervisor's
socket with the per-session credential — exactly as `farhelm spawn` does — and the supervisor forwards the request to
the helm, because the session has no route, address, or credential back to the machine the helm runs on. Nothing else
changed direction: the helm still learns about sessions by drain, so version 10's "no supervisor-edge push channel"
holds for everything but this. The supervisor picks the helm that holds an attachment to the asking session — well
defined by the one-attachment-per-session rule, and by construction the helm the user is looking at, which is also the
rule that stays correct if several helms per supervisor are ever supported. Request ids are per connection on this
protocol and stay that way: the asking process numbers its own leg, the supervisor numbers the upcall from a counter it
keeps per helm connection, and the relay holds the mapping for one round trip.

The helm's client closes its transport when its final owning handle drops, even with an unanswered upcall. Answer tasks
hold writer senders of their own, so closing the client's sender alone cannot make a quiet connection reach EOF. The
destructor aborts its registered answer tasks and signals the existing reader and writer cancellation paths.
Cancellation also interrupts a frame already being written: the connection is closing, so preserving frame
synchronization cannot justify retaining its transport until a live-peer stall timeout. The manager also retires a
withdrawn connection explicitly: it must close while callers still retain obsolete handles, rather than waiting for
final-owner cleanup. Neither path claims that cancelling an answer rolls back a mutation the handler may already have
applied.

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
budget (5 seconds) separate from the helm's answer budget (30 seconds) precisely to keep this distinction honest: a
request that spent its whole budget waiting for room on a full writer queue was never sent, and reporting that as
`Timeout` would invert the one thing the two kinds exist to say. Both budgets live on the supervisor because it is the
only party that can tell them apart; the asking CLI blocks with no deadline of its own so that the specific answer
reaches it.

A MUTATING verb is fenced against its own asker being deleted mid-flight, and its failures speak a different vocabulary
from a listing's. The credential that admits an `AgentRequest` is validated once, but a rename/stop/restart stays in
flight to the helm and back for as long as thirty seconds, which is ample room for a `DeleteSession` to revoke that very
credential underneath it. So the supervisor claims a per-asking-session fence (`Supervisor::agent_request_locks`) BEFORE
it checks the credential — checking first and claiming after leaves a gap a whole delete fits inside — and
`handle_delete_session` waits on the same key before tearing anything down. The fence is released when the MUTATION
ends, not when the CLI's answer budget does: a budget expiring says nothing about whether the helm is still working, so
the guard is held until the helm answers or the connection dies. That is bounded by the LINK's life rather than by a
clock, which is only a bound if the link can be counted on to end — and it cannot, because a response naming no pending
entry is dropped, which is right for an ordinary late answer and indistinguishable from a helm answering under an id it
has already used. So the retention has a last resort of its own (ten minutes), and its expiry RETIRES THE LINK rather
than dropping the guard: dropping it would be the same budget-shaped release on a longer clock, still guessing that the
mutation ended, whereas ending the connection makes every pending upcall on it resolve as the delivered-outcome-unknown
ending the relay already speaks. A response correlated to a `req_id` that was NEVER ISSUED is retired the same way and
immediately, on both legs of the relay: it cannot be a late answer, so the only readings are a broken peer and a hostile
one, and on a connection that stays healthy the waiter it strands has nothing else to end it. Correspondingly, a
connection lost after the request was queued is reported to a mutating caller as `Timeout` ("delivered, outcome
unknown") rather than `Unavailable` ("never delivered, retry freely"), with a remedy that says to look at the session
before retrying — the change may already have taken effect, and the retry-safe kind would be an invitation to apply it
twice. A listing keeps `Unavailable`, having nothing to double-apply. Which verbs are mutating is
`AgentVerb::is_mutating`, one exhaustive match in the protocol crate that both the supervisor and the helm read, so a
verb added later cannot be fenced on one side and not the other.

That vocabulary is a rule about a PHASE, not a list of failures, and every hop applies it the same way: once a mutation
has been handed to the next hop, the only endings that may speak plainly are the expected success reply and a refusal
the peer itself authored. Everything else — the link dying, a frame that will not decode, a reply correlated to another
request, a well-formed reply of the wrong shape, a reply of the RIGHT shape whose payload this side has to refuse (a
created session id that is empty, past the ingress cap, or carrying control characters), a refusal that could not be
enqueued — is delivered-outcome-unknown for a mutating verb, because each of them leaves the same question unanswered
and a peer broken enough to produce one is not thereby proof that nothing happened. The refused-payload case is the one
where "the peer probably did it" is strongest rather than weakest: the target answered with the very reply that says the
session was started, and the id that could address it afterwards is precisely what got thrown away. The helm's own
connection to a target supervisor enforces this by ending rather than by guessing: an agent answer it cannot even refuse
(its writer queue full) closes the connection instead of dropping the refusal, since the link dying is the terminal
event a mutation's retained fence is waiting for, and a silent drop on a link that then recovers holds that fence until
the retention's own last-resort bound expires. A wrong-shape reply is named by its `ControlMsg` VARIANT and nothing
else, for the same reason the phase rule exists at all: the full rendering of a legal near-frame-limit listing pushed
the agent's own reply frame past the protocol limit, whereupon the size backstop replaced the whole outcome and the
mutation vocabulary was lost to a bare `Internal` — and it carried a session's raw invocation and cwd into an
agent-facing error chain besides. That backstop now preserves an outcome-unknown verdict's kind and remedy when it has
to drop oversized prose, for the same reason: a size check must not be able to revoke a claim about durable state. The
phase rule is confined to the AGENT path (`agent_requests::transport_outcome`) rather than folded into `error_kind`, so
the REST surface keeps mapping the same transport failure to `Internal`: an HTTP caller has its own idempotency story,
and inventing a status for this would be a contract change no client asked for.

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
or authorize effects outside the named fleet operations and temporary execution exceptions in SPEC.md's
maintainer-confirmed decisions. Provisioning a supervisor does not establish trust in its responses. What the helm does
check is that the connection is still the CURRENT one for that host row, since registry rows outlive the machines behind
them. Version 14 replaced session-list pagination with a bounded whole-list reply, and version 15 carries helm-resolved
launch bundles and upward profile resolution. The historical paragraph below describes why 13 was current at the time;
later released additions took the wire to 26. Version 16 introduced the durable optional structured launch snapshot
carried with a create and `SessionInfo`. The snapshot is declarative provenance beside the resolved invocation, never a
browser-owned compiler input; old sessions remain absent rather than being reconstructed from a command. Version 17 adds
`BrowseDirectory` and `DirectoryListing`: the helm routes one authenticated, connection-incarnation-guarded request to
the chosen supervisor, which expands `~` from its own recorded home, canonicalizes the requested directory, and returns
only a sorted bounded immediate child-directory listing plus parent and truncation state. Neither the helm nor the
client reads the target filesystem. Version 18 adds accepted-create `canonical_cwd`, the identity fact that binds folder
history to the destination the target supervisor actually accepted. Version 19 adds OpenCode to the structured-harness
enum. Version 26 retires session archival and its wire fields and messages. A supervisor must retain that snapshot
alongside the resolved invocation, so an older peer that cannot decode the new enum value refuses the connection rather
than silently losing the selection. The following 13 paragraph is historical context, not the current protocol version;
the frozen changelog stops at 11. Version 13 also carries `AgentVerb::Rename`/`Stop` and the two creating verbs
`AgentVerb::Create`/`Clone` (answered by `AgentReply::Created`), all added additively within the version rather than as
version bumps of their own — which was possible ONLY because 13 itself had not yet shipped when they landed, still being
developed on this branch with no released build speaking it yet. That is a one-time allowance for a version still in
flight, not a standing license to keep adding to 13 after it ships; once a protocol version has shipped, a wire-shape
addition needs a version of its own, same as any other. The same allowance covers the one thing in 13 that is not an
addition at all: `AgentSession::host` became `Option<String>`, so a reply carrying a row the helm just mutated or
created can say "there is a session here but no host name I can vouch for" instead of encoding that as an empty string
indistinguishable from a real value. A decoder built against 13 EARLIER IN ITS OWN DEVELOPMENT rejects `host: null`
outright — the running additive rule does not stretch to cover it under any reading — so it is allowed here only because
nothing released speaks 13 yet. It must not be carried forward the same way once 13 ships: the identical edit made
afterwards needs a version of its own.

Version 25 adds `AgentVerb::Restart`, `AgentReply::Restarted`, and the non-secret `AgentSession::restart_offer`
discovery field. The new tagged request and reply require an exact-version handshake refusal for older peers. Each verb
is routed and recorded through the same `sessions.rs` functions the corresponding REST route uses — `route_session`, the
client call and `record_session` for the lifecycle four, and `do_create_session` (the shared internal function
`POST /api/sessions` was refactored onto) for the creating two. So a refusal that comes out of the SHARED operation — an
unknown session, a disconnected host, a title the owning supervisor rejects — is the identical sentence the UI would
have shown, and a session an agent creates is seeded into the helm's cache and published exactly as one the create
dialog made.

The equivalence covers that shared path and stops there, deliberately, in two places. The relay adds a doorway check of
its own (`validate_agent_verb`) that the REST surface has no counterpart for, since only the relay puts an
attacker-chosen target, title, directory and host name onto two byte-unbounded queues before anything downstream can
look at them; its refusals are relay-specific by construction. And the agent CLI escapes and caps a refusal before
printing it, because the destination is a terminal rather than a browser — so the WORDING is the UI's, while the bytes
may be escaped and the tail cut. Both divergences are one-directional: they can refuse something the UI would have
allowed through to the same shared code, never the reverse.

`Created` is a distinct reply tag from `Session` even though the payload is identical, because the tag is the only thing
separating "what your creating verb produced" from "the row you changed" and the CLI checks it before printing an id. It
is not a novelty claim: a create or clone carrying an idempotency key the target has already served replays that
session, which arrives under this same tag. The helm draws the one distinction that matters from it — a clone whose
result is the asking session or the named source is refused rather than reported, before anything durable is recorded
for the replayed row. The two creating verbs name their target host by display name, matching `AgentHost::name`.
Registry IDs join host and session discovery and distinguish duplicate labels; they are not destination selectors. A
name matching two registered hosts is refused as a `Conflict` naming the collision, never resolved to whichever row the
listing ordered first: display names are not unique by construction (the local row renders as `this machine`, and an ssh
destination may be spelled the same), and guessing between them would put a session on a machine nobody chose. A
registered name carrying a control character can never be typed back at all, since the relay refuses one in `--host`;
the not-found refusal says so by count, because the fix is a rename and an agent has no rename verb for hosts.

The helm resolves an agent's profile name exactly against its one catalog before the target call. Zero or multiple
matches are `InvalidRequest` refusals that name candidates; one match becomes the invocation, agent kind, resume
template, and immutable profile snapshot carried to the supervisor. A clone follows its source's snapshotted profile id
through the same helm catalog on any host. A missing id is refused before a target call, with no fallback to the old
name or source invocation.

`Clone` resolves the explicitly named source's live owner, drains that owner's pinned connection, and rechecks the owner
before dispatching to the destination. It does not use the helm's cache: a clone built from a cached row could copy a
title or directory the session no longer has, or read from a host that stopped owning it. It refuses a result whose id
is either the SOURCE or the ASKING session: a keyed replay must never be reported as a new child. Both verbs take the
fence on `agent_request_locks` that the lifecycle verbs take, since a create that completes while the asking credential
is being invalidated would otherwise leave a session running that nobody was told about.

A KEYED RETRY IS BOUND TO THE RESOLVED BUNDLE SENT TO THE SUPERVISOR. The fingerprint covers the invocation, agent kind,
resume template, and profile snapshot, so editing a profile between two attempts under one key makes the second request
different and produces a conflict rather than replaying the first outcome under changed settings. The same applies when
a clone's source metadata changes between attempts.

The relay's own doorway bound treats the host name SEPARATELY from the create payload (`AGENT_HOST_NAME_CAP`, the same
number every session id is held to). It is routing metadata the helm consumes and no supervisor ever sees, so charging
it against SPEC.md's 64 KiB create-field allowance would let a long registered host name make an otherwise-legal create
fail through the agent surface alone. What the aggregate covers is exactly what a create carries: directory, selector
and title, summed.

And a created session's id is checked at INGRESS, where the reply enters the helm — non-empty, control-free, and inside
the same length cap `manager::drain_sessions` holds every listed id to. That id is the one value on this path with a
machine consumer: it is printed on the CLI's stdout as the answer, and an id carrying a newline forges a second line in
whatever captured it. A reply that fails the check is refused rather than sanitized, because a scrubbed id is not the
session's id and everything done with it afterwards would address something that does not exist.

One deliberate difference from `farhelm spawn` is worth stating rather than discovering. Spawn's `intent_key` gets
`CreateAdmission::Spawn`'s session-lifetime reservation scope, because the create arrives on the asking session's own
credential. An agent's `create`/`clone` reaches the target supervisor over the HELM's full-authority connection, so the
key currently gets the same permanent, interactive scope any other helm-mediated create gets. Permanent retention is not
a security requirement for these agent-originated requests: SPEC.md's temporary creation/cloning exception also covers
their existing retry exposure. The current implementation remains described here until a separate retention change is
made. Session-lifetime scoping is not merely unimplemented here — it is not expressible, since the target supervisor may
never have heard of the asking session. Spawn requires an explicit selector: `--inherit-agent` copies the asking
session's exact stored launch bundle on its own supervisor and therefore works offline, while `--agent` and
`--profile-id` send `ResolveProfile` through the existing upward relay and are refused with the `--inherit-agent` remedy
when no helm is attached. Agent create likewise requires an explicit profile name, profile ID, or raw invocation.

One divergence is worth stating rather than discovering later. A RAW clone — one whose source came from no profile —
copies the invocation and nothing else, so the target re-derives the integrated kind from the invocation's first token
and takes that kind's default resume template. A source created with an explicit `agent_kind` (including the explicit
"no integration" the tri-state can express) or a custom `resume_template` therefore clones into a session whose
conversation capture, status classification and restart behavior may differ from the original's. Nothing the helm can
read carries those values: `SessionInfo` — the shape `drain_sessions` returns and the helm's only view of another host's
session — exposes `invocation` and `source_profile` and no integration fields at all. Copying them means putting them on
the wire, populating them everywhere a supervisor builds a session row, and persisting them for a reload to find.
Refusing the raw clone instead is not available: SPEC.md promises a raw session "clones as that invocation".

The discovery verbs are answered from the helm's own listings, narrowed to what an agent can name and act on. Two
narrowings are contractual rather than incidental. The session listing is the same whole-fleet listing the UI reads, cut
at the same cap (`LIST_SESSIONS_CAP`, a few hundred rows) and additionally at an encoded-byte allowance of 6 MiB —
leaving the reply's envelope room under the 8 MiB frame limit — and carries a `truncated` flag when either cuts it,
because a partial fleet listing is otherwise shaped exactly like a complete one and "that session does not exist" would
be indistinguishable from "that session is past the cut". The byte allowance exists because rows bound nothing about
size: session creation admits tens of kilobytes of caller-supplied text per row, and a fleet of legally fat records
would otherwise produce an answer no frame could carry — discarded whole, reaching the agent as `Internal` rather than
as the partial listing the verb promises. The per-session `agent` field is a non-secret label — the source profile's
snapshotted name, or the invocation's program basename — never the raw command line. Users put credentials in command
lines, this listing is readable with any one attached session's credential, and its reader is a model that will quote
what it read, so arguments must not cross this wire at all.

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

## Supervisor internals

### Owned checkout admission and lifetime

Protocol 24 carries validated checkout destinations separately from existing cwd requests. Only full-authority callers
can preview, discover or create them; restricted session clients cannot supply helm-owned configuration. Shared proto
validation constructs the sole HTTPS GitHub URL and validates naming; the supervisor repeats validation at admission.
Existing request fingerprint encodings remain frozen. Fresh fingerprints include the original client identity and
resolved configuration, while reconciliation looks up the original request before consulting mutable settings.

Reconciliation defaults to lookup: an unknown key has no effect. When the helm must refuse an unknown request, it asks
the supervisor to settle a permanent identity-bound refusal under intent and directory admission. The supervisor reads
back the stored row before returning proof, so a concurrent winner remains authoritative. Known pending attempts still
use their recorded recovery state; reconciliation is therefore not generally read-only. Storage or authority failure, an
identity mismatch or an unverified installation cannot establish definite refusal. The identity-only refusal uses a
versioned fresh fingerprint variant and the ordinary serialized create-field cap; existing fingerprints stay unchanged.

Schema 18 stores the checkout registry, memberships, origin provenance and preparation snapshot alongside session and
intent state. Directory admission serializes allocation, membership insertion and last-reference teardown. Intent locks
precede directory admission; profile catalog round trips precede admission so a restricted parent's lifecycle claim
cannot be held while waiting on the helm. Discovery uses its own subprocess budget rather than directory admission.
Fresh allocation records its plan before mkdir, then records filesystem identity before exposing preparation. A
post-allocation failure atomically retains an error session and membership instead of losing the only deletion handle.
Borrowers join every applicable canonical managed ancestor; they never inherit the origin's preparation duty.

If identity capture never committed, an existing candidate path remains ambiguous: explicit Delete retires the plan
without adopting or moving that object and names the preserved path in its diagnostic.

The launch shim executes clone, the frozen optional hook, and the real agent invocation in the existing terminal.
Preparation has durable claim/progress/Ready state. The parent-directory durability barrier precedes launch publication;
once publication may have started, missing or ambiguous evidence refuses automatic repetition. Only the original pending
create may recover an unstarted preparation. Ready restart validates the recorded identity and skips clone and hook. The
agent's kind and launch metadata remain its actual values, rather than identifying the preparation wrapper.

Last-reference Delete journals the source identity and archive destination before one no-replace rename. Linux uses
`renameat2` through its syscall with `RENAME_NOREPLACE`; macOS uses `renameatx_np(RENAME_EXCL)`. Parent fsync barriers
precede journal retirement and atomic metadata settlement. Recovery accepts a matching already-moved destination but
does not adopt a foreign source object. No recursive-copy or delete fallback is permitted. Directory admission also
orders ordinary same-path creates against that move, so a new reference either commits before Delete or observes the
directory as unavailable afterward.

Root identity is checked even before accepting an apparently missing source. Common archive entry points refuse
overlapping active registry paths, including during startup recovery. After process teardown and committed final
retirement, Delete removes the private preparation lock and state files. This cleanup is best effort: a crash or unlink
failure can leave private evidence, but cannot authorize another directory move.

### Runtime state

- State in SQLite (rusqlite) at `~/.local/state/farhelm/supervisor.db`: sessions and their metadata (SPEC.md's
  supervisor-authoritative list), each session's profile snapshot taken at creation, spawn idempotency keys, captured
  conversation identities, host identity, and the boot id last seen. The helm owns the mutable profile catalog, so the
  supervisor records the snapshot but sends `ProfileExistence::Unresolved` on its wire replies. Comparing the stored
  boot id against the current one (`/proc/sys/kernel/random/boot_id`; `kern.bootsessionuuid` on macOS — a per-boot UUID,
  chosen over `kern.boottime` because the kernel rewrites boottime on clock steps and a boot id must never change
  mid-boot) is how "interrupted" is classified per SPEC.md.
- Host identity: generated once at first run, stored in the db.
- The interrupted-session surface is a centered neutral card in the empty terminal area: it explains that a host restart
  paused the session and keeps the terminal absent until the user intentionally chooses Restart or confirms Replace.
  Restart is an unconfirmed normal action because no process remains; Replace starts as a normal trigger and keeps its
  destructive confirmation inline. Replace reuses the existing create-then-delete endpoint and, on success, hands the
  new session back to the page selection owner so the fresh conversation becomes visible immediately; refusal remains on
  the card with the endpoint's actionable wording. An open Replace confirmation keeps its trigger visibly pressed.
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
  sits BELOW the recorded-error and dead-pane rules in the existing precedence, so a heuristic only chooses among the
  live statuses once sampling has produced evidence. The generic baseline is observed output alone, counted in a
  session's OWN samples rather than in elapsed time: three consecutive samples showing an unchanged screen reads idle,
  anything else live reads running. A new launch reads running before its first comparison. A reloaded live pane instead
  reports provisional `unknown` until a changed screen, positive work hint, recognized waiting prompt, or three quiet
  comparisons provide fresh status evidence; dead-pane and recorded-error outcomes bypass that provisional state. The
  helm retains the prior status from its per-host cache for an `unknown` reply, if it has one, and replaces the rest of
  the row as usual. Its identity-less in-memory list follows the same rule within a connection. Disconnect clears those
  rows by the existing identity-less host rule, so there is no previous status to retain after its supervisor restarts.
  Counting samples rather than seconds is load-bearing — the sampler works through live panes on a budgeted round robin,
  so a session's real sampling period grows with the fleet, and any wall-clock window would eventually report a
  continuously-working agent as idle because the HOST was busy. Waiting is never derived from activity at all (a blocked
  agent and a finished one are equally quiet); it comes only from per-kind sharpening. Codex is the one audited
  exception to raw comparison: the sampler takes a temporary 64 KiB visible-grid tail, recognizes only its bottom
  composer (including its known sparkle cells and safely bounded draft rows), then applies the normal UTF-8-safe
  4096-byte cap to canonical comparison text. It separately retains the raw 4096-byte tail for waiting recognition.
  Unknown composer, popup, and output shapes remain unchanged. The pinned Codex `Working (elapsed • esc to interrupt)`
  widget adjoining that composer can prevent quiet decay in both its animated and reduced-motion forms, including its
  bounded inline context and detail rows; historical or quoted copies elsewhere in the pane do not. Waiting still wins.
- Last-activity timestamp: the same ticker that samples for status also DATES the changes it sees, into a
  `last_activity_at` column on the session row and onto the wire. It drives the row's displayed age and the helm's
  seen/unseen comparison, seeded to the session's creation time so one that has never produced output has an honest age,
  and restored verbatim on supervisor restart. Persisting it does not contradict the rule that liveness is never
  persisted: a status is a claim about NOW and rots the instant the process it describes moves on, while this is a claim
  about a past instant that the passage of time cannot falsify. The two must not be conflated in the other direction
  either — classification still reads sample COUNTS and never this clock, for the population-dependence reason above.
  The value advances only when the observed change is at least a minute newer than what is already stored, and the
  reason is blast radius rather than resolution. Two costs, scaling differently: a durable `UPDATE` per session per
  crossing, which without the quantum would be a write per busy session every two seconds; and a fleet-wide UI wake,
  which is COALESCED — the helm detects a changed session by comparing whole serialized `SessionInfo`s, but bumps the
  invalidation feed at most once per host refresh that found anything different, however many sessions moved. So the
  wake is bounded per refresh while the writes are bounded per session, and without a quantum a single busy agent would
  re-render every connected client on every drain. No user distinguishes two sessions whose last output was twenty
  seconds apart. Writes are monotonic in SQL as well as in memory, so a backwards clock step cannot make displayed
  activity younger or undo an unseen observation; a lost write costs age precision after restart until another observed
  change crosses the quantum, and nothing else.
- Work-start ordering: the ticker separately advances `last_work_started_at` only when changed output moves a session
  from a previously known idle or waiting state to running. The first sample, the first successful sample after a
  capture failure, one or two quiet comparisons, continued output in the same burst, and completion do not advance it.
  This uses the same live-status classifier replies use, so waiting recognition and three-comparison idle hysteresis
  cannot drift between the badge and ordering. The key is milliseconds since the epoch, but allocation is
  supervisor-local monotonic state rather than a bare clock read: startup seeds it to the greatest effective key across
  every loaded row (ended rows included), and a start reserves `max(now_ms, previous + 1)` with saturation. This orders
  same-millisecond bursts and survives a backward clock step on one supervisor. Separate supervisors still have only
  their wall clocks and the helm's deterministic creation/id/host tie-breakers; this does not claim distributed
  causality across skewed hosts.

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
  naming the kind itself): status sharpening over the sampled tail, and conversation-identity capture. Sharpening is a
  DEFAULTED trait method that may only promote a live baseline to waiting, never invent liveness, and never panic on
  arbitrary terminal bytes; the default is "no sharpening", which is deliberately different from the no-integration case
  (generic sessions still get the baseline). Recognition is conservative by design — a vendor question phrase AND a
  rendered menu of numbered answers, both at the bottom of the screen — because a status that reads waiting at a working
  session teaches users to ignore the column, while a missed prompt merely reads idle. Claude Code: watch
  `~/.claude/projects/<munged-cwd>/` for the session record. Audited specifics that shape this: the record appears at
  first prompt submission, not at launch, so correlation keys on first-input time and tolerates an unbounded
  launch-to-first-input gap; the cwd munging is non-injective (`/`, `.`, `_` all become `-`); and per-line JSON fields
  (sessionId, cwd, timestamps) are the reliable correlators — file birth times can postdate content after rewrites. An
  identity is claimed only when correlation is unambiguous — two near-simultaneous launches in one cwd stay uncaptured
  rather than choosing a record arbitrarily. A scan-derived Claude identity retains its exact record locator for
  append/restart re-verification; a Claude hook report instead remains the agent's direct answer. Codex no longer uses
  this fallback: even a single matching rollout may belong to a nested invocation rather than the foreground.

  **Codex attribution and exact-record validation.** The Unix accept loop captures the kernel peer PID and its process
  start token before scheduling the connection handler. For a Codex report, a bounded, revalidated ancestry walk must
  reach the session's owned live pane and contain exactly one native executable whose basename is `codex`. The pane
  anchor itself is exempt from intermediary classification; additional surviving launch wrappers above Codex are not.
  This deliberately rejects some multi-layer package-manager launchers that older builds accepted. The supported process
  chains are documented in [the Codex integration](website/src/content/docs/docs/harnesses/codex.md). The reporter must
  spell the installed hook invocation (`<farhelm> internal hook …`, matched syntactically so an upgraded supervisor
  still accepts older hooks), and every other non-Codex link except the pane anchor must be a narrow shell trampoline
  directly invoking it (`sh -c '<farhelm> internal
  hook …'` — the shape vendor hook runners produce) as exactly one
  simple command: any unquoted control operator, redirection, substitution, or other executable shell syntax refuses
  even when the hook comes first, while metacharacters inside quoted paths stay literal; another session-hosting runtime
  or any unclassified intermediary (interactive shell, script, chained command, unreadable argv) refuses. Argv
  classifies honest trampolines only: a descendant forging the exact hook shape is outside the attribution model.
  Unavailable process evidence refuses the report without changing the current identity. This is attribution under
  inherited credentials, not a security boundary against the same Unix user. Renamed native executables are not
  recognized.

  Process evidence is necessary but not sufficient: threads may share a process. A versioned `codex:` locator binds the
  reported runtime session ID to an absolute transcript path and a separately verified persistent thread ID. The bounded
  no-follow regular-file reader requires the first complete record to be root `session_meta`, with source `cli` or
  `exec` rather than a subagent source. Runtime metadata must match the report; once bound, a different persistent
  thread cannot replace the file's identity. Resume substitutes the persistent thread ID. No directory scan or home
  inference participates, so a configured custom home works without becoming a second source of truth.

  Only an attributed `SessionStart` with a recognized source is accepted. A foreground `clear` may install a pending
  locator before the exact file is available, withdrawing the discarded conversation immediately. Capture passes and
  resume verification re-read that exact file; absent or mismatched evidence cannot advertise a usable resume target.
  Report and refresh transactions share a capture-only per-session claim and read the current durable binding while
  holding it. This is separate from the lifecycle claim: a pre-publication hook must not wait for its own launcher.
  Promotion of the previous record cannot discard a legitimate clear, and a repeated report must preserve an established
  thread binding. Both writes also compare the generation and complete prior locator, including an initially absent
  capture. Reports received before in-memory publication use the durable launching row and discover its owned pane from
  tmux. Historical bare Codex IDs are retained but fail closed rather than being guessed into a new locator.

  **Grok attribution, ordering, and exact-record validation.** Grok uses the same peer PID, bounded ancestry walk,
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
  bounded UTF-8 JSON document carrying the same UUID at `info.id`; a valid prefix is not enough. Reconciliation rechecks
  that pair under the shared claim, persists readiness withdrawal without discarding UUID or timestamp, and publishes
  only the matching generation. Resume rechecks immediately before launch and substitutes the verified UUID, never
  either path. No history scan or path derivation participates. Grok's manual hook configuration is the deliberate
  exception to per-launch injection: Farhelm invokes no editor and writes no vendor file.

  **Shared attribution framework and the five-step admission.** The ancestry walk above is shared mechanics, not Codex
  code: at most 64 live `Running` edges from the socket peer to the owned pane, the peer's start token verified first,
  loops and a missing pane refused, and every edge plus every image observation re-read before the walk returns (an exec
  between the walk and the re-read refuses, since the observed process is no longer the one the walk saw). Each edge
  captures its image (required — an unreadable image refuses, as it always did) and its argv (optional evidence: `/proc`
  exe plus bounded NUL cmdline on Linux, a new bounded `KERN_PROCARGS2` argv reader on macOS, 64 KiB per process and 1
  MiB per walk, with over-budget or truncated argv recorded as missing rather than prefix-matched). The per-kind step
  applies its own restrictive corridor to the returned chain — only the reporter plus a narrow trampoline between
  runtime and reporter; any other session-hosting runtime or unclassified intermediary refuses. Codex's instance is
  exactly one native `codex` image, a hook-shaped reporter, and shell-`-c` trampolines only; Grok's adds the required
  `--no-leader` option to its one native image. Admission runs five steps: cheap envelope/kind/generation gating with no
  vendor I/O (the doorway's discriminator check re-applied against the fenced resolution, plus raw
  event/source/agent-identity validation before diagnostic sanitation); the bounded capture claim (one shared
  session-keyed `capture_locks` registry for report admission and readiness refresh, never the lifecycle claim, at most
  a second of waiting on the report path) and a reload comparing kind, generation, and the complete prior binding; the
  mutation-free runtime and vendor-root proofs with repeat attribution around the evidence; the atomic
  generation-plus-complete-binding CAS committing identity, locator, provenance, source, readiness, and the ambiguity
  reset together; and the mirror of only the committed result into the matching current-generation entry under the same
  claim. Rejection at any pre-write step changes nothing durable, in memory, ambiguous, pending, or offered. Refresh and
  report-only reconciliation passes take the same claim and reload before mirroring, carrying the row's version beside
  the identity, so memory-derived offers apply the same gate as row-derived ones without a second lookup — and a
  rejected report never triggers a readiness withdrawal through them. Scan writes, restart verification and lifecycle
  resets retain their durable generation/binding fences; they do not all acquire this capture claim. The timeout bounds
  lock acquisition, not the entire admission operation.

  **Ownership provenance and the offer gate.** Migration 20 adds `capture_ownership_version`
  (`INTEGER NOT NULL
  DEFAULT 0`, identical in fresh DDL and `ALTER`, tables kept `STRICT`): 0 means not established
  under the ownership contract — every historical capture — and only the authoritative admission transaction ever writes
  1, for the kind whose proof ran. Exact Resume requires version 1 for such kinds on every surface (reload, list and
  replay views, direct restart requests, pre-spawn verification), with the stored row's version beside the identity at
  each one; unknown versions preserve their data but refuse Resume and promotion. The deliberate Codex exception keeps
  valid `codex:` v1 tokens admitted before the column existed resumable at 0 through the existing exact-record verifier
  — bare IDs stay excluded, nothing is backfilled, and file existence or a valid header can never upgrade a version.
  Relaunch clears provenance to 0 exactly when it clears the capture (Fresh/fallback) and preserves both together on
  Resume; the restart claim compares the version alongside the identity, so a provenance change under an unchanged
  conversation still invalidates a stale claim. Protocol 28 introduced the required closed-enum report discriminator
  that the doorway gates on; protocol 29 adds Grok to that closed enum together with its agent-kind and launch-harness
  variants. Senders predating either required variant fail closed at decode and at the missing CLI flag alike. Migration
  21 adds `omp_reporter_asset` (nullable `TEXT`, identical in fresh DDL and `ALTER`): the gated reporter asset's file
  name as installed by the session's current launch, written pre-spawn — after the launch spec publishes, before tmux
  can start anything — from the one place that decides injection and fenced on the launch generation, so no report of
  the generation can arrive ahead of its provenance. Migration 22 adds `omp_launch_program` the same way: the program
  that launch's argv started, retained beside the marker so admission classifies the current launch rather than the
  resume template (a future resume's command). OMP admission requires the marker to name the current binary's asset with
  the file's bytes re-verified; pre-21 rows adopt `NULL` and fail closed. OMP takes no Codex-style exception: every
  older OMP row offers fresh-only until its first proven report.

  **Interim ownership states.** The discriminator gate applies to every kind now: it is envelope, migrated together.
  Attribution proofs apply to Codex, Grok, and OMP. Goose, Claude, and Pi retain their existing acceptance behind the
  discriminator gate, and new framework entry points default to deny rather than allow. The offer gate has its final
  shape but flips per kind: Codex, Grok, and OMP require version 1, while the other kinds keep today's offer behavior
  until their proof lands, writes 1, and flips the single per-kind predicate every surface consults. There is no general
  report epoch. Grok's locator carries only its vendor-specific selection timestamp; no other kind inherits that
  ordering rule. OMP uses serial cancellation fences, not cross-reporter chronology. Old processes and assets fail
  closed after the upgrade; nothing is grandfathered.

  **The per-launch identity hook.** Scanning cannot see a conversation being replaced inside a live process: Claude
  Code's `/clear` and Codex's `/new` both mint a new conversation id with nothing on disk pointing back at the record
  they replaced, so a scan-derived identity keeps resuming the conversation the user just threw away. Both vendors fire
  a `SessionStart` hook whose payload carries that id, and both accept a hook supplied on the command line for a single
  launch, so farhelm appends itself as that hook (`farhelm internal hook --vendor <adapter>`, reporting over the
  supervisor's one shared `supervisor.sock` and authenticating with the per-session credential the launch already
  carries) and lets the agent state its own identity. The `--vendor` flag is the report envelope's discriminator,
  sourced from the installed entry point rather than inferred from the payload; the Goose helper supplies its own value
  internally so the persisted declaration keeps invoking the same command, while the Pi/OMP assets pass theirs on the
  spawned command line and keep their JSON `vendor` field purely as a consistency check. Claude takes it as
  `--settings <json>`; Codex takes `--dangerously-bypass-hook-trust -c features.hooks=true -c hooks.SessionStart=…`.
  Per-launch is the whole point: nothing is written to `~/.claude` or to Codex's active configuration home
  (`$CODEX_HOME` when set, `~/.codex` otherwise), no trust state is left behind, and flags cannot outlive the process
  they were passed to — which is what keeps SPEC.md's no-agent-configuration rule intact rather than merely bent. The
  costs are accepted deliberately, and both are scoped to the launches that actually carry the injected flags rather
  than to Codex launches in general: on those, Codex prints a hook-trust warning line above its composer, and with trust
  bypassed any hook the user has in that same configuration home but has not trusted runs too. Codex fires
  `SessionStart` at the first prompt rather than at process start, so a Codex session's identity arrives only once the
  user has typed something, where Claude's arrives at startup. And three invocation shapes disqualify a launch, which is
  skipped with a logged reason rather than made to work: an argv that already carries `--settings` (Claude honors only
  the last one, so injecting ours would silently drop the user's), an argv already steering Codex's own hook
  configuration (a second bypass flag risks a rejected command line, and the `hooks.`/`features.hooks` tables are the
  user's once they touch them), and — for either vendor — an argv containing a bare `--` (our flags would become prompt
  text). `FARHELM_AGENT_HOOKS` in the supervisor's environment — `all`, `none`, or a comma list of kinds — turns
  injection off wholesale or per kind, read once at supervisor start and carried as a seam value. Claude's scan remains
  the fallback when no report has been accepted; Codex requires attributed reporting and does not infer ownership from
  nearby rollout files. An accepted report dominates scan-derived state, including ambiguity.
  `website/src/content/docs/docs/concepts/agent-hook-injection.md` is the user-facing account of the same mechanism.

  Grok uses the same hook executable and authenticated supervisor message but not this injection path. Its native TUI
  cannot take a per-launch hook overlay, so the user installs three matcher groups under `$GROK_HOME/hooks`: one each
  for `SessionStart`, `UserPromptSubmit`, and `Stop`, all invoking `farhelm internal hook --vendor grok` without
  `--announce`. The tracked launch supplies the credential in the inherited environment; the configuration contains no
  Farhelm token or session identity. Missing configuration leaves the Grok session usable but unable to gain a new exact
  resume target.

  **Goose and Pi reporters.** These integrations never scan vendor state. A fresh Goose launch registers one named stdio
  MCP server, `farhelm-reporter`; Goose persists that declaration in its conversation, so resumed launches add no second
  reporter and only supply current-launch enablement, executable, and instruction controls. The persisted command
  contains no credential or session identity and falls back to `farhelm` on `PATH` for a manual Goose resume. Its empty
  MCP interface reports `AGENT_SESSION_ID` when current Farhelm credentials enable it and carries the instruction
  pointer in the initialize result. Pi loads a versioned TypeScript artifact materialized with private permissions under
  Farhelm's state directory. The extension serializes `session_start` and `agent_end` reports, including the exact
  absolute session file only after Pi has persisted it. The database retains a bounded, versioned Pi locator containing
  both ID and optional file; list/status inspect only that token. Resume alone opens the exact file through the bounded
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
  acceptance, not by file bytes. OMP's resume template strips OMP's own session selectors before appending
  `--resume <verified-file>`, because unlike Pi's, OMP's `--resume`/`-r`/`--session` consume optional values (and its
  `--fork`, `--continue`, and import flags have their own shapes), so Pi's valueless-flag classification would leave an
  old session source standing between the user and the verified target. An OMP argv containing a GENUINE end-of-options
  delimiter (an unconsumed `--`, walked with the same argument grammar the stripper and the classifier share) refuses
  the DERIVED template: everything after `--` is prompt text in OMP, so an appended resume target can never be read as
  one, and refusing the create fails closed rather than persisting a template that cannot work. A `--` consumed as an
  option value (`--system-prompt --`, a prompt spelled `--`) is not a delimiter and creates normally, and an explicit
  template override — filled verbatim, never appended — creates normally behind a genuine delimiter too. Which OMP
  launches get the extension is decided by an OMP-specific interactive-shape classifier following OMP's own command and
  flag-consumption tables; Pi's classifier is untouched. The gated extension asset is materialized per vendor under
  Farhelm's state directory (`integrations/omp/farhelm-conversation-v2.ts`, sourced from `assets/omp-conversation-v1.ts`
  — the published name is versioned past the gateless `v1` bytes, published beside them never over them) with the same
  exact-bytes, private-file, no-follow rules Pi's had, loaded with `-e <materialized path>` and pointed at the reporter
  through `FARHELM_OMP_REPORTER_EXE` (scrubbed from preparation children like the Goose/Pi reporter variables; its only
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
  unclassified intermediaries, Node-executed OMP, unknown wrappers, and ambiguous package scripts all refuse; the live
  runtime argv is re-read through the same grammar injection uses, so a process that exec'd from a TUI launch into a
  utility or print shape refuses too. Attribution repeats around the evidence and the identities are compared. The
  source vocabulary is the asset's four tags (`session_start`, `session_switch` with its opaque upstream reason,
  `session_branch`, `agent_end`), allowlisted at the doorway and re-checked at admission; `agent_id` stays a rejection
  signal. The session-file header stays a pre-resume file↔id check, not ownership evidence, and a parent lineage field
  never rejects. Supported versions are 18.2.4 and 18.2.6 with equal gate-semantic pins verified at the pinned sources
  (static claim, no runtime probe). Compiled and package-launcher forms have chain-level shape coverage; live lifecycle
  evidence covers the Bun-executed entry. Node execution and unknown wrapper shapes remain refused, as described in
  `website/src/content/docs/docs/harnesses/omp.md`.

  **The instructions pointer.** The same hook carries a second job, added because it costs nothing extra: with
  `--announce` on its injected command line it prints one line on stdout after the identity round trip, telling the
  agent that `$farhelm <request>` means the `farhelm agent` CLI and that `farhelm agent instructions` explains it. Both
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
  `FARHELM_SESSION_TOKEN`, socket path), checked by the supervisor on the unix socket.
- Process-tree ownership (SPEC.md's stop/reap promises): killing the tmux pane is not enough — tmux signals the
  foreground process group, and daemonized descendants escape it. The portable sweep combines the pane's descendant tree
  with environment-marker selection through the platform process API. Every marker selection requires the matching
  `FARHELM_SESSION_ID`. Stop and restart select the agent's `FARHELM_AGENT_ID`; delete selects the whole session,
  including tabs; closing one tab selects its exact `FARHELM_TAB_ID`. Agent selection also retains the existing legacy
  case with neither kind marker. These are process-ownership hints, not authenticated credentials or a promise of
  indefinite historical compatibility. Launch boundaries scrub the opposite kind's marker so nested supervisors do not
  misclassify a new agent as an outer tab's process.

  The sweep sends SIGTERM, allows a short grace, quiesces with SIGSTOP, re-enumerates, sends SIGKILL, and polls for
  confirmed disappearance. PID/start-time revalidation narrows reuse races; it does not make the separate identity read
  and signal syscall atomic or guarantee globally unique start timestamps. `systemd-run --user --scope` cgroup scopes
  layer on top as the Linux hardening (M3): where a functional systemd user manager exists — probed once by actually
  running a trivial transient scope and then showing, killing, and confirming the collection of it, not by `which`, and
  through absolute binary paths so a login shell's `$PATH` cannot substitute what the probe approved — each launch is
  wrapped in its own generation-named scope (audited on systemd 255: the wrapper execs in place, so the pane's process
  tree, exit codes, and liveness checks see exactly the unwrapped shape), the per-launch SELECTION is recorded durably
  as a boolean while the unit name is re-derived from session id plus generation at every use (a stored name would let a
  tampered row aim a kill at another session's unit), and stop kills through the scope first — SIGTERM, the same grace
  the sweep gives, SIGKILL, then confirming the unit was actually collected, because `systemctl kill` returning only
  proves delivery. The sweep ALWAYS runs afterwards as the backstop, and is the whole mechanism where no user manager
  exists — a missing manager never degrades stop below the sweep's guarantees, and neither does a broken one: the
  sweep's verdict is the answer, and the scope's troubles are diagnostic. A wrapper that fails runs before the shim can
  write its exec-failure sentinel, so the supervisor classifies that shape (a launch spec nothing ever consumed, on a
  dead pane, for a scoped launch) as **error** rather than letting it masquerade as a plain exit.

  Terminal tabs also receive separate scopes, named from the session and tab IDs. Delete collects those units both from
  tmux-discovered tabs and independently from the systemd manager using the session-specific tab-unit glob. The second
  source preserves a cleanup handle when tmux no longer supplies tab IDs. A manager enumeration failure is distinct from
  having no usable manager; it can refuse Delete before the portable sweep.

  Containment starts at the agent launch, after login-shell initialization: the cgroup wrapper and the shim's
  environment markers deliberately exclude services started by shell startup files. A detached startup-file service may
  belong to the user's ambient login environment and need not be reaped on session teardown. A child that remains in the
  pane's process tree can still be found by the ordinary descendant walk; this is an exclusion from guaranteed cleanup,
  not a promise that every startup-file child survives.

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

### Checkout configuration and discovery

Schema 27 adds global checkout settings, per-host overrides and a monotonically increasing revision. Effective settings
are read in one SQLite snapshot. The configuration CLI opens only an existing current-schema database; it neither
creates missing state nor migrates a database owned by another running process. Clearing a setting restores inheritance,
whereas an empty hook override explicitly disables it. Configuration stays with the registry row on adoption; history
stays with the installation identity.

`POST /api/github-checkout-preview` asks the target supervisor to expand/canonicalize the configured root and propose
the exact destination, without writing it. Responses include the configuration revision and installation/incarnation
claim, never the hook. Naming scans refuse incomplete results after 100,000 entries. Create verifies this binding and
uses atomic mkdir as the collision authority. Known intent keys reconcile their original snapshot before current
configuration or profile resolution; replacement identities additionally bind the source session and preserve its veto.
The composer reads its checkout folder field from that accepted preview, or from the original binding while reconciling
an ambiguous create. It leaves the field read-only until an explicit existing-folder action changes the destination
draft; the old editable `cwd` seed is never presented as a fresh-checkout path. Preview failures leave the path empty
with an error state rather than displaying the old seed.

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
enters it once, whether the request was structured, raw, or profile-backed. Structured suggestions are projections of
the surviving admission rows, so 100 later raw creates evict an earlier structured setup and remove its frequency
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
the accepted request, not arbitrary supervisor display metadata. Raw fresh creates age the same window and contribute
repository recents; structured history additionally projects the saved selection. Fresh requests retain accepted cwd for
diagnostics but skip the folder projection. Repository setups group by destination kind, canonical repository and
complete selection, independently of their previous ephemeral cwd. Adoption purges the old install's repository
suggestions with its other history; ordinary history rows retain their existing semantics.

Schema 30 adds nullable `remembered_workspace_trust` to the preference row. The helm updates it in the same admitted
create transaction as structured launch history, only for an explicit Codex, Muse, or Pi choice from a user-originated
create. Codex true and false compile to a whole-argv config marker. The supervisor fills it at the shared spawn seam,
after any GitHub checkout has fixed the final cwd, with one `projects.<cwd>.trust_level` override set to `trusted` or
`untrusted`. The cwd is quoted as a TOML key and resolved on the target host; omitted trust adds no Codex override. Muse
true adds `--trust-workspace`; Muse false adds no trust flag and cannot undo trust from `--yolo` or vendor settings. Pi
true adds `--approve` and Pi false adds `--no-approve`, independent of Pi's YOLO tool mode. Unsupported harnesses reject
an explicit trust value. The composer clears that value on a switch to an unsupported harness and restores it from the
preference row on a fresh open or reset. Older selection JSON decodes without a trust choice.

Schema 25 resets schema-24 composer history for the same reason. Schema 24 retained only the timestamp attached to its
sequence eviction cutoff, which cannot be converted into a safe timestamp/ID frontier when sequence and clock order
disagree. Schema 25 records both frontiers from the start; resetting bounded suggestion data is the only honest upgrade
because the missing eviction history cannot be reconstructed.

Ordinary recents and search-result recents group a complete selection by the same per-launch canonical destination.
Legacy launch rows without that fact keep their own spelling distinct instead of consulting a mutable folder projection
or guessing aliases. Frequency sorts descending and the newest retained occurrence breaks ties. An omitted model,
effort, or permission remains the saved harness default and is part of the complete selection identity, not a wildcard
that merges explicit choices.

The composer retains the installation claim when a recent setup or saved folder is applied, not only while offering the
suggestion. A replacement under the same registry row disables Launch and requires an explicit host or folder choice.
The same installation reconnecting keeps the claim valid. Captured history callbacks check the current destination
before changing the draft; submission checks it before and after key minting. New carries the selected session's folder
beside its installation snapshot from AppBody, independently of the filtered sidebar projection.

- State in SQLite at `~/.local/state/farhelm/helm.db`: host registry (SSH destinations, host identities, and optional
  aliases), last-known session cache (survives helm restarts per SPEC.md), the helm-wide profile catalog and its one
  remembered default, recoverable web token, hashed browser device sessions, and the one client preference (list order,
  last-selected session, compact rows) every client shares.
- The `profiles` table is bounded on both axes — 128 stored profiles per helm, 8 KiB of caller-supplied text per profile
  — so the unpaginated catalog reply stays predictably bounded. The helm combines those stored rows with eight
  release-owned Claude Code, Codex, Muse, and Cursor built-ins in its read and resolution paths; built-ins are never
  seeded, persisted, or mutable. The response carries an authoritative `builtin` boolean, defaulting false for stored
  rows and old responses, so the UI need not derive source policy from an opaque ID. Existing stored starters remain
  ordinary editable rows. A profile names its kind explicitly (`generic` means no integration), and an absent resume
  template selects that kind's default. The helm resolves every profile-backed create into an invocation, kind,
  template, and immutable id/name snapshot before the supervisor call. When the template is absent for Claude or Codex,
  the supervisor derives it by retaining the parsed original invocation argv and appending that kind's resume arguments;
  the argv is captured before per-launch Farhelm hook injection. This deliberately assumes original arguments are
  reusable and has no parser for initial prompts or launch-only options. It also resolves every supervisor
  `SourceProfile` marked `Unresolved` against one catalog read per reply before browser JSON or session-cache storage;
  missing ids become `Deleted`, and ids whose current names differ from the snapshot become `Renamed`. Profile writes
  are last-write-wins and carry no definition fingerprint. Muse's two definitions explicitly select `generic` with no
  resume template. They use the ordinary terminal launch path and generic activity classifier, without per-agent hooks
  or conversation-identity capture. OpenCode is a structured harness only: its release catalog holds verified Zen model
  IDs, accepts an omitted model for OpenCode's configured default, passes explicit bare custom Zen names as
  `opencode/<model>`, and maps YOLO to OpenCode's `--auto` flag. Its empty effort vocabulary, generic activity
  classifier, and absent resume template deliberately avoid claiming a provider-specific effort or conversation
  lifecycle contract. Cursor likewise maps its structured harness to the existing Generic kind, with no resume template
  or capture machinery. Its two release-owned profiles invoke `agent` and `agent --force`; models use `--model`, with no
  separate effort flag. The UI preserves its harness in launch intent while explicitly disclosing the lack of tracking
  and Resume. Protocol 27 adds the Cursor harness variant, not a new runtime integration kind. Protocol 29 adds Grok as
  both a structured harness and a durable agent kind. Its compiler emits `grok --no-leader`, maps YOLO to
  `--always-approve`, refuses model and effort choices, and stores
  `grok --no-leader [--always-approve] --resume {conversation}` as argv elements. The dedicated kind preserves the
  ownership policy across helm and supervisor storage. It uses the generic activity classifier while the manually
  configured reporter supplies ownership-proven conversation capture. OMP is also a structured harness only at launch
  time: its release catalog holds the same OpenRouter model IDs as Pi's, omits model and provider flags for the harness
  default, and compiles an explicit model as `omp --provider openrouter --model <id>` (provider intent explicit; a
  literal custom id stays one argv element and is stored verbatim — provider qualification is not a promise of literal
  upstream routing for unknown ids; OMP's own resolution still runs alias, fuzzy, and `:suffix` interpretations on the
  id it receives, as documented in `website/src/content/docs/docs/harnesses/omp.md`). `--thinking <effort>` carries the
  seven-level list (`off` through `max`; OMP's `auto` is not offered), and `--approval-mode yolo|always-ask` carries the
  YOLO/Approve choices while `default` adds no flag and stays omitted in the stored selection — unlike Pi, no YOLO
  default is rewritten on. SmartApprove and Chat are refused for OMP; the row glyph is the Greek capital omega, chosen
  so it cannot read as Pi's "P" at sidebar size. Resolving `SourceProfile` snapshots while draining remote sessions
  discovers catalog state only: those observations never select the helm-wide remembered default. A successful
  profile-backed create through the user's REST surface alone writes that default; agent-relay creates and clones do
  not, so an agent's work cannot change the profile the user's next dialog suggests.
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
- A connected host's cache refresh is one request and one replacement: a single `ListSessions`, whose reply carries the
  host's whole list, then that host's whole cache slice replaced in one identity-bound write. One request per refresh
  matters beyond round-trip count, because the supervisor's conversation-capture sweep rides the `ListSessions` handler,
  so every request is a whole-host scan on the far side. A failed refresh records the failure and keeps the previous
  cache, never wiping it: the cache's whole job is to answer "what did this host have, last we knew" while the host is
  unavailable, so clearing it on failure would destroy the answer exactly when it becomes the only one available, and
  would make a transient failure look identical to "this host genuinely has no sessions". A host whose supervisor
  reports no identity at all connects and serves live but writes no cache, since the identity binding has nothing to
  bind to. The reply is checked at ingress and refused whole — an ordinary failed refresh that keeps the previous cache
  — when it is longer than `LIST_SESSIONS_CAP` (a peer ignoring the one bound on what this side retains), when a session
  id exceeds the id length cap, or when an id appears twice; a reply cut AT the cap is accepted and remembered as
  truncated, which is what the served list's own `truncated` flag carries forward.
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
- The order a supervisor lists in is neither validated nor relied on. The helm sorts the union of every host's rows —
  cached and in-memory alike — per request, so an unsorted or differently sorted reply costs nothing and the one host
  that serves from memory rather than from the cache needs no special handling. Session ids are still bounded at every
  peer ingress, for a reason that survived the cursors: every later request naming the session embeds its id in a frame
  head, and an id near the frame limit is one no such request could carry.
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
  always fetched live — the cache exists for the stale list, not as a general serving layer. The live path reads the
  owner's whole list, which is the only list the wire serves.
- `POST /api/sessions/{id}/replace` (SPEC.md's "replace") is a composition of the create and delete this section already
  describes, not a third code path: it routes the source by owner lookup exactly like any other lifecycle operation,
  reads the source's row live from the owning host (the same live read `clone_for_agent`'s agent-CLI clone performs, and
  for the same reason — the cache is for the stale list, never a source for a fresh mutation), derives a `CreateMode`
  from that row through a function (`sessions::mode_from_source`) shared with `clone_for_agent`, then calls
  `do_create_session` followed by a delete, reusing the ONE `(claim, client)` pair the owner lookup produced for both
  calls rather than re-routing before the delete. The shared derivation takes a fallback policy as an argument because
  the two callers disagree about what a dangling snapshotted profile should do: the agent-CLI clone refuses it (a raw
  invocation may name a binary absent on another machine), while replace falls back to the source's raw invocation,
  because replace never changes machine — the refusal clone needs has no failure mode to guard against here. The create
  half carries the same idempotency-replay veto `clone_for_agent` gives its own create: a same-host replace with no
  overrides can reconstruct the source's own creation fingerprint, so a caller reusing the source's key would otherwise
  receive the source row BACK as the "replacement" — this route refuses that reply with `Conflict` before any
  bookkeeping runs, rather than deleting the session it was just handed back. Create runs before delete so a failed
  create leaves the source untouched. A delete that fails AFTER a successful create is reported one of two ways
  depending on whether the failure is a DEFINITE answer: an explicit supervisor refusal, or a delete that never reached
  the wire at all, means the source was not removed, and the reply names both ids and says both sessions still exist; a
  delete that reached the wire and then lost its answer (the connection dying after the frame was sent, or any other
  post-send ending this client cannot interpret) is NOT definite — the supervisor may have completed it — so the reply
  instead says the replacement exists and that the source's fate is unknown and must be checked before deleting it again
  or retrying. Neither shape ever rolls the create back (killing an agent the caller just asked for) or claims success
  (hiding a session, or an uncertainty, the caller needs to see).
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
  only the device secret's SHA-256 hash, and rotation deletes all device credential rows, rejecting their use on new
  requests. Already-admitted HTTP requests may finish. The current implementation also closes terminal and event-feed
  sockets on rotation; SPEC.md permits either closing or retaining those existing connections, so preserving that
  behavior is not a reason to add cancellation machinery elsewhere. This deliberately gives up HttpOnly: script
  execution in the authenticated origin can read the secret, but such a script can already drive the same API, while
  port scoping prevents an unrelated loopback service from receiving an ambient host-scoped credential. The loopback
  Origin guard remains defense in depth; no ambient browser credential remains, so this flow has no CSRF edge.
- The native app embeds farhelm-helm in-process; the Linux helm is the same code behind `farhelm helm run`. The local
  supervisor is a separate process either way — the app discovers one that already answers and leaves it alone, or
  starts `farhelm supervisor run` from its sibling binary and owns that child for its own lifetime.
- Per-session "seen" state (the idle dot's grey/blue split and the read/unread toggle; SPEC.md, Status) lives in its own
  `session_seen` table keyed by session id alone, not as a `session_cache` column: that cache is replaced WHOLESALE by
  each host's refresh, so a column there would need every refresh to carry forward a viewer fact the supervisor never
  reported, and a bare id key (no foreign key to `hosts`, no `ON DELETE CASCADE`) is what lets the state survive a
  retarget or an adoption, both of which keep the session. The stored value is the ACTIVITY STAMP that was current the
  last time some client had the session open, never a wall-clock "seen at" time — comparing a later activity stamp
  against an earlier one from the SAME host involves no clock at all, where a wall-clock comparison would pit the
  session's host's clock against the helm's own, on a remote host a different machine's clock entirely (the same reason
  the relative-age column reads an activity stamp rather than a "last seen" timestamp). One caveat follows directly: the
  supervisor quantizes `last_activity_at` at the source, advancing it only when what it observes is at least a minute
  newer than what it already holds, so output landing within that minute after a mark-read does not yet register as
  unseen — cosmetic, and not worth a second, finer-grained stamp. The table is a helm-local write with nothing to
  refuse: `PUT /api/sessions/{id}/seen` does not route through a session's owning host at all (unlike every lifecycle
  verb above), so a session on an unreachable host can still be marked read or unread, and the write bumps the
  fleet-events revision only when the stored value actually changed — a client re-marking the SAME stamp it already
  recorded — which happens when a session is reopened with no new activity behind it, or when a client retries a PUT
  whose response it missed — must not wake every other connected client to redraw a dot that has not moved. Deleting a
  session drops its `session_seen` row explicitly, since nothing else cascades into a table with no foreign key; a
  session deleted through ANOTHER helm, or dropped from a cache because its host was removed here, can leave a row
  behind, which is accepted as garbage bounded by the number of sessions that ever existed, at a few dozen bytes each.

## Standalone uninstall

`farhelm uninstall` establishes ownership from the installer's adjacent NUL-separated receipts before confirmation. The
receipts name the physical installation directory and SHA-256 digests; deletion targets come from fixed paths in the
code, never arbitrary receipt fields. Payloads and receipts must be regular files owned by the current user, and
receipts must not be group or world writable. App contents are checked against the fixed bundle layout. An app-local CLI
invocation directs the operator to the flat CLI so bundle removal cannot delete the retry command prematurely.

Removal is ordered to leave the flat CLI until the other required work succeeds. Already-absent payloads are accepted
when the surviving receipt still proves ownership. During final bundle-directory removal, the validated app receipt is
published without replacement at `~/Applications/.Farhelm.app.uninstall-receipt`. This preserves retry authority after
the internal receipt is deleted. If both receipts survive, they must agree. The adjacent receipt is deleted after the
bundle; flat receipt cleanup after CLI deletion is nonfatal and reports any retained metadata. No recursive deletion or
rollback is needed.

On Linux, setup's existing managed-unit parser and removal machinery select services whose recorded executable resolves
to this installation. Custom and other-installation units, drop-ins and linger remain untouched. Services are disabled
and stopped before their unit files or executables are removed. The operation does not examine effective overrides or
processes. Manual shutdown and excluding concurrent install, update, setup and startup remain operator prerequisites.

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
  command neither initializes missing state nor migrates an incompatible database, and never creates target directories
  or runs the hook.
- `farhelm helm run` — run the helm (flags: `--port`, `--state-dir`, `--ui-dist`, `--ensure-hosts <file>`,
  `--payload-dir <dir>` (env `FARHELM_HELM_PAYLOAD_DIR`), `--release-base-url <url>` (env `FARHELM_RELEASE_BASE_URL`)).
  The last two select where "add host" provisioning payloads come from — an operator-staged directory (verified not at
  all, D3) or a download source other than the default GitHub release (D2); `--payload-dir` wins if both are given
  (D18). It takes no session or transport flags: M1's `--ssh`, `--cwd`, `--agent`, `--title`, `--remote-farhelm`, and
  `--remote-state-dir` were dropped with M6's registry (user decision 2026-08-04). A helm drives every registered host
  at once, so a flag naming one of them could only ever have meant the wrong thing; the last two live on as per-host
  registry fields, and creation is `POST /api/sessions`, which is where the host selection belongs. A release build
  compiles its own web UI in (`FARHELM_UI_DIST` at build time); `--ui-dist` still overrides it at runtime, and an
  ordinary developer build with neither serves the API alone.
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
- `farhelm spawn --cwd <dir> (--agent <name> | --profile-id <id> | --inherit-agent) [--title ...] [--parent ...]
  [--idempotency-key ...]`
  — the in-session spawn CLI from SPEC.md.
- `farhelm agent hosts|sessions|profiles [--json]` — the in-session ASKING CLI from SPEC.md, on the same injected
  credential spawn uses. It prints an aligned table on stdout, `*` marking the asking session and its host, and puts a
  refusal on stderr with a non-zero exit exactly as spawn does. Human output is a table because the reader is usually a
  model quoting its own shell output. The JSON form uses schema version 2, includes exact IDs, caller identity, and
  completeness fields, and omits invocation arguments, credentials, resume templates, and provider configuration. A
  session row's non-secret `OFFER` capability is the exact mode its restart command may request; it does not disclose
  the template, captured conversation locator, or a live-stop recommendation. Every dynamic table cell is escaped to one
  printable line and every non-final column is capped at 48 characters: these values are fleet-wide user text printed
  straight to a terminal, so a raw newline forges a row, an ESC drives the terminal, and one long title would otherwise
  be padded onto every other row. A cut listing prints its rows on stdout and one warning on stderr, so a script
  capturing stdout still gets nothing but the table. It has no timeout of its own: the supervisor bounds the relay and
  is the only party that can distinguish its two failures (see the transport section's version-20 paragraph).
- `farhelm agent rename --session <id> --expected-title=<old> -- <new>`, `farhelm agent stop --session <id>`, and
  `farhelm agent restart --session <id> --mode <resume|fallback-template|fresh>
  [--stop-if-running]` — the in-session
  ACTING CLI, on the same relay and credential. Every target is explicit, including a deliberate self-action. Rename
  compares the observed title and changes it atomically in the owning supervisor; a mismatch is a conflict with no
  mutation. Success prints one plain confirmation line on stdout (`renamed <id> to "<title>"`, `stopped <id>`,
  `restarted <id>`), its dynamic cells run through the same escaping the listing tables use, so a scripted caller gets
  exactly one line rather than a table with one row. Restart forwards the mode and consent unchanged to the owning
  supervisor, which rechecks both current offer and liveness; the CLI never infers consent from discovery. An explicit
  self-stop or self-restart may terminate the CLI before its line is printed because it belongs to the process tree
  being ended. Self restart prints its interruption/outcome-unknown warning before dispatch and treats a lost reply as
  unknown rather than success.
- `farhelm agent create --host <name> --cwd <dir> (--profile <name> | --profile-id <id> | --invocation <cmd>) [--title ...]
  [--idempotency-key ...]`
  and `farhelm agent clone --source-session <id> --host <name> [--cwd <dir>] [--title ...]
  [--idempotency-key ...]` —
  the in-session CREATING CLI, on the same relay and credential. These invert the stream convention the lifecycle verbs
  follow: stdout is the new session's id and nothing else, matching `farhelm spawn`'s contract, with one confirmation
  line on stderr (`created <id> "<title>" on <host> in <cwd>`, escaped the way the listing tables escape their cells).
  The id is the one agent output meant to be captured as a SINGLE VALUE — an agent takes it and hands it back as
  `--session` — so a confirmation on stdout would make the two verbs that need parsing the two that cannot be. The
  listings are parsed too (the relay fixture reads the hosts table, and `docs/old_readme.md` shows the same), but they
  are parsed as TABLES: a table that grew a column is still a table, while an id that grew a sentence beside it is not
  an id. The stderr line is written through a fallible `write!` whose result is discarded rather than through
  `eprintln!`, because the macro panics on an unwritable stderr and the session already exists by then — turning a
  create that succeeded into a command that failed would tell a caller holding the id to retry a create it must not
  repeat.

  `--host` takes a NAME from `farhelm agent hosts`, printed there WHOLE: the NAME column is exempt from the truncation
  every other non-final column takes, because that column is a selector rather than a description and a name cut at 48
  characters is a host an agent can see and can never target. Duplicate names remain separate rows and are refused as
  ambiguous targets. `--cwd` and `--host` are required on `create`; `--source-session` and `--host` are required on
  clone. The three create selectors are mutually exclusive and exactly one is required, refused by clap before anything
  is sent (the helm refuses malformed wire shapes too). Profile IDs use exact ID lookup without name fallback. Every
  value-taking option on both verbs carries `allow_hyphen_values`, because every one of these values is judged
  downstream — by the registry, by the helm's catalog, by the target filesystem — and every one of them may legally
  begin with `-`; refusing such a value locally would be this CLI declining to carry a name the far end would have
  explained.
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
supervisor puts its own binary on the session PATH). clap-derive because it is the standard and keeps the grammar
declared next to the types.

## Native app packaging

Dioxus desktop (wry) wrapping farhelm-ui, shipped as a BARE BINARY rather than a `.app` bundle: `farhelm-desktop`, a
thin crate (`crates/farhelm-desktop`) whose `main` is one call into farhelm-ui's desktop module, built by cargo-dist
alongside `farhelm` and installed next to it. The two are one artifact pair, not one bundle — the shell embeds
farhelm-helm from the same workspace version and reaches supervisor code by finding its CLI sibling next to its own
executable, discovering a local supervisor that already answers or spawning `farhelm supervisor run` when none does.

On top of that pair, `install.sh` assembles `~/Applications/Farhelm.app` on macOS: an Info.plist (bundle identifier
`org.scode.farhelm.desktop`), the icon shipped in the desktop archive, and COPIES of both committed binaries in
`Contents/MacOS/` — where the executable keeps the name `farhelm-desktop`, because the default case-insensitive APFS
would make an executable named `Farhelm` the same directory entry as its required `farhelm` sibling. The bundle exists
for LAUNCHER IDENTITY only: Spotlight/Alfred launchability, a Dock icon, a Cmd-Tab name, and Launch Services'
single-instance activation (relaunching activates the running app instead of racing it for the embedded helm's state).
It is a derived artifact rebuilt wholesale by every install run — the flat pair stays the source of truth and the only
state the installer's transaction journal covers — and nothing in the app reads it: asset serving stays the embedded
tree below, and the sibling contract is satisfied inside `Contents/MacOS/` exactly as it is in `~/.local/bin`.
`FARHELM_NO_APP_BUNDLE=1` skips it.

The dx-produced bundle went away because a bare binary has nowhere to put a `Resources/` directory, and Dioxus's
`asset!()` files were the only thing that needed one. They are served instead from the UI tree compiled into
`farhelm-helm`, through a `dioxus-desktop` asset handler registered on `/assets/*`, handed to the webview over the
`dioxus://` scheme. By default — and in every release build — those are the same bytes the helm serves to a browser,
since both read the compiled-in tree; `FARHELM_DESKTOP_UI_DIST` breaks that identity deliberately, pointing only the
loopback helm at a directory on disk while the window keeps rendering from the embedded tree. Registering a handler for
a path prefix takes precedence over dioxus's own filesystem resolver, so there is no bundle-directory fallback at all;
the price is that the desktop build's asset set and the web bundle's must be identical, which
`scripts/check-desktop-assets.sh` enforces on every change.

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

## Provisioning

Implemented in the helm over the same system-ssh access: sftp the cross-compiled `farhelm` binary (plus a private static
tmux build when the host has no tmux) into `~/.local/lib/farhelm/`, write user-level systemd units,
`systemctl --user enable --now`, `loginctl enable-linger` as the optional-step (proceed-without-if-privileged per
SPEC.md). Discovery-first: probe for a running supervisor via `farhelm internal stdio` before proposing any of this, and
show the full concrete action list before touching the host for initial setup.

ADD and UPDATE both retain that concrete plan behind an opaque, one-use confirmation id. Planning is inspection-only;
consuming the id revalidates the host and registry facts the plan relied on, and only then admits the host-scoped run.
Setup shows the rendered plan and consumes the id on explicit confirmation; a remote update consumes it automatically
after the user's Update click, which is the authorization — the wire keeps the same single-consumption authority either
way, and the UI never replays a spent token. Discovery records the resolved supervisor binary, state directory, and
identity together so a later helm dials the same installation that answered the probe. UPDATE starts from those recorded
coordinates rather than assuming the standard layout.

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
keeps its old inode until the explicit supervisor restart. Hash checks skip identical payloads and unit files are
written only when their content differs, so rerunning provisioning converges from wherever an earlier run stopped.
Remote plans report binary upload and installation as separate actions. Upload verifies the nonce temporary's digest;
installation checks it again before the atomic rename. Both actions use the same staged payload snapshot, and local
plans retain a single install action because they do not transfer over the network. Matching content also repairs
installed-file mode drift. Provisioning may create directories with explicit modes and repair permissions on directories
dedicated to Farhelm; the supervisor state directory is private to its user (`0700`). Existing shared directories,
including a shared executable directory or the systemd user-unit directory, must retain their permissions. If those
permissions prevent installation, report the obstacle rather than changing them. This ownership restriction is
maintainer-confirmed policy; existing provisioning paths still require assessment against it.

The supervisor unit uses `KillMode=process`. Sessions started through Farhelm belong to the private tmux server that the
supervisor launches, so systemd's default `control-group` policy would kill that server and every session whenever an
explicit UPDATE restarts the supervisor. Limiting the unit stop to its main process preserves the same ownership model
as running `farhelm supervisor run` manually: stopping the supervisor detaches management, while tmux continues to own
the session processes and terminals until the user deletes them or the host reboots.

Motivation for shipping tmux ourselves when absent or too old: apt needs root, and SPEC.md forbids requiring it; a
static tmux under our own lib dir keeps the no-root promise without asking the user to install anything.

The provisioning payloads — linux-musl `farhelm` binaries for both architectures plus the static tmux builds — are no
longer embedded in the helm's own distribution (D2). This REVERSES the earlier "provisioning must work with no
third-party downloads" posture: a release-shaped build (D13 — one that embedded the web UI) downloads them, on demand,
from the GitHub release matching its own version, verifies them, and caches them under helm state before pushing them
over SSH exactly as before. A developer build defaults to no payloads at all (`NoPayloads`, D13) rather than to a
download, and `--payload-dir <dir>` (env `FARHELM_HELM_PAYLOAD_DIR`) selects an operator-staged directory instead —
files in an explicitly selected local directory are treated as operator-trusted and are not verified, on ANY build,
developer or release. The downloading source is `ReleasePayloadSource` (`provisioning/release_payloads.rs`), which
caches one release's assets, their extracted binaries, and the signed checksum file under
`<state_dir>/payloads/v{version}-<first 12 hex of sha256(base_url)>/` — keyed by version so an upgraded helm never
reuses the previous release's binaries, and by base URL so a mirror or test server can neither read nor poison what the
real release wrote. Motivation for reversing the embedding: bundling every target's binaries inside every platform's own
artifact cost tens of MB per download and coupled a payload's presence to whichever platform happened to embed it; a
download keeps the size cost where it belongs (paid once, by the host actually being provisioned) while keeping the same
"provisioned host runs exactly what the provisioning helm expects" property, because the default download always names
the provisioning helm's own version.

Verification chain (D3): CI writes one `SHA256SUMS` covering the six release binaries/archives and signs it with an
unencrypted minisign secret key held as a repository secret, passing `-t "farhelm $TAG"` so the release version lands in
the signature's trusted comment; the `farhelm` binary embeds the matching public key and refuses any download whose
`SHA256SUMS.minisig` does not verify, whose trusted comment names a version other than the helm's own, or whose per-file
SHA-256 does not match. The trusted comment is not decoration: the signature otherwise authenticates only the CONTENTS
of `SHA256SUMS`, which name no version, so whoever serves the release URL could replay an older release's valid manifest
and assets at a newer version's URL and downgrade every host that helm provisions. A release tag is `vX.Y.Z` and already
carries the `v`, so the comment is `farhelm` plus the tag verbatim: signing without `-t`, or with a second `v`
(`farhelm v$TAG` → `farhelm vv1.2.3`), produces a release no helm can install. `--payload-dir` is the one path that
skips all of this — nothing there is downloaded, so nothing there is checked. SPEC.md's "no public relay, no third-party
rendezvous service" line still holds: GitHub is a download source the helm's own machine reaches directly, never a relay
or rendezvous point sessions or connections pass through.

Release signing key. The key pair behind that chain is the project's one long-lived secret, and its handling is
deliberately minimal. The public half is committed twice — `MINISIGN_PUBKEY` in `release_payloads.rs` and
`crates/farhelm-helm/src/provisioning/farhelm-release.pub`, with a test that they agree. The secret half exists only as
the `MINISIGN_SECRET_KEY` repository secret: it was generated locally, stored with `gh secret set`, and the file
destroyed; it is never committed, never printed, and never present on a developer machine. Only the `sign` job of
`sign-sums.yml` receives it, after a secretless `validate` job has already checked the assets, so the generated dist
workflow and the build jobs never see it. Neither minisign keys nor repository secrets expire; rotation happens when the
maintainer chooses. Rotating is one PR: `minisign -G` a fresh pair, `gh secret set MINISIGN_SECRET_KEY` from the new
secret file, shred it, replace both committed copies of the public key, and cut the next release. Nothing in the field
notices, because under D2 a helm only downloads the release built from the same commit as itself, which is signed by the
key that commit compiled in; old helms keep verifying their old releases with the old key. The one future feature that
changes this is a cross-version download such as an auto-updater: it would verify the next release with the key it
already carries, so a rotation would then need a transition release signed by the old key but carrying the new one.
Sequencing rotation before shipping such a feature, never in the same release, is the whole rule. Note also what the key
does not protect: `install.sh` runs on a machine with nothing to pin a key in, so installing trusts GitHub over TLS and
the `SHA256SUMS` served beside the archive; the signature guards what a running helm provisions onto other hosts, not
the first download of the helm itself.

A release also carries cargo-dist's own metadata, none of which is signed and none of which Farhelm reads:
`dist-manifest.json`, a `<archive>.tar.gz.sha256` beside each of the four archives, and a lowercase `sha256.sum` over
what dist built. That last one is worth naming explicitly because it looks like the file that matters and is not it:
`SHA256SUMS` — uppercase, six entries, the one `SHA256SUMS.minisig` authenticates — is what the helm and `install.sh`
verify against. The metadata is nonetheless part of the release contract rather than incidental: the signing job
REQUIRES the manifest and the four per-archive checksums to be present as stable, machine-readable metadata for
downstream tooling, and treats `sha256.sum` as optional. Homebrew distribution remains deferred, with no publishing
model selected. Anything else appearing on a release fails it, so no published asset can sit outside both the signed set
and that list.

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

## Version and skew

One version number across the workspace; the protocol hello carries protocol and build versions; incompatibility refuses
with a clear error at the edge (helm↔supervisor connect, client↔helm load) per SPEC.md. Once a hello is compatible, the
helm compares the peer and its own build strings as semantic versions for an advisory age signal: prerelease ordering
applies, build metadata does not change precedence, and an unparsable value leaves age unknown and false. That signal
adds `old_version` to the connected REST state, without changing the `connected` phase or operational routing; only the
incompatible protocol case is displayed as `needs update`. Protocol version bumps with any incompatible change — which
includes a field whose omission changes what the receiver DOES, not only changes to frames and message sets. A
serde-additive field can still be semantically load-bearing: the non-displacing attach is the worked example (a peer
that ignores it displaces a client it was asked to leave alone, silently, on both ends), and decode tolerance is why
such a bump is required rather than why it is unnecessary.

The client↔helm edge has no hello to refuse at, so the helm stamps its build on every reply and the UI compares it
against the one compiled into its bundle. A mismatch — including a helm that reports no build at all — surfaces a reload
prompt and, more importantly, withdraws every UNATTENDED behavior that depends on the helm honoring this milestone's
vocabulary: the terminal heartbeat and automatic reconnect both stop, while anything the user explicitly asks for keeps
working. The sidebar's app bar shows the helm's build at all times: the client's own compiled build until a mismatching
stamp is reported (agreement means the two are the same string), and the reported stamp from then on. The bar leads with
the Farhelm wordmark, inlined at compile time from `packaging/farhelm-desktop/wordmark-dark.svg` (the brand file every
use of the name as a mark shares) rather than served as an asset, so both the web bundle and the desktop build carry it
without a desktop asset-parity entry.

### Restart-with backend wire and persistence

`RestartSession` optionally carries the compiled structured launch bundle: `invocation`, `launch`, and
`resume_template`. Protocol 30 adds these fields; the exact-version handshake refuses an older peer, because one that
ignored them would relaunch with the old settings and still report success. A selection the catalog refuses is a 400
from the helm and never reaches the supervisor. The helm compiles a supplied `LaunchSelection` and passes those fields
through without checking cached offer or harness state; attached-session relay calls omit them. The supervisor
revalidates the current stored structured selection, fixed harness, `Resume` offer, and `Resume` mode immediately before
destructive work, then resolves and fills the supplied template using the same executable and integration checks as
create. After the new process spawns, one generation-fenced store write updates invocation, launch, and the resolved
resume template while leaving the integration kind and working directory fixed. A compiler-omitted template is resolved
before this write, so the saved row retains the concrete template required to resume. A spawn failure leaves the prior
bundle untouched. If the post-spawn write fails after an otherwise successful relaunch, the restart still reports
success because the new process is already running; the failure is logged, and the reply and live session retain the old
stored settings. A relaunch that published its new process but hit an independent cleanup or reply error still attempts
the bundle write and retains that error reply. A later restart uses the saved settings, or the old settings if the write
failed, as it would after a crash between spawn and the write. This is the only exception to the ordinary create-time
immutability of those launch columns, and the fixed kind avoids PATH-dependent kind re-derivation.
