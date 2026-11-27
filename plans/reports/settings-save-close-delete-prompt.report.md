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

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round's changes to the helm, the supervisor, two
comments in the desktop app's launcher and two tests, none of which touches Settings. The Settings close-wait work
(closing Settings waits for the helm's answer) was already in this plan's starting point, and its spec text agrees with
this plan's. In the same round, agent-blocked-notifications changed the Notifications page of Settings,
claude-restart-before-first-turn changed Restart for Claude, and shell-tab-first-close-sweep-backoff changed how an
exited terminal tab is cleaned up. None touches the recipe or custom agent editors.

An independent reviewer and I read the change and found nothing to fix.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing I found. The landing's checks were wider than the report's: the whole Rust test suite, the desktop app's
end-to-end smoke and the full screenshot comparison ran.

#### Worth knowing

- **Two questions on screen at once can share button names.** When the recipe editor's "Save these changes before
  leaving?" opens while its Delete question is still up, both are on screen with answers of the same internal names.
  Settings' close question already did this before this plan. It causes no wrong answer that the reviewer or the tests
  found; it only matters to tools that find buttons by those names, such as test automation and accessibility tools.

#### Checks

Run on the round's four plans stacked on main as it stood when the landing started (agent-blocked-notifications,
claude-restart-before-first-turn, settings-save-close-delete-prompt, then shell-tab-first-close-sweep-backoff), after
the landing's fix to claude-restart-before-first-turn's test. Because agent-blocked-notifications raises the protocol
version and the four plans between them touch the helm, the supervisor's agent-specific code, the command line, the
desktop app, its reference screenshots and the docs, the whole Rust test suite ran. The SSH test worker was still
unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- A first run failed at its first step: `cargo clippy --all-targets -- -D warnings` could not compile a new test of
  claude-restart-before-first-turn, the problem described in that plan's notes. The other clippy checks passed and the
  test build then failed on the same error, so no test ran. After the fix, every check below ran from the start.
- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app that runs by default (`cargo nextest run --workspace`,
  built once, split in three and run through the recorder), recorded runs d273d9bc-7896-4c1c-89a4-106d7f060a1d,
  a74ca485-4be5-4e2f-bbf5-ff250895f815 and 3e5673f4-c3a7-461f-9d15-7c0c7956e6a1: 9,180 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache; a boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and no plan in this round touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  8fc4d4bc-523d-4650-856d-e560a8137b59, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  693426e3-0300-4c24-8257-adc94ddb9901: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  fb93b279-02be-42ce-94f6-f269a97c0d79, the test of the screenshot tooling's list of screens, recorded run
  653ebb0b-2dbc-432c-8da0-1f814940b0cb, and the workspace's documentation examples
  (`cargo test --doc --workspace --exclude farhelm-desktop`), recorded run 5dfbb55e-5cbe-4bee-beb4-cac09ef01146: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown,
  `python3 scripts/check-agents-md.py` (the drift-tests document is one the agent instructions name), and the docs
  website build (`bun run build`, which checks every link): passed.

Reused: nothing. Skipped: the opt-in tests, among them the real-agent drift tests, which spend vendor turns; the
installer and provisioning suites, since no plan in this round touches them; and any run on a Mac, since none was
available to the landing.
