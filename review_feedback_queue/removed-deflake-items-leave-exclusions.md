# Removed flake work leaves orphaned sweep exclusions

Reviewed commit: 2897c9141088894ca44dd1172ce094c9ca8cea2b

## TLDR

Two tests keep their deflake-sweep exclusions after the work items tracking those exclusions are deleted. Removing the
items does not restore those tests to future sweeps.

## Details

Suggested bucket: **other**. Confidence: **confirmed source inconsistency**; no test failure or product regression is
claimed.

The removed entries are at parent `968e842eaab2eadcb6644b0d28a4d17a507b6fab:TODO.md:406–431`; their deletion collapses
to `TODO.md:389` in the reviewed commit. Both identities remain in `deflake/known-flakes.txt:14–15`.
`deflake/bin/deflake:266–306,326–331,1259–1303` consumes that file independently of TODO, so the exclusions survive.
`deflake/SPEC.md:32–37` requires removing both representations together, including when work is dismissed.

The tests were already excluded before this commit: the new defect is orphaned tracking, not a newly reduced test
selection. The later structured-Settings report also says one deleted item remains open
(`917c63b388eb90b7f5b8d5ead4651135fdba47d9:plans/reports/structured-settings-dialog.report.md:169–170`), another symptom
of this inconsistency. The work's deliberate removal does not settle whether the machine exclusions should remain.
Reconcile them with that decision.

Possible cover: none found. Single-user tooling and concurrent-run acceptances do not cover orphaned exclusions; this
persistent bookkeeping mismatch matches no filter. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g021/f1`.
