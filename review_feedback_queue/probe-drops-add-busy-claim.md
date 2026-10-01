# A host probe can let a second install start while one is running

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When the user reruns a failed host setup, a probe of the same destination at the wrong moment can clear the "setup in
progress" marker, so a second setup or update is accepted and runs right after the first, with each one's completion
clearing the other's marker.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F18 / COR-BUSY-CLAIM`, tagged **possible**. Anchor and title:
`crates/farhelm-helm/src/provisioning/service.rs:1243` — a host probe can drop the "busy" claim a confirmed ADD just
took, letting a second install run.

When the helm installs Farhelm on a new host (ADD) or upgrades one (UPDATE), it marks the host busy in an in-memory set.
That busy set is the only thing that refuses a second ADD or UPDATE for the same host while one is running. Confirming
an ADD for a destination that already has a registry row goes through `claim_register_and_start`
(`crates/farhelm-helm/src/provisioning/service.rs:638-671`). That function marks the host busy (line 640), re-registers
the row (several database awaits), and then calls `start_run` with "already claimed", so `start_run` does not mark it
again (lines 913-916). Until `start_run` installs the new run's progress view, the helm still shows the host's previous
run. When the user is re-running a failed ADD, that previous run is a failed ADD.

Probing a destination, which the hosts panel does to see whether a supervisor already answers there, can run in the same
window. If the probe finds a live supervisor, it registers the host and then calls `resolve_failed_add_discovery` (line
371). That function exists so that a failed ADD whose supervisor later turns out to be fine gets marked completed. When
it sees the retained failed ADD, it marks it completed and unconditionally removes the host from the busy set (line
1243). It never checks whether someone else now holds that claim.

Once the claim is gone, a second ADD or UPDATE confirmation is accepted. It waits on the per-host provisioning lock,
overwrites the progress view, and runs a second install straight after the first. Each run clears the busy flag the
other one relies on when it finishes. This is marked possible because the probe's handshake has to complete inside the
short gap between registration and run start. The path was confirmed by reading the code, but the timing was not
reproduced. Either fix closes the gap. One is to have `resolve_failed_add_discovery` leave the busy set alone, since the
failure paths that end a run already release it. The other is to have `claim_register_and_start` install a "running"
placeholder view at the same moment it takes the claim, so a probe never sees the old failed ADD.
