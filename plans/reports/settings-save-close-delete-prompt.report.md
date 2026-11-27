# Report: Save takes an open Delete question away, Keep editing keeps it

## What this was about

In Settings, a custom agent and a recipe each have a Delete button whose confirmation ("Delete this custom agent?",
"Delete recipe …?") replaces the footer until you answer it. #5075 made one case behave a particular way: with a custom
agent's Delete confirmation still unanswered, closing Settings asks "Save changes before closing?", and choosing
**Save** quietly drops the Delete confirmation and saves your edits. The specification has a general rule that nothing
closes a confirmation it did not open, with no exception for this, so the TODO entry asked you to either bless the
behavior in the spec or change it back.

When the plan was written you decided to keep it: the Save is the newer answer, and a Delete still answerable after you
chose to save could delete what you just kept. You also decided the spec exception should cover recipes too, and that
the recipe editor's own question when you go Back ("Save these changes before leaving?") should behave the same way.
That question went further than Settings' close: it dropped the Delete confirmation the moment it appeared, so choosing
**Keep editing** brought you back to an editor whose Delete confirmation had vanished without an answer.

What is on main now:

- The specification's rule about closing confirmations has one stated exception: choosing **Save** for a Settings form
  in an unsaved-changes question (Settings' close question, or the recipe editor's question on leaving) takes that
  form's own unanswered Delete confirmation away once the save starts. **Keep editing** leaves it open. The Settings
  closing paragraph and the Recipes paragraph say the same, and the exception does not clash with the rule that a dialog
  or menu opened from Settings stays open after a close's saves finish: the Settings closing paragraph now says a form's
  own Delete confirmation is not one of those dialogs.
- In the recipe editor, going Back with unsaved changes while the Delete confirmation is up now leaves it in the footer.
  **Keep editing** returns to it, still answerable. **Save** takes it away once the save goes out, as Settings' close
  already did. **Discard** leaves the editor, and the confirmation goes with it.
- For a recipe, tests cover Keep editing in both questions (the confirmation stays and still deletes when answered) and
  Save in the recipe editor's question (the confirmation goes). The existing test of Save in Settings' close question
  now also checks that the confirmation was still up when that question opened.
- A changelog entry for the recipe change, and the TODO entry is removed.

## Things you should know

- **This changes something that shipped, so I added a changelog entry although the plan said none was needed.** The
  recipe editor's leave question has dropped the Delete confirmation in every release so far, so the PR is a `fix` with
  its own entry: "Keep editing" no longer drops a recipe's open Delete question. The custom agent part comes from #5075,
  which has not been released yet, and its existing entry already says that Save drops the question, so I left that
  entry alone.
- **A Save that does not go out leaves the confirmation alone.** If the save is refused before anything is sent (a form
  error, or the recipe already busy with a build), the Delete confirmation stays. The spec says "once the save starts"
  for this reason.
- **Add host is unaffected.** It embeds the recipe editor for a new recipe only, which has no Delete button.

## Open questions and possible follow-ups

- **A custom agent's Keep editing has no test of its own.** The spec now says Keep editing in Settings' close question
  leaves a custom agent's Delete confirmation open. The code does that (it takes the confirmation away only when a save
  goes out), but the tests exercise it on a recipe only. A small test in the custom agent tests would pin it.

## PRs

- #5136 fix: keep a recipe's Delete question open on Keep editing (the spec exception, the recipe editor change, the
  tests, the changelog entry, the TODO removal).

## Checks

Run during this plan, through the test recorder:

- Run `f68f823b`,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::recipe_leave_tests::/) | test(/^settings::recipe_editor_tests::/) | test(/custom_agent/)'`:
  44 passed. That is every test of leaving the recipe editor and deleting a recipe, the recipe editor's other tests, and
  every custom agent test in the desktop app, run after the review fixes.
- Run `1f538d3f`, the new Keep editing test with the old behavior temporarily put back: it failed where it should (the
  Delete confirmation was gone once the leave question opened), so the test does tell the two behaviors apart. The
  temporary change was removed before committing.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test sleep check,
  `dprint check` on SPEC.md and TODO.md, and the changelog format check: all clean.

Run now, on the change as it lands: `cargo check -p farhelm-gui --all-targets`, clean.

Reused: run `f68f823b`, from before the final rebase. What landed in between (an agent's request to add a host, two
Codex conversation fixes and a command-line fix) touches neither Settings nor the shared question code.

Earlier runs, superseded: `c7f28e82` (42 passed, 2 failed: the first version of the two new tests clicked Back while it
was scrolled out of sight, so the click missed; fixed in the tests, not a product problem), `4c952fee` and `2b1e8fa5`
(diagnostic runs that showed it), and `60ccd692` (44 passed, before the review fixes).

Skipped: the desktop app's golden pictures, since nothing drawn changed (the confirmation looks the same and only stays
up longer), and every other test suite, since nothing outside Settings changed.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker and confirmed the behavior above, including that only a
Save that actually goes out takes the confirmation away and that Add host is unaffected. Its one should-fix was that the
new Save test did not check that the Delete confirmation was still up when the leave question opened, so it would have
passed against the old behavior too. That check now runs at the start of every such test. Its four small points were
fixed as well:

- two small cleanups of the test code;
- the Recipes paragraph says "a Save that starts", matching the rest of the spec;
- a sentence in the code's notes was reworded.

The fixes were not reviewed again. Run `f68f823b` ran after them.
