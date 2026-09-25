# setup ignores an unreadable restart marker

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In rare filesystem error cases, `farhelm helm setup` can report success while the helm or supervisor keeps running its
old configuration until the next manual restart.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F16 / COR-RESTART-MARKER-EXISTS`, tagged **possible**. Anchors and title: `crates/farhelm/src/setup.rs:694` — helm
setup treats an unreadable restart marker as "no restart owed" and reports success

When setup replaces the file of a unit that is running, it first writes a restart marker, `.<unit>.restart-pending`
beside the unit, and deletes it only after the restart succeeds. This way a run that fails partway leaves the pending
restart for the next run to perform. The marker is read back at L694 with `Path::exists()`, which returns `false` for
any metadata error (permission denied, I/O error, not-a-directory), not just "file not found". In that case setup skips
the restart, leaves the marker, and reports success, while the running unit keeps its old configuration. Writing and
clearing the marker both propagate errors; only this read swallows one.

That is exactly the failure the module's own docs describe as the reason markers exist: unit replaced, running process
not restarted, success reported. Suggested change: use `try_exists()` and propagate the error, as `unit_is_active` does
for its unclear case.

Restater note: the triggers are rare. SPEC.md "Healthy local filesystems" accepts that I/O errors may cause failures or
halt progress, so EIO alone is within accepted territory. But that decision covers failing or stalling, not reporting
false success, so the silent-success outcome is not clearly covered by it.
