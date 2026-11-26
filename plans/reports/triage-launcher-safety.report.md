# Session launcher keyboard and confirmation safety

## What this was about

Several launcher actions could do more than the user intended. In Replace with, assistive technology described a
highlighted recent setup as a fill action without saying that Enter starts the replacement and deletes the source
session. In New and Clone, Enter could launch the draft if a highlighted choice disappeared before the next redraw. As
decided in triage, clicking a recent setup in Replace with fills the draft, and Enter starts the replacement and deletes
the source session. The highlighted option now says both. In New and Clone, Enter still belongs to the choice the user
saw.

Holding Escape could discard the draft after closing the results or Save as template. The hold now stops at that panel.
A fresh Escape can close the launcher. A permanent YOLO answer that can no longer start its launch now explains why. If
the host already saved the choice, the launcher says so even if another warning or confirmation has appeared. A failure
to load recent repositories now has its own message, and typing an owner and repository still works.

## Things you should know

A host can save the permanent YOLO setting while the draft or host connection changes. The launcher reports the saved
setting and leaves the user to review the current draft before launching again. The previous answer cannot start the
changed draft.

## Open questions and possible follow-ups

None.

## Pull requests

- Keyboard guard for vanished choices and accessible Replace with instructions.
  [#5050](https://github.com/scode/farhelm/pull/5050/changes)
- Held Escape, permanent YOLO outcome feedback and repository history failures.
  [#5051](https://github.com/scode/farhelm/pull/5051/changes)

## Checks

- On rebased revision `412f494b12727c6d2d7db5bba94aa2ccd05ba908`, all 636 selected GUI tests passed through nextest. The
  selection covered the launcher, the workspace trust hint and the launch-versus-restart approval wording that reached
  main during the plan. Ran `cargo nextest run --locked -p farhelm-gui --lib -E '<launcher and related trust tests>'`.
  Recorder run `6c315f77-def6-4a96-9e40-fcf5d321bf81`.
- The full four-process Linux GUI render on that revision completed all 578 images. The six additions matched the
  reviewed images byte for byte. Existing references matched. Ran
  `python3 scripts/check-gui-goldens.py --update --jobs 4`. Recorder run `84d71e82-6bff-44c4-ba49-345ab9f27459`.
- GUI library and test Clippy with warnings denied, Rust formatting, test-sleep checks and changelog format passed.
  Instruction links passed with a pre-existing archived reference to a removed heading.
- The rebase kept the reviewed launcher source and scene definitions. Publication and PR bookkeeping only added the six
  approved references and archive links, so the recorded runs cover the final drafts. The broader battery and docs shots
  were outside the affected paths. The GUI tests drive fake helm replies and presented controls.

## Review

Independent Sol source review cleared both PRs after revisions. An independent scope review confirmed that the
saved-setting notice needed separate storage to preserve newer warnings. Opus checked the first PR's selected images and
the second PR's three new scenes in both appearances, then approved the two revised scenes. In the saved-setting image,
the scroll viewport cuts through part of the More options row. The notice and launch controls are visible.
