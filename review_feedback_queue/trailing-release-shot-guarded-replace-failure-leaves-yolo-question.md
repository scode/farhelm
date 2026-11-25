# A failed v0.45.0 guarded-Replace release shot leaves a YOLO question open for later shots

Reviewed commit: b0608d96ee728c73678c6a4c0527395281f0f0cc

## TLDR

Maintainer tooling only. When the maintainer drafts or recaptures the v0.45.0 guarded-Replace release screenshot and it
fails after Replace is confirmed, the unanswered YOLO question stays open under the `bench tokenizer` session row for
the rest of the capture. Later session-list screenshots that frame that row can then show the stray question, or show
the row's Stop, Restart, Replace, Delete and Rename unavailable, and nothing flags those draft images. A run in which
this shot failed cannot be published, so published images are not affected.

## Details

**other.** Recorded by the trailing review of `e159ffc7..f6d8d8bb`; the tip `f6d8d8bb212be18c1f5cb6566d4fa2f92d64bacd`
still has it.

### Reviewer record (`g001/f2`)

Location: `scripts/docs-shots/release-notes.py:350`. Summary: A failed guarded-Replace shot leaves a YOLO question that
the engine's reset cannot see, tainting later shots.

`guarded_replace_offer` (`scripts/docs-shots/release-notes.py:350-374`) clicks `Confirm replace`, which sends the
Replace. The helm refuses it because the host asks before YOLO launches, and the GUI asks the YOLO question. The shot
waits for `prompt-alternative-0` and the text `Replace as guarded instead`, and only after both succeed returns its
`cancel_offer` cleanup. If either wait fails, no cleanup is registered (`scripts/docs_shots.py:426, 440-442`). Example
triggers are a GUI change to the button's wording or order, or a scenario change that leaves the saved launch without a
guarded equivalent (`launcher/view/yolo_question.rs:63-83` offers the guarded answer only for a harness that offers
Guarded). The same happens if `cancel_offer` raises.

A plain Replace's YOLO question is drawn inline under the row, not as a layer (`session_list/replace_yolo.rs:16-21`,
`prompt.rs:1604-1609`). The per-shot `reset` (`docs_shots.py:305-324`) presses Escape only while a `layer-` element
exists and then clicks the agent terminal tab, so the question survives every reset. While it is open, the session's
lifecycle actions are refused and drawn unavailable (`replace_yolo.rs:30-35`). Pages are captured in sorted order
(`docs_shots.py:67`), so session-list runs after release-notes: its `statuses` shot (`session-list.py:110-117`) frames
the `bench tokenizer` row, and its `row-menu` shot (`session-list.py:137-146`) opens that row's menu. The engine's
session-list guard passes in both cases, so those draft images carry no warning.

Fix: mirror `grouped_sessions` (`release-notes.py:48-80`). Wrap everything from the `Confirm replace` click onward in
`try`/`except` that clicks `id:prompt-cancel` when it is drawn, then re-raises. Alternatively, teach `reset` to cancel
an open inline prompt on the page.

Suggested bucket: other; the harm stays in maintainer tooling and in draft images from a run that already failed. A
failed release shot that is not yet frozen blocks publication (`docs/docs-shots/SPEC.md`: the publisher refuses a
referenced shot the run did not produce).

Confidence: confirmed by source for the failure path and for the question not being a layer. Not observed in execution.
What the session-list pictures would show is inferred from the row's rendering rules, not seen.

proposed_drop: none.

possible_cover: none. The related queue item `docs-shots-share-host-details-state.md` is about a different shot leaking
state on its success path.

Review evidence (findings log, delivery audits, dispositions and drop-check verdicts) is retained under run
`7c1cb54d-c73b-4387-bc50-5406ecc9bae4` as `g001/f2`.
