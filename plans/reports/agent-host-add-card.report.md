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
