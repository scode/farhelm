# A refused create retry strands a credential-bearing launch spec on disk

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

An ordinary failed-and-retried session creation can leave a file containing the agent's full command line, credentials
included, sitting on disk until the supervisor next restarts — past the lifetime of the session it belonged to.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite (correctness-general p2).
Coordinator confirmed the evidence gap, the missing cleanup, and the credential content.

When a keyed-create retry is refused by validation (e.g. the working directory vanished since the crashed attempt), the
`Retry` arm of `record_refused_create` deletes the stranded `Launching` row and settles the intent `Failed`
(`crates/farhelm-supervisor/src/service/core.rs:6565-6580`), then drops the map entry — but never removes that launch's
on-disk artifacts. The crash window is real: an attempt that published its spec but died before the shim ran leaves a
spec with none of the four launch-evidence sources set, because `reserved_launch_evidence` (core.rs:6338-6402) consults
the durable row, the launch sentinel, the cgroup scope, and tmux — the spec file is not among them. So the retry
legitimately reaches validation, gets refused, and the row deletion orphans the file. The spec holds the agent's full
command line, credentials included, and the codebase treats it as toxic everywhere else: delete removes every
generation's launch files fail-closed ("a missed generation is a credential leak, not untidiness", teardown.rs:804-816),
archive does the same, and the create path's own confirmed-absence rollback removes the spec explicitly (core.rs:7201).
The only remaining sweeper is the startup-only `sweep_launch_dir` (core.rs:5578), which removes only orphaned specs — so
on a long-lived supervisor the file sits until the next restart, and any non-spec debris is never swept at all.

Suggested fix: after the row removal commits in the `Retry` arm, remove that launch's artifacts for the reservation's
session id and generation (the same per-generation cleanup the delete and relaunch-unwind paths use), failing the
refusal closed or logging loudly if the removal itself fails.
