# An empty heading can change another flake ledger’s count

Reviewed commit: 7a3270966d90aaf67c0fb69faa8a066ba4fb9a2f

## TLDR

A malformed heading in one selected flake ledger can invalidate the final cause tag in a different ledger.

## Details

`scripts/summarize-test-runs.py:36–62` admits an empty level-two heading and joins the files before parsing.
`scripts/test_run_summary.py:29–38` and `:57–75` require a named heading to start a new entry, so the previous entry can
absorb the empty heading. Parse the ledgers independently or retain an explicit boundary. The source supports the
custom-ledger failure. The default ledgers contain their entry boundaries.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `test-substrate/f2`.
