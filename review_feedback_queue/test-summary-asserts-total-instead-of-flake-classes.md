# The flake archive test accepts the wrong class totals

Reviewed commit: 7a3270966d90aaf67c0fb69faa8a066ba4fb9a2f

## TLDR

The test can pass if the summary counts both fixture flakes in the same class.

## Details

`scripts/test-summarize-test-runs.py:101–127` assigns distinct classes to the active and archived entries. Its final
assertion adds their class counts and accepts either class holding the full total. The adjacent date and Cause
assertions exercise separate classifiers. Assert the expected count for each class. The assertion gap is
source-confirmed.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `test-substrate/l9`.
