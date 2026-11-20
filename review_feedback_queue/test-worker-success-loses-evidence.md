# A test worker can report success when the evidence copy fails

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

A successful SSH worker run can exit successfully even when its logs and summary failed to reach your machine. If you
reused the output directory, the driver can print the previous run's summary.

## Details

The driver saves the remote runner's status, calls the evidence copy, prints any local summary it finds, then exits with
the saved status. This leaves the copy failure out of the result even when it prevented all evidence from arriving. Keep
the runner and evidence outcomes separately and print a summary only when it belongs to that run. Source:
`scripts/test-worker/test-worker.sh:128` and `:162`.

Introduced by `663b6c9ce`. Source-confirmed. A successful remote run can still have finished correctly.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g065/l7.
