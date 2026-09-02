# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Four buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove —
the decision is made, only the work remains; "near term" is what should be picked up next; "maybe later" is wanted but
not soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either way.
Within a bucket, no order.

## Definite simplification

## Near term

- Show the running farhelm version in the top-right of the window. The version is already on hand — the UI compiles the
  workspace version in as `skew.rs`'s `CLIENT_BUILD`, and every helm reply carries the helm's own stamp — so this is
  presentation work, not plumbing: a small always-visible version readout in the window's top-right corner. Wanted
  because the desktop app and a browser tab both outlive updates, and "which build am I actually looking at" should be
  answerable at a glance; the skew banner already covers the mismatch case, this covers the question before anything
  mismatches.

- Stop the create dialog's inert command field from impersonating the launch command after a clone. Cloning a
  profile-backed row seeds the dialog with the row's RAW invocation, and while a profile is selected that field is
  deliberately disabled-but-not-emptied (`CreatePrefill::invocation` documents why: switching the picker to "custom
  command (below)" must offer the row's own command rather than whatever the form last held). What the user then sees
  misleads: clone a Claude session, switch the agent picker to a Codex profile, and the disabled field still shows the
  claude command line, reading as "this is what will run and you cannot change it". Nobody is actually stuck — in
  profile mode the field's text is inert (the create names the profile id and the wire refuses a request naming both),
  and the "custom command (below)" option re-enables editing — but nothing on screen says either of those things, and
  the parenthetical label does not carry the weight. Not a model gap: sessions record `source_profile` as a snapshot
  with existence states, and `prefill_from` already maps a custom-created source to the editable command mode. Fix
  directions: in profile mode display the SELECTED profile's own command in the field (or empty it and keep the clone's
  raw invocation off-screen as the seed for a later switch to custom), and make the label state the contract outright.
  Observed 2026-08-31 in live use of the new clone action, immediately after its first release into the stable install.

- Show the missing-or-too-old-tmux refusal in a native window, not just on stderr. `farhelm-desktop`'s tmux preflight
  now prints one plain message and exits (see `desktop.rs`'s `run_tmux_preflight_or_exit`), but a Finder launch has no
  terminal for stderr to land in, so that message currently reaches nobody there. No longer just theory: verified
  2026-09-01 on a real Mac while trialing a hand-rolled Farhelm.app — a launch that dies pre-window looks like nothing
  happened at all. The same silence covers every pre-window exit, not just the tmux preflight (the unusable-state-dir
  bootstrap failure exits the same way), so the fix should sit on the common refusal path. Keep the stderr line exactly
  as it is — the desktop-smoke gate asserts on it — and add a macOS-side surface next to it: a native alert before
  exiting, or at minimum an os_log line so Console shows the reason. Gains urgency with the planned .app bundle, which
  makes Finder and Spotlight launches the normal path rather than the exception.

- Stop terminal-query replies leaking into the shell as typed garbage. Symptom: after a short-lived command that probes
  the terminal — `sprite list` is a reliable case, and anything on lipgloss/termenv, vim's `t_RV`, or a bare `CSI 6n`
  will do — the pane shows `^[]11;rgb:0000/0000/0000^[\^[[2;1R` on its own line and the next prompt has
  `11;rgb:0000/0000/00001R` sitting in it as input. Cause, reproduced outside the UI with a control-mode client and
  farhelm's tmux config: the program sends `OSC 11;?` (background colour) and `CSI 6n` (cursor position) and waits for
  the replies; tmux answers both itself, immediately, so the program is satisfied, exits raw mode and exits. But control
  mode's `%output` carries the pane's RAW bytes, queries included, so farhelm forwards the query sequences to xterm.js,
  which is a real terminal and answers them too; its replies come back through `term.onData` → websocket → helm →
  supervisor → `send-keys -H` one round trip later, by which time only the shell is listening, and the tty echoes them
  in cooked mode and bash reads them as keystrokes. Every program under farhelm is answered twice; only the ones that
  stop reading after the first answer show it, which is why long-running TUIs are unaffected. Stock tmux never shows
  this because termenv refuses to query under `TERM=tmux-*`/`screen-*`; farhelm's `default-terminal
  xterm-256color`
  (correct for fidelity) removes that self-protection, and a rendering `tmux attach` never forwards a query it already
  answered. FIX, on the supervisor side in Rust, deliberately not in terminal.js: in `OutputStream`, strip from the live
  stream exactly the queries tmux answers itself — a fixed table of literal byte strings, roughly `CSI 6n`, `CSI ?6n`,
  `CSI 5n`, `CSI c`/`CSI 0c`/`CSI >c`/`CSI >0c`, and `OSC 10/11/12;?` with either `BEL` or `ESC \` as terminator — using
  a streaming literal matcher with a bounded hold-back (longest pattern is 8 bytes) so a query split across two
  `%output` lines is still caught, plus a flush of the hold-back on idle so a stream that ends mid-prefix is not stuck.
  This is NOT a VT parser and must not grow into one: no parameters are interpreted, and nothing outside the table is
  touched. Do not put anything in the table that tmux does not answer under the pinned version (DECRQM `CSI ?…$p` is the
  trap: xterm.js answers it, tmux does not, and stripping it would hang the program on its timeout); the guard is an
  integration test against the pinned tmux that sends each table entry into a pane and asserts a reply comes back, so a
  tmux bump that changes what it answers fails CI instead of hanging a user. Also a fuzz-style unit test: random split
  points over a stream must yield output identical to the unsplit result. Replay (`capture-pane -e`) never contains
  queries, so the matcher applies to the live stream only. The rejected alternative, for the record:
  `registerCsiHandler`/`registerOscHandler` in terminal.js swallowing the same set is ~10 lines, but it grows the JS
  island layer, leaves the table unguarded (no node-driven tmux test exists), and fixes only this client. DOCUMENTATION
  IS PART OF THE DELIVERABLE: the module or type that holds the matcher must explain, in plain language a reader with no
  tmux background can follow, what problem it solves (a pane program gets every terminal query answered twice, once by
  tmux and once by the browser, and the late duplicate lands in the shell as typed text), why the fix lives here rather
  than in the browser, why the table is exactly the set tmux answers and no more, what the hold-back and idle flush are
  for, and what this code must never become. A future reader who finds a byte-matcher in the output path with a comment
  that only names the sequences will delete it.

- Move agent profiles from the supervisor to the helm. Today a profile belongs to the supervisor that runs it: the
  catalog is a table in each host's supervisor.db, seeded with the four starters per install, edited through the host
  row's "⋯ → profiles" surface, and a create names a profile ID that the target supervisor resolves to an invocation,
  agent kind, and resume template at launch. SPEC.md states the reason as "the invocation has to exist on the host that
  runs it", with cross-host sync deferred as post-v1 convenience. That reason is about the command, not the storage — a
  profile saying `claude` runs through the login shell on whichever host launches it, and the supervisor already
  launches arbitrary invocations for custom-command creates — and the per-host model works against how hosts are
  actually used: remote hosts come and go, are not backed up, and should be treated as ephemeral rather than as places
  configuration accumulates. One catalog per helm is also the natural fit for the planned mode where the desktop app
  connects to a helm running elsewhere, which is what lets the browser and the desktop app share one logical helm
  instead of each carrying its own state. Intended user experience, settled 2026-09-02: profiles are the helm's, one
  catalog, every profile applies to every host, and the create dialog's picker offers the whole catalog regardless of
  the chosen host. The management surface is reachable from the sidebar but takes no standing vertical space — a popup
  of some kind (the row "⋯" menu / filter-popover pattern), holding the list with new, edit, and delete, and the
  "profiles" item leaves the host row menu entirely. No host-specific profiles in this change; if a need ever shows up
  it becomes an optional "only on these hosts" scope on a helm profile, never a different kind of profile and never
  something the supervisor knows about. Existing catalogs on hosts are simply DROPPED — no import, no migration, no
  trace going forward; the simplest code that removes them wins (the maintainer is the only user, and the edited
  profiles on the two live hosts get recreated by hand). "Last used profile" becomes one memory per helm rather than one
  per host. A session whose snapshotted profile this helm has no row for — created before the change, or by another helm
  — shows the snapshotted name as a plain label with no warning state, and clone offers the profile only when this helm
  holds one under that name, falling back to the raw command otherwise, the same way it already does for a deleted
  profile. Accepted consequence: the desktop app's embedded helm and the service helm on one machine get separate
  catalogs, exactly as they already have separate host registries; the connect-to-a-helm mode is what closes that. The
  architecturally major internal is the create wire: it must carry the resolved bundle (invocation, agent kind, resume
  template) inline instead of a profile ID, so a profile-backed create stops being distinguishable from a custom-command
  one on the supervisor side — and custom-command creates gain integration fields for free. The session's profile
  snapshot (id and name) stays on the supervisor; present/renamed/deleted is derived by the helm against its own catalog
  when it builds the listing. The agent CLI's `--profile <name>` resolves against the helm, and the spawn fallback that
  reads the supervisor's most recently used source profile becomes a helm lookup. SPEC.md's "Agent profiles belong to
  each supervisor" paragraph and the spawn-resolution paragraph, and SPEC_impl.md's supervisor profile-table bullet, are
  rewritten in the same change; the "post-v1 convenience" sentence goes.

- Move the session list's filter and sort controls onto the session list's own header. Today the sidebar's top line is
  "hosts filter sort [recently active]": one hosts toggle and three session-list controls sharing a row, which makes the
  filter and sort read as host controls, and the "13 sessions" count banner further down is the line that actually heads
  the list. Intended shape: the count banner grows to two rows and becomes the header — the count on the first row ("13
  sessions", or "5 matching of 13 sessions" when a filter is in force), the filter toggle and the sort select on the
  second. The visible word "sort" goes (the select's value reads as one; keep it as the control's accessible name). The
  amber "filtered" word that today appears beside the filter toggle whenever an applied filter narrows the list (bar
  open or closed; the archive switch alone does not trigger it) goes too: the count wording on the same line already
  says the list is narrowed, which is what SPEC.md's "the list says visibly that a filter is in force" asks for. The
  filter itself becomes a POPOVER anchored on its toggle, like the row "⋯" menus (`menu_panel.rs`'s fixed-position,
  measured-rect pattern, which also already escapes the sidebar's overflow clipping), rather than a bar that opens in
  the flow and re-jigs the whole sidebar. Inside the popover the filter is LIVE: the list re-filters on every keystroke,
  so what you see always matches what the fields say, with no apply button and no separate "applied" state to reason
  about; Escape or a click outside only closes the popover, and the filter stays as typed; a "clear" control stays.
  Today the bar is a form applied on submit, with `filter` and `filter_draft` as separate signals in `list/view.rs`,
  kept apart so that a feed-triggered re-read landing mid-edit would use the filter whose results were on screen rather
  than a half-typed one. Live filtering makes that distinction moot — the typed text IS the filter — so the draft signal
  goes and the field writes the applied one directly. Mechanics to keep in mind, not UX: every change is a helm round
  trip (the helm filters the whole fleet in memory and the client walks the reply by cursor), so a short debounce on the
  text fields is fine as an implementation detail but the intent is that the list follows the typing; `commit_listing`
  already refuses a read that was walking under an older filter, so a fast typist cannot get a stale result painted over
  a newer one; and the count and the "no sessions" placeholder keep answering only for a committed result, never for an
  in-flight read. The hosts toggle on the old top line goes away with the hosts-list rework below. Settled 2026-09-02.

- Make the host list one list, and make it look like it belongs next to the session list. Today the sidebar renders
  hosts TWICE: an always-visible compact strip (`.hosts-compact` in `list/view.rs`) of name plus colored phase word,
  where the word trails the name at whatever x the name ends on so "connected" lands in a different column on every row
  and the strip reads as ragged and unfinished; and, behind the "hosts" toggle, the full panel (`hosts.rs`) with its own
  "HOSTS / add host" header, bold rows with a right-aligned chip and a hover-revealed "⋯" menu, a detail line (version;
  identity; N sessions), and a per-host action row holding "update" (or "re-run" after a failed run, or "set up
  automatically" for the local host). Intended shape, mirroring the session list's header: a first row reading "N
  hosts"; a second row with a "details" toggle and the "add host" button; then ONE list of rows that always show the
  name, the status pinned to the trailing edge in the same gutter the session rows use, and the "⋯" menu — always
  visible, muted, rather than hover-revealed (hover has no touch story and is discoverable only by accident). The
  details toggle reveals, under every row, what the expanded panel shows today: the version/identity/session-count line
  and any remedy, warning, or error text. The provisioning actions move INTO the "⋯" menu — "update", and the "re-run"
  and "set up automatically" variants — so no row carries a standing button; the plan confirmation and the run's
  progress still render inline under the row, and a menu action that starts a run should open that row's details so the
  confirmation is visible, while a failed run needs a visible trace with details collapsed, since a failed provisioning
  run does not by itself change the phase the chip shows. Visual polish that rides along: use the session list's status
  vocabulary (a small dot plus a word) so the two lists speak one visual language, with "connected" quiet (a dot alone)
  and the words spent on states that need action; humanize the phase words for display ("unreachable, retrying",
  "identity mismatch") while keeping the hyphenated tokens on the data attributes tests read; consistent row height and
  hover background. SPEC.md's session-list paragraph says the full hosts panel "opens on demand rather than occupying
  the session list" — a per-row menu is on demand, but reword that sentence in the same change. Observed 2026-09-02 in
  the stable install with two hosts; shape settled the same day.

## Maybe later

- Automate end-to-end testing of the host UPDATE path, including across releases. Nothing in CI updates a host: the
  CentOS provisioning test only ever installs onto a fresh container, and the update flow's own tests drive fake
  backends. The gap shipped a real field failure (2026-09-01), which is the worked example any design here should be
  checked against: the first cross-protocol update ever attempted — a protocol-12 farhelm 0.1.1 host under a protocol-14
  0.2.1 helm — failed at the PROBE, whose classifier treated the version-skew refusal as a transport failure ("the
  supervisor probe closed before hello completion with exit status 0"), making exactly the host the update action exists
  for un-updatable; the operator recovered by stopping the remote supervisor by hand so the probe would see clean
  absence and take the fresh-install path. The eventual fix (`ProbeObservation::SkewedSupervisor`) added unit and
  service-level regression tests, but the CLASS of bug wants end-to-end coverage: something like a CentOS-leg variant
  that provisions a PREVIOUS RELEASE's binary (the harness builds its payloads from this tree today; the helm's own
  verified release-download path — D13, `release_payloads.rs` — is the existing machinery that can fetch a pinned
  released one), lets it register and run, then drives the panel's update action to the workspace build and asserts the
  supervisor comes back at the new version with its tmux sessions intact. The old half must be a real released artifact,
  not this tree's build — same-version update tests are exactly what could never see this bug.

- Guard provisioning against pushing payloads older than the helm's own protocol. A release-shaped helm built from a
  commit newer than the latest release (the local stable-binary flow does exactly this) provisions remote hosts with
  DOWNLOADED released payloads by default (D13), so the freshly provisioned supervisor can speak an older protocol than
  the helm that just installed it — and the helm then refuses it at the hello gate. Nothing is damaged (the refusal is
  the version rule working), but the failure arrives one step late, as a skewed host instead of a refused provisioning
  attempt. Possible shapes: compare the payload's version against the helm's `PROTOCOL_VERSION` before pushing and
  refuse with a message naming the mismatch; or make the staged-payload path (`--payload-dir`,
  `FARHELM_HELM_PAYLOAD_DIR`) the documented answer for from-main helms. Noted 2026-08-31 when upgrading the stable
  install to a from-main build while the newest release was still 0.1.1.

- Custom hover tooltips on buttons and menu items. Native `title` tooltips are free (the UI already uses them on the
  activity time, the cwd line, the profile chip and the header's archive button) but the browser owns their ~1s delay
  and nothing — no CSS, attribute, or JS — shortens it; WebKit's web content ignores the macOS tooltip-delay default
  too. A faster, themed tooltip is a component shown on hover after a delay of the app's own choosing (~300ms), and it
  has to escape the sidebar: `.app-sidebar`'s `overflow: hidden auto` clips anything anchored inside a row near its
  edges, so the tooltip needs a body-level portal or `position: fixed` with measured coordinates — the row `…` menu's
  popover is the pattern to copy. If the native delay turns out tolerable, a `title` pass over the terse actions (stop /
  archive / delete, the host row's buttons) is an hour and needs none of this.

- Consider dropping conversation-identity SCAN support and keeping only the per-launch hook. The resume promise stays;
  what goes is the second mechanism. The hook is the agent's own answer and covers `/clear` and `/new`, which the scan
  cannot see at all; the scan (`agent_kind/capture.rs`, `service/capture.rs`, and their e2e suites — roughly 6k lines)
  exists only for launches where the hook cannot be attached (a profile already passing `--settings` or Codex hook
  config, a bare `--`, or `FARHELM_AGENT_HOOKS` opting out) and for a hook that failed. Those launches would take the
  fallback SPEC.md already defines for an uncaptured identity — restart says so and offers the resume template or a
  fresh launch — and the vendor-record parsing that breaks whenever a vendor changes its on-disk layout goes away. The
  spec edit is one clause in Durability and resume ("and scanned from the outside … otherwise", plus "Scanning stays the
  fallback …"). Write-up: https://claude.ai/code/artifact/554790ce-c744-4daa-b9a5-151facdb1f42

- Consider dropping the race-proofing around host identity, keeping the identity itself. To be clear about what stays:
  the per-install identity the supervisor mints on first run and stores in its own database, independent of hostname and
  address, so a retargeted row or a state directory moved to another machine is recognized as the same install; "never
  silently merge" as a user-visible rule; and the mismatch surfaced with both identities and an adopt choice. What goes
  is the machinery that closes millisecond windows in RECORDING it: the empty-slot-only compare-and-swap in
  `record_first_contact`, the dialed-configuration check inside the same transaction (a retarget straddling a
  handshake), the separate adopt CAS with in-transaction cache purge, the never-reused connection tokens that every
  session-cache and mutation write carries, and the split of one "something is off" situation into three connection
  states (`identity-mismatch`, `duplicate`, `identity-unverified`) each with its own remedy text and re-probe policy —
  about 1,500 lines of helm implementation plus ~2,000 of tests across `store.rs`, `manager.rs`, and `hosts.rs`. The
  replacement is check-and-ask: on connect, compare the reported identity with the stored one; equal or empty means
  record and proceed, different means freeze and ask. The races it stops defending against need a retarget or a second
  first-contact to land within one handshake of another, and even then the consequence is a wrong identity the next
  connect flags as a mismatch, not a merge nobody sees. SPEC_impl.md's "structurally impossible at the storage layer"
  and "SCHEMA invariant" paragraphs under Helm internals would be rewritten to say the check is a check. Write-up:
  https://claude.ai/code/artifact/c3d3a74b-ae55-45c7-b3ca-fe30f9f97432

- Replace `install.sh`'s park/journal/rollback with plain idempotency. The floor to keep: re-running the installer from
  ANY intermediate state converges to a correct install, and no single binary is ever torn — download and extract into a
  staging directory inside `$INSTALL_DIR`, verify, then `mv` each binary into place, which is an atomic rename on one
  filesystem. Keep the outer brace group too; it is two lines and is what makes a truncated `curl | sh` execute nothing.
  What goes is the transactional layer built on top of that: the `mkdir` lock with ownership checks, the
  `PARK`/`INSTALL`/`UNDONE` journal, `rollback_from_journal`, the two rollback branches in the replacement loop, the
  `.old` parking files and the refuse-unless helpers around them — about 220 of the script's 527 logic lines (the other
  ~570 lines of the file are comments). Be honest about the size: the ~300 logic lines that remain are things any
  correct installer needs — target detection with the Rosetta case, prerequisite probing, latest-version discovery,
  `SHA256SUMS` handling, tar member validation, the `--version` cross-check, PATH and tmux advice, the closing messages
  — and `test-install-sh.sh` mostly tests THOSE (404, checksum mismatch, malformed archives, versions, prerequisites,
  the closing-message contract, the nothing-outside-`$INSTALL_DIR` diff); only the forced-failure rollback leg goes.
  Given up: a kill between the two macOS binaries' renames leaves one new and one old until the next run (the desktop
  shell finds its sibling by path, so a mismatch shows as a refusal, not silence), and a failure after placement no
  longer restores the previous binaries — re-run instead. A modest cut, worth taking when someone is in the file anyway
  rather than on its own.

- Fly.io Sprites as a host kind: a session backed by a per-second-billed microVM that freezes when idle, with "pause
  this host" in the UI meaning "stop paying for it". Assessed 2026-08-30 against a real sprite; the findings, the code
  mapping (a `HostKind::Sprite` over the existing ssh transport via the sprite CLI's ProxyCommand emulation, a
  provisioning flavor for a host with no systemd and no sftp, a `Paused` host state), the SPEC conflicts to surface, and
  a build order are in `lore/2026-08-30-fly-sprites-as-a-host-kind.md`. The same entry sizes the related "native app
  attaches to a remote helm" mode and the cheaper installed-web-app alternative.

- Tensorlake sandboxes as a host kind: the same idea assessed 2026-09-01 against a real sandbox, in
  `lore/2026-09-01-tensorlake-sandboxes-as-a-host-kind.md`. Fits better than sprites — a real SSH gateway makes the
  transport farhelm's existing ssh path with zero code changes (binary stdio and sftp both verified), suspend/resume
  preserves running processes under the same boot id, and platform-managed processes replace the missing systemd — at
  the cost of resume needing an explicit `tl sbx resume` (plain ssh refuses a suspended sandbox) and, today, an sshd
  session leak that defeats idle-suspend until the leaked sessions are reaped (evidence in the lore entry; worth
  reporting upstream).

## Unbucketized

- Make the never-started verdict say which link died. When a scoped launch dies before farhelm's exec shim, the
  supervisor's `wrapper_failure_detail` (launch_artifacts.rs) records "the agent was never started: the launch never
  reached farhelm's exec shim, so something before it — the transient cgroup scope wrapper, or the login shell itself —
  exited first", which names two suspects and separates neither. The wrapper's stderr is still sitting in the dead pane
  under `remain-on-exit`, so a `capture-pane` at classification time could say which. A first attempt (2026-08-23)
  appended the pane's last words to the durable `LastOutcome::Error` detail and was withdrawn in review for three
  reasons any retry must design around: (1) SPEC.md's terminal-retention contract — terminal content lives only as long
  as the host-side terminal, with no separate history store — which a durable excerpt of startup/rc output contradicts,
  so either the quote must not be persisted (log it, or surface it only while the pane exists) or the spec must
  authorize a bounded exception first; (2) the pane is reused across relaunches and keeps its scrollback, so a
  generation N+1 that died before printing anything would quote generation N's conversation unless the capture is fenced
  to text written after the wrapper started; (3) the ownership-and-deadness check and the capture are separate steps
  with no lifecycle claim across them on the list path, so a same-pane restart in between would quote a live later
  generation — revalidate atomically with the capture, and budget the capture so N never-started rows cannot cost N tmux
  timeouts on the hot list. The e2e harness already has `wait_for_agent_ready` (harness.rs), whose failure text shows
  the same pane text for a test's own diagnosis; that is the non-durable shape to start from.

- Decide the reservation tombstone scope for interactive creates, then do the work the verdict leaves standing. When a
  client attaches an intent key to a create, the supervisor records it in `create_reservations` so a retried request
  returns the already-created session instead of double-launching an agent. The durability-era decision made these
  reservations PERMANENT for interactive creates (spawn's are session-bounded), which makes `create_reservations` the
  only store table that grows without bound — every interactive create ever made adds a row nothing deletes. Two
  follow-on debts, deliberately distinct: (a) digest the reservation fingerprint — rows currently retain enough of the
  original request to match retries, i.e. request plaintext (titles, cwds, invocations) retained forever; hashing bounds
  each row and ends the plaintext retention but does nothing about row count; (b) expiry/pruning — actually bounds the
  count, at the deliberate cost that a pruned key becomes reusable after the horizon (a very late retry could
  double-create). The digest half is worth doing under either verdict. If the verdict bounds the scope (session-lifetime
  — defensible, since a retry outliving the session it protects is protecting nothing), the pruning half mostly
  evaporates; that reverses a durability-milestone decision, so record the reversal where that decision lives. The store
  module's own docs describe both debts.

- Run the review-cap residue pass: one targeted review (test-quality and docs lenses only) over the three largest M7
  surfaces — auth, provisioning, packaging. During the M7 stack's reviews, the test-quality lens (often docs too) was
  still producing ACCEPTED findings at the hard three-pass cap on every large PR (#114, #115, #117, #118, #119/#120,
  #121, #122, #123) — those reviews ended because the budget ran out, not because the reviewers ran dry, so there are
  almost certainly real accepted-grade findings never surfaced in security-critical code. Treat a pass that returns zero
  accepted findings as saturation finally reached; anything it does return gets the normal fix-or-reject treatment.
  Cheap to run, and the difference between "reviewed until done" and "reviewed until the meter ran out" — do it before
  declaring the first real release final.

- Close the HostId-reuse create-default window. The create dialog defaults its host field to the selected session's host
  BY ROW ID, but a host row id survives a retarget (or an adopt where a new install takes over) while the machine behind
  it changes: look at a session on the old machine, have the row retargeted, open the create dialog within one
  listing-refresh interval, and the create lands on the successor install. The request's own install-incarnation check
  passes — the request was genuinely built against the successor — so the system does what it was told while the user's
  intent lands on the wrong machine. Raised as a definite security finding in #156's review; accepted there as residual
  because selection reconciliation already narrows the window to one refresh interval. The full fix: the helm's listing
  must denormalize install identity per session so the client binds its create default to the install the user was
  actually looking at, not the row id. Not urgent (needs a concurrent retarget plus a one-interval race), but "accepted
  residual" should not quietly become "permanent".

- Run the manual Mac checklist (`docs/manual-mac-checklist.md` — that file IS the record; its "Observed:" fields are the
  state, all "not run"). Blocked on a human with a real Mac. Not covered by any CI: Playwright's WebKit is not
  WKWebView.

- Decide whether several helms sharing one supervisor becomes supported, and what that requires. SPEC.md says concurrent
  helms are unsupported in v1, with the supervisor's one-attachment-per-session rule as the only backstop. Observed on
  2026-08-27 while acceptance-testing the 0.1.0 rc: a desktop helm (0.1.0-rc.1) and the browser helm (0.0.3) both
  registered the same host and both listed the same sessions, live, with no disconnects, and switching between the two
  surfaces worked. That is not luck — sessions, their status and `archived` are supervisor-owned and the helm's
  `session_cache` is an explicit mirror, so any helm reaching the supervisor sees the same list. What was deliberately
  NOT tested: opening the SAME session in both helms. The expected result is the displaced-client path the spec defines
  for a second client (snapshot plus take-control, and auto-reconnect never seizing), since the supervisor enforces that
  rule, but the path has only ever been exercised between two clients of one helm. Known gaps before this could be
  called supported: (1) D2 version coupling — each helm expects the supervisor at its OWN version and offers `update`
  otherwise, so helms of different versions would tug the host up and down (the rc helm already offered to "update" the
  0.0.3 production supervisor; a compatibility rule such as "at least mine" plus a protocol version is design work, not
  a fix); (2) no lock against two helms provisioning or updating the same host at once; (3) the cross-helm takeover,
  replay-after-takeover and dimension handoff have no tests; (4) SPEC.md and SPEC_impl.md would need to state the
  supported model. Same-version helms look like a small step; mixed versions are the real work. First action when
  returning: run the untested case with two same-version helms and record what the displaced side shows.
