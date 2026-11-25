# Agent container-environment launch skips the place-host connection recheck after approval

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

When an agent asks to launch a session in a new container environment on a named host and the user approves the card,
Farhelm can create that container on a different machine than the card named if that host was retargeted or
re-identified while the card waited. Ordinary agent creates guard against exactly this; the environment-launch path does
not. The card shows the host's display name, which a retarget need not change, so the user cannot tell the machine
behind it changed.

## Details

**highest.** Source inspection; commissioner re-read every load-bearing link (below). Security consequence: execution on
a machine other than the one the approval named. The route is established by code reading — missing pin, fresh identity
read at create time, and an idle retargetable place host during the card wait; rarity of a concurrent retarget does not
lower the bucket. SPEC_impl.md "Permission prompts for agent actions" (line 9739 at this commit): "A launch is also
refused if the target host's connection changed while the card waited, since the approval was for the machine the card
named."

At the reviewed commit:

1. `crates/farhelm-helm/src/agent_requests.rs:4548-4747` (`launch_environment_for_agent`) builds
   `ApprovalAction::LaunchEnvironment { place, place_name, .. }` with `place = Place::PodmanHost(host_id)` for a
   container, calls `approved(...)` (which rechecks only the requesting origin's liveness,
   `agent_requests.rs:2981-3006`), and after the Allow rechecks only `plan.source_owner` for a clone (4660-4672). It
   never captures the place host's connection incarnation before the card nor rechecks it after. Contrast
   `approve_launch` (`agent_requests.rs:2722-2724` captures `target_before`; `2772-2781` refuses if it changed), which
   ordinary create/clone/spawn use.
2. The launch task then fresh-reads the place: `provisioning/tensorlake_add.rs:338-410` (`start_session_environment`)
   rechecks availability after the Allow, and `environment/podman.rs:1298-1311` reads `places.fresh(...place.host)` at
   create time when the row has no recorded identity, using the current identity.
3. `environment/podman_probe.rs:675-715`: `fresh` joins/starts a refresh and then `confirm_cached_admission`, whose own
   doc says "An explicit route edit releases the claim in storage. Even if the route later returns to the same text,
   only a new probe can claim it." So a GUI retarget during the wait releases the claim, the fresh probe claims the
   replacement machine, and creation proceeds there — honored, not refused.
4. During the card wait the place host is idle: the provisioning task starts only after the Allow via `run_owned`
   (`agent_requests.rs:4685-4692`), so a GUI retarget lands freely in the window. The ordinary-create guard's existence
   shows the maintainers treat this window as reachable.

Not covered by the existing queue item `trailing-agent-create-drops-target-incarnation-after-approval.md`: that item is
the session-create dispatch path (`approve_launch` captures the incarnation, then `dispatch_agent_create` drops it for
non-folder-pinned launches). This finding is the environment-launch path, which never captures the place host's
incarnation at all and does not call `approve_launch`; the fix for the queued item does not close this gap. Related
highest items on the same principle: `trailing-agent-stop-does-not-pin-target-connection-across-approval.md`.

Verify: read the two paths side by side; or live — `farhelm agent create --recipe R --container-host X`, leave the card
waiting, retarget X in the GUI, Allow, observe the container on the replacement machine. Fix shape: for a `PodmanHost`
place, capture the place host's connection incarnation/install identity before the card and refuse after the Allow if it
changed, mirroring `approve_launch`'s `target_before`/post-check (a Tensorlake-account place has no host connection to
pin).

Reviewer metadata (area 2 chunk B, agent_requests second half): suggested bucket: highest; confidence: confirmed at code
level, end-to-end live retarget not reproduced (read-only review); proposed_drop: none; possible_cover: SPEC_impl "Agent
requests and environments" says the agent path mirrors the launcher REST route but does not accept a retarget during the
wait; the "Permission prompts" rule above is the requirement this path fails. Commissioner validation: links 1-4
confirmed at the cited lines; coverage searched (queue incl. the sibling incarnation item, TODO.md, BUGS.md,
TRIAGE_OUTCOMES.md, plans/queue) — no match for this path.
