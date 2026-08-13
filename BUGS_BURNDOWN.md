# Bugs burndown

Preliminary triage for reported bugs, written so a later session can fix each one unattended. Each entry records the
symptom, the mechanism as verified in the code (file:line references checked at triage time), the product decision where
one was needed, and a fix sketch with the tests that pin current behavior.

Entries are ordered by suggested fix order: the two small independent cwd items first (2 depends on 1), then the
supervisor-heavy tab work, and the layout redesign last — it touches everything the tab fixes' UI side lives in.

Environment note: all bugs below were observed in the desktop app against the "this machine" host on macOS. Nothing in
the triage suggests any of them is platform-specific.

## 1. Working directory field rejects `~`

Status: fixed in PR #152 (`pr/tilde-cwd`).

Symptom: creating a session with working directory `~` fails with "working directory is not absolute: ~ (a relative path
would resolve against the supervisor process, not the client)". `~` should be accepted, resolving to the home of the
user running the supervisor on the target host — that semantic is expected and is the point.

### Mechanism

The refusal is supervisor-side: `ensure_cwd_usable` (`crates/farhelm-supervisor/src/service/core.rs:2009`) requires an
absolute path. Its docstring (:1997-2008) explains why relative paths are rejected: an accepted relative cwd would be
stored durably and handed to tmux, which resolves it against the supervisor daemon's cwd, so its meaning would drift
across daemon restarts — a real past failure mode. `~` is caught by that net even though it has a perfectly stable
meaning.

### Fix sketch

Expand `~` and `~/...` to the supervisor process's home at request time, before validation and before storage, so what
is stored durably is the expanded absolute path — this preserves the anti-drift rationale untouched (expansion happens
exactly once, at create).

- Expansion must be supervisor-side, not helm/UI-side: the helm runs on the user's machine, and expanding there would
  substitute the wrong home for remote hosts.
- Entry points: the create paths that receive a fresh cwd from a client, `service/core.rs:4767` and `:4963`. Restart and
  tab-open re-check the STORED cwd (`core.rs:6300`, `:8109`), which is already absolute after create-time expansion — no
  change needed there.
- Home source: the supervisor's own environment. Keep it injectable — the repo rule forbids tests that mutate process
  env, so thread home in as a parameter rather than reading `$HOME` at the check site.
- `~user` forms: reject with a clear message naming the supported forms (`~` and `~/path`). Expanding other users' homes
  is a separate feature nobody asked for.
- A bare `~` must expand to home itself, not `home/`; `~foo` (no slash) is a `~user` form, not `~` + garbage.
- Amend SPEC.md's working-directory rule (SPEC.md:141, "named by an absolute path") in the same PR: `~` and `~/path`
  accepted, expanded once at creation against the target host's home, expanded absolute path stored, `~user` refused.

Tests: cwd validation coverage lives around `crates/farhelm/tests/e2e/session_lifecycle.rs:1100`. Add: `~` and `~/sub`
accepted with the stored cwd expanded (visible via session info), `~otheruser` rejected, plain relative paths still
rejected. The CLI `farhelm spawn --cwd` path (SPEC.md:382) flows through the same create handler and gets this for free.

## 2. Default the new-session working directory to `~`

Status: fixed in PR #153 (`pr/tilde-default`), stacked on issue 1's expansion.

What the user wants: `~` as the default working directory for a new session, so the field only needs touching when a
different directory matters.

Fix sketch: prefill the working-directory input in `CreateSessionForm` (`crates/farhelm-ui/src/list.rs:2007`) with `~`
instead of empty — confirmed with the user (interviewed 2026-08-13) over the blank-means-`~` placeholder alternative:
the field visibly contains `~`, and what is sent is always what is shown. This keeps the supervisor's contract
unchanged: cwd is always explicit, and issue 1's expansion makes `~` valid. Mention the default in SPEC.md's creation
section alongside issue 1's amendment.

Two consumers of the form assume the field starts empty and need auditing in the same change:

- `scripts/desktop-smoke.sh:510` clicks the field and TYPES an absolute path — xdotool types at the caret rather than
  replacing, so a prefilled field would submit `~<absolute-path>` and fail the mandatory smoke gate. Select-all before
  typing.
- Playwright fills the form through `fillCreateForm` (`e2e/tests/terminal.spec.ts:278`), which uses `fill()` — that
  replaces content and is safe as-is; audit any `type()`/`pressSequentially` stragglers.

## 3. Terminal tab stays open (and silent) after its process exits

Status: fixed in PR #154 (`pr/tab-reap`).

Symptom: open a terminal tab, hit ctrl-d. The shell exits but the tab stays in the strip looking exactly like a live tab
— no exit indication of any kind — and the user has to close it manually via the x. The user wants the tab to go away by
itself when the process exits.

### Mechanism

The current behavior is deliberate and specified, not an accident:

- `remain-on-exit on` is set server-wide (`crates/farhelm-supervisor/src/tmux.rs:1531`), so the tab's window survives
  its shell's death as a dead pane. tmux's control protocol carries no pane-death notification (`tmux.rs:3687`), so
  nothing event-driven fires anywhere.
- Tabs are rediscovered from window markers on every listing; `tabs_from_pane_states`
  (`crates/farhelm-supervisor/src/service/terminals.rs:190`) filters on session/marker only and never reads
  `PaneState.dead` — even though the very query it consumes already fetches `dead: bool` and `exit_code: Option<i32>`
  (`tmux.rs:1172`, `:1235`). Listing-time `PaneState.dead` is what nothing consumes; pane deadness as such IS consulted
  elsewhere — the dead-at-open refusal (`service/core.rs:8283`) and the manual close/reap path, which checks whether the
  pane is dead before using its PID as the process-tree anchor. The reap must compose with that existing close flow, not
  duplicate it.
- `TabInfo` is deliberately just `{ id }` (`crates/farhelm-proto/src/lib.rs:1294`), so no liveness bit reaches the helm
  or UI, and the UI's `Tab` mirror (`crates/farhelm-ui/src/lib.rs:734`) is equally bare.
- Contrast with the agent pane: the same `pane_states()` poll drives `session_status` (`service/status.rs:187`), which
  classifies `dead` into `SessionStatus::Exited { exit_code }`. The agent's deadness is classified; the tab's is
  discarded.

### Spec conflict — resolved by user decision 2026-08-13

PLAN_M4.md explicitly chose the current behavior ("a shell that starts and later exits is just a dead pane … established
tabs have no error-vs-exited story to tell", PLAN_M4.md:68; same at :158), and SPEC.md calls close "the whole per-tab
operation set in v1" (SPEC.md:206). Two tests pin it:

- `crates/farhelm/tests/e2e/tab_lifecycle_edges.rs:91` (`a_tab_whose_shell_exited_stays_listed_replayable_and_closable`)
- `e2e/tests/terminal.spec.ts:7364` ("a tab whose shell exits stays listed with its scrollback readable")

The user's directive overrides that stance: a tab whose process exits should be reaped automatically. The fix must amend
SPEC.md's tab lifecycle wording and rewrite both pinning tests to assert auto-removal instead. PLAN_M4.md is a
historical build-order doc; leave it alone.

NOTE the accepted tradeoff, so nobody relitigates it mid-fix: auto-close discards the dead pane's scrollback. A command
that prints output and exits takes its output with it. That is what the user wants for tabs (the agent pane keeps its
exited-with-scrollback behavior — session exit classification is untouched).

Decision (interviewed 2026-08-13): ANY exit removes the tab silently — no transient notice, no exit-code badge, no
special treatment for nonzero exits. The scrollback-loss tradeoff above was explicitly offered and accepted in that
form. Scope: silent removal applies to tabs whose OPEN succeeded. The dead-at-open refusal is untouched — a shell that
dies before the open reply still fails the open loudly with the pane's last words, exactly as today.

### Fix sketch

Supervisor-side reap, so the tab disappears from listings for every client rather than being a UI trick:

- When a tab pane is observed dead, kill its window. The 2s ticker (`service/ticker.rs:169`) is the home for the side
  effect: it already polls `pane_states()` but currently filters tab panes out (`ticker.rs:774`), and listing paths are
  read paths that should stay side-effect-free. Do NOT count on "every `SessionInfo` reply probes panes" for coverage —
  that claim is overstated (a successful create returns a placeholder `SessionInfo` without a pane probe, and the reply
  builders differ per path); inventory the actual probing paths during implementation, and let the ticker be the
  guarantee. Reuse the close/reap flow (`close_tab_window`, `service/core.rs:8417`) rather than a bare kill-window so
  scope teardown happens too.
- Either is enough on its own for correctness, but also skipping dead panes in `tabs_from_pane_states` makes the listing
  honest during the window between death and reap.
- No new protocol or event: the helm caches the serialized `SessionInfo` per session and bumps the revision feed when it
  changes (`crates/farhelm-helm/src/store.rs:3166`, `events.rs`), so the tab vanishing from the list wakes every open UI
  automatically.
- UI: verify selection falls back sanely when the selected tab disappears from the server list (`session_view.rs:506`
  `commit_detail`, `tabs.rs:156` `visible_tabs`). The manual-close path already clears selection (`session_view.rs:1067`
  `do_close_tab`); the server-driven removal path needs the same outcome.
- Races to keep in mind: manual close vs. reap of the same window (both paths must tolerate the window being already
  gone), and the existing dead-at-open refusal (`core.rs:8283`) already covers a shell that dies before the open reply.

Test helpers for spawning/killing shells in tabs already exist in `crates/farhelm/tests/e2e/terminal_tabs.rs`
(`wait_for_shell` :79, `run_in_shell` :109, `tab_pane` :156, `listed_tabs` :173) and `e2e/tests/terminal.spec.ts`
(`openSessionWithTabs` :6320, `runInShell` :5805).

## 4. New tab shows leftover content from a previously closed tab

Status: fixed in PR #155 (`pr/tab-presize`) — the at-source fix (tab window pre-sized to the agent window at open, so
the attach-time resize stops provoking a mid-capture repaint), per the goal decision to fix without a confirmed repro.
The snapshot-consistency fix direction below was ATTEMPTED AND REJECTED: bracketing the cursor samples and retrying
required separating the captures from the cutover, and the cutover-losslessness tests caught it dropping real output —
capture-to-cutover adjacency is a hard contract (see the command-group comment in tmux.rs).

Symptom: after closing a tab (whose shell had exited) via x + confirm, opening a fresh tab with "+ terminal" renders TWO
prompt lines. The cursor sits on the FIRST line; the second "→ /tmp" line below it is residue that should not exist.
Relaunching the desktop app fully clears it: the same tab then shows a single clean prompt.

### What it is NOT (verified)

The obvious suspicion — stale client-side xterm state reused by the new tab — has no path. Tab ids are UUIDv4
(`crates/farhelm-supervisor/src/service/core.rs:8187`), the tab strip and panes are keyed by tab id in Dioxus
(`session_view.rs:1646-1656`, `:1764-1767`, with comments explaining exactly this hazard), and the JS islands
(`crates/farhelm-ui/assets/terminal.js`) key everything by element id with no pooling: every mount is a fresh
`new Terminal` (`terminal.js:2251-2273`), every unmount disposes fully (`:3534-3617`), and `sync()` unmounts departures
before mounting arrivals (`:1842-1955`). A new tab cannot inherit the old one's buffer.

### Leading hypothesis: the attach replay paints an inconsistent snapshot

The replay a fresh attach receives is: pane modes → captured content → trailing sequences ending in an ABSOLUTE cursor
placement `\x1b[y+1;x+1H` (`tmux.rs:684-689`), written to the client as one atomic `term.write` with no reset
(`connection.rs:945-946` — "The attach replay never resets: the client's terminal is brand new";
`terminal.js:2758-2816`). The cursor position comes from `display-message` (`tmux.rs:236-239`, first command of the
capture group at `tmux.rs:427-441`) — sampled SEPARATELY from the `capture-pane` calls that read the content. If the
pane is still settling when the group runs, the content can contain a prompt line the shell is about to erase while the
cursor sample already points back at row 0 — rendering exactly the observed shape: cursor on row 0, orphan line below
it.

A brand-new tab is plausibly the worst case: `new_window` is issued with no explicit size (`tmux.rs:2086-2121`) and
inherits the current session geometry, so whether the attach's `resize_window` (`handlers.rs:1615-1625`) is a real
geometry change depends on how the new island's cols/rows compare to that inherited geometry. When it IS a change, the
just-started shell repaints via SIGWINCH concurrent with the capture group. This is a hypothesis to verify with geometry
evidence, not an established mechanism — the disambiguation plan below collects it. On app relaunch the pane is already
at the client's size and idle, so content and cursor agree — matching the relaunch-fixes-it observation.

Related inconsistency found while tracing: `crates/farhelm-helm/src/terminal.rs:64-69` documents "the supervisor
captures before it resizes", but `handlers.rs:1615-1625` resizes before the capture. One of the two is stale; resolve as
part of this fix since the ordering is central to the race.

### Disambiguation plan (do this before fixing)

1. In the buggy state, dump `window.__farhelmIslands["terminal-<tab_id>"].term.buffer.active` rows 0-2 plus `cursorY`
   (map published at `terminal.js:996-1002`). Residue present in the buffer kills the client-side theories for good.
2. Log the replay payload for a fresh tab attach and compare the trailing `\x1b[y;xH` against the content it followed. A
   mismatch is a strong clue, not proof — cursor rows need not match non-empty line counts, and a history capture can
   include scrollback outside the visible grid's coordinate space. Prefer time-correlated snapshots of the visible grid
   and the cursor position, taken with a deterministic shell, over raw line counting.
3. Record the pane's geometry at creation and at attach (before/after the `resize_window`) to establish whether the
   attach resize was a real change for the failing case — the leading hypothesis depends on it.
4. Compare `tmux capture-pane -p` on the pane right after attach vs. a second later — confirms the tmux grid itself is
   clean.

### Fix directions (pick after disambiguation)

- Size the tab window before publishing it: tmux's `new-window` has NO size flags (`-x`/`-y` belong to `resize-window`
  and `new-session`), so this means create the window, then `resize-window -x <cols> -y <rows>` to the opening client's
  geometry before the open reply publishes the tab — with the same window cleanup on failure the open path already does.
  That makes the attach-time resize a no-op for fresh tabs and removes the repaint race at its source.
- And/or make the snapshot self-consistent: sample cursor position in the same tmux command as the content, or re-sample
  and re-send positioning after content capture.
- A client-side reset before first-attach replay is NOT the preferred fix: "never reset on attach" is deliberate
  (scrollback-preserving reattach), and it would only mask the inconsistent snapshot.

### Test gap

Nothing asserts the content of a freshly opened tab, and nothing does close-then-open-new-tab.
`islandText`/`islandLines` helpers exist (`e2e/tests/terminal.spec.ts:7911`, `:7989`); the missing test is: open tab →
close it → open a new one → assert the new island is clean. Do NOT phrase "clean" as "exactly one prompt line" — tabs
run the host's real interactive shell, whose prompt shape is not this project's to assume. Pin the shell
deterministically (the tab suites already have shell-controlling helpers, e.g. `harness_with_shell` in
`crates/farhelm/tests/e2e/terminal_tabs.rs:30`) and assert no content BELOW the cursor row, or compare against a control
attach of a never-closed tab.

### Suspicion checked and refuted: no xterm root-div leak

An earlier draft of this entry claimed vendored xterm's `dispose()` leaks its root `div.terminal.xterm` on in-place
remounts. Verified false (2026-08-13): the vendored bundle registers a disposable that runs
`this.element?.parentNode?.removeChild(this.element)` on dispose, so the root element is removed with everything else.
Recorded here so the claim does not get re-derived and turned into a fix PR for a defect that does not exist.

## 5. Layout redesign: persistent session sidebar, terminal fills the rest

Status: in progress across several PRs. PR #156 (`pr/sidebar-shell`) landed the shell: two-pane layout, keyed
SessionView remounts, stacked sidebar rows, left-truncated cwd, the revived open-session create-host default, the
two-pane feed fan-out contract, the cross-pane write gate (`ops::PaneGate` — neither pane starts a write under the
other's), and selection reconciliation when the selected row is deleted or archived away. The popup-menu PR
(`pr/row-actions-menu`, stacked on it) moves every per-row action and confirm into a floating panel behind a "⋯" toggle
and drops the profile chip from the row line into that panel. Still owed: the hosts/filter toggles with the SPEC
amendments, auto-select + rename consolidation + back-button removal, and their Playwright migrations.

What the user wants: a left sidebar that always shows the session/agent list, with the entire right-hand side being the
terminal area for the selected session. The tab strip ("agent | Terminal 1 | + terminal") moves to the top of the right
pane. Each sidebar row shows title + status; the per-session actions (rename/stop/archive/delete) collapse into a popup
menu opened by a small button to the right of the session name, instead of the current row of buttons. The sidebar gets
a reasonable max width — enough for title + status — and the terminal gets everything else.

### Shape BEFORE this issue's PRs (historical baseline — the shell PR replaces it)

There was no router. `AppBody` (`crates/farhelm-ui/src/lib.rs:817`) owned a single `current: Signal<Option<Session>>`
(:819) and matched on it (:849-859): `None` rendered `ListView` (`src/list.rs:503`), `Some` rendered `SessionView`
(`src/session_view.rs:254`) with an `on_back` that set it to `None`. The two views were deliberately mutually exclusive
— module docs at `lib.rs:15-24` and `lib.rs:775-790` argued from that exclusivity, and several behaviors leaned on it
(below). The sidebar-shell PR turns `current` into a SELECTION beside a permanently mounted list; line numbers below
describe the baseline, not the current tree.

Key structures:

- List view rsx starts at `list.rs:1547` as a bare document-flow fragment: `HostsPanel` (hosts.rs:662), the filter bar
  (`form.session-filter`, list.rs:1566), the new-session toolbar + `CreateSessionForm` (list.rs:1741-1793, component at
  list.rs:2007), then `div.session-list` (list.rs:1835) of `SessionRow`s (component list.rs:2869). A row's action
  buttons are at list.rs:3114-3141; inline confirm/rename branches at list.rs:3033-3111; visibility rules in
  `row_control_visibility` (list.rs:118).
- Session view rsx at `session_view.rs:1361`: `div.layout` (flex column, `height:100%`, app.css:12) → titlebar with back
  button (:1365), rename (:1387), offers/notices, `div.tab-strip` (:1630), `div.terminal-panes` (:1722). NOTE: the
  session view has NO stop and NO delete — those exist only on the list row, so the popup menu reuses `SessionRow`'s
  handlers (list.rs:1100-1490), not session-view code.
- Styling is one hand-written stylesheet, `crates/farhelm-ui/assets/app.css` (no Tailwind, no inline styles). Nothing
  uses 100vw/100vh; the only absolute positioning is scoped inside `.terminal-panes` (app.css:1632/1638), so a
  two-column flex parent is structurally easy — but every ancestor of `.terminal-panes` must keep a definite height or
  the terminal collapses.
- Terminal sizing self-heals: xterm FitAddon plus a per-island `ResizeObserver` (assets/terminal.js:3418) already
  handles the width change a sidebar introduces.

### Decisions (interviewed 2026-08-13)

These were put to the user directly; treat them as settled:

- Sidebar contents: session rows plus a new-session button ONLY. The hosts panel and the filter bar open on demand
  (toggle), not permanently stacked in the sidebar. This deliberately relaxes SPEC.md:215-225's "per-host connection
  state always visible" to a compact indicator — amend SPEC.md accordingly as part of the change.
- Sidebar row shows: title, status badge, host, working directory (truncate from the left), and invocation. NOT the
  profile badge. Rows stack vertically rather than squeeze horizontally.
- Sidebar width: fixed (no drag handle, no persistence, no collapse). Pick a width that fits the row contents above
  comfortably — with cwd + invocation in the row, err wider than the 280px floated during the interview rather than
  truncating everything.
- Actions popup menu confirms destructive items IN the menu: clicking delete swaps the menu's contents to a consequence
  line + confirm/cancel (autofocus on cancel, per house convention). The menu does not close and bounce the user to a
  second surface.
- Empty right pane: auto-select a session; a placeholder only when the fleet is empty. Three sub-decisions taken during
  triage review (each could reasonably go another way; revisit with the user if they look wrong in practice):
  - "Most recently active" means: the client's own last-selected session, persisted client-side (localStorage, keyed by
    helm identity), falling back to the newest-created non-archived session. `SessionInfo` carries no last-activity
    timestamp, and inventing a server-side one is out of scope for this redesign.
  - Auto-selection ATTACHES, exactly as clicking the row would. SPEC.md's "takeover is deliberate" rule is satisfied by
    the user's deliberate act of opening the client — the interviewed decision ("most recently active opens
    automatically") already implies a live terminal on launch. Consequence to be aware of: opening a second client
    silently takes the terminal over and the first shows its usual detached banner. Amend SPEC.md's takeover/opening
    language to say app-open counts as opening the auto-selected session.
  - Selection reconciliation: deleting or archiving the selected session, or filtering it out, clears the selection and
    re-runs the auto-select rule (next eligible session, else the empty placeholder). Note this changes today's behavior
    where a detail-view 404 deliberately preserves the displayed session.
- Rename lives in the sidebar row's popup menu ONLY. The right pane's titlebar shows the title read-only; its rename
  affordance (session_view.rs:1387-1430) is removed, which also removes the dual-optimistic-overlay disagreement.

### Design notes for the implementer

- `AppBody`'s `current` becomes the selection driving the right pane; the list renders always. The session view MUST
  remount on selection change (`key: "{session.id}"` or equivalent) — it seeds internal state from its prop via
  `use_signal` (session_view.rs:261) and would keep talking to the old session otherwise. Today remount is implicit in
  the match-arm swap.
- Only ONE `SessionView` mounts at a time. The agent terminal's DOM ids are global singletons (`tabs.rs:31-45`), and the
  attach lease is minted per view instance (session_view.rs:409). The sidebar changes what is visible, not the
  one-attached-client model.
- `use_drop` (session_view.rs:1287) currently means "navigated back"; under the redesign it fires on session switch,
  which is the desired unmount-previous-islands behavior.
- Sidebar rows: the current `.session-row-main` is a non-wrapping flex row with min-width floors (app.css:157-300) that
  already produced overflow bug MT-8; a narrow sidebar re-enters that regime, so rows should become stacked (title line,
  status line) rather than squeezed.
- The popup menu: there is NO existing dropdown/modal/portal primitive anywhere — all confirms are inline by design
  (wry's macOS webview has no browser dialogs, session_view.rs:274). The closest pattern is the hosts panel's "at most
  one open, parent owns it" toggle (`profiles_open`, hosts.rs:965-968, button at hosts.rs:1186) — copy that shape:
  `menu_open: Signal<Option<String>>` keyed by session id. Since `.session-row-open` is itself a `<button>`, the menu
  button must be a sibling, not a child (HTML forbids nested buttons — same constraint that produced `.tab-slot`,
  app.css:1467). Keep the autofocus-on-cancel convention for destructive confirms (list.rs:3058).
- Nav-locking assumptions to revisit: `guarded_open` (list.rs:1487) and the "open ends the list's tasks" reasoning
  documented at list.rs:445-450 and :495-502 — no longer true when both are mounted. Both views' feed readers
  (list.rs:965, session_view.rs:789) being live at once is supported by design (feed.rs:278) but doubles reads per
  notification; fine, just expected.
- Create-host default comes back: once the create form and a selected session coexist, SPEC.md's rule that creation
  defaults to the host of the currently open session applies again. Today's code always defaults to the local host
  BECAUSE the two views were mutually exclusive — lib.rs:775-790 documents exactly that reasoning and the plumbing that
  was removed. Restore it: pass the selected session's host into the create surface (`default_create_host` or its
  successor), update that doc, and test with a local and a remote selection.

### Docs and tests that pin the current layout

- Docs to amend: DONE in the shell PR for lib.rs, list.rs, session_view.rs and feed.rs (rewritten around the two-pane
  shape rather than the old exclusivity). STILL PENDING: SPEC.md:203-211/215-225 ("opening a session" language vs.
  persistent selection). SPEC.md's Errors and diagnostics section ALSO requires per-host connection state and retry
  phase to stay visible — amend it together with the Session list section (a compact indicator must still carry
  connection state and retry phase, with the full panel a toggle away), or the spec ends up internally contradictory.
  PLAN_M2.md references are historical; leave them.
- Playwright: heavy breakage, budget for it. ~18 uses of `.session-row-open` + 70 of `.session-row` in terminal.spec.ts
  alone (helpers `rowByTitle` :310, `sharedSessionRow` :363); ~20 uses of `.back-button` which the redesign deletes (its
  disabled-while-busy assertion at archive.spec.ts:289 needs a new home or removal); back-to-list assertions
  (terminal.spec.ts:1130, :3267, :4087, :6560-6580 — the last one, islands unmount + sockets close on back, must be
  re-expressed as "switching sessions unmounts the previous session's islands"); mouse-modes.spec.ts:395-455
  back-then-reopen cycles become select-other-then-reselect; titlebar.spec.ts:141 is entirely titlebar flex arithmetic
  and needs re-tuning for the narrower right pane.
- `scripts/desktop-smoke.sh` drives the UI by absolute pixel coordinates in a 1200x900 window and says so. The shell PR
  moved the x coordinates into the sidebar (fields x=170, terminal click x=700), recaptured every y for the two-pane
  layout (the persistent hosts/filter chrome sits ABOVE the new-session button now, pushing it to y≈474), re-cropped the
  form-oracle gate, and pinned the restarted app's window to origin 0,0 before the first click — the leg previously
  clicked wherever openbox happened to place the fresh window. If the sidebar chrome changes height in a later PR (the
  toggles work), the y set must be recaptured again.
