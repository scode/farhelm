# An interrupted case is retained as a failed drift verdict even before libtest starts

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A cancelled case can leave a failed drift verdict in its retained evidence before the test binary starts.

## Details

At the pinned head, scripts/record-test-run.py:1819–1854 announces the evidence directory and can finalize it with
outcome=interrupted before spawning libtest. Its contract distinguishes interruption from actual command and recorder
failure in docs/test-run-evidence.md:189–204. The drift runner's read_verdict at scripts/run-harness-drift.py:84–108
handles a missing Rust verdict and reads the recorder manifest. It recognizes timed_out but not interrupted, so an
interrupted manifest falls into a generic failed result. recorded_case writes that result to drift-verdict.json at
:147–160, then :164–169 correctly exits with the signal status if the parent got the signal. The manifest and exit
status still preserve interruption. It is the case-level verdict that incorrectly says failed, although
docs/harness-drift-tests.md:34–40 describes FAILED as a CLI or test failure after starting. Keep canceled attempts
distinguishable in retained evidence, either through an explicit interrupted outcome or by retaining only the recorder
manifest when no case verdict exists. Still preserve any genuine test verdict produced before cancellation.

Confidence: confirmed by inspection of the pre-spawn interruption branch, manifest reader, and write order.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l19`.
