# Missing recovery evidence can downgrade a separately verified product failure

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Missing retry diagnostics can downgrade a recorded product failure to Could not run.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
releasing/mac-vm-test/brick_test.py:961–976,1362–1394,1440–1443,1468–1475,1506–1513;
releasing/mac-vm-test/BRICK-TEST.md:194–197,639–641; releasing/mac-vm-test/RECIPE.md:389–390

Once the verifier has recorded a product failure, losing a different recovery attempt’s evidence can change the
prominent pass and release verdicts to could not run even if the product failure’s evidence is intact. The coordinator
records completed product refusals before CheckStopped and does not convert that exception into automation failure.
Later, verdict checks all three classes of recovery records before checking the current result's fail status, so missing
or damaged recovery artifacts short-circuit it. P0 and the overall report aggregate those verdicts. The underlying
recorded result still exists. RECIPE says a demonstrated product failure makes the relevant verdict fail and reserves
could not run for missing environment without a demonstrated failure. Revalidate and surface the valid current failure
independently of the recovery-evidence gap, which should still block any pass. The reviewer suggested other. Confirmed
source precedence given independently retained failure and damaged recovery evidence. No evidence removal or affected
production run was observed. , Possible coverage: null. Provenance: inherited verdict precedence.
9d6c18cb3d0c5d87b6911c2e891293c8e42c6ee8 brick_test.py:1378–1393 has the same early recovery-evidence checks.
2d1bb2e4ef0113729c79e3ea7e2648c262669cd1 brick_test.py:866–873,897–920 now permits retry under the effective checkpoint
floor when the base timeout alone would have refused it. That widens reachability to those retry cases while leaving the
misclassification unchanged.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `test-and-release-tools/trl-17`.
