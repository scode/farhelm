# Some tmux command paths fail to remember server absence

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

After the old terminal server disappears, some capture and cleanup paths continue issuing tmux commands despite
receiving a definitive absence reply. The extra work is finite; no indefinite churn or loss of user work is established.
The sixteen-sample bound applies to the sampler alone, not every cleanup roster.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:2856-2876,2900-2933,4734-4748`;
`crates/farhelm-supervisor/src/tmux.rs:2856-2933`;
`crates/farhelm-supervisor/src/tmux.rs:2727-2791,2811-2844,2856-2876,2898-2934`;
`crates/farhelm-supervisor/src/tmux.rs:2856-2933,4734-4748`;
`crates/farhelm-supervisor/src/tmux.rs:2995,3030-3032,4680-4748; crates/farhelm-supervisor/src/tmux/presence.rs:280-291`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The ordinary runner records the command's presence epoch and calls observe_failure on typed NoServer/SocketMissing
failures (tmux.rs:2995,3030-3032; observe_failure at2610-2616). The separate bounded streaming capture runner checks
presence before spawning at4680, but its nonzero-exit path simply constructs/returns TmuxCommandFailure at4734-4748. It
never samples an epoch or calls observe_failure. Thus after a real capture returns the exact NoServer/SocketMissing
diagnostic, the model can remain Running and the next capture passes the cached gate again. This is distinct from e1:
the before-spawn gate exists; it lacks negative-result publication.

A concrete shipped caller supplies additional commands without an intervening ordinary liveness query:
Terminal::screen_tail dispatches tmux to capture_pane_tail (service/terminals.rs:324-334), through run_bytes_tail ->
run_capture -> run_capture_within (tmux.rs:4641-4688). The sampler first selects live entries from one pane-state
snapshot (ticker.rs:1664-1690), then iterates up to its bounded budget (1712-1717). Each screen capture error forgets
only that entry's sample and continues (1734-1748), rather than disabling captures. If the server disappears after the
successful whole-server snapshot but before the first capture connects, the first capture can return NoServer; the next
selected old terminal still spawns another capture because presence was not updated. The pane-death waiter lives in this
same ticker task's select and is not polled during this capture loop (ticker.rs:1026-1032,1039), so its existence does
not guarantee an intervening cache update. A concurrent UI command can learn absence sooner, but is not required and is
not assumed in this quiet-viewer interleaving.

The next ordinary listing/command normally repairs the cache, and each sampling pass has a budget. Do not infer
unbounded churn, a stuck server, additional terminal loss, duplicate agents, or a consent violation from this omission.
The old terminal server has actually gone; its loss is not caused by these extra clients. Source/conditional behavior is
verified; no runtime occurrence, process-count measurement or frequency is claimed.

SPEC_impl.md Terminal substrate: private tmux server:2829-2831 states that a command reporting no server stops all tmux
traffic for the rest of the process unless an old terminal is created again. The same driver contract is documented by
observe_failure at2602-2616. Capture is a normal driver command, not an exception named in that contract. Apply the same
epoch-aware negative observation to capture failures (and audit other direct command funnels), using the epoch from
before spawn to preserve concurrent creation protection. A focused test can keep presence Running, have a capture return
typed NoServer, then assert a second capture is refused without invoking tmux. No runtime experiment was authorized or
performed.

Suggested bucket: other, for bounded unnecessary process work and a missed no-tmux-traffic contract, not a highest/high
work-loss consequence. Confidence: confirmed conditional source path; disappearance timing unmeasured. proposed_drop
false. possible_cover none among the latest relevant named peer records read. Not g032/d2 (implicit attach autostart),
g031/d1 (startup gone error latched Unusable), f1 (live server falsely Absent), or f4 (consent before cleanup).
Mandatory assigned observe_failure/runner changes are the root context; capture body is necessary adjacent interface
source, not claimed additional mandatory coverage.

Additional verification for e2, not a new independent finding. Reviewed tip407a49435278844248d573083fdea6e23fd96c02 and
introduction f972458cc78eea039ab185354d13cf8bd395c982. The full producer chain is verified: capture_pane_tail
at4505-4512 calls capture_pane_plain, which builds native capture-pane args at4606-4622 and calls run_bytes_tail; that
reduces through run_capture_within. At introduction4799-4876 the same body adds refuse_if_absent before spawn but still
returns nonzero typed errors without any observe_failure; the ordinary runner's new observer is a separate path. Thus
this is an incomplete edge of the in-range no-server learning policy, not a newly invented pre-existing bug under an
older policy that always retained tmux.

Exact consequence bound: ticker.rs:196-208 fixes SAMPLE_TAIL_BUDGET=16, and1712 limits selection to
budget.min(live.len()). If the first selected old-terminal capture establishes NoServer, at most fifteen remaining
captures in that sampler pass can still spawn by the same omission, absent another concurrent operation correcting the
cache first. This is a source bound, not a measured process count. The next ordinary pane listing or pane-death error
usually clears Running. No indefinite polling, background process survival, new security/data/work loss or high
consequence claimed. SPEC_impl.md:2829-2831 explicitly requires command-reported absence to stop traffic, so the bounded
nature limits impact rather than accepting the missed publication. Suggested bucket other unchanged; confidence
confirmed conditional source path, server-loss timing unmeasured. proposed_drop false; possible_cover none among named
records read. No runtime experiment.

Supplement to retained e2 at reviewed tip407a49435278844248d573083fdea6e23fd96c02, not a replacement of the original
capture finding. New q3 records exact additional direct-funnel callers and introduction ancestry:
list_client_pids_and_flags maps typed NoServer to None without observe_failure; disable_control_client_output does not
type/observe its nonzero stderr at all. After a successful roster then actual server loss, startup reap and planned-stop
quiet-down can continue their remaining roster commands. Startup verification can also issue its required second clean
listing after the first typed NoServer. Same incomplete f972458 command-reported-absence policy, suggested other;
consolidate q3 with e2 rather than counting a new independent root. Preserve original IDs and evidence. This addition is
source-verified conditional behavior with occurrence/timing unmeasured. It does not change e2's exact sampler bound
of16selected/up to15remaining doomed captures: that count belongs only to the sampler; cleanup work is finite
roster-dependent, with planned-stop fallback overall2s and startup clean-verification policy separately bounded. No
indefinite traffic, surviving process, security, user-work loss or higher consequence is established. Fix audit should
include all these direct command funnels and preserve epoch/concurrent-create protection; do not just add a callback to
captures and declare the policy complete. SPEC_impl.md:2829-2831 and the observe_failure driver contract at2602-2616
remain the basis. Existing f1 false-Absent, f4 early-Unusable destructive admission and d1 startup gone classification
are distinct and unchanged. proposed_drop false for combined e2; possible_cover own q3 exact additional variant, no
known named peer duplicate. No original finding dropped or lowered.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g031/e2`, `g031/q3`,
`g009/g009-i15`, `g009/g009-d17`, `g032/i1`, `g033/i16`. The retained append-only histories preserve all earlier claims
and corrections. Reviewed source references name their own revisions; the inclusive reviewed tip is the hash above. This
review used source inspection only, with no product runtime experiments or live-install changes.
