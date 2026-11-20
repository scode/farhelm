# Flake archive tests skip the command’s file selection

Reviewed commit: 7a3270966d90aaf67c0fb69faa8a066ba4fb9a2f

## TLDR

The reporting test can pass if the command stops loading the closed-flake archive by default or ignores a repeated
`--flakes` option.

## Details

The split-ledger test at `scripts/test-summarize-test-runs.py:101–127` hands both paths to the report function. The
command-line cases pass a single option, leaving default selection and repeated-option parsing in
`scripts/summarize-test-runs.py:101–107` outside their assertions. Add command-level fixtures for both selection paths.
The source confirms the coverage gap.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `test-substrate/f1`.
