# Surviving-tab adoption still lacks a supported history

Reviewed commit: 969d68f22c377d0d346f052c4f3d83981c0cbc56

## TLDR

Startup could adopt a surviving shell tab as the agent if the required agent-gone/tab-alive state existed. Ordinary
Delete and tab creation did not establish that history; the conditional concern is not a reproduced loss of Resume.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location:
`review_feedback_queue/startup-reload-adopts-surviving-tab-as-agent.md:7-104 (969d68f22c377d0d346f052c4f3d83981c0cbc56 parent)`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Final later-turn synthesis, same original candidate g200/f5 = g201/f2. Reviewed deletion
969d68f22c377d0d346f052c4f3d83981c0cbc56. No concrete supported producer established; retain conditional OTHER,
potentially highest if an ordinary agent-window-removal/tab-survival route is established. The source mechanism remains
confirmed and is not refuted by unknown runtime frequency. Additional focused checks: Delete runs on a supervisor-owned
task under the lifecycle claim (service/handlers.rs:1095-1182), captures all pane roots and sweeps WholeSession
(service/teardown.rs:325-439), then issues whole-session kill (teardown.rs:603-625; tmux.rs:3093-3104) before artifact
cleanup and session-row deletion (teardown.rs:640-657,778-788; store.rs:3423-3426,3521-3523). A failure after tmux
teardown can retain the durable row, but that alone leaves no tab window; before tmux teardown, process exits leave
remain-on-exit windows. No partial vendor kill-session behavior is inferred. Open Tab cannot recreate a tab-only
substrate after that failed Delete: it takes the same lifecycle claim, requires an entry terminal and existing tmux
session, and calls only new_window (core.rs:13179-13215,13231-13249,13375-13390). Static test source in
teardown.rs:1790-1922 specifies whole tmux-session absence plus retained row after a forwarder failure and successful
retry; it does not fixture a surviving tab and was not executed. The row-refusal test at 1924-2023 verifies
attachment/row retry semantics, not tab survival. SPEC.md at pinned tip
463cac1528b5c5e259404975380d321a3fff0412:3232-3238 accepts partial Delete, but does not accept wrong-target destructive
recovery. Reviewed SPEC.md:3414-3425 (pinned tip:3501-3512) excludes deliberate private-server mutation; AGREE on that
exact variant. AMBIGUOUS on dropping the whole supported-route remainder. possible_cover: that exact mutation contract
and reviewed TRIAGE_OUTCOMES.md:17901-17916 (tip:17948-17963). proposed_drop: deliberate private-server mutation only,
not subsystem or whole conditional candidate. No new finding, no withdrawal, no runtime verification. Missing source
premise and bounded nature of mutation checks preserved.

Provenance: `a063/f2`, `g200/f5`, `g201/f2`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
