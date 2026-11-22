# Tab close discards retry state when its final process sweep fails

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

A terminal tab can disappear after Close reports incomplete process cleanup, leaving no tab to retry, even while its
session remains.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_tabs.rs:1024`.

host_tabs.rs:993-1003 first reaps, then :1008-1022 ends the host and independently runs a second sweep to catch what
survived its hangup. With the host confirmed ended and the second sweep failing, :1024-1034 removes the launch record
and forgets the tab before returning the error. The retry state at :1037-1048 covers host-end failure only.
host_tabs.rs:921-926 requires a matching retained tab, so another Close cannot repeat cleanup once it was forgotten.
SPEC.md:2272-2279 makes unconfirmed cleanup visible and allows a later attempt to retry; SPEC_impl.md:2036-2046 puts the
final sweep in tab close and documents its retry mechanism. Keep retryable cleanup metadata for a failed final sweep
even after terminal removal. Do not require a visible or attachable tab to finish it. Bucket: other for lost tab-level
retry after a source-supported failure path. Session Delete can still be an independent cleanup route, and no actual
surviving process or lost work was established. Confidence: confirmed by pinned source for failure ordering and
inability to retry Close. No runtime observation. Possible coverage: lifecycle_unit, lifecycle_e2e. Parent comparison:
fc2b0b46e6211c7ca4001d05d90221f35ee5bd97 host_tabs.rs:1009-1048 has the same ordering, so the cleanup gap predates the
shared shutdown change. At the reviewed revision, core.rs:16092-16108 explicitly reports post-host failure, and
:13569-13585 visits only retained tabs on a later Stop. The final pass at :14256-14316 can fail independently of a
successful first pass, for example if the second enumeration fails or the scope becomes temporarily uncheckable.
Existing host-end retry assertion at core/tests/host_terminals.rs:4278-4292 covers the different host-unconfirmed
branch. No checked assertion injects only a final sweep failure. If host ending and the final sweep both fail,
:1037-1048 still keeps the retry state, but its error claims the processes were reaped. Report both outcomes in the same
repair rather than count a second finding.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r14`.
