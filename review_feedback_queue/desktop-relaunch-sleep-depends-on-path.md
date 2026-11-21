# The update relaunch helper can skip its wait on the restored PATH

Reviewed commit: 5f97d10518d6b2e598ecab8e8387e0f78f6e68d8

## TLDR

Restart to update can close the app without reopening it if the original PATH cannot find `sleep`.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed source defect: updater.rs:339-346 runs sleep by name, ignores failure, and increments its nominal tenth-second
counter anyway. At :388-393 the patch puts the original PATH on the whole shell helper and silences its stderr.
Successful spawn tells the GUI to quit (updater.rs:290-305 and GUI desktop/updates.rs:142-153). If the sleep lookup
fails and the old PID remains visible until the count is exhausted, the helper exits without invoking the absolute
opener. That consequence depends on the inherited PATH and relative scheduling. No quit latency, recurrence rate, or
macOS /bin/sh behavior when PATH is absent was measured or inferred. The wait violation follows from the source
independently of whether the app usually exits first. Bundle.rs:222-243 has the same dependency, but its uncapped loop
can spin until the old PID goes away. Lib.rs:1838-1866 tests only Command environment overrides, and
updater.rs:1917-1953 uses the ambient test PATH. SPEC.md:794-797 promises a reopen, and SPEC_impl.md:7504-7506 and
:7764-7771 require restoration for the opener and a roughly one-minute waiting bound. Synthesis across commits: both
unqualified sleep calls already existed at b6e33332cd8beab675da31b04509fbbee70be8bb (updater.rs:343, bundle.rs:226),
where the app had not modified PATH. 23ae2b9850c539d6d90284dd48322a98af7d78de let the helper inherit the login PATH on
probe success, incidentally masking the problem where only that PATH finds sleep.
5f97d10518d6b2e598ecab8e8387e0f78f6e68d8 re-exposes the old dependency while restoring the original PATH for the new
app. This is a relaunch integration defect at the restore point, with a pre-existing failure mode at the review base.
Synthesis with path-notices/f10: when no next app opens, no new probe observation is available for notice
reconciliation. Estimated bucket: high for a conditional failed restart. Possible_cover: none identified. No later
helper fix through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Source and contract only.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f6`.
