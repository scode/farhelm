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

## trailing-custom-agent-settings-launch-preview-breaks-bidi-isolation.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md "Host-supplied text": "Host text is never concatenated with the app's words
  in any other way." The code puts the command and Farhelm's flag words into one host value:
  `HostText::from_host(format!("{command} [--model <model>] [--effort <effort>]{tail}"))`
  (`crates/gui/farhelm-gui/src/settings/custom_agents.rs:633-643`). The fix follows directly: draw the command as its
  own host value and the flag words as app text in the same host sentence. It needs no spec change, and the code carries
  no comment marking this as deliberate. One small detail remains: whether the app words keep the terminal font, which
  styling the preview's container covers. Estimated diff under 30 lines including a test, well under 100.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes. Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-custom-agent-settings-omits-per-field-unsaved-dots.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, Settings dialog paragraph (line 2569): "A dot marks each edited field and
  section." The custom-agent editor always passes `None` for the edited marker
  (`choices::labelled(label, id, field, None)`, `crates/gui/farhelm-gui/src/settings/custom_agents.rs:462`), and its
  capability switches have none (`custom_agents.rs:466-497`). The recipe editor shows the pattern
  (`settings/recipes/editor.rs:782-794`, passing `edited.then_some(palette)` to `editing::caption`). The fix compares
  each field with the stored registration, or with empty for Add, and passes the marker. It needs no design choice and
  overturns no commented choice. Estimated diff 30-70 lines with a test, under 100.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes. Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-launcher-custom-agent-yolo-pill-omits-escalation-outline.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Sessions" → "Creation" (lines 1166-1171): "When an agent switch automatically
  makes the shown permission more dangerous, a red banner slides down ... The permission control gets a thick red
  outline that fades after about two and a half seconds. Reduced motion replaces slide and fade with a static outline
  while the banner is shown." The code contradicts it. Switching to a custom agent raises the banner:
  `omitted_permission(Custom) = Yolo` (`crates/farhelm-proto/src/launch.rs:394`) feeds `escalation_since`
  (`launcher/draft.rs:1021-1045`). But `custom_agent_fields` draws the YOLO pill as a plain div
  (`launcher/view/choices.rs:286-290`) without `with_escalation_outline`. Every built-in agent, including YOLO-only
  ones, gets the outline through `permission_row` (`choices.rs:558`). The fix follows directly: wrap the pill. Verified:
  Confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — pass
  `cx` and wrap the pill in `with_escalation_outline`, plus one view test; size S (well under 100 lines).). Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry,
  or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-settings-custom-agent-nav-tooltip-omits-bidi-isolation.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "Host-supplied text" (lines 1192-1197): "Host text is never concatenated
  with the app's words in any other way", and "Hover help may name a host value ... only as a host sentence". The code
  contradicts it: `crates/gui/farhelm-gui/src/settings/frame.rs:291,312` builds
  `HoverHelp::tooltip(format!("Show {label} settings"))` from `display.accessible_text()`, which embeds a custom agent's
  display name into an app string. The fix follows directly: build the tooltip as a `HostSentence` for the custom-agent
  section. Verified: Confirmed by reading the code. Escaping already happens, so only direction isolation is missing.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — use
  `HoverHelp::sentence` for `Section::CustomAgent`; size S.). Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says
  remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-templates-editor-allows-generic-custom-on-agent-template.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Templates" (line 1338): "Incompatible fields stay visible and prevent saving
  until corrected or removed." On an Agent template, the editor itself marks generic Custom as incompatible: it offers
  generic Custom only for Command templates (`templates/editor/rows.rs:302-304`) and labels it "Custom (not offered)"
  otherwise (:313-317). Yet `Field::refusal` (`templates/fields.rs:229-295`) does not refuse it, so Save succeeds. The
  fix follows directly, mirroring the existing refusal for a shell agent on a Command template (`fields.rs:237-242`).
  Verified: Confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — add
  the refusal and an editor test; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-ui-channel-tests-omit-custom-agents-mutation-round-trip.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "What is pushed, and how changes are found" (line 444): "Enforced by a
  round-trip contract test per collection". The per-collection contract test,
  `every_pushed_collection_round_trips_through_its_proto_type` (`crates/farhelm-helm/src/ui_channel_tests.rs:3042`
  onward), decodes and re-encodes each served collection but not custom agents. `Collection::CustomAgents` appears only
  in the empty-snapshot assertions (:1433, :1474). The fix follows directly: register one agent, serve
  `/api/custom-agents`, and `round_trip` it into `CustomAgent`. The spec does not require the finding's wider proposal
  (a live create, edit and delete push test over the UI channel), so execution should stop at the round-trip entry
  unless the maintainer wants more. Verified: Confirmed by reading the tests. No runtime bug is claimed.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — add
  custom agents to the round-trip test; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-custom-agent-settings-test-locks-in-kick-and-misses-dirty-delete.md

- Outcome: `other`.
- Assessment: The Settings test for custom-agent lifecycle asserts the jump to Add custom agent on an external delete
  (item 82). It also deletes only a clean registration, so it misses the ghost draft of item 81. Verified: Confirmed by
  inspection (`crates/gui/farhelm-gui/src/settings/tests.rs:272-373`, the assertion at line 330). The first half asserts
  behavior with no shipped route (82 is filtered). The second half is a missing test for 81's bug.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: other. Add the dirty-delete case as part of 81's fix, and
  discard the rest. Size S.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-settings-custom-agent-duplicate-host-sentence-id.md

- Outcome: `other`.
- Assessment: When a custom agent's Settings page is open, its sidebar entry and its page heading both draw a label with
  the same element ID. Screen-reader nodes are unaffected, but the app's test and automation lookups, which match on the
  bare ID, become ambiguous. Verified: Confirmed by reading the code. `Section::display_label` hard-codes
  `custom-agent-name-{id}` (`settings/custom_agents.rs:52`), and `settings/frame.rs` calls it for both the nav row
  (:290) and the heading. The reviewer's analysis of the vendored GPUI shows the accessibility IDs are scoped and do not
  collide; only the flat driver ID collides. This breaks the `HostSentence` usage rule in `host_text.rs`, not a spec
  rule. No user-visible effect.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — no user-visible effect (or fix code with a slot
  prefix, size S).
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-custom-agent-settings-delete-confirmation-bypasses-ask-inline.md

- Outcome: `fix code`.
- Assessment: Deleting a custom agent in Settings asks for confirmation with two plain footer buttons rather than
  Farhelm's question control. Focus does not move to Cancel. Escape closes Settings, or asks to save, instead of
  cancelling the delete. A failure to start the delete replaces the question text while the buttons stay. The question
  does not name the agent. Verified: Confirmed. `Editor::footer` renders raw `custom-agent-keep` and
  `custom-agent-confirm-delete` buttons (`custom_agents.rs:500-595`), and Delete reads live `self.stored`
  (`custom_agents.rs:391-392`). SPEC_impl.md "Questions: the prompt" says "Owner: the prompt, the one way to ask the
  user a question", and that an inline question "takes focus on its starting answer" and "Escape on it is Cancel and
  does not reach the owner's dialog". The recipe editor's delete uses `prompt::ask_inline(...).starts_on_cancel()`
  (`settings/recipes/editor/leaving.rs:270-287`). There is no established wrong-target route, because each editor is
  keyed by an immutable ID. This is nearly clearly settled. It is held back because the fix still needs choices: the
  question's wording, whether to start on Cancel, and how it interacts with 79.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code. Port the delete question to `prompt::ask_inline` as the
  recipe editor does, which also settles 79. Size M (about 60-120 lines with tests, so it may exceed 100). Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-custom-agent-settings-delete-dirty-leaves-ghost-draft.md

- Outcome: `fix code`.
- Assessment: If the user edits a custom agent and then deletes it without saving, the agent disappears from the
  launcher but stays in the Settings sidebar as an unsaved draft. Closing Settings then asks whether to save "Custom
  agent", and choosing Save registers the deleted agent again. Verified: Partly verified. The code is confirmed. The
  delete's completion clears `deleting` (`custom_agents.rs:416- 423`). The render-time sync keeps any editor that is
  `!deleting && (saving || dirty)` (`custom_agents.rs:128-136`), and a removed editor's Save POSTs a fresh registration
  (`custom_agents.rs:326-336`). Delete is offered while dirty (`custom_agents.rs:544-556`). One step is unverified: that
  the removal push and the delete's reply are both handled before the next frame. If a frame falls between them, the
  editor is dropped correctly. The reviewer's ordering claim (`crates/farhelm-helm/src/ui_channel.rs:1082-1100` sends
  Changes then Reply back to back) makes that common, but it was not run.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code. Discard the editor on a confirmed delete in the completion
  itself, and add the dirty-delete test (487's second half). Size S. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-custom-agent-settings-save-delete-collapses-retired-and-missing-store.md

- Outcome: `fix code`.
- Assessment: When a custom agent's save or delete cannot start, Settings always says "Another change to this custom
  agent is in progress". That includes the case where the connection to the helm is being reset, and the busy case does
  not use Farhelm's shared wording. With no connection at all, Save or Delete silently does nothing. Verified:
  Confirmed. Both paths turn any `Err` into that message (`settings/custom_agents.rs:378-388`, `425-435`) and return
  silently when there are no operations (`custom_agents.rs:338-340`, `401-403`). The busy wording is settled in so many
  words by SPEC_impl.md "Work that finishes later: the operation stores": "it gives one shared reason
  (`NotStarted::BUSY_REASON`) after a lead-in of its own that names what did not happen ("Not saved:", "Not started:")".
  The reset and no-connection cases rest only on SPEC.md "Errors and diagnostics" in general, and need a wording choice.
  The reset case can occur only briefly, while the window is inert.
- Decision: q-settings, "Settings: what Save and Close saves, waits for and shows": the maintainer picked "Fix them all
  to the spec's contract". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Save and Close binds to the forms and draft versions it listed, waits for every pending save,
  stops on a refusal and shows it; every Settings page, the custom-agent page included, follows the shared Settings and
  prompt contracts. Recommendation for reference: fix code. Use "Not saved:" or "Not deleted:" plus `BUSY_REASON` for
  Busy, and give the other two cases a plain reason. Size S. The busy half alone would qualify as clearly settled. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-custom-agents-share-one-remembered-wrapper-prefix.md

- Outcome: `fix spec+code`.
- Assessment: The launcher remembers the last wrapper prefix per agent type and fills it in when that type is chosen.
  All registered custom agents share a single memory. A wrapper used with one custom agent is therefore pre-filled for a
  different one, and launching another custom agent without a wrapper erases the first agent's remembered wrapper.
  Verified: Confirmed. Memory is keyed by `LaunchHarness` alone (`crates/farhelm-helm/src/store.rs:263-274`), and the
  launcher asks for `LaunchHarness::Custom` (`launcher/view/choices.rs:136`). Whether this is a defect depends on
  reading the spec. SPEC.md 1178-1180 says "remembered per agent type". SPEC_impl.md says a session's agent "is always
  its agent type's word or `custom`", and SPEC.md 4808 lists "built-in types followed by the helm's registered custom
  agents", which reads as if "custom" were one type. The pre-filled wrapper is visible as a pill on the folded More
  options row (SPEC.md 1509-1510).
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix spec+code if per-registration is wanted: key the memory by custom-agent ID, prune it
  on delete, and reload on a custom-to-custom switch. That covers 83, 84 and 85 in one change, size M. Otherwise fix
  spec only, to say custom agents share one memory. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-custom-to-custom-switch-retains-previous-wrapper-prefix.md

- Outcome: `fix code`.
- Assessment: Switching the launcher from one registered custom agent to another, by chip, search or template, keeps the
  first agent's wrapper prefix and marks it as the user's explicit choice. A built-in agent switch would load the new
  type's remembered prefix instead. It also gives no notice when the model or effort the user chose is cleared.
  Verified: Confirmed. `switch_agent` runs its cleanup only when the harness changes
  (`crates/gui/farhelm-gui/src/launcher/draft.rs:825-836`). `choose_custom_agent` clears model, effort and conversation
  but not the wrapper (`draft.rs:759-767`), and template replay does the same (`templates/apply.rs:519-529`). Whether
  this is a defect rests on the same spec reading as 83.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: Fold into 83's decision. Size S on top of 83. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-delete-custom-agent-leaks-remembered-wrapper-prefix.md

- Outcome: `fix code`.
- Assessment: Deleting a custom agent leaves its remembered wrapper prefix in place, so the wrapper is pre-filled for
  another custom agent or for one registered later. Verified: Confirmed, but it is a consequence of 83's shared key.
  `delete_custom_agent` deletes only the row, and its comment says that is deliberate ("Drop only the registration",
  `crates/farhelm-helm/src/custom_agents.rs:94-106`). With one shared memory there is nothing per-agent to prune.
  Pruning only matters if 83 keys the memory by ID.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: Fold into 83: prune on delete only if the memory becomes per-ID. Otherwise discard. Size
  S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-templates-editor-hides-custom-effort-and-refuses-agentless-modifier.md

- Outcome: `fix code`.
- Assessment: In Templates, a template that sets a custom-agent effort word without an agent type cannot be saved from
  the editor or from Save as template, although the helm accepts it. This kind of template is meant to stack onto a
  custom agent template. When such an effort sits on a template with no agent or a built-in one, Effort displays
  "Default" above the refusal. When Effort is a dropdown, choosing Default does not clear it. Verified: Confirmed.
  `Field::refusal` refuses any non-built-in effort when the agent is not custom
  (`crates/gui/farhelm-gui/src/templates/fields.rs:281-293`). `check_template_shape` deliberately accepts it for
  agentless templates (`crates/farhelm-proto/src/launcher.rs:579-594`, comment "A modifier with no agent can acquire its
  harness from an earlier template"). The effort row collapses a custom value to `known()` = None, which matches
  "Default" (`templates/editor/rows.rs:326-338`). SPEC.md 1338 ("Incompatible fields stay visible and prevent saving")
  supports showing the real value. Whether agentless custom effort should be savable from the GUI is not stated.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code. Accept a valid agentless custom effort, and show a stored custom value as
  "<value> (not offered)". Size S-M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-templates-editor-hides-custom-yolo-control-and-saves-false.md

- Outcome: `fix code`.
- Assessment: In Templates, adding Approvals to a command template that declares a custom agent starts it at "Keeps
  safety checks". The editor shows only "Custom agents always run in YOLO mode." and still saves YOLO as off. Every
  apply of that template is then refused, and the editor offers no control to fix it. Choosing a custom agent also adds
  an Approvals field the user did not ask for. Verified: Confirmed. `Field::Yolo.add` sets `Some(false)`
  (`templates/fields.rs:148`). The custom case renders only a hint (`templates/editor/rows.rs:341-344`). Neither
  `Field::refusal` nor `check_template_shape` refuses the combination, while `apply_template` does
  (`crates/farhelm-proto/src/launcher.rs:1103-1113`). Choosing a custom agent sets `yolo = Some(true)` even when the
  field was absent (`rows.rs:423-426`). This overlaps 469 heavily.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code, together with 469 and 450. Size S-M. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-apply-template-custom-command-yolo-diverges-from-gui.md

- Outcome: `fix spec+code`.
- Assessment: Applying a command-launch template that declares a custom agent behaves differently in the desktop app and
  from the CLI. The app silently sets YOLO on. The shared rule the CLI uses does not, so a template without an explicit
  YOLO answer fails from the CLI. Stacking templates can also flip a later built-in command to YOLO in the app.
  Verified: Confirmed. `choose_command_agent` forces `yolo = Some(true)` for a custom agent and never restores it
  (`crates/gui/farhelm-gui/src/launcher/draft.rs:1062-1069`), and template replay calls it (`templates/apply.rs:514`).
  `apply_template` refuses only `fields.yolo == Some(false)`, not an inherited `false`, and never sets true
  (`crates/farhelm-proto/src/launcher.rs:1103-1113`). SPEC.md 1281-1284 requires templates to have "the same effects as
  making them by hand". SPEC.md 494-496 ("Either declaration counts as YOLO") does not say whether the YOLO answer is
  implied. It fails safe, since it only ever flips toward YOLO, which asks first.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix spec+code. State whether a custom declaration implies YOLO, then make the fold and
  the GUI agree. Size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-check-template-shape-allows-custom-agent-yolo-false.md

- Outcome: `fix code`.
- Assessment: Both the CLI and the Templates editor can save a command template that declares a custom agent with YOLO
  off. Farhelm refuses to apply that template every time. In the desktop editor the stored "off" is hidden behind the
  "always run in YOLO mode" message. Verified: Confirmed. `check_template_shape` has no such check
  (`crates/farhelm-proto/src/launcher.rs:555-660`). `apply_template` (`launcher.rs:1103-1113`) and stored-launch
  validation refuse it. The CLI path relies on `check_template_shape`, per the reviewer (`agent_requests.rs`; not
  re-read). The GUI half is item 93.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code. Refuse the combination in `check_template_shape`, together with 93. Size S.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-check-template-shape-skips-validate-model-on-custom-agent.md

- Outcome: `fix code`.
- Assessment: Saving a custom-agent template from the CLI or the API accepts a model the custom-agent rules forbid, such
  as uppercase letters or spaces. The desktop editor refuses the same value, and so does every apply. Verified:
  Confirmed. `check_template_shape` validates a custom effort but not the model
  (`crates/farhelm-proto/src/launcher.rs:579-594`). `apply_template` validates the model (`launcher.rs:987-989`), and so
  does the GUI (`templates/fields.rs:243-259`). SPEC.md 493-494 defines the value grammar, and SPEC.md 4801 says agent
  template writes "go through the same path as the GUI's template editor". Neither says in so many words that a save
  must refuse it.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code. Apply the model check alongside the existing effort check. Size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-custom-agent-restart-readiness-none-misreports-no-resume-command.md

- Outcome: `fix code`.
- Assessment: For a custom agent session whose registration did not enable resumption, the greyed-out Restart with entry
  says "Custom sessions can't be resumed". The accurate explanation is that this launch has no resume command, so the
  user should use Clone or Replace with. Verified: Confirmed. `restart_readiness` returns `None` for Custom ("The custom
  agent's caller decides when to report", `crates/farhelm-proto/src/lib.rs:1749-1750`), against its own doc ("`None` for
  kinds with no conversation reporting", `lib.rs:1718-1719`). `restart_with_reason` checks that first
  (`crates/gui/farhelm-gui/src/launcher/entry.rs: 369-370`). SPEC.md 2213-2214 requires a tooltip "explaining why". The
  trigger is common for these sessions, so the rare-glitch filter does not match.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code. Check `NoResumeCommand` before the readiness shortcut, and correct the doc.
  Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-launcher-target-names-command-custom-agent-outside-restart-with.md

- Outcome: `fix code`.
- Assessment: On the launcher's Command tab, the main action's accessible name names a declared registered custom agent
  ("Launch Command · <agent> · <host> · <folder>") in New, Clone and Replace with. A declared built-in agent or generic
  Custom is not named there; it is named only in Restart with. Verified: Confirmed. In
  `crates/gui/farhelm-gui/src/launcher/view.rs:1792-1803`, the `harness` match is gated on `Mode::RestartWith` but the
  `custom_agent` match is not, and the doc comment (:1783-1784) describes the gated rule. No spec text covers this
  (SPEC_impl.md:9354 says only that the footer "names the launch target through the main action's accessible name").
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — gate it to match the documented rule (or name both everywhere, if the
  maintainer prefers); size S. Discard is reasonable too. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-session-menu-and-quick-switcher-show-generic-custom-agent.md

- Outcome: `fix code`.
- Assessment: For sessions running a registered custom agent, the session row's actions-menu header and the quick
  switcher's agent column say only "Custom agent". They show neither the agent's current display name nor, after
  deletion, its saved ID. The row's hover card and agent-mark tooltip do show the name. Verified: Confirmed.
  `crates/gui/farhelm-gui/src/session_list/menu.rs:1439-1444` uses `agent_kind.display_name()` only, and the quick
  switcher reuses it (`quick_switcher.rs:1478`). The `harness::custom_name` helper exists (`harness.rs:23-31`).
  SPEC.md:497-498 says "The session list uses the current display name and falls back to the saved ID when the
  registration is deleted". Whether the actions menu and the quick switcher count as "the session list" is a reading.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — pass collections into `agent_name` and use `custom_name`; size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-settings-custom-agents-unavailable-collection-silently-appears-empty.md

- Outcome: `fix code`.
- Assessment: If the helm cannot read the custom-agents catalog (a database error, or a corrupt saved entry), Settings →
  Custom agents shows no error. On a first load it looks empty, with only "+ Add custom agent", and re-adding an
  existing ID then fails as taken. After a later failed read it silently shows the last list it had. Verified:
  Confirmed. `settings/custom_agents.rs:61-78` and `:114-121` ignore `Availability::Unavailable`, while Recipes and
  Containers show the error (`settings/recipes.rs:492-495`). Calling SPEC.md:4944 ("Every failed operation surfaces a
  concrete, actionable error in the client") binding on a background catalog read is a reading.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — show the helm's error as a failure line, as Recipes does; size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-settings-search-omits-registered-custom-agent-sections.md

- Outcome: `fix code`.
- Assessment: Searching Settings for a registered custom agent's name or ID finds nothing, and searching "custom" hides
  every registered agent's section, although each is its own section in the sidebar. The result group for the add page
  is also headed "+ Add custom agent", with the button's "+". A catalog-coverage test checks the sidebar button rather
  than the page body. Verified: Confirmed. `crates/gui/farhelm-gui/src/settings/search.rs:441` has
  `Section::CustomAgent(_) => {}`. SPEC.md:2585 says search "finds settings by section, label or help text". Matching
  dynamic sections is a design extension, so this is not clearly settled.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — add dynamic entries, strip the "+" and point the test at a body element; size
  M (likely under 100 lines). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-stacked-quick-launch-eligible-accepts-custom-agent-without-id.md

- Outcome: `fix code`.
- Assessment: A launch template can choose the generic Custom agent type for an Agent launch without naming a registered
  agent, for example via `farhelm agent template create foo --agent custom --quick-launch`. Such a template can still
  take the quick-launch mark (⚡). Pressing it then fails right away ("select a registered custom agent") instead of
  opening New with the "needs more choices" banner. Verified: Confirmed. `crates/farhelm-proto/src/launcher.rs:417-438`
  returns `agent.is_some()` for Agent launches. The helm always refuses an Agent launch of Custom without an ID
  (`farhelm-helm/src/sessions.rs`, per the reviewer). SPEC.md:4800 ("⚡ needs a template that chooses an agent ...") is
  met literally. Whether generic Custom "supplies what its active launch kind needs" (SPEC.md:1379-1380) is a reading.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — track the custom ID in the eligibility fold; size S. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-template-apply-custom-to-custom-misses-switched-agent.md

- Outcome: `fix code`.
- Assessment: Suppose a switch to a custom agent raised the red permission-escalation banner with Undo, and the user
  then applies a template that selects a different custom agent. The old banner stays up, and its Undo would also undo
  the template. Switching between custom agents by hand dismisses it. Verified: Confirmed.
  `crates/gui/farhelm-gui/src/templates/apply.rs:219-230` compares only `agent.harness`, and every registered agent
  shares `Custom`. The hand path sets `switched_agent` on an ID change (`launcher/draft.rs:760-766`). The doc comment at
  `apply.rs:181-183` gives the rationale that is violated. SPEC_impl.md:9373-9374 says replay makes the draft "react
  exactly as they do to the same edits by hand". That supports the fix, but it is a reading.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — also compare `custom_agent`; size S. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-yolo-confirmation-custom-agent-reads-every-custom-permission.md

- Outcome: `fix code`.
- Assessment: On a host that asks before YOLO launches, launching a registered custom agent shows the confirmation
  "Farhelm treats every Custom permission choice as YOLO." "Custom" is also the name of the custom permission-flags
  mode, so the sentence reads as if the launch used custom flags, and it does not name the agent. Verified: Confirmed.
  `crates/gui/farhelm-gui/src/launcher/yolo.rs:40-42,73-78` formats `OnlyMode(LaunchHarness::Custom)` with
  `harness_label` = "Custom". No spec text fixes this wording.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — add a custom-agent sentence, for example "Farhelm treats every custom agent
  launch as YOLO."; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-more-options-locked-open-without-field-on-custom-restart-no-resumption.md

- Outcome: `fix spec+code`.
- Assessment: Restart with can open on a custom-agent session that has no captured conversation, after that agent's
  registration turned off resumption (or was deleted). "More options" is then forced open and marked "A setting here is
  required or needs attention", but it contains no conversation field and no explanation, only the wrapper prefix.
  Verified: Confirmed. In `crates/gui/farhelm-gui/src/launcher/view/more_options.rs:33-43`, `required` forces
  `must_open` while `offered` is `None` because `offers_conversation` needs `custom_resumption`
  (`launcher/request.rs:536-541`). SPEC.md:2201-2203 says Restart with "is available even without a captured
  conversation; then its conversation field is required and explains why". SPEC.md:497 says "Existing launches retain
  their commands and capabilities after a registration changes". The draft instead refreshes capabilities from the
  current catalog (`draft.rs:235-237`), so the right behavior depends on which rule wins.
- Decision: q-restartreg, "Restart with after a custom agent's registration changed": the maintainer picked "Follow the
  current registration". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "I think this
  is what users would actually expect. But if this means that it's very, very complicated to implement, please push back
  on me."
- Completion criteria: Change SPEC.md so a custom agent's registration change applies to Restart with (replacing
  "Existing launches retain their commands and capabilities after a registration changes" for that case), and make the
  launcher's prompts and disabled reasons follow the current registration. If this turns out very complicated, stop and
  push back to the maintainer instead of landing it. Recommendation for reference: fix code — decide together with 522;
  at minimum show why the restart is unavailable instead of an empty forced-open panel; size S. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-restart-with-mishandles-custom-agent-with-resumption-disabled.md

- Outcome: `fix spec+code`.
- Assessment: Restart with can be opened on a custom-agent session after the user turned off "Supports resumption" for
  that agent. With no captured conversation, it says "Enter a conversation to restart" but hides the field. With a
  captured conversation, an edit enables Restart, and the helm or supervisor then rejects the request. Verified:
  Confirmed for the GUI. `restart_refusal` (`crates/gui/farhelm-gui/src/launcher/request.rs:380-382`) asks for a
  conversation that `offers_conversation` (:536-541) hides. `agent_refusal` (:324-372) never checks `custom_resumption`.
  The later server-side rejection (helm `compile_custom` uses the current registration; the supervisor refuses in
  `restart_session_named`) is the reviewer's trace, and I did not confirm the exact message. It hinges on the same
  conflict as 516: SPEC.md:497 ("Existing launches retain their commands and capabilities after a registration changes")
  versus the draft's refresh from the current catalog.
- Decision: q-restartreg, "Restart with after a custom agent's registration changed": the maintainer picked "Follow the
  current registration". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "I think this
  is what users would actually expect. But if this means that it's very, very complicated to implement, please push back
  on me."
- Completion criteria: Change SPEC.md so a custom agent's registration change applies to Restart with (replacing
  "Existing launches retain their commands and capabilities after a registration changes" for that case), and make the
  launcher's prompts and disabled reasons follow the current registration. If this turns out very complicated, stop and
  push back to the maintainer instead of landing it. Recommendation for reference: fix code — refuse up front with "This
  custom agent no longer supports resumption" (or follow stored capabilities if that is the answer); size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-settings-custom-agent-deleted-banner-omits-a11y-role-and-label.md

- Outcome: `fix code`.
- Assessment: In Settings → Custom agents, if the agent being edited is deleted elsewhere while the user has unsaved
  edits, the page shows "This registration was deleted. Register it again to keep these edits." Screen readers never see
  or announce that line. Verified: Confirmed. `crates/gui/farhelm-gui/src/settings/custom_agents.rs:685-687` is a bare
  div with no role. GPUI exposes only elements that have a role (per `host_text.rs`). No SPEC rule requires this status
  to be announced.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Expose this consent, warning or status text to assistive technology under that principle.
  Recommendation for reference: fix code — render it through `choices::outcome_line`; size S. Size gate: about 100 lines
  of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-radio-group-chip-flex-none-overflows-long-custom-agent-name.md

- Outcome: `other`.
- Assessment: In the session launcher's Custom group, a registered agent with a very long display name gets a chip that
  can neither shrink nor shorten its label with an ellipsis, so it runs past the dialog's right edge and is clipped.
  Verified: Confirmed by reading the code. The chip is `.flex_none()` with no `min_w_0`, `max_w_full` or truncation
  (`crates/gui/farhelm-gui/src/components/radio_group.rs:1017-1048`). `CustomAgent::validate` sets no length cap
  (`crates/farhelm-proto/src/custom_agents.rs:25-33`). I did not check how it renders. No spec rule covers it.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — truncate chip labels; size S.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-settings-heading-lacks-min-w-0-and-truncate-for-long-custom-agent-name.md

- Outcome: `other`.
- Assessment: Opening a custom agent with a very long display name in Settings can push the page's scope label ("Every
  host on this helm") off the right edge, or grow the pinned heading over several lines. Verified: Confirmed by reading
  the code. `#settings-heading` and its row lack `min_w_0` and truncation (`settings/frame.rs:429-447`), unlike the nav
  row (:299). I did not check how it renders. No spec rule covers it.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — add `min_w_0` and `truncate`; size S.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-custom-agent-tilde-command-kills-live-agent-before-failing.md

- Outcome: `fix code`.
- Assessment: A custom agent can be registered with a command under `~/`, which each host expands against its own home.
  If a host's supervisor has no usable home directory, that expansion fails, and it fails late. Restart and Restart with
  first stop the running agent and its whole process tree, without asking when the agent is idle, and only then refuse
  the launch. A create into a new managed checkout makes the checkout folder and leaves an error session behind before
  it fails. Verified: Partly correct. The code path is confirmed. The only expansion is in `prepare_launch`
  (`crates/farhelm-supervisor/src/service/core.rs:14621-14633`). `restart_session_named` pre-checks placeholders and the
  folder (`core.rs:12547-12566`) and then calls `stop_live_agent` (`core.rs:12632`) with no expansion check.
  `validate_create` expands `~` only for the folder (`core.rs:9137`). What is not established is the trigger. The
  supervisor reads `HOME` once at start (`core.rs:6278-6284`), and supervisors run under a systemd user manager, which
  sets `HOME`. Nothing shows a shipped install that starts a supervisor without a usable `HOME`. SPEC.md 2208-2209 ("The
  edited launch is validated exactly as a create validates it before anything is stopped") is satisfied word for word,
  because create does not pre-check this either. Its stated purpose ("so an invalid edit leaves the agent running") is
  not.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code, low priority.
  A shared expansion pre-check in create, retry and restart is cheap. Or discard, given that no shipped route exists.
  Size S-M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-restart-with-custom-reporting-and-offer-use-stale-row-launch.md

- Outcome: `fix code`.
- Assessment: Restart with on a custom agent session picks up its registration's current reporting and resumption
  settings. The new agent starts before Farhelm saves those settings. If the agent reports its conversation at startup,
  as the custom-agent contract invites, the report is judged against the old settings. Turning reporting on can
  permanently discard a valid report, leaving Restart unavailable for that run. Turning it off can still admit one. The
  restart's reply, and listings during that window, show a stale Restart offer. Verified: Confirmed by tracing the code,
  not run. `relaunch_into_terminal` spawns and publishes the new generation before `update_launch` saves the new launch
  (`crates/farhelm-supervisor/src/service/core.rs:12941-12990`). Report admission gates on the stored row's
  `custom_reporting()` under the capture claim (`service/core/vendor/positional.rs:120-144`). `restart_with_refusal`
  allows the same custom agent with a changed reporting flag (`core.rs:3637-3667`). A non-internal refusal settles the
  drop file (per the reviewer, `report_files.rs:250-265`; not re-read). How often a startup report lands in the window
  (`end_replaced` and `restart_reply`) is unverified. SPEC_impl.md "Custom agents" requires "A structured launch that
  disabled reporting refuses capture even if it supplied a resume command".
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Persist the
  new launch with the generation bump, or pass it into publication. Size M. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

