# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Three buckets, assigned by the maintainer: "near term" is what should be picked up next; "maybe later" is wanted but not
soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either way. Within
a bucket, no order.

## Near term

- Show the missing-or-too-old-tmux refusal in a native window, not just on stderr. `farhelm-desktop`'s tmux preflight
  now prints one plain message and exits (see `desktop.rs`'s `run_tmux_preflight_or_exit`), but a Finder launch has no
  terminal for stderr to land in, so that message currently reaches nobody there. Unverifiable without a Mac to try it
  on, so deferred rather than guessed at.

## Maybe later

- Custom hover tooltips on buttons and menu items. Native `title` tooltips are free (the UI already uses them on the
  activity time, the cwd line, the profile chip and the header's archive button) but the browser owns their ~1s delay
  and nothing — no CSS, attribute, or JS — shortens it; WebKit's web content ignores the macOS tooltip-delay default
  too. A faster, themed tooltip is a component shown on hover after a delay of the app's own choosing (~300ms), and it
  has to escape the sidebar: `.app-sidebar`'s `overflow: hidden auto` clips anything anchored inside a row near its
  edges, so the tooltip needs a body-level portal or `position: fixed` with measured coordinates — the row `…` menu's
  popover is the pattern to copy. If the native delay turns out tolerable, a `title` pass over the terse actions (stop /
  archive / delete, the host row's buttons) is an hour and needs none of this.

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

- Review the two security-relevant resolutions made during the M7 run, and record a dated verdict (a note in
  SPEC_impl.md or a review comment on PR #117). (a) A product-spec/impl-spec conflict over where the web token is stored
  was resolved in the product spec's favor, with the impl spec amended in #117 — confirm the amendment says what the
  code does. (b) The web credential's transport was redesigned mid-run from an HttpOnly cookie to localStorage + an
  Authorization bearer header + a WebSocket subprotocol, because cookies are host-scoped rather than port-scoped:
  another local user's loopback-port server could lure the browser and replay the cookie under a forged Origin. The
  deliberate trade: giving up HttpOnly means in-origin XSS can read the credential, judged acceptable because in-origin
  XSS already has the API authority the credential grants. The review is completable from this checklist alone — verify
  that: the master token is stored recoverably server-side and `farhelm helm token show|rotate` both work after a helm
  restart; device secrets are stored hash-only; the browser credential lives in localStorage with no ambient cookie sent
  or honored anywhere; REST carries it as a bearer header and WebSockets as a subprotocol; and rotation both 401s every
  existing device session and drops already-open feed/terminal sockets. Implementation surfaces: the helm's auth and
  middleware modules, the UI's auth/api modules. Spec surfaces: SPEC.md's token section, SPEC_impl.md's auth/storage
  section as amended in #117.

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
