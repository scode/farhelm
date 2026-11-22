# Agent re-adoption can discard cleanup identities without confirming the host ended

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

Recovery can drop the main terminal's saved host and child identities while the recorded host may still run, weakening
later Delete cleanup.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_handover.rs:1693`.

hosts/client.rs:640-646,1221-1239 turns an unreachable socket with a discoverable process into NotAccepting, and
hosts/terminal.rs:617 and host_handover.rs:1453-1456,1526-1530,1553-1563 schedule re-adoption for that unavailable
terminal. The retry uses a directory candidate scan (hosts/registry.rs:519-541), not a direct connect or adopted-process
check when the pathname is absent. host_handover.rs:1245-1256 returns Absent for that missing pathname, and :1680-1694
drops the old handle and both identities. This is a mismatch between an unavailable connection with positive process
evidence and the retry's weaker absence result, without requiring the later Delete's process lookup to fail. Delete can
rediscover and confirm a recorded host via the process lookup (host_launch.rs:981-1008.
Session_hosts.rs:395-430,744-767), but with no answer it cannot recover the old child's pinned root
(teardown.rs:319-345. Session_hosts.rs:510-547). Existing assertions at core/tests/host_terminals.rs:2242-2289 keep the
child pin on an unavailable retry and :2897 and :3037 keep tab peer evidence on a gone retry. Neither asserts
preservation for this agent Absent path. SPEC_impl.md:2159-2172 requires Delete to use already-pinned identities on a
held terminal. SPEC_impl.md:2232-2238 allows a missing socket for startup when pinned evidence is unavailable;
:2264-2288 distinguishes in-process re-adoption and explicitly keeps pins for tabs. Preserve the held agent's pins for
cleanup and keep connection availability independent of process confirmation. Bucket: other for established loss of
identity and weaker Delete evidence. Exact remaining gaps are an unscoped, live child tree absent from marker/other
roots whose subsequent cleanup is missed, and a live host that the recorded process lookup fails to rediscover. Neither
consequence was observed or used to raise the bucket. Confidence: source-confirmed identity loss and reachable retry
branch. No runtime observation. for the identity-loss finding. Drop the stronger claim of a live-host leak because the
durable record does supply a fresh, independent process lookup. Possible coverage: recovery_contract and lifecycle_unit.
More precise boundary: terminal.rs:1192-1203 preserves Connect(NotAccepting) and :612-618 marks it retryable. A later
Restart with no held handle scans the record instead of skipping it. Host_restart.rs:588-635 refuses a recorded socket
that still returns NoAnswer. Do not infer an overlapping Restart from r10. Preserve trustworthy identity for
unconditional cleanup without relaxing the guard on an unreadable current program. The pinned registry walk at
hosts/registry.rs:718-762 confirms the only filter selects existing directory entries and never creates a candidate for
a missing name. HostTerminal::unavailable_again at hosts/terminal.rs:2008-2024 explicitly documents that a later missing
socket cannot prove the original process ended and retains the two pins. The agent Absent arm bypasses that protection.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r10`.
