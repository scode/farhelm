# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Five buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove —
the decision is made, only the work remains; "near term" is what should be picked up next; "systematic deflake" is the
plan for making the test suites stop failing under load as a class, distinct from the per-test deflake entries under
"near term" that it is meant to retire; "maybe later" is wanted but not soon, and may never happen; "unbucketized" is
everything not yet sorted, which carries no implication either way. Within a bucket, no order.

## Definite simplification

## Near term

- Deflake `agent_relay::a_helm_that_dies_mid_upcall_ends_the_request_at_once` (crates/farhelm/tests/e2e). Fingerprint:
  `the supervisor never answered the agent request: Elapsed(())` from the peer's 20 s `answer()` budget, panicking at
  the `answer()` expect. It is a load flake that predates the profiles/host-list stack: on 2026-09-02 a 4-vCPU sandbox
  running the e2e binary alone at `--test-threads=4` hit it in 2 of 5 runs on main and 3 of 7 on that stack's tip, while
  16 runs alone passed on both, and every local run on a 6-core machine passed. Never yet seen on a GitHub runner, but
  the release workflow's build gate runs this suite, so it is a tag-build risk. Diagnosed on 2026-09-03 and NOT a test
  budget: with the test included in the loaded four-thread binary it failed 1 of 10 runs, and a sandbox-only run with
  the peer read widened to 60 s (the 10 s promptness assertion kept) then got `ErrorKind::Timeout` instead of
  `Unavailable`, which is the relay's own upcall-answer budget expiring before the helm connection-loss path ran. So the
  product's helm-death detection loses to the upcall budget under load, and neither a wider peer read nor a wider oracle
  would be honest. The fix is product work, needing a maintainer decision: trace the helm control connection's
  reader/demux scheduling and EOF path under a loaded four-thread e2e binary, and make the connection-loss path reach
  `HelmLink::fail_all` before the answer budget can expire. The test stays `#[ignore]`d until then.
- Deflake three profiles-popup Playwright cases that fail only under load (e2e/tests/profiles.spec.ts). All three pass
  locally in both engines, repeatedly, and fail on a 4-vCPU sandbox running the spec with the default worker count
  beside a live helm, supervisor, and both browsers; seen 2026-09-03 at the 0.3.0-rc.1 tip, Chromium only. Start at rung
  3 of `.agents/narrow-tests.md` on a 4-vCPU box; rung 1 does not reproduce any of them.
  - `the profiles popup follows its focus and Escape dismissal contract`: after `locator.focus()` moves focus to the
    host details toggle, the popup is still mounted 5 s later. The focus-out classifier answers `Unknown` when its
    `document.activeElement` eval overruns the 370 ms settlement budget, and `Unknown` never dismisses; the rc.1 code
    retries such an obligation six times at 150 ms and then drops it, and on this box that was still not enough. Either
    the budget scales with observed bridge latency, or a dropped obligation is retried on the next focus event rather
    than forgotten.
  - `unknown then transit waits for the pending focus request`: the popup is gone by the time the test looks 400 ms
    later. This test drives the classifier through the `window.__farhelmTestProfiles` hooks (`classificationErrors`,
    held requests); the retry loop added for the case above re-classifies while those hooks are still armed, and under
    load the retry's sample lands after the held request is released. The test and the retry need one story about which
    classification the hook is holding.
  - `stale focus-out classifiers cannot clear newer obligations`: "the page never opened feed socket #1 (saw 0)" — the
    stubbed feed harness in `helpers/fleet.ts` (`stubFeed`) never saw the page's socket within its wait. Harness, not
    product; the same helper serves every profiles test, so the first look is the wait's length under load.
  - Attempted on 2026-09-03, not shipped. A loaded before leg (10 full-spec runs on a 4-vCPU sandbox, both engines, a
    `cargo build` looping beside Playwright) reproduced: the pending-focus case failed 5/10 on Chromium and 1/10 on
    WebKit, the stale-classifier case 2/10 and 2/10, the focus-and-Escape case 0/10. The attempt made an exhausted
    `Unknown` obligation wait for the next document focus event (reading the event revision when the classifier settles,
    since the `focusin` can land while the bridge is still evaluating), gave the test hook page-local ordinals so a hold
    names the classification it owns, added a quiescence wait before tests arm holds, and widened `stubFeed`'s socket
    wait from 15 s to 30 s. Its loaded after leg failed: the pending-focus case then failed 11/20 on Chromium (the popup
    gone before the test looks, more often than before), and one WebKit focus-and-Escape repeat still found the popup
    mounted. The stale-classifier case was clean. Whoever picks this up next starts from that attempt's diff (kept
    outside the repo) and from why the event-driven retry closes the popup sooner than the pending-focus test expects;
    the two tests and the retry still do not share one story.
- Deflake `a client that stops draining is detached with the stall reason after the full stall interval`
  (e2e/tests/terminal-flood.spec.ts), WebKit. On the same loaded 4-vCPU sandbox, 2026-09-03, the poll "the attachment
  must cross HIGH_WATER and pause before the stall clock can start" saw zero pauses in 30 s: the flood never built
  enough backpressure to pause the client, so the stall clock the test measures never started. The spec and the
  backpressure code were untouched by the stack that surfaced it; the load itself is the difference. Attempted the same
  day without reproduction: 30 loaded WebKit runs of the test on a 4-vCPU sandbox (a `cargo build` looping beside
  Playwright) all passed, and a temporary timer around the gate-to-first-pause interval measured 1.3 to 2.7 s loaded
  (ten runs), 1.3 s unloaded on WebKit and 0.9 s on Chromium, against the poll's 30 s budget. No change was made on that
  evidence. The next attempt needs the full Playwright output of a failing run, since the one sighting's log was not
  kept; if it recurs, the first question is whether the flood finished before the client ever paused.
- Deflake `session_lifecycle::non_utf8_terminal_output_survives_live_stream` (crates/farhelm/tests/e2e), again. On
  2026-09-03, on a 4-vCPU sandbox with a `cargo build` looping beside the tests, 2 of 10 single runs and 1 of 3
  full-binary runs at `--test-threads=4` against 0.3.0 timed out after 40 s waiting for `BINARY-MARKER`: the request
  byte sent through the attachment never produced the fixture's reply within the budget. The fixture is already in raw
  mode before it prints READY (#339), so the first suspect is the input path under load (`send_input`, the supervisor's
  `send-keys` exchange, the fixture's read) rather than the fixture, and the second is the budget. Start at rung 1 of
  `.agents/narrow-tests.md` on a loaded 4-vCPU box and keep the full transcript the harness panic prints; the sandbox
  run that found this kept only a summary. It then failed the same way on a GitHub-hosted runner (CI run 33716079614,
  the `test` job on a docs-only PR of this stack), so it is a CI flake, not only a sandbox one; kept transcripts from
  later sandbox runs show READY present, once in snapshot shape, and nothing at all after the request byte. Localized on
  2026-09-03 and NOT a budget: 8 of 20 loaded single runs failed, and with a temporary test-side barrier (a
  `ListSessions` request queued right after `send_input`; the helm writer keeps frame order and `handle_connection`
  finishes the input handler before reading the next frame, so the list's reply proves the `send-keys` exchange
  completed) a failing run showed `send_input` queued in 11 µs, the barrier replied in 6 ms, and the marker still never
  came in 40 s. So tmux acknowledged the `send-keys` command and the raw-mode fixture never received the byte, or never
  produced its reply. The fix is product work, needing a maintainer decision: make the input-delivery contract detect or
  recover from a `send-keys` that tmux acknowledges without the pane seeing it (or find why the pane does not). Neither
  widening the 40 s wait nor changing the fixture addresses the measurement. The test carries `#[ignore]` since
  2026-09-03, after failing 3 of 9 GitHub runs of one stack in a day, the same move the other load flakes got; the fix
  un-ignores it.
- Deflake `terminal_backpressure::memory_stays_flat_while_a_viewer_is_stalled` (crates/farhelm/tests/e2e): on
  2026-09-03, on a loaded 4-vCPU sandbox running the full binary at `--test-threads=4`, it failed 1 of 3 runs on its 64
  MiB in-process RSS assertion (terminal_backpressure.rs:906 at 0.3.0), not in the flood-start wait its siblings died
  in. It then passed 21 loaded runs (one full-binary baseline plus 20 `terminal_backpressure::` module runs) while the
  flood-start oracle was being fixed, so there is one sighting and no hypothesis; a first look is what else the test
  process held in memory during that run (the module runs 16 tests, 4 threads, each with a 12 MiB flood).
- Deflake `session_lifecycle::attach_with_degenerate_size_still_works` (crates/farhelm/tests/e2e): on 2026-09-03, on the
  same loaded sandbox, `timed out waiting for "FAKE-AGENT READY"` in 1 of 3 full-binary runs on 0.3.0 and 1 of 3 on the
  attach-boundary stack, never alone. No hypothesis yet; start at rung 3 of `.agents/narrow-tests.md` on a loaded 4-vCPU
  box with the full transcript kept.
- Deflake `session_rename::a_renamed_title_survives_a_supervisor_restart` (crates/farhelm/tests/e2e): on 2026-09-03, in
  1 of 3 loaded full-binary runs at `--test-threads=4` on a 4-vCPU sandbox (a `cargo build` looping beside the tests),
  the restart helper it shares with `create_idempotency.rs` panicked in its own setup check, "the replacement must hold
  the state directory's claim, or it reconciles nothing and this test would pass for the wrong reason". One sighting,
  never alone, no hypothesis beyond the fingerprint: the replacement supervisor did not win the state directory's claim
  in time under load. Start at rung 3 of `.agents/narrow-tests.md` on a loaded 4-vCPU box with the full transcript kept.
- Deflake two more profiles cases seen once each in a full browser-suite run on a 4-vCPU sandbox on 2026-09-03 (both
  engines, no extra load beside the suite itself):
  `only layout changes after a profiles opening invalidate its
  geometry` (Chromium) and
  `a saved profile is what the next editor sees, before the re-read lands` (WebKit, which also failed twice in ten
  loaded runs during the profiles attempt above). Same spec, same box, same day as the three cases above; nothing about
  either cause is known beyond the names. Start at rung 1 of `.agents/narrow-tests.md` with `--repeat-each` on the
  engine that failed.
- Put the tmux e2e suite back into the release gate, and un-ignore the two load-flaky tests still ignored, once the
  deflake entries above are done. As of 0.3.0-rc.3 the release build's test step (`.github/dist-build-setup.yml`) runs
  every test target EXCEPT the `farhelm` crate's integration tests, because on the GitHub-hosted 4-vCPU runner one or
  two of the load-sensitive tests above failed on most tag builds (rc.1: three gate runs, rc.2: two), each time a
  different one, with the code they check unchanged; and
  `agent_relay::a_helm_that_dies_mid_upcall_ends_the_request_at_once` and
  `session_lifecycle::non_utf8_terminal_output_survives_live_stream` carry `#[ignore]` so CI's full suite stops rolling
  the same dice (the delete fail-closed test and the two `terminal_backpressure` tests were ignored too, and were
  un-ignored in #355 and #357; the invalid-byte test joined the list after its stall was localized to the input path and
  failed 3 of 9 GitHub runs of this stack in one day). The suite still runs in CI's `test` job on every ready PR and
  before a stack lands, so the gap is at tag time only. Reversing both is the definition of done for the deflake work.
- Show the session dot as plainly grey when the agent is idle and its last output has been seen. Today the idle state
  can pass for the dim phase of the pulsing green, so "idle" is not obvious at a glance.
- Add a blue dot state: the agent is idle but has produced output since the session was last looked at. Distinct from
  grey (idle, seen) and from the pulsing green (active).
- Add "mark unread" and "mark read": toggle the dot between blue and grey by clicking the dot itself, and from the
  session row's popup menu, where the item reads "mark read" or "mark unread" depending on the current state.
- Make local versus remote obvious in the sidebar: an icon for local vs remote (which icon is not decided), and a more
  prominent hostname, probably moved up to the session name row with some visual weight (details to be settled later).
- Add "replace" on sessions: like clone, but the new session takes the old one's place instead of duplicating it. A
  fresh session and a fresh agent process, with the same directory, host, and the rest of the settings.
- Add host aliases, so a machine can carry a shorter label. The default label stays the hostname or IP as entered; when
  an alias is set it is shown everywhere instead, except in the host details view, which keeps the real name.

## Systematic deflake

Context for every entry here: cutting 0.3.0 on 2026-09-02/03 produced fourteen distinct test failures that reproduced
only under load (a 4-vCPU sandbox running the browser suite beside a live stack, or the GitHub-hosted 4-vCPU release
runner running the Rust suite beside its own build), never on a developer machine. Three tag builds of rc.1 and two of
rc.2 failed on a different one or two of them each time. They were not evenly spread: almost all of them came from two
seams, the tmux attach boundary in the Rust e2e harness and fixed time budgets tuned for a fast box, with the profiles
popup's focus machinery a third on the browser side. The per-test entries under "near term" hold the fingerprints; the
work below is what retires them as a class. Order is a suggestion, from cheapest and most certain to most speculative.

- One scaled time budget instead of literals. The waits that failed are all fixed numbers chosen on a fast machine: the
  relay peer's 20 s `answer()`, the backpressure wait at terminal_backpressure.rs:289, terminal-flood's 30 s stall poll,
  the profiles popup's 250 + 120 ms settlement, the stub feed's socket wait, and every 5 s Playwright `expect`.
  Introduce one factor the harness reads from the environment (something like `FARHELM_TEST_SLOW=3`, default 1) that
  every wait multiplies by, and set it in CI's `test` job and the release gate; for Playwright, the same via
  `expect.configure`/`test.setTimeout` from an env variable in `playwright.config.ts`. A measured latency probe at
  harness start (time one trivial tmux control exchange) could set the factor automatically, but the env knob is the
  first step because it separates "the machine is slow" from "the code is wrong" without touching a single test.
- Retries scoped to named tests, reported as flaky, not hidden. `cargo nextest` runs each test in its own process,
  supports `retries` per test through filtersets, and reports "flaky" as its own outcome in JUnit output; Playwright has
  per-project `retries`. A single load hiccup then stops being fatal to a gate while the ledger of what flaked stays
  visible. Nextest also removes the "one hung test wedges the binary" failure shape. Adopt it for the e2e suite first;
  keep plain `cargo test` working for the developer loop.
- Tier by sensitivity, not by suite. The 0.3.0 emergency cut removed the whole tmux e2e suite from the release gate (see
  the "put the tmux e2e suite back" entry above); the durable form is a "load-sensitive" tier — a nextest test group or
  a Playwright tag — that the gate runs with retries or on a quieter runner, while everything else gates as before.
  Membership starts as the per-test entries under "near term" and shrinks as they are fixed.
- Run the gate's suite where it is not competing with the release build. The tag build runs the suite inside the build
  job on a 4-vCPU runner while the release build itself is warm; `--test-threads=2` there, or a separate job on a larger
  runner, is a cheap experiment that may remove most of the load by itself. Measure before and after.
- Find harness bugs under load early, not at the end. Every deterministic harness mistake in the 0.3.0 stack (a
  fabricated reply without the build stamp latching skew, a click target the popup could cover, a test hook consumed by
  the wrong mount) was found by the sandbox run hours after it was written. Give ready PRs a loaded run (the browser
  suite on a 4-vCPU runner, drafts excluded as today) and add a nightly `--repeat-each` hunt over the load-sensitive
  tier so new members are caught before they block a tag.
- Product-side policies that only show under load, worth their own decisions rather than test tweaks: the profiles
  popup's "an unknown focus classification never dismisses" rule leaves the popup open on a slow renderer (retries were
  bolted on; the real fix is retrying on the next focus event instead of dropping), and the launch shim consuming a
  planted spec before a delete runs (pin the spec or make delete's fail-closed check independent of shim timing).

## Maybe later

- Take a pre-upgrade backup of the on-disk state so a release can be rolled back. Rerunning the installer pinned to an
  older `FARHELM_VERSION` swaps the binaries back cleanly, but it does not make a downgrade work: both stores refuse to
  open a database whose `user_version` is above what the binary understands (deliberately — misreading is worse than
  refusing), so any release that bumps a schema (0.3.0 takes both the helm and the supervisor from 14 to 15) leaves the
  older binary unable to open the state it finds. Today the only rollback is a state-directory backup taken by hand
  before upgrading (the 2026-08-31 upgrade kept one next to the old binary), and remote hosts updated by the helm have
  the same problem for their own supervisor state with nobody taking a backup at all. Wanted: the upgrade paths — the
  installer for the local machine, and the helm's host update for remote ones — snapshot the state directory (a copy, or
  SQLite's backup API, taken while the old version is stopped) before the new version first opens it, keep a bounded
  number of such snapshots, and document the restore. Noted 2026-09-03 while cutting 0.3.0-rc.1.

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
