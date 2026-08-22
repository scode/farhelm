# TODO

A running list of things the maintainer wants fixed or built, in no particular order. This is intent, not history: an
entry is REMOVED in the same PR that addresses it, so the file only ever describes what is still wanted. It is not a
roadmap and carries no priorities unless an entry says so itself.

- Consider always running the bundled, pinned tmux instead of gating on a minimum host version. Today a host tmux at or
  above the floor (3.3) is accepted and the private build is used only when the host's is missing or too old
  (SPEC_impl.md's terminal-substrate section) — for Linux provisioning and any ordinarily started supervisor, that is;
  the desktop app already puts its bundle directory first on its managed supervisor's PATH, so there the bundled tmux
  wins even beside a supported host copy. That policy treats tmux versions as interchangeable above the floor, and
  experience says they are not: the supervisor's tmux driver is full of behavior audited per version (3.3a, 3.4, 3.7b
  each differ in ways that shaped real code), the pinned 3.7b build has its own crash-regression suite
  (`scripts/test-tmux-3.7b-shutdown.sh`), and on 2026-08-16 the bundled 3.7b server segfaulted and took every session
  with it — BUGS.md records a related abort class on distro 3.6. For a substrate this sensitive, "the exact version we
  built, tested, and ship" may be a better bet than "anything at or above N", and raising the floor does not buy that. A
  decision would have to weigh: build and packaging cost on every supported platform (Linux release tarballs already
  carry a private build; the Mac app must, since macOS ships none), losing the distro's patched tmux and its security
  updates, operators who expect their own tmux to be used, and whether the floor check stays as a fallback when the
  bundle is absent (a from-source install today has no bundle at all). This is a consider, not a decision.

- Finish the macOS release bundle. The release workflow's `macos` job (workflow_dispatch-gated; builds an
  aarch64-apple-darwin `Farhelm.app` with embedded helm, managed supervisor, private tmux, and the CLI at
  `Contents/MacOS/farhelm`) has never completed, which means the README's primary quickstart references an artifact that
  does not exist. Two failed attempts so far, 23 minutes spent of a standing 180-minute macOS-runner budget: attempt 1
  died on ncurses terminfo installation under case-insensitive APFS (already fixed — `--disable-db-install` + system
  terminfo); attempt 2 died because tmux's darwin configure demands an explicit utf8proc decision. That demand is
  deliberate upstream and not going away: 3.7b errors outright when neither flag is given, and 3.7c/master instead
  auto-tries utf8proc on darwin and falls back to the same error when it is not found — an upgrade would only change who
  makes the decision, and a silently found Homebrew utf8proc is exactly what the private build must not link. The
  decision is `--disable-utf8proc` (2026-08-22): add it to the darwin `tmux_configure` in
  `scripts/build-private-tmux.sh`. Cost: macOS's stale `wcwidth(3)` misreports widths for newer Unicode (emoji
  sequences, CJK extensions), so Mac-LOCAL sessions may draw those glyphs a cell off; remote Ubuntu sessions run Linux
  tmux and are unaffected. Enabling it properly means a fourth pinned static source, and is worth revisiting only if
  Mac-local rendering becomes a real complaint. Do the same change with a link-isolation assertion, decided at the same
  time: after `make`, in the darwin case, fail the build unless every load command in `otool -L tmux` is an Apple system
  library under `/usr/lib/`. The concern it closes: macOS cannot link a fully static executable, so "static" here means
  every non-Apple library is a private `.a` under the script's prefix — and the darwin leg of that isolation (the
  `--disable-shared`/`--without-shared` builds, the `PKG_CONFIG_LIBDIR` restriction, the ncurses symlink) has never
  actually executed on a Mac. `ld` prefers a `.dylib` over a `.a` in the same directory, so any dynamic library that
  reaches the prefix or a pkg-config-leaked `-L/opt/homebrew/lib` would produce a tmux that works on the runner and dies
  on every user's Mac with `dyld: Library not loaded`. Linux gets this guarantee for free from `-static`; darwin
  currently has nothing, and the assertion turns "we believe the prefix isolation works" into a build failure if it does
  not, at the cost of one `otool` call. It also makes a later `--enable-utf8proc` safe to attempt (the
  dylib-beside-archive trap is the main reason not to), provided utf8proc is then installed as an archive only and
  linked by path. Then: land both script changes on main, tag the release commit, and
  `gh workflow run release.yml --ref <tag> -f release_tag=<tag>`, counting the job against the remaining 157 minutes.
  Budget for one more configure-iteration stop behind the utf8proc one; everything after tmux's configure — the
  ncursesw/ncurses symlink trick, the prefix-restricted pkg-config, `dx bundle`, and the bundle assembly's `find` for
  dx's `.app` output — has never run on darwin either. If a Mac is available, run
  `scripts/build-private-tmux.sh aarch64-apple-darwin <out>` there first; it costs no CI budget and flushes out anything
  past the utf8proc stop. Unblocks the manual Mac checklist (`docs/manual-mac-checklist.md`) and the README quickstart
  NOTE.

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

- Fix `scripts/desktop-smoke.sh`'s "relaunched page did not request sort=title" failure on CI (first seen 2026-08-22 on
  PR #210's `desktop-smoke` job, run 32584494800; passes locally on the devbox and on a fresh Ubuntu 26.04 worker). This
  is NOT a timing flake, and the feature under test is working: the run's `desktop-smoke-failure` artifact holds
  `desktop-restart.log`, and it contains the restored listing requests two seconds after relaunch — three
  `desktop_smoke: session listing requested query=sort=title` lines between 16:35:17 and 16:35:26, well inside the
  script's 30-second window. What the script greps for (`desktop_smoke.*query=sort=title`, near line 439) never matches
  that log because tracing's ANSI styling is on in the CI job's log: the bytes are `[3mquery[0m[2m=[0msort=title`, so
  `query=sort=title` is not a literal substring. The local runs pass because their log carried no escape codes.
  Candidate fixes, in order of preference: make the emitting hook (`crate::desktop::log_smoke_session_query`,
  crates/farhelm-ui/src/desktop.rs) write its own plain line to the smoke log instead of relying on the tracing
  subscriber's formatting, or have the script strip escapes (`sed
  's/\x1b\[[0-9;]*m//g'`) before grepping, or pin
  `NO_COLOR`/`RUST_LOG_STYLE=never` for the relaunched app; whichever is chosen, make the first-boot leg assert the same
  way so both legs share one oracle. Worth checking the same run's tmux `list-clients` assertion that follows (the
  remembered, non-newest session should own the page's output client), which never ran because the grep failed first;
  the same artifact's `failure.png` is the relaunched window. Also decide whether the trace hook should read an env var
  at every call — it currently checks `FARHELM_SMOKE_CLIENT_LOG_MARKER` per listing walk, which is fine for a smoke-only
  hook but a reviewer may ask.

- Deflake `manager::tests::default_changed_alone_bumps_the_fleet_revision` (crates/farhelm-helm/src/manager.rs) if it
  fires again. One occurrence: 2026-08-22, CI `test` job on PR #206 at e44900f9 (run 32561406561); the same job passed
  on the PR's previous head two hours earlier with identical manager/profile code, and the helm lib suite passed
  repeatedly on the devbox with the test in it. The assertion at the end of the test expected the remembered default to
  read `profile-a` and got `profile-b` — i.e. the refresh's disappearance repair (which is what should overwrite the
  `profile-b` the test itself planted) had not landed within the test's three `advance(REFRESH_INTERVAL)` + `yield_now`
  iterations on the paused tokio clock. That loop breaks early when the store already shows `profile-a`, so the shape is
  a bounded poll that can lose to actor scheduling, not a wrong expectation. First steps: widen the poll to a
  deadline-bounded wait on the store value (or on the fleet revision bump the test is actually about) rather than a
  fixed three ticks, and confirm under `--test-threads=4` with the rest of the helm suite running.

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

- Reassess the host row's component-prop shape: the earlier "regroup when props are actively growing" condition has
  fired (provisioning grew it to 20 props). Only with a memoization-preserving grouping — state-only structs, never
  callback structs (the framework's callback-prop memoization does not survive struct nesting; the session row learned
  this) — and with a host-row render-count regression test like the session row's.
