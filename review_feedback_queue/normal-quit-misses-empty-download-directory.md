# Normal Quit can miss the last owner's empty download directory

Reviewed commit: ca1baa54e2d225723a97e1381bf741cc235df376

## TLDR

A narrow normal-Quit race can leave an empty hidden folder in Downloads. Later launches do not reclaim it. No partial
file, completed download or original host file is lost by the inspected mechanism.

## Details

`crates/farhelm-helm/src/downloads.rs:28–35` discovers cleanup targets by upgrading weak directory registrations.
`SaveFiles` drops its stream before its directory Arc (`308–315`); cancellation schedules destruction on the blocking
pool (`378–382`). Weak upgrade fails once the last strong count reaches zero, before `TempDir::drop` necessarily
finishes directory removal (pinned `tempfile` 3.27.0 `src/dir/mod.rs:498–502`). If hard exit interrupts that destructor,
the hook skips the now-unupgradeable entry. Later saves prune dead registrations (`downloads.rs:338`), rather than
reclaiming their paths. There is no fixed cumulative cross-launch bound.

The staged file is removed before directory ownership is released (`crates/farhelm-supervisor/src/files.rs:715–727`);
successful publication removes the temporary name first (`649–675`). Thus this is empty-directory debris on a healthy
filesystem. Cleanup ownership would need to remain discoverable until removal finishes; this record does not authorize a
startup sweep or waiting for unfinished transfers.

Suggested bucket: **other**. Confidence: **likely**, with static ownership permitting the race but scheduling incidence
and native Mac behavior unmeasured. Proposed drop: none.

Possible cover: final `SPEC_impl.md:77–82`, “Leftover files,” allows bounded debris, rare explicit-action growth or
later reclamation. Normal Quit is ordinary; a rare schedule does not clearly meet the rare-action condition.
Crash/force-quit acceptance at `84–107` and `TRIAGE_OUTCOMES.md:16567–16583` has a different trigger. The existing
download-exception wording record is separate. No exact Planned, BUGS, queue or filter coverage was found; persistent
directories are not a self-correcting display glitch. Provenance: `transfers-contracts-1/f1`, reconciled with the
implementation sibling in the trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime
reproduction was performed.

Original finding IDs: `transfers-contracts-1/f1`.
