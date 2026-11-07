# Tab-survival test has no successor live-process witness

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The converted tab-survival test can potentially pass after a handoff has incorrectly ended the shell. Its successor
checks the host's remembered child number and old terminal text, rather than asking the shell to do anything. This is a
test-coverage concern, not a claim that shipped handoff kills shells. Exact false-pass scheduling remains unverified
after investigation.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/tab_lifecycle_edges.rs:704-721`;
`crates/farhelm/tests/e2e/tab_lifecycle_edges.rs:617-628,691-721`. References below use these paths' basenames unless
another path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

The assigned F f42178f9402ce22c60530360edc10d6721ef5b16 migration introduces tab_host_shell (tip
tab_lifecycle_edges.rs:617-628), which returns host.status().child_pid without inspecting liveness. The test establishes
a live shell before handoff at 679-694. After retire_supervisor and reconstruction it only compares that PID at 704-708,
attaches at 710-719, and looks for pre-handoff BEFORE-RESTART at 720-721. There is no successor input/response or
independent process-is-alive assertion.

Accessible source checks: core.rs:18267-18273 returns the stored tab runtime without filtering finished tabs;
host_tabs.rs:1530-1567 adopts a found host as Recovered and returns its HostTab without a post-replay liveness wait. A
host whose child was already reaped is initialized Running { child: None } until its exit record is processed
(hosts/terminal.rs:1781-1785). The PID remains info.child_pid, so equality of this field is not proof the shell still
executes. host_tabs.rs:190-195 calls a tab finished only from processed host evidence, and terminals.rs:916-922 admits
an interactive attach while that evidence remains live. Once attached, grid_stream.rs:423-460 and 492-497 sends
screen/history and ReplayComplete separately from later state, while harness grid_text at 177-200 ignores State
messages. Consequently the cached BEFORE-RESTART text can satisfy the test even if a later screen read already knows the
shell exited; the transcript predicate never checks the exit state.

The converted oracle differs from the old one. F parent tab_lifecycle_edges.rs:648-679 compared tmux's pane PID map then
attached to the tab. Although pane_pid alone was also cached, old terminal resolution probes current pane state and
rejects a dead tab at terminals.rs:990-1004. The host attach instead permits the recovered Running-until-Exit gap. This
does not claim the old test was immune to a shell exiting after any successful liveness check; the newly different
observation is a remembered PID and asynchronous exit evidence during recovery. SPEC.md, Session view, 1744-1748
promises tabs survive supervisor restarts as running shells; Terminal experience distinguishes retained terminal
contents from a live process. A unique post-replay shell command with an assembled response, as run_in_shell already
uses before handoff, would test the missing promise and would reject terminal-driver echo.

Unverified after investigation: no retained trace establishes the exact fixture admitting its successor attach before
the replacement processes an exit caused during handoff. Source establishes the conditional false-pass route and missing
witness, not an observed handoff failure or frequency. No runtime experiment was authorized. A controlled
shell-exit-at-handoff fixture or per-frame trace would settle actual false-pass reachability.

Suggested bucket: other (maintainer test coverage only). Confidence: likely with the scheduling premise above.
proposed_drop false. possible_cover: peer g040/l2 and g043/f6 concern other tests that falsely reject an unprocessed
successor exit; those records are known and not repeated here. This record concerns the opposite survival claim
accepting remembered PID/text, at a distinct assigned test. Commissioner should decide whether to group the shared
recovered-exit observation context.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g045/g045-d1`,
`g046/g045-d1`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source
references name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection
only, with no product runtime experiments or live-install changes.
