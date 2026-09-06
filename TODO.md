# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Six buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove —
the decision is made, only the work remains; "near term" is what should be picked up next; "difficult deflake" holds
unresolved test failures after targeted investigation, with evidence and the next useful measurement; "systematic
deflake" is the plan for making the test suites stop failing under load as a class, distinct from the per-test entries
it is meant to retire; "maybe later" is wanted but not soon, and may never happen; "unbucketized" is everything not yet
sorted, which carries no implication either way. Within a bucket, no order.

## Definite simplification

## Near term

## Difficult deflake

These entries remain unresolved after targeted investigation; clean repetitions are non-reproduction evidence, not
fixes. The 2026-09-05 baseline was `d71a87fb`, on Ubuntu 24.04 workers with four CPUs, 8 GiB RAM, and pinned tmux 3.7c.
Unless stated otherwise, Rust batches ran twenty fresh invocations of the built `farhelm` e2e binary with the exact
named test and `--exact --show-output`, stopping at the first failure. The ignored binary-output case also used
`--include-ignored`. Browser batches used the named project/test with `--workers=1 --repeat-each=20
--max-failures=1`.
`.agents/narrow-tests.md` gives the corresponding Cargo and Playwright commands. Extra load, changed fixtures, and
historical evidence are called out per entry.

The combined native run at `aa333815` used four test threads and the same pinned tmux, with a real systemd user manager
and no extra CPU-load process. The stalled-viewer RSS, degenerate-size READY, replacement-claim, and malformed sentinel
cases all passed in that run. The whole e2e binary was 336 passed, four failed in the shared forced-pause helper
described below, and five ignored (four credentialed real-agent cases plus binary output). That wider non-reproduction
does not resolve the four historical cases.

An earlier corrected combined browser run passed 467 Chromium tests with two credential skips. WebKit passed 457,
skipped eleven (two credential cases and nine unsupported clipboard-permission cases), and failed the large-message case
below. That failure also reproduced on the frozen baseline; the WebKit command remains a failed command, not a clean
gate.

The final browser runs at `6903cf90` selected all 469 tests per engine with one worker. Chromium finished with 465
passed, two profiles failures, and two credential skips in 24.4 minutes. WebKit finished with 456 passed, two failures
(menu focus and stalled-client detachment), and eleven expected skips in 29.4 minutes. The profiles failures reproduced
on the frozen baseline; the menu failure appears pre-existing but did not reproduce in twenty baseline attempts; the
stall failure recurred from the existing entry. Their evidence and remaining uncertainty are below. Neither full command
is a clean gate.

- Investigate the remaining profiles startup/bridge symptoms in `e2e/tests/profiles.spec.ts` from the loaded 0.3.0-rc.1
  runs on 2026-09-03. `stale focus-out classifiers cannot clear newer obligations` failed inside `stubFeed`
  (`e2e/tests/helpers/fleet.ts`) with "the page never opened feed socket #1 (saw 0)", before exercising classifiers. The
  focus-and-Escape case was also diagnosed then as an exhausted Unknown classification leaving the popup mounted; that
  product diagnosis remains unproven. Against the baseline above, exact runs reproduced different harness focus races,
  corrected in #385: a reopened popup was visible before focus entered it, and the Unknown fixture produced known
  Missing instead. Those corrections passed six initial cases and 120 repetitions (20 per case per engine) with two
  CPU-load children; the later explicit Unknown oracle passed another twenty per engine without extra load. Neither
  older fingerprint recurred. The previous event-driven retry/classification-ordinal attempt made pending focus failures
  more frequent and did not settle Escape dismissal; do not revive it as a proven solution. On recurrence, retain the
  full browser/bridge trace and feed open/close timestamps, separating no socket request from a late request and a
  classifier exhausting observations. A new retry policy needs that evidence first.
- Fix the distinct focus failures in `an inert sidebar click dismisses the profiles popup` and
  `a popup-created profile is offered on every host`, in `e2e/tests/profiles.spec.ts`. Both failed in the final Chromium
  run at `6903cf90` on the worker shape and pin above, without extra load. The first observed body focus after its
  outside click, then found the popup still mounted with its new-profile button focused: the delayed opening handoff
  returned focus inside and canceled dismissal. The second filled invocation while an editor handoff was still pending;
  the trace shows that text appended to the name, an empty required invocation field, and no POST to the profile route.
  Its catalog wait therefore timed out without a save ever being sent. A pinned candidate batch passed the first case
  once, then failed the second on its first attempt. Separate exact baseline batches on untouched `d71a87fb`, with
  `--repeat-each=20 --max-failures=1 --workers=1`, failed the inert case immediately and the profile case after one
  pass. Both failures therefore predate this work; passing retries would not settle them. The relevant popup production
  code is unchanged. The inert-click failure is a product focus-obligation defect: a trusted outside click must override
  or invalidate the stale opening-focus request, as `SPEC_impl.md` requires. Preserve that immediate click in the test;
  waiting for opening focus first would hide the defective ordering. The profile-creation failure is a separate fixture
  handoff race: apply the existing editor-name-focus precondition before filling fields. Repeat both engines and retain
  focus-event traces after each correction. Do not add retries or widen the catalog wait.
- Investigate `opening the actions menu enters it, and Tab leaves it` in `e2e/tests/sidebar.spec.ts`, WebKit. At
  `6903cf90`, the full run failed to open the menu with ArrowDown. Its trace shows the toggle focus assertion passing,
  then terminal focus in the keyboard-action snapshot about 23 ms later; the menu handler never received that key.
  Initial terminal reveal was still pending despite `__farhelmTermReady` being true. The test, menu handler, and
  `terminal.js` are unchanged from `d71a87fb`, suggesting a pre-existing fixture race, but twenty exact baseline
  executions passed without extra load on the worker shape and pin above. The new layout may affect its frequency; there
  is no direct baseline reproduction. First check whether awaiting `__farhelmTest.replay.revealed` before focusing the
  toggle settles initial reveal, then retain focus and reveal receipts in repetitions of both engines. Keep this
  initial-attach race distinct from reconnect behavior, and retain the keyboard-entry and Tab-exit assertions.
- Deflake `a client that stops draining is detached with the stall reason after the full stall interval` in
  `e2e/tests/terminal-flood.spec.ts`, WebKit. The loaded 2026-09-03 failure saw zero pauses after thirty seconds, before
  the sixty-second stall interval could start. Thirty prior loaded repetitions passed; ten gate-to-first-pause
  measurements were 1.3–2.7 seconds versus the thirty-second allowance. Source inspection found the fixture still
  patches future writes before mounting, waits for readiness, and releases a gated producer of 800,000 numbered records
  that then idles. The final `6903cf90` WebKit run reproduced zero pauses for thirty seconds, with a stalled-detach
  banner already visible about 404 ms after the gate send. This is much earlier than the supervisor's sixty-second
  interval. The unchanged helm outgoing-channel backstop may have detached first, before the browser paused; the trace
  does not prove that cause. Retain detach-reason and queue receipts alongside gate send, received bytes, pending
  writes, pauses, replay state, and FLOOD-DONE to distinguish helm backpressure from supervisor stall, producer
  completion, and replay cutover. Do not widen the budget before locating why HIGH_WATER was never reached.
- Deflake `an over-one-megabyte message does not drop the terminal socket` in `e2e/tests/terminal-flood.spec.ts`,
  WebKit. The combined run failed its fifteen-second `echo:after-big-message` wait after the socket-open/drained
  assertion passed. The trace shows steadily growing echoed input and the exact reply arriving at the deadline; the
  final inner assertion succeeded just after the outer poll timed out. Twenty fresh-stack candidate executions passed,
  but the same exact test failed on execution thirteen of untouched `d71a87fb`, after twelve passes, on the same
  four-CPU worker without extra load. This establishes a pre-existing flake. The 1-MiB-plus-one-byte paste becomes 4,097
  tmux send-key commands; draining the browser socket does not mean the pane has processed them. The poll also returns
  the whole megabyte-scale terminal buffer. Measure pane processing and marker delivery separately from buffer
  serialization before choosing a scoped assertion or budget correction. Retain the open-socket, drain, and exact-reply
  assertions; no timeout increase or product change was made in this pass.
- Deflake `session_lifecycle::non_utf8_terminal_output_survives_live_stream` in
  `crates/farhelm/tests/e2e/session_lifecycle.rs`. The baseline failed on the fifth exact execution (four passed): READY
  arrived but BINARY-MARKER did not arrive within forty seconds. Earlier command-acknowledgement diagnostics localized
  this as missing input, but new fixture receipts disprove that diagnosis for a reproduced occurrence: the fixture
  consumed its input and flushed the binary reply, yet the client still saw no marker. Twenty quiet diagnostic runs
  passed; with two CPU-load children, sixteen passed before the seventeenth failed with both receipts present. Keeping
  the fixture alive after flushing passed twenty loaded runs. Restoring immediate exit with failure-only pane capture
  and dead-state diagnostics also passed twenty loaded runs, so no failing capture was obtained. This points toward an
  output/exit handoff without proving where bytes were lost; the timeout without a detach also weakens a simple early
  terminal-end explanation. Keep `#[ignore]`. Next record raw tmux control markers, decoded payload counts, forwarder
  enqueue, writer completion, and terminal-end handoff for this pane. Input replay, sleeps, or a final capture protocol
  would add delivery/duplication semantics without a demonstrated cause and exceed this pass's scoped-fix boundary.
- Deflake `terminal_backpressure::memory_stays_flat_while_a_viewer_is_stalled` in
  `crates/farhelm/tests/e2e/terminal_backpressure.rs`. Twenty exact baseline runs passed. The historical loaded
  four-thread failure exceeded the 64-MiB supervisor RSS allowance; twenty-one earlier loaded runs also passed. That
  supervisor lives inside the e2e process, so the sample includes libtest, harness, and sibling allocations; the
  separate tmux RSS sample belongs to this test's private server. The producer-progress assertion excludes a stopped
  producer as the explanation for a pass. On recurrence, retain every RSS/progress sample, active sibling identities,
  and a bounded allocator breakdown to attribute growth before changing a queue or bound. The four-thread full binary
  supplies the co-resident allocations an isolated loop omits.
- Deflake `session_lifecycle::attach_with_degenerate_size_still_works` in
  `crates/farhelm/tests/e2e/session_lifecycle.rs`. Twenty exact baseline runs passed. Historical failures occurred only
  in loaded four-thread full binaries, waiting for READY before asserting the clamped 1x1 geometry. `basic_session`
  returns after pane creation, while `basic_session_ready` waits for agent execution; substituting the latter would
  change the launch/attach boundary under test. On a failing run retain attach/replay markers and bounded pane capture,
  dead state, current command, and dimensions to distinguish launch, tmux grid, and live delivery. A missing READY is
  not evidence that the clamp failed, and no budget correction is established.
- Deflake `session_rename::a_renamed_title_survives_a_supervisor_restart` in
  `crates/farhelm/tests/e2e/session_rename.rs`. Twenty exact baseline runs passed. The historical loaded four-thread
  failure was the replacement supervisor ownership assertion in the shared `create_idempotency.rs` handoff helper,
  before the rename reload assertion. Its successful temporary probe takes a flock and closes the file before creating
  the replacement. A concurrent fork can retain that open file description until exec, the mechanism demonstrated for
  the separate sweep fixture fixed in #384. This is a concrete hypothesis here, not a reproduced cause. Trace probe
  acquisition/release and the replacement claim result during concurrent process creation. If inherited probe ownership
  is confirmed, explicitly unlocking that probe is a scoped fixture correction. Retain the ownership assertion: a
  read-only reload could otherwise make the rename test pass without exercising a real successor.
- Deflake `only layout changes after a profiles opening invalidate its geometry` in `e2e/tests/profiles.spec.ts`. Twenty
  isolated Chromium baseline repetitions passed. The historical sighting was a full-suite Chromium failure on a 4-vCPU
  worker on 2026-09-03, with no extra load. The saved-profile case formerly grouped here was a separate editor focus
  race, fixed in #385 and validated twenty times per engine. For this remaining geometry case, retain the pre-open
  scroll epoch, opening epoch, measured rectangle epoch, focus settlement, and post-open scroll event on recurrence. The
  test already waits for popup focus before the second scroll. No failing trace yet establishes that its timing or
  geometry contract should change.
- Deflake `launch_sentinel_error_status::a_planted_malformed_spec_sentinel_classifies_error_with_its_detail` in
  `crates/farhelm/tests/e2e/launch_sentinel_error_status.rs`. Twenty exact baseline runs passed. The historical loaded
  four-thread assertion found the expected durable Error state but a surviving sentinel. Source awaits cleanup after
  `transition_many` commits; removal is best-effort and logs non-NotFound errors. It is not an unawaited deletion race.
  On recurrence capture unlink path/errno, planted versus derived generation paths, and the committed session ID. If the
  paths match and no removal warning exists, inspect the actual directory entry before changing cleanup semantics.
- Fix the forced-pause helper's client-list parsing in `crates/farhelm/tests/e2e/terminal_backpressure.rs`. The combined
  `aa333815` run failed four cases with "no output control client found among tmux clients":
  `replay_marker::a_tmux_pause_catch_up_replays_without_a_marker`,
  `terminal_backpressure::a_forced_tmux_pause_is_recovered_through_the_real_attachment`,
  `terminal_backpressure::a_forced_tmux_pause_recovers_an_alternate_screen_pane`, and
  `terminal_backpressure::a_forced_tmux_pause_restores_modes_and_cursor_state`. The listing visibly contained the output
  client's `pause-after=5` flag, but an underscore separated its name from the flags where the helper expects a tab. The
  exact replay-marker case also failed on untouched `d71a87fb` with one test thread in 0.47 seconds, on a second worker
  with the same pinned 3.7c and no extra load. Both the helper and these test bodies are unchanged across the
  comparison; this is a newly observed pre-existing test/substrate compatibility failure, not evidence that catch-up
  itself broke. Retain the listing, check the formatter's delimiter bytes, and use an unambiguous supported separator
  while keeping the positive `pause-after` discriminator. Then validate all four callers against the pinned substrate.
- Restore the release integration gate and remove the remaining ignored binary-output test when the named Rust flakes
  above are fixed. #382 restored the helm-death test. Binary output still blocks its own un-ignore; it and the stalled
  viewer RSS, degenerate-size READY, replacement claim, malformed-sentinel, and forced-pause helper cases still block
  restoring the entire `farhelm` integration target in `.github/dist-build-setup.yml`. Browser flakes are separate
  coverage and do not themselves gate that Rust target. The integration suite still runs in CI's test job for ready PRs.
  A single clean combined run cannot establish that these latent failures are fixed; retain the release exclusion until
  the evidence supports reversing it.

## Systematic deflake

Context for every entry here: cutting 0.3.0 on 2026-09-02/03 produced fourteen distinct test failures that reproduced
only under load (a 4-vCPU sandbox running the browser suite beside a live stack, or the GitHub-hosted 4-vCPU release
runner running the Rust suite beside its own build), never on a developer machine. Three tag builds of rc.1 and two of
rc.2 failed on a different one or two of them each time. They were not evenly spread: almost all of them came from two
seams, the tmux attach boundary in the Rust e2e harness and fixed time budgets tuned for a fast box, with the profiles
popup's focus machinery a third on the browser side. The historical cases from those runs are now retained under
"difficult deflake"; the work below is what retires them as a class. Order is a suggestion, from cheapest and most
certain to most speculative.

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
  Membership starts as the historical cases from those 0.3.0 runs now under "difficult deflake" and shrinks as they are
  fixed. Newly encountered failures share that bucket for tracking; adding one does not automatically enroll it in this
  initial tier.
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

- Support agents' native voice modes by piping audio through the helm to the supervisor and onward to the agent. Assess
  feasibility later, including how agents accept audio and what transport or audio-device integration would be needed;
  this entry does not commit to a design.

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
