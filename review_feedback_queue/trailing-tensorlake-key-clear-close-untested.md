# Tensorlake key Clear and a failed key Test: the close-wait routes this commit added are untested

Reviewed commit: ee8ef7294a6638d7905fca7dd27083d0d0bb45c9

## TLDR

Closing Settings while a Tensorlake key Clear is out should wait for it and then close without asking about a typed key
the Clear emptied; closing while a key Test is out that then fails should stay open showing the failure. Neither is
covered, so a regression would let Settings ask about a key that is already gone, hang open, or close over a failed Test
with no test failing.

## Details

The commit adds `form.save_receipt.settled_other(refused, cx)` to the Clear completion (tensorlake_key.rs:534-537) with
the comment "After `empty_later`, so a waiting close sees the field emptied", and `settled_other(failed, cx)` to the
Test completion (tensorlake_key.rs:581-583). That ordering claim is load-bearing: `empty_later`
(tensorlake_key.rs:381-392) defers replacing the input, and the receipt's continuation is deferred after it, so a
resumed close finds the typed key gone and closes without a question. Tests covering the close wait on this form are
`a_close_waits_for_the_switch_and_a_test_on_a_clean_key_form` (tensorlake_tests.rs, Test answered 200 on a clean form)
and credential_tests.rs around 1321-1345 (Test answered 200 with a typed key, then the question). No test presses Escape
or Close while `tensorlake-key-clear` is out (its uses at credential_tests.rs:1352 and
tensorlake_sandbox_tests.rs:334/861/909 do not close), and no test closes while a key Test fails, so the `failed = true`
path (refusal recorded, `CloseRefusal::edit` -> `reveal_edit_failure` for `Edit::Key`) is unpinned. Contrast ntfy, whose
failed-Test close is pinned by `send_test_sends_nothing_of_the_form_and_reports_how_it_went`. Fix: add a test with a
typed key, Clear out, Escape (nothing asked), Clear answered 200 (Settings closes without a question), and one with a
Test answered with an error while a close waits (Settings stays open on the form's page with the outcome line, nothing
asked).

Reviewer revision notes: Revision (turn 2): line numbers are at the pinned tip b2d540a8dacfb94140e5eb0927d78e447ce46111
(at the reviewed commit: empty_later at tensorlake_key.rs:369, Clear receipt at 523-525, Test receipt at 570-571); no
later commit in the range changes these completions or adds the missing tests. Title: A close waiting on the Tensorlake
key form's Clear, or on its Test that fails, is never exercised by a test.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (test coverage; the shipped code looks right by reading).
- Confidence: confirmed (by searching the test files for Clear and Test presses followed by a close).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g03/f2 (finding), group g03 "Settings close waits for outstanding work: spec, panes and tests", findings log line 9.
