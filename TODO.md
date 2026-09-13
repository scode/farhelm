# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Seven buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove
— the decision is made, only the work remains; "near term" is what should be picked up next; "tricky bugs" retains
unresolved bug reports and their investigation findings; "deflake" gathers test and harness reliability work, including
CI execution and restoring gates; "code review" is the residue of the September 2026 review swarms after the policy
pass, ordered by confidence and risk; "maybe later" is wanted but not soon, and may never happen; "unbucketized" is
everything not yet sorted, which carries no implication either way. Within a bucket, no order unless the bucket
explicitly says so.

Known product fixes stay in their product bucket. "Difficult deflake" retains unresolved failures and their
investigation evidence within "Deflake"; that placement does not establish that the cause is test-only. Move a diagnosed
product fix out of "Deflake" rather than changing user-visible behavior as a test correction.

## Definite simplification

## Near term

- Stop the whole app scrolling. After the recent UI changes, a wheel or trackpad scroll that lands over the top of the
  window, or over the thin bar between the terminal and the sidebar, scrolls the entire app contents and exposes the
  black background underneath. Only the terminal viewport and the sidebar list should ever scroll; the app shell itself
  must not.

## Tricky bugs

- Investigate corruption in the Codex input area when typing quickly. In ordinary use, appending exactly
  `include a SPEC.md` to a prompt quickly made the display show `include a SPE` followed by another line containing
  scattered fragments such as `COMMI`, `PR`, and repeated `SPEC` text, with large gaps between them, before submission.
  No bug screenshot or logs were supplied. Whether the underlying input was corrupted or only its rendering is unknown.
  [Investigation findings](docs/codex-input-investigation.md): direct tmux and Linux Chromium/WebKit probes did not
  reproduce the scattered current input; the report remains unresolved, including native macOS coverage.

- Investigate Codex resuming the existing conversation after using "Replace" on a session. Reported in ordinary use: the
  replacement retained the previous conversation and could summarize the earlier work, instead of starting a fresh
  conversation. Replace should preserve the session's directory, title, and agent choice while starting fresh. Cause and
  reproducibility are not established. [Investigation findings](docs/codex-replace-investigation.md): controlled and
  real Codex comparisons started distinct replacement conversations, including after Resume; the incident remains
  unresolved without its launch configuration and vendor conversation identities.

## Deflake

- Investigate intermittent recovery assertions in
  `rotation logs out an open client and drops its feed and terminal
  sockets`, in `e2e/tests/auth.spec.ts`. Chromium
  observed an aborted recovery detail read in a broad run and a missing sidebar row after a successful detail read in an
  exact run. A failure past the exchange no longer cascades (the suite refresh moved to right after it); what remains is
  the recovery provenance itself. Twenty local repetitions passed without reproducing either shape; the next failure's
  retained trace carries network, DOM, and console. Establish provenance before changing authentication behavior or
  recovery assertions.

  Provenance established 2026-09-12, two new reproductions of the missing-sidebar-row shape via recorded hunts (batch
  `d1859bb8-dc57-4c74-bc41-1c9ca25036cd`): the row is missing because the recovery batch's `/api/sessions?sort=activity`
  and `/api/hosts` fetches were NEVER SENT — trace network snapshots show `send: -1` from creation until teardown, twice
  in a row (the feed-handshake re-read batch too), while the same frame's profiles/detail/preferences reads were served
  in tens of milliseconds throughout, and the reconnected feed and terminal sockets' upgrades themselves queued 4.3s and
  45s. The helm and supervisor are innocent by logs (both silent through the whole window; supervisor logs preserved
  from the second reproduction) and every request the helm received was answered. So the stall is in the BROWSER's
  dispatch of exactly those two fetches, cause not established — what remains unknown is renderer-side network state a
  Playwright trace does not carry. Note the designed recovery cannot show within the test's own 60s budget: the UI's
  request timeout is 60s, and only a failed read hands the surface to its retry ladder, so any hung read outlives the
  test. Next rung: renderer-level receipts in the UI's fetch wrapper (dispatch and completion per request,
  console-carried) to catch a never-dispatched fetch in the act.

- Investigate the remaining initial profile focus failures in `e2e/tests/profiles.spec.ts`. WebKit failed the editor
  focus premise in `Tab leaving the document preserves busy dismissal intent` and the first client's popup focus in
  `a profile edited in another browser reaches this one over the real feed`. The latter occurs before the separately
  corrected second-client terminal-readiness boundary. Preserve both tests' focus and feed assertions; the new
  observation does not establish a regression in saving profiles or delivering their updates.

  Recorded hunt 2026-09-12 (batch `daf6d0e1-8081-4caa-85f1-c39624a75d8e`): the two-client feed case reproduced its
  focus-premise failure 1/20 on WebKit (`toBeFocused` received inactive for 5s while the input sat rendered), trace and
  screenshots retained in the run; `Tab leaving the document` passed all 40 of its attempts across that hunt. A read of
  the coordinator's receipts (not a reproduced cause): opening a form's focus request can be consumed as `Unknown` when
  the 250ms `FOCUS_SETTLE_MS` budget cannot cover the eval round trips on loaded WebKit, which settles into exactly this
  never-focused state — but no instrumented rerun caught receipts in the act in 25 further attempts, and the
  fixture-diagnosis history warns against treating that as proven. The test is also named under "Difficult deflake"
  below.

- Investigate `the profiles popup border box stays inside a constrained viewport`, in `e2e/tests/profiles.spec.ts`.
  WebKit failed its focus premise (`toBeFocused` received inactive) once in browser run
  `ed375214-fa16-4b8f-bcad-00117ff59e97`, a landing-time run of the launcher composer stack rebased onto the OpenCode
  and macOS-header changes; the same run passed the test on Chromium. The launcher change touches only the profiles
  spec's name-field label, so this reads as the same WebKit initial-focus family recorded above rather than a composer
  regression, but that attribution is not established. Retain focus-event traces before changing the test. A recorded
  hunt on 2026-09-12 (batch `addd38f4-a637-49b0-86f6-b1bd947876de`) passed all twenty repetitions on both engines —
  non-reproduction evidence, consistent with the earlier twenty isolated repetitions.

- Investigate the retained host-action fixture failure from browser run `7fd44a19-ce3f-42fb-a3df-410da327634a`:
  `a failed removal stays visible with details collapsed`, in `e2e/tests/terminal-multihost.spec.ts`, could not find
  `.host-details-toggle`. Twenty repetitions did not reproduce it. The run's other failure, the sidebar aliasing test's
  disposed `route.fetch` response, was the WebKit navigation-disposal mechanism and is fixed; this one has no
  reproduction or established cause. Preserve host-row state evidence before changing product behavior; the failure
  alone does not establish a composer regression or a confirmed pre-composer cause.

- Investigate two retained WebKit attachment-fixture failures in `e2e/tests/terminal-tabs.spec.ts`, from browser run
  `7fd44a19-ce3f-42fb-a3df-410da327634a`.
  `stalling one tab's writes pauses only that tab; the agent and a sibling stay
  live` never established a HIGH_WATER
  pause within its observation window. `a tab list past the island cap is listed
  in full but only partly attached` had
  the expected path and mounted/revealed state but a closed socket at readiness. Retain gate, attachment and
  close-reason receipts; keep these failures distinct from the existing single-client stall entry, and do not weaken
  liveness assertions based on a later passing run.

  The island-cap shape reproduced 3/20 recorded WebKit repetitions on 2026-09-12 (batch
  `396d6272-a07d-4bd4-b9f1-6409bc916cc9`, timelines attached in the run): the agent terminal's established socket
  errored and closed about two seconds into the 32-phantom attach-refusal storm, with NO helm log line for it; every
  reconnect-ladder attempt after that was refused within milliseconds for the rest of the test, and one past-cap
  phantom's attach retried on a ladder of its own for 20s. The helm logged only the phantom refusals ("has no terminal
  tab"); the supervisor's log was silent; what killed the agent attachment is not established.

  The stall test's zero-pause shape also reproduced once in ten recorded repetitions (batch
  `e5f7f17f-7f24-4118-8559-62a1ea5df624`, then 24 consecutive passes): one tab's socket closed about 1.2 seconds after
  the flood started — before any HIGH_WATER crossing, with no helm log line — the pause poll then waited its full sixty
  seconds over a dead socket, and at the supervisor's stall interval the session's other two sockets closed before
  reconnect ladders that were refused instantly. That is the same early-silent-close signature as the island-cap
  reproduction, and it is consistent with the helm's outbound side winning the race before the browser could pause, but
  the closing half's own receipt (detach reason, queue depth) still needs a supervisor-side log from a reproduction —
  preserve the stack's supervisor logs past teardown when hunting again.

### Difficult deflake

The 2026-09-08 browser gate added these follow-ups, with retained evidence in FLAKES.md:

- Make the raw-byte fixture in `e2e/tests/terminal-keys.spec.ts` use a dumper that emits live bytes on supported test
  substrates. Ubuntu 26.04's uutils od withheld the sentinel; GNU od passed all ten cases without source changes.
  Preserve the complete byte sequence and single-write assertions rather than ending the stream early to flush output.
- Investigate the full-run backspace/Ctrl+C failures in `e2e/tests/terminal-flood.spec.ts`. Both engines failed during
  session-deletion setup after the large-paste case, before the input assertions: `deleted.ok()` was false. Both passed
  in narrow candidate and baseline sequences. Inspect the deletion response and session lifecycle evidence to
  distinguish paste contamination from an independent failure; do not infer a cause from a retry.
- Stabilize the intended boundaries of `an outside click overrides a delayed opening focus commit` and
  `a profile edited in another browser reaches this one over the real feed` in `e2e/tests/profiles.spec.ts`. WebKit
  missed the held commit's deadline or popup focus readiness before the behavior under test. A baseline pass does not
  establish that the new layout is uninvolved. Preserve trusted-pointer, unexpired-release, and focus assertions.

The earlier entries below remain unresolved after targeted investigation; clean repetitions are non-reproduction
evidence, not fixes. Their 2026-09-05 baseline was `d71a87fb`, on Ubuntu 24.04 workers with four CPUs and 8 GiB RAM.
Those workers reported pinned tmux 3.7c, but no resolved executable hash was retained; exact substrate identity remains
unverified. Unless stated otherwise, Rust batches ran twenty fresh invocations of the built `farhelm` e2e binary with
the exact named test and `--exact --show-output`, stopping at the first failure. The ignored binary-output case also
used `--include-ignored`. Browser batches used the named project/test with
`--workers=1 --repeat-each=20
--max-failures=1`. `.agents/narrow-tests.md` gives the corresponding Cargo and Playwright
commands. Extra load, changed fixtures, and historical evidence are called out per entry.

The combined native run at `aa333815` used four test threads and the same reported tmux pin, with a real systemd user
manager and no extra CPU-load process. The stalled-viewer RSS, degenerate-size READY, replacement-claim, and malformed
sentinel cases all passed in that run. The whole e2e binary was 336 passed, four failed in the shared forced-pause
helper described below, and five ignored (four credentialed real-agent cases plus binary output). That wider
non-reproduction does not resolve the four historical cases.

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
  classifier exhausting observations. The controlled regressions for retaining unresolved obligations and honoring later
  focus events do not establish the cause of these older startup/bridge failures.
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
  with the same reported 3.7c pin and no extra load. Both the helper and these test bodies are unchanged across the
  comparison, but the worker's executable identity is unverified. CI run 34006471792 at `2069e0c8` passed all four cases
  on its built pin with four threads. The 2026-09-06 FLAKES.md caveat records that counterevidence; the observed
  delimiter failure remains open, without a claim of deterministic failure on the exact pin. Retain a recorder run,
  check the formatter's delimiter bytes, and use an unambiguous supported separator if the mismatch is reproduced while
  keeping the positive `pause-after` discriminator. Then validate all four callers against the pinned substrate.
- Restore the release integration gate and remove the remaining ignored binary-output test when the named Rust flakes
  above are fixed. #382 restored the helm-death test. Binary output still blocks its own un-ignore; it and the stalled
  viewer RSS, degenerate-size READY, replacement claim, malformed-sentinel, and forced-pause helper cases still block
  restoring the entire `farhelm` integration target in `.github/dist-build-setup.yml`. Browser flakes are separate
  coverage and do not themselves gate that Rust target. The integration suite remains available for explicit local or
  worker validation; ordinary CI and the release gate do not run it while this exclusion stands. A single clean combined
  run cannot establish that these latent failures are fixed; retain the release exclusion until the evidence supports
  reversing it.

### Systematic deflake

Deferred work, with its original triggers:

- A typed scale factor on harness budgets, triggered by a budget-class recurrence after the readiness-oracle changes
  land. Keep harness budgets distinct from product deadlines (`Budget::harness` versus `Budget::product`, and
  `expect.configure` plus a browser helper); `terminal-flood.spec.ts` has both kinds of deadline. A slow wait caused by
  an invalid fixture premise is not evidence for scaling its budget.
- Product observables that attribute their cause without changing the deliberately shared stall-detach reason string:
  supervisor stall versus helm backstop, sentinel unlink path and errno, and bounded helm-death detection latency.
  Attribution belongs in a secondary field or retained log line. This remains product work for "Near term" when the
  maintainer chooses to schedule it.
- Running the tag gate's suite away from the release build, after "Restore the release integration gate" lands. The
  release gate still excludes the e2e target; evaluate any concurrency experiment using the then-current runner budget
  rather than reviving the old libtest thread setting.

## Code review

The residue of the September 2026 review swarms (700 findings over seven areas, reviewed at `db76f00b`) after the policy
pass applied the maintainer-confirmed decisions in SPEC.md, and after a per-finding check against main at `7c2cdd83` on
2026-09-13 established which mechanisms still exist. Finding IDs such as `A5-C4` are area-N plus the finding's tag in
that area's review; each resolves to a line in the brain's verbatim mirror, linked from
[this page](https://claude.ai/code/artifact/5a79de5b-90d5-4eee-8aa1-ea3c803ef50c). The full working archive, including
the fourteen group notes whose fences the entries below quote, is in the private brain under
[farhelm-review-postprocessing](https://github.com/scode/brain/blob/main/personal/farhelm-review-postprocessing.md); the
raw swarm reports are the seven mirrors beside it. The final synthesis is also on
[HackMD](https://hackmd.io/a2F0fZKqQsSh4anyxFrjlA?type=view).

Every entry names the code it rests on as of the check; a later reader must confirm the mechanism is still present
before building. "Fence" is what the group notes say the fix must preserve or must not become. Severity and effort are
agent judgments, not measurements. Nothing here authorizes a broader rewrite than the entry names: the notes repeatedly
warn against folding neighbours into one lifecycle or locking overhaul.

Closed by the check, so nobody re-raises them: `A4-T8` was fixed on main by the per-test capture buffers (#419, #423);
`A3-C8` (delete cancels uploads before its preflight) is accepted by SPEC's partial-deletion decision; `A6-C6` (the
14→15 profile-table drop) and `A6-C7` (per-session SQL parameters at fleet scale) are excluded by the compatibility and
client-scale decisions.

### Do first: high confidence, low risk

Mechanism verified on main, fix is trivial or small, and the notes attach no design question. Ordered roughly by
severity.

- **Snapshot self-witness.** `A3-C3`. High, trivial. `procs::snapshot` (procs.rs:372-402) returns `Ok` with an empty map
  when `/proc` is present but unmounted, and the macOS path (procs.rs:765) returns `Ok(Vec::new())` for a zero-sized
  `KERN_PROC_ALL`, so every stop and delete reports success having examined nothing, against the module's own
  fail-closed contract at procs.rs:91-99. Fix: after the walk, return `Err` unless the map contains
  `std::process::id()`. Also backstops the real-uid changes below.
- **`remote_farhelm` panics and empty install directories.** `A2-C4`, `A2-C21`. High and medium, small.
  `PlanLayout::plan` (provisioning/plan.rs:283-289) does `file_name().expect(...)`, and `plan_for_row` feeds it the
  stored `remote_farhelm` verbatim; `add_ssh_host` (helm store.rs:3208-3239) validates only the ssh destination, so
  `POST /api/hosts` with `"remote_farhelm": "."` followed by an update panics the request handler. A bare relative name
  instead yields `Some("")` as `override_lib_dir` and a plan whose first step creates an empty path. Fix: return a
  `BackendFailure` when `file_name()` is `None`, validate the field at the store or API boundary with a 400, and treat
  an empty or relative parent as "no override". Fence: keep valid relative and PATH registrations working; check the
  bare-name probe path before choosing a blanket absolute-only rule.
- **Provisioning sha256sum with backslash paths.** `A2-C1`, `A2-C2`. Medium, trivial. Both the post-upload digest check
  (provisioning/backend.rs:661-670) and the metadata probe (:465-490) pass the path as an argument, so GNU coreutils
  escapes the line and prefixes `\`, and the parse fails as a bogus "digest mismatch" or "malformed output" on any home
  directory containing a backslash. `install.sh`'s `sha256_of` documents this quirk and feeds stdin. Fix:
  `sha256sum <
  path` in both; the existing whitespace parser keeps working with the literal `-`. Fence: one
  verification for both; a refusal is not acceptance of bad bytes; no general path restriction.
- **Installer umask and lock hygiene.** `A2-C7`, `A2-S5`, `A2-S2`. Medium, trivial. `mkdir -p "$INSTALL_DIR"`
  (install.sh:806-819) chmods only the leaf, so under `umask 000` a freshly created `$HOME/.local` is 0777 and another
  account can replace the `bin` entry the leaf chmod was meant to protect; the macOS bundle tree (:1099, :1139) and the
  lock directory (:588, :628) are created the same way. Fix: `(umask 022; mkdir -p ...)` for the install chain and
  bundle assembly, `umask 077` or an immediate `chmod 0700` for the lock, and have `is_our_lock` refuse a group- or
  world-writable lock directory. Fence: only paths this run creates; preserve pre-existing shared directories; no
  recursive chmod.
- **Terminal WebSocket bounds and exits.** `A4-C2`, `A4-C3`, `A4-C4`, `A4-C5`, `A4-C6`, `A4-C1`. Medium, trivial each.
  terminal.rs:346 sets `max_message_size` but not `max_frame_size`, leaving tungstenite 0.29's 16 MiB default, whose
  reader reserves the declared header length before any payload arrives (verified in the pinned crate source); the
  events socket sets both. Three detach-notice sends (terminal.rs:589, :572, :490) carry no timeout where every events
  write is bounded by `WRITE_DEADLINE`; the stall path at :589 is the one whose peer is proven not to read. The inbound
  task's `JoinError` is handled with `?` at :677 before `client.detach` at :693, breaking the module's "detach runs on
  every exit path" invariant on the panic path. And an authenticated non-upgrade GET on a WebSocket route gets axum's
  500 naming `farhelm_helm::auth::AuthenticatedSocket`, because the handlers extract the extension before
  `WebSocketUpgrade` (auth.rs:259-269, terminal.rs:300-325, events.rs:126-130). Fix: add the frame bound plus the
  terminal counterpart of the events oversized-header test; wrap all three notice sends in
  `timeout(WS_TEARDOWN_GRACE, ...)`; fold the `JoinError` into the result so the detach tail runs; extract
  `WebSocketUpgrade` first. Fence: count only the stall path as a proven hang; the other two sends are consistency.
- **Store one-liners.** `A6-C3`, `A6-C14`, `A6-C2`. Medium, trivial to small. `register_probed_ssh_host` (helm
  store.rs:3307-3311) builds `IdentityMismatch` with `expected` and `actual` reversed relative to the variant's doc and
  every other site, so the operator reads the opposite of reality when re-provisioning a reinstalled machine.
  `mark_seen` (store.rs:3035-3038) guards its shared row with `!=` rather than `<`, so a stale client un-sees a session
  for everyone; the supervisor's `record_activity` already uses `<`. The insert branch of `register_probed_ssh_host`
  (:3341) bails with a string for an already-claimed identity while the converge branch returns the typed
  `IdentityClaimed`, so the same situation is a 409 on one path and a 500 on the other. Fix: swap the two fields with a
  test on the probe path; change the `DO UPDATE` predicate to `<`; add a typed variant carrying identity and owner only
  and map it to Conflict in `error_kind`. Fence: do not invent a host id where registration failed before creating one.
- **Relay diagnostics and redaction.** `A1-C11`, `A1-C8`, `A1-C19`, `A1-C6`. Medium to low, trivial. The relay's only
  warn line for a failed upcall (agent_relay.rs:637-647) asserts the helm "did not answer in time" with a budget field
  for three endings where no budget elapsed. `list_sessions` (helm client.rs:2547) is the one wrong-reply site still
  using `{other:?}`, which restores raw invocation argv into an agent-visible message; its five siblings use
  `wrong_reply()`. The delete-fence docs (agent_relay.rs:222-225, core.rs:3566) claim every non-retained ending means
  the mutation cannot still be running, omitting the two post-queue connection-loss exits. `ResolveProfile` shares
  `ReplyKind::Created` with the creating verbs (farhelm main.rs:1387, :1398), so a wrong reply to create or clone
  bypasses the outcome-unknown remedy and hits a bare bail. Fix: log the outcome's own message and drop the budget field
  where none expired; `Err(wrong_reply("ListSessions", &other))`; add the fourth category to the docs; give
  ResolveProfile its own variant. Fence: keep the Timeout-means-outcome-unknown vocabulary; do not extend fences to make
  the old claim true.
- **Provisioning download sanity limit.** `A2-C8`. Medium, small; a SPEC requirement with no implementation.
  `download_verified` (release_payloads.rs:689-742) streams every chunk to `<asset>.part` with no byte counter; the only
  caps in the file are for the control files. Fix: an `ASSET_MAX_BYTES` constant far above the largest archive, a
  counter in the loop, a refusal naming the asset and limit, removal of the `.part`, and a corrected `SUMS_MAX_BYTES`
  docstring, whose "the expected hash is already known" rationale is unsound. Fence: not a quota system; not
  `install.sh`; not attachment uploads.
- **Installer terminal states.** `A2-C9`, `A2-C26`, `A2-C6`. High and medium, small. A crash between journal removal and
  backup cleanup (install.sh:1038-1041) strands `.farhelm.old`, which `refuse_unless_absent` (:983) then rejects on
  every later run while the message advises a re-run that cannot help. `is_our_lock` (:334-342) parses `ls -A` output,
  which an inherited `QUOTING_STYLE` reshapes, so release silently skips and every later run refuses. The download
  channel takes `FARHELM_RELEASE_BASE_URL` unvalidated, passes no `--proto` pins, and never reports a non-default
  source. Fix: sweep reserved `.old` backups right after `acquire_lock` succeeds with no journal (committed debris by
  the file's own invariant); enumerate the lock with a glob or `QUOTING_STYLE=literal`; add
  `--proto
  '=https' --proto-redir '=https'` on the default path, validate a set base URL like
  `parse_release_base_url`, and name a non-default one in the output. Fence: never delete a foreign collision; an
  operator-selected source is not an attacker; no bundled signatures or URL restrictions.
- **Small contained items.** `A2-C3`, `A1-C9`, `A6-D26`. Low, trivial. `is_stale_generation`
  (release_payloads.rs:1020-1024) splits a name at `len - 12` bytes and panics on a non-boundary; the panic is contained
  by `spawn_blocking` but the `OnceCell` stays uninitialised so every download retries and fails while the entry exists.
  `safe_cell` (farhelm main.rs:1713-1729) escapes only Cc, so U+2028/U+2029 and bidi controls reach the agent-facing
  table whose widths are computed after sanitizing. `LastOutcome::Exited`'s doc (supervisor store.rs:222-225) says the
  annotation is set only by a user-initiated stop, but archive writes it too. Fix: a boundary-safe split; widen
  `safe_cell` (ideally one shared predicate with its two siblings) to emit visible `\u{...}` escapes; name both writers
  in the doc without blessing the archive overwrite. Fence: leave the acceptance checks alone, they are specified
  policy; no prompt-injection promise.

### Next: high confidence, needs care

Mechanism verified on main, but the fix touches lifecycle, locking, or the kill set, or needs a reproduction before it
is safe. Each is its own review unit.

- **Kill sweep cgroup verdict.** `A3-C4`, `A3-C5`, `A3-C13`, `A3-C7`. High, small each, medium risk. The systemd
  availability verdict is a per-process `OnceCell` (scope.rs:294, :417) with no invalidation, so one transient probe
  failure at startup permanently disables cgroup teardown for every inherited session; `reap_process_tree`
  (sweep.rs:1072) then discards every recorded unit name with a `debug!`, the opposite of the policy `reap_tab_tree`
  documents and of what durable `entry.scope` means. A failed scope kill is downgraded to a warning even for delete
  (sweep.rs:1097-1108), after which the row is removed and nothing can retry, while merely failing to enumerate tab
  scopes is fatal on the same path. And no teardown path names a previous launch generation's scope (only
  `tab_unit_glob` exists, scope.rs:160), so a generation whose kill failed while the portable sweep said clean is
  orphaned by the next delete. Fix: ask the manager about names backed by durable evidence even when the cached verdict
  is negative, letting `kill_scope`'s existence check settle it, and re-probe once when a teardown holds such a name;
  return `Err` for delete and archive on a scope-kill failure so the row stays retryable; add a session-scoped
  launch-unit glob enumerated with the tab glob's strictness; promote the teardown-side skip to `warn!` when the row
  says `launch_scoped`. Fence: no new manager machinery; speculative names on manager-less hosts must stay skippable; a
  portable sweep is not proof a scope is empty.
- **Real uid in the process walk.** `A3-C2` then `A3-C1`. High, small on macOS and medium on Linux, medium risk because
  widening the table widens the kill set. macOS `snapshot` (procs.rs:826) filters `kinfo_proc` rows by `cr_uid`, the
  effective uid, while `p_ruid` sits transcribed at :599 marked "never read"; Linux (procs.rs:391, :343) uses
  `/proc/<pid>` directory ownership, which also tracks the effective uid. A descendant that execs a setuid binary, and
  everything below it, is silently dropped, and stop reports success; macOS has no cgroup backstop. Fix: on macOS add an
  offset assertion for `kp_eproc.e_pcred.p_ruid` beside the four at :678 and accept a row when real or effective uid
  matches; on Linux select by the real uid from `/proc/<pid>/status` and keep the directory-owner check only where
  `read_process` uses it to classify a failed read; correct the `snapshot` docstring's "not killable" premise. Do macOS
  first, Linux second, each with the self-witness above already landed. Fence: ordinary descendants in scope, deliberate
  same-account escape not; never broaden a kill set on identity that has not been revalidated.
- **Archive preserves a known outcome.** `A3-SP1`, `A6-C23`. High, small, medium risk. `teardown_for_archive`
  (teardown.rs:385-392) builds `Exited{exit_code: None, annotation: STOP_ANNOTATION}` unconditionally and
  `archive_session` (supervisor store.rs:3572-3576) runs
  `UPDATE ... outcome_state = 'exited', exit_code = NULL,
  annotation = ?2, error_detail = NULL` with no condition on
  the prior outcome, so an Error with its detail or an Exited with its code is rewritten as "stopped by user". Fix: read
  the outcome quartet in the same transaction and synthesize the annotated exit only when the archive actually tore down
  a live agent, in both the SQL and the in-memory entry. Fence: archiving a live agent really is a user-initiated stop;
  only the Error-to-Exited conversion is a SPEC divergence, so narrow rather than blanket preservation.
- **Tab close leaves input aimed at the agent pane.** `A5-S4`. High, small, medium risk. `close_tab_window`
  (core.rs:8901-8932) reaps, kills the window, reaps again, and only then calls `detach_closed_tab`; the audited
  `=<session>:.<pane>` target doc (tmux.rs:1552-1559) records that a vanished pane silently degrades to the session's
  active pane, which is the agent window. Keystrokes typed in the seconds between kill and detach can land in the
  agent's pane. Fix: move `detach_closed_tab` ahead of the reap and kill, or invalidate the attachment's `InputClient`
  under the attachments lock immediately before the kill. Fence: the fallback was audited for `display-message`, not
  `send-keys`; verify input delivery separately from output capture.
- **Upload stall attributed to the browser.** `A4-C12`. High, small, medium risk. uploads.rs arms the stall deadline at
  :197 and re-arms only when a non-empty chunk arrives (:289), immediately before the potentially long
  `send_upload_chunk` (:293) that waits on supervisor credit; the biased select at :222-227 then takes the expired
  Stalled arm and aborts with "no body progress from the client". Fix: arm the deadline immediately before the select in
  the body-not-ready arm instead of at chunk receipt, keeping the empty-chunk fast path. Fence: trace
  `wait_for_credit`'s own re-arming in client.rs first; the magnitude depends on credit waits exceeding 60 s.
- **Dispatch under the attachments and lifecycle locks.** `A1-C3`, `A1-C4`, `A5-C1`, `A1-C2`. Medium, small to medium,
  medium risk; one unit per finding. A tab attach (handlers.rs:1440) claims the session's lifecycle lock inline on the
  read loop with no timeout, behind a stop or delete that holds it for the whole sweep. The restricted create arm
  (handlers.rs:2858) claims the parent's lifecycle lock across a `ResolveProfile` round trip to the helm. The writer
  task's `else =>
  break` (connection.rs:286-296) is unreachable at shutdown because `priority_tx` and a full-authority
  link's `tx` clone outlive the drop at :669, so every teardown burns the full drain window and force-aborts with a
  false warning; the addendum rules out sender accounting because upload and detach tasks hold more clones. A delete
  parked on a retained agent fence (handlers.rs:1166, :1187) holds one of eight process-wide admission permits for up to
  the 600 s retention while the reply that would free it is dispatched by the loop those permits park. Fix: a short
  timeout on the tab-attach claim refusing with the Conflict shape `handle_attach` already uses; resolve the profile
  before taking the parent claim, then claim and re-check the credential; `rx.close()` and `priority_rx.close()` in the
  shutdown tail; claim the fence before the permit or bound the wait well under retention with a retry-safe Conflict.
  Fence: takeover ownership across every chunk; the credential re-check stays under the claim; ordinary unrelated
  controls must progress; no fair scheduling for hostile local workloads; the shipped helm chunks input at 32 KiB so the
  8 MiB frame is not an ordinary paste.
- **Helm-owned default profile.** `A6-S1`. Critical, medium, medium risk. `source_is_newer` (helm store.rs:569) falls
  back to raw `candidate.created_at > stored.created_at` for any cross-host pair, and `replace_host_sessions`
  (:4227-4283) feeds it drain-derived timestamps with no sanity check before writing `remembered_profile`; a remote
  session naming a different starter profile with a high timestamp pins the fleet-wide default and the user's later
  direct choice is rejected as older. Reproduced through real store APIs by the investigation probe, which also showed
  recovery after complete source disappearance. Contradicts SPEC's "the helm owns the remembered default". Fix: give
  user-originated creates unconditional authority over the remembered default and stop drain observations from replacing
  it, distinguishing agent create and clone origin in `do_create_session`; reconcile the discovery and default prose and
  tests in the same change. Fence: a timestamp clamp is insufficient by design; keep profile-catalog discovery
  independent of default selection; its own review unit.
- **Provisioning quoting and provenance.** `A2-S3`, `A2-C5`, `A2-C18`. Medium, small, medium risk. `shell_path`
  (backend.rs:1627-1629) uses `shell_words::quote`, whose minimal set excludes braces, so a path can brace-expand into
  several remote words under `sh -c`; the same helper builds the steady-state argv in ssh.rs. `parse_reach_output`
  (backend.rs:1761) stores the host's `os-release` ID unvalidated, `confirmation()` (plan.rs:217-222) splices it into a
  line, and `PeerBlock` splits on `lines()` so a newline in it becomes a plan step the operator approves.
  `linger_was_refused` (backend.rs:1726-1740) substring-matches "permission denied" against whole stderr, so ssh's own
  refusal with exit 255 is reported as a benign degraded linger. Fix: an always-single-quote helper used by both layers;
  reject or sanitize newlines and bound the length in `distro_id` at the boundary; require positive evidence the linger
  command reached the host before accepting degradation. Fence: correctness for the operator's own path, no new
  authority; no double escaping at the GUI; neither an unconditional success claim nor a blanket exit-255 classifier.
- **Credential cap self-eviction.** `A6-C15`. Medium, small, medium risk. `exchange_device_session_inner` (helm
  store.rs:2755-2769) inserts, then deletes rows past `OFFSET 64` ordered by `created_at DESC, cookie_hash DESC`, with
  nothing excluding the new row, and returns `Ok(true)` regardless; a clock rollback or tie evicts the credential just
  issued and hands the browser an unusable secret. Fix: order eviction by `rowid DESC` so insertion order decides,
  keeping the offset. Fence: the cap must stay exactly 64; excluding the new row while keeping the offset leaves 65.
- **Helm store resilience.** `A6-C13`, `A6-C4`, `A1-C14`. Medium and low, small. `HelmStore::profiles` (helm
  store.rs:5445-5460) collects decode results into one `Result`, so one undecodable stored row fails the catalogue read
  that host refresh and create reach through `load_profile_name_index`. `update_ssh_destination` (:3530-3562) runs the
  alias-collision check even for a host that already has an alias, refusing a retarget whose display name would not
  change. `helm_link_for_session` (agent_relay.rs:588-600) takes the first matching attachment and gives up if that one
  link is unregistered, which a helm reconnect can produce while another terminal is live. Fix: skip-and-warn an
  undecodable row naming its id; include alias in the lookup and skip the check when present; iterate every matching
  attachment and return the first whose queue matches a registered link, correcting the lease-versus- connection
  docstring. Fence: do not assert all hosts or all creation fail; the alias contract's fail-closed behaviour is optional
  UX; no lease redesign.
- **Unlisted launch rows after ambiguous failure.** `A5-C2`, `A5-C3`, `A5-C10`, `A3-C11`. High, medium, medium risk;
  needs a focused failure reproduction first. The retain-the-row exits of the create path (core.rs:6551-6556 and
  :6576-6596) return `Err` without publishing a `SessionEntry`, so a possibly-running agent is unlisted and unstoppable
  until restart while the error text tells the user to stop or delete it; `reload_sessions` has exactly two call sites,
  both at startup. The keyed-retry takeover removes the existing entry at :6341 and never restores it on those exits,
  rewrites the row at `generation: 0` (:6303, :6256, :6683) rolling back a counter the module treats as monotonic, and
  clears the previous attempt's launch artifacts (:6251-6262) before `restart_pending_launch` decides the takeover is
  warranted. `Supervisor::relaunch` (core.rs:7084-7122) already re-publishes after a failed restart. Fix: a shared
  helper publishing a Launching-shaped entry on every retaining exit, paired with delete's durable `tmux_name` fallback;
  read `generation` in the takeover's snapshot and thread it through; move the artifact clear into the
  `RetryClaim::Acquired` arm. Fence: the removal is load-bearing for serializing against stop and delete; the original
  new-session-timeout premise was wrong; do not claim the probe established every interleaving; separate unit from the
  Error-reload fix.
- **Ticker witnesses agent exits.** `A5-C5`. Medium, medium, medium risk. `sample_pass` (ticker.rs:898-905) drops dead
  or missing panes and the file contains no `Transition` or `record` call, so an exit that happens while nobody polls is
  durably lost if the host then reboots; `reload_sessions` blanket-converts live rows to Interrupted on a boot-id change
  (core.rs:4419-4426). Fix: before the liveness filter, collect `ObservedExit` transitions for positively owned dead or
  absent panes and commit them via `transition_many`, gated on `may_record()`, fenced on the entry generation, behind
  the launch-sentinel check. Fence: do not infer exits from moved or unmatched panes; no generalized reconciliation.
- **`token show` migrates under a running helm.** `A6-C1`. Medium, medium, medium risk. `HelmStore::open` takes no
  `may_migrate` and its module doc argues none is needed, but `token_control::show` (token_control.rs:120-124) opens the
  store with no ownership lock, so the ordinary install-then-`token show` sequence migrates `helm.db` under the
  incumbent; today's top rungs are additive, so the incumbent survives by luck. Fix: a `may_migrate` or read-only
  distinction on `open`, `false` from `show` or the token lock taken first, and corrected module docs. Fence: a
  mixed-version workflow item; do not declare incumbent breakage without an actual destructive crossed migration.
- **Event-feed seats never reclaimed.** `A4-C7`. Medium, small, medium risk. `serve_events` (events.rs:204-236) has no
  idle or ping arm, `WRITE_DEADLINE` bounds a blocked write rather than a dead peer, and no `SO_KEEPALIVE` is set, so a
  subscriber lost without a FIN keeps its seat in the 64 cap for the helm's life; at 64 every new subscriber gets 503
  and the fleet reverts to polling. Fix: an idle arm sending a Ping and ending the subscription when no Pong arrives by
  the next interval; correct the module header's bounding claim. Fence: the ratchet is argued, not observed; the
  64-enrollment decision does not excuse stale seats.
- **Harness environment mutation.** `A4-T7`. Low, small, medium risk. `exposeHarnessDeviceSecret`
  (e2e/tests/helpers/device-auth.ts:63-65) assigns into `process.env` from a test-body path, against the standing rule
  in `.agents/test-authoring.md`. Fix: drop the env channel and read only the persisted storage state, after confirming
  a clean-tree run supplies it before config load. Fence: preserve global-setup, config-load, and worker-refresh
  ordering rather than deleting the fallback blind.

### Later: low confidence or needs an argument first

Real enough to keep, not established enough to act on. Each names what would settle it.

- **Capture parsing at the 64 KiB prefix.** `A5-C20`. `read_prefix` (agent_kind/capture.rs:626-630) takes a flat byte
  cut, and one unparseable leading line marks the whole scan incomplete and blocks every durable claim. Conditional on a
  supported vendor record whose first line exceeds 64 KiB and on no successful identity hook; neither verified. If
  shown: make the front of the read line-aware under a hard ceiling, or report mid-line truncation.
- **Partially removed `Farhelm.app`.** `A2-C25`. install.sh:1084-1090 requires `Contents/Info.plist` and :1140 does
  `rm -rf` then a cross-volume `mv` from the staging directory, so a partial failure leaves a plist-less directory the
  guard reads as foreign. Fix would stage into a same-filesystem sibling and move the old bundle aside; a missing
  Contents directory is not proof of ownership, and a foreign collision must never be deleted.
- **One-sided activity clock guard.** `A6-C22`. `record_activity` (supervisor store.rs:3660-3665) accepts only forward
  moves, so a single forward clock jump pins the stamp. The notes want the whole activity, seen, and merge path
  investigated before any clock slack, and do not accept the literal permanent-freeze claim for every excursion.
- **Descriptor ceiling and terminal socket admission.** `A4-C8`. `render_helm_unit` sets no `LimitNOFILE` or
  `StartLimit` directive, `serve_term_upgrade` takes no seat where `events_ws` does, and with the fatal accept path
  above the chain ends with the helm down and not restarting. Reachability in ordinary use is the open premise, and
  SPEC's handful-of-clients decision argues against treating hundreds of terminal sockets as expected. If pursued:
  `LimitNOFILE` first, non-fatal accept second, admission control last.
- **Aggregate input cost under the attachments lock.** `A7-C33`, `A5-C9`. `InputClient::send` has no total bound and the
  caller holds the supervisor-wide lock across it; the shipped helm chunks at 32 KiB so ordinary use is about 128
  pipelined round trips, and SPEC's local-authority decision removes the hostile framing. State the aggregate cost in
  the contract; if a bound is wanted, cap what one frame may carry where the chunking lives rather than releasing the
  lock mid-send.
- **Relaxed ordering on the relay's issued-id check.** `A1-C12`. agent_relay.rs:270 issues ids Relaxed and :374 compares
  Relaxed before the pending-table lock, then retires the connection on a miss. Nothing in-process establishes a
  happens-before, but the proposed Acquire/Release swap does not either, since the reader never synchronizes with the
  issuer. Either document the real ordering argument or make the check consult the pending table.
- **Capture columns after a failed non-Resume restart.** `A6-C5`. `begin_relaunch` clears five capture columns and
  `PriorRun` restores four other fields; the headline loss of a usable conversation is unreachable because the mode is
  validated against a non-Resume offer, leaving unrestored `first_input_at` and `capture_ambiguous` plus overbroad
  restore prose. Confirm a reachable consequence first; otherwise correct the prose.
- **Local install path without fsync.** `A2-C19`. backend.rs:620-644 renames a payload into place after `flush()` with
  no `sync_all`; the panel refuses local provisioning outright, so production reachability is doubtful. Trivial if ever
  wanted; no blanket power-loss guarantee and no remote-branch change.

## Maybe later

- Extend Muse beyond basic terminal launching: integrate per-launch hooks/instructions, capture the correct conversation
  identity for resume, and recognize Muse's waiting/status signals. Built-in `muse` and `muse-yolo` profiles currently
  use generic activity status without hooks or conversation resume; these are Farhelm integration gaps, not established
  limitations of Muse.

- Make `install.sh`'s output easier to scan. The completion message is a wall of text mixing installation results,
  restart instructions, and setup advice. Improve the layout and visual hierarchy, possibly with color; details TBD.

- Free up more space for session names in the sidebar: the agent and yolo/profile labels currently take too much of each
  row. Use one small indicator for the agent and a separate small indicator for whether it is running in yolo mode.
  Decide the agent indicator's form (SVG icon, short name, etc.) and the remaining display details when doing the work.

- Separate an agent profile's common invocation (how to invoke the agent), initial launch arguments, and resume
  arguments into three independently specified parts. The immediate restart fix is deliberately simpler: when no
  explicit resume template is supplied, reuse the original invocation and append the agent's resume syntax, assuming
  every original argument is reusable and there is no initial prompt, launch-only input, or custom command shape to
  interpret. Keep the immediate fix to launch arguments plus resume arguments; custom argument handling belongs to this
  follow-up. It must remove that assumption: initial prompts and launch-only options must not be replayed on resume, and
  existing resume/continue selectors or an end-of-options marker must not collide with the generated resume command.
  Preserve shared permission and configuration options without requiring users to duplicate them between launch and
  resume. Settle the composition, editor, and migration behavior when this is picked up; keep it out of the immediate
  fix.

- Reconsider agent parent/child relationships: either remove them or make them useful. Current parent tracking is
  optional, and fleet `agent create`/`clone` do not record the asking session, so the parent filter cannot reliably
  answer which sessions an agent created. This is largely unused complexity today; assess whether useful tracking is
  worth keeping before extending it. The current limitation is explicitly accepted in SPEC.md.

- Close the cross-host execution hole in agent-requested session creation and cloning, then remove their temporary
  exception from the host-isolation policy. These operations currently let a remote host cause arbitrary execution on
  another host; this is explicitly accepted temporarily to defer redesign, not permission to add more such operations.
  Preserve the eventual ability for agents to orchestrate sessions across hosts through an explicitly authorized launch
  policy, potentially trusted profiles, without letting the requesting host choose arbitrary execution. Include the
  existing agent/supervisor-originated creation and retry paths: a delayed resubmission must be considered when deciding
  what launch authority remains valid, including whether a forgotten retry key can launch a session again. Permanent
  retention of these agent-originated retry records is not required; their replay exposure is accepted pending this
  work. Do not add further exceptions or infer a waiver of user-initiated GUI request correctness. Cross-host stop,
  archive, and rename remain intentionally allowed bounded operations.

- Let the user mark each host as "yolo is fine" or "yolo is not fine", controlling which hosts appear red in the session
  list. This could also support warnings when the user is about to run an unsandboxed agent on a host marked "yolo is
  not fine".

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

- Replace xterm.js with libghostty via WebAssembly, assessed 2026-09-12 in `lore/2026-09-12-libghostty-assessment.md`.
  Native libghostty on macOS is ruled out there: it owns its own PTY and renders into an NSView, neither of which fits a
  WebSocket-fed terminal inside a webview. The WASM route through `coder/ghostty-web` (xterm.js-compatible API over the
  upstream wasm) is plausible but not a drop-in: `onBinary`, `parser.registerOscHandler`, `buffer.active`, and `refresh`
  are missing, its `write` parses synchronously so the backpressure model in SPEC_impl.md has to be re-derived, and the
  upstream wasm ships only on the rolling `tip` release with nothing stable to pin. Medium to high effort; the payoff is
  dropping the scroll-freeze workaround and getting Ghostty's own grapheme and SGR handling. First step is a one- or
  two-day spike mounting ghostty-web in the island under WebKit.

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
