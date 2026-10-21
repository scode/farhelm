# The replacement cleanup test never proves its child lived

Reviewed commit: 1f991980581df1304ce955e40b5bdee34942a7f3

## TLDR

The cleanup regression can report successful reaping when its stand-in failed or exited before cleanup. An isolated
early child failure has not been reproduced; this is missing test evidence, not a demonstrated production leak.

## Details

`scripts/test-start-stack-cleanup.sh:380–382` backgrounds a supervisor-shaped `sleep` and records its PID; `386` signals
the stack without proving that child is alive with matching argv. `await_gone` (`192–204`) and the success assertion
(`391–394`) accept an already absent child. No child exit-status assertion distinguishes that counterexample. Establish
bounded live matching-argv readiness before the action and retain diagnostics while the fixture is owned.

The decoy survival checks (`106–114,383–399`) are useful negative controls but prove a different process lived. A
globally missing `sleep` would generally fail those checks, so that is not a convincing silent-pass trigger; failure
confined to the new child is the unverified premise.

Suggested bucket: **other**. Confidence: **confirmed missing positive premise and indistinguishable absence oracle;
actual false pass unverified**. Proposed drop: none.

Possible cover: none found across both SPECs, Planned TODOs, BUGS, queue, ledger and FILTER. The trap-deferral TODO at
final `TODO.md:282–296` concerns a different failure. Accepted short-window PID reuse and hostile-account tooling rules
do not waive fixture readiness or prove reaping. Provenance: `browser-harness-tests-3/f2` and sibling corroboration,
trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `browser-harness-implementation-1/f4`, `browser-harness-tests-1/f4`, `browser-harness-tests-2/f3`,
`browser-harness-tests-3/f2`, `browser-harness-tests-4/f4`, `browser-harness-contracts-1/f5`.
