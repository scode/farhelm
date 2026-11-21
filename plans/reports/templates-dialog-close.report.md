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

### Landing

#### What landed with it

Main gained nothing but plan bookkeeping since this plan's work started, and nothing else landed in this round, so there
was nothing for it to interact with. An independent reviewer and I read the two PRs against the plan. Neither of us
found a way for the new centered question to stay on screen after its dialog closed, to close a different dialog from
the one you were leaving, or to let a second Escape close the dialog behind it. The Templates and Settings Close buttons
cannot be clicked while the question dims the dialog, which the tests check by clicking where those buttons are. The
plan deleted four tests of the old question at the bottom of the editor. Each checked something only that question
allowed: editing fields while it was up, a second attempt to leave replacing it, scrolling it into view, and pressing
the ⚡ quick-launch buttons under it.

#### Fixes made while landing

One went in as a third PR, [#4347](https://github.com/scode/farhelm/pull/4347/changes). It changes the rule the report
describes. As delivered, Templates asked whenever the draft differed from what it showed when you opened or saved the
template, even when there was nothing to save. Open a template saved before launcher choices existed and pick "Don't
switch": the draft then matches the stored template again, "Unsaved changes" goes away and Save greys out, yet leaving
still asked "Save these changes before leaving?", with Save and Discard doing the same thing. Templates now asks only
when the draft holds edits since opening or saving that Save would store. That let the question's Save drop the special
case the executor had added for having nothing to write. The executor's test of that case now checks that Escape just
closes, and the test of an untouched older template now also checks the header Close with no edit first, which it had
skipped.

The same PR fixes a lint error (clippy) in one of the plan's new tests, which the landing's lint run caught; the
executor's report does not mention running clippy. It rewords SPEC.md's sentence for the new rule. It fixes code
comments that still placed the question at the bottom of the editor, and a test description whose stated reason was
wrong. It also updates the notes that map the old browser app's tests to the desktop app's: they now say how the old
question behaved, and that one old test's check that the question scrolls into view no longer applies. It retitles the
save question's screenshot in the list that describes each screenshot (a text label; the picture is unchanged), adds a
context paragraph to the changelog entry for release curation, and makes the "+ New template" hover help say "the open
one" rather than "this one".

#### Checks

The plan changes only the desktop app's GUI code (the Templates dialog and the Settings recipe editor), the screenshot
of the save question (light and dark), SPEC.md, SPEC_impl.md, notes and a changelog fragment. So the landing ran the GUI
crate's whole test suite, the app's end-to-end smoke and the main lint (clippy) run on a separate arm64 Linux test
machine, and the screenshot comparisons and small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run -p farhelm-gui`, on the test machine (`ede1d28c-0c23-4e79-93fb-fad1c32f59a8`): all 3222 tests
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (`cc2ba1fc-0390-4ea2-9913-17a5167dd1c1`): every step passed
  but paste, which skips on Linux as it always does.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: failed on a lint in a new
  test, fixed in #4347.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (`985b6450-1869-4f1b-9d50-1c0e7d631d50`):
  passed, with the plan's re-captured pictures of the question.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

After the landing fix (#4347), which changes the GUI crate's code and tests, SPEC.md, the notes, the screenshot list's
label and the changelog fragment:

- `cargo nextest run -p farhelm-gui`, on the test machine (`29e39ad5-1807-4225-9bcc-455ae3a3343f`): all 3222 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 -B scripts/test-gui-screenshot-batch.py` (which validates
  the screenshot list), locally: passed.

A last amendment to #4347 after its own review changed only code comments and a note, so the runs above were reused for
it; `cargo fmt --all -- --check` and `dprint check` passed again locally.

Reused for the fix: the smoke, since it exercises no save question, and the screenshot comparisons, since the one
screenshot showing the question renames the template first, which still asks. Skipped: the workspace suite and the tests
of the desktop app's startup and supervisor handling, since nothing outside the GUI crate changed, and Mac runs, since
no macOS-specific code changed; the report's devbuild check covers the Mac by hand.

#### Things you should know that the report does not say

- Two report sentences describe behavior #4347 removed: that Save "proceeds … immediately if your edits already match
  the stored template", and the Review paragraph's "Save now honors the requested departure without sending a write in
  that case". When nothing would be saved, Templates no longer asks at all.
- Escape on an edited template goes back and forth between the question and the editor, and the dialog stays open until
  you answer Discard or Save. The original problem was untouched templates doing this; for edited ones it is the plan's
  decision, since Escape in the question means Keep editing.
- Smaller points from the review, left as they are. No recipe editor test checks where focus lands after Keep editing.
  When a recipe save finishes while the question is showing, the recipe editor treats the question as coming from
  closing the dialog; that is safe today because Back and Cancel are disabled during a save, but if either were enabled
  then, a finished save could close the dialog instead of going back to the list. And in Settings the recipe editor's
  buttons sit above the dialog's footer, so SPEC.md's new wording that keyboard navigation reaches "the footer" after
  the fields is slightly off there; it said "the answers" before.

#### What you should still do

The report's devbuild check still stands. Open the older template that showed the problem, leave it untouched, scroll to
the bottom and press Escape, then repeat with the header Close. Both should close Templates at once.
