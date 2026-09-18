# Startup sweep can delete a live shim's staged sentinel file

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In a narrow timing window around a supervisor restart, a genuinely failed agent launch can be misreported as a normal
exit instead of an error — the evidence file proving it failed is deleted before it is finished being written.

## Details

Source: pre-pr-review-swarm, area supervisor-launch, 2026-09-17. Confidence: possible. The mechanism is fully verified;
the window is narrow (one rename step vs. one startup sweep) and the final misclassification step was traced from
reading the code rather than demonstrated end to end.

When Farhelm starts an agent it writes a launch spec, and the launch shim reads the spec, unlinks it, and execs the
agent — recording any pre-exec failure in a `.status` sentinel so a later step can tell launch failure apart from a
plain exit (a deliberate durability promise: failures must read back as errors, never silently downgrade). The shim
publishes the sentinel via `files::write_durable_sync` (stage to a `.tmp-` file, then rename), called at
`crates/farhelm-supervisor/src/launch.rs:855`.

`sweep_launch_dir` runs once at supervisor startup
(`crates/farhelm-supervisor/src/service/launch_artifacts.rs:334-382`). For finished specs it checks ownership against
the reloaded session map (launch_artifacts.rs:359-372) precisely because a supervisor restart does not kill tmux (docs
at 317-322). But staged temp files are deleted unconditionally (`is_staged_temp_name` → `true`, line 357-358; the
predicate is `name.contains(".tmp-")`, files.rs:291-293). So a pre-restart launch sitting inside the shim's stage→rename
window when the fresh supervisor's sweep runs loses its staged file; the rename fails with ENOENT; no sentinel is ever
published; the spec is already gone — and the failure later reads back as `Exited`.

Suggested fix: attribute staged files before deleting them, the way teardown already does — only remove a staged temp
file when `staged_name_belongs_to` maps it to a session id absent from `sessions` (or its stem is not a launch name at
all). A shim can only be mid-write for a session in the reloaded map, so orphans are still swept while live ones are
left for their writer to publish.
