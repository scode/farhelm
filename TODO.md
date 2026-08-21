# TODO

A running list of things the maintainer wants fixed or built, in no particular order. This is intent, not history: an
entry is REMOVED in the same PR that addresses it, so the file only ever describes what is still wanted. It is not a
roadmap and carries no priorities unless an entry says so itself.

- Add a visual highlight in the left-hand sidebar showing clearly which agent session the main pane is currently
  interacting with. The selected row should be obvious at a glance, not inferable only from the titlebar.

- Make Shift+Enter insert a line break everywhere it plausibly should, not just in Claude Code. The terminal sends ESC
  CR for the chord (the binding Claude Code's terminal-setup guide names, and it works there), but observed behavior
  elsewhere: Codex treats the pair like a plain Enter (or close to it) — so Codex evidently does not honor ESC CR as
  insert-newline and the right sequence for it needs investigating (candidates: whatever Codex's own terminal-setup
  binds, or the CSI-u encoding `ESC [ 13 ; 2 u` that kitty-protocol-aware TUIs understand) — and a plain shell tab does
  nothing visible with it. Decide and implement per-target behavior: Codex must get a real line break, and define what
  (if anything) the chord should do in a bare shell rather than leaving it a silent no-op.

- Finish the macOS release bundle. The release workflow's `macos` job (workflow_dispatch-gated; builds an
  aarch64-apple-darwin `Farhelm.app` with embedded helm, managed supervisor, private tmux, and the CLI at
  `Contents/MacOS/farhelm`) has never completed, which means the README's primary quickstart references an artifact that
  does not exist. Two failed attempts so far, 23 minutes spent of a standing 180-minute macOS-runner budget: attempt 1
  died on ncurses terminfo installation under case-insensitive APFS (already fixed — `--disable-db-install` + system
  terminfo); attempt 2 died because tmux 3.7b's darwin configure demands an explicit utf8proc decision.
  Believed-complete next step: add `--disable-utf8proc` to the darwin tmux configure in `scripts/build-private-tmux.sh`
  (utf8proc is not a pinned source, and tmux runs without it on Linux); budget for one more configure-iteration stop
  behind it. With the old stack long merged, the stacked-history mechanics in the original mop-up entry no longer apply:
  land the script fix on main, tag the release commit, and
  `gh workflow run release.yml --ref <tag> -f release_tag=<tag>`, counting the job against the remaining 157 minutes.
  Unblocks the manual Mac checklist (`docs/manual-mac-checklist.md`) and the README quickstart NOTE.

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

- Deflake `terminal_tabs::a_tab_whose_shell_is_dead_by_reply_time_is_refused_with_its_last_words` if it ever fires
  outside an artificial recipe. The test's premise is a race it assumes it wins: the fixture shell (`exit 9`) must be
  dead BEFORE `open_tab`'s reply, so the open is refused with the shell's last words. Under a deliberately brutal repro
  recipe (2026-08-18: THREE concurrent e2e test binaries pinned to the terminal_tabs module on a 4-core box — harsher
  than any real runner) the dying shell was routinely delayed past reply time, `open_tab` returned a live TabInfo, and
  the `expect_err` at terminal_tabs.rs:428 fired in roughly half the iterations. It has never failed on CI or under a
  normal local run, which is why this is recorded rather than fixed: the honest fix (have the test wait for the pane to
  be observed dead before opening the tab, or drive the shell's death through a barrier instead of a sprint) is real
  work, and the failure mode is currently confined to a recipe nothing real resembles. If it shows up on CI, this entry
  has the mechanism ready. Same recipe, same class, one occurrence and recorded here rather than separately:
  `a_tab_runs_a_shell_in_the_sessions_working_directory` failed once with "timed out waiting for XREADYX", transcript
  empty. The interesting part is WHERE it failed: `wait_for_shell` (terminal_tabs.rs) retries a 3-second shell round
  trip inside a 30-second budget, but each round's inner `wait_for` panics at its OWN 3-second deadline, racing the
  outer `tokio::time::timeout` that was supposed to convert the round into a retry — when the panic wins, one slow first
  round fails the test with 27 seconds of budget unspent. Fixing the ladder (give the inner wait no deadline of its own,
  or a longer one than the wrapper) would make the observed failure impossible without changing what is pinned. (The
  same campaign's third finding — SIGKILLing a supervisor with queued control output can abort the tmux server itself —
  lives in BUGS.md: it is a substrate defect nothing here can fix, not wanted work.)

- Deflake `boot_id_durable_outcome::a_list_polling_through_a_stop_never_erases_the_annotation` under local full-suite
  load. One occurrence so far: 2026-08-18, full workspace suite at libtest's default thread count (one runnable test per
  logical CPU — six on this devbox); passed in isolation immediately after, and has never failed on CI. The assertion at
  boot_id_durable_outcome.rs:606 expected `Exited` and got
  `Error { "the agent was never started: the launch never reached farhelm's exec shim, so
  something before it — the transient cgroup scope wrapper, or the login shell itself — exited first" }`
  — the launch-artifact classifier's never-started verdict (launch_artifacts.rs), meaning the failure is UPSTREAM of the
  list-versus-stop behavior under test: the session's launch itself died before exec'ing the shim, so the stop recorded
  an error rather than the annotated exit. Candidate mechanisms, unverified: under full-suite process-heavy load the
  systemd user manager is slow or resource-starved enough that the transient scope wrapper or the login shell inside it
  exits before reaching the shim (this box has a user manager, so launches take the scoped path; CI runners have none
  and take the unscoped path, which would make this a local-only shape); or a concurrent test's scope sweep caught the
  launch mid-flight. First steps: reproduce with a full-suite loop at the default thread count; make `basic_session`
  (harness.rs) optionally wait until the fake agent's READY marker lands (a post-exec sentinel — a merely live pane is
  NOT enough, since the scope wrapper or login shell keeps the pane alive before exec and can still die in exactly the
  pre-shim way under investigation), so a launch that never started fails setup by name instead of corrupting the
  assertion under test — every basic_session caller shares this exposure — and capture the scope wrapper's stderr on the
  never-started path so the next occurrence says which of the two candidates it was.

- Deflake `provisioning::tests::local_provisioning_and_update_preserve_a_running_session` under heavy CPU contention.
  One occurrence so far: 2026-08-18 on the 6-core devbox, panic "real provisioning run did not finish: deadline has
  elapsed" (crates/farhelm-helm/src/provisioning.rs, the bounded wait around the real local provisioning run), during a
  full workspace suite that was — honest context — sharing the box with an unrelated multi-agent review workload, so
  total load was well above a normal full-suite run. Passed in isolation right after, taking 79s alone, which says the
  test's real work is already a large fraction of whatever deadline it is given. Candidate mechanism, unverified: the
  deadline is sized for an idle box and the test loses it under CPU contention rather than anything being wedged. First
  steps: find the deadline constant behind that panic and measure the test's runtime distribution under a loaded
  full-suite loop; if the margin is thin, either give the bound honest headroom or make the panic distinguish "no
  progress" from "progress but slow" so a genuine wedge stays loud.

- Deflake `session_lifecycle::reattach_replays_history_and_modes` on four-thread CI. The exact fingerprint is a replay
  that contains the later `before-reattach` marker but has lost the earlier `FAKE-AGENT READY` line, ending at
  session_lifecycle.rs's `replay missing pre-detach history` assertion. A scan of the preceding 200 CI runs found eight
  occurrences (4%); three failed commits also had a simultaneous successful run, and PR #194 repeated that same
  pass/fail pair on one commit SHA. This proves the result is intermittent, not whether the cause is fragile test timing
  or a rare product replay-boundary race. First steps: loop the exact test under CI-like process load while retaining
  the full replay transcript and tmux capture at detach and reattach, then locate whether the earlier history is absent
  from tmux, lost while Farhelm constructs the replay, or dropped by the test's receive window. Do not merely lengthen
  the wait: the assertion runs after the later marker has already arrived, so more time cannot restore bytes that should
  precede it. Once the boundary that loses the line is known, replace the race with a deterministic barrier and keep a
  regression for the discovered mechanism.

- Deflake `terminal.spec.ts`'s `list renders the session row with title, cwd, invocation, and a running badge`. During
  the Design 1 split it failed once in a focused Chromium run and once under WebKit in the full two-engine suite: the
  shared session row was present with the expected metadata, but its badge stayed `idle` through the 10-second assertion
  instead of becoming `running`. The same unchanged test passed in the full suite's Chromium half, and the split changed
  neither the test body nor the runtime that computes session status. This establishes intermittency without deciding
  whether the listing missed an invalidation or the supervisor was late to classify the already-running session. First
  steps: retain timestamped `/api/sessions` replies and feed invalidations from page load through the assertion, then
  correlate them with the supervisor's status observations. Fix the missing transition or notification once its boundary
  is known; do not merely lengthen the assertion timeout.

- Deflake `terminal-reconnect.spec.ts`'s `takeover-during-backoff-does-not-steal-the-session` under WebKit. In the full
  two-engine suite, Chromium passed but WebKit reached the taken-over banner, reclaim control, and uninterrupted-winner
  assertions with the losing page's island registry still containing `"terminal"`; the test expected the refused
  reconnect attempt to leave that registry empty. The test had also passed in its focused Chromium project before the
  split, whose only change was moving the test into its own spec file. First steps: record the losing page's reconnect
  rung, socket close, refused attach, `cancelReconnect("restore")`, island teardown, and any replacement mount as one
  ordered trace. That should show whether WebKit delays teardown or opens a replacement after the refusal. Replace the
  race with a deterministic lifecycle barrier once the ordering is known; do not hide it behind a longer sleep.

- Dispose of prerelease v0.0.3-rc.1 — the release AND the tag together — once a real release exists. Both were minted
  only to exercise the release workflow (it checks out and verifies `refs/tags/<release_tag>` before building, so a real
  tag was required). Not before a real release exists: rc.1 currently carries the only published Linux artifact, so
  deleting it today would leave the README's install path pointing at an empty releases page. Deleting only the release
  would leave a tag pointing into abandoned pre-merge history — remove both (`gh release delete v0.0.3-rc.1` and
  `gh api -X DELETE repos/<owner>/<repo>/git/refs/tags/v0.0.3-rc.1`).

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
  state, all "not run"). Blocked on the macOS release bundle above and a human with a real Mac. Not covered by any CI:
  Playwright's WebKit is not WKWebView.

- Decide the count banner's denominator: "N matching of M sessions" counts the whole fleet as M while the default view
  excludes archived sessions, so the two numbers disagree out of the box. Shipped choice: fleet total (archived sessions
  are still fleet members); alternative: exclude archived so the default view is self-consistent. Cheap UX verdict,
  worth making explicitly.

- Add a "sort by" control for the left sidebar's session rows. Default to most recently active, with alphabetical and
  newest-created options; newest-created is the current behavior (`created_at` descending). Define the activity signal
  and stable tie-breakers when implementing it so equally active rows do not jump around nondeterministically.

- Reassess the host row's component-prop shape: the earlier "regroup when props are actively growing" condition has
  fired (provisioning grew it to 20 props). Only with a memoization-preserving grouping — state-only structs, never
  callback structs (the framework's callback-prop memoization does not survive struct nesting; the session row learned
  this) — and with a host-row render-count regression test like the session row's.

- Desktop cross-restart selection memory: browser clients remember the last-selected session across reloads
  (localStorage `{helm, id}` record); the desktop app remembers only within a process, so a relaunch auto-attaches the
  newest-created session. SPEC.md documents exactly this, so nothing is WRONG — it was deferred because the webview's
  localStorage is not synchronously reachable from native Rust, not because the fallback is right. If desktop use makes
  it annoying: persist the record native-side (the desktop bootstrap owns a state dir) and restore it before the
  fallback in `list.rs`'s auto-select effect.
