# Triage outcomes

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
