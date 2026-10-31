# The creator fence can end before Podman allocation

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A terminal interrupt can release the creation fence while Podman still registers a container, leaving late storage after
Stop.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Matching Podman source establishes deferred SIGINT through registration while the creator's CLOEXEC lock can end.
Helper-scope reaping is not a second storage removal. The narrow Ctrl-C route is source-confirmed; a separate
creator-alone fatal-death history still has an unestablished ordinary fatal producer, although its conditional
late-idle-container mechanism is complete. Live keeper survival, requested agent execution, and user-work loss are not
assumed. No signal or process experiment ran.

Source locations: `crates/farhelm-supervisor/src/container.rs:567-588,619-639`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d, introduced in assigned 81d6ddbcfaf8ba1bd4e761be9c579e6cea53ea8f
container.rs:411-446. User-facing TLDR: interrupting creation and then deleting an unscoped session can leave a
container in Podman with no Farhelm session left to remove it. Confidence now confirmed by source composition for Podman
4.9.3, with practical frequency unmeasured; no vendor code or experiment ran. Farhelm's disposable creator ignores only
SIGHUP and owns a CLOEXEC flock; its Podman child does not inherit the fence (launch.rs:2254-2290). Ctrl-C is an actual
terminal input producer (terminal.js:4924-4980; tmux/input.rs:257-308), and neither direct creator spawn nor systemd
scope wrapper creates a new terminal process group. Podman source at immutable 8d2b55ddde1bc81f43d018dfc1ac027c06b26a7f
establishes the previously missing child-continuation premise: libpod/runtime.go:214-226 registers SIGINT/SIGTERM
shutdown; libpod/shutdown/handler.go:35-79 waits for an inhibitor before invoking os.Exit; runtime_ctr.go:303-305
inhibits shutdown before lock/storage allocation and retains inhibition through state.AddContainer at566-588. SIGINT
therefore terminates the Rust lock owner immediately but can leave Podman creating. A valid schedule pauses the child in
that pre-storage/pre-registration inhibited interval, lets unscoped Delete acquire/revoke/remove/observe absence, and
resumes the child after the row is gone. ContainerExists defaults to libpod lookup (ABI containers.go:160-175;
runtime_ctr.go:1218-1224; SQLite state:561-615 or Bolt state:680-721), whose registration has not occurred. Runtime
init's aliveLock ends before these operations (runtime.go:557-656); there is no creation-wide observation fence.
ContainerRm also tries external storage, but a child still before allocation has no such record (ABI
containers.go:405-438). Podman can then commit a configured container before deferred SIGINT exits it; a running keeper
is not required to establish the orphan. Scoped Delete has an additional exact-unit sweep (teardown.rs:418-435,454-471),
so the strongest confirmed orphan route is the supported unscoped fallback. Stop keeps its row and can be retried; do
not conflate that with Delete losing ownership. Matching package metadata reads podman 4.9.3+ds1-1ubuntu0.2; inspected
Ubuntu patch manifest and patch target paths do not modify the signal/creation/state sources cited. Tests cover SIGHUP,
revoked/superseded tickets, normal scoped/unscoped Stop/Delete and rollback, not this SIGINT lifetime. Suggested bucket
other: retained Podman resources and missing cleanup handle; no established highest loss. Coverage sources
searched/read: SPEC.md:1505-1528,3582-3664; SPEC_impl.md:5756-5837; TODO Planned empty; plans/queue/INDEX empty; BUGS
complete; feedback INDEX complete; targeted ledger searches and nearby supervisor-signal/rollback outcomes. No exact
cover. possible_cover: SPEC.md's exact supervisor-crash/two-fault exception does not directly describe ordinary Ctrl-C
terminating this separate creator while the supervisor and filesystem remain healthy. proposed_drop:null. A creator-only
fatal SIGKILL/OOM variant remains weaker as a producer and is not needed for the confirmed SIGINT composition.
Additional cancellation check: pkg/rootless/rootless_linux.go:145-165,393-431 proxies signals to its reexec child and
waits; it does not terminate because Farhelm creator died. rootless_linux.c:830-925 sets parent-death SIGTERM for the
namespaced child relative to its Podman parent, which remains in the proxy wait, rather than relative to the Rust
creator. Even a SIGTERM delivered to the namespaced child uses the same shutdown inhibitor. These inspected rootless
paths do not refute continuation; no unexamined claim that Podman never uses PDEATHSIG is made.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g114/f1`, `g116/f1`, `g019/f5`, `a007/e3`,
`a004/x7`, `a021/d2`, `a024/g019-prose-creator`, `a032/g113 delayed-creator hypothesis`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
