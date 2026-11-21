# Let Templates close and keep save questions in view

## What this was about

After opening a template and scrolling down, you could get stuck in Templates. Escape alternated between the save
question at the bottom and the name field at the top. The header Close could ask the same question again. The cause was
found by reading the code. Opening some older templates automatically filled in a launcher choice or dropped a legacy
trust setting, which was enough to ask even if you had made no edits.

You chose to let untouched templates close without asking and to show the same centered question in Templates and the
recipe editor. In Templates, you can close, select another template or start a new one without a question if the draft
is as it appeared when you opened or saved it. The editor can still show Unsaved changes, and Save stores the automatic
changes. An edited new draft or an unsaved duplicate asks before you leave it. The question appears over the dimmed
editor and starts on Save. Escape keeps the draft and returns to its name field. Discard goes where you were heading.
Save proceeds once the write succeeds, or immediately if your edits already match the stored template.

## Things you should know

The same centered question appears when you leave a recipe with edits in Settings → Recipes or in Add host when you use
New recipe… on the Tensorlake form. Its Delete confirmation still appears with the editor's buttons. If you try to close
during a save, the question offers Keep editing and Save. The pending save can finish closing the dialog while the
question is unanswered. A refusal dismisses the question, shows the helm's reason and puts focus on Save.

## Open questions and possible follow-ups

The GUI fixtures covered the close controls and save question. Replaying the original scroll sequence in the app is
pending. On a devbuild, open the older template that showed the problem, leave it untouched, scroll to the bottom and
press Escape. Templates should close on the first press without a question. Reopen the same template, scroll down and
repeat with the header Close button.

## Pull requests

- Templates leaves unchanged drafts without asking and centers the save question.
  [#4345](https://github.com/scode/farhelm/pull/4345/changes)
- The recipe editor centers its save question in Settings and Add host.
  [#4346](https://github.com/scode/farhelm/pull/4346/changes)

## Checks

- Linux `cargo nextest run -p farhelm-gui --lib` passed 371 tests across the Templates, Settings recipe editor, Settings
  recipe departure and Tensorlake Add host test modules (recorder `7473f794-179a-46ca-a5c4-b1ae52fa9384`). This run
  preceded the final save handling, focus and interaction assertions. The second run selected the Templates and Settings
  recipe departure tests plus the Add host discard test. All 32 passed (recorder
  `18bc0df6-0591-4942-aa83-cf30e147bcaf`). Both builds used main at `dd38cc5b` with the plan changes applied. Only docs
  and screenshot references changed after the second build.
- `python3 scripts/check-gui-goldens.py --update` captured 298 Linux images (recorder
  `0e0031da-5e7a-44ab-9b7d-9b5ad18212eb`). Exactly the two Templates save question images changed. Both themes were
  inspected and published with `python3 scripts/gui-goldens.py publish`. This capture preceded final edits to Save
  handling and tooltip copy, which the screenshot does not exercise.
- Rust and Markdown formatting, changelog format and test-delay rationales passed. The agent-instruction link checker
  reported one existing warning in the triage archive.
- The focused GUI tests exercise the affected controls and save outcomes. The workspace suite, hosted CI and native
  macOS checks were skipped for this dialog-only change. The devbuild scroll sequence is still for manual confirmation.

## Review

Both changes received an independent source review and cold commit wording checks. The Templates review caught an edge
case where choosing Don't switch restores the stored fields. Save now honors the requested departure without sending a
write in that case. The recipe review strengthened assertions for pointer input blocked by the question and the order of
the answer buttons.
