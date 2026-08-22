# TODO

A running list of things the maintainer wants fixed or built, in no particular order. This is intent, not history: an
entry is REMOVED in the same PR that addresses it, so the file only ever describes what is still wanted. It is not a
roadmap and carries no priorities unless an entry says so itself.

- Adopt an aggressive tmux version floor (decided 2026-08-22) in place of today's "any host tmux at or above 3.3"
  policy. Today a host tmux at or above the floor is accepted and the private build is used only when the host's is
  missing or too old (SPEC_impl.md's terminal-substrate section). That treats tmux versions as interchangeable above the
  floor, and experience says they are not: Farhelm exercises the nooks and crannies of tmux — control mode,
  output-client teardown, pane-death timing — and the supervisor's driver is full of behavior audited per version (3.3a,
  3.4, 3.7b each differ in ways that shaped real code); crashes have been observed on older versions (the distro 3.4
  server hosting every production session on the devbox died in BUGS.md's fatal()-abort shape on 2026-08-19, and BUGS.md
  records the same abort class reproduced on distro 3.6), and the pinned 3.7b build has its own crash-regression suite
  (`scripts/test-tmux-3.7b-shutdown.sh`). The alternative of always running a bundled static tmux was considered and
  rejected: it loses distro security patching (tmux AND libevent, which is where the 2026-08-16 3.7b segfault lived),
  concentrates a bad build's blast radius on every host at once, needs a static build per platform — the darwin one has
  never completed — and a from-source install has no bundle to run anyway, so a floor check survives as the fallback
  regardless. The decision, in three parts: (1) The floor is the regression-tested version, currently 3.7b, bumped
  deliberately together with the regression suite — not "whatever Homebrew ships today". Be explicit in the spec that
  this floor is DESIGNED to exclude current distro packages (Ubuntu 24.04 ships 3.4, 26.04 about 3.6, Debian 13 and
  Fedora 42 3.5a): on Linux this is close to always-bundled in practice, with the opt-outs below. Versions above the
  floor are accepted; consider a tested-through ceiling that warns rather than refuses, since Homebrew will ship 3.8
  before anyone has audited it. (2) User instructions route through Homebrew: on macOS the app requires
  `brew install tmux` (3.7c today) and probes the known prefixes itself (`/opt/homebrew/bin`, `/usr/local/bin`,
  `/opt/local/bin`) because GUI apps do not inherit the shell PATH; on Linux, Linuxbrew is the documented way to meet
  the floor without taking Farhelm's static build, and provisioning keeps installing the private musl build when the
  host has nothing acceptable. The quickstart grows a "brew install tmux" step and a clear first-run refusal that names
  the version found and the floor. (3) A simple, documented "pick your own" override — one knob (a `--tmux <path>` flag
  / `FARHELM_TMUX` / config key) honored by every launch path, desktop app included — with documented caveats: the
  chosen binary is floor-checked like any other candidate and refused by name if too old; Farhelm drives tmux harder
  than interactive use does, versions below the floor have crashed under it, and versions above the tested one are
  unaudited, so the override is "you own the substrate", not a supported configuration. The devbox already runs this
  policy by hand (Homebrew tmux pinned at or above 3.7c behind a `tmux-gate` ExecStartPre); this is making that the
  product. Changes SPEC.md / SPEC_impl.md's terminal-substrate sections, so surface the conflict there rather than
  quietly editing.

- Finish the macOS release bundle. The release workflow's `macos` job (workflow_dispatch-gated; builds an
  aarch64-apple-darwin `Farhelm.app` with embedded helm, managed supervisor, and the CLI at `Contents/MacOS/farhelm`)
  has never completed, which means the README's primary quickstart references an artifact that does not exist. Under the
  tmux floor decision above, the job no longer needs to build a private darwin tmux at all — the app requires Homebrew's
  tmux and probes for it — so the first step is removing the "Build Apple arm64 tmux" step, its cache, and the
  `cp … Contents/MacOS/tmux` from the bundle assembly, then dispatching. Both prior failures (23 minutes spent of a
  standing 180-minute macOS-runner budget) were inside that tmux build: attempt 1 died on ncurses terminfo installation
  under case-insensitive APFS (fixed — `--disable-db-install` + system terminfo); attempt 2 died because tmux's darwin
  configure demands an explicit utf8proc decision — deliberate upstream and still present in 3.7c/master, which
  auto-tries utf8proc on darwin and falls back to the same error. Should the private darwin build ever come back (a
  bundled fallback, say), the decisions already made for it stand: `--disable-utf8proc` (macOS's stale `wcwidth(3)` may
  draw newer Unicode a cell off in Mac-local sessions; enabling means a fourth pinned static source, installed as an
  archive only and linked by path), plus an `otool -L` assertion that every load command is under `/usr/lib/`, because
  macOS cannot link a fully static executable, `ld` prefers a `.dylib` over a `.a` in the same directory, and a leaked
  dynamic library yields a tmux that works on the runner and dies on every user's Mac with `dyld: Library not
  loaded`
  — the darwin leg of the script's prefix isolation has never executed on a Mac. The remaining unknowns are the
  never-run darwin steps after that: `dx bundle` and the bundle assembly's `find` for dx's `.app` output. Then tag the
  release commit and `gh workflow run release.yml --ref <tag> -f release_tag=<tag>`, counting the job against the
  remaining 157 minutes. Unblocks the manual Mac checklist (`docs/manual-mac-checklist.md`) and the README quickstart
  NOTE, both of which must also gain the Homebrew tmux step.

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

- UI refresh (brainstormed 2026-08-22 from screenshots of the web UI): a set of chrome tweaks to make the shell read as
  modern rather than as the "minimal M1 chrome, nothing decorative" placeholder app.css still declares itself to be. The
  terminal itself is off-limits throughout — fidelity to the agent's real TUI is the product — and the sidebar stays
  fixed-width (a recorded decision; denser rows solve the same pain). Each sub-item is its own PR; the token item goes
  first (see it for why):

  - Introduce design tokens in app.css — RECOMMENDED FIRST, before any other entry (here or elsewhere) that tweaks the
    UI's look, since every later visual change otherwise pays a scattered-edit tax. Today the file has no CSS custom
    properties and ~40 distinct hex literals inline (`#8a919e` 24 times, a long tail of one-offs that are probably
    unintended near-duplicates). Replace with a `:root` block of named roles — surface levels (`--bg-0/1/2`), foreground
    levels, one accent, ok/warn/danger, radius, and `--font-ui`/`--font-mono` — and `var()` at use sites.
    Pixel-identical refactor; audit the one-offs into roles as part of it. Same file, no framework, no build change; a
    light theme later becomes a second `:root` block.

  - Finish the JetBrains Mono conversion. The chrome font is still `system-ui` (app.css's html/body rule) and the
    vendored face is applied only through xterm's `fontFamily`; app.css's own header says nothing else uses it. The
    earlier intent was everything in JetBrains Mono. Use the already-vendored Nerd Font face for chrome too (its Latin
    glyphs are identical to upstream JetBrains Mono, and the browser has already fetched it for the terminal); do NOT
    vendor a second non-Nerd copy. If UI-size line spacing looks off, that is a `line-height` fix, not a font-file one.

  - Sidebar row hierarchy. Rows are four near-equal-weight lines (title, host, full cwd, full invocation), ~120px each.
    Drop the host line entirely when the session is on the helm's own machine (show it only for remote hosts); tilde-
    abbreviate the cwd and, if it still does not fit, hard-cut on the LEFT (right-most segments are the informative
    ones) with the full path on hover; render the invocation compactly (profile name or `claude ·skip-perms` style
    badge) rather than the full command line. Target roughly half the current row height.

  - Status as dot plus timestamp, not a word. Replace the `running`/`idle`/`exited` text with a color-coded dot beside
    the title (pulsing for running) and put a relative last-activity timestamp (`2m`, `1h`) where the word was — it
    makes the "recently active" sort visibly meaningful.

  - Surface layering. Sidebar, rows, header, and terminal all sit on essentially the same background and the selected
    row is barely distinguishable. Use two or three surface levels and a single accent (selection edge/tint, focus
    rings, primary button) — sparingly; this is stared at all day, so hover transitions stay ~100ms and the status pulse
    is the only animation.

  - Consolidate the main-pane header. Title/cwd line, archive button, restart banner, and tab strip currently stack into
    ~170px of chrome before the terminal. Fold title, cwd, status dot, and actions (archive, restart, overflow menu)
    into one ~40px row with tabs beneath; the restart explanation becomes a tooltip or dismissible inline note rather
    than a permanent band.

  - Two-tier buttons. Everything is an outlined gray box. Keep one filled primary (`new session`); everything else
    becomes ghost (no border, hover background). Per-row kebab menus reveal on hover/focus instead of sitting as eight
    identical boxes down the column.

  - Row overflow menus look like a stack of outlined buttons, floating centered over the row with no tie to the `⋯` that
    opened them; with several `⋯` in view there is no way to tell which one is open. Make it an actual menu: one raised
    surface (1px border, soft shadow) anchored to the opening button's corner, full-width left-aligned rows that
    highlight on hover with no per-item borders, a divider before the destructive `delete` (red text, no red box), and
    the `profile: …` line demoted to a small muted footer under its own divider. The opening `⋯` stays in a visible
    pressed/active state and its row gets a subtle highlight for as long as the menu is up. Check that Escape and
    arrow-key navigation work while at it.
