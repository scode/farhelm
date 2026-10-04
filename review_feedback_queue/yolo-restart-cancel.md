# Restart with accepts a cancelled YOLO answer

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Restart with accepts a cancelled YOLO answer.

## Details

`F22 / COR-YOLO-RESTART-CANCEL` — **definite** — `crates/farhelm-ui/src/restart_with.rs:608` — Restart with accepts a
cancelled YOLO answer

Cancelling the YOLO question inside Restart with clears the parent's question, but an affirmative handler rendered
before that cancellation can still request an unrestricted restart. Its eligibility check comes from the earlier render.
The parent checks that a restart is not already running and that the submitted settings still fit the session; it does
not require a live YOLO question before accepting the override.

Both affirmative answers can therefore restart the agent without approval prompts after the user cancelled that consent.
If the original Restart with action allowed stopping a working agent first, the stale answer can also stop that agent.
For the permanent answer, a missing question yields no host to update, yet the unrestricted restart still proceeds.
Require the parent to consume a live question bound to the opening and submitted settings whenever an override is
requested, and reject a permanent answer without its bound host. Cover Cancel followed by each answer in one event
burst.

Suggested bucket: highest

Possible cover: none

Caveats: The queued-event premise is source-established; no browser reproduction was performed. Ordinary Restart
intentionally does not ask a new YOLO question, because it repeats a launch choice already made. This finding concerns
Restart with new settings.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `ui_lifecycle p1`, `ui_security p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Same source-established eventburst premise; no browser repro. Ordinary Restart intentionally no YOLO
question.

Location accounting: the independently editable parent acceptance callback at `session_view.rs:2302` is recorded
separately as `COR-RESTART-PARENT-CANCEL`. This item retains the child callbacks at `restart_with.rs:608`.

## Filed reviewer metadata

- `ui_lifecycle p1`: confidence as filed: definite; confirmed by the child-to-parent handler trace. Caveat: not runtime
  reproduced. Suggested bucket as filed: highest (security consent; running processes can also be stopped).
- `ui_security p1`: confidence as filed: definite / confirmed by code under the queued-event premise; no runtime replay
  performed. Suggested bucket as filed: highest (unwanted unrestricted restart and stopping a live agent).
