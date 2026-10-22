# Respawn reflow crosses the launch-excerpt boundary

Reviewed commit: a46ef2263243f698db133d8cf7c449d0463d16f5

## TLDR

After a terminal is narrowed while showing an alternate screen, a later silent startup failure can quote and durably
retain text from the preceding run. That text can contain sensitive work; a consequential confidentiality boundary
crossing has not been established.

## Details

Suggested bucket: provisionally **highest** for possible confidentiality impact; diagnostic misattribution alone may be
filtered. Confidence: **source-established old-output capture mechanism**, with content sensitivity, extra exposure and
runtime frequency unmeasured. No unauthorized recipient or credential disclosure was demonstrated.

`crates/farhelm-supervisor/src/tmux.rs:3258–3281` samples history before stamping the new generation and respawning.
Capture later trusts that history boundary (`3747–3785`, especially `3770–3779`). It validates token, pane ownership and
death, but those checks do not detect history inserted internally during respawn.

The exact pinned upstream tmux 3.7c source explains the insertion. `spawn.c:248–265` calls `screen_reinit`;
`screen.c:120–127` exits alternate mode before clearing the visible grid. `screen.c:717–754` restores the saved primary
screen at its old dimensions, enables history, then reflows it to the current width. `grid.c:1364–1507` splits long old
rows and increases history. Clearing the visible grid leaves those promoted history rows intact, after Farhelm's sample
and under the new token. For example, an 80×24 primary screen with one 60-character row and 23 blank rows, followed by a
40×24 alternate screen, restores 25 rows; old characters enter history and can be quoted by a silent pre-shim failure.
Earlier explicit-resize invalidation does not cover this internal respawn reflow.

The source archive's SHA-256 matched the repository's pin. No tmux execution or product test was run. The ordinary
command-queue interleaving hypothesis and one-row death-banner readiness hypothesis were separately refuted from that
source; neither is needed for this mechanism.

Possible cover: pinned `SPEC.md:1756–1764` requires failed-launch attribution. The status/history-after-partial-failure
filter can cover diagnostic-only misattribution, but excludes security consequences. It therefore does not justify a
blanket drop while the confidentiality interpretation remains unresolved. Documented retention of the failed launch's
own output does not cover preceding-run text. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, corrected `g100/f1`, independently discovered as
`g144/f5`; `g099/f1` was reopened for cross-group reconciliation. These describe one mechanism.
