# Delete mistakes lost tab markers for complete cleanup evidence

Reviewed commit: 5f235d92f7184cdf949cf9d45e86dc19e71ff4df

## TLDR

If an old terminal server disappears during Delete's tab discovery, Delete can succeed and remove the session while
background jobs from an old unscoped tab keep running.

## Details

Suggested bucket: **high**. Confidence: **source-confirmed conditional interleaving**; no runtime occurrence or
frequency was measured. No additional security or user-work-loss consequence is established.

`crates/farhelm-supervisor/src/tmux.rs:4887-4940` reads pane facts and window markers separately. If the server is
definitively gone at the second read, it returns the old fact map successfully with `tab: None`; those defaults came
from `tmux/control_codec.rs:182-195`. `service/terminals.rs:650-659` omits these rows from tab discovery. The new
completeness predicate in `service/core.rs:15165-15168` nevertheless treats the matching session's fact row as complete
discovery. No tab reaches the per-tab Unknown-selection fallback.

A migrated host agent can remain alive in a known scope while an older unscoped tmux tab disappears with the private
server. A detached tab job with readable markers can survive its shell. `service/teardown.rs:478-501` then receives no
tab selection and a true completeness flag; its unit globs at `513-531` cannot recover an unscoped job. A captured shell
root is already gone or becomes a zombie, and `service/sweep.rs:1542-1549` can return after collecting only the live
agent scope. `SessionHosts` excludes the current runtime's host record from stray-host fallback
(`service/session_hosts.rs:253-262`), so that ordinary record does not rescue the omission. Final private-server absence
is tolerated too (`tmux.rs:3454-3474`).

At this revision, `SPEC_impl.md:5067-5074` says incomplete historical tab discovery retains the sweep and a successful
empty listing does not prove containment. `SPEC.md:1852-1859` states the same requirement. This needs one server
disappearance between two reads, without a supervisor crash or unsupported direct private-server manipulation. It is
separate from nested user tmux migration: preserving nested-server history would not restore missing tab selection.

Carry a lost-marker/completeness result through discovery, or force fallback rather than interpreting stale fact
presence as a complete account. Preserve the existing identity checks for tabs that are actually discovered.

Possible coverage: no exact existing queue/ledger item, accepted specification, known bug or unlanded plan covers this
route; Planned was empty. The landed plan's ordinary empty-listing repair does not cover facts surviving a lost marker
read. The success claim and surviving work exclude the rare-glitch filter.

`TRIAGE_OUTCOMES.md`, "startup-probe-false-tmux-absence.md", records the maintainer's broader preference to discard
old-terminal findings unless very serious. This finding requires old tabs beside a migrated host agent. Whether
successful Delete permanently stranding their jobs meets that exception is unresolved; retain this possible coverage for
triage rather than silently interpreting the severity boundary. The three specific discarded findings concern unwarned
termination, not this omission of cleanup.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `cleanup-callers/f2`, `cleanup-scope/f6`, `cleanup-sweep/l6`.
All are the same defect, with independent source checks and original confidence retained.
