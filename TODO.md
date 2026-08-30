# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Four buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove —
the decision is made, only the work remains; "near term" is what should be picked up next; "maybe later" is wanted but
not soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either way.
Within a bucket, no order.

## Definite simplification

- Serve the session list WHOLE instead of paginating it at every layer. The user experience stays exactly as it is —
  three sort orders, the "N matching of M" count, cross-client refresh, stale rows from unreachable hosts, the "could
  not read to the end" notice — but the fleet this product is for is tens of sessions on a few hosts, not thousands, and
  the machinery built to scale past that is the largest source of incidental complexity in the codebase: keyset cursors
  at the wire AND at the helm (deliberately incompatible encodings), per-order database indexes with a per-host
  `UNION ALL` page query, Rust-side Unicode title folding cut to 128 chars with a batched backfill migration, cursors
  bound to order and filter, byte budgets as a second page cut, a bounded matching-count cache keyed to a store
  generation, drain-to-exhaustion refresh with three termination bounds and wire-order validation, and in the UI the
  "underfilled listing" predicate with its three readers plus a one-row creation-order fallback request — all of it
  existing because sorting by mutable keys under pagination lets a row cross the cursor between two pages. The
  replacement: each supervisor returns its list in one reply up to a hard cap (a few hundred rows); the helm merges the
  hosts in memory, sorts in Rust, and hands the browser one array; a whole reply is a snapshot, so rows cannot move
  between pages and the drift edge cases disappear with the pages. The cap is where the spec's "could not read to the
  end" notice lives. Wire pagination contract goes too (protocol bump). SPEC EDITS, in the same PR: SPEC.md's Session
  list section must state the scale assumption and the prohibition in so many words — the fleet is tens of sessions, the
  list is served and rendered WHOLE up to a fixed cap of a few hundred, "could not read to the end" means the cap was
  hit, and no pagination, cursors, streaming or incremental listing, or per-order server-side indexing is wanted at any
  layer; it is fine for the helm and every client to hold and sort the entire fleet in memory. Without that sentence the
  next agent re-derives pagination from "a list the client could not read to the end". SPEC_impl.md: delete "The page is
  a PAGE all the way down", the three-orders/cursor/collation/matching-count bullets under Helm internals, the
  drain-then-replace and wire-order-validation bullets, and the GUI section's underfilled-listing paragraphs; replace
  with one paragraph describing the whole-list reply and cap. Write-up:
  https://claude.ai/code/artifact/1352315d-aab4-43e9-8cb9-2f58c06b8b4e

- Move the remembered sort order and last-selected session into the HELM, as one preference every client reads and
  writes, and delete the per-client persistence. Today the browser keeps `farhelm.sort` and `farhelm.last-selected` in
  localStorage and the desktop app mirrors both into `desktop-client.json` beside its credentials through a coalesced
  eval round trip with an acknowledgement, a native in-memory mirror, and a read-merge under the auth state-file lock —
  about 700 lines plus 1,000 of tests (`desktop.rs`, `desktop-auth.js`), all for the four words "and desktop relaunches"
  in SPEC.md's session-list section. The replacement is a two-column row in helm.db behind a small REST pair; every
  client fetches it once after authentication, which both engines already block the tree on, so first render is seeded
  with no flicker, and writes it on change. Preferences then share no file with credentials at all. Two visible changes,
  both to be recorded in SPEC.md, which today says "remembered per client": the values are shared across clients (a
  browser tab and the desktop app open in the same order and on the same session — for one user and one helm, "where I
  left off" rather than "where this window left off"), and opening a second client attaches to whatever was most
  recently selected ANYWHERE, taking it over per the one-attached-client rule, where today it takes over that client's
  own remembered session. Survival becomes a helm-restart story rather than a client one; a helm reinstall loses the
  preference, which the best-effort clause in Errors and diagnostics already covers. SPEC EDITS, in the same PR: SPEC.md
  Session list — replace "remembered per client across browser reloads and desktop relaunches" with "remembered by the
  helm as one preference shared by every client", and say outright that no client keeps its own copy and that per-client
  persistence (browser storage, a desktop state file, anything letting two clients remember different answers) is not
  wanted. Terminal experience — "the session the user most recently had selected there — including after a desktop
  relaunch" becomes "most recently selected from any client", with the takeover consequence named. Errors and
  diagnostics — the best-effort exception is reworded from "desktop selection and sort across relaunches" to the
  helm-side preference (a helm that lost it falls back to defaults). SPEC_impl.md GUI section: delete the
  localStorage/`desktop-client.json`/eval-round-trip paragraphs. Write-up:
  https://claude.ai/code/artifact/7d64a1a0-64fe-41f6-b9a3-610fd7b75434

- Store the helm's remembered profile default as a bare id per registry row, and drop the install-identity binding, the
  request preconditions, and the commit-after-cancel task around it (`crates/farhelm-helm/src/profiles.rs`, ~900 lines
  plus ~2,200 of tests, and the `remembered_profiles.host_identity` column with its revalidation on every read). What
  this touches is ONLY which entry the create dialog preselects; the catalog itself always comes live from whichever
  supervisor the row currently reaches, and editing, deleting, and what a session snapshots at creation are untouched.
  The binding exists for one moment: a row that once reached supervisor install A, where the last create used profile id
  N, later reaches install B (wipe-and-reprovision, retarget, adopt). Every fresh supervisor seeds the same starter ids,
  so B also has an id N under the same name; today the helm notices the identity changed and asks, whereas a bare id
  preselects B's N — identical to A's unless the user had customised that starter on A, and even then a wrong DEFAULT in
  a dropdown they can change before clicking create. SPEC.md's "asks instead of guessing" is about a profile that no
  longer exists, which a bare id still honours (the catalog lookup fails, the dialog asks); the "exists but on another
  install" extension is what goes. Also gone: the precondition on profile edits ("only if the host is still the
  connection I prepared against" — an edit landing after a retarget just applies to the install that is there now, which
  is where the catalog on screen came from on any realistic timescale) and the detached commit task (a save whose
  browser disconnected mid-request relies on the next 3-second refresh to reach other clients instead of an immediate
  push). SPEC EDITS, in the same PR: SPEC.md Sessions/Creation, after "defaults to the last-used profile on the target
  host; if that profile no longer exists, the client asks" — add that "last-used" is a plain profile id remembered per
  registry entry and that is the whole mechanism: not bound to the install behind the entry, no precondition, no
  revalidation beyond the host's current catalog; a reinstalled or retargeted host with a profile under the same id gets
  it preselected, accepted because a default is a suggestion in a dropdown, never an action; machinery to detect "same
  id, different install" is not wanted. SPEC_impl.md: the profiles paragraph under Supervisor internals keeps its
  per-host catalog rules; remove the "a default belongs to an INSTALL" reasoning wherever it appears. Write-up:
  https://claude.ai/code/artifact/6476812c-681b-4d06-9b08-59911451f1e0

## Near term

- Deflake the browser merge gate: three e2e tests fail on main itself, deterministically — found 2026-08-31 while gating
  a PR stack, and every failure reproduces on main's own tree, idle box, both engines. Two independent causes. First,
  `provisioning.spec.ts` hardcodes `BUILD = "0.0.3"` into every stubbed reply's `x-farhelm-build` header; the workspace
  version has been past that since the 0.1.0/0.1.1 release bumps (2026-08-26/27), so the stale stamp latches the UI's
  build-skew gate, and a latched page deliberately never opens its feed socket — the two feed-driven provisioning tests
  ("a failed local ADD keeps its rerun action...", "a progress read failure recovers...") then time out on
  `feed.openSockets() > 0` while everything else on the page works. Fix: read the stamp off the running helm the way
  `terminal-suite.ts`'s `HELM_BUILD` already does, so a release bump cannot silently break the gate again. Second,
  terminal-flood's "a multi-megabyte message does not drop the terminal socket" times out waiting for
  `echo:after-big-message`: the 2 MiB payload becomes ~8192 synchronous 256-byte `send-keys` round trips (deliberate —
  see the test's own comment on `InputClient::send`), and the 60s budget does not cover that on an 11 GB / 6-core
  machine even with nothing else running. Measure the per-chunk round-trip cost before choosing between a bigger budget
  and a smaller payload; whatever the choice, the payload must stay above the 1 MiB cap the test exists to guard
  against. Until both are fixed, a red merge-gate run needs manual triage against this entry to tell a real regression
  from the known set.

- Show the missing-or-too-old-tmux refusal in a native window, not just on stderr. `farhelm-desktop`'s tmux preflight
  now prints one plain message and exits (see `desktop.rs`'s `run_tmux_preflight_or_exit`), but a Finder launch has no
  terminal for stderr to land in, so that message currently reaches nobody there. Unverifiable without a Mac to try it
  on, so deferred rather than guessed at.

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

## Maybe later

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

- Deflake `terminal_backpressure::memory_stays_flat_while_a_viewer_is_stalled` on four-thread CI. One occurrence,
  2026-08-23, on the `test` job of a commit that changed only the tmux source pin (CI run 32610561516, PR #225): "timed
  out waiting for FLOOD-000000; 13231534 bytes seen, last records: [799999, 799998, 799997]" at
  terminal_backpressure.rs:166 — the flood's final records had arrived and the marker the test waits for never did
  within its window. Passed on the eight CI runs of the same stack before it and on a same-SHA re-run. Unreproduced in
  15 runs on a four-core box under concurrent Playwright load with tmux 3.7c. Candidate mechanism, unverified: the
  marker is written after the flood completes and its wait is a flat deadline that a loaded runner can exhaust while the
  13 MB drain is still in flight; first steps: keep the per-record progress as the rearm signal (the test already sees
  record numbers advance) and make the marker wait fail by name only when progress stops, not when a flat budget expires
  — the same shape `provisioning::tests::wait_real_run` was given.

- Deflake `session_lifecycle::input_bytes_survive_verbatim_through_hexecho` on four-thread CI with the pinned tmux 3.7c
  first on PATH. One occurrence, 2026-08-23, `test` job of PR #228: "control bytes must arrive verbatim; transcript:
  FAKE-AGENT READY" followed by blank rows — the typed bytes never echoed at all, rather than arriving mangled. Passed
  on a same-SHA re-run and 15/15 on a four-core box under load with 3.7c. Candidate mechanism, unverified: the same
  fixture race `reattach_replays_history_and_modes` had — input sent on the strength of the READY text before the
  fixture is reading stdin — except the hexecho script may not print a prompt to wait for; first steps: check what
  `hexecho` writes after READY and give the test a post-READY barrier the fixture actually emits (add a prompt to the
  script if it has none), per the `wait_for_after` pattern in session_lifecycle.rs.

- Deflake `restart_with_resume::an_interrupted_codex_session_resumes_its_conversation_in_a_fresh_terminal` on
  four-thread CI. One occurrence, 2026-08-23, `test` job of PR #228: "the resume ran the TEMPLATE, not the launch
  invocation: …/codex internal fake-agent --script codex-rec" at restart_with_resume.rs:759 — the restarted launch used
  the profile template's invocation instead of the recorded one. Passed on a same-SHA re-run and 15/15 on a four-core
  box under load with 3.7c. Candidate mechanism, unverified: the restart was issued before the conversation record (or
  the launch row's recorded invocation) had been committed, so the resume path found nothing to resume and fell back to
  the template; first steps: find what the resume reads to choose between recorded invocation and template, and have the
  test wait for that record to be durable (a named setup wait) before interrupting, instead of relying on the fake
  agent's output having landed.

- Deflake `session_lifecycle::stop_kills_an_unmarked_child_of_a_reparented_daemon_via_closure_seeding` in four-thread
  full-suite runs. One occurrence, 2026-08-23, full battery (`--test-threads=4`, pinned tmux 3.7c) on a four-vCPU Ubuntu
  26.04 box, on PR #233's CSS-only tree: the SETUP assertion "test setup: the child must NOT carry the marker — that is
  the point" failed — `marked_pids(&session.id)` (harness.rs) contained the pid read from `unmarked-child.pid`. 299/300
  otherwise; passed 3/3 isolated reruns on the same code under concurrent load. Candidate mechanism, unverified but
  visible in the fixture: `spawner_reparent` (fake_agent.rs) backgrounds `env -u FARHELM_SESSION_ID sh -c "sleep 120"`
  and the daemon shell writes `$!` to `unmarked-child.pid` immediately — but between the fork and `env`'s exec,
  `/proc/<pid>/environ` still shows the daemon shell's image, which DOES carry the marker, so a scan racing that window
  sees the "unmarked" child as marked (a loaded box widens the window; pid reuse is the less likely alternative). The
  property under test is fine — this is setup racing the fixture. First steps: make the setup read tolerate the window —
  poll `marked_pids` until the recorded pid drops out (bounded, with the offender's `/proc/<pid>/cmdline` in the failure
  text so a real regression stays diagnosable) — before asserting; restructuring the fixture so the CHILD writes its own
  pid post-exec would close the window outright, but the pid-file write would then need `$$` inside the third quoting
  level, the exact trap the fixture's own comment documents choosing `$!` to avoid.

- Deflake `service::ticker::tests::samples_accumulate_for_a_busy_pane_and_stay_quiet_for_a_still_one` on four-thread CI.
  One occurrence, 2026-08-23, `test` job of PR #238 (job 97254597359): "a pane printing a new line every 50ms must have
  changed at its most recent comparison; a streak here is change detection that never fires" at ticker.rs:1916 —
  `busy.unchanged_streak` was 1, not 0. 519/520 otherwise; the identical supervisor code passed the full battery on a
  four-vCPU box the same day. Candidate mechanism, unverified: the assertion couples the two panes' clocks — it reads
  the busy pane's streak at whatever instant the still pane's streak reaches 3, and demands the busy pane's MOST RECENT
  comparison saw a change; the busy pane is a real tmux pane driven by a `sleep 0.05` echo loop, so one loaded-runner
  stall (or one capture landing twice on the same grid) across a single comparison window yields a streak of exactly 1
  without change detection being broken at all. First steps: decouple the oracle from that instant — assert the streak
  stays BELOW the classifier's quiet threshold rather than exactly 0, or wait (progress-rearmed, like the
  `wait_real_run` shape) for the busy streak to return to 0 after the still pane qualifies, so a single stalled window
  recovers instead of failing the run.

- Deflake `archive::deleting_an_archived_session_removes_its_row_and_attachments` on four-thread CI. One occurrence,
  2026-08-25, `test` job of PR #256 (job 97658478803, a workflow-only change): `archive_session` failed with "archive:
  killing process tree for archive: process-tree kill hit 1 error(s): quiesce did not converge within 5 passes; the
  process tree may not be fully frozen" (sweep.rs, `kill_process_tree` step 3). 323/324 otherwise; I did not audit
  earlier runs for it, and nothing in TODO.md or docs/ records it. The pane under test is `/bin/sh -c 'sleep 120'`, a
  finite tree a few processes deep, so five consecutive passes each finding a pid absent the pass before should be
  impossible once everything is SIGSTOPped. Candidate mechanism, visible in the code but unverified as the cause:
  `enumerate_tree`'s PPID closure admits a child when `found.contains_key(&ppid)` — the parent's pid NUMBER, with no
  check that the parent still has the starttime `found` recorded for it. Seeds themselves are starttime-validated, but
  only on the pass after the death: a tree member that died (SIGTERMed in step 1) whose number the kernel handed to an
  unrelated process makes every child of that unrelated process a "descendant" for as long as its number stays in
  `found`, and on a four-vCPU runner hosting three other harnesses that fork continuously, that is a steady supply of
  never-before-seen pids. First step is instrumentation either way: the error names no pids, so the next occurrence is
  as opaque as this one — have the non-convergence message list each pass's newly-found
  `(pid, ppid, starttime,
  /proc/<pid>/cmdline)`. If the cmdlines are unrelated processes with a parent outside the
  pane, close the hole by expanding the closure only through parents whose CURRENT starttime matches the one recorded in
  `found`.

- Fix `terminal-keys.spec.ts`'s four Shift+Enter tests (lines 212, 243, 269, 381: "Shift+Enter sends ESC then CR",
  "plain Enter sends bare CR", "Ctrl+Shift+Enter does not trigger this fix's own ESC injection", "a plain Enter after
  the chord stays bare"). They fail deterministically — every run, Chromium and WebKit alike — with the same transcript:
  the raw-mode fixture prints `RAWREADY` and the bytes the test types never reach the pane (`waiting for " 7a"` against
  a transcript that ends at the marker). Observed 2026-08-23 on a four-core Ubuntu 26.04 box with Playwright's own
  browsers, on the stack tip with tmux 3.7c, on a pre-floor tree with both 3.7b and 3.7c, and on a near-main tree (main
  plus two test-only commits) with 3.7b, so neither the tmux floor stack nor the tmux version is the cause; the browser
  end-to- end suite does not run in CI, so when this started is unrecorded. Every other test in the file passes,
  including the ones that type into the same fixture, so the first step is to diff what these four do before typing (the
  chord arming, the focus dance) against a passing sibling, and to check whether Playwright's keyboard delivers
  Shift+Enter the way the fix expects in current browser builds — a changed key event shape would explain all four at
  once.

- Deflake `sort.spec.ts`'s `an incomplete non-created walk resolves the newest session with the helm` in FULL suite
  runs. On 2026-08-23 it failed on both engines in a complete two-engine run (the titlebar resolved `e2e-session` where
  `sortfallback-zzz` was expected) and then passed 28/28 when its spec ran alone on the same tree. Every Playwright
  project shares the one helm stack `start-stack.sh` boots, so "the newest session" depends on what other specs have
  created by the time this test runs — another spec's `beforeAll` recreates `e2e-session`, and the resolution picks
  whichever is newer at that instant. First steps: have the test pin "newest" to a session it creates inside the test (a
  fresh title, created after any shared fixture) instead of assuming nothing else is being created, or mark the shared
  `e2e-session` so the walk excludes it.

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
