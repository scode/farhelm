# Check for tmux use after exercising new sessions

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The desktop smoke can pass if creating or restarting a new terminal unnecessarily starts tmux. Its new no-tmux check has
already finished before it exercises any sessions. This weakens evidence for the requirement that a host with only new
terminals never runs tmux.

## Details

Source coordinates from the original records: `scripts/desktop-smoke.sh:1365`. References below use these paths'
basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Introduced by f972458cc78eea039ab185354d13cf8bd395c982, scripts/desktop-smoke.sh:1355-1369; survives at tip at
1356-1370. The sentinel log and absent tmux.sock are checked once, immediately after the initial app launch. The normal
path first creates sessions at 1451 and 1458, then exercises a hard app restart, later creates sessions at 1648, 1832
and through the form at 2090-2173. Searching every occurrence of tmux-override-used and tmux-supervisor-invocations
shows subsequent launches still append to the log (1530, 1873, 1945), but no later assertion reads it.
expect_host_terminal verifies the database kind and therefore cannot catch an unwanted ancillary tmux invocation for an
otherwise real host terminal. The newly changed assertion establishes only empty startup. Move or repeat the log/socket
assertions after the session and restart phases while the owned state is still available. A source-level counterexample
is an unconditional ensure_server call added to the new-terminal create path: all new-terminal-kind checks still pass
and no sentinel assertion follows the logged operation. No runtime mutation or experiment was run. SPEC.md, Terminal
experience:2520-2526 says a host whose terminals are all new runs no tmux server and uses no tmux.

Suggested bucket: other (maintainer validation gap; no product failure or user-data loss established). Confidence:
confirmed for the missing observation window. proposed_drop: false. possible_cover: none; commissioner should check
whether another test already covers the same lifecycle guarantee.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g001/l4`, `g003/l2`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
