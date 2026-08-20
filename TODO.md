# TODO

A running list of things the maintainer wants fixed or built, in no particular order. This is intent, not history: an
entry is REMOVED in the same PR that addresses it, so the file only ever describes what is still wanted. It is not a
roadmap and carries no priorities unless an entry says so itself.

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

- Deflake `terminal_tabs::a_killed_supervisor_leaves_no_orphaned_sink_client` on CI. It failed three CI runs on
  2026-08-16/17 with "a control client outlived the supervisor that owned it: 1 still attached"
  (crates/farhelm/tests/e2e/terminal_tabs.rs, the 20-second drain loop after the SIGKILL), each time passing on a plain
  rerun of the same commit, and it has never failed locally — including three full-suite runs on the 12-core devbox the
  same day. Failing runs for reference: actions/runs/31953418151/job/95180287538 (on #172, so it predates the PaneProbe
  work entirely), actions/runs/31980616526/job/95246609920 and actions/runs/31992550444/job/95278542779 (on #174). What
  the test pins: a SIGKILLed supervisor's three tmux control clients (output, input, session sink) must all die from
  stdin EOF when the kernel reaps the supervisor's pipe ends — teardown by protocol, not by cleanup code. On CI (4
  vCPUs, `--test-threads=4`, three other process-heavy tests running concurrently) exactly one client is still attached
  when the 20s deadline expires. Unknown which of the three it is — the assertion only counts. First steps: make the
  failure say WHICH client survives (`list-clients -F` output in the panic, not just the count); then decide between the
  two candidate mechanisms — the deadline is simply too tight under CI load, or a concurrently spawned tmux client
  process inherited a duplicate of another client's stdin write end (missing CLOEXEC would keep EOF from ever arriving,
  and would also explain why an idle rerun always passes). The second one would be a real bug in exactly the guarantee
  the test exists to pin, so do not reach for a bigger timeout until the survivor's identity rules it out.

- Deflake desktop-smoke's clean-exit leg on CI. Twice on 2026-08-16/17 the gate failed with "desktop app did not exit
  cleanly" — scripts/desktop-smoke.sh's final leg sends alt+F4 via xdotool and gives the app 10 seconds (20 × 0.5s) to
  leave the process table — immediately after the "rotating the token and refreshing both client stacks on 401" step.
  Same commit failed and then passed on rerun (actions/runs/31990446660: job/95272892308 fail, job/95274891991 pass;
  also actions/runs/31979811670/job/95244680072), and the leg has never failed locally, so this is nondeterministic, not
  content-driven. Candidate mechanisms, unverified: the alt+F4 keystroke races window focus under openbox on a loaded
  runner (xdotool windowactivate returns before focus actually lands, so the chord goes nowhere and nothing was ever
  asked to exit), or the just-rotated-token state leaves the app genuinely slow or stuck on its shutdown path — the
  script keeps `failure.png` and the state dir on failure, so the next CI failure's artifacts can distinguish "window
  still up, never got the keystroke" from "window gone, process wedged". First steps: upload the kept state as an
  actions artifact on failure (today it dies with the runner), and make the leg re-deliver alt+F4 once after
  re-verifying focus before concluding the app is wedged; only then consider whether 10s is honestly enough on a 4-vCPU
  runner.

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

- Run the manual Mac checklist (`docs/manual-mac-checklist.md` — that file IS the record; its "Observed:" fields are the
  state, all "not run"). Blocked on the macOS release bundle above and a human with a real Mac. Not covered by any CI:
  Playwright's WebKit is not WKWebView.

- Decide the count banner's denominator: "N matching of M sessions" counts the whole fleet as M while the default view
  excludes archived sessions, so the two numbers disagree out of the box. Shipped choice: fleet total (archived sessions
  are still fleet members); alternative: exclude archived so the default view is self-consistent. Cheap UX verdict,
  worth making explicitly.

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
