# Bound the tmux probes at both new callers

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The new server-exit test does not enforce its stated one-minute wait while a tmux probe is in flight. Its companion
test's one-shot server-alive assertion also has no probe deadline. These are source-established local timeout gaps in
maintainer validation; no dependency hang or product failure was observed.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/host_processes.rs:688`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Later-turn synthesis checked every occurrence at tip and at introducing commit f972458cc78eea039ab185354d13cf8bd395c982.
The helper tmux_server_runs has exactly two callers. At tip host_processes.rs 865–872, the exit poll creates an absolute
deadline then awaits the unbounded helper before checking it. Pass that same deadline into each probe; minting now+WAIT
inside each loop iteration would reset the intended total wait. At 945–948 the unusable-tmux test calls the same helper
once to prove that the preserved server is still running. That caller should supply its own absolute probe deadline (for
example now+WAIT), even though it has no polling loop. The corresponding introduction lines are 685–691, 862–869 and
942–946.

The helper awaits harness::tmux_query (tip harness.rs 542–550), which calls Command::output without a timeout or
kill-on-drop. The existing tmux_query_before_deadline at 589–601 takes an absolute deadline and delegates to the bounded
command worker; 553–569 documents that worker queue delay counts and direct-child cleanup has an additional reserved
second. Preserve expiry as a failed observation with context, not false meaning the server has exited. A timeout must
never make the exit test pass.

Three claims must remain distinct: (1) the local WAIT contract is not enforced around either new subprocess; this is
confirmed by the call graph; (2) .config/nextest.toml 18–21 imposes an outer emergency whole-test termination after five
60-second periods and grace, which is not that local WAIT contract or normal fixture cleanup; (3) no tmux hang was
induced, observed, or established as occurring in these tests. The consequence is conditional on a probe not returning
promptly. This finding does not claim an observed hang, a release regression, or a product security/data-loss route.

The exact governing instruction is the test-authoring checklist: 'Bound polling time and retained output, and use shared
polling helpers.' SPEC_impl.md's Testing and private-tmux lifecycle sections do not exempt this poll. Suggested bucket:
other. Confidence: confirmed for the local deadline gaps; an actually hung dependency is not observed evidence.
proposed_drop: false. possible_cover: none. This revision clarifies both callers and the evidence boundary without
increasing or lowering severity.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g039/l3`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
