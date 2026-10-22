# Spare owner tests register child cleanup after readiness

Reviewed commit: 8f5d016f04ab13c284530e759e828826790f2843

## TLDR

Two prepared-checkout owner-death tests can leave their disposable background process running for up to five minutes
after a readiness failure. This affects the private test fixture, not a shipped service or user-owned process.

## Details

Suggested bucket: **other**, for incomplete test cleanup. Confidence: **confirmed conditional source mechanism**. A
readiness timeout while the owner remains alive is required; that schedule and its frequency have not been measured.

`crates/farhelm/tests/spare_builder.rs:565–574` has the child owner spawn `sleep 300` and publish its PID. The parent
does not register `Indirect` cleanup until the later guard/rc readiness checks finish (`603–620`), or until the held
fetch readiness checks finish (`677–706`). A timeout before registration unwinds through `Owned(owner)`, whose
destructor sends SIGKILL (`215–220`). That prevents the owner's own `Owned(unrelated)` destructor from cleaning the
sleep, while the parent has no registered cleanup for it. The guard's separate process-group cleanup does not own this
unrelated child.

If the owner exits normally after an immediately failing guard, its destructor does clean the sleep; that is not the
reported trigger. The leak ends when the 300-second sleep ends. The same-run numeric-PID acceptance in
`SPEC_impl.md:4902–4910` at pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7` permits the existing cleanup's
identity assumption, not missing ownership registration. The queued browser-supervisor publication gap concerns another
fixture and is not duplicate coverage. No exact Planned, BUGS, queue, ledger or FILTER coverage was found. Possible
cover: none. Proposed drop: none. Register parent cleanup ownership before waiting on independent guard readiness.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g016/f3`, independently corroborated by `g048`.
