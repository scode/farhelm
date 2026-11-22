# Triage outcomes

## browser-shard-failure-sweeps-running-siblings.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. In the Tensorlake Rust test runner, one shard failing to create its sandbox,
  or failing to save local output, makes the run's final sweep terminate sibling sandboxes that are still running,
  losing their evidence. Verified: The browser half is gone: #3401 removed the browser harness, and sandbox-tests.ts is
  now only make-template and sweep. The Rust half is still true: rust-tests.ts ~164-173 uses `Promise.all` and then
  sweeps in `finally`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix together with browser-shard-failures-exit-successfully.md and
  worker-evidence-deleted-after-download-failure.md as one small runner PR (`allSettled` before the sweep). Narrow the
  item to the Rust runner.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## browser-shard-failures-exit-successfully.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. The Tensorlake Rust test command exits successfully even when shards failed.
  It prints "SOME SHARDS FAILED" but does not set an exit code. Verified: The browser half was removed by #3401. The
  Rust half is still true: rust-tests.ts ~170 prints the result without setting `Deno.exitCode`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix (one line).
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## worker-evidence-deleted-after-download-failure.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. If copying a failed Rust shard's evidence back fails, the runner still
  terminates the sandbox and deletes the only copy. Verified: The browser half was removed by #3401. The Rust half is
  still true: rust-tests.ts `shard()` runs `tl sbx cp ... .noThrow()` and then terminates in `finally`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix together with the two items above, or discard.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## worker-allocation-precedes-ownership-ledger.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. A Tensorlake sandbox created by the test runner can escape cleanup (and keep
  billing) if writing the local ledger fails, or if the create call times out after the sandbox was actually made.
  Verified: Still true. sandbox-tests.ts `create()` creates first and appends to the ledger afterwards. A failed ledger
  write is a local filesystem failure (accepted under "Healthy local filesystems"); the timeout case is not.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Discard (the remaining case is rare, and the reserved prefix makes leftovers easy to spot).
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## rust-sandbox-runner-passes-empty-selection.md

- Outcome: `other`.
- Assessment: Maintainer tooling. The Tensorlake Rust test runner prints "ALL SHARDS PASSED" after running nothing when
  given `--shards 0`, a non-number, or a filter that matches nothing. Verified: Confirmed.
  `scripts/tensorlake/rust-tests.ts` ~L151 does `Number(args.shards)` with no validation, and `Array.from({length: 0})`
  followed by `every()` is true. Shards run with `--no-tests warn` and only exit codes are combined.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code. A false pass claim is the worst kind of tooling bug, and the guard is tiny.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## rust-sandbox-pack-ignores-cargo-target-dir.md

- Outcome: `other`.
- Assessment: Maintainer tooling. A checkout built into a shared `CARGO_TARGET_DIR` cannot be packed for the Tensorlake
  Rust tests, because the runner looks for `target/debug/farhelm-fixtures`. Verified: Confirmed (`rust-tests.ts` L58).
  AGENTS.md, "Sharing the machine with other agents", explicitly supports a shared `CARGO_TARGET_DIR`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code (small). At minimum, refuse with a clear message.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## rust-sandbox-staging-misquotes-checkout-path.md

- Outcome: `other`.
- Assessment: Maintainer tooling. The Tensorlake Rust runner cannot stage a checkout whose path contains a space.
  Verified: Confirmed. The `unpack` string at `rust-tests.ts` ~L88 interpolates REPO into `sh -c` without quoting. The
  repo elsewhere quotes for paths with spaces (`harness.rs` `fixture_cmd`).
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code (tiny), or discard if checkouts with spaces are not a supported case for this runner.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-rust-worker-trims-explicit-payload-directory.md

- Outcome: `other`.
- Assessment: None (maintainer tooling). Distributed Rust test staging fails if an operator overrides the payload
  directory with a path ending in a space or tab. Verified: scripts/tensorlake/rust-tests.ts:62 on main still `.trim()`s
  the producer output before `tar -C`. No filter matches (a retry repeats the failure).
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard (nobody names directories with trailing whitespace). A fix is also trivial (strip only the trailing
  newline) if preferred.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## suspended-plan-test-erases-its-suspension-premise.md

- Outcome: `other`.
- Assessment: None. The test that a setup confirmed after a sandbox was found suspended never dials it may race:
  confirmation re-registers changed install paths, which briefly republishes Connecting before the guard reads the
  state. Verified: The test still exists as `a_plan_made_while_running_does_not_run_once_suspended` (`provisioning.rs`).
  The fixture uses root/lib and root/state while the harness admits /fixture paths, so the reconfiguration happens. I
  did not verify which state the guard sees or whether it would then probe. Not in FLAKES.md.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard unless it shows up as a flake. Otherwise fix the fixture so its paths match the planned ones.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## tunnel-fixture-assumes-simple-short-temporary-root.md

- Outcome: `other`.
- Assessment: None. Two Tensorlake transport unit tests fail when the machine's temp directory path contains `%`, `"`,
  `\` or `'`, or is too long for SSH connection sharing. Verified: The tests still exist (`transport.rs`
  `a_tensorlake_row_is_dialed_with_farhelms_key` compares raw path text against the encoded `IdentityFile`). They pass
  with normal /tmp.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## redirect-regression-omits-location-header.md

- Outcome: `other`.
- Assessment: None. The test meant to catch Farhelm following a Tensorlake redirect would still pass if the no-redirect
  policy were removed, because its 307 reply has no Location. Verified: Confirmed (`tensorlake/tests.rs` scripts a 307
  with no Location). Production `http.rs` sets `Policy::none()`.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code, tiny (add a Location to a sentinel route and assert that route is never hit). It guards a security-relevant
  policy.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-http-golden-omits-populated-environment.md

- Outcome: `other`.
- Assessment: None. The HTTP contract golden for the host list has no sandbox or container example, so renaming the
  `environment` field or changing its shape would not fail that test. Verified: http_contract_tests.rs:126 is the only
  `environment:` entry (None). http-contract/host-list.json has no "environment" key. The GUI decodes the same Rust
  type, which limits how much a rename could actually break.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: add one populated Tensorlake or Podman row to the golden. Discarding is also defensible, since the
  shared serde type protects the GUI.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md

- Outcome: `other`.
- Assessment: None. The test that a sealed sandbox's setup is refused before it takes the host compares the public
  state, which the seal overlay masks, so it would not notice a connection hold. Verified: Partly fixed: a sibling test,
  `a_setup_refused_after_its_claim_withdraws_it_through_the_feed` (tensorlake_add_tests.rs:2198-2227), now asserts that
  no run is retained. The hold half is still unobservable (:2172-2176 compares `manager.state`).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code (assert the raw hold or incarnation, or narrow the docstring). Discarding is also reasonable.
- Execution: planned in `plans/queue/triage-tensorlake.md`.
