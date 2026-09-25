# setup can fail after writing only the supervisor unit

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A setup run that fails partway can leave the helm and supervisor services configured with different state directories,
so after the next reboot or reload the helm cannot find its supervisor.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F6 / COR-SETUP-PARTIAL-UNIT-WRITE`, tagged **possible**. Anchors and title: `crates/farhelm/src/setup.rs:639`,
`crates/farhelm/src/setup.rs:660`, `crates/farhelm/src/setup.rs:664`, `crates/farhelm/src/setup.rs:666` — helm setup can
fail after rewriting only the supervisor unit, leaving a mismatched helm/supervisor pair on disk

`install()` in setup.rs checks ownership of both unit files up front, under the setup lock, precisely so a refusal on
the second unit cannot leave the first one replaced. But the per-unit loop at L639 still interleaves fallible steps with
writes. For each unit whose text changed it: asks systemd whether the unit is running (`unit_is_active`, L660, which
errors on any exit status other than 0, 3 or 4); if running, writes a restart marker (L664); then publishes the new file
(`write_unit`, L666). The supervisor unit is processed first. So if the helm unit's `is-active` query, its marker write,
or its own `write_unit` fails, setup returns an error with the new supervisor file already on disk next to the old helm
file, and without having run `daemon-reload`.

This only produces a real mismatch when the run changed something both units pin, such as `--state-dir`, the
`XDG_STATE_HOME`-derived default, or the binary path. In that case the two files now name different state directories or
binaries, and the next `daemon-reload`, login or reboot starts a supervisor and helm that cannot find each other.

The fix is to run every fallible pre-write step for both units (both `is-active` queries and both marker writes) before
the first `write_unit`, and then publish the two files back to back. Alternatively, if the second write fails, restore
the first unit's previous text, which setup already holds in `existing`.

Restater note: rerunning setup converges the pair (the supervisor's restart marker survives too), so the failure is
recoverable. It is harmful only if the operator does not rerun after the error and the machine reloads first.
`is-active` returning an unexpected status between two calls a few milliseconds apart is rare.
