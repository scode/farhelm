# A shared rootless pause can inherit a session marker

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The rootless container infrastructure process can inherit a session marker and be selected for unnecessary termination
or recreation.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Matching catatonit, Podman and Linux process-visibility source supports inheritance and same-owner environment
readability for the inspected ordinary namespace mapping. Catatonit pause mode does not forward termination to live
container work, and Podman can rejoin surviving conmon namespaces. No user-process loss or security consequence was
established. Restrictive host profiles or different deployed bytes narrow applicability; actual interleaving was not
measured.

Source locations:
`crates/farhelm-supervisor/src/container.rs:314-325; crates/farhelm-supervisor/src/procs.rs:876-879,968-969`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery-phase refinement, original g114 marked-pause prose candidate, reviewed
d6645f771bee8ed03a66486ef8755834937d759d. Available catatonit and process-visibility premises have now received focused
primary-source checks. Upstream catatonit v0.1.7 main 414-581 does not clear inherited FARHELM environment; -P spawns no
child and exits on TERM/INT without forwarding. Podman 4.9.3 rootless_linux.c 792-925 joins the existing rootless
user/mount namespaces, sets uid/gid0 and creates pause with inherited environment. On Linux v6.8, security/commoncap.c
67-100 gives the namespace owner in its parent namespace capabilities in the child; 135-156 permits ptrace access with
that namespace CAP_SYS_PTRACE. kernel/ptrace.c 268-346 applies identity/capability/dumpability checks; fs/proc/base.c
937-1009 uses PTRACE_MODE_READ for environ and 1814-1922 maps proc ownership to the task's effective/root-in-userns uid.
The rootless uid0 mapping is the host account, compatible with Farhelm's same-euid proc filter (procs.rs 876-879).
yama_lsm.c 355-393 adds restrictions only for PTRACE_MODE_ATTACH, not this READ. Thus generic 'catatonit may scrub' or
'different namespace necessarily hides environ' are no longer missing source premises for the inspected upstream
versions and ordinary owner mapping; do not claim source unavailable. A restrictive host LSM/profile or different
deployed bytes can narrow admission, but absence of such a profile is a supported source path. Conditional
marked-pause/reaping mechanism is now source-supported beyond the prior categorical no-producer objection. Remaining
consequence is infrastructure-marker contamination and potentially unnecessary pause termination/recreation: Podman
SetupRootless explicitly rejoins surviving conmon namespaces, and inspected catatonit -P does not signal live container
work. No user-data/process-work-loss or security consequence was established. Suggested other, confidence
source-confirmed conditional inheritance/readability for inspected upstream versions, actual deployed interleaving
unmeasured. proposed_drop null for narrow correctness question; possible_cover none exact, Near-term TODO203-235 is not
Planned coverage. No broader implicit environment/proxy conclusion (a016 owns that). Source fetch failure for commoncap
was resolved by focused alternate immutable tag URL; no runtime experiment occurred.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a007/e4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
