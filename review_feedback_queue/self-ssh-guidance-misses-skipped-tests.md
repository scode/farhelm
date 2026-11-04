# Missing self-SSH can skip tests instead of failing them

Reviewed commit: 237d6f28d6dff8ccca3559174110d3fb24e2d2e0

## TLDR

The sandbox instructions say failed self-SSH makes dependent browser tests fail. In the ordinary non-CI branch, the
multi-host suite skips those tests instead. An operator looking only for the documented failures can miss absent
coverage.

## Details

Suggested bucket: **other**. Confidence: **confirmed for the non-CI branch**. Proposed drop: none. Possible cover: the
following sentence correctly says a failed self-SSH run is not evidence; this limits the consequence, but does not
correct the promised failure signature.

`.agents/tensorlake.md:145–146` promises every dependent spec fails with the SSH-fixture error. In reality,
`scripts/tensorlake/run-shard.sh:124,136–148` records the failed probe and later returns Playwright's exit status.
`e2e/tests/helpers/fleet.ts:1482–1490` returns false for failed localhost SSH. `terminal-multihost.spec.ts:807–814`
leaves the fleet unready; `requireFleet` at 501–512 skips dependent cases unless `CI` is set. The shard command does not
force `CI`, and the recorder preserves that ordinary environment input. An external runner that sets `CI` takes the
explicit failure branch.

Describe both outcomes and require the independent self-SSH success witness. The nonfatal script policy predates the
reviewed patch; this finding concerns its newly added documentation, not a new harness exit-policy defect. No actual
false coverage claim is established when the instruction's evidence warning is followed. No sandbox or browser test was
run for this finding.

The route persists through c5ae8548439cc5bfa33c11ef7ce8da6129d8072a. Targeted specification, Planned, unlanded-plan,
known-bug, queue and ledger checks found no exact cover or filter. Provenance: review run
`ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g059/g059-c3`.
