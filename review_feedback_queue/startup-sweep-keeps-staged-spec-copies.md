# The startup sweep keeps staged launch-spec copies

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Rarely, a hidden duplicate of the command line and credentials stays until the session is deleted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F23 / COR-STAGED-SPEC-KEPT`, tagged **definite**. Anchors and title: `service/launch_artifacts.rs:392-399`,
`files.rs:408-416` — The startup sweep keeps staged `.tmp-` copies of launch specs for live sessions

The supervisor writes launch specs in stages so that no reader ever sees a half-written file (`files::write_staged`,
`Tier::AtomicPublication`). It first writes a hidden temporary file `.<id>.<generation>.json.tmp-<uuid>` (mode 0600),
then hard-links it to the real name, then unlinks the temporary name and ignores the result (`files.rs:408-416`). The
comment there says a failed unlink is "debris for the launch-dir sweep to catch later". A supervisor crash between the
link and the unlink leaves the same debris, as does a crash while the temporary copy is still being written. Each such
file is a full or partial second copy of the command line and session token.

The startup sweep does not catch it for live sessions. In `sweep_launch_dir` (`service/launch_artifacts.rs:392-399`), a
staged file is kept whenever its parsed session id belongs to an existing session. That rule was written for the
`.status` sentinel, which the shim stages the same way (by rename): a shim still running after a supervisor restart may
have a legitimate in-flight staged sentinel. Staged `.json` files, however, are written only by the supervisor. The
sweep runs after the new supervisor has proven it is the state directory's sole owner and before it launches anything.
Any staged `.json` present at that moment therefore belongs to a dead supervisor and can never be finished or read.
Keeping it only preserves a hidden duplicate of the credentials until the session is deleted, which Delete's
`remove_launch_artifacts_for_session` does handle.

The trigger is rare: a failed unlink or a crash in a narrow window. The mechanism, though, is certain. The fix is small:
at startup, always delete staged `.json` files, and keep only staged `.status` files whose owning session exists. This
finding differs from F20 to F22 in that the leaked file is the hidden staging copy, not a published spec.
