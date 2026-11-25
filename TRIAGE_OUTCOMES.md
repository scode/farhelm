# Triage outcomes

## settings-close-can-retry-refused-save.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "One GUI at a time" (5785): "The GUI never retries or replays a failed save on
  its own." SPEC.md Settings paragraph (2574-2575): "A close that saves waits for pending operations and sends the
  remaining saves one at a time. The first failure keeps Settings open and shows the affected page with its error."
  Code: when a page batch is already running, `ask_to_close` sets only `batch.after` and never watches pending saves
  outside that batch (`editing.rs:636-639,647-653`); a recipe editor's own Save is outside page batches (`is_form`,
  `:166-171,367`). A refused recipe save goes back to an editable dirty draft (`recipes/editor.rs:579-590`) with nobody
  listening; when the page batch finishes, `AfterBatch::SaveAndClose` runs `start_batch(None)` (`:477`), which sends the
  refused recipe draft again. Fix follows directly: in the batch branch also watch out-of-batch pending saves and cancel
  the close (keep Settings open, reveal the failure) on refusal, as `watch_close_question` already does for the no-batch
  branch. Verified: confirmed by inspection. Size estimate ~50-100 lines including a regression test that overlaps a
  recipe save with a page save; may approach the 100-line gate because the test setup is involved.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  direct application of the quoted rules; size S-M (near the gate).). Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings
  the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment
  says remains.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-save-can-hide-credential-error.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md Settings paragraph (2575): "The first failure keeps Settings open and shows the
  affected page with its error." SPEC.md Credentials (2599): "A failed save keeps the draft and shows its reason." Code:
  a key conflict sets the Credentials overview in front of an open credential editor (`editing.rs:408-412` →
  `credentials.rs:305-307`, `key_revealed`); nothing clears that flag when the conflict is later resolved through the
  overview's footer. The next Save and Close still saves the hidden credential (`editing.rs:267-270`); on refusal
  `reveal_edit_failure` only selects Credentials for a `Credential` target (`editing.rs:544-549`), so the overview with
  "Resume editing credential" is shown (`credentials.rs:296-303,570-590`) while the refusal is stored in the unmounted
  editor (`credentials/editor.rs:321`). Fix follows directly: on a credential-editor failure, clear `key_revealed` so
  the editor and its error show, mirroring the key-failure branch. No comment defends the current behavior. Verified:
  confirmed by inspection. Size estimate ~30-60 lines with a regression test; under 100.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  direct; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## tensorlake-key-discard-lacks-preservation-test.md

- Outcome: `other`.
- Assessment: No test checks that pressing Discard on a typed Tensorlake key replacement leaves the saved key alone (no
  request, field emptied, Test still available, the other form's draft kept). Verified: confirmed; no key-form Discard
  assertion in `credential_tests.rs`/`tensorlake_tests.rs` (grep). Code path is a plain local reset (`editing.rs:220`).
  Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code if test gaps are wanted) — no product
  failure; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## ntfy-test-no-longer-witnesses-pending-save-lock.md

- Outcome: `other`.
- Assessment: The Notifications Send test test no longer proves that Save is unavailable on an edited form while a test
  is running; it only checks a clean form, which has no Save anyway. Verified: confirmed. `tests.rs:4145-4155` asserts
  `!drawn(FOOTER_SAVE)` on a just-saved (clean) form; the dirty branch (`:4161-4166`) answers the test before looking at
  the footer. Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code) — no product failure; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## sound-preview-regression-misses-separate-mappings.md

- Outcome: `other`.
- Assessment: The sound preview test presses only the "Turn finished" Play button, so a wrong sound on the other three
  Play buttons would go unnoticed. Verified: confirmed (per-row match arms in `choices.rs:395-` `preview`; single-row
  test). Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code) — no product failure; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## pi-settings-tests-miss-trust-route.md

- Outcome: `other`.
- Assessment: Pi's Settings tests no longer check a successful trust read showing Pi's stored value, or that Pi's page
  has no hint control. Verified: confirmed per reviewer as a coverage gap; no shipped failure claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## agent-settings-tests-miss-late-trust-replies.md

- Outcome: `other`.
- Assessment: No test checks that a trust read or save started on one agent's page settles on that agent after you
  switch to another agent's page. Verified: confirmed as a coverage gap; the product binds replies to the starting agent
  (per reviewer).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## agent-settings-search-test-omits-focus-target.md

- Outcome: `other`.
- Assessment: The Settings search test for an agent's trust result checks the page and label but not which agent's
  switch the result focuses. Verified: confirmed as a coverage gap; the shipped catalog binds both correctly (per
  reviewer).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-masked-input-test-omits-a11y-value-checks.md

- Outcome: `other`.
- Assessment: The test for secret (masked) text fields checks that the typed secret is not painted and not in accessible
  names, but never checks the accessible value that screen readers read, and never runs with accessibility active. A
  future library change could leak the secret to screen readers unnoticed. No leak exists today. Verified: Confirmed on
  main: `components/text_input.rs:209-215` sets `.masked` without a password content type. The reviewer's library trace
  (gpui-component and gpui-base 0.9.1) was not re-read. The suggested code change (password content type) interacts with
  `set_masked` reveal toggling (`:340-342`), so it needs a choice.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code, only if cheap. The guard protects a secret from
  screen readers. Size S to M.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-credential-delivery-test-stale-claude-settings-assertion.md

- Outcome: `other`.
- Assessment: A credential-delivery test still asserts that the old Claude settings file was not created, which is
  always true now, and does not check the files that actually fail or the Codex file after retry. Verified: confirmed.
  `credential_delivery_tests.rs:2076` still has the vacuous assertion; no Codex-after-retry assertion (`:2077-2084`).
  Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code) — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-footer-can-save-the-wrong-page.md

- Outcome: `fix code`.
- Assessment: The Settings footer's Save and Discard act on whichever page is current when the press is handled, not the
  page drawn under the pointer. If the page changes and a press lands before the next frame is shown, Save can save the
  other page's draft, including replacing the one stored Tensorlake API key with the other key form's draft. Verified:
  confirmed by source; timing unmeasured. Both footer closures call `save_page`/`discard_page`, which read `self.shown`
  live (`editing.rs:301-314,337-355`). SPEC.md "One GUI at a time" (5788-5789): "A pointer or keyboard action uses the
  target, consent and preconditions from the frame the user was shown, or is ignored when that frame has not yet been
  shown or its target is no longer in view." SPEC_impl.md 995-999 says the same for controls that send requests
  directly. The fix needs a design choice (capture the section at render vs. a shared `when_shown` gate, since GPUI
  draws a dirty frame before key dispatch), so not clearly settled. Not the sub-second-race filter: the consequence can
  replace a stored credential.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code — bind footer actions to the drawn section and refuse a press
  for an unshown page; with 43; size S-M. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## hook-settings-toggle-uses-unshown-state.md

- Outcome: `fix code`.
- Assessment: On an agent's Settings page, "Set up conversation hooks on all hosts" saves at once. If an earlier attempt
  to turn it on is refused just as you press Space to turn it off, the switch can act on the newly drawn (off) state
  that you have not seen yet and save "on", recording all-hosts consent you meant to withdraw. Verified: confirmed by
  source; timing unmeasured. The switch sends `!self.checked` from the frame being drawn
  (`components/checkbox.rs:225-228`), with no presentation gate; the row passes `switch.state.shown()`
  (`settings/hook_settings.rs:51-55`). SPEC.md 5788-5789 and SPEC_impl.md 995-998 ("a handler for a newly drawn frame
  may have to refuse the press") govern. The fix is in the shared Checkbox or a per-switch snapshot, a design choice.
  Not the sub-second-race filter: the consequence is a consent (trust-boundary) write.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code — gate switch toggles on the presented frame (shared Checkbox
  change); size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal
  or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-footer-can-discard-the-wrong-page.md

- Outcome: `fix code`.
- Assessment: When a Settings save is refused, Settings jumps to the refused form's page. A Discard press that was aimed
  at the footer of the page you were on, landing before the new page is shown, discards the refused page's draft
  instead, before you have seen its error. Verified: confirmed by source. A refusal takes the batch and calls `show` for
  the failing section (`editing.rs:457-468,544-562`); the footer Discard reads `self.shown` at press time
  (`editing.rs:301-310,337-339`). Same governing text as 7. Not the sub-second-race filter: a discarded draft is neither
  a refillable cache nor convenience history, which that filter requires.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code — with 7 (same binding); size S. Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-can-save-unlisted-drafts.md

- Outcome: `fix code`.
- Assessment: When you close Settings while another Save is still going, Settings asks "Save changes before closing?"
  and names the unsaved forms. If you answer Save, Settings waits for the earlier save and then saves whatever is
  unsaved at that moment, including a form you edited after answering (for example a Notifications server change, which
  by the normal rule clears the saved ntfy access token). Discard can likewise drop a draft that appeared after the
  question. Verified: confirmed. The Save answer captures nothing (`editing.rs:635-643`); with a batch running it only
  sets `AfterBatch::SaveAndClose`, which later calls `start_batch(None)` (`editing.rs:477`), and that re-reads the live
  form list (`editing.rs:364`). Settings stays interactive after the question closes; forms waiting behind another save
  are editable (ntfy locks fields only while its own save runs, `ntfy.rs:21,368`). SPEC_impl.md "Questions: the prompt"
  (lines 984-985): "When a confirmation opens, it freezes what it shows: targets, consequences and preconditions. Its
  answer buttons come pre-wired to that frozen state." The ntfy form does show "Changing the server clears the saved
  token when you save" beside such an edit (`ntfy.rs:648-656`). The Codex late-file-read variant depends on picker
  timing (unmeasured).
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — freeze the listed edits (entity ids) in the answer and
  leave later edits unsaved with Settings open; one fix with 6 and 47; size M. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-queued-save-can-commit-later-draft.md

- Outcome: `fix code`.
- Assessment: A Settings Save (page Save or Save and Close) that covers several forms saves them one at a time. A form
  waiting its turn is saved with whatever it contains when its turn comes, so edits made after pressing Save are saved
  too; for Notifications that can include a server change that clears the saved access token. Verified: confirmed. The
  batch stores form identities only (`editing.rs:229-239`) and each queued form is saved from its live contents when
  dispatched (`editing.rs:489-530`, e.g. `save_from_settings` for the second Checkouts field). No version of the
  authorized draft is kept. Same governing text as 5 (SPEC_impl.md 984-985 for the close question; for a page Save no
  spec sentence speaks to queued edits).
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — record each form's draft revision at authorization and
  skip/stop on a changed draft; fix with 5; size M. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-can-miss-detached-credential-save.md

- Outcome: `fix code`.
- Assessment: If you press Save in a credential editor and then leave it with Back or Cancel while the save is still
  running, Settings forgets about that save. Closing Settings can then offer Discard and close without waiting, so a
  refusal of the credential save is never shown. Verified: confirmed. Back and Cancel stay active while saving
  (`credentials/editor.rs:507-517,608-619`) and drop the editor (`leave`, `:465-476`); the close inventory sees
  credential saves only through the mounted editor or a batch's `active` list (`editing.rs:267-283`). SPEC.md Settings
  paragraph (2573-2574): "Discard is absent while a save is in flight. A close that saves waits for pending operations".
  The fix needs a place to track admitted saves independently of editors (operation store or dialog), a design choice.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — track admitted Settings saves by operation key so the close
  question sees them; size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-can-miss-conflicting-tensorlake-keys.md

- Outcome: `fix code`.
- Assessment: The Tensorlake API key can be typed in two places (Tensorlake and Credentials). If one key save is running
  and the other form holds a different key when you choose Save and Close, Settings waits, then saves the second key
  over the first instead of stopping on Credentials to ask which key you want. Verified: confirmed by source. With a
  batch running, the close answer only records `SaveAndClose` (`editing.rs:637-639`); the conflict check in the later
  `start_batch(None)` needs both typed keys (`:394-406`), and the first key's field has cleared after its save. SPEC.md
  2575-2576: "Two different unsaved Tensorlake keys leave Settings open on Credentials with a message to save the
  desired key first." Whether a key whose save is in flight counts as "unsaved" is a reading (the close question itself
  lists it as unsaved).
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — run the key-conflict check when the close answer is given;
  size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-can-save-newly-staged-codex-login.md

- Outcome: `fix code`.
- Assessment: In the Codex login editor you can pick an `auth.json` file. If the file is still being read when you close
  Settings and the question appears, the read can finish behind the question; answering Save then saves the chosen file
  rather than the text that was shown when you asked to close. Verified: partly correct, timing unverified. The file
  read completes asynchronously and switches the draft if nothing was typed meanwhile (`credentials/editor.rs:398-447`);
  the close question captures no draft revision (`editing.rs:572-656`). Save does stop a pending read once it is
  dispatched (`:289-290`), but not one finishing while the question is open. The saved file is one the user picked
  themselves.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code with 5/6 (draft revision binding covers it); size S on top
  of 5. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-can-hide-credential-delivery-warnings.md

- Outcome: `fix code`.
- Assessment: Replacing a saved credential pushes it to hosts that use it; a plain Save shows "Saved; not delivered to
  <host>" lines on the Credentials overview. If the replacement is saved through Save and Close, Settings closes and
  those per-host lines are never shown. Verified: confirmed. A done credential save passes delivery failures to the
  overview and signals success (`credentials/editor.rs:310-320`); the batch then closes Settings when nothing is left
  (`editing.rs:481-483`). SPEC.md 2601-2602: "A successful save with failed deliveries closes and clears the secret
  editor, and shows the delivery failures on the Credentials overview"; SPEC.md 535-536 has the save report them. Each
  failing host still turns amber with the reason in its details (SPEC.md 532-533), so the information survives
  elsewhere. Whether Save and Close must stop for these warnings is a reading.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — keep Settings open on Credentials when a close-save returns
  delivery warnings; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## hook-settings-close-hides-opt-out-failure.md

- Outcome: `fix code`.
- Assessment: Turning off "Set up conversation hooks on all hosts" saves at once. If you close Settings before that save
  settles and the helm refuses it, nothing tells you; the setting stays on and later agent starts keep installing hooks.
  Verified: confirmed by source. The close inventory lists forms and editors only (`editing.rs:251-286`) and closes at
  once when it is empty (`:577-579`); the switch's failure lives in the dialog's own state (`hook_settings.rs:57-62`).
  SPEC.md "Errors and diagnostics" (4944-4945): "Every failed operation surfaces a concrete, actionable error ... A
  dialog must never close as though an operation succeeded when it failed", with named exceptions for some switches
  (4963-4969) that do not include hook setup. Other switches without a named exception (trust switches) likely share the
  gap; the fix shape (hold the close, or a surviving notice) is a design choice.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — let close wait for or surface pending switch saves (or
  decide a spec exception per switch); size M. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-custom-agent-settings-close-save-silent-fail-during-delete-confirm.md

- Outcome: `fix code`.
- Assessment: In Settings → Custom agents, the user clicks Delete on an edited agent, which shows the delete
  confirmation. They then close Settings and choose Save in "Save changes before closing?". The save silently does
  nothing. Settings stays open on that page with no error, and the remaining unsaved forms are not saved either.
  Verified: Confirmed. `Editor::save` returns `false` when `confirm_delete` is set, without setting an error
  (`crates/gui/farhelm-gui/src/settings/custom_agents.rs:316-318`). The close batch treats that as a failure and reveals
  the page (`settings/editing.rs:425-470`). SPEC.md 2575 says "The first failure keeps Settings open and shows the
  affected page with its error". Here there is no error. Not executed.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code. Either have Save clear the open confirmation and proceed, or
  refuse with an error. There are two reasonable shapes, which is why this is not clearly settled. The fix is subsumed
  if 80 moves the confirmation to the prompt. Size S. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-save-can-hide-notification-refusal.md

- Outcome: `fix code`.
- Assessment: When Save and Close stops because the ntfy alerts save was refused, Settings switches to Notifications at
  the top of the page. The refusal sits below the four Sounds rows and the ntfy form, which at small window heights is
  below the visible area. Verified: partly correct. `reveal_save_failure` only switches section (`editing.rs:551-562`),
  which scrolls to the top; it does not reveal the failing form. Whether the error is out of view at ordinary window
  heights was not verified (the reviewer computed it for the 560px minimum). SPEC.md 2575: "shows the affected page with
  its error." The "Dialog explanations out of view" filter needs an unusually small window; since overflow at ordinary
  heights is unverified, it is unclear, so it does not match.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — scroll the failing form into view on a close-save failure;
  size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-save-stalls-silently-during-tests.md

- Outcome: `fix code`.
- Assessment: If Notifications' Send test or Tensorlake's Test is running when you choose Save and Close, the form
  refuses to save because it is busy; Settings stays open on that page with no message, so it looks as if nothing
  happened. Verified: confirmed. The close question counts only `Saving` as pending (`editing.rs:594-597`); a locked
  form's save returns false (`ntfy.rs:368,481`; Tensorlake key `holds_save`), which `dispatch_edit` treats as a failure
  with no text (`editing.rs:432-434,457-468`). SPEC.md 2574-2575 asks for sequencing or showing the failure; whether to
  wait for the test or explain the refusal is a design choice. Not the rare-glitch filter: a test can run for seconds
  (not a narrow window).
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code — treat a held form as pending (wait for the test) or show a
  busy line; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## settings-close-can-dismiss-a-newer-question.md

- Outcome: `fix code`.
- Assessment: After you answer Save in the close question, Settings stays interactive until the saves finish. On a Mac,
  if you meanwhile flip the tunnels switch, its warning opens; when the saves finish, Settings closes and takes that
  newer, unanswered warning with it. Nothing is written; you just have to open Settings and flip the switch again.
  Verified: confirmed by source for the Mac path (per reviewer's trace; layers cleanup not re-traced). `finish_close`
  closes the layer unconditionally (`editing.rs:564-568`). SPEC_impl.md "Work that finishes later" (1091-1092): "A
  completion that would focus, open, select or close something asks the focus coordinator whether the user has moved
  on." What "moved on" should mean after an explicit Save-and-Close answer is a design choice. The window lasts the
  credential save (up to the 20 s delivery bound), so the rare-glitch filter's "narrow window" is unclear and does not
  apply.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: discard — the user asked to close and then started a new action;
  nothing is lost; size S if fixed. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-custom-agent-settings-busy-save-button-and-saw-saved-restore.md

- Outcome: `fix code`.
- Assessment: While a custom-agent save is in flight, an update pushed from the helm can turn the busy Save button into
  a disabled one, so it drops keyboard focus. A second push can overwrite the fields mid-save. Verified: Partly correct.
  The rule exists: SPEC_impl.md "Drafts" says "While a save is in flight its Save button is busy rather than disabled".
  The button is `.disabled(!this.dirty(cx)).busy(busy)` (`settings/custom_agents.rs:580- 583`), and `saw_saved` ignores
  `busy` (`custom_agents.rs:301-310`). In normal saves dirty stays true until the reply, so the rule holds. Part 1 needs
  a frame between the save's own pushed change and its reply. That is unverified, and item 81 assumes the opposite
  ordering. Part 2 needs a second write from another client, which no shipped control produces (see the header).
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: discard. The established part is timing-dependent and costs only
  focus, and the rest has no shipped route. Alternatively, a one-line `disabled` fix. Size S. Size gate: about 100 lines
  of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-custom-agent-settings-dirty-close-prompt-collapses-multiple-agents.md

- Outcome: `fix code`.
- Assessment: With unsaved edits on two custom-agent pages, or on Add custom agent and one agent, the close prompt lists
  a single "Custom agent". It also switches Settings to whichever draft sorts first rather than leaving the user on
  their page. Verified: Confirmed. `Edit::name` returns "Custom agent" for every custom-agent edit
  (`settings/editing.rs:141`), and `ask_to_close` deduplicates by name, then reveals `edits[0]` when one name remains
  (`editing.rs:585-593`). SPEC.md 2573 says "asks once with the affected forms named". Whether one name per form kind
  satisfies that is a reading. Save and Discard still cover every draft.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code. Reveal a page only when one section is affected, and name
  each custom-agent draft through a host sentence. Size S. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## agent-settings-search-omits-agent-name.md

- Outcome: `other`.
- Assessment: In Settings search, results for different agents' pages (for example the "no settings" entry of several
  agents) have the same accessible name, so a screen-reader user hears identical options and cannot tell which agent
  each opens. Verified: confirmed. Each result option's accessible name is the entry label alone (`search.rs:789-799`);
  the agent appears only in a separate heading row (`:690-697`). Screen-reader speech not verified. SPEC.md 2585-2590
  describes grouping but not option naming; reading.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code — include the section name in each option's accessible name; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## sound-preview-overlay-blocks-settings-switch.md

- Outcome: `other`.
- Assessment: Each sound switch in Notifications has a small Play button. The invisible strip above and below that
  button, as tall as the row, swallows clicks, so clicking there neither plays nor toggles the switch. Verified:
  confirmed. The wrapper is `absolute().top_0().bottom_0()` with `block_mouse_except_scroll()` (`choices.rs:723-735`),
  wider in height than the 24 px button. The dead area is about 24 px wide.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — move the blocking hitbox to the button; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## app-settings-shared-upgrade-option-has-local-heading.md

- Outcome: `other`.
- Assessment: Settings → App's heading says its settings apply to "This Mac", but the page also holds "Show what's new
  after updates", which the helm shares. Verified: confirmed per reviewer (frame scope mapping not re-traced). SPEC.md
  2563: the heading "shows its icon, name, description and scope"; SPEC.md 2687-2690 places both an installation setting
  and the helm-shared choice in App. The spec does not say what scope label a mixed page shows, so this is a reading.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard or fix spec (say App's scope is the installation's, with the shared switch saying so itself);
  size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-settings-containers-reload-omits-loading-keeps-error.md

- Outcome: `other`.
- Assessment: In Settings → Containers, after a leftovers refresh failed, pressing Refresh again (or deleting the last
  listed leftover) shows no "Reading container leftovers…" while the helm checks (up to 20 s); only the old error stays,
  as if the new check had already failed. Verified: confirmed. Loading text shows only for `(None, None)`
  (`containers.rs:512-520`); `start_round` and the Delete follow-up read leave `leftover_error` set
  (`:174-184,350-358`). Self-corrects when the read answers. Not the rare-glitch filter: a retry after a failed refresh
  is not a rare trigger.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — track a reading flag and show the status; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md

- Outcome: `other`.
- Assessment: In a host's settings, flipping the Tailscale switch while a session launch is starting shows a bare
  lowercase fragment ("not started, a session launch is in progress…") without naming the setting, can show it beneath
  an older Tailscale error, and can leave it on screen after an earlier Tailscale save succeeds. Verified: confirmed.
  `hosts/settings.rs:759-764` sets the flag and returns; the row draws the bare `LAUNCH_IN_FLIGHT` (`:960-966`) beside
  the draft's refusal (`:957-959`); nothing clears the flag when a save settles. Diagnostic/stale display only.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — prefix, suppress the old error, clear on settle; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-settings-trust-help-omits-agent-requested-launches.md

- Outcome: `fix code`.
- Assessment: The workspace-trust switch help on the Codex, Muse, Pi and Antigravity Settings pages says it applies to
  New, Clone, Restart with and Replace with, but not that it also applies to launches an agent requests. On a host where
  agent requests are approved without asking, turning it on lets agents start trusted sessions anywhere, which the help
  never mentions. Claude's help does mention it. Verified: confirmed. `agent_trust.rs:212-224,239` omit agent-requested
  launches; Claude's text at `:205` includes "including launches an agent requests" (dropped for the others by
  4aff3f1b). SPEC.md 1852-1853: "New, Clone, Restart with, Replace with and agent-requested launches apply the current
  setting in any directory." The spec requires the help to explain that hooks and commands may run (1856-1857) but does
  not dictate listing launch routes, so this rests on a reading. Not covered by `drift-workspace-trust.md` (that plan
  adds live drift tests, not Settings copy).
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code — restore the agent-requested wording and refresh goldens; size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md

- Outcome: `fix code`.
- Assessment: In Settings → Tensorlake, saving a different API key over the saved one keeps showing the old key's
  "Sandboxes not on this helm" while the new list loads, and keeps them (with Delete buttons) under "Could not refresh"
  if the new key cannot list sandboxes. Verified: confirmed. `follow_account` re-reads without clearing cached rows when
  a key stays saved (`tensorlake_sandboxes.rs:159-166`); a failed discovery keeps `last_discovered` (`:241-246`). The
  module docs say "A key change invalidates that evidence too" (`:21-22`). SPEC.md 613-614 ("above previously discovered
  sandboxes") does not distinguish keys, so a reading. Delete still confirms by name and targets the vendor ID
  (reviewer), so no wrong deletion is established.
- Decision: q-staleconf, "Confirmations and kept state whose subject is gone": the maintainer picked "Close or
  invalidate them". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A confirmation or kept state whose subject is gone (host removed, listing emptied, account key
  changed) closes or is invalidated. Recommendation for reference: fix code — clear cached discovery on a key
  replacement; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

