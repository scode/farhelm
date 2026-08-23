# TODO

A running list of things the maintainer wants fixed or built, in no particular order. This is intent, not history: an
entry is REMOVED in the same PR that addresses it, so the file only ever describes what is still wanted. It is not a
roadmap and carries no priorities unless an entry says so itself.

- Finish the macOS release bundle. The release workflow's `macos` job (workflow_dispatch-gated; builds an
  aarch64-apple-darwin `Farhelm.app` with embedded helm, managed supervisor, and the CLI at `Contents/MacOS/farhelm`)
  has never completed, which means the README's primary quickstart references an artifact that does not exist. The tmux
  floor decision (SPEC_impl.md's "Terminal substrate" section) already dropped the job's private darwin tmux build, its
  cache, and the bundle-assembly copy — the app now requires Homebrew's tmux and probes for it
  (`crates/farhelm-ui/src/desktop.rs`) — so what remains is dispatching a run and seeing the never-run darwin steps
  through for the first time: `dx bundle` and the bundle assembly's `find` for dx's `.app` output. One product gap found
  in review and not yet closed: when the managed supervisor refuses its tmux (missing, or below the floor), its message
  goes to inherited stderr and the app exits before a window exists, so a Finder launch shows nothing — the refusal the
  quickstart promises needs capturing and presenting (an alert or a minimal startup-error window) before the Mac build
  is called done. Both prior failures (23 minutes spent of a standing 180-minute macOS-runner budget) were inside that
  now-removed tmux build: attempt 1 died on ncurses terminfo installation under case-insensitive APFS (fixed —
  `--disable-db-install` + system terminfo); attempt 2 died because tmux's darwin configure demands an explicit utf8proc
  decision — deliberate upstream and still present in 3.7c/master, which auto-tries utf8proc on darwin and falls back to
  the same error. Should the private darwin build ever come back (a bundled fallback, say), the decisions already made
  for it stand: `--disable-jemalloc` (3.7c's darwin configure refuses to guess about it, as it already did about
  utf8proc; the build links nothing but its own prefix), `--disable-utf8proc` (macOS's stale `wcwidth(3)` may draw newer
  Unicode a cell off in Mac-local sessions; enabling means a fourth pinned static source, installed as an archive only
  and linked by path), plus an `otool -L` assertion that every load command is under `/usr/lib/`, because macOS cannot
  link a fully static executable, `ld` prefers a `.dylib` over a `.a` in the same directory, and a leaked dynamic
  library yields a tmux that works on the runner and dies on every user's Mac with `dyld: Library
  not loaded` — the
  darwin leg of the script's prefix isolation has never executed on a Mac. Then tag the release commit and
  `gh workflow run release.yml --ref <tag> -f release_tag=<tag>`, counting the job against the remaining 157 minutes.
  Unblocks the manual Mac checklist (`docs/manual-mac-checklist.md`) and the README quickstart.

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

- UI refresh (brainstormed 2026-08-22 from screenshots of the web UI): a set of chrome tweaks to make the shell read as
  modern rather than as the "minimal M1 chrome, nothing decorative" placeholder app.css still declares itself to be. The
  terminal itself is off-limits throughout — fidelity to the agent's real TUI is the product — and the sidebar stays
  fixed-width (a recorded decision; denser rows solve the same pain). Each sub-item is its own PR, and every one of them
  builds on the design-token layer app.css now has (its `:root` block) rather than on literals:

  - Status as dot plus timestamp, not a word. Replace the `running`/`idle`/`exited` text with a color-coded dot beside
    the title (pulsing for running) and put a relative last-activity timestamp (`2m`, `1h`) where the word was — it
    makes the "recently active" sort visibly meaningful.

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
