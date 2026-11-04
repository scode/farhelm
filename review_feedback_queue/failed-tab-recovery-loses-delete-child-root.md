# Failed tab recovery loses the child identity Delete needs

Reviewed commit: 82fe09bf28f7050b582a3f249f5f056dcf1f0119

## TLDR

On macOS, deleting a session after a failed terminal-tab recovery can leave its foreground program running untracked,
even though Farhelm previously knew that program's identity. The conditional source route is established under the
repository's documented macOS behavior; this exact sequence has not been reproduced at runtime.

## Details

Suggested bucket: **highest**, for losing ownership of the session's foreground process during successful deletion.
Confidence: **source-confirmed conditional mechanism**, relying on the documented macOS 26 platform-binary environment
behavior. Proposed drop: none. No frequency or observed end-to-end occurrence is claimed.

An unavailable runtime retains its pinned child identity at `crates/farhelm-supervisor/src/hosts/terminal.rs:737–767`. A
failed tab retry replaces it through `service/core/host_tabs.rs:240–243` with `old.host.unavailable_again`. That
constructor at `terminal.rs:1983–2009` keeps the adopted host process, but constructs an ended placeholder with
`child_pid = 0` and `child_start = None`. It discards the already pinned child even though another failed connection has
not disproved that identity.

Delete filters placeholder tabs from its held-runtime list at `service/session_hosts.rs:243–247`, then settles their
launch records. If the host is still unreadable, it becomes `RecordedHost::Unreadable` at 391–434. `sweep_roots` at
510–547 takes saved child roots from retained tab runtimes and repins children only for readable `RecordedHost::Ours`
entries. Neither supplies this tab's former root. The actual caller at `service/teardown.rs:397–430,557–592` sweeps
those roots before ending hosts and removing the session; it has no independent tab-child recovery or later sweep.

This matters on macOS without a systemd scope. `procs.rs:239–248` documents an observation on macOS 26.5.1: Apple
platform binaries such as `/bin/sleep` expose argv without their environment even to a same-user parent.
`service/sweep.rs:295–304` explains why their process-tree root is needed. A tab foreground command such as
`trap '' HUP; exec /bin/sleep 10000` therefore supplies a concrete marker-less, hangup-ignoring foreground program.
Without its root, the tree expansion at `sweep.rs:588–620` does not find it. Ending the unreadable host by its pinned
peer at `session_hosts.rs:761–767` closes its pty but need not end a program ignoring HUP; the host's spawn tests
document that portability premise. No such command or process experiment ran in this review.

SPEC.md, "Lifecycle operations", 1526–1530 at this revision preserves cleanup of foreground processes. Its
environment-hiding exception covers detached descendants, not a foreground child whose identity this supervisor already
pinned. The implementation's no-root residual for unreadable launch records does not explicitly authorize discarding a
known runtime root. One continuous unreadable-host episode suffices; no crash, second independent fault, or deliberate
private-socket interference is required. A readable host at Delete repins the child and avoids the bug; a usable scope
also covers it.

Source checks followed failed-retry construction, placeholder filtering, record classification, root collection, the
Delete caller, host shutdown, and the documented platform limitation. The affected route remains through
c5ae8548439cc5bfa33c11ef7ce8da6129d8072a. Targeted searches of SPEC.md, SPEC_impl.md, empty TODO Planned, BUGS.md,
unlanded plans, the queue index and TRIAGE_OUTCOMES.md found no matching acceptance, scheduled repair or existing
finding. `readopt-unavailable-hosts` is landed. No FILTER.md rule covers a persistent loss of process ownership after
successful Delete.

Preserve pinned child provenance across failed retries and make Delete validate/use it as it does a runtime's root,
rather than keeping only the host identity or trusting a bare PID. The separate suspected loss of the host process
identity was source-refuted; it is not this child-root defect.

Provenance: review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, original reviewer `g049/s10`, findings event 23. This was
a source review. The repository's earlier platform observation was inspected as documented evidence, not rerun or
independently authenticated here.

### The same provenance loss during agent recovery

The failed agent-recovery path added at `6bd019f5359ed2ec4c69b13ab75b4840626f842f` has the same ended-placeholder
construction. Its `service/core/host_handover.rs:1495–1499,1527–1529` calls `HostTerminal::unavailable`, rather than the
tab path's `unavailable_again`; `terminal.rs:1961–1978` also clears `child_pid` and `child_start`. Replacing the session
entry after that failed midlife retry loses the previously pinned child root. Its placeholder is not held as a live
runtime during Delete, and a still-unreadable recorded host cannot repin the child. The documented macOS
foreground-child route above therefore also applies to the agent, without a second fault or a deliberate socket
replacement. This is the same child-provenance defect, not another item.

Confidence remains source-confirmed and conditional; no end-to-end agent sequence was run. Additional provenance:
`g032/g032-d7` event 21 (including the constructor correction) and `g048/g048-d9` event 15 in the same review run.
Preserve both original reviewer identities.
