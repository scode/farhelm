# Header copy feedback is cut short by a second click

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

The "✓ copied" confirmation can disappear almost immediately after a second copy click.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F26 / COR-COPY-FEEDBACK-TIMER`,
tagged **definite**. Anchors and title: `crates/farhelm-ui/src/session_view.rs:1512`,
`crates/farhelm-ui/src/session_view.rs:1515` — A second header copy click cuts the "copied" feedback short

This one is cosmetic. Each click on a header copy button sets the button's `copied` flag and spawns a task that sleeps
1.5 seconds and then clears it (lines 1512–1515). The timers are never cancelled. If the user clicks twice, say 1 second
apart, the first click's timer clears the flag 0.5 seconds into the second click's feedback. The suggested fix is a
per-button generation counter, so a timer only clears the flag if no newer click has happened since it started.
