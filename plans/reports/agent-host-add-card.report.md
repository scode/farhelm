## What this was about

An agent's request to add a host could open the Add host dialog while someone typed in a terminal. Their next Enter
could then approve the request. Requests now wait as approval cards in every window. Only clicking the card's open
action opens the dialog, so arriving requests leave typing alone.

## Things you should know

Explicit opening keeps the existing approval pause, editable destination, Allow/Deny behavior and setup confirmation.
The setup confirmation still starts on Set up. As agreed, clicking a card while another dialog, unfinished manual add or
add submission is in the way still queues that opening. It can therefore appear later, after the user moved on. The
implementation specification now describes both the corrected arrival behavior and this retained exception.

Two WebKit layout audits initially exhausted their test deadlines while successful assertions were still progressing.
Traces identified repeated browser calls for each text block. Batching those passive observations preserved every
assertion and the existing deadline; all six pane-width and engine cases then passed.

## Open questions and possible follow-ups

None introduced by this plan. The deferred explicit opening above remains intentional.

## The PRs

- [#3326](https://github.com/scode/farhelm/pull/3326/changes): stop agent host-add requests from opening dialogs.

## Checks run, reused and skipped

- `cargo build -p farhelm`, the pinned web build, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, changed-Markdown `dprint check`, the changelog format check and the isolated test-sleep
  checker passed. The sleep checker inspected 429 delays with no missing rationale.
- Recorder-backed approval and modal-isolation Rust unit selection passed 26 tests, with 927 selected out
  (`cc0418e7-ce9f-4457-a059-84651a8e1596`).
- Approval-layout and selected host-add/setup and queued host-dialog browser cases on Chromium and WebKit initially
  passed 68 of 70, with the two layout deadline failures above (`c1497f80-6cbf-4a59-b0fd-d466f0bed950`). A focused
  WebKit diagnostic reproduced both failures and was deliberately interrupted before the remaining repetitions; its
  recorder required forced cleanup (`ffb14e0a-6d3c-4056-be3d-7f704ede25ca`). It is retained diagnostic evidence, not a
  passing run.
- After batching the observations, all six layout audits passed through the standard browser recorder
  (`61e55f43-4f93-4068-8213-b78dbc3b0f6f`). Reusing the 64 unaffected initial cases gives coverage of 70 distinct
  browser cases, including terminal focus and Enter, explicit opening behind another dialog, edited destinations, answer
  receipts, expiry, competing windows and setup refusal/reopening. Successful runs have complete source and report
  capture, stream EOF and cleanup; no runtime SKIPPED marker was present.
- Runtime evidence was captured on base `1ba949ff` with the plan's working changes, then reused after rebasing onto
  `6d0ecc2e`. The plan's interdiff was empty; main's intervening Trash changes affect separate readers and fixtures,
  with no change to host-add approval, modal admission or setup. Formatting and fragment checks passed after rebase.
- Skipped the remaining browser and workspace suites, desktop runtime, hosted CI and cloud checks because the targeted
  checks cover the changed presentation and shared modal admission. No live installation was changed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues, including on the test-observation
correction. It received the full test-authoring checklist. The commit-message cold read passed, and a separate
documentation pass covered every touched file.

### Landing

#### What landed with it

Nothing else landed in this round. While it was being checked, main gained only a planning change that queues four held
plans (#3327), which touches no code.

#### Fixes made while landing

A second, independent review of the diff found the code change correct and two documentation gaps, fixed before merging:

- The docs page "Approve what agents do" still said an add request opens the Add host dialog in your focused window. It
  now says the request waits as a card in every window and that the card's **Open add dialog** opens the dialog.
- The removed TODO entry also named a second hazard, which this plan leaves in place. When you click **Open add dialog**
  while something else is in the way (another dialog, your own unfinished Add host form, or an add still being
  submitted), the dialog opens later, once that clears; you chose to keep that, and the plan kept it working as before.
  Such a late dialog gets no fresh 700 ms pause (the approval pause the report mentions, during which Add cannot be
  chosen). Your decision did not mention the pause either way. If you click **Open add dialog** while the quick switcher
  is open and then close the switcher, the Add host dialog appears ready to answer, and an Enter pressed at that moment
  approves the request. SPEC_impl.md now says so, both in the host-add paragraph and in its list of places the code does
  not yet follow the keyboard focus rules. Nothing in the code changed for it.

#### Possible follow-ups

- Restart the 700 ms pause when a late dialog appears, closing the hazard above. This is the most useful follow-up here.
- A small regression this plan introduced, despite the report saying none: before the dialog appears, the page runs a
  check that waits for any other dialog to close. If the browser fails to run that check (an error in the page itself,
  which should be rare), the opening now stays stuck: no dialog appears, clicking that card's **Open add dialog** again
  does nothing, and later opens wait behind it until the request leaves the list or the page is reloaded. Before this
  plan, such a failure dropped the opening so the card could be clicked again.
- The new test that Enter in the terminal answers nothing checks the recorded answers right after pressing Enter, so it
  would miss an answer sent a moment later. It does prove the regression the plan was about: the old automatic dialog
  would have taken focus from the terminal, and that check fails.
- The ordering test clicks the cards in the same order the list shows them, so it cannot tell click order from list
  order.

#### Checks

Test runs went through the recorder with the pinned tmux first on `PATH`, and none of the runs printed a runtime
`SKIPPED` line. Browser runs were on Tensorlake sandboxes at the maintainer's request, split across ten sandboxes, which
run Node 24.16 against 24.20 locally.

On the plan's code, which the landing fixes did not change:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop` passed, and `node --test` in `crates/farhelm-ui/js-tests` passed 563
  tests.
- `cargo nextest run -p farhelm-ui --lib`: 952 passed (run `91853db2-d845-4d74-a7b0-693e96b4658f`).
- `npx playwright test --project=<engine>-<spec> --repeat-each=2 --retries=0` for every spec that drives the Add host
  form, agent host-add cards or dialog admission (approval-layout, buttons, provisioning, terminal-multihost,
  tooltip-coverage, modal-focus and modal-trap) on Chromium and WebKit: 790 passed and one failed. The failure is
  provisioning's `incarnation churn keeps a followed run while a target change detaches it` on Chromium (run
  `5f478cad-1f63-4604-a132-2b67bbfa1902`), a known flake already listed in `deflake/known-flakes.txt` (a host update's
  progress line sometimes shows its current step's name later than the test allows), which does not involve agent
  requests. The other runs: `1febc000-a323-4f8d-8e07-3636dd90fc7d`, `eea0cbd7-6eee-4ab3-8cc4-bb0b30683e7d`,
  `e118e1a5-ea73-4029-9f54-203fe62b8b68`, `5364333d-a4ab-418d-94c2-1518cf491e35`,
  `36d06ef9-a237-4b06-ab9d-943759b78f6e`, `8e169ecd-5b62-4483-a75d-b0f5923d9a8d`,
  `eb1361d4-f67d-4be4-9d88-f65b0ae24d85`, `35a9f153-da73-413b-8023-7a151f3c96dc` and
  `a8660f97-7e91-4336-82d4-4f48dd203d1f`.

After the documentation fixes: `dprint check` on the two changed files and `bun run build` of the docs website passed.

Reused: nothing from the executor. Skipped: the rest of the browser suite and the workspace's other Rust tests, because
the change reaches only the approval cards and the host dialog's admission; hosted CI (manual-only, not dispatched).
