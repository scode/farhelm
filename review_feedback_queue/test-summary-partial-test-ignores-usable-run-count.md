# Partial-report test ignores the valid run count

Reviewed commit: 7a3270966d90aaf67c0fb69faa8a066ba4fb9a2f

## TLDR

The partial-report test can pass if an unavailable flake ledger causes the summary to lose valid test-run counts.

## Details

`scripts/test-summarize-test-runs.py:129–139` seeds a valid run and asserts incomplete discovery and an unavailable
ledger. It never checks the seeded run’s contribution. The production report retains independent run evidence on a
ledger read failure as described in `docs/test-run-evidence.md:426–431`. Assert the eligible run remains counted,
preferably with one readable ledger and one unavailable ledger. The test gap follows from source.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `test-substrate/l10`.
