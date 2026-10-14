# Detached planners can permanently consume every planning slot

Reviewed commit: 07b7906519301d115802a8b388a43a3f37bb6663

## TLDR

Repeated update or removal planning can leave adding, updating, and removing hosts stuck until the helm restarts, even
after the pages close.

## Details

Suggested bucket: `high`. Confidence: likely; unverified premise is a configured SSH helper that escapes the process
group and retains stderr on a successful-hello or timeout path. No runtime reproduction.. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

crates/farhelm-helm/src/provisioning/service.rs:990–1002 and :1183–1201 now detach planners while retaining a permit
from the shared four-slot semaphore (:49–51). The probe’s successful-hello and timeout paths call stop_probe
(backend.rs:1903,2112), which kills the child’s group then awaits stderr without a deadline (:1845–1850).
drain_probe_stderr waits for EOF indefinitely (:1483–1495). A configured local SSH helper that leaves the process group
and retains stderr therefore prevents completion even after the handshake deadline. Four abandoned attempts exhaust the
shared planning slots, including discovery (:483–487), affecting unrelated hosts. Before this commit, dropping these
request tasks released their permits; the underlying drain defect pre-existed. Apply the bounded stderr join already
used by the probe’s exited-child branch (:2071–2077) to all cleanup paths.

Possible cover: BUGS.md:80–101, heading A host install or update can hang until the helm restarts if a local ssh helper
keeps its output open, shares the inherited-pipe mechanism but describes execution hanging on the affected host. It does
not explicitly cover abandoned planners exhausting the global planning semaphore. TRIAGE_OUTCOMES.md headings
probe-cancellation-leaves-helper-processes.md and uninstall-cancel.md address helpers remaining after cancellation, not
this permanent cross-host planning blockage.

Current-main status: Persists at pinned main: service.rs:983–1012,1178–1204 retain detached ownership and shared
permits; backend.rs:1845–1850 still joins stderr without a deadline. No fixing commit found.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.
