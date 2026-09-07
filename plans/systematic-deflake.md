# Systematic deflake

Written against `2069e0c8` on 2026-09-06. Plan for the "Systematic deflake" bucket in TODO.md; the bucket carries the
ordered one-paragraph entries, this file carries the evidence and the detail behind them. Effort: the bucket's entries
are low to medium each; the word on each entry rests on the mechanism notes below, not on a measurement.

## The goal

Become less flaky as a system: the rate at which flaky tests are CREATED falls, and a flake that is created is
DISCOVERED and DIAGNOSED while the author still has the context. Fixing the entries under "Difficult deflake" is not the
goal. Those entries and FLAKES.md are evidence of which kinds of flakes this codebase produces; the plan is judged by
whether it stops producing those kinds, not by whether it closes those tickets.

Execution order is not the same as the weight of the sub-goals. Prevention is the largest lever by count, and its inputs
(the diagnosed classes) already exist, so it runs in parallel with the evidence work rather than behind it. The early
evidence work makes local/worker runs inspectable without adding a hosted hunt or a mandatory stress campaign.

## What the evidence says

One row per distinct issue in FLAKES.md, plus one unit-level failure CI recorded and no ledger did. Status: D = cause
diagnosed and fixed or confirmed; H = hypothesis with trace support; U = unknown after investigation.

| Class                                                                | Issues                                                                                                                                                                                                                                | D | H | U |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | - | - | - |
| Acted before a readiness/focus/reveal handoff settled                | profiles focus fixtures ×3 (D, #385); editor focus during fill (D, #385); menu ArrowDown vs initial reveal (H); popup-created profile (H)                                                                                             | 2 | 2 |   |
| Same bytes reachable by two delivery paths (snapshot vs live)        | hexecho tokenizer (D, #333); 0xff via snapshot (D, #333); argv-width guard ×2 (D, #338); one seam, no recurrence since                                                                                                                | 3 |   |   |
| Fixture premise silently invalid; passed only by winning a race      | aged-out FLOOD prefix ×3 (D, #357); planted launch path (D, #355); fixture waiting for a canonical-mode line that never completed (D, #339)                                                                                           | 3 |   |   |
| Teardown or write while the other side was already gone or in flight | hook exited before payload write (D, #353); interceptor removed with fetches in flight (D, 2026-09-05)                                                                                                                                | 2 |   |   |
| Cross-test interference inside one test process                      | sweep flock inherited by a sibling's child (D, #384, `farhelm-teststate` unit binary); rename replacement claim (H, same mechanism, e2e); stalled-viewer RSS sampled from the shared e2e binary (H)                                   | 1 | 2 |   |
| Fixed budget with no margin                                          | over-one-megabyte paste, reply at the deadline (D by trace)                                                                                                                                                                           | 1 |   |   |
| Two mechanisms share one observable                                  | stall-detach banner 404 ms after gate (H: helm channel-full backstop)                                                                                                                                                                 |   | 1 |   |
| Pointer/focus state left by the previous step                        | tab hover tint vs accent (H)                                                                                                                                                                                                          |   | 1 |   |
| Investigating environment's substrate unverified                     | forced-pause tab/underscore: reported deterministic on pinned 3.7c, but CI run 34006471792 on `2069e0c8` runs all four tests green on the pinned build at four threads, and the pinned binary emits a tab under `C` and `C.UTF-8` (H) |   | 1 |   |
| Product defect that load or a fixture exposed                        | helm-death client-lifetime leak (H: fixed 2026-09-05, but the entry says it does not establish the historical cause); inert-click focus obligation (D by trace, Near term)                                                            | 1 | 1 |   |
| Deterministic regression whose first run was a loaded run            | replay-stale-mount (D, #340)                                                                                                                                                                                                          | 1 |   |   |
| Unknown                                                              | non-UTF-8 marker lost; degenerate-size READY; surviving sentinel; geometry epoch; `manager::tests::editing_a_duplicates_destination_resolves_it_while_the_twin_lives` (CI run 33992744776 on main, 2026-09-05, in no ledger)          |   |   | 5 |

Totals: 26 issues; 13 diagnosed, 8 hypothesis, 5 unknown.

Observations that drive the ordering:

- Ordering races dominate: about twelve of the twenty-one diagnosed-or-hypothesized issues are "the test acted,
  measured, or tore down at a moment the system was not in the state it assumed." Budgets are one issue. Three "timed
  out waiting for X" failures were read as budget problems before turning out to be an invalid premise (#357); a scaled
  budget would have made them fail slower.
- Discovery's denominator is near zero. `ci.yml`'s concurrency group keys on the ref with `cancel-in-progress`, so
  landing a stack cancels every main run but the tip's: 34 of the last 40 main runs were cancelled, 5 completed. The
  full suite completes about once per landed stack. The manager unit failure above was not re-run green; it was
  superseded.
- Diagnosis is the bottleneck for the unknowns, and the evidence they need is mostly supervisor-internal (raw control
  markers, forwarder enqueue, writer completion, unlink errno), which no harness-side tmux query reaches. Per-test
  tracing capture does, and it also reaches unit-test binaries, where the one unit-level failure lives.
- The corpus in FLAKES.md is a five-day, e2e-only sample. An older corpus is scattered through harness docstrings (the
  never-started launch that `basic_session_ready` exists for, tmux 3.4 exit-code loss under load, `pane_states`
  degrading to an empty map, the `kill-server` race, six control-budget expiries in M6.5) and `docs/watch-items.md`
  items 11–14. Two classes appear only there: single-shot reads of an eventually-consistent listing, and long-lived
  process resource exhaustion. Four `service::sweep::tests` also fail under an ambient `FARHELM_AGENT_ID`
  (`.agents/narrow-tests.md`).

## The items, in order

### 1. Ledger and execution evidence (low)

Implemented by the local recorder and release/agent integration. Its consumer contract is
[`docs/test-run-evidence.md`](../docs/test-run-evidence.md); historical substrate claims that cannot be verified are
explicitly qualified in TODO.md and the appended FLAKES.md caveat. The remaining items still use this plan.

Cheapest discovery multiplier on the page, and the precondition for every metric below.

- A failed local, worker, or applicable release run is not cleared by a later green run: retain its evidence until a
  FLAKES.md entry or a fix PR exists. The entry may be short, but it follows the file's shape.
- Substrate and environment identity recorded in every failure output and every FLAKES entry: `command -v tmux`,
  `tmux -V`, sha256 of the resolved binary, locale, and any ambient `FARHELM_*` variables. The harness already runs
  `tmux -V`; this widens what it keeps. Controlled release and hunt runs assert equality with `source-pins.env`; the
  developer loop records and warns. The harness scrubs or refuses ambient `FARHELM_*` at startup. Motivated by the
  forced-pause entry claiming a determinism CI contradicts, with no way to check what the worker ran.
- Preserve bounded failure artifacts in existing applicable release jobs. Do not add hosted test jobs, schedules, or
  workflow-dispatch hunts merely to collect evidence.
- Re-verify the 2026-09-05 baseline reproductions' substrate before "Difficult deflake" entries cite them as baseline
  evidence, and correct the forced-pause entry.

### 2. Readiness oracles by naming (medium)

The oracles exist and are unused: `wait_for_replay_complete` ("snapshot consumed, everything after is live") is called
at 3 of 161 attach sites, `basic_session_ready` at 11 of 159 session creations, `waitForReplayReveal` in three specs
while specs read `__farhelmTermReady` directly 106 times. #338 went around the oracle that would have prevented it.

- Make every call site choose by name, so the exclusion set is a grep and never a list: `attach_live()` returns a stream
  positioned after `ReplayComplete`, `attach_at_boundary()` is the raw form for the tests that pin the boundary itself;
  `basic_session` takes the ready-waiting behavior and today's form becomes `basic_session_mid_launch`. The docstring on
  `basic_session_ready` already says why the split is deliberate; the rename makes the deliberate choice visible in
  every diff.
- Optionally a type: `attach_live` returns a stream on which `wait_for_replay_complete` does not exist, retiring the
  "must be the first wait" rule its docstring has to shout.
- Browser: `attachSession` in `term.ts` is the chokepoint for the raw `__farhelmTermReady` reads; focus-driving fixtures
  wait for the pending handoff before injecting, as #385 did per test, as the helper's behavior.
- Single-shot reads of an eventually-consistent listing get a named poll helper; this is the older corpus's readiness
  variant.
- A test that asserts on live bytes obtains them through its own attachment after the oracle, never from the snapshot.

### 3. Evidence on first failure (medium)

- Per-test `tracing` capture, dumped on failure. The capture layer in `attachment_uploads.rs` uses `try_init`
  (process-global, first wins) and cannot be per-test as written; use `tracing::subscriber::with_default` around the
  test body, or a layer partitioned by test thread. Harness-owned, and it covers unit-test binaries, which no tmux dump
  can.
- A harness `Drop` that runs when `std::thread::panicking()` and issues synchronous tmux queries (`list-clients` and
  `list-panes` with flags, pane capture with dead state and dimensions) plus the harness's timestamped event timeline.
  `TmuxServerGuard::drop` already does synchronous work in `Drop`; the struct's fields drop after the body, so the
  server is alive when the dump runs. About 34 sites across 11 files build `TmuxServerGuard` by hand and need a shared
  constructor or stay undumped. Supervisor-internal state (forwarder, writer, control stream) comes from the tracing
  capture, not this dump; a test's private samples (the RSS test) are written to the timeline by the test.
- Browser: bridge, feed, and focus helpers emit timestamped console events so the retained trace answers "no socket
  request vs late request" and "focus moved before or after the fill."

### 4. Process-per-test with cargo-nextest, workspace-wide (medium)

Retires cross-test interference as a class instead of fixing instances, and supplies per-test timeouts, per-test JUnit
(the ledger's input), and retries that report "flaky" instead of hiding it.

- Workspace-wide, not e2e first: the one confirmed instance (#384) is in `farhelm-teststate`'s unit binary.
- nextest interleaves every binary's tests under one thread budget, where `cargo test` runs binaries sequentially. That
  is new cross-binary contention (e2e stacks beside `farhelm-supervisor` tests spawning tmux and `farhelm-teststate`
  sweeping `/tmp`), the one way the switch could create flakes. The profile sets an explicit `test-threads` (nextest
  defaults to logical CPUs, and the harness's `SLOTS` semaphore becomes inert at one permit holder per process), a test
  group with `max-threads` for the e2e binary, and `success-output = immediate` so loudly-skipped tests keep their
  reason visible.
- One runner model: CLAUDE.md's gate list, explicit local/worker instructions, and release execution change together, or
  the shared-process class survives locally and vanishes from the evidence model.
- Retries only for the load-sensitive group (initial membership: the "Difficult deflake" entries), always reported, with
  the JUnit "flaky" outcome feeding FLAKES.md. Deterministic failures do not retry away.
- The RSS test starts measuring the supervisor plus one test's harness instead of everything co-resident, which is the
  measurement it claims; re-baseline the allowance and say so in the test.

### 5. Local/worker hunt tooling (low for Rust, medium for the browser leg)

- Developer- or worker-invoked tools reproduce the gate's relevant substrate and concurrency, run bounded repetitions
  over selected or changed tests, and preserve item 3's evidence. This is the shape that produced the corpus; the
  2026-09-05 "two CPU-load children" shape reproduced only part of it (the profiles pair, over-one-megabyte, and the
  sentinel all failed with no extra load).
- A changed Rust test can use twenty fresh invocations, and a changed Playwright spec can use `--repeat-each=20` on both
  engines. A change to `harness.rs`, `terminal-suite.ts`, or `terminal.js` may require the whole binary or suite, since
  every module imports them. Twenty repetitions are a filter, not a proof: #355 passed three gates and a sandbox before
  failing once. The command and its cost stay explicit; no repetition is mandatory for every PR or edit.
- The browser leg needs `cargo build`, a `dx` release build, and a Playwright install per run, the cost that got the e2e
  job disabled. It remains explicit local/worker work, never a nightly, on-demand workflow, or per-PR stress job.

### 6. Authoring rules as a reviewer checklist, and a sleep allowlist (low)

A paragraph in a fingerprint document is not something a reviewer prompt consumes. Rules go into a short checklist that
the review swarm's test-quality lens loads verbatim and that CLAUDE.md's "Finishing work" names for any PR touching
tests; `docs/watch-items.md` keeps the fingerprints and mechanisms and cites the checklist.

- A fixture's premise is asserted, not assumed; a timed-out wait reports whether the premise still held.
- Readiness comes from the named oracle, never from `__farhelmTermReady` or a sleep.
- Before writing to or tearing down another party, confirm it is still there.
- No lock or fd held across a spawn the test or product performs; nextest removes siblings, not the mechanism.
- Measure a process the test owns, never the shared binary.
- If two mechanisms can produce the observable, assert on something that distinguishes them.
- Polls do not return megabyte buffers; measure processing and serialization separately.
- Reset pointer and focus state between steps that depend on it.
- Sleeps live in harness helpers (`poll_until`, `observe_quiet_for`); a sleep in a test body carries a
  `// sleep-ok: <why>` annotation and an existing lightweight validation path requires zero un-annotated ones. A count
  ratchet was considered and rejected: of about 103 sleeps in e2e test bodies, 28 are poll intervals and
  `feed.spec.ts`'s deliberate observation windows are legitimate, so a count punishes the wrong thing and is gamed by a
  named constant.

## Deferred, with triggers

- A scale factor on harness budgets. Budgets are one issue in 26 and three budget diagnoses were wrong. Trigger: a
  budget-class recurrence after item 2 lands. When it comes, it comes as a typed budget (`Budget::harness` vs
  `Budget::product`, and `expect.configure` plus a helper on the browser side) so the factor cannot touch a product
  deadline; `terminal-flood.spec.ts` has a 75 s harness wait and a 55 s product claim on adjacent lines.
- Product observables that attribute their cause. The stall-detach reason string is shared by design (`client.rs`: "they
  must be identical — that is why the constant exists"), so attribution is a secondary field or a log line the tracing
  capture keeps: detach source (supervisor stall vs helm backstop), sentinel unlink path and errno, bounded and
  observable helm-death detection latency. Product work for "Near term," not this bucket.
- Running the tag gate's suite away from the release build. The tag gate excludes the e2e binary
  (`dist-build-setup.yml`: `--exclude farhelm`), so nothing is measurable until "Restore the release integration gate"
  lands; the `--test-threads=2` experiment belongs to that entry.

## Tracking

FLAKES.md is append-only and excludes same-session flakes, so it cannot measure creation rate alone. A short explicit
manual script (under `scripts/`) summarizes FLAKES.md headings and retained local/worker/release reports:

- failures per hunt run (item 5), the denominator;
- share of new FLAKES entries whose last line is `Cause: established`, against `hypothesis` and `unknown`, and the count
  still saying "on recurrence" or "not kept" (FLAKES.md's header is amended to require the `Cause:` line on new
  entries);
- development runs, focused repetitions, and release runs separately, with missing or incomplete evidence called out;
- new entries per class, tagged on new entries only; existing entries are classified in the table above.

The operator chooses where portable summaries are archived; raw local output and environment identities stay out of
public source. The first reading that means anything is months out; a class recurring before then is the signal to
sharpen its item, not to add a retry. The script must report its denominator and must not claim a long-term reduction
from a handful of development runs.

## Review record

Two adversarial reviews by independent reviewers, 2026-09-06. Review 1 moved diagnosis ahead of prevention, reframed the
oracle item as adoption of existing oracles, split nextest out of gate management, added the ledger-integrity item, and
corrected the classification table. Review 2 found the concurrency cancellation on main, moved evidence capture to
per-test tracing (the tmux dump reaches one of five unknowns), corrected nextest's target binary and load shape,
replaced the exclusion list with call-site naming, replaced the sleep count ratchet with an annotated allowlist, and
collapsed nine items to six. Declined from the reviews: dropping the scale factor outright (deferred with a trigger
instead), and adding a "Near term" entry for product attribution on the plan's own initiative (recorded above as
deferred; the maintainer adds Near term entries).
