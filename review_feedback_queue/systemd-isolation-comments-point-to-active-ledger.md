# Systemd isolation comments point to the wrong flake ledger

Reviewed commit: ea133b9941769c69dc4683aa485b265dd09ed70c

## TLDR

The new test comments send maintainers to the active flake ledger for evidence that has moved to the archive.

## Details

The references in `crates/farhelm/tests/e2e/harness.rs:1880`, `crates/farhelm/tests/e2e/create_idempotency.rs:634` and
`crates/farhelm-supervisor/src/service/core.rs:27731` point to `FLAKES.md`. The specified observations are in
`FLAKES_ARCHIVE.md` at lines 4906, 4958, and 5006 of the reviewed revision. Point the comments to the archive and keep
the dates.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `test-substrate/l6`.
