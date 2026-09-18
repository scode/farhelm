# The no-selector create error blames the wrong caller

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A local user who forgets to say what a new session should run gets an error message explaining session-spawn rules that
do not apply to them — and following its advice leads straight into a second error saying the opposite.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite. Two lenses agreed
(correctness-edge-inputs p1, correctness-systems p1). Impact is a misleading diagnostic only — nothing is wrongly
mutated. The restater corrected the quoted text and the fix's mechanics; the coordinator verified both against the code.

`create_mode` (handlers.rs:243) sorts the wire fields into one create shape. Its all-`None` arm answers "a restricted
create requires an explicit profile name, profile id, or inheritance selector" (320-323). But the restricted dispatcher
pre-validates exactly-one-selector before ever calling it (3041-3053), so that arm is reachable only from the
full-authority path. Worse, the next validation refuses profile selectors to non-spawn callers ("available only to a
session-authenticated spawn", 353-359) — the two messages contradict each other back to back.

Suggested fix: vary the message by caller authority. Note the helper takes no authority parameter, and the sibling that
branches (362-368) lives in `resolve_create_selector`, which distinguishes callers by matching on the `CreateAdmission`
value — so either pass the admission into `create_mode` or reword at the call site.
