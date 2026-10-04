# Launcher YOLO answer can restore cancelled consent

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Launcher YOLO answer can restore cancelled consent.

## Details

`F21 / COR-YOLO-LAUNCHER-CANCEL` — **definite** — `crates/farhelm-ui/src/list/create_form.rs:4115` — Launcher YOLO
answer can restore cancelled consent

Cancelling the launcher's YOLO question leaves the refused launch's request identity intact. Each affirmative button
retains that identity and, when clicked, records it as confirmed; both buttons then submit the surrounding launcher
form. The submit path allows a launch without approval prompts when its request identity matches the recorded
confirmation. It does not require the question to remain open.

Consequently, an affirmative click queued behind Cancel can restore consent for the unchanged draft and launch it. In a
Replace with launcher, that can also delete the source session; the permanent answer can change whether the host asks
about future YOLO launches. Consume a live refusal bound to the question's opening and the intended launch before
recording consent. A stale answer must leave the form unauthorized, while a genuine confirmation must still support
retrying the same request.

Suggested bucket: highest

Possible cover: none

Caveats: The queued-event premise is established by the project's confirmation tests and earlier review decision, but
this launcher sequence has no browser reproduction. An edit that changes the intended launch prevents the old identity
from matching; cancelling the question while leaving the same draft does not.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `ui_data p1`, `ui_lifecycle p1`, `ui_security p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Same source-established eventburst premise; no browser repro. Draft changes prevent matching but
cancel same draft does not.

## Filed reviewer metadata

- `ui_data p1`: confidence as filed: **definite / confirmed**, under the same already-established queued-event premise.
  Suggested bucket as filed: **highest**.
- `ui_lifecycle p1`: confidence as filed: definite; confirmed by the handler/state trace. Caveat: not runtime
  reproduced. Suggested bucket as filed: highest (security consent).
- `ui_security p1`: confidence as filed: definite / confirmed by code under the same queued-event premise above; no
  runtime replay performed. Suggested bucket as filed: highest (unwanted YOLO launch, and possible replacement of an
  existing session).
