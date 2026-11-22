# Worker refusals can copy unrelated evidence even without overlapping runs

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A test worker refusal can return evidence from an earlier invocation under the current run's output directory.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d scripts/test-worker/test-worker.sh:54–66,85–86,127–133,146–168;
scripts/test-worker/remote/run-run.sh:7–17,19–27; scripts/test-worker/remote/with-lock.sh:12–24

A fresh output directory can receive an earlier run’s summary and console log even though the requested command never
ran. The strongest route is sequential: test-worker.sh only requires nonempty --out, so generic run can reach the worker
with no child command. with-lock.sh acquires its lock, invokes run-run.sh, and relays its exit 2. run-run.sh:9 refuses
before resetting the fixed out/run directory (12–13); a previous generic invocation can have left console.log and
summary.txt there (17–25). The driver then fetches that shared directory without checking invocation identity and
retains status 2. A separate conditional overlap route remains: distinct --checkout paths with the same basename and
worker destination share the same base (61,85–86). A held lock returns 75 without starting or rotating output, but fetch
still runs outside the lock. The Rust path can contribute a summary, console and prior run archive
(run-rust.sh:12–13,30–47) if present. A clean local destination and ordinary argument handling do not prevent either
route. GUI invalid-suite validation is different: run-gui.sh:13–14,25–39 resets its worker output first.
.agents/test-worker.md:58–65 warns about reused local output only. The verified consequence is evidence misattribution,
with a refusal still returned; whether a human consumes the stale files is unverified. Check generic arguments before
dispatch and bind every copied output to the specific invocation. The reviewer suggested other, confirmed by source, ,
Possible coverage: null.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `test-and-release-tools/trl-02`.
