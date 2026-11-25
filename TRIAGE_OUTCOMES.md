# Triage outcomes

## template-delete-can-target-unshown-editor.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md:5787-5792 ("One GUI at a time", Confirmed 2026-10-10): "A pointer or keyboard
  action uses the target, consent and preconditions from the frame the user was shown, or is ignored when that frame has
  not yet been shown or its target is no longer in view." The code contradicts this: - The footer's Delete handler
  captures only the drawn `DeleteKind` (`crates/gui/farhelm-gui/src/templates/editor.rs:1305-1326`). -
  `TemplatesDialog::delete_editor` then deletes whatever editor is live (`templates/dialog.rs:482-495`), and
  `Editor::delete` takes the live name and stored copy (`editor.rs:790-822`). - Opening another row from a clean editor
  replaces the editor synchronously inside that click (`dialog.rs:448-462` -> `leave` :532-547 -> `depart` :610-653 ->
  `show_editor` :773-790). - GPUI dispatches mouse events against the last drawn frame without redrawing first
  (`vendor/gpui-pre/src/window.rs:6429-6463`). Key events do redraw a dirty frame first (:6510-6512), so only the
  pointer route exists. So a row click followed by a Delete click before the next frame deletes the newly opened
  template B. It uses A's delete kind, so B goes without Undo when A's label read "Delete permanently". No commented
  choice protects the live lookup: the comment at :1305-1306 deliberately binds the kind only. Fix: capture the drawn
  name with the kind, and ignore the press when `editor.previous()` no longer matches. That is the "ignored ... target
  is no longer in view" branch of the rule. The other branch, deleting A by its captured name, also conforms, but A's
  stored copy would then have to be re-read for Undo. The ignore shape is the direct one. Size about 30-60 lines with a
  test, well under 100. Note for the maintainer: the trigger needs two clicks on different controls before a redraw,
  which a human can only manage while the main thread is stalled. The bucket stays `highest` because the route is
  established and the spec itself contemplates such input. Verified: confirmed by code reading. The real-world
  likelihood of two clicks inside one frame is unmeasured.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — the
  spec rule is explicit and the guard is tiny; size S.). Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## hook-save-uses-newer-replace-warning.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "Lifecycle operations", Replace with: "launching carries the precondition
  matching what the launcher showed at the click (see the confirmation rule below)". The confirmation rule (Confirmed
  2026-10-01) says: "the answer applies to the wording on screen at the click, never to a state the user was not shown".
  SPEC.md launcher section (around line 1221) adds: "launches that retire a session (Replace with) limited to the action
  and consent already shown". The code contradicts this. **Set up and launch** is the click, but its continuation
  (`launcher/view/hook_question.rs:188-208`, `:247-253`) calls `press_at` (`launcher/view.rs:1335-1356`), which takes
  `SourceConsent` from `source_frame.current()` at the moment the settings save settles. That is not the frame shown at
  the answer. Since #4623 the helm pushes live updates while the save request is out. If a source that showed nothing
  alive at the click becomes live during the save, the newer frame draws a warning with the `Unconditional` guard, and
  the continuation sends that guard. The source is then deleted without the user ever confirming that warning. The
  module's own contract agrees with the spec (`hook_question.rs:2`): "Its launch continuation belongs to the draft that
  was answered." The YOLO question already solves the same problem by freezing the shown source into its answer
  (`launcher/view/yolo_question.rs:95-101`, `view.rs:2302-2306`). Verified: confirmed by reading the code. Runtime
  frequency was not verified. A sub-issue goes away with the same fix: if the window has not yet presented the frame
  when the save settles, `press_at` returns without a word (`view.rs:1346-1352`), so the setting is saved and the launch
  is silently dropped.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Freeze
  the `SourceConsent` shown at the hook answer, as the YOLO question does, and pass it to the continuation's submit.
  Size M; with a GPUI test it may approach or exceed ~100 lines. Sibling item: 51.). Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-agent-fresh-checkout-rereads-claude-trust-after-approval.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md "Permission prompts for agent actions" (around line 9733): "What a card
  shows is what an approval does. Launches are resolved fully before the card, and the dispatched launch is the one
  shown." The code's own contract says the same: `farhelm-helm/src/workspace_trust.rs:109-112` ("Agent-requested
  launches carry the choice shown on the card through dispatch") and `sessions.rs:1546-1547` ("An agent launch instead
  retains the choice already shown on its approval card"). The code contradicts it. After approval,
  `create_fresh_session_resolved` reads Settings again (`sessions.rs:2639`) and builds the checkout resolution with
  `claude_trust: config.claude_trust` (`sessions.rs:1586`). The approved launch keeps its frozen workspace trust
  (`resolved_mode`) for a new managed checkout (`create_checkout_for_agent`) and for an agent-requested environment
  (`launch_environment_for_agent`). The supervisor carries the fresh value into preparation (`service/core.rs:11383`).
  If the switch went from off to on in between, the checkout is marked trusted even though the card said "Not trusted"
  (`launch.rs:1986-1999`). From on to off, the shim writes trust for an ordinary folder, with no removal record
  (`launch.rs:1218-1227`), so archiving never removes it. Toggling trust does not change the configuration revision, so
  the preview check does not catch it. Verified: confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Freeze
  the pre-approval `claude_trust` alongside `resolved_mode` in both agent paths and use it in place of
  `config.claude_trust`. Size M; plumbing through two contexts plus tests in both toggle directions likely exceeds ~100
  lines. A residual question remains outside the clearly settled part: for a non-Claude agent's managed checkout,
  Claude-trust-after-clone is not shown on the card at all.). Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says
  remains.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

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

## trailing-hosts-menu-rerun-reads-live-host-state.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "Questions: the prompt": "The same rule starts at the control that asks the
  question or sends the request directly. Its handlers carry the drawn session key, row, skip preference and the parts
  of the environment's state that determine the command. … Live state may also refuse a missing or unavailable target.
  The question and request use the snapshot that admitted the press." SPEC_impl.md (registered setup, around line 5540):
  "A registered SSH host's set up again sends the `ssh-host` target, naming that row and the destination the desktop
  showed at the click". The code contradicts both. `gui/.../hosts/menu.rs:409-437` renders `which` (Setup or Update),
  but `on_click` ignores it, re-reads `current(host, cx)` and dispatches whichever Re-run is live. It also passes the
  live `view` (so the live destination) to `setup::rerun`. The fix follows directly: capture the drawn `which` and
  `HostView` in the closure, and keep the live check only as a refusal. That matches the existing comment ("Checked
  again at the click") and the Adopt and Take over pattern in the same menu. Admitting through
  `prompt::ActionFrame`/`when_shown` follows session_list/menu.rs. The menu's live-follow drawing is not overturned,
  since the closure from each render carries that render's values. Estimated size ~40-90 lines with a GPUI test. It may
  approach 100 if the `when_shown` admission is included. Verified: Confirmed by reading code (menu.rs:425-437).
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  follows the quoted rule; size S-M.). Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-hosts-menu-remove-reads-live-skip-confirmation.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "Questions: the prompt": "Its handlers carry the drawn session key, row,
  skip preference and the parts of the environment's state that determine the command." The code contradicts it. The
  Remove item's `on_click` (`gui/.../hosts/menu.rs:494-512`) captures only `host`, and `remove::start` reads the live
  `listed(host, cx)` and live `asks_first(cx)` at activation (`hosts/remove.rs:136-153`, decision at :145). The fix
  follows directly: capture the drawn view and `asks_first` when the item renders, admit through
  `ActionFrame`/`when_shown` as session_list/menu.rs does, and pass the snapshot to `remove::start` while live state
  only refuses a missing or busy host. Provider deletes keep always asking (`removal.deletes.is_some()`). Estimated size
  ~40-90 lines with a GPUI test. Verified: Confirmed by reading code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  follows the quoted rule; size S-M.). Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-podman-claim-and-recipe-guards-omit-tailscale.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "One route for transport and provisioning" (about line 6343): "An ordinary
  SSH row stores a false-by-default `tailscale` bit. It travels with the destination, through the supervisor handshake's
  identity write guard, the container place check, and confirmation of a provisioning plan." In current code,
  `claim_podman_place` fences on `tailscale IS ?7` (`store/podman.rs:395-399`). But `has_podman_place_claim`
  (`store/podman.rs:79-99`) compares `entry_uuid` and `destination` and not `tailscale`. And `write_recipe_state`
  (`tensorlake/build.rs:345-358`) compares kind, entry and destination and not `tailscale`. Both are container place
  checks that compare the destination without the bit that the spec says travels with it. The fix is direct: add
  `tailscale` to both comparisons, plus a test of a toggle followed by a reclaim. Estimated 30-60 lines, well under 100.
  Nothing in the code chose to leave it out. 74fa1951b updated only the claim query. On the bucket: the established
  consequence is that a build started just before a mode toggle keeps going on the same machine and account, so `other`
  fits better than `high`. The reviewers' highest path (a MITM'd unpinned route) is not established.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  clearly settled; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## legacy-template-can-ask-to-save-automatic-cleanup.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md:1343-1346: "Leaving asks Keep editing, Discard, or Save ... only when the draft
  has edits since opening or saving that Save would store. ... An inferred switch or removed legacy workspace-trust
  field enables Save but never makes leaving ask." The code contradicts it for one template shape: a kindless template
  that still has the old workspace-trust field. - The baseline holds the raw stored fields, trust included, and the
  leave baseline holds the normalized opening draft with the switch inferred and trust dropped (`editor.rs:262-279`,
  `:296-301`). - After the user picks "Don't switch" (`rows.rs:169`, `editor.rs:491-501`), the draft differs from
  storage only by the dropped trust field and from the opening draft only by the switch. `needs_leave_confirmation`
  (`editor.rs:415-417`) still returns true, and both departure routes ask (`dialog.rs:432-440`, `:532-547`). The
  existing tests cover each fill-in alone (`templates/leave_tests.rs:155-187`, `:190-209`) but not this combination. The
  method's own comment states the intended rule that neither fill-in asks, so no deliberate choice is overturned. Fix:
  compare against a stored baseline with retired workspace trust removed for the leave decision only, keeping the raw
  baseline for Save's dirty state, and add a test for the combination. Size about 20-40 lines. Verified: confirmed by
  code reading.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — the
  spec says "never" in so many words; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## antigravity-trust-hint-omits-live-announcement.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "Managed checkouts" trust section (around lines 1842-1847): the band "takes
  layout space rather than covering the question, is announced without taking focus". Around 1868-1869: "Codex, Muse and
  Antigravity show the same nonmodal discovery hint". The code contradicts it. `session_pane/trust_band.rs:384-400`
  (`strip`) sets only `Role::Status`, and nothing in GPUI sets a live property by role (no `Live::` in
  `vendor/gpui-pre/src`). In the pinned `accesskit_consumer` 0.38.0, `node.rs:889-892` defaults live to Off. In
  `accesskit_macos` 0.26.3, `event.rs:236` announces a new node only when it has a value and live is not Off. The
  repository already uses this pattern elsewhere (`launcher/view/checkout.rs:1209-1214` sets `Live::Polite`). Verified:
  confirmed by inspection. Actual speech was not checked. The omission covers the shared band for Claude, Codex, Muse
  and Antigravity alike. `strip` is also used by the hook setup band, so the fix should apply live only to the trust
  band or accept it on both. The implementer must check that the emitted node carries both live and a value.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Size
  S, under 100 lines including a node-property test.). Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

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

## hook-hint-dismissal-survives-ui-reset.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, the UI reset rules (around lines 5724-5728): "The selected session survives a
  reset by id ... The window itself ... is outside the reset, and settings come back with the rest of what the helm
  reports. Everything else starts over". The code contradicts it. The hook band, and with it its × set, is created once
  per window pane (`session_pane.rs:398`; `main_view.rs:155`) and never cleared on a reconnect. While disconnected,
  `follow` skips its cleanup, and it keeps entries that are still eligible in the rebuilt store (`hook_band.rs:59-75`).
  Verified: confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Clear
  the band's local state when the UI session changes. Size S, under 100 lines with a test. The trust band probably has
  the same pattern; that is outside this item.). Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## hook-hint-dismissal-reply-loss-hides-all-hints.md

- Outcome: `fix code`.
- Assessment: Clearly settled. the same SPEC.md reset rules (around lines 5721-5728): "No action tracks its own fate
  across a reset", and "Everything else starts over". The code contradicts it. **Don't show again** sets `saving`, which
  only its reply clears (`hook_band.rs:89-105`). A reset drops that reply, the band survives, and `draw` keeps hiding
  every hint for that agent while `saving` is set, even when the restored preference says to show them
  (`hook_band.rs:133-136`). Verified: confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Reset
  `saving` and `error` on a UI session change; this shares the fix with 227. Size S.). Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## hook-recovery-text-omits-direction-isolation.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md "Host-supplied text" (~~:1172-1197) says: "Host-supplied text is its own
  type from the moment it is decoded. The only element that can draw it escapes hidden characters and isolates text
  direction". It also says "Host text is never concatenated with the app's words in any other way", and "Hover help may
  name a host value … only as a host sentence". SPEC.md "Confirmed 2026-10-10" (~~:5614) requires escaping wherever host
  text appears. Current code: the host's install error reaches the GUI as a plain `String`
  (`crates/farhelm-proto/src/lib.rs:939-946`). `restart_blocker` formats it into app prose after only `text::escaped`,
  with no isolate (`lib.rs:973-976`). The GUI builds Restart's tooltip, header menu description, recovery card and
  launcher entry from that `String`: `session_list/restart.rs:134-140` via `not_captured_clause`,
  `session_pane/header.rs:242,374`, `session_pane/card.rs:293,305,463` and `launcher/entry.rs:411`. The launcher's save
  refusal already does it right with `HostSentence::host` (`launcher/view/hook_question.rs:242-244`). The fix follows
  directly: carry the reason as host text and draw it as a host sentence on these surfaces. The CLI wording, which is
  terminal output, is out of scope. This is not a filter match, because direction isolation is part of the
  untrusted-host-text boundary. Verified: confirmed by source. How a particular right-to-left error actually renders has
  not been checked.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code.
  Plumbing the reason through about five GUI surfaces plus tests is likely over ~100 lines, so execution will probably
  hit the size gate. Size M.). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## hook-setup-accepts-nonutf8-config-path.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "Paths that are not valid UTF-8" (~:5366-5370) says: "Farhelm does not support
  paths that are not valid UTF-8 … Every surface that meets one refuses it with a clear message naming the path as well
  as it can be shown". Current code reads `GROK_HOME` and `HOME` as raw `OsString`s and checks only that they are not
  empty (`crates/farhelm-supervisor/src/service/core.rs:6280-6296`). It then installs to the derived paths with no UTF-8
  check (`agent_kind/persistent_hooks.rs:43-58,64-75`). The fix follows directly: refuse a non-UTF-8 destination before
  writing, through the existing optional-setup failure (`setup_vendor_hooks`'s `Failed { reason }`,
  `core.rs:14913-14923`). The launch still continues. No comment explains the current choice, so this overturns no
  deliberate decision. Verified: confirmed by source. Today the files go to the right bytes and setup reports Installed.
  The gap is the missing refusal, not any harm, so the maintainer may simply prefer to drop it.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Under
  ~100 lines with a test. Size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## trailing-retired-yolo-flag-refusal-overstates-when-farhelm-asks.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "Agent-spawned sessions" (4539-4540): "A request from a host whose 'run farhelm
  commands from this host without asking' setting is on (see Topology) is not asked about for commands." The "Manage
  sessions on this host without asking" permission waives the card too (4550-4553). The code contradicts this:
  `crates/farhelm/src/main.rs:853` says "Farhelm asks the user before any agent launch". The fix is mechanical: drop the
  blanket claim and keep the obsolete-flag and YOLO-guard explanation. No other copy of the text exists. Diff about 5-10
  lines, well under 100. Borderline: the required wording of the message is not itself specified, only the facts it
  misstates.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes. Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-completed-setup-save-steal-focus-from-newer-input.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Who may move focus": "Something the user started that finishes later … first
  asks whether the user has moved on since starting it. Moving on means any key press or any click inside Farhelm." The
  code contradicts it. The launcher's inline checkout-folder setup takes its moved-on mark when the save's result
  arrives (`launcher/view/checkout.rs:1128-1133`, `focus_after_setup = Some(Mark::take(cx))`), not when the user pressed
  Save, so keys typed while the save was in flight do not count. The later focus move to Repository
  (`checkout.rs:1114-1120`) then checks the wrong interval. The fix follows directly: take the mark when Save is
  pressed, carried on the field's `CheckoutUpdated` event from `settings/checkout_field.rs`, and use it for the focus
  move. Nothing deliberate is overturned; the field doc at `checkout.rs:229-230` only says "unless the user has moved on
  since". Estimated size ~30-70 lines with a test. Verified: Confirmed by reading code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  follows the quoted rule; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-shortcuts-chord-opens-during-terminal-ime.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "Desktop window chrome" (around line 4002): "Cmd+/ on macOS, Ctrl+Shift+/ on
  Linux, opens **Keyboard shortcuts**, including from a focused terminal. ... repeat and composition do not open it."
  Current code opens it unconditionally (`crates/gui/farhelm-gui/src/main_view.rs:307-309`). The same file already
  guards Delete and Rename with `session_pane.terminal_is_composing` (`main_view.rs:268-283`; helper
  `crates/gui/farhelm-gui/src/session_pane.rs:533-540`). The recorded gap
  (`crates/gui/farhelm-gui/src/shortcuts.rs:27-31`; `docs/gpui/parity/other-dialogs.md` "Keyboard shortcuts") says
  "nothing in the app can tell that handler whether the focused input is composing". That reason no longer holds for
  terminals since 2ce86b879 (#4528), so the note records a limitation rather than a deliberate choice. The fix: add the
  same guard, and narrow the note and parity entry to text fields, which still cannot be queried. No spec rule changes.
  Estimated size: under 40 lines including a test modeled on the Delete and Rename composition tests, well under 100.
  Verified: Confirmed for the terminal case. Text-field composition stays an open gap after the fix (no input-method
  query exists), so the queue item should be narrowed to that, not removed.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code.
  Directly follows from the spec. Size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-pending-scope-cleanups-logs-invalid-session-id-unescaped.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "Session IDs in logs": "Session IDs must not inject log lines or use
  invisible or direction-changing characters to disguise the ID shown in logs. … Safe escaping by the configured logging
  formatter also satisfies the requirement." Current code contradicts it. - `service/core/host_restart.rs:917` logs
  `session = %id` in exactly the branch reached when the ID failed validation as a UUID (`unit_name` returned `None`).
  The retire warning at :924 also runs for that ID with `%id`. - The configured formatter
  (`crates/farhelm/src/main.rs:2785-2793`, the default `tracing_subscriber::fmt()`, version 0.3.23 per Cargo.lock)
  escapes only ESC, BEL, BS, FF, DEL and C1 controls (`fmt/format/escape.rs` in the registry source). Newlines and bidi
  or invisible characters pass through unchanged. - The fix follows directly and takes no design choice: `?id` at both
  call sites. That uses `str`'s Debug escaping, the simplest option, as the rule prefers. - I did not run a check that
  Rust's Debug escapes U+202A-U+202E. I believe it does (they are not printable under `core::unicode`). Verified:
  confirmed.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — the
  spec rule is literal and the code contradicts it; size S.). Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says
  remains.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-later-codex-github-failure-hides-claude-cleanup-warning.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "Named credential catalog" (6220-6223): "A failed cleanup keeps that stamp
  for retry and withholds that agent's entry. The other agent and GitHub can still finish. The host warning names the
  affected agent, and credential replacement reports the warning when that agent's selection uses the replaced name."
  Current code: `crates/farhelm-helm/src/provisioning/credential_delivery.rs:822-825` builds
  `lookup_warnings: warnings.lookup_aborted.then_some(warnings)`. So when a Claude or Codex legacy cleanup is pending
  (`lookup_aborted` false) and the Codex login or GitHub slot then fails, the host warning (`connect`, `:533-535`) and
  the replacement reply (`push_one`, `:719-723`) both drop the cleanup warning and name only the later failure. The fix
  follows directly from the existing machinery: carry the warnings whenever they are present, and let the existing
  `push_failure` choose the cleanup reason as the success path already does. No comment justifies the filter; the
  field's doc comment (`:166-172`) gives no reason to exclude cleanup warnings. Verified: Confirmed. It also covers the
  Codex legacy cleanup, as the file's extension says.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Size S
  (about 5-10 lines of code plus a test with a failed cleanup and a failing GitHub slot, well under 100).). Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry,
  or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-podman-settle-claim-refuses-third-host-duplicate-transfer.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Hosts" (about line 350-353): "When Farhelm has verified which host owns the
  ordinary identity, that entry can take an unused container host claim from a second entry whose ordinary identity is
  absent or refused." SPEC_impl.md "Podman places, creation and images": "An unused twin yields to the verified ordinary
  owner when the twin lacks a recorded ordinary identity or has a current identity refusal." `Duplicate` is one of the
  refusals (`ordinary_identity_refusal`, `podman_probe.rs`), and `record_first_contact` (`store.rs:8820-8821`) puts an
  entry that now reaches a third host's installation into `Duplicate`. `settle_claim` handles `Duplicate` only when
  `twin == row.id` and otherwise falls to `None`, so a connected, verified owner cannot take the unused claim. The
  code's own comment ("Any other refusal needs the challenger's own live hello…") calls for using `verified_supervisor`
  there. Fix: add a `Some(HostState::Duplicate { .. }) => verified_supervisor` arm, plus a test. Estimated 30-50 lines.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  clearly settled; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

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

## trailing-launcher-search-custom-agent-rename-refused-as-stale.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Creation" (launcher search, lines 1209-1211): "If an explicitly highlighted
  choice disappears before Enter, or a clicked row has already disappeared, the field shows 'That choice is no longer
  available...'". A renamed custom agent has not disappeared, yet `same_search_choice`
  (`crates/gui/farhelm-gui/src/launcher/view/search.rs:870-886`) has no `Row::CustomAgent` arm. The `_ => a == b`
  fallback compares the label as well, so a rename drops the highlight and Enter or click gives the stale-choice
  refusal. The fix is the one-line ID arm the code already uses for hosts. That is the intent documented on
  `Row::CustomAgent` ("A registration's ID selects it even after the display name changes"), so it overturns nothing.
  Verified: Confirmed by reading the code. Not executed. The trigger is a rename (from Settings or another client) while
  the row is highlighted or drawn.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — add
  the ID-only arm and a test; size S.). Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-launcher-search-effort-default-misses-custom-effort.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Creation" (line 1188, then 1194): search matches "effort words offered by that
  harness and model plus `effort:default`", and "Accepting a result applies it and clears the box". For a custom agent,
  `offers_effort` is true (`launch.rs:375`), so search offers "Effort: Default" (`launcher/search.rs:669-671`). But
  `LaunchDraft::choose_effort` (`launcher/draft.rs:894-896`) writes only `agent.effort`, while a custom agent's effort
  is read from `custom_effort` (`draft.rs:273-279`). So accepting it applies nothing, and the typed effort is still
  sent. The visible field is not synced either (`view/choices.rs:112-116`). Verified: Confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — clear
  `custom_effort` for a custom harness and sync the field, plus a draft test; size S.). Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-mac-reach-omits-date-for-bootout.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Topology": "There is no macOS version floor: inspection refuses a Mac missing a
  command the path needs, naming that command." The code contradicts it. `MACOS_REACH_SH`'s tool list
  (`provisioning/backend.rs:4080`) omits `date`, which the Update and uninstall bootout wait calls
  (`provisioning/macos.rs:343,349`; the doc at :336 notes `date` and `sleep` are called by name, and `sleep` is in the
  list). The fix follows directly: add `date` to the list, with a reach test. Optional, not required by the spec text:
  read `date +%s` before `launchctl bootout` so a broken `date` fails before unloading. Estimated size ~5-30 lines.
  Verified: Confirmed by reading code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  follows the quoted rule; size XS.). Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-reminder-frame-cfg-test-branch-in-render-path.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md, "GPUI cookbook: traps", "A `cfg(test)` branch in production code tests a
  program that does not ship" (SPEC_impl.md:1624-1635): "Rule: pass the dependency in, so the production path reads it
  the same way in every build and a test hands it a different one … A test-only setter for such a value … are not
  branches. … In a render path it must not change what is drawn or done, or the frame a test checks differs from the one
  the app draws." The code contradicts this. `crates/gui/farhelm-gui/src/update_readout/view/reminder.rs:31-32` gates
  the `held_frame` field with `#[cfg(test)]`, and lines 81-84 add a `#[cfg(test)]` early return in `Reminder::frame`,
  which the render path calls and which changes the drawn frame. The fix follows directly from the rule and its sibling
  pattern (`step_hold`, `update_readout/view.rs:156-160,232-238`): make the field and its read unconditional (always
  `None` in the app) and keep only `holding_reminder` (`view.rs:671-676`) under `cfg(test)`. No comment explains the
  gate as a deliberate choice. Verified: Confirmed by reading the code (unchanged since 5b1cfcdb0).
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Size S
  (about 5 lines, no test change, well under 100).). Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

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

## trailing-unknown-agent-typo-fails-with-misleading-custom-flag-error.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Agent-spawned sessions" (lines 4737-4738): "`--agent` given something that is
  not an agent type is refused with the list of agent types." The CLI turns any well-formed ID (`cluade`) into a custom
  agent (`crates/farhelm/src/main.rs:909-921`). The helm applies the flags first
  (`crates/farhelm-helm/src/agent_launch.rs:256`) and checks registration only afterwards (:323-333). So `--prompt`
  (refused at `farhelm-proto/src/launcher.rs:921-928`), a non-YOLO `--permissions`, or a dotted model fails first with a
  custom-agent flag error and no list of agent types. `ff379d4cc` (#4752) changed only the list's text, not this order.
  The fix follows directly: check the flags' own custom ID against the catalog before applying the flags, and keep the
  later check for IDs inherited from templates. Verified: Confirmed by reading the code.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code — move
  the early unknown-ID check ahead of `apply_template`, plus a helm test; size S.). Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry, or narrow it to any
  part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-bell-open-announcements-silent-on-read-failure.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md, "Errors and diagnostics" (4944): "Every failed operation surfaces a concrete,
  actionable error in the client." Its best-effort exceptions name only "the automatic announcement check and recording
  a dismissed batch", not the user-pressed bell action. SPEC.md 3304 says Open Announcements "loads the current pending
  batch and closes the popover." Current code: `crates/gui/farhelm-gui/src/announcements.rs:140-155` only logs a failed
  read, returns with nothing if the path cannot be built, and ignores whether the read was sent.
  `rail_notices.rs:240-264` therefore leaves the panel open with no message. The fix follows the panel's existing
  pattern for Clear resolved, which keeps the refusal and shows it in the panel (`rail_notices.rs:359,394-409,548-554`).
  Verified: Confirmed for the error path. The empty-batch path was already narrowed out at recording under "Rare,
  self-correcting glitches", so it is out of scope.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code. Show
  the refusal in the panel when the read fails or was not sent. Size S to M (about 30-70 lines including a test; under
  100).). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-link-disclosure-module-doc-soft-seam.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md (terminal links paragraph): "Rows join where the hyperlink's text fills one row
  to its last column and continues at the start of the next, as a wrap draws it, whether the terminal wrapped it or the
  program moved to the next row itself." SPEC_impl.md agrees ("without the wrap flag, because a program that fills a row
  and writes CR LF draws exactly what a wrap draws"). The current code's docs contradict it:
  crates/gui/farhelm-grid-model/src/link_disclosure.rs:23-25 says "A soft seam is where the terminal wrapped the text …
  A hard seam is everything else: a program that ends a row and continues its hyperlink on the next", and
  crates/gui/farhelm-grid-model/src/link_claims.rs:61 says `Seam::Soft` is "The terminal wrapped the text". The behavior
  (`RowSeam::Soft`, link_disclosure.rs:209-214) and its own doc already follow the spec. The fix is a rewording of two
  comments: no design choice, no spec change.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-24 triage, main c38b93552.) The maintainer can revise it.
- Completion criteria: Make the code match the quoted rule as the Assessment describes (recommendation: fix code —
  reword both comments to "the hyperlink's text meets edge to edge across the rows, as a wrap draws it, whoever moved to
  the next row"; size XS (well under 100 lines).). Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry, or narrow it to any part the Assessment says remains.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

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

## staged-host-update-controls-promise-unattended-installs.md

- Outcome: `other`.
- Assessment: When Farhelm runs with a locally staged host build (a developer-only launch mode), it never updates hosts
  automatically. The attention row's "Always update" tooltip and the Settings help still promise later updates will
  install without asking. Verified: Confirmed. Staged mode is set only by `FARHELM_HELM_PAYLOAD_DIR`
  (`farhelm-desktop/src/bundle.rs:15-31`, "opt-in per launch"), and the automatic gate refuses (`payloads.rs`,
  `service.rs:1969-1971`). SPEC.md (attention rows, around 3011-3015) itself prescribes the tooltip text "later updates
  install without asking", and SPEC.md 199-200 says staged mode skips all hosts. That is a spec-internal tension visible
  only in a developer mode.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (developer-only mode).
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

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

## recipe-delete-test-misses-warning-clipping.md

- Outcome: `other`.
- Assessment: The test for the recipe Delete confirmation checks the outer prompt box and its accessible name, but not
  that the warning text and buttons stay visible in a very short window. No actual clipping was shown. Verified:
  Confirmed as a test gap only (`settings/recipe_leave_tests.rs`, as cited). No product bug is established.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only, no observed clipping; size S if kept.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## launcher-host-reorder-test-accepts-old-host.md

- Outcome: `other`.
- Assessment: A launcher test for the Host menu checks that Enter picks the highlighted host after the helm reorders
  hosts. It never checks the drawn row order, and the highlighted host equals the draft's host, so some regressions
  would pass. Verified: confirmed. The test is `launcher/view_tests.rs:3022`
  (`an_open_host_menu_gets_its_first_highlight_when_hosts_arrive`). The shipped path honours the order. Test gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code with a small test edit. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-tab-test-omits-placement-premise.md

- Outcome: `other`.
- Assessment: A Tab-order test meant for "all destinations offered" never checks that the sandbox and container offers
  are actually present, so it would pass if the fixture lost them. Verified: confirmed. `launcher/view_tests.rs:641-689`
  asserts only the closed-form stops. Test gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code by asserting both offers in the menu
  first. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## shift-enter-test-assumes-one-program-read.md

- Outcome: `other`.
- Assessment: A terminal-host integration test checks that the two bytes of Shift+Enter reach a raw-mode program in one
  read. The reviewer argues that one read depends on the PTY rather than on Farhelm's write contract, so the test could
  in principle fail falsely. Verified: Mostly incorrect as a risk.
  `crates/farhelm/tests/terminal_host_client.rs:683-707` is unchanged since the reviewed commit. Its docstring already
  says that a separate host test checks the write boundary. With `stty raw` (VMIN 1), bytes written together, or queued
  before `dd` reads, come back in one read, so nobody has shown a split. The 200 ms sleep is unproven but harmless. The
  remaining observation (the test exercises neither write atomicity nor GUI chord encoding) is accurate but already
  disclosed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. No established false failure, and the docstring
  already scopes the claim. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## terminal-prehello-test-misses-output-loss.md

- Outcome: `other`.
- Assessment: A terminal-host test promises that output written before any client connects is read in full, followed by
  the exit record. It checks only that one substring appears, and skips other records before the exit, so losing a
  trailing fragment would go unnoticed. The production host is believed correct. Verified: Confirmed test gap. The
  docstring says "yields all its output and then an exit record", while the test checks `output_until("early output")`
  and then skips to `Exited` (`crates/terminal-host/tests/host_lifecycle.rs:17-58`). It is unchanged since aecc8f8ec. No
  production defect is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code with an exact-output assertion through
  the exit record. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

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

## terminal-paste-test-misses-first-ack-premise.md

- Outcome: `other`.
- Assessment: A supervisor unit test for a bracketed paste interrupted by a connection loss claims that the first piece
  was acknowledged before the loss. It never shows that the runtime processed that acknowledgement, so a regression that
  ignores it would pass. Verified: Confirmed test gap. `crates/farhelm-supervisor/src/hosts/terminal/tests.rs:2273-2290`
  sends the ack, reads the second piece and disconnects, with no fence. The output fence used earlier in the same test
  (:2258-2264) is not reused. No production defect is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code (reuse the existing output fence) or discard. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-busy-claim-retry-test-bypasses-fleet-wake-it-depends.md

- Outcome: `other`.
- Assessment: Maintainer test gap only. A test for automatic updates retrying a host that was briefly busy calls the
  retry by hand. It would not catch a regression where the host is never woken for a retry. Verified: Confirmed. The
  test is unchanged (`provisioning.rs:1713-1775`; it stops the listener and calls `scan_automatic_updates` twice).
  Whether another listener-level test covers the wake is unverified.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-reconnect-regression-test-does-not-exercise-push-waiting-for.md

- Outcome: `other`.
- Assessment: Maintainer test gap only. The credential-delivery reconnect test does not cover a delivery still queued
  for a host slot when the connection is replaced. Verified: Confirmed by the reviewer. The test file changed afterwards
  (107915bb7, 67d8541dc) and I did not re-check whether a queued-candidate case was added.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-archive-wake-regression-test-covers-delete-but-leaves-replace.md

- Outcome: `other`.
- Assessment: When a Delete moves a checkout to trash, Farhelm tells every window to re-read Trash, and so does Replace
  when its source's checkout moves. A test guards this for Delete even when the session row was already forgotten.
  Replace has its own copy of the code with no such test. Verified: confirmed.
  `delete_session_happy_path_returns_200_with_the_archive_fact` exists (`sessions_tests.rs:2053`), and no Replace
  counterpart was found. Replace's wake is at `sessions.rs:5115-5121`. Behavior is correct today; this is a test gap
  only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code with one test (size S).
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-podman-diagnostic-tests-bypass-connection-stage-fixed-by-patch.md

- Outcome: `other`.
- Assessment: The tests for early container connection failures feed the diagnostic function directly, so a regression
  in how the connection stage is picked for a container's connection would not be caught. Verified: Confirmed as a test
  gap by the reviewer's citations. Current line numbers in `manager.rs` have moved and I did not re-locate each test. No
  product bug is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S if kept.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-confirmed-delete-does-not-have-regression-assertion-for-capture.md

- Outcome: `other`.
- Assessment: No test checks that a confirmed Delete anyway still goes through the ordinary ownership and usage checks
  when the image turns out to exist. Verified: Confirmed as a test gap. SPEC_impl.md "Podman places, creation and
  images" ("If the image exists, it follows the usual ownership and usage checks") is the behavior that lacks a test.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S if kept.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-first-deletes-live-confirmation-reply-lacks-regression-assertion.md

- Outcome: `other`.
- Assessment: No test checks that a first recipe Delete answered promptly opens the Delete anyway question rather than
  showing an ordinary error. Verified: Confirmed as a test gap. The existing test forces a timeout first.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S if kept.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-final-frame-boundary-test-leaves-osc-8-and-file.md

- Outcome: `other`.
- Assessment: When a full-screen program exits, the final screen and the earlier scrollback are kept as separate
  buffers, and link detection must not join a file path or a hyperlink label across that boundary. The guards that
  prevent this have no test, so removing one would go unnoticed. Verified: Confirmed gap on current main. The link code
  has been reworked a lot since (#4737-#4779). The boundary stop is still documented
  (`crates/gui/farhelm-grid-model/src/link_disclosure.rs:277`), but no test in
  `crates/gui/farhelm-grid-model/tests/links.rs` or `tests/paths.rs` uses an exited alternate screen. Every state
  fixture there has `alt_screen: false`. No current mis-link is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code with boundary tests. Size S-M.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-all-phase-fixture-does-not-compare-managers-phase-strings.md

- Outcome: `other`.
- Assessment: A test that checks every host phase uses the same word in the agent-facing view and on the wire starts
  from view values rather than from the host manager's own states, so a phase renamed only in the manager would pass.
  Verified: Confirmed as a test gap; there is no current mismatch. The earlier sibling,
  trailing-host-phase-vocabulary-test-omits-stopped.md, was fixed in PR #4510 under Q11.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Rebuilding the table from manager states is more than
  the roughly 20 lines Q11 allows. Size S to M if fixed.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-status-mark-tests-lost-their-per-target-geometry-check.md

- Outcome: `other`.
- Assessment: The session row's status-mark hover tests check the help text, the notch and the missing ring, but no
  longer check that each mark's help points at that mark, so placement bound to the wrong element would pass. Verified:
  Confirmed as a test gap; no wrong placement has been shown. Generic placement and clamp tests and one golden still
  cover parts of it.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Not cheap, and the generic geometry tests cover the
  shared logic. Size M if fixed.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-maximized-relaunch-contract-has-no-first-frame-or-subsequent.md

- Outcome: `other`.
- Assessment: No test checks that a Mac app relaunched while maximized opens maximized without first showing the
  ordinary frame, or that the next Zoom restores the remembered rectangle. Verified: Confirmed as a coverage gap; no
  wrong behavior has been shown. Closing it needs native window sampling across a relaunch in the Mac VM harness.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Not cheap (Mac VM harness work). Size L if fixed.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-one-create-test-skip-already-readable-duplicate.md

- Outcome: `other`.
- Assessment: A helm test checks that the new-session dialog's exact-preview flow sends only one create to the host. Its
  stand-in host stops counting as soon as the first create succeeds. If a duplicate create is already waiting at that
  moment, it can go uncounted, so the test could miss the very retry bug it guards against. Verified: Confirmed.
  sessions_tests.rs:602-605 uses an unbiased `tokio::select!` between `finished` and the next frame, and the counter
  increments only when the frame branch wins (:645-656). A duplicate the helm has not yet written would be missed either
  way. No product bug is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — add `biased;` with the frame branch first (or drain
  ready frames before stopping); size S. Or discard, if test gaps are being dropped.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-recipe-fixture-omits-create-time-collision-and-provisioning-cleanup.md

- Outcome: `other`.
- Assessment: No test covers a new-environment managed checkout whose folder is taken after its preview, which should
  clean up the new environment and show "Not launched." with the conflict. Verified: Confirmed as a test gap. The
  behavior is specified (SPEC.md about line 1899) and no production failure is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S-M if kept.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-shell-teardown-coverage-stops-at-signal-and-delete-reply.md

- Outcome: `other`.
- Assessment: Farhelm promises that when it stops or deletes a Bash or Zsh session, the shell gets a hangup so it can
  save its history. The tests check that the hangup signal is sent and that Delete succeeds. None checks that a typed
  command actually ends up in the history file. Verified: Confirmed as a coverage gap. sweep.rs:3553-3587 asserts a
  SIGHUP exit with a fake manager. The real-Bash end-to-end test (crates/farhelm/tests/e2e/host_creates.rs, the
  startup/evaluated-input/Delete case the reviewer cites) does not read history. I found no history assertion anywhere.
  Actual Bash or Zsh history behavior was not observed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — the signal-level tests cover Farhelm's side of the
  promise, and a real-history end-to-end test depends on the shell's configuration and costs more than it buys; size M
  if kept.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-recorded-tab-scope-test-omits-deletes-discovery-and-refusal.md

- Outcome: `other`.
- Assessment: Suppose a session's only systemd scope belongs to a terminal tab, and systemd cannot list it. Delete is
  supposed to refuse and keep the session. The product code does that. No test drives that path through Delete itself:
  the nearby tests either start from the agent's scope or call the later cleanup step directly. Verified: Confirmed as a
  coverage gap. teardown.rs:427-447 refuses with `TabScopeEnumeration` when a recorded scope exists and listing fails.
  The direct reaper test is sweep.rs:3734. The fake scope manager's listing always succeeds (the reviewer cites
  scope.rs:1072-1087; I did not re-read it). No product failure.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — the product path is correct, and the test needs a
  new fake listing-failure seam; size M if kept.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-no-existing-assertion-covers-terminal-loss-between-publication-and.md

- Outcome: `other`.
- Assessment: Restart's reply re-checks, at the last moment, whether the new terminal still exists, so a host lost
  during Restart is not hidden from the recovery card. No test covers that re-check, so removing it would go unnoticed.
  Verified: Confirmed test gap. The re-sample is at `crates/farhelm-supervisor/src/service/core.rs:13278-13285`, and the
  reviewer found no assertion that exercises it. No production defect.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Size S-M if a test is added (it needs a seam to lose
  the host mid-reply).
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-delete-level-tests-miss-retained-gone-host-refusal.md

- Outcome: `other`.
- Assessment: If a terminal host looks gone but its adopted process is verifiably still alive, Delete should refuse and
  keep the session and its launch record. The code does that. No test drives that case through Delete. The tests cover
  Close and other Delete branches instead. Verified: Confirmed as a coverage gap. teardown.rs:556-560 returns an error
  from ending the host before the row is removed. The reviewer's list of existing tests matches what is there. No
  product failure.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — the product path is correct, and the shared refusal
  is already tested through Close; size M if kept.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-tab-resume-pass-without-receiving-flood-output.md

- Outcome: `other`.
- Assessment: An end-to-end test pauses one tab's output, floods it, checks that the agent still responds, and then
  resumes the tab. Its final check looks for "FLOOD-". The echoed command already contains that text, so the check can
  pass even if no output produced during the pause ever arrives. Verified: Confirmed. terminal_tabs.rs:1532-1536 says so
  in its own comment. This overlaps 333 (same test).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — wait for a marker that the echo cannot contain (for
  example, `FLOOD-199999`, or a trailing printed sentinel); size S. Combine with 333.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-paused-tab-test-pass-without-exercising-pause.md

- Outcome: `other`.
- Assessment: The same paused-tab test never shows that the paused tab actually stopped receiving output. It only looks
  at the tab after resuming. The client's pause sends no acknowledgement, and queued text counts. Verified: Confirmed.
  terminal_tabs.rs:1503-1536 has no check during the pause. It is the same test as 332.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code, together with 332 — size S–M. Or discard both.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-landing-removed-ambiguous-create-refusal-marker-assertion.md

- Outcome: `other`.
- Assessment: When a new session's terminal host cannot be confirmed ended, Farhelm keeps the session, and its retry key
  must stay usable. A test checked that the error does not tell the client to throw that key away. A later landing fix
  removed that check, saying the test could never see that message. That premise is wrong, so the regression is no
  longer guarded. Verified: Confirmed. #4552 (4fc89689) removed the assertion, saying "Only the helm writes that text,
  and the test talks to the supervisor directly". But the harness client is `farhelm_helm::SupervisorClient`
  (harness.rs:25), whose request handler adds the `SettledCreateUnaccepted` context (client.rs:3035) that carries that
  exact text (lib.rs:1915). The test formats the error with `{error:#}` (host_creates.rs:1452), so the context would
  show. No wrong reply is established.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — restore the one assertion near
  host_creates.rs:1456; size S.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-alternate-screen-stop-diagnostics-overstate-replay-oracle.md

- Outcome: `other`.
- Assessment: An end-to-end test checks that Stop leaves no stale full-screen frame in the replay. Its failure message
  says only a separate screen capture could cause a failure, but replaying the wrong emulator buffer could too, so a
  failure would point a maintainer at the wrong cause. Verified: Confirmed. The message at
  `crates/farhelm/tests/e2e/session_lifecycle.rs:3775-3780` ("only a capture taken before the kill could produce it") is
  narrower than the docstring's own explanation at :3728-3736. Test diagnostics only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code (reword the assertion messages). Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-tab-cleanup-test-leave-first-two-fixture-jobs-running.md

- Outcome: `other`.
- Assessment: A tab cleanup test starts background jobs that ignore the hangup and sleep for 1000 seconds, and relies on
  the Close and Stop under test to kill the first two. Only the third job has its own kill guard. If the test fails
  midway, two sleepers can be left running on the test machine for about 17 minutes. Verified: Confirmed.
  host_tabs.rs:1011-1012 (`trap '' HUP`, `exec /bin/sleep 1000`), and only `third_job` is guarded (:1054). It affects
  test hygiene only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — arm a `PidKillGuard` for each job as its pid is
  read; size S. Or discard.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-alternate-screen-stop-waits-on-output-written-before-term.md

- Outcome: `other`.
- Assessment: A test draws a full-screen frame with a fake agent, then stops it and expects the agent to leave the
  alternate screen cleanly. The test sends Stop as soon as it sees the frame. The fake agent installs its signal handler
  after drawing that frame, so Stop can land before the handler exists, and the test then fails even though Farhelm
  behaved correctly. Verified: Confirmed. session_lifecycle.rs:3761-3764 waits for "ALT-SCREEN APP" and then stops. The
  fixture writes that line (fake_agent.rs:1853), and its own comment (:1859-1867) says the handler must be live before
  the ready marker that tests should wait on. The test asserts the frame is gone (:3778). This is a latent flake. None
  has been observed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — wait for the fixture's post-handler ready marker
  before Stop; size S.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-failed-restart-annotation-test-accepts-errors-before-restoration-is.md

- Outcome: `other`.
- Assessment: A test checks that a failed Restart puts back the "stopped by user" status. It accepts any Restart error.
  An early refusal that never touched the session would pass just as well, so the test does not prove the restore
  happened. Verified: Confirmed. restart_under_concurrency.rs:240-254 uses `expect_err` and checks only the status and
  annotation afterwards. It never checks that the generation advanced or that the error came from writing the launch
  spec.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — assert the error names the spec write, or that the
  row's generation advanced; size S.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-real-supervisor-restart-assertions-leave-recovered-socket-identity-unchecked.md

- Outcome: `other`.
- Assessment: This is the end-to-end test where a real supervisor process is stopped (orderly or killed) and a new one
  takes over a running agent. The test checks that the same terminal host process survives and that the session still
  takes input. It never checks that the session's saved record still names that host's socket. A regression that wrote a
  wrong address while the live connection still worked would pass. No user-visible bug is claimed. Verified: confirmed.
  `crates/farhelm/tests/e2e/host_processes.rs:569-623` reads the socket from the row before the restart (`host_socket`,
  :183-195). After the restart it checks `host_process(&socket) == Some(host)`, the outcome and the kind (:612-619), but
  never `row.terminal_address`. The file has not changed since fcb6fa5e4. The in-process restart tests compare the
  durable address (the reviewer cites `host_restarts.rs:351-372`; I did not re-read it).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The in-process restart tests already cover the
  durable address, so this adds little. Size S (one assertion) if fixed.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-strengthened-stop-tree-test-permits-cleanup-after-success-reply.md

- Outcome: `other`.
- Assessment: Stop must not report success until the agent's processes are confirmed gone. The test for Stop on an agent
  with a hangup-immune grandchild waits up to the full wait budget, after Stop replies, for both processes to disappear.
  So it would still pass if Stop replied early and the cleanup finished afterwards. Verified: confirmed.
  `crates/farhelm/tests/e2e/host_terminals.rs:302-304` calls `stop_session` and then `wait_until_pid_gone` (which polls,
  `harness.rs:2498-2511`) for the child and the grandchild. SPEC.md (Session operations, "Confirmed 2026-09-28: when
  Farhelm cannot confirm that the cleanup an operation needs has finished, the operation fails visibly instead of
  reporting success") requires confirmed cleanup before success. `process_is_gone` counts a zombie as gone, so an
  immediate assertion would not trip on a process that is waiting to be reaped. No premature reply was observed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. The test's own spec line claims "Stop succeeds; both
  processes go", and the fix is to assert `process_is_gone` right after the reply and keep polling only for the exit
  record. Size S.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-restart-gap-open-tab-checks-survivors-but-never-witnesses.md

- Outcome: `other`.
- Assessment: Opening a terminal tab on a session whose agent terminal is gone (the restart gap) must be refused before
  anything starts. The test checks the refusal and that no tab record or host process is left afterwards. A launch that
  started and was cleaned up before the reply would leave the same empty state. Verified: confirmed as a gap.
  `crates/farhelm/tests/e2e/tab_lifecycle_edges.rs:75-107` only checks state after the refusal. Shipped code refuses at
  the gate before any tab startup (`service/core.rs:13955-13975`, the `restart_first` refusal for `None` or
  `Liveness::Gone`). This matches SPEC_impl.md.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Reading the code shows the gate refuses first, and
  counting launch attempts needs a seam threaded into an end-to-end test. Size S-M if fixed.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-restart-ordering-witness-record-gone-without-reading-old-pid.md

- Outcome: `other`.
- Assessment: The test that proves Restart stops the old agent's whole process tree before the new host starts uses a
  wrapper script. The script records whether the old PIDs were alive when the new host began. If it cannot read a PID
  file, it records "gone", so a broken fixture read would count as proof that the old tree had stopped. Verified:
  confirmed. `crates/farhelm/tests/e2e/host_terminals.rs:359-368` uses `pid=$(cat "$file" 2>/dev/null)`. With an empty
  `pid`, both `ps -p ""` and `kill -0 ""` fail, and the script records " gone". Before the Restart, the test verifies
  the fixture PIDs (`hangup_immune_tree`, :394), and the files stay in place, so the read fails only if they disappear
  between those steps. No failed read was observed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The failure needs the fixture's own PID files to
  vanish mid-test. Size S if fixed (fail the wrapper on an empty or invalid PID and arm it only for the Restart's
  start).
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-list-admission-test-observes-reply-timing-instead-of-list.md

- Outcome: `other`.
- Assessment: Building the session list is capped at a few concurrent builds per supervisor, to bound the cost. The test
  around that cap checks that typing still flows while a list waits, and that the list is answered once a slot frees. It
  would not catch a regression that built the list before taking a slot and only held the reply. Verified: partly
  correct. The gap is real: `service/handlers.rs:11306-11338` checks only reply timing. But the test's own docstring
  says its purpose is that a waiting list "does not hold up typing", which it does establish. It does not claim to guard
  where the list is built. Shipped code takes the permit before `list_all` and drops it before sending
  (`handlers.rs:595-619`).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The test proves what it claims, and the construction
  placement is correct by inspection. Size S-M if fixed (a witness seam inside the task).
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-stop-classification-test-pass-without-stop-waiting-for-exit.md

- Outcome: `other`.
- Assessment: A supervisor test is meant to show that Stop of an already-dead agent briefly waits for the terminal's
  final exit record before quoting the screen. It could pass even if Stop skipped that wait. Verified: Confirmed by the
  reviewer's source trace (`service/core/tests/host_terminals.rs:743-773`). Not re-traced line by line here. The
  production wait exists (`service/handlers.rs:1010-1016`). The gap is that no test ties Stop to that wait.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (closing it needs a new scheduling seam). Size M if
  fixed.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-seal-delete-test-cannot-show-delete-reached-directory-admission.md

- Outcome: `other`.
- Assessment: Deleting a session in a sandbox or container environment can seal and destroy the environment when it was
  the last session. That decision must wait for any concurrent create to settle. The regression test for this does not
  prove that Delete actually reached the create's lock, so a Delete that skipped the lock could still pass because of
  timing. Verified: confirmed. `service/handlers.rs:12148-12183` waits only until the Delete claims the agent-request
  fence (:12162-12168, and its comment admits "whether it then waits at the permit or at directory admission"). It then
  plants the session and releases. The sibling test at :12103-12141 uses an exact `claims_reached_for_test` witness;
  this one has no equivalent for `working_copy_operations`. SPEC_impl.md requires holding directory admission for the
  seal decision. Shipped behavior is not shown wrong.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. This guards a potentially work-losing regression,
  and the sibling test shows the witness pattern. Size S-M (may need a "reached the lock" seam).
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-borrower-reload-lacks-completed-origin-runtime-handoff-witness.md

- Outcome: `other`.
- Assessment: A test covers how a session sharing another session's checkout (a borrower) is classified across a
  supervisor reload. To set up the reload, it releases the origin session's terminal, but it never waits for the release
  to finish and never checks that the origin is recovered afterwards. Verified: confirmed. `service/core.rs:37525-37530`
  calls `release_fixture_host`, which calls `HostTerminal::abort` (`hosts/terminal.rs:2421-2427`, which requests
  cancellation without awaiting). The test then reloads and asserts only the borrower's outcome (:37531-37548). The
  comment's premise ("so reload can adopt it without a takeover") is not enforced. The test's subject is the borrower's
  classification, which it does check.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Origin recovery is outside this test's subject. Size
  S-M if fixed.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-live-route-conformance-pass-when-terminal-control-skips-target.md

- Outcome: `other`.
- Assessment: A conformance test checks that terminal controls (pause, resume, detach, resize) do not stall other
  readers. It checks only that the other reader progressed, not that each control acted on its target. The resize even
  asks for the size the target already has. Verified: Confirmed by the reviewer's source trace (`service/handlers.rs`
  test module, around :10860-11196). Not re-traced line by line here. No production control failure is claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code (add per-control postconditions and a
  distinct resize). Size S-M.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-omp-owner-exit-test-overstates-evidence-behind-resume-offer.md

- Outcome: `other`.
- Assessment: For OMP sessions (an agent harness without first-class support), a test's docstring says the saved Resume
  offer "stands on the durable binding and the verified file alone". The test never reads the conversation file, and by
  design the file is checked at Restart, not when the offer is shown. Verified: Confirmed. The docstring is at
  `crates/farhelm-supervisor/src/service/core.rs:24503-24506`. The offer is built from the stored row only.
  Documentation-only. The "Rare edge cases in harnesses without first-class support" filter does not fit: its
  consequences are about integration features, and this is a test docstring.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code (reword the docstring) or discard. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-borrower-classification-test-does-not-establish-checkout-membership.md

- Outcome: `other`.
- Assessment: A supervisor test checks how a session reports an interrupted checkout preparation when it borrowed
  someone else's managed checkout. It never confirms the second session actually joined that checkout, so it could pass
  for a plain non-member. Verified: confirmed by source. `checkout_interruption_status_matches_reload`
  (`service/core.rs:37419ff`) creates the borrower through `create_session_without_overrides` and asserts only the
  classification and reload agreement. Other tests check membership separately. Test gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard, or fix code with one premise assertion (size S).
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-ready-and-borrower-cases-do-not-assert-vanished-terminal.md

- Outcome: `other`.
- Assessment: A supervisor test covers sessions whose terminal vanished during checkout preparation. In two of its cases
  it checks only that no launch error was recorded, not that the session became an ordinary exit, so a regression that
  left the session "running" would pass. Verified: Confirmed.
  `crates/farhelm-supervisor/src/service/core.rs:37518-37545` asserts only the sentinel's absence and that the outcome
  is not an Error. Production reconciliation is believed correct (the reviewer agrees).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code (assert the ObservedExit proposal and the reloaded
  Exited outcome) or discard. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-running-seed-cancellation-checks-readability-without-observing-returned-screen.md

- Outcome: `other`.
- Assessment: When a Restart request is cancelled while the terminal's history is being encoded, the terminal's screen
  must come back intact. The test checks that the screen can be read afterwards and that new output appears. It does not
  check that the earlier screen content survived, so a regression that swapped in an empty screen would pass. Verified:
  confirmed. `service/core/tests/host_terminals.rs:4560-4574` reads with a unit callback, accepts any `Some` seed, and
  checks only output sent after the cancel. "seed line 01" is written before the cancel (:4529) and never checked
  afterwards. Shipped code returns the original emulator (per the reviewer's citations; not independently traced).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. A one-line assertion that the read sees "seed line
  01" matches the test's "gives the emulator back" claim. Size S.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-corrected-clone-doorway-test-still-omits-valid-absent-host.md

- Outcome: `other`.
- Assessment: The supervisor test for agent-verb validation documents that Clone may omit its host, but never checks a
  Clone with a source and no host. A regression that reinstated a host requirement would still pass. Verified:
  confirmed. `crates/farhelm-supervisor/src/service/handlers.rs:7225-7340` has no positive absent-host Clone case, and
  its positive Clone cases all name a host. Test gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code with one assertion (cheap), or discard. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-held-start-fixture-coverage-of-launch-publication-remains-unverified.md

- Outcome: `other`.
- Assessment: A test checks that, for OMP sessions, Farhelm records how the agent was launched before the terminal host
  can start the agent. It holds the host's first stage open and reads the saved record while the start is held. It would
  catch the record being written after the start returns, but not a write placed between launching the first stage and
  the test's read. Verified: partly correct. `service/core.rs:24844-25083`: the read happens after the fake publishes
  "entered" and before release (:24996-25021), and `consumed-release` proves the hold was up (:25069-25073). The
  docstring's "Why" (a publish moved after the start returns) is exactly what it catches. Only its first line ("before
  the terminal host can start anything") overstates it. The reviewer's own independent check did not settle the claim.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The test catches the regression it names. Size S if
  one only narrows the docstring.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-restricted-connection-refusal-of-whole-environment-delete-remains-unverified.md

- Outcome: `other`.
- Assessment: A session's own Farhelm connection must not be able to seal and delete its environment. The recording
  could not confirm the refusal from excerpts. Verified: incorrect, the refusal is verified.
  `handle_restricted_control`'s catch-all `other =>` answers `Unauthorized` (`service/handlers.rs:3463-3471`).
  `DeleteSessionOrSeal` is handled only in full dispatch (`handlers.rs:2999`). The restricted-request test lists
  `DeleteSessionOrSeal` (`handlers.rs:7659-7664`) and asserts an `Unauthorized` reply for each (`:7666-7671`).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — the finding is refuted; size n/a
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## sqlite-planner-test-misses-stale-statistics.md

- Outcome: `other`.
- Assessment: When the supervisor opens its database, it runs SQLite's planner maintenance. The test covers only a
  database that has no statistics yet. It would not catch a regression where maintenance stops refreshing statistics
  that already exist but are stale. Verified: confirmed. `crates/farhelm-supervisor/src/db.rs:404-435` asserts
  `sqlite_stat1` is absent before maintenance. `maintain_on_owning_open` (:158) runs a fixed `PRAGMA optimize=0x10002`,
  so whether stale statistics get refreshed is SQLite's own heuristic, not Farhelm's logic.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The refresh decision belongs to SQLite, and a
  stale-statistics fixture would mostly test SQLite. Size S-M if fixed.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## terminal-link-oracle-regeneration-loses-wide-padding.md

- Outcome: `other`.
- Assessment: Maintainer tooling. The script that regenerates the recorded terminal-link answers drops a row flag that
  marks wide-character wrap padding. A regenerated fixture would quietly stop testing that case. Verified: Confirmed.
  The checked-in grids 15/16 have `"p": true` on the first row
  (crates/gui/farhelm-grid-model/tests/fixtures/links-oracle.json). The generator's `row()` helper
  (crates/gui/farhelm-grid-model/tools/generate-links.ts:125) and the wide-wrap case (:161-165) never emit `p`.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. A one-field change to the generator keeps the oracle
  reproducible. Size XS.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## remote-probe-absence-test-does-not-witness-proxy.md

- Outcome: `other`.
- Assessment: A setup-probe test expects the same "not installed" result before and after installing its stand-in
  program, so it cannot tell whether the stand-in actually ran. Verified: Confirmed. `provisioning/backend.rs:4331-4364`
  (`remote_probe_reads_absence_from_the_record_when_ssh_reports_zero`) asserts `Absent` both times and never shows that
  the stand-in ran.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S if kept.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## host-lifecycle-test-misses-other-host-control.md

- Outcome: `other`.
- Assessment: While a new session is launching on one sandbox host, that host's Suspend and Resume controls are
  disabled, and other hosts' controls should stay available. The test checks the other host only through an internal
  helper, not through that host's actual menu, so a menu that was wrongly disabled would pass. Verified: confirmed.
  `crates/gui/farhelm-gui/src/hosts/environment_tests.rs:1178-1181` calls `host_lifecycle_refusal(5, cx)` directly, and
  the only menu opened is host 4's. Partial nearby coverage: `a_launch_elsewhere_does_not_hold_suspend` (:1154-1165)
  drives a real Suspend while a launch targets no panel host. It does not cover a launch to a different host in the same
  panel. Shipped `lifecycle.rs:93-126` derives the disabled state per host from `launch_refusal`.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The per-host refusal is the only input to the
  control, and a nearby test drives the real menu. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## inline-resume-test-misses-disabled-control-state.md

- Outcome: `other`.
- Assessment: In the same scenario, a suspended host's inline Resume button must look and announce itself as unavailable
  while a launch to it is in progress. The test checks the hover help and that clicking sends nothing. It does not check
  that the button is drawn or announced as disabled. Verified: confirmed. `environment_tests.rs:1199-1210` checks only
  `hover_help` and the absence of a resume command. Shipped `crates/gui/farhelm-gui/src/hosts/lifecycle.rs:116` sets
  `.disabled(!enabled || launch_refusal.is_some())` from the same refusal that picks the help text.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. The disabled flag and the help text come from one
  value. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## environment-launch-history-test-bypasses-origin-handoff.md

- Outcome: `other`.
- Assessment: When an agent (not the user) asks for a sandbox or container session, the desktop must not learn that
  setup as a recent setup or default place. The code does keep agent-made sessions out of that history. But the
  regression test feeds the "came from an agent" fact straight into the final bookkeeping step. It never goes through
  the real request routes, so a future change that loses that fact on the way would not be caught. Nothing goes wrong
  for users today. Verified: confirmed. `crates/farhelm-helm/src/sessions_tests.rs:146-202` builds `CreateAcceptance`
  with `CreateOrigin::Agent` directly. The real routes take the origin from the request (`sessions.rs:1980`
  `req.origin()`) and pass it on separately at `sessions.rs:2235-2246` (ordinary) and `sessions.rs:2589-2606` (fresh
  checkout). No other test checks `environment_launch_history`/`last_container_host` through a request (grep across
  `crates/farhelm-helm/src`). `sessions.rs:2934` only checks the request's `origin()` parse.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — the shipped handoff is correct, and the guard would
  protect against a hypothetical regression; size S if kept (one request-level test).
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## tensorlake-prepared-add-test-misses-pending-snapshot-guard.md

- Outcome: `other`.
- Assessment: When you add a Tensorlake host from a prepared snapshot, Farhelm keeps that snapshot from being retired
  until the new sandbox is first seen running. The code does this correctly. The test only checks that the hold is
  released by the later SSH wait. It never checks that the hold is still in place while the sandbox is starting, so
  releasing it too early would go unnoticed. Verified: confirmed. The spec rule is `SPEC_impl.md:6971` ("One per-recipe
  async mutex excludes source selection/create through the first observed Running state from snapshot publication and
  deletion"). In production, `crates/farhelm-helm/src/tensorlake/hosts.rs:485-498` drops the guard only after
  `SandboxStatus::Running`. The test at `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1876-1895`
  acquires `lock_snapshot_use` only after `wait-tensorlake-ssh`. No test observes the Pending period (the other
  `lock_snapshot_use` tests in `tensorlake/prepared_tests.rs` cover build and retirement exclusion, not Add).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — the production behavior matches the spec; size S if
  kept (a Pending fixture step).
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

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

## trailing-wake-folder-tests-omit-timeouts-removal-and-replace.md

- Outcome: `other`.
- Assessment: The tests for "wake the host, then launch into an existing folder" never let the 60-second waits actually
  run out. They never remove or retarget the host during the wake, and never complete a Replace with… after a successful
  wake. The product code for these paths exists. Nothing is shown to be broken. Verified: confirmed.
  `crates/gui/farhelm-gui/src/launcher/wake_tests.rs` has no test that lets either 60-second budget expire (the longest
  advance is 75 s during Resuming, line 136), no host-removal or retarget test, and
  `a_replacement_holds_its_source_through_a_wake_refusal` (`:300`) covers only a 409 refusal. The file is unchanged
  since the reviewed commit apart from `7164b1113`.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — no product defect alleged; size M if kept (several
  GPUI tests).
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-wake-checkout-tests-omit-replace-with.md

- Outcome: `other`.
- Assessment: No test covers Replace with… into a fresh managed checkout on a suspended host, although the parity notes
  claim it is covered. The one product bug this gap hid (Down+Enter on a recent checkout setup skipping the fresh-clone
  warning) is already queued separately as `trailing-replace-with-suspended-checkout-recent-skips-warning.md` (highest).
  Verified: confirmed. The wake-checkout tests in `crates/gui/farhelm-gui/src/launcher/checkout_tests.rs` (from
  `suspended_checkout`/`suspend_with_recents`) build only `LaunchDraft::new`. The product consequence is tracked in the
  separate item (INDEX.md line 87). What remains here is coverage, plus a stale parity claim in
  `docs/gpui/parity/launcher.md`.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — or fold the Replace with checkout test into the fix
  of the highest-bucket sibling, which would need it anyway; size M if kept separately.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-wake-checkout-tests-omit-unverified-and-stopped-hosts.md

- Outcome: `other`.
- Assessment: The wake-checkout tests use only a verified, suspended sandbox. They never use an unverified host (the
  contradictory hint in item 405) or a stopped container. Verified: confirmed. The fixtures assert `identity.is_some()`
  and `Suspended` (`launcher/checkout_tests.rs:2293-2310`, `templates/dispatch_tests.rs:600-649`). Part 1 duplicates the
  product finding in item 405. Part 2 is coverage only; `wake_tests.rs:253` covers a stopped container for folder
  launches, not checkouts.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — part 1 is handled by 405's fix and test; size S–M if
  kept.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-claude-trust-and-antigravity-missing-regression-tests.md

- Outcome: `other`.
- Assessment: Three test-coverage gaps around folder trust. Nothing is wrong in the product; the claim is that some
  behavior is not pinned by tests. Verified: Part 1 (an end-to-end shim test that Claude trust is skipped at home) is
  largely addressed by a4cdd1bbd (#4664). It removed `claude_trust_covers_too_much` and added the child-process test
  `plain_claude_launch_skips_trust_when_home_is_unusable` (`crates/farhelm-supervisor/src/launch.rs:4696`) plus the unit
  test `trust_resolves_only_projects_outside_the_home_ancestor_chain` (`agent_kind/claude/trust.rs:555`). A
  child-process test of cwd equal to HOME specifically still does not exist. Part 2 still stands: `pins_approval_cwd`
  (`crates/farhelm-helm/src/workspace_trust.rs:53-58`) has no direct test. Part 3 still stands:
  `workspace_trust_settings_remain_independent` (`crates/farhelm-helm/src/sessions.rs:3119-3145`) omits Antigravity.
  However, `workspace_trust_enabled` is an exhaustive match (`crates/farhelm-helm/src/launches.rs:793-812`), so the
  practical value is low.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Part 1 is mostly covered and parts 2-3 are low value.
  Alternatively fix code for parts 2-3, size S.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

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

## trailing-ensure-hosts-skips-tailscale-validation-for-registered.md

- Outcome: `other`.
- Assessment: In the startup host list that agent builds use (the ensure-hosts file), a Tailscale entry with a malformed
  address is not checked if the host is already registered. When it is checked, the error does not name the entry or
  explain the Tailscale format. Verified: Confirmed (`store.rs:7916-7985`; the Tailscale check sits inside
  `if !already_registered` and returns `InvalidDestination`). SPEC_impl.md says the file "adds what is missing and
  touches nothing else", so skipping registered entries is consistent with that, but also "fails startup with the entry
  named". The file is set outside tests only by the desktop's agent builds (`FARHELM_DESKTOP_AGENT_ENSURE_HOSTS`), so
  this is maintainer tooling.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard. Alternatively fix code XS: move the check into the
  up-front loop with the entry index.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-codex-dual-agent-delivery-tests-raw-substring.md

- Outcome: `other`.
- Assessment: Several credential-delivery tests share one token between Claude and Codex and check only that the token
  appears somewhere in the delivered file, or that Codex's endpoint is missing without checking its token is too. A
  regression that drops one agent's credentials, or leaves an orphan Codex token that makes the supervisor refuse every
  launch, would still pass. Verified: Confirmed. `credential_delivery_tests.rs:629` and others still use raw `contains`.
  The gap guards user-facing behavior: a file the supervisor rejects blocks launches.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Make the fixture's snapshot assert the supervisor's
  invariant and decode the three checks. It guards a launch-blocking failure and is cheap. Size S.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-codex-renamed-executable-missing-admission-and-e2e-test.md

- Outcome: `other`.
- Assessment: The fix that lets Restart resume Codex sessions started from a renamed or versioned executable has only a
  unit test of the shared rule. No supervisor admission test or end-to-end test uses a renamed Codex executable.
  Verified: Confirmed as a coverage gap; the shipped code works. Grok got an end-to-end test of this kind in #4725.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard under Q11 (an end-to-end test is not cheap). If the
  maintainer wants parity with Grok: fix code, size M.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-custom-agent-ancestor-correction-test-misses-pin-correction.md

- Outcome: `other`.
- Assessment: An end-to-end test is meant to prove that when a custom agent's child process reports a conversation
  first, the parent's later report corrects it. The child exits before the parent reports, so the test would also pass
  if the correction were broken and an ordinary dead-process release accepted the parent instead. It never checks the
  correction notice. Verified: Confirmed by inspection.
  `custom_agent_positional_report_corrects_to_the_parent_and_restarts_it`
  (`crates/farhelm/tests/e2e/hook_identity.rs:2215`) asserts no notifications. `NotificationKind::CustomPinCorrected`
  appears in no test (`crates/farhelm-supervisor/src/service/notifications.rs:98`; grep finds no test use). Whether
  `ReportPin::judge` takes the release path here was not run.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code (test only). Keep the child alive across the parent's
  report, and assert the correction notice. Size S.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

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

## trailing-custom-report-forwarder-lock-test-lacks-witness.md

- Outcome: `other`.
- Assessment: This is a maintainer test of the installed `farhelm` command (the forwarder). It checks that while an
  update or uninstall holds a version, a custom agent's conversation report still exits quietly. The test cannot tell
  "blocked by the lock" apart from "the real CLI ran and quietly did nothing", or from "the forwarder failed early".
  Verified: Confirmed. `crates/farhelm/tests/version_lock_forwarder.rs:254-295` runs the forwarder with a cleared
  environment and no session variables. In that case `run_custom_report` exits silently with `no-credential`
  (`crates/farhelm/src/hook.rs:314-338`), and the forwarder's `trap 'exit 0' 0` (`scripts/install.sh:352-358`) hides an
  early failure. The only assertions are exit 0, no output, and that the lock is still held. No shipped bug.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — add session variables and assert no report file or
  log while locked, then a positive run once unlocked; size S.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-launcher-footer-test-checks-stale-bg2-token.md

- Outcome: `other`.
- Assessment: A launcher test meant to catch Reset gaining a filled button background still compares against the old
  fill colour, so it would miss that regression since the button restyle. Verified: confirmed.
  `launcher/view_tests.rs:536-543` checks `palette.bg_2`, while `Tier::Neutral` now paints `palette.button_rest`
  (`components/button.rs:812`). Test gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code with a one-line test fix, or discard. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

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

## trailing-supervisor-positional-pin-tests-omit-custom-agent.md

- Outcome: `other`.
- Assessment: Custom agents share the supervisor's process-pin admission, ancestor correction with its notice, and the
  clearing of the old capture when a named Restart with turns reporting off. No supervisor test exercises any of these
  for custom agents. Verified: Confirmed. A grep finds no test references to `CustomPinCorrected`,
  `custom_pin_corrected` or `CUSTOM_PIN_CORRECTED_TEXT`. The `PinCase` fixtures cover Claude, Muse and Antigravity only.
  No bug is shown.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code — add a CUSTOM `PinCase` and a capture-clearing test;
  size M.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-ticker-live-motion-resume-test-misses-blink-resume.md

- Outcome: `other`.
- Assessment: Test-only. The test that turning Reduce Motion off resumes the cursor blink would still pass if the blink
  never resumed. Verified: Confirmed. crates/gui/farhelm-gui/src/ticker_tests.rs:619-629 asserts `(true, …)` both during
  the pause and after resuming, with no `false` in between. The test is unchanged since 681a3909.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Assert the intermediate `(false, 2)`. Size XS.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-toolbar-650px-height-test-unguarded-else-branch.md

- Outcome: `other`.
- Assessment: A layout test meant to check that the environment button keeps its condensed 23-pixel height in a
  650-pixel pane also passes when the toolbar never condenses. Verified: Confirmed.
  `session_pane/header_tests.rs:3046-3050` picks the expected height from Stop's height without asserting Stop's height.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. One assertion; cheap and it guards the toolbar's
  shipped layout (Q11). Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-bell-stale-reply-guards-untested.md

- Outcome: `other`.
- Assessment: If you press Open Announcements and then click elsewhere or close the panel before the helm answers, the
  late answer must not pop the dialog. The code guards this, but no test checks it. Verified: Confirmed. Only the
  happy-path test exists (`rail_notices/tests.rs`). The guards are at `rail_notices.rs:248,253`.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Two or three short test cases; they guard focus
  behavior users see (Q11), though they may come to more than 20 lines. Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-credential-test-name-says-setup-refuses.md

- Outcome: `other`.
- Assessment: Maintainer-only. A test's name says a failed Claude login lookup makes host setup refuse, but setup
  actually completes and shows a note, as the spec and the test's own description say. Verified: Confirmed
  (`credential_delivery_tests.rs:1353`, `claude_lookup_failure_allows_codex_login_rotation_and_refuses_setup`). The
  callers treat the error as a notice.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code XS (rename the test). A rename is cheaper than
  discussing it.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-link-disclosure-warning-wording-untested.md

- Outcome: `other`.
- Assessment: Test-only. No test checks the hover warning's quoted-text heading ("Link text:", "Link text (partial):",
  none when over budget) or its "and N more" line, all of which the spec fixes. Verified: Confirmed. The branches are at
  crates/gui/farhelm-gui/src/terminal/disclosure.rs:239-259 and :364. A grep finds no assertion on them in `terminal/`
  tests (only a docstring mention at attached/tests.rs:5072).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. One unit test per branch. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-oracle-exception-asserts-only-absence.md

- Outcome: `other`.
- Assessment: Test-only. For an excepted case, the recorded-answers test only checks that the old answer no longer
  holds, so the excepted spoof (`https://example.com` above `.evil.test/x`) could go completely quiet with the test
  still green. Verified: Confirmed. crates/gui/farhelm-grid-model/tests/links.rs:1076-1081 asserts only `!holds`.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Require a Mismatch for the excepted grid keys, or
  add the case as a loud case. Land it with 105. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-hidden-row-test-step-cannot-fail.md

- Outcome: `other`.
- Assessment: Test-only. A test step meant to show that only the hovered row is re-judged while the pointer rests cannot
  fail, because the "other row" belongs to a different hyperlink. Verified: Confirmed. Row 1's `y` uses link index 1
  (`other`) (crates/gui/farhelm-grid-model/tests/links.rs:1786, :1821-1826), so it is never part of `tag`'s label.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Make row 1 continue `tag`. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-label-budget-untested.md

- Outcome: `other`.
- Assessment: Test-only. The rule that link text over 8,192 UTF-16 units is "couldn't be checked" from every row has no
  test. The 128-row rule does. Verified: Confirmed. `MAX_UNITS`
  (crates/gui/farhelm-grid-model/src/link_disclosure.rs:198, :308, :328) has no test. `over_budget()` is used only by
  the row-budget tests (tests/links.rs:1316, :1354).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Add a test across the unit edge, plus the one-row
  two-stretch case. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-joined-second-reading-untested.md

- Outcome: `other`.
- Assessment: Test-only. No test has a label whose first reading mismatches and whose joined reading matches. If the
  rescue broke, honest links split over rows at a dot would start warning unnoticed. Verified: Confirmed by reading
  `labels_over_several_rows_join_and_separate_at_their_seams` (crates/gui/farhelm-grid-model/tests/links.rs:1194-1281).
  Every quiet case matches in the first reading. No mutation was run.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code with 105: one quiet and one loud case. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-scanner-zwsp-scheme-test-vacuous.md

- Outcome: `other`.
- Assessment: Test-only. One case in a scanner test (`\u{200b}https://…`) passes only because its constructed target is
  malformed, so it no longer tests what it claims. Verified: Likely correct. The case is at
  crates/gui/farhelm-grid-model/src/link_claims/tests.rs:1242-1251. A leading ZWSP is its own cluster and folds to
  nothing, so the address is the plain `https://example.test/x`. The claim that the built target is host `https` with
  path `//example.test/x` depends on URL/IDNA handling that I did not execute.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Drop the case, or put the ZWSP inside the scheme.
  Size XS.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-scanner-disguised-scheme-reason-unpinned.md

- Outcome: `other`.
- Assessment: Test-only. No test pins that a scheme spelled with invisible or look-alike characters is reported as
  "look-alike or invisible characters" rather than as a plain mismatch. Verified: Confirmed by reading. The
  scheme-disguise cases assert only `mismatch(...)` (crates/gui/farhelm-grid-model/src/link_claims/tests.rs:~1150-1176),
  and the verdict table's Disguised rows are host-only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Add verdict rows. Size XS.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-scanner-comparison-leeways-untested.md

- Outcome: `other`.
- Assessment: When terminal link text differs from where the link actually goes, the terminal warns. The spec lists
  small differences that must not trigger a warning: one trailing slash, upper- versus lower-case percent escapes, an
  empty `?` or `#`. Since the old reference-answers test stopped checking its "no warning" cases, nothing tests most of
  these allowances, so a regression would make honest links warn without any test failing. Verified: Confirmed.
  SPEC.md:3606 lists the leeways. `link_disclosure.rs:719-723` now asserts only cases the old client warned on and names
  `link_claims/tests.rs` as the quiet-side specification. A grep of `link_claims/tests.rs` finds only the empty-`#` case
  (`tests.rs:722-725`) and `%7e` versus `~` (line 1053, a mismatch), with no trailing-slash, escape-case or `?#` quiet
  rows. The current behavior in `compare.rs:90-122` was not exercised.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Add the 4-5 quiet rows the finding lists. They
  cheaply guard a spec rule in a user-facing security warning. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-link-scanner-host-start-test-docstring-wrong.md

- Outcome: `other`.
- Assessment: Comment-only. A test's docstring says a host start is judged on the original characters "not the
  recognition view". The code and a sibling test use either reading, so a maintainer trusting the docstring could weaken
  the check. Verified: Confirmed. Compare crates/gui/farhelm-grid-model/src/link_claims/tests.rs:782-783 with
  scan.rs:580-586 and the either-reading test. Not clearly settled, because neither SPEC.md nor SPEC_impl.md states the
  host-start rule; only the code's own doc does.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: fix code. Reword the docstring. Size XS.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## installed-agent-runner-can-leave-interrupted-build-running.md

- Outcome: `other`.
- Assessment: The live agent drift runner first builds the test binary, then runs each agent's updater, then runs the
  cases. If someone sends a termination signal to the runner process alone during the build or upgrade phase, the runner
  exits but leaves the Cargo build or the vendor updater running unsupervised. Ctrl-C at the terminal reaches all of
  them and does not cause this. The leftover build keeps using CPU, and the Cargo lock, until it finishes. Verified:
  Confirmed. `build_binary` and `discover` use plain `subprocess.run` (scripts/run-harness-drift.py:44–53, 71–75). The
  updaters added since 470e9e6c have the same gap (`scripts/harness_upgrades.py:102, 127`). Signal forwarding exists
  only inside `recorded_case` (:131–171). The orphaned children are an inference from default SIGTERM handling, not
  observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the trigger is unusual and the child ends on its own. If it is kept, the fix follows
  `recorded_case`'s existing pattern; size S (~30–50 lines with a test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## interrupted-drift-case-retains-failed-verdict.md

- Outcome: `other`.
- Assessment: When the maintainer interrupts a drift run, the retained evidence for the case that was running records a
  FAILED drift verdict, though the run was cancelled. The runner itself exits with the signal's status and the
  recorder's own manifest says "interrupted". Only the case's summary file (`drift-verdict.json`) is wrong. Verified:
  Confirmed, and wider than the finding says. `read_verdict` maps only `timed_out` and recorder errors
  (scripts/run-harness-drift.py:93–107). So any interruption before the test writes its verdict, not only one before
  libtest starts, writes `failed` to `drift-verdict.json` (:156–162). After that comes `SystemExit(128+sig)` (:169–171).
  For a cancellation before libtest starts, the detail text does include "recorder interrupted ...".
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the maintainer caused the interruption and the exit status says so; size S if fixed (~20 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## docs-shot-partial-failure-hides-success-paths.md

- Outcome: `other`.
- Assessment: The docs screenshot capture prints the path of every PNG it produced. When one page fails, it prints none,
  although the sibling pages' screenshots were captured and kept. This hits Boat runs, where one worker now captures
  several pages in one go. The run still fails and nothing is publishable. Verified: Confirmed. On a nonzero engine
  status, `scripts/docs-screenshots.sh:140–147` exits before the listing at :148–169. The engine keeps successful crops
  (`scripts/docs_shots.py:415–441`). The PNGs remain on disk and in the archive, so only the log is missing them.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — this is a missing log listing for files that are still present; size S if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## invalid-boat-member-limit-clears-previews.md

- Outcome: `other`.
- Assessment: When docs screenshots are captured on the Boat pool, a non-numeric `--members` value deletes the selected
  pages' local previews and the publish record, builds the source pack, and only then crashes. The pack is left behind
  in scratch. Verified: Confirmed. `clearDocsOutputs` (scripts/boat/boat-tests.ts:573) and `packGui` (:574) run before
  `Number(a.members)`. A non-numeric value becomes NaN, so `spread` builds zero lists (:494–495) and `lists[i % 0].push`
  throws. This is outside the try/finally that removes the pack (:577–594). A selected run deletes `capture.json`
  anyway, so the extra loss is the selected pages' previews, which can be regenerated, plus the leftover pack.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the trigger is a maintainer typo and the outputs can be regenerated; size S if fixed (~10 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## docs-shots-share-host-details-state.md

- Outcome: `other`.
- Assessment: Docs screenshots are captured one after another in one app run. The "Manage hosts" page's Details shot
  turns on the Hosts panel's Details view and never turns it off. The panel deliberately remembers that choice while
  hidden. So when the "Start a session" host-menu shot runs later in the same app, it shows the host row expanded, while
  capturing that shot alone shows it compact. In a full run, which is what gets published, the published picture
  probably has the expanded row. Verified: Partly confirmed by inspection. `details()` clicks the toggle with no cleanup
  (scripts/docs-shots/manage- hosts.py:43–47). `reset` only closes the Hosts panel (scripts/docs_shots.py:305–324).
  Pages run in file order (:65–90), so manage-hosts comes before start-a-session. `host_menu` reopens Hosts
  (scripts/docs-shots/start-a-session.py:92–96) and crops the host row (:263–271). The panel's persistence across hiding
  is deliberate (crates/gui/farhelm-gui/src/ hosts.rs:11–16). Unverified: whether the expanded details actually fall
  inside the crop, since no render was done.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — return Details to its initial state in the shot's cleanup; size S (~5–10 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## docs-shot-run-can-overwrite-earlier-failure-frame.md

- Outcome: `other`.
- Assessment: When a docs screenshot fails a check after it was taken (wrong size, a frame crop failure), the error says
  the rejected picture "is kept at" the run's capture file "to look at". The next shot in the same app run overwrites
  that file. So the retained evidence for the failure is a different shot's picture. Verified: Confirmed.
  `scripts/gui-agent.py:1907–1925` unlinks and rewrites the run's single `capture.png` and names it in the error.
  `capture_pages` continues to later shots (scripts/docs_shots.py:415–441). Since #4446, Boat and local multi-page
  selections share one app run. docs/docs-shots/SPEC.md:69–71 says a failed run is "retained with its full capture and
  failure messages". That is a tooling spec, not SPEC.md.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — the message is false and a per-shot file name fixes it; size S (~10 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## upgrade-highlights-feed-can-outgrow-desktop-reader.md

- Outcome: `other`.
- Assessment: After a stable upgrade, the Mac app's What's new dialog downloads a website file that collects highlights
  from every release since v0.42.0. The app refuses that file above 2 MiB. Neither the website build nor the release
  checks enforce that limit, so once the file outgrows it, What's new would silently stop appearing. Verified: Correct,
  but the trigger is far off. `crates/farhelm-desktop/src/whats_new.rs:13-14` sets `FEED_LIMIT = 2 MiB` with the comment
  "Bound it during streaming while leaving room for years of notes", which is a deliberate, commented choice. The whole
  `CHANGELOG.md` across about 45 releases is 258 KB, and the feed holds only highlights from v0.42 on. Nothing enforces
  the cap at generation or promotion. Unverified: the deployed size, which is gitignored and generated.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. The headroom is deliberate and large; a size assertion in the generator would be a nicety. Size S
  if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## focused-golden-update-replaces-unpublished-full-captures.md

- Outcome: `other`.
- Assessment: When updating the GPUI screenshot goldens, a full `--update` leaves captures ready to publish. If the
  maintainer then runs a focused `--update --scene ...` to inspect a few scenes, the focused run wipes the same capture
  directory and its completion marker. The full update must be run again before publishing. Verified: Confirmed.
  `run_locked` removes `tool.CAPTURES` for every update, selected or not (scripts/check-gui-goldens.py:289–297), and
  `finish_update` never certifies a selection (:339–341). The documented workflow puts the full update last: "Before
  publishing, run one full `--update`" (.agents/validation.md:735–737), and the runner says "Any explicit selection is
  for inspection" (:22–23). The cost is a repeated full update, about 90 seconds of render time per validation.md, plus
  the build.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — this is outside the documented order and costs one rerun; size S–M if fixed (~30 lines plus a
  test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## golden-output-backpressure-can-block-cancellation-cleanup.md

- Outcome: `other`.
- Assessment: The golden runner prints each worker's buffered log after that worker exits. If the terminal or pipe has
  stopped accepting output, for example output paused with Ctrl-S, a cancellation that arrives during that replay waits
  until output can flow again. Meanwhile the other renderers keep running and the update lock stays held. Verified:
  Confirmed. `shutil.copyfileobj(log, sys.stdout)` blocks (scripts/check-gui-goldens.py:230–233), and `stopping` is
  checked only around the waits (:222–229). Once output drains, the next loop iteration sees `stopping` and the
  `finally` block kills the workers (:237–243). So it recovers on its own after the reader resumes. The test recorder
  bounds it separately. I could not show a documented invocation whose reader stops draining.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the user causes the output pause and it clears when output resumes; size S if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## failed-golden-reference-setup-can-leave-partial-files.md

- Outcome: `other`.
- Assessment: A golden comparison creates its own run directory before it fetches the reference pictures. If the fetch
  fails (missing credentials, a missing picture, a local write error), the directory is left behind under the build
  output, with whatever pictures were already copied. A retry makes a new one. Verified: Confirmed. `mkdtemp` comes
  before `tool.fetch` (scripts/check-gui-goldens.py:301–306), and the only removal is on a successful compare
  (:325–327). With missing credentials the directory is empty. Partial files need a local write failure partway through
  the fetch. Failing comparisons already keep their `compare-*` directories by design.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — it leaves clutter in gitignored build output, much as failing comparisons do by design; size S if
  fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## shell-release-shot-parks-on-removed-control.md

- Outcome: `other`.
- Assessment: The v0.42.0 and v0.43.0 release notes have frozen screenshots: Antigravity in the launcher, and shell
  sessions. Recapturing them on purpose, to correct them, fails. Their capture steps park the pointer on a launcher
  title that the launcher no longer draws, because the launcher's heading is now the command field. Verified: Confirmed.
  Both shots set `park="id:launcher-frame-title"` (scripts/docs-shots/release-notes.py:153, 214). The dialog draws
  `<id>-frame-title` only for a plain title (crates/gui/farhelm-gui/src/components/dialog.rs:415–425). The launcher uses
  `heading_control` (launcher/view.rs:2866). The same file already defines
  `LAUNCHER_PARK = "id:launcher-destination-heading"` (:224). Ordinary runs skip frozen shots, so only an explicit
  recapture hits this.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code, together with 174 — two known-broken selectors; size S (~5 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## antigravity-release-shot-waits-on-folded-field.md

- Outcome: `other`.
- Assessment: The same frozen Antigravity release-note shot also waits for the launcher's conversation field. That field
  now sits folded under "More options", so a deliberate recapture times out before it even reaches the parking problem
  in 173. Verified: Confirmed. The `antigravity` step waits for `id:launcher-conversation`
  (scripts/docs-shots/release-notes.py:92–97). The field is pushed only when `more_options_open` is set
  (crates/gui/farhelm-gui/src/launcher/view/more_options.rs:41–62), and that requires a required conversation or an
  error.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code, together with 173 — click the More options toggle first; size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## container-comparison-targets-removed-control.md

- Outcome: `other`.
- Assessment: The old-app screenshot comparison batch has a "New container" launcher scene. Its script clicks a "Runs
  on" control that the launcher no longer has, because the choice moved into the Host picker. So that scene can never be
  captured, even on a machine with Podman. Verified: Confirmed. The scene clicks `id:launcher-runs-on-New container`
  (docs/gpui/screenshot-scenes.json:772–774). No `launcher-runs-on` id exists in the GUI sources, only
  `templates-runs-on` (launcher/destination_rows.rs:136).
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code if the batch is still used, otherwise retire the scene; size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## launcher-comparison-heading-selector-case.md

- Outcome: `other`.
- Assessment: The launcher scenes in the old-app comparison batch wait for the text "Destination" after opening New. The
  launcher now paints that heading as "DESTINATION", and the wait is case-sensitive. So the scenes stall right after
  opening New, or pass on some other "Destination" text for the wrong reason. Verified: Confirmed. The heading is
  `section_label("DESTINATION", ...)` with "Destination" only as its accessibility name
  (crates/gui/farhelm-gui/src/launcher/view.rs:2575–2578). The gpui-agent text match is a case-sensitive `contains`
  (vendor/gpui-agent/crates/cli/src/main.rs:1358). The scene list has 27 `"Destination"` text waits, more than the three
  scenes the reviewer cited. I did not check whether each one is a launcher wait.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code, together with 175 — retarget to `id:launcher-destination-heading`; size S–M (~30–60 JSON lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## golden-worker-interrupt-bypasses-cancel.md

- Outcome: `other`.
- Assessment: The golden runner treats an interruption of the runner itself as a cancellation. A SIGINT or SIGTERM sent
  only to one render or review worker is instead treated as an ordinary failure. A comparison then still builds a
  partial changes page, though it reports failure. A full update whose optional review worker was killed this way can
  still be marked publishable, with a logged review failure. The captures themselves are complete. Verified: Confirmed
  by inspection. `run_workers` returns a child's negative status unnormalized (scripts/check-gui-goldens.py:234–236).
  `run_locked` checks only the positive `STOP_STATUSES` before normalizing (:313–317). `review_update` logs a non-stop
  status as an optional failure (:264–267). Nothing in the tooling sends a worker-only signal. Certification depends
  only on complete captures (:159–168), so a publishable update has correct pictures and lacks only its review page.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the trigger is an out-of-band signal and the only loss is the optional review page; size S if
  fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## golden-report-omits-failed-scenes.md

- Outcome: `other`.
- Assessment: For visual review, the golden runner writes a portable changes page. The instructions say to hand that
  page's directory to a reviewer along with the intended design. When some scenes failed to render, the page shows only
  the scenes that rendered, and nothing on it says the run was incomplete. A reviewer who only gets the directory can
  approve a design without knowing some scenes were never drawn. The terminal still reports the failure. Verified:
  Confirmed in outline. The page is built whatever the render status (scripts/check-gui-goldens.py:320–331). The handoff
  text is ".agents/validation.md:733–734" ("give the reviewer the entire `target/gui-goldens/changes/` directory along
  with the intended design"). A scene that fails to render is an ordinary event ("A broken scene is reported alongside
  the others", validation.md:724–725), not a rare one. I did not re-trace the page HTML for a coverage banner. I am
  relying on the reviewer's reading of `golden_changes_page.py`.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — the trigger is ordinary and the page is meant to be read without the terminal output; size M
  (~50–100 lines with a test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## capture-stop-retry-can-lose-starting-host.md

- Outcome: `other`.
- Assessment: The agent-screen tool (used by the capture script and `agent-tui.py`) runs an agent in a private terminal
  host, so maintainers can see what the agent draws. Pressing Ctrl-C at one exact moment during startup keeps the run
  directory for a retry. If the retried stop runs before the host has finished starting, it can find nothing, report
  success and delete the directory. The host that comes up afterwards keeps the agent running with no handle left to
  stop it. Verified: Partly correct, confirmed by source. `scripts/agent_screen_host.py:138-144` keeps the directory
  when Popen is interrupted before it returns a handle. On retry, `stop()` (lines 214-256) gets no answer from the
  control socket and has no process or pid, so it relies on one `stop-terminal-hosts` scan. If that scan comes back
  empty it removes the directory. Not verified: whether the controller can still bind and start a host after its run
  directory is deleted (the orphan sequence depends on that), and the CPython interrupt window. Nobody has seen it
  happen.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It needs a Ctrl-C inside Popen plus a retry within the startup window, and the leak is one process
  the maintainer can kill. Size M if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## capture-close-setup-error-discards-frames.md

- Outcome: `other`.
- Assessment: When the screen capture finishes driving an agent, it stops the terminal host and publishes the captured
  screens. If the operating system refuses to create the small control socket at that moment (for example, out of file
  descriptors), the stop skips its fallback cleanup. The screens already captured are thrown away, the remaining agents
  are skipped, and the host may keep running. Verified: Confirmed by source. `HostRun.request` creates the socket
  outside its `try` (`scripts/agent_screen_host.py:155`), so an `OSError` escapes untranslated. `stop()` catches only
  `HostError` (line 217) and never reaches the fallback. `capture_harness`'s `finally` also catches only `HostError`
  (`scripts/capture-agent-screens.py:784-791`), so the error unwinds past publication, and `main`'s `finally` deletes
  the staging area. The trigger (socket allocation failing) was not observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard on its own, or fold it into the 210 fix (move socket creation into the `try`, a two-line change).
  Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## capture-drive-error-discards-staged-screens.md

- Outcome: `other`.
- Assessment: The screen capture drives a real agent through several scenarios that spend real vendor turns, staging
  each screen as it goes. If the agent's terminal stops answering partway, for example a control request times out after
  30 seconds, every screen staged for that agent is deleted. The capture of all later agents is skipped, so the
  maintainer has to rerun everything. Verified: Confirmed by source. `Recorder.attempt` catches only `ScenarioTimeout`
  (`scripts/capture-agent-screens.py:376-387`). A `HostError` from any request (`agent_screen_host.py:166-173`,
  including a socket timeout after `REQUEST_SECONDS = 30`) propagates out of `drive`. The `finally` in `capture_harness`
  (lines 783-791) runs close but then re-raises, so the copy loop (lines 798-799) never runs, and `main` (lines 878-885)
  removes `work_root`, which holds the staging directory. The trigger is ordinary: a stalled or crashed controller, or
  an agent that stops reading its terminal. Not observed in a run.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. Treat a drive `HostError` as an unsuccessful harness outcome, still publish the validated
  screens, and continue with the next harness. Size S (about 20-40 lines plus a stand-in test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## capture-fixture-copy-can-break-screen-pair.md

- Outcome: `other`.
- Assessment: After a capture, the new agent screens are copied into the committed fixture folder one file at a time. If
  a copy fails or is interrupted (disk full, Ctrl-C), a screen and its title can come from different captures or be
  truncated, and the staged originals are deleted afterwards. Verified: Confirmed by source: `shutil.copy2` writes
  straight into `target` (`scripts/capture-agent-screens.py:798-799`), with no atomic swap and no check that a screen
  and its title come from the same capture. The damage is to files in a git working tree, which `git checkout` restores,
  and the fixture tests that run next would likely flag a truncated pair. Neither the trigger nor the damage has been
  observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It needs an I/O failure mid-copy, and git restores the damaged files. Size S-M if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## capture-stop-retry-requires-original-binary.md

- Outcome: `other`.
- Assessment: When the agent-screen tool cannot confirm that it stopped a run, it keeps the run directory and tells the
  maintainer to retry the stop. The retry always uses the exact `farhelm` build that started the run. If that build has
  since been removed or rebuilt elsewhere, the documented retry cannot finish, and the command has no option to name
  another compatible build. Verified: Confirmed by source. `HostRun.__init__` reads the path from the run directory
  (`scripts/agent_screen_host.py:88-92`), the fallback stop runs that path (line 242), and `agent-tui.py stop` takes
  only the run directory (`scripts/agent-tui.py:134-140`, 308-310). Workarounds exist: rewrite the directory's `farhelm`
  file, or run `farhelm internal stop-terminal-hosts --state-dir <dir>` by hand.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It needs a stop that already failed, followed by a missing build, and a manual workaround exists.
  Size S if fixed (a `--farhelm` option on stop, or one line in `.agents/real-agent-tui.md`).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## release-verifier-retry-overruns-deadline.md

- Outcome: `other`.
- Assessment: When a brick-test check hits a provider rate limit, the coordinator retries it within the check's time
  budget. It computes the remaining seconds, then stages files and starts the guest executor. The executor starts a
  fresh countdown for that same number of seconds, so the staging time is not counted and a retry can run slightly past
  the check's deadline. Verified: Confirmed. `releasing/mac-vm-test/brick_test.py:1034-1037` passes a relative
  `remaining` to `task_for` with no recheck at dispatch. The overrun is bounded by staging time, which the Tart command
  limit bounds. No real overrun was observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It does not crash, lose evidence or change a verdict, and the overrun is small. Size S if fixed
  (pass an absolute deadline).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## release-verifier-does-not-bind-relaunch-samples.md

- Outcome: `other`.
- Assessment: The optional ("bonus") upgrade check "reopen" proves that a session process survives the app being quit
  and reopened by comparing a before sample with an after sample. The harness never checks that the before sample was
  taken before the reopen. If the model takes both samples after the reopen, the check can still pass. Verified:
  Confirmed as a missing invariant. `releasing/mac-vm-test/guest_operations.py:1062-1075`: `continuity_reopen_after`
  takes a fresh sample when `reopen_before` is absent and does not check launch order. The prompt asks for the right
  order. This affects bonus passes only. Unverified: whether a model ever deviates.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. A false pass would need the model to deviate, and it is limited to bonus coverage. Size S-M if
  fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## centos-gate-can-stage-sibling-binary.md

- Outcome: `other`.
- Assessment: The CentOS provisioning gate builds the supervisor binary and then tests it from Cargo's output directory.
  When several checkouts share that directory, which the repository allows, another checkout's build can replace the
  binary between the build and the test, so the gate reports results for code it did not build. Verified: Confirmed.
  `scripts/test-provision-centos.sh:64-65` uses `CARGO_TARGET_DIR` directly, and lines 262-271 and 494-500 build, check
  and hand over that same mutable path. The same flaw in the Podman payload builder was fixed in 4ffa4f72e (#4503) under
  Q14 ("Fixed under Q14 ... can otherwise mix source revisions in test evidence"), and that commit changed only two
  lines of this script. Unverified: an actual concurrent replacement.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It misattributes test evidence, which is the class Q14 fixed for Podman. Build into a
  checkout-local or run-owned path, or copy the binary before checking it. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## centos-gate-resolves-relative-target-from-caller.md

- Outcome: `other`.
- Assessment: If someone runs the CentOS gate from outside the checkout with a relative `CARGO_TARGET_DIR`, Cargo builds
  inside the checkout but the static-binary check looks relative to the caller's directory. The gate then fails with
  "missing file" after a successful build, or checks some other file that happens to sit at that path. Verified:
  Confirmed. `scripts/test-provision-centos.sh:64` keeps the path as given, line 263 builds in a subshell after
  `cd "$repo"`, and line 271 checks the path from the caller's directory. Unverified: that a second file ever exists at
  the caller-relative path.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. A crash of a documented command, or a check of the wrong bytes; resolve the path against `$repo`.
  Size S; it can go with 216.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## container-golden-stays-on-loading.md

- Outcome: `other`.
- Assessment: The screenshot golden for the session launcher with "New container" selected is meant to show the
  container setup, including the note under Container host that says whether the recipe is ready or builds on first
  launch. The fake helm behind the golden never supplies the recipe's build state, so the settled picture shows the
  interim "Checking build status…" text (or a host reason) instead of the note users actually see. Verified: Confirmed
  by source. The scene seeds only a `Recipes` collection (`goldens/launcher.rs:276-286`), and no `RecipePlaces` row is
  seeded anywhere under `goldens/` except `goldens/settings.rs:132`. `place_note` (`launcher/view/placement.rs:222-258`)
  shows `note.checking` whenever the place row is missing and the host has no container reason. `wait_for_container`
  (`goldens/launcher.rs:939-957`) stops staging once the recipe value appears. Not verified: the reference pixels
  (`tests/goldens.txt:151-152` records only hashes), and whether the fixture host's container check is usable, which
  decides between the checking text and a reason.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. Seed one recipe-place row (the pattern is in `goldens/settings.rs:123-135`), wait for the drawn
  note, and re-render the golden. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## capture-test-python-minimum-undocumented.md

- Outcome: `other`.
- Assessment: The standalone self-test for the screen-capture tool now needs Python 3.11, but its documented command
  just says `python3`. On an older Python it fails at import rather than with a clear message. Verified: Confirmed.
  `scripts/test-capture-agent-screens.py` uses `contextlib.chdir` (line 10) and `TestCase.enterContext` (lines 115-224),
  both new in Python 3.11. The entries at `.agents/validation.md:1164` and `.agents/run-all-tests.md:136` name no
  minimum. The repository's other tooling already assumes 3.11+ (`.agents/validation.md:58`,
  `docs/test-run-evidence.md:132`, `scripts/plans-landing/README.md:12`), so the real-world impact is close to nil.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. Every documented tooling path already needs 3.11+. Size S if fixed (one clause).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-indented-blank-lines-defeat-release-note-pr-coverage-checks.md

- Outcome: `other`.
- Assessment: The release-notes checker verifies that every PR cited in a release's highlights also appears in its
  folded "All changes" list. A blank-looking line made only of spaces, placed after an entry, makes the checker lose
  that entry's PR citation. The gate can then miss a highlighted PR that is absent from All changes, or reject a list
  that does cite it. Verified: Confirmed. `releasing/check-changelog.py:430-431`: `_cited_prs` appends `" " + ""` for a
  two-space line, and `PR_REFERENCE_RE` at line 209 is anchored at the end (`...\)$`), so the citation stops matching.
  `_check_bullets` at lines 569-571 skips whitespace-only lines, so the two readers disagree. `CHANGELOG.md` is excluded
  from dprint (`dprint.json:15`), so such lines can persist. No occurrence has been observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. A release gate can pass wrongly (misattributed release evidence). Skip whitespace-only lines in
  `_cited_prs` and add a test. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-empty-highlights-release-path-lacks-consumer-test.md

- Outcome: `other`.
- Assessment: A release can legitimately put every entry in "All changes" and highlight nothing. The checker accepts
  that, but no website-generator, desktop-reader or dialog test checks that What's new still behaves sensibly for such a
  release. It would show headers with no highlights, and its subtitle still says "Highlights". Verified: Correct as a
  test gap. The code paths cited (`crates/gui/farhelm-gui/src/whats_new.rs:238-273`,
  `website/scripts/release-notes.mjs`) handle an empty list without error. Every release published so far has
  highlights. No rendering was observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It is coverage only, on a path never used. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## deflake-sweep-fixtures-inherit-git-hooks.md

- Outcome: `other`.
- Assessment: The deflake tool has self-tests for its "is this base commit new enough to sweep?" check. Those tests
  build a small throwaway Git history with ordinary `git commit`. If the developer running them has a global Git hook
  set up (for example a commit-message hook), the hook runs on the fixture commits. If it refuses them, the tests fail
  during setup, before they test anything. Verified: confirmed by reading. `deflake/test-deflake.py:68-79` overrides the
  author identity and signing but not `core.hooksPath`, does not pass `--no-verify`, and does not isolate the global Git
  config. The repo's test-worker self-test does isolate it (`scripts/test-worker/test-test-worker.py:286-287`,
  `GIT_CONFIG_GLOBAL=/dev/null`). Unverified: whether any developer machine has a hook that would refuse these commits.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — a one-line fix (`-c core.hooksPath=/dev/null` on the fixture's `git` calls). Under Q14 it counts
  as "turns a real verdict into a harness failure", though only when such a hook exists. Size S (under 10 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## deflake-base-refusal-test-misses-run-state.md

- Outcome: `other`.
- Assessment: Deflake refuses to sweep a base commit that is too old or on a side branch. Its own rules say that refusal
  happens before any run state is created. The code does that today, but no test checks it: the tests call only the
  check itself, not the start command. A future reordering could slip through. Verified: confirmed, and it is a test gap
  only. `cmd_start` resolves the base first and returns 1 before `prepare_state_root` and `run.create()`
  (`deflake/bin/deflake:1375-1393`). `deflake/SPEC.md:16-17` requires exactly that ("refuses an older base or a side
  branch missing that commit before creating run state"). `deflake/test-deflake.py` never calls `cmd_start`. Behavior is
  correct on main.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the current behavior is correct, and under Q14 a missing test is "make no change"; size S if
  fixed (one test, about 30 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## worker-staging-failure-leaves-in-progress-evidence.md

- Outcome: `other`.
- Assessment: When the test worker can't be reached, or uploading the scripts or the working copy fails, the command
  prints an error and exits 2. The note it leaves in the local output folder, though, still says the run is "staging",
  as if it were still in progress. Verified: confirmed. `scripts/test-worker/test-worker.sh:212` stamps
  `outcome: staging`. A failure at line 214 or 216 calls `die`, which exits without restamping. The error is printed and
  the exit status is right. Only the stamp is stale. The trigger (an unreachable worker) is common, so the "rare
  glitches" filter does not apply.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — under Q14, no evidence is lost or misattributed (the stamp names the right invocation) and
  nothing crashes; size S if fixed (restamp in the two failure branches, about 5 lines plus a test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-drift-turn2-hook-check-passes-on-noop-stop-ack.md

- Outcome: `other`.
- Assessment: The live drift tests check that each installed agent CLI still works with Farhelm: they start a session,
  Restart it, and confirm the resumed agent recalls a code word and that its hook reported the conversation. For Codex
  and Antigravity, the resumed turn's end-of-turn callback can be acknowledged without being verified or recorded. The
  acknowledgment carries the conversation ID inherited from the first turn, so the test passes even if the resumed
  agent's own hook report never got through. Verified: Confirmed by source. `saw_report` with `MatchingIdentity`
  (`drift.rs:512-526`) accepts any new `acked` line naming the snapshot's identity. Codex's enrichment path returns
  `Ok(())` when the previous locator is already resumable (`vendor/codex.rs:171-183`), Antigravity's Stop returns
  `Ok(())` on a failed main-transcript check (`vendor/positional.rs:189-200`), and both are logged as `acked`
  (`report_files.rs:245-248`). The recall witness still proves that resume works, so the gap is that the test does not
  prove the resumed hook path. The unlanded `drift-model-selection` branch keeps the same witness logic, and no queued
  plan's acceptance criteria change it.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code, together with 498, after `drift-model-selection` lands: require the resumed run to have a
  committed report, or distinguish committed acks from no-op acks in the hook log. Size M (the log format or a snapshot
  field changes, and four profiles are affected).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-test-worker-stop-during-prep-mutates-and-prunes.md

- Outcome: `other`.
- Assessment: Stop on the test worker can be called while a run is still preparing: setting up the worker or installing
  the source. When that happens, tests never start, as the docs promise. Preparation still runs to the end, though. It
  copies the cancelled run's source into the worker's tree, deletes the oldest of the 20 kept run results to make room,
  and records an empty "finished, exit 143" result. A later collect of that run succeeds but brings back nothing.
  Verified: confirmed by reading. The stop marker is checked only at `with-lock.sh:79`. Bootstrap (`:88`), the source
  install (`:139-142`), the output directory and pruning (`:144-146`) all run after that, and `run-workload.py:82-84`
  returns 143 before starting anything, which `with-lock.sh:152` records as finished. Every documented promise holds
  (`.agents/test-worker.md:58`: "During preparation, Stop records a cancellation so tests never start"). The evicted
  result is one the next real run would evict anyway. The trigger is not a narrow window (bootstrap takes minutes), so
  no filter applies.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the documented promise holds, and the effects are one early eviction and an empty result; size S
  if fixed (re-check the stop marker before the install and before pruning, about 10 lines plus a test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-test-worker-relative-checkout-breaks-lock-isolation.md

- Outcome: `other`.
- Assessment: The test-worker command takes an optional `--checkout` folder. If someone passes `.` or `..`, the worker
  derives the wrong folder names. It puts that checkout's files directly in the worker's tree root or the home
  directory, and it uses lock files that no other checkout looks at. Setup can then run while another checkout's tests
  are running, and every run repeats the whole worker setup. Verified: confirmed by reading. `test-worker.sh:70` keeps
  the value as given, and `:137-138` takes its basename (`.` or `..`). The worker resolves that path (`with-lock.sh:22`)
  and treats its parent as the worker root (`:25`). Its locks and setup marker then live under that wrong root
  (`:72,81`), but `bootstrap.sh:97` always writes the marker to the fixed `~/farhelm-test-worker/bootstrapped`. Only a
  literal `.` or `..` (or a path ending in one) breaks it. The default and ordinary paths are fine. Unverified: how
  often an agent passes `.`.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — resolve `--checkout` to an absolute path and refuse an empty or dot name, which is the shape the
  default already uses (`test-worker.sh:61`); size S (about 5 lines plus a test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-test-worker-collect-discards-recorded-exit-status.md

- Outcome: `other`.
- Assessment: If the local client disconnects or the copy fails, `collect` fetches a finished test-worker run's results
  later. It throws away the exit status the worker recorded and stamps the local folder only "collected". The pass or
  fail verdict then comes only from the runner's summary, which shows a graceful-stop 143 for a run that was actually
  force-killed (137), or no exit line at all if the runner itself was killed. Verified: confirmed. `with-lock.sh:152`
  writes the code to `finished`. `collect.sh:95-96` prints only the source commit. `test-worker.sh:192` skips `finished`
  when copying, and `:243` stamps `outcome: collected`. A direct run stamps `exit <code>` (`:227`).
  `.agents/test-worker.md:83-84` says the stamp records "the source commit and result". That is a maintainer doc, not
  SPEC, so this is not clearly settled. The self-test at `test-test-worker.py:651` does not check what was collected.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — have collect return the recorded code and stamp `collected (exit N)`; size S (about 15 lines
  plus test assertions).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-test-worker-status-hides-bootstrap-lock.md

- Outcome: `other`.
- Assessment: When a test-worker run gives up after waiting five minutes for worker setup, its message says to check
  `status`. But `status` shows only this checkout's own lock holder. It can't show a setup run on its own, or a setup
  another checkout triggered, so the user sees nothing busy. Verified: confirmed. The message is at
  `with-lock.sh:75-77`. `status.sh:5-7` reads only `<tree>/lock` and `<tree>/lock.holder`. A standalone setup takes no
  tree lock and writes no holder file (`with-lock.sh:121-124`). The consequence is a misleading diagnostic only. The
  trigger, setup taking over five minutes on a fresh worker, is not clearly rare, so the diagnostics filter does not
  clearly match.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — diagnostic only, and under Q14 "make no change"; size S if fixed (a worker-level holder file and
  a status line, about 15 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-tensorlake-rust-tests-summary-head-60-evicts-count.md

- Outcome: `other`.
- Assessment: The Tensorlake Rust test runner decides that a shard passed by reading nextest's "N tests run" line from a
  short summary file. That summary keeps only the first 60 sorted lines of interest. If a passing shard prints enough
  distinct SKIPPED lines that sort ahead of the count line, the count is cut off. The runner then fails a passing run
  and keeps its billed sandbox around as evidence. Verified: partly correct. The mechanism is confirmed:
  `run-rust-shard.sh:123` writes `grep ... | sort -u | head -60`, and `rust-tests.ts:115-120,165` throws when the count
  is missing, before the result is marked collected (`:166`), so the `finally` keeps the sandbox. The full console log
  is already copied locally (`:152`). Not verified: that a real passing shard prints 60 or more such lines. The sandbox
  is checked for a user manager and self-ssh, so most runtime skips should not fire. It also depends on the sandbox's
  locale: in C ordering an unindented `SKIPPED` line sorts after the indented `Summary` line. A sandbox with a broken
  substrate (for example no user manager, where one e2e helper prints a distinct SKIPPED line per test) could reach 60.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code — read the count from the full console log, or write the Summary line outside the 60-line cap; a
  cheap fix for a false failure that keeps a billed sandbox; size S (under 10 lines).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-tensorlake-sandbox-tests-allocates-before-ledger.md

- Outcome: `other`.
- Assessment: The Tensorlake test tooling creates a sandbox first and only then writes it into its local cleanup ledger.
  If the tool is interrupted, or the ledger write fails, between those two steps, a billed sandbox is left running that
  cleanup doesn't know about. Verified: confirmed (`scripts/tensorlake/sandbox-tests.ts:33-40`), and not new: this is
  the finding of `worker-allocation-precedes-ownership-ledger.md`, decided 2026-11-21 under Q14 and executed as "No
  change" in #4559 (`TRIAGE_OUTCOMES_ARCHIVE.md`, "worker-allocation-precedes-ownership-ledger.md": "The finding
  concerns resource ownership and billing before tests start, outside the evidence and verdict scope approved in
  triage"). Q14's recommendation for reference was Discard. The file header promises only that a sandbox is recorded
  "before it is used", which holds.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — the maintainer already decided no change for this exact finding; size none.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-mac-reboot-shell-discards-command-status-tag.md

- Outcome: `other`.
- Assessment: A manual, ignored test measures a remote Mac's reboot. Its shell helper ignores the exit-status record
  that remote commands are now wrapped in. Over an SSH server that loses exit codes (such as Tailscale SSH on macOS), a
  refused reboot command looks successful, and the test then waits up to 30 minutes before failing with a misleading
  message. Verified: Confirmed. `crates/farhelm-helm/src/provisioning/tests/macos_real_tests.rs:929-945` discards the
  tag from `remote_command` and returns the raw `capture_child` result. The test is manual and ignored.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It is a misleading diagnostic and a slow failure in a manual test, with no evidence lost. Size S
  if fixed (route the helper through `run_shell`).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-cleanup-skips-p1-retry-and-preflight-evidence.md

- Outcome: `other`.
- Assessment: Brick-test cleanup deletes a pair of test VMs only when its evidence checks out. Otherwise the operator
  must explicitly approve the discard. Cleanup skips some of the checks the P1 verdict applies: lost P1 retry archives,
  and, for the remote-Mac P1 pair, the native readiness evidence and a failed preparation. So VMs can be deleted without
  approval while the report says the P1 check could not run. One route deletes a failed-preparation pair: narrow the run
  to "stop after P0", then run cleanup. Verified: Confirmed. `releasing/mac-vm-test/brick_test.py:1871-1879`:
  `(not p0['required'] or self.verdict(p0) == 'pass')` short-circuits for `p1-remote-mac`, and the later conditions
  never look at `provider_retries`, `native_preflight` or `preparation_error`. `Run.verdict` (1653-1700) does check
  them. Cleanup never deletes retained host-side evidence. What is lost is the live VMs, which may be the only evidence
  of a failed preparation. Not run.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It deletes the only evidence of a failed preparation without the required approval. In the tiered
  branch, require `phase == 'ready'`, no `preparation_error`, intact retry archives for checks in scope and intact
  native readiness evidence, then add tests. Size S-M.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-dead-end-misses-deleted-p0-automation-failure.md

- Outcome: `other`.
- Assessment: Suppose the coordinator is interrupted right after recording a P0 pair's last check, and the operator
  releases that pair. The installer pair then reaches the P2 feedback check, is not told that P2 can never run, and is
  refused. The coordinator records a spurious harness failure on the installer pair instead of a clean skip, which then
  needs a discard approval to clean up. Verified: Confirmed. `releasing/mac-vm-test/brick_test.py:1525-1531`: `dead_end`
  checks only statuses and missing results for deleted P0 pairs, while `p0_executable_complete` (1580-1583) uses
  `verdict()`, which treats `automation_failure` as could not run. `chain_block` then refuses (1538-1540), and the
  `except BaseException` handler records `automation_failure` (1121-1131). The trigger is an interruption in a narrow
  window.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It turns a real verdict (P2 could not run) into a harness failure. Treat a P0-affecting
  `automation_failure` or `preparation_error` on a deleted pair as a dead end. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-exiting-stdio-proxy-mid-scan-fails-remove.md

- Outcome: `other`.
- Assessment: The brick test's Linux "Remove host" check compares the remote processes before and after Remove. An SSH
  proxy that is expected to go away is allowed to. If the proxy exits in the microseconds between two `/proc` reads, it
  is counted as an unexplained leftover Farhelm process, and the check records a false product failure. Verified:
  Confirmed, and deliberate. The docstring of `remote_host_snapshot`
  (`releasing/mac-vm-test/ guest_operations.py:177-178`) says "A missing command line leaves the PID in the full list
  without assigning the proxy role", and the code does that at lines 215-224. Keeping the PID is the safe choice for
  Uninstall's "must leave none". The race window is microseconds.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. The window is microseconds, the choice is documented, and the failure is false, not a false pass.
  If the maintainer wants it fixed, treat PIDs already classified as proxies in the before snapshot as proxies (size S).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-fixture-probe-forwards-sys-executable.md

- Outcome: `other`.
- Assessment: When the brick test's headless remote Mac uses a different base image from the GUI Mac, which the docs
  allow, the clean-fixture probe runs the peer's Python by the GUI Mac's own resolved interpreter path instead of the
  configured `/opt/homebrew/bin/python3`. If Homebrew resolves that path to a version-specific location that differs
  between the images, the fixture check fails before testing starts. Verified: The code is confirmed; the macOS behavior
  is not verified. `releasing/mac-vm-test/fixture_probe.py:229` uses `sys.executable` for `remote-mac`, while
  `guest_operations.py:490` uses `settings['python']`. The docstring at fixture_probe.py:221-223 assumes "Both Macs use
  the same prepared Python installation", which contradicts BRICK-TEST.md:287-290 (a separate `remote_mac_base` needs
  Python at the configured paths). Unverified: that Homebrew Python on macOS reports a resolved, versioned
  `sys.executable`.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It would crash a documented setup. Pass the configured interpreter path to the peer. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-linux-remote-start-accepts-empty-sessions.md

- Outcome: `other`.
- Assessment: The brick test's Linux "Remove host" and "Uninstall" checks must prove that the remote host's saved
  sessions survive. They accept a starting state with zero sessions. In that case "the same sessions after Uninstall" is
  trivially true, even if Uninstall emptied or recreated the database, so a data-loss bug in Uninstall could pass. The
  remote-Mac version of the check already requires at least one session. Verified: Confirmed.
  `releasing/mac-vm-test/checkpoint_contract.py:261`: on Linux, `retained_sample = before['session_ids'] is not None`,
  against `bool(...)` for `REMOTE_MAC_PASS` (257). The Linux path also never calls `require_remote_host_start` at
  `remote_host_before` (only `guest_operations.py:810` does, for the Mac). On `p0-installer-upgrade` the earlier checks
  should leave real sessions, so an empty start needs a tester mistake or a broken fixture.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It weakens a release gate's retained-data proof (misattributed evidence). Require a non-empty
  start on Linux and add the `[]` subtest. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-machine-profile-conflicting-mac-address.md

- Outcome: `other`.
- Assessment: A brick-test machine profile can name the same base VM for both Macs but give two different hardware
  addresses, which is impossible. Loading the profile accepts it, so the typo is caught only hours later, when the
  remote-Mac pass prepares. Verified: Confirmed. `releasing/mac-vm-test/machine_profile.py:156-159` compares only
  against `linux_base`. The run refuses later at preparation, which fails safely.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. The configuration error is caught late but safely. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-old-sessions-nonexistent-unavailable-status.md

- Outcome: `other`.
- Assessment: The brick-test prompt for the "old sessions" check tells the tester to report could-not-run if two
  operations return "status unavailable". Neither operation ever returns that. A mismatched fixture instead fails the
  operation, which the coordinator records as a guest automation failure, and at P1 that becomes could-not-run with an
  "automation failed" label. Verified: Confirmed. `releasing/mac-vm-test/guest_workflow.py:147-148` has the sentence. No
  operation returns `status: 'unavailable'`. The final verdict is could-not-run either way, so only the label and
  wording differ.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. The verdict is the same; the difference is diagnostic wording. Size S if fixed (delete the
  sentence).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-old-sessions-stdout-prints-fail.md

- Outcome: `other`.
- Assessment: While a brick test runs, the coordinator prints a progress line per check, and supervising agents relay
  these to the maintainer. When the tester reports "fail" on the "old sessions" check, the coordinator correctly saves
  could-not-run (that check runs on the previous release), but the progress line still says "fail". Verified: Confirmed.
  `releasing/mac-vm-test/brick_test.py:1117` prints `result['status']` after `record()` rewrote the stored status
  (725-729). The saved state and the report are correct.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. Only the live progress line is wrong, and the final report is correct. A fix is about 3 lines if
  the maintainer prefers.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-p1-automation-failure-drops-unverified-fail.md

- Outcome: `other`.
- Assessment: If the tester reports a product failure at a P1 check but the turn ends in an automation failure, the
  coordinator is meant to keep that claim so the report blocks promotion until someone reviews it. If importing the
  attempt then also fails (for example, an oversized event log left out of the archive), the claim is discarded before
  it is saved, and the report says nothing needs review. Verified: Confirmed.
  `releasing/mac-vm-test/brick_test.py:1054-1061` appends to `unverified_failures` and calls `record()` without
  `save()`. The `except` handler re-enters `locked()`, which reloads from disk (line 599). This contradicts
  BRICK-TEST.md:502-504 ("`unverified_product_failures` surfaces that observation"). The completed-fail path saves first
  (1098-1103).
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It loses release evidence that should block promotion. Save before `record()` and add a test.
  Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-p1-permission-block-swallowed.md

- Outcome: `other`.
- Assessment: The brick test must stop a test pair as soon as it hits an operating-system, computer-use or sign-in
  permission prompt, so the maintainer can handle it. At P1 checks, a permission block reported before any fixed command
  ran, or one that surfaced as a refused tool call, is turned into an "automation failure". The coordinator then records
  that as "could not run" and keeps testing the blocked guest. Verified: Confirmed on both sides.
  `releasing/mac-vm-test/guest_tester.py:295` accepts a missing receipts file only for `fail` or `could-not-run`, and
  lines 864-868 keep only `fail` as a claimed status. `brick_test.py:1043` downgrades any P1 `automation-failure`
  without checking whether the observation is a `permission-block`, despite its own comment "A permission block still
  stops the pair below" (1049-1050). BRICK-TEST.md:118 says "a permission block still stops it". Not run.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It misattributes a permission block and breaks a documented stop. Accept `permission-block`
  without receipts, preserve it on `TesterError`, and exclude it from the P1 downgrade. Size S-M.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-record-settle-breaks-retry-unavailable.md

- Outcome: `other`.
- Assessment: When a P0 check on the installer pair cannot run because of a fixture problem, the documented recovery is
  to repair the fixture and run `retry-unavailable`. Under the default full scope (or stop after P1 or P2), that command
  always refuses on the installer pair. Recording the unavailable result immediately marks the later P1 and P2 checks
  "skipped", and the retry counts those skips as later observations, so the operator has to discard the pair and start
  over. Verified: Confirmed. `releasing/mac-vm-test/brick_test.py:1156-1157` uses `key in entry['results']`, which
  counts skips; `order_block` (1504-1505) ignores them. `settle()` (1463-1487) skips P1 and P2 on any P0 non-pass
  through `order_block` and `dead_end`. Even with the retry check fixed, the persisted skips would keep P1 and P2 from
  running after a successful retry, so the fix needs a design choice: defer the skips, or clear them on retry.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. A documented recovery command always fails on the most important pair. Size M (two coupled
  changes plus tests).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-require-no-failure-allows-rerun-on-failed-p0.md

- Outcome: `other`.
- Assessment: After a P0 check fails or cannot run, the operator should either retry it with `retry-unavailable` or
  discard the pair. Running `test` on that pair again by mistake is not refused. Depending on which check stopped, it
  either quietly marks the tester complete and erases what `retry-unavailable` needs, or records a spurious harness
  failure that permanently blocks the retry. Verified: Confirmed. `releasing/mac-vm-test/brick_test.py:885-886` refuses
  only `running` or `reconciliation-required` testers. `require_no_failure` (1427-1446) ignores P0 results on unreleased
  pairs. A rerun overwrites `entry['tester']` (897). Recorded verdicts stay non-passing; recoverability is what is lost.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. It turns a retriable result into a harness failure. Refuse `test` while the tester phase is
  `failed`. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-unavailable-retry-p2-stale-dict.md

- Outcome: `other`.
- Assessment: A coordinator unit test meant to show that `retry-unavailable` cannot replay a P1 or P2 check actually
  tests P1 twice, because after the first iteration it edits a stale copy of the run state. Verified: Confirmed.
  `releasing/mac-vm-test/test_native_coordinator.py:1129-1139` reads `entry` once, and `retry_unavailable` reloads
  `self.run.data` via `locked()`. The production guard at `brick_test.py:1153` does reject P2 today.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It is only a test gap and the guard works. The fix is about 2 lines if wanted.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-drift-grok-home-overwritten-by-login-shell-rc.md

- Outcome: `other`.
- Assessment: The Grok drift test points Grok at a private copy of its config folder, so the test exercises the hook
  file from the build under test instead of the maintainer's real one. It does this with an environment variable set
  before the agent's login shell starts. A maintainer whose shell startup files export their own `GROK_HOME` overrides
  that variable, so Grok reads the real config. The test then either fails to capture or passes on the real config's
  hook, while the private-setup check still reports success. Verified: Confirmed by source. `PrivateConfig::apply`
  pushes `GROK_HOME` into `seams.launch_env` (`drift/private_config.rs:287-298`). Host launch applies it to the host's
  environment (`host_launch.rs:127-155`) before the `-l -i -c` login shell (`launch.rs:716-722`) sources rc files. Muse
  and OMP overrides instead go through an `env` wrapper prefix, which applies after the shell (`drift.rs`
  `update_override_prefix`). The trigger (an rc-file `GROK_HOME` export) was not observed on any machine. HOME, the
  other root, is rarely re-exported by rc files. The unlanded `drift-model-selection` branch keeps the same `launch_env`
  route.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. Also pass the mirror's root through the wrapper's `env` prefix, as Muse and OMP already do. Size
  S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-drift-private-config-copy-back-aborts-first-error.md

- Outcome: `other`.
- Assessment: After a Grok or Antigravity drift test, the harness copies back any config files the agent rewrote in its
  private copy (a refreshed login token, for example) into the maintainer's real config folder. If one copy fails
  partway, the cleanup stops there. A temporary file is left in the maintainer's real config folder, later files are not
  copied back, so a refreshed login can be lost, and the final check that the real hook files were untouched is skipped.
  Verified: Confirmed on main and still present in the unlanded `drift-model-selection` branch. `copy_back` uses `?` on
  canonicalize, copy, set_permissions and rename with no removal of `staged` (`drift/private_config.rs:217-253`), and
  `finish` returns on the first error before checking later mirrors (lines 312-319). The trigger (a copy, permission or
  rename failure in the maintainer's own config folder) was not observed. The lost item is a vendor login token, which
  is recoverable by logging in again.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It needs a filesystem failure inside the maintainer's own config folder. Size S if fixed (remove
  `staged` on error, collect per-file notes, always run the integrity check).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-drift-runner-list-upgrade-only-conflict.md

- Outcome: `other`.
- Assessment: The drift runner's `--list` option is supposed to list test cases without running agents. Combined with
  `--upgrade-only`, the runner ignores `--list` and upgrades every installed agent CLI instead. Verified: Confirmed on
  main (`scripts/run-harness-drift.py:217-235`: `--list` is outside the mutually exclusive group, and `upgrade_only` is
  checked first). The delivered `drift-model-selection` PR #4781 adds
  `parser.error("--list cannot be used with --upgrade-only")` (branch `run-harness-drift.py:238-239`). That plan's
  acceptance criteria name only the `--category`/`--upgrade-only` refusal, so this is not strictly "covered-plan".
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: other. Remove the item once #4781 lands, since the delivered code fixes it, or treat it as already fixed at
  that point. Size none.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-grok-drift-test-uses-hook-witness-fresh-after-noop-acks.md

- Outcome: `other`.
- Assessment: The same resume witness weakness as 384, for Grok. Since b14f3355c, Grok's turn callbacks from a process
  that is not the pinned one are acknowledged without effect. The Grok drift test still accepts any new acknowledgment
  after Restart as proof that the resumed agent reported its conversation. Verified: Confirmed by source. `drift.rs`
  (Grok profile) uses `HookWitness::Fresh`, and `vendor/grok.rs:180-183` returns `Ok(())` for non-SessionStart callbacks
  that are not `Accept`, which is logged as `acked`. `reported_runtime_identity` (`real_agent_capture.rs:401-411`)
  unwraps only `codex:` locators, so switching Grok to `MatchingIdentity` also needs `grok:` parsing. In a drift run the
  only Grok process is the one under test, so a rival ack is unlikely in practice. Unchanged in the unlanded
  `drift-model-selection` branch. The "Rare edge cases in harnesses without first-class support" filter does not fit,
  because a false test pass is not one of its listed consequences.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code with 384 (one witness redesign covers both), or discard with 384. Size S-M.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-harness-upgrades-orphans-updater-grandchildren.md

- Outcome: `other`.
- Assessment: When an agent CLI's updater or install script hangs until the runner's timeout, or the runner is stopped
  during the upgrade step, only the top-level process is killed. Its download or unpack children keep running and can
  replace the agent's files while the drift test for that agent is already running. Verified: Confirmed by source.
  `run_updater` calls `subprocess.run(..., timeout=UPGRADE_TIMEOUT)` without a new session or group kill
  (`scripts/harness_upgrades.py:123-139`), and `prepare` (`run-harness-drift.py:174-191`) installs no signal cleanup.
  Not established: an updater that has actually hung, or a test actually disturbed by an orphan. The likely result is a
  confusing drift failure or a run against a half-updated CLI. The "Rare, self-correcting glitches" filter is unclear
  here (orphan processes persist until they finish), so it was not applied.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. It needs a vendor updater that hangs past the timeout. Size S-M if fixed (`start_new_session` plus
  `killpg`, and signal handling in `prepare`).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-session-toolbar-golden-height-clips-on-macos.md

- Outcome: `other`.
- Assessment: The screenshot golden for the session toolbar has a fixed window height that fits Linux's 30-pixel header
  rows. On macOS the same rows are 48 pixels tall (the title band), so a macOS capture clips the top and bottom of the
  two rows, and the readiness check does not notice. Verified: Confirmed by source. The scene size is `(1428., 104.)`
  (`goldens/mod.rs:801-802`), the scene has 12 px of padding (`goldens/mod.rs:188`), and two `header_row`s sit 12 px
  apart (`goldens/session_pane.rs:82`) with `min_h(title_band::HEIGHT)` = 48 px on macOS
  (`session_pane/header.rs:561-565`, `desktop/title_band.rs:46-50`): 108 px of content against 104 px.
  `tests/goldens.txt` tracks only Linux captures, so no committed reference is affected. Not run on macOS.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. Derive the height from `title_band::ON`. Size S (one line). Discard is also reasonable while no
  macOS goldens are tracked.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-test-worker-disk-guard-df-du-edge-cases.md

- Outcome: `other`.
- Assessment: Before each run, the test worker checks free disk space and cleans the checkout's own build output when
  space is short. The finding lists four edge cases. A negative free-space reading would be read as positive. A failed
  second reading after a cleanup step counts as zero, which deletes all of this checkout's build output and wrongly
  reports the disk as full of other workloads. A build folder that could not be removed can be blamed on other
  workloads. And a fresh checkout with nothing to remove still says it "removed" output. Verified: partly correct. - (1)
  Likely incorrect on Linux ext4. `scripts/test-worker/remote/env.sh:38` does strip the sign, but ext4's statfs clamps
  available space at 0 rather than going negative. That comes from kernel knowledge; it was not checked on the worker. -
  (2) Confirmed in code: the re-reads at `:65` and `:73` are not guarded the way `:56-59` is, and `:67` and `:76` treat
  an empty reading as 0. It needs the first reading to succeed and to have already called for cleanup, then a later
  reading to fail, for example when Stop's signal kills `df` mid-cleanup. The worst result is a cold rebuild, plus a
  false "FULL BY OTHER WORKLOADS". `.agents/test-worker.md:93-97` tells unattended flows to notify the maintainer on
  that line. - (3) and (4) are confirmed wording imprecisions (`:64,72,77`). - Only the checkout's own build output can
  ever be removed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard — no crash and no lost evidence, and the worst case is a cold rebuild in a rare window. If it is
  fixed, guard only the empty re-reading in (2) and correct the wording in (4); size S (about 15 lines plus a test).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-announcement-override-survives-app-relaunch.md

- Outcome: `other`.
- Assessment: The brick test starts the Mac app with a setting that makes it read the test announcement feed for that
  one launch. When that app restarts itself, through Restart to update or the automatic reopen after its app bundle
  changed, the new instance inherits the setting and keeps reading the test feed. Ordinary users never set it, so only
  release testing is affected, and today's brick test order does not hit it. Verified: Confirmed.
  `crates/farhelm-desktop/src/updater.rs:358-361` removes `LATEST_OVERRIDE_ENV` and `REOPENED_ENV` but not
  `FARHELM_ANNOUNCEMENTS_PATH`, and `bundle.rs` `reopen_command` (around 215-231) removes nothing. `lib.rs:1428` strips
  it from spawned children, with the comment "A brick-test announcement feed belongs to this launch alone". SPEC_impl
  says the reservation applies "in that process" but does not state the relaunch rule in so many words, so this is not
  clearly settled. Unverified: that `open -n` passes the variable on (the code at `bundle.rs:205-208` asserts it does).
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. Add `env_remove` on both relaunch paths and extend the environment test at `updater.rs:1869`.
  Size S. The changelog fragment is `kind: none`.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-announcement-override-check-untested.md

- Outcome: `other`.
- Assessment: The brick test checks that the announcement launch really used the test feed and that the following normal
  relaunch did not. Its only unit test feeds process listings that always agree, so it would keep passing if the check
  stopped comparing. Verified: Confirmed. `releasing/mac-vm-test/guest_operations.py:571-585` is correct today, but
  `test_announcement_override_is_scoped_to_one_candidate_launch` (`test_guest_operations.py:805`) has no mismatch case,
  and the function returns the expectation, not the observed value.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. The guard works and only its test is weak. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-missing-announcement-feed-reported-as-candidate-failure.md

- Outcome: `other`.
- Assessment: If the maintainer has not published the standing test announcement files, the brick test reports the
  candidate's announcements check as failed, even though the app never ran. The P2 verdict then reads "fail" for a
  missing fixture. Verified: Correct, and deliberate. `releasing/mac-vm-test/guest_operations.py:560-564` maps only 404
  to `fail`, and the docstring says "A missing file is an ordinary retained P2 observation". RECIPE.md:244: "A 404 fails
  this check with the missing filename." BRICK-TEST says the same (per the finding). This conflicts with the tester's
  general rule (`guest_tester.py:261-263`: fail only for an actual product failure). P2 is nonblocking, and the detail
  names the missing file.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard. The choice is deliberate and documented, and the detail names the cause. If the maintainer
  prefers, map 404 to could-not-run (size S, with code, tests and docs).
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-brick-test-announcement-check-has-no-delivery-wait.md

- Outcome: `other`.
- Assessment: After the brick test relaunches the candidate with the test feed, nothing waits for the app to download
  the feed before the tester looks at the notification bell. A slow download can produce a false "fail". If the app's
  one download attempt hits a network blip, it does not retry for 12 hours, so the check reports the candidate as
  failed. Verified: The code path is confirmed. `releasing/mac-vm-test/checkpoint_contract.py:55-76` says only "require
  an unread notice", with no wait or recheck. `network_announcements.rs` fetches on the first tick and then every 12
  hours (SPEC_impl.md, "Shared preferences and announcements": "later ticks are 12 hours apart"). Unverified: how often
  this happens. The result can be a false fail, never a false pass.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code. A transient error is misattributed as a candidate failure. Tell the tester to recheck the bell
  with fresh reads for a bounded time (around 60 seconds) and report could-not-run when no notice arrives. Size S for
  the prompt. Telling a network error apart from a signature or schema rejection would take a log-reading operation (M),
  which is optional.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-release-shot-custom-agent-failure-strands-settings-draft.md

- Outcome: `other`.
- Assessment: The release-notes screenshot tool captures each picture in turn and resets the app between them. If the
  v0.45.0 custom-agent picture fails after the agent name is typed, Settings stays open with an unsaved draft that the
  reset's Escape presses cannot close. Every later picture in the run, including the session-list and other docs pages,
  then fails with a misleading "layers remain" error. Published pictures are not affected, because a failed run cannot
  be published. Verified: Confirmed by source. `custom_agent_draft` returns its `discard_draft` cleanup only after both
  fills succeed (`scripts/docs-shots/release-notes.py:324-340`), and `reset` (`scripts/docs_shots.py:305-324`) only
  presses Escape. The reviewer's chain (Escape on dirty Settings asks "Save changes before closing?", and Escape on that
  question means "Keep editing") was taken from the cited GUI code without re-tracing every hop. Not observed in a run.
  `docs/docs-shots/SPEC.md` promises that a failed shot does not cost later shots, but it is not SPEC.md or
  SPEC_impl.md.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code, together with 578. Wrap the steps after the first mutation in `try`/`except` with cleanup, as
  `grouped_sessions` (`release-notes.py:48-80`) already does. Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-release-shot-guarded-replace-failure-leaves-yolo-question.md

- Outcome: `other`.
- Assessment: The same tool as 577. If the v0.45.0 guarded-Replace picture fails after Replace is confirmed, the YOLO
  question stays open under the "bench tokenizer" session row for the rest of the run, because it is drawn inline rather
  than as a layer the reset can see. Later session-list pictures that show that row can include the stray question or
  show its actions as unavailable, with no warning. That run has already failed, so nothing it produces is published.
  Verified: Confirmed by source. The cleanup is registered only after both waits succeed (`release-notes.py:350-374`),
  and `reset` acts only on `layer-` elements (`docs_shots.py:313-324`). That the question is drawn inline and not as a
  layer, and that the row's actions are disabled, were taken from the reviewer's GUI citations without full re-tracing.
  What the later pictures would show was inferred, not observed.
- Decision: q-tooling, "Robustness gaps in maintainer-only tooling": the maintainer picked "Re-apply the Nov 21 rule
  (Q14, "Fix only what corrupts evidence or crashes")". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: fix code with 577 (the same try/cleanup pattern, clicking `id:prompt-cancel`). Size S.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## template-leave-spec-still-inline.md

- Outcome: `fix spec`.
- Assessment: The implementation spec's Templates dialog paragraph still calls the "Save these changes before leaving?"
  question an inline prompt. The app centers it over the dialog, which is what the product spec says. Readers of the
  implementation spec get the wrong picture. Nothing the user sees is affected. Verified: confirmed.
  SPEC_impl.md:9391-9392 says "share a pending departure and the same inline save/discard/keep-editing prompt". The code
  asks through the centered `prompt::ask` (`dialog.rs:588`). SPEC.md:1343-1346 says "The question is centered over the
  dialog and starts with focus on Save. An inferred switch or removed legacy workspace-trust field enables Save but
  never makes leaving ask." The paragraph also omits the narrower leave rule (`editor.rs:410-417`). The rule is stated
  in SPEC.md, so its absence from SPEC_impl is an omission, not a contradiction. No code fix is involved, so the case
  does not fit clearly-settled.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec — replace
  "inline" with "centered", and optionally point to SPEC.md's leave rule. Size S, a few lines. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## sealed-container-delete-spec-conflicts.md

- Outcome: `fix spec`.
- Assessment: The product spec gives two different answers for where a late failure of a whole-container Delete is
  reported. The per-session Delete paragraph says a container that was already known to be sealed shows as being
  deleted, with the reason and Retry. The host-level paragraph (Host Delete container, and Settings → Containers Delete
  of a leftover with a host row) says the reason reaches only the helm's log. Verified: Confirmed in the spec text.
  SPEC.md "Container sessions" about line 5490 has the known-seal exception. About line 5541-5543 ("…if that deletion
  fails after the reply, the host stays listed and the reason reaches only the helm's log") does not. The code follows
  the exception: `hosts.rs:1349-1357` takes `finish_seal_locked` when the host is sealed. The sandbox wording (SPEC.md
  about line 1735) already includes the exception.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec — add the
  known-seal exception to the host-level paragraph. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## tensorlake-key-clear-spec-misstates-leftover-refresh.md

- Outcome: `fix spec`.
- Assessment: The implementation spec lists a Tensorlake key Clear among the events that re-read "Sandboxes not on this
  helm"; the app actually just empties the list on Clear without asking the helm. Verified: confirmed. SPEC_impl.md
  7232: "after a key Save or Clear in this opening"; same section 7240: "Clear ends the round and empties both lists."
  Code reads only when a key is saved and otherwise clears (`tensorlake_sandboxes.rs: 164-173`); the module docs say "(a
  Clear empties it at once)". Documentation drift only.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec — reword the
  trigger list; size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## release-gate-regeneration-guidance-stale.md

- Outcome: `fix code`.
- Assessment: The comment on the release build gate tells maintainers to regenerate the release workflow with
  `dist init --yes`. The project now regenerates through its own wrapper script, and `dist init` skips CI generation
  under the current `allow-dirty = ["ci"]`. Following the comment would leave the workflow stale, and the next tag's
  plan check would fail. The same comment and `releasing/.agents/workflow.md` also say that a skipped build could
  publish an empty release. That is no longer true, because the artifact handoff checks refuse missing artifacts.
  Verified: Confirmed. `.github/dist-build-setup.yml:7` still says `dist init --yes`, and lines 16-25 still say "the
  release is then created with none of its payloads". `dist-workspace.toml:225` has `allow-dirty = ["ci"]`. The wrapper
  is `releasing/generate-workflow.py`. `.github/workflows/release.yml:429-542` runs
  `releasing/check-artifact-handoff.sh` before publishing. The cargo-dist internals the finding cites were not re-read.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (comments
  only). Point both places at `releasing/generate-workflow.py` and describe the handoff gate. Size S. Run
  `generate-workflow.py --check` afterwards, since the setup file is spliced into the workflow. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## quick-launch-placement-contract-omits-local-host.md

- Outcome: `fix spec`.
- Assessment: Quick launch beside an open sandbox or container session deliberately starts the template on this machine
  rather than in a new environment of the same recipe, which New would preselect. SPEC.md says quick launch "uses
  exactly New's defaults", and "Sandbox sessions" says "New and quick-switcher creation preselect its recipe". Verified:
  confirmed as a wording mismatch. `launcher/entry.rs:24-34` records the 2026-11-16 launcher-area decision. SPEC.md
  1367-1368 and 1676-1678 do not describe the exception.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. State the
  quick-launch exception, and check whether the switcher's "Quick launch:" row falls under "quick-switcher creation".
  Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## launcher-tab-order-omits-command-message.md

- Outcome: `fix spec`.
- Assessment: When the command field shows a message (a template refusal, a held-field refusal, a repository note), Tab
  stops on a scrollable message box in the header. The launcher's documented Tab order does not list that stop.
  Verified: confirmed. `search.rs:775-791` wraps the notices in a `ScrollBox`, which always has a stop
  (`components/scroll_box.rs:10-15`). SPEC.md 1515 lists the order without it, and the general rule in "Reaching
  everything by keyboard" (3888-3889) implies it.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec (one clause),
  or discard. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## terminal-steady-block-comment-stale.md

- Outcome: `fix code`.
- Assessment: Terminal cursor drawing. A program that asks for a steady block cursor gets a blinking one. This is an
  accepted, documented difference. One inline source comment still says the cursor blinks "unless the program says
  otherwise". Only the comment is affected, not behavior. Verified: Confirmed. The inline comment is at
  `crates/farhelm-supervisor/src/grid_output.rs:396-397`. The module docs at `grid_output.rs:45-51` and
  `farhelm-proto/src/grid.rs:192-195` state the exception correctly. No test pins the steady-block exception. No product
  effect.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (one-line
  comment fix, optionally a test that pins the exception) or discard. Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## custom-command-spec-misstates-hook-arguments.md

- Outcome: `fix code`.
- Assessment: The docs page on how Farhelm adds its conversation reporters says a custom command that declares its agent
  "gets the declared agent's reporter where `{farhelm_args}` stands". Grok and Antigravity get theirs from saved
  configuration, which the next bullet on the same page says. Verified: Partly correct, and minor. The sentence is still
  there (`website/src/content/docs/docs/how-it-works/agent-hook-injection.md:116-121`), and the following bullet (lines
  124-126) covers Grok and Antigravity. SPEC.md 1260-1263 is accurate ("replaced by nothing for an agent type that takes
  no such arguments"). `custom-commands.md:49-62` already names Grok's and Antigravity's hook switch. This is about
  custom commands, not custom agents.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: discard. The adjacent
  bullet already corrects it. A one-line narrowing is the alternative. Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-environment-delete-implementation-spec-still-describes-desktop-port-as.md

- Outcome: `fix spec`.
- Assessment: The implementation spec says the desktop app does not yet send the Delete for a session in a sandbox or
  container. It does. Verified: confirmed. SPEC_impl.md:6546-6547 still says "The desktop app does not send this Delete
  yet". The desktop sends `DELETE …?delete_environment_session=true` (`session_list/environment_delete.rs:206-216`,
  dispatched from `session_list/delete.rs`). Documentation only.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec (replace the
  sentence with the shipped request and its late-notice handling). Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-osc-52-copy-rule-omits-primary-selection-distinction.md

- Outcome: `fix spec`.
- Assessment: SPEC.md says a program's OSC 52 write "also copies to the clipboard", and a few lines later calls a
  primary-selection write a kind of "program clipboard write". Together these suggest that a primary-selection write
  replaces the system clipboard. It does not: it only counts toward the drag guidance. Verified: Confirmed wording
  issue. SPEC.md "Terminal experience", around lines 3552-3565, says: "A terminal program's own OSC 52 WRITE also copies
  to the clipboard" and "Any completed program clipboard write, including a primary-selection write, suppresses it".
  SPEC_impl.md "Primary-selection notices" says that the viewer "leaves the system clipboard alone". The code and its
  tests agree with SPEC_impl.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Qualify the
  copy sentence, and keep the notice rule. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-registered-all-numeric-ssh-names-bypass-first-label-shortening.md

- Outcome: `fix spec`.
- Assessment: A host saved under an all-digits-and-dots name with too many parts to be an IP address (such as
  `1.2.3.4.5`) shows the whole string as its short name in the collapsed rail instead of just `1`. It is display only.
  Verified: Confirmed. `farhelm-proto/src/host_name.rs:44-52` keeps any digit-and-dot spelling whole. SPEC.md "short
  name" says "IP addresses stay whole, including abbreviated IPv4 spellings", and `1.2.3.4.5` is not an IP address. But
  this exact behavior was chosen in triage. The archived abbreviated-ipv4-short-label-scope-ambiguity.md completion
  criteria say "keeping an all-digits-and-dots destination whole; no real DNS name has an all-numeric top-level label"
  (PR #4493). The archive is not a coverage source, so this is not covered-spec.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec XS (align the
  SPEC.md wording with the decided rule), or discard. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-delete-scope-comment-describes-old-stop-behavior.md

- Outcome: `fix code`.
- Assessment: Two code comments describe how Stop worked before it began closing terminal tabs. They say Stop leaves
  tabs running and that Delete is the only operation that takes tabs down. SPEC.md now says Stop closes tabs. Only the
  comments are wrong. The code is right. Verified: Confirmed. teardown.rs:370-373 says "delete is the one lifecycle
  operation that takes tabs down with the agent (SPEC.md — stop leaves them running)". launch.rs:105-106 says "Stop and
  restart must reap the agent and leave the session's tabs running". SPEC.md:2144-2150 says Stop "then closes the
  session's terminal tabs", and SPEC_impl.md (Process-tree ownership, :4839-4842) has Stop's agent reap followed by
  separate tab closes. Behavior matches the spec, so this is not clearly settled under the rule, which needs code
  behavior to contradict the spec.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code — reword both
  comments to say Delete reaps the whole session in one sweep while Stop reaps the agent and then closes each tab; size
  S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-exited-session-skips-portable-sweep.md

- Outcome: `fix spec`.
- Assessment: After a session's program exits, its systemd scope can stay alive because of a leftover background job.
  Stop, Restart or Delete then signals the scope, confirms it is gone, and skips the machine-wide fallback search. The
  implementation spec says these cases "always sweep". SPEC.md's own rule allows skipping the search here, and no job
  outside the accepted containment limits is shown to survive. Verified: The mismatch with SPEC_impl is confirmed.
  SPEC_impl.md "Cleanup" (scope-only paragraph): "an already-gone or initially unconfirmed scope keeps the sweep.
  Deleting a session whose agent or a tab already exited, a Stop or Restart after the agent exited therefore always
  sweep." The code (`service/handlers.rs:1120-1132`, `core.rs` Restart branch, `teardown.rs`) passes the recorded scope
  without forcing the sweep, and the reaper skips it for a scope observed live and then confirmed collected. SPEC.md
  "Lifecycle operations" allows exactly that: "Farhelm omits the machine-wide sweep only when containment is established
  for every target, ... every named scope is observed live then confirmed gone, and no explicitly identified process
  remains alive afterwards." The "therefore" in SPEC_impl only follows when an exited program's scope is already gone.
  The user consequence is none beyond the limits SPEC.md accepts. Not clearly settled: either the spec or the code could
  change.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec (narrow the
  SPEC_impl sentence to scopes already gone at cleanup). SPEC.md accepts the behavior, and forcing the sweep re-adds the
  cost that the 2026-11-08 decision removed. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-product-spec-still-promises-no-decision-after-unanswered-environment.md

- Outcome: `fix spec`.
- Assessment: The product spec says that when Delete of a session in a sandbox gets no answer within 20 seconds,
  "nothing is decided". In fact the environment's Farhelm may still delete the session, and even seal the sandbox.
  Verified: confirmed. SPEC.md:1714-1716 still groups "cannot ask" with "no answer within 20 seconds ... nothing is
  decided". SPEC_impl.md:6544-6545 ("the supervisor's decision can still be pending or have committed") and the helm's
  error text (`sessions/sandbox.rs:254-266`) say otherwise. The doc comment at `sandbox.rs:123-128` keeps the absolute
  "Nothing was decided", although `:130-132` qualifies it. Documentation only; no consent gap.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec (plus the doc
  comment). Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-interrupted-card-guide-still-names-only-host-restart.md

- Outcome: `fix spec`.
- Assessment: The implementation guide describes the card shown on an interrupted session as saying "a host restart
  paused the session". The product spec and the shipped card both say a host reboot or a Farhelm update ended the
  session. Only the guide is wrong. Verified: confirmed. SPEC_impl.md:4119-4120 ("it explains that a host restart paused
  the session"). SPEC.md ("Session view": "An Interrupted session keeps its explanation about the reboot or update") and
  `crates/gui/farhelm-gui/src/session_pane/card.rs:280-286` ("A host reboot or a Farhelm update ended …") agree with
  each other. This is not clearly settled, because the fix edits SPEC_impl.md itself.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Align the
  SPEC_impl.md card sentence with SPEC.md, in one edit together with 356. Size S. Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-work-start-spec-still-says-rename-replaces-sampling-state.md

- Outcome: `fix spec`.
- Assessment: The implementation spec contradicts itself on whether renaming a session keeps the current screen-activity
  reading. Code keeps it on Rename and replaces it only on Restart. Verified: confirmed. SPEC_impl.md 4249-4254 still
  groups Rename with Restart ("fresh transition evidence ... without inheriting old screen evidence"). SPEC_impl.md 3692
  says the activity cell is shared "across renames". The code follows the latter, per the reviewer; not re-traced beyond
  the spec text.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec by correcting
  the 4249-4254 paragraph. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-interrupted-recovery-guide-still-assumes-no-prior-process-remains.md

- Outcome: `fix spec`.
- Assessment: The same guide sentence says Restart on an interrupted session needs no confirmation "because no process
  remains". That is not always true. When Farhelm ends a terminal left over from an older release and that cleanup
  fails, the old agent may still be running. The product spec explicitly accepts that a Restart could then start a
  second agent beside it. Verified: confirmed as a documentation mismatch. SPEC_impl.md:4121 states "because no process
  remains". SPEC.md (Durability and resume, the "Old terminals" bullet): "those sessions are treated as ended all the
  same, accepting that a Restart could then start a second agent beside one still running." There is no runtime defect
  beyond that accepted residual.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Qualify the
  rationale with the accepted old-terminal exception, in the same edit as 354. Size S. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-clone-cli-reference-still-omits-environment-exception.md

- Outcome: `fix spec`.
- Assessment: The implementation spec's command reference shows `farhelm agent clone` with a required `--host`. For a
  sandbox or container source, the command takes no `--host` or `--cwd`. Verified: confirmed. SPEC_impl.md:7512 shows
  `--host <name>` as required. SPEC.md:4921-4922 says "`farhelm agent clone` of a sandbox or container session takes no
  `--host` or `--cwd`". Documentation only.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Size S.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-migration-comments-reboot-classification-remains-disputed.md

- Outcome: `fix code`.
- Assessment: A code comment on a very old database migration explains why old sessions are backfilled as "launching"
  rather than "running". Part of its reason is that "running" would make them show as interrupted after the next reboot.
  A "launching" row is turned into interrupted at a reboot too, so that contrast is wrong. Verified: confirmed.
  `crates/farhelm-supervisor/src/store.rs:1871-1879` ("would additionally paint such a session `interrupted` at the next
  reboot"). The boot conversion covers `launching`, `running` and `stop_requested` (`store.rs:6567-6576` and the blanket
  update after it). The migration logic is correct.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (comment only)
  or discard. It is a one-line comment on the version-1 migration. Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-restart-disposition-documentation-still-promises-no-external-change.md

- Outcome: `fix code`.
- Assessment: Inside the supervisor, a failed Restart is classified to decide whether the session's previous state may
  be restored. The comments defining that classification say a "definitive" failure means nothing ran outside the
  supervisor. The current code also treats a replacement that briefly ran and was confirmed cleaned up as definitive.
  That is what the implementation spec describes, so only the comments are out of date. Verified: confirmed.
  `service/core.rs:2793-2812` ("only failures that PROVED nothing spawned are `Definitive`", "Nothing outside this
  process changed"). The same applies to :13021-13033 ("never created (that is what makes the failure definitive)") and
  :13045. Meanwhile `service/core/host_restart.rs:764-791` returns `definitive` after "The start's child may have run
  for a moment" once the end and the sweep are confirmed. SPEC_impl.md (Terminal host, Restart: "The supervisor confirms
  any replacement host has ended … and sweeps the replacement's process tree … After definitive unwind the supervisor
  tries to restore the previous outcome") matches the code. Shipped behavior matches the spec, so this is not clearly
  settled in the sense of code contradicting spec; it is comment drift.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (comments
  only). Restate "Definitive" as "no replacement remains (absent or confirmed cleaned up)" at the three sites. Size S.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-work-start-guide-omits-deferral-while-lifecycle-claim-is.md

- Outcome: `fix spec`.
- Assessment: The session list orders sessions by when each one last started working, and each row shows a status badge.
  The implementation guide says the two cannot drift apart and that a work start is written immediately. The ticker
  deliberately does not wait while a Stop, Restart or Delete holds the session: the badge updates at once, the ordering
  update is postponed to a later pass, and a postponed start is dropped if the run is replaced meanwhile. Verified:
  confirmed. In `service/ticker.rs:1396-1404` the status hint comes before `persist_work_started`. When the claim is
  busy, :1435-1446 parks the start as `deferred_work_start`, and the field's docs (:440-451) say it is deliberately
  run-scoped. SPEC_impl.md:4243 ("cannot drift apart") and :4251 ("A start writes immediately") omit this. The behavior
  is deliberate and commented, and SPEC.md "Waiting between operations on one host" motivates it.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Describe the
  deferral and why the ticker cannot wait. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-recent-activity-product-spec-still-promises-retired-capture-recovery.md

- Outcome: `fix spec`.
- Assessment: The session list's "most recent activity" sort only moves a session when the supervisor sees a new burst
  of work. The product spec says the first look after a "recovery from failed screen captures" only sets a baseline.
  Screen reads can no longer fail that way, and the code that reset the baseline was removed, so the sentence describes
  behavior that no longer exists. It is a documentation error only. Verified: Confirmed. SPEC.md:2788-2790 still lists
  "a recovery from failed screen captures". #4529 (44137d065) removed the matching reset from SPEC_impl.md and the
  ticker under the archived outcome for `trailing-sampler-docs-promise-deleted-capture-failure-reset.md`, which reads:
  Assessment "Host screen reads can't fail, and the reset code is gone", Decision "Correct the text". That entry is in
  TRIAGE_OUTCOMES_ARCHIVE.md, not TRIAGE_OUTCOMES.md, and fixing this means editing SPEC.md, so it does not meet the
  clearly-settled bar. The reviewer's runtime reasoning (skipped samples keep the prior reading) was not re-traced
  beyond that decision.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Drop the
  clause, or say that a skipped sample keeps the prior reading, consistent with the 2026-11-21 "Correct the text"
  decision. Size S (one sentence). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-interrupted-be-superseded-by-inferred-preparation-failure.md

- Outcome: `fix code`.
- Assessment: The protocol's description of the "Interrupted" status says it lasts until the user acts or Farhelm finds
  a definitive launch error. The supervisor can also change it to an error inferred from missing, unreadable or
  incomplete checkout-preparation evidence. That fits the product rule never to repeat an uncertain checkout
  preparation, but the protocol text understates it. Verified: confirmed. `crates/farhelm-proto/src/lib.rs:867-870`
  ("until the user acts or the supervisor finds a definitive launch error"). `service/status.rs:514-535` produces the
  preparation details, and :641-670 turns them into `SentinelError { inferred: true }`. `store.rs:507-525` accepts a
  sentinel error over `Interrupted` whether or not it is inferred. Behavior agrees with SPEC.md (checkout preparation is
  not repeated automatically).
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (protocol doc
  comment only). Name the proved shim failure and the accepted preparation inference separately. Size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-session-pane-stale-doc-on-helm-pushes.md

- Outcome: `fix code`.
- Assessment: A developer comment in the session pane still says the helm pushes nothing to the window while a command
  is in flight, which stopped being true in #4623. Verified: Confirmed.
  `crates/gui/farhelm-gui/src/session_pane.rs:60-63` still has the old wording. The helm's `feed.changed()` branch
  (`ui_channel.rs:1064`) is no longer gated on an in-flight command.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (comment
  only), following the Q12 precedent. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-cli-effort-hyphen-and-spec-drift.md

- Outcome: `fix spec+code`.
- Assessment: For a custom agent, `farhelm agent create --effort -fast` (with a space) is rejected by the command line
  as an unknown flag. `--effort=-fast` and `--model -fast` work, and the custom-agent rules allow a value that starts
  with a hyphen. Separately, SPEC_impl.md still explains that `--agent` and `--effort` don't accept leading hyphens
  because each takes a word from a closed list, which is no longer true for either. Verified: Confirmed.
  `crates/farhelm/src/main.rs:1063` gives `--effort` no `allow_hyphen_values`, while `--model` at line 1060 has it.
  `parse_effort` (lines 963-975) and `validate_model_or_effort` (`custom_agents.rs:74-84`) accept a leading `-`, as
  SPEC.md:494 does (`[a-z0-9_-]`). SPEC_impl.md:7539-7542 states the rule "Every free-text value option on both verbs
  carries `allow_hyphen_values`" but excludes `--agent` and `--effort` as closed sets. `--agent` now accepts any
  well-formed custom ID, which SPEC.md:488 says cannot start with a hyphen, so it does not need the flag. The guidance
  part of the finding was fixed by #4752. This is not clearly settled: the fix edits SPEC_impl, and the finding offers a
  second shape (forbid a leading hyphen in custom values).
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec+code. Add
  `allow_hyphen_values = true` to `--effort` (as the SPEC_impl principle says) and reword the parenthetical so only
  `--permissions` is a closed set. Size S (about 10 lines plus one parse test). Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-codex-fixture-and-notifications-stale-corridor-comments.md

- Outcome: `fix code`.
- Assessment: Comments in the Codex test fixture and in the supervisor's notification kinds still describe the
  attribution rule #4728 removed. Verified: Confirmed. `farhelm-fixtures/src/fake_agent/codex_conversation.rs:28,462`
  and `farhelm-supervisor/src/service/notifications.rs:84` still have the old wording.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (comments
  only), following the Q12 precedent. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-custom-pin-corrected-text-omits-prior-restart-consequence.md

- Outcome: `fix spec+code`.
- Assessment: When a custom agent's nested process reports a conversation first and a parent process later corrects it,
  Farhelm posts a notice. Unlike every other agent's notice, it does not say what a Restart before now would have
  resumed. It also says "its parent" even when a more distant ancestor made the correction. Verified: Confirmed.
  `crates/farhelm-supervisor/src/service/notifications.rs:238-239` reads "Farhelm was following a conversation reported
  by a child process in this session. Restart now uses the conversation reported by its parent." Its own doc comment at
  :237 promises the prior-Restart fact. The rule in SPEC.md:3227-3230 ("says which conversation a Restart before now
  would have resumed ... without claiming no further ancestor can correct it") lists only "a Claude, Codex, Muse or
  Antigravity launch". So the spec does not govern custom agents in so many words, which is why this is not clearly
  settled. SPEC_impl.md:5934-5938 lists the same four agents.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec+code — do it
  together with 536: add custom agents to the rule and align the text with the Codex wording; size S. Size gate: about
  100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over
  that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-docs-github-checkouts-stale-single-dir-cache-path.md

- Outcome: `fix code`.
- Assessment: The managed-checkouts guide in `docs/` still gives the old repository-cache path
  (`repo-cache/<owner>/<name>.git`). Caches now live in generation directories chosen by a `:current` selector, so
  anyone following the guide to inspect or remove a cache by hand will not find it. Verified: confirmed.
  `docs/github-checkouts.md:93-97` still has the old path. SPEC_impl.md describes the layout (`SPEC_impl.md:3706-3707`:
  "`<name>.git:current` holds the UUID of the selected bare cache directory, `<name>.git:gen-<uuid>`"). The website page
  was updated in `1d711f95`, and this file was last touched in `8e109e11f`. This would be clearly settled except that
  the contradiction is in docs prose rather than code, and the "remove by hand" advice needs new wording.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (docs) —
  mirror the website's wording; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-spec-impl-contradiction-orphan-discovery-local-read-bounds.md

- Outcome: `fix spec`.
- Assessment: The implementation spec contradicts itself about whether listing leftover Tensorlake sandboxes bounds its
  local reads. One section says those reads are unbounded, and a later section and the code say they share the reply
  deadline. Users are not affected; the risk is misleading a future change. Verified: confirmed.
  `SPEC_impl.md:6639-6641` ("its ownership reads come after the listing and are not bounded either") conflicts with
  `SPEC_impl.md:7225` ("Discovery's reads of the local key, the provider, and local ownership use the same reply
  deadline") and with `crates/farhelm-helm/src/tensorlake/orphans.rs:126-133`, where both reads use
  `timeout_at(reply_by, …)`. The old sentence's purpose (a sandbox created during the listing is never shown as a
  leftover) still holds, because ownership is still read after listing and a timeout fails the discovery. This is a
  spec-only edit, so it does not fit the "clearly settled → fix code" rule.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec — scope the
  unbounded-read clause to `POST /api/tensorlake/test` and keep the ordering rationale; size S. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-spec-md-security-still-describes-ongoing-codex-merges.md

- Outcome: `fix spec`.
- Assessment: SPEC.md's Security section still describes an ongoing Codex settings merge that can grow the config file
  past 1 MiB. Elsewhere SPEC.md (557-566) now says delivery makes a one-time legacy cleanup and leaves
  `~/.codex/config.toml` alone after it. Verified: Confirmed. SPEC.md 5107-5109 still has "A Codex merge can grow a file
  near that size past it. Later merges then refuse the file..." The spec contradicts itself; the code does not merge.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Reword to say
  the one-time Claude and Codex cleanups read files of at most 1 MiB. Size S. Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-spec-md-status-contradicts-codex-pin-corrected-text.md

- Outcome: `fix spec`.
- Assessment: SPEC.md's wording for the "conversation corrected" notification ("would have resumed", "the agent process
  that started it") no longer matches the Codex notification text shipped in #4728 ("could have resumed", "one of that
  process's ancestors"). Verified: Confirmed. SPEC.md 3229 and 4157/4191 keep the old phrasing, and
  `notifications.rs:211-213` has the new text. The shipped text is the intended one per the reviewer, and the spec was
  not updated with it.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec. Align the
  pin-correction wording with positional attribution. Size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-specs-and-core-omit-grok-and-custom-from-pin-correction-lists.md

- Outcome: `fix spec+code`.
- Assessment: The specs and one code comment still say that only Claude, Codex, Muse and Antigravity correct a
  conversation first reported by a nested process. Grok and custom agents now do it too, each with its own notice. Some
  lists of which agents use the shared process-pin admission also leave out custom agents. Verified: Confirmed. These
  still list only those agents: SPEC.md:3227-3230; SPEC_impl.md:5934-5937; SPEC.md:4254 ("These three use version-0
  captures"); SPEC_impl.md:4471 (attribution list without custom); and
  `crates/farhelm-supervisor/src/service/core.rs:15685`. The code emits `GrokPinCorrected` and `CustomPinCorrected`
  (`notifications.rs:98`, `vendor/positional.rs:71-72`). The Grok notice text also uses the older "child ... the Grok
  process that started it" wording.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix spec+code —
  complete both specs and the comment, and do 490's wording at the same time; size S. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-tensorlake-add-tests-misleading-codex-null-docstring.md

- Outcome: `fix code`.
- Assessment: A Tensorlake setup test's docstring gives the wrong reason why the host records no Codex settings cleanup
  during setup. The real reason is that setup defers Codex work until the supervisor connects, not that a fresh host
  never needs one. The test also never checks that the agent settings were not read. Users are not affected. Verified:
  confirmed. The docstring is at `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1126-1130`. Setup
  resolves no Codex endpoint (`credential_delivery.rs:796-799`, `DeliveryPhase::Setup => None`), and a fresh host that
  reaches reconciliation records `Cleared` without a stamp (`credential_delivery.rs:1145`, matching SPEC_impl's "records
  Cleared directly"). The `read-agent-settings` negative assertion is absent.
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code — reword the
  docstring and add the one negative assertion. The user's docstring rules treat a wrong "why" as worse than none; size
  S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-ticker-reduced-motion-doc-misstates-outline-fade.md

- Outcome: `fix code`.
- Assessment: A developer comment says the permission control's fading outline "shows its final state at once" under
  reduced motion. It actually holds its first, full-color outline, which is why the check exists. Verified: Confirmed.
  `ticker.rs:366-367` versus `components/transition.rs` and SPEC_impl.md "Animation time: the ticker".
- Decision: q-text, "Spec, docs and code comments that describe behavior the code no longer has": the maintainer picked
  "Correct the text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the spec, doc, help or comment prose so it matches the accepted current behavior, as the
  Assessment describes; no behavior change beyond the text itself. Recommendation for reference: fix code (comment
  only), following the Q12 precedent. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## stale-permanent-delete-can-remove-repaired-template.md

- Outcome: `fix spec`.
- Assessment: In the Templates dialog, a stored template that fails today's save rules shows "Delete permanently", which
  offers no Undo. If an agent or the command line repairs that same template after the screen was drawn, and the user
  clicks the still-drawn "Delete permanently" before the redraw, the repaired template is deleted with no Undo.
  Verified: the behavior is confirmed, but my reading is that the spec accepts it: - The label is judged from the stored
  template each frame (`editor.rs:422-437`). - The dialog redraws on store pushes, because GPUI tracks entities read
  during render (`vendor/gpui-pre/src/window.rs:4206-4221`). So the stale window is one frame long, or longer only while
  the dialog is hidden behind another dialog. - Delete goes by name with no revision check (`editor.rs:790-822`). Three
  spec passages cover it: - SPEC.md:1354-1356: "The label the user saw decides: a click on "Delete permanently" never
  offers undo ... even if the template list changed in between." - SPEC.md:5774-5778: "Undo of a delete restores the
  copy the Templates dialog had loaded rather than the one deleted". This presupposes that the deleted revision can
  differ from the loaded one. - SPEC.md:5787-5790: an action inside a sub-second window "uses the target, consent and
  preconditions from the frame the user was shown". Against that, SPEC.md:5794-5796 (2026-10-26) says "Only what a
  destructive action or a launch is shown and fenced against has to be current". SPEC.md:5798-5800 (2026-11-21) allows
  stale controls only "as long as the helm refuses what is no longer valid or the result is trivially undone", and
  neither holds here. Those are the grounds the recording's independent check used to dispute coverage. Unverified:
  whether any path keeps the dialog undrawn for longer while a repair lands.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Add the one-sentence Templates spec clarification: Delete removes the named template whatever
  revision is stored, and the drawn label alone decides Undo. No code change. Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

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

## launcher-resize-changes-enter-action.md

- Outcome: `fix code`.
- Assessment: In a wide Replace with launcher, the user moves the keyboard highlight onto a template, where Enter only
  fills the form. If the window is then narrowed and Enter arrives before the narrow layout is drawn, Enter presses
  Replace instead. Replace creates the new session and deletes the source session, an action the drawn template
  highlight never offered. Verified: confirmed in current code. `launcher/view/search.rs:293-322` (`search_enter`)
  re-checks the live width and goes straight to `press()` when the pane no longer fits. `reflow_search`
  (`search.rs:633-645`), which runs in render (`launcher/view.rs:2294`), also clears the start highlight on narrowing.
  `press_at` (`view.rs:1335-1356`) reuses the source-consent snapshot, which a width change leaves unchanged
  (`view.rs:2301-2307`, `prompt.rs:1225-1240` reuses an equal snapshot), and that snapshot carries no command-field
  action or row identity. This contradicts SPEC.md "Creation" 1221-1222 and "One GUI at a time" 5788-5790. Not verified:
  how often native GPUI dispatches a key between a dirty draw and its presentation. SPEC_impl.md 996-998 says it can
  happen.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. The spec already requires this, but the fix has more than one
  possible shape: bind the command-field Enter to an ActionFrame of the drawn action and highlight, and swallow it on
  change. Share the fix with 9 and 49. Size M, likely over 100 lines with tests. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-recent-enter-before-display.md

- Outcome: `fix code`.
- Assessment: In a wide Replace with launcher, the user presses Up from a template to the recent setup above it and then
  Enter, before the new highlight is drawn. Enter takes the recent setup, and in Replace with that submits a replace
  that deletes the source session. The screen still showed a template, whose Enter only fills the form. Verified:
  confirmed. `step_start_choice` (`search/start.rs:87-103`) moves the live highlight. `take_start_choice` and
  `take_start_row` (`start.rs:107-158`) resolve it live and call `press()` for a keyboard pick. The only frame guard is
  the source-consent snapshot, which this move does not change (`view.rs:1335-1356`). This contradicts SPEC.md 1221-1222
  and 5788-5790. Not measured: native timing.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code, with the same mechanism as 8. Size M, shared. Size gate: about
  100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over
  that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

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

## replace-can-submit-before-checkout-warning.md

- Outcome: `fix code`.
- Assessment: In Replace with, the user can pick a saved repository setup, which switches the replacement to a fresh
  clone (the source's checkout with its changes goes to trash if it was the last session there). On Wayland, a second
  Enter can launch that fresh-clone replacement before the frame that shows the fresh-clone warning ever appears. The
  last thing the user saw said the replacement keeps working in the existing checkout with its changes. Verified: partly
  correct, and the route is plausible. The launcher's shown-snapshot for Replace with, `SourceConsent`, holds only
  `stop_working` and the delete guard (`launcher/view.rs:566-569`, built at `:1290-1295`, captured at `:2302-2305`). An
  `ActionFrame` keeps the same snapshot, already marked shown, across redraws whose value is equal
  (`prompt.rs:1215-1246`). So switching from Folder to Managed checkout does not reset "shown", and `press_at`
  (`view.rs:1335-1355`) accepts the old consent. On a connected host the first Enter is refused by `WAIT_FOR_PREVIEW`
  (`request.rs:403-431`). A second press after the preview's Ready answer submits. The remaining premise, Wayland
  deferring presentation while key dispatch and replies run, is GPUI/compositor behavior I did not check. SPEC_impl.md,
  "Questions: the prompt" (`SPEC_impl.md:997-998`) already accepts that GPUI can draw before it dispatches.
  Sub-second-race filter: the trigger fits. It does not match, because the consequence is a destructive action beyond
  the shown warning (see the bucket rule), and the bucket is `highest`.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Add "replacement leaves the source checkout" to the shown
  snapshot, so a changed value needs a presented frame before a press counts. The same change fixes item 22. Size M
  (about 50-120 lines with tests). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-printed-terminal-link-stale-frame.md

- Outcome: `fix code`.
- Assessment: Clicking a web address printed in a terminal opens it straight away, with no destination display, because
  what is shown is supposed to be what opens. When the program's output changes the address under a resting pointer,
  Farhelm updates the link it will open before the screen is redrawn. A click that lands in that gap (about one frame)
  opens the new address, which the user never saw. Verified: Confirmed for the handler. Output goes through
  `update_shown`, which calls `refresh_hovered_link` (crates/gui/farhelm-gui/src/terminal.rs:855-858) before the next
  paint. The press takes the live hovered link (`pressed_link`,
  crates/gui/farhelm-gui/src/terminal/attached.rs:2738-2744; `press`, :4072-4089), and `LinkPress` carries no
  drawn-frame witness for a printed link (only `file_disclosure`, :2629). Nothing changed this since 0592c5c6.
  Unverified: that a click can actually be dispatched, and the browser opened, before the compositor shows the newer
  frame. Not filtered: the sub-second race filter excludes acting on the wrong target and trust-boundary consequences.
  Not covered by "a change to the link that is undone between a press and its release is not noticed" either, since this
  change happens before the press.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Record the hovered link from the last painted frame and
  require the press to match it (file links already do this). Size M. Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings
  the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-replace-with-folder-enter-out-of-bounds-highlight.md

- Outcome: `fix code`.
- Assessment: In Replace with, the user filters the Folder dropdown and arrows down to a suggestion. A template
  shortcut, or a template batch that was waiting for the model catalog, then changes the folder, so the dropdown shrinks
  while the highlight index stays. Enter now finds nothing at that index, falls through to the main action, and replaces
  the session, deleting the source. With index 0 it silently picks Home instead. Verified: confirmed.
  `launcher/view/folder_list.rs:221-246` guards Replace only when `highlighted.is_none()`. An out-of-range `Some(i)`
  skips that guard, `get(i)` returns `None`, and the function returns `false`. Its caller then calls `press()`
  (`view.rs:859-862`). `apply_now` (`view.rs:1060-1106`) calls `sync_fields` without dropping the folder highlight or
  `filtering`, and the waiting-batch route is real (`view.rs:506-512`). Nuance: once the shrunken list is presented with
  nothing highlighted, Enter launching matches SPEC's "with no highlighted entry it launches". The spec violation is an
  Enter that arrives before that frame (SPEC.md 1221-1222), plus the silent Some(0)-to-Home pick.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Swallow an unresolvable highlight in Replace with, and drop
  the folder highlight whenever a batch changes the folder. This is close to clearly settled, but the
  identity-versus-index choice is a design call. Size S. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-replace-with-suspended-checkout-recent-skips-warning.md

- Outcome: `fix code`.
- Assessment: Replace with on a session in a managed checkout, with its host suspended while the launcher is open: Down
  then Enter on a retained repository setup fills the form and launches in the same keypress. The host wakes, a fresh
  clone is made without the source's changes, and the source session is deleted (its checkout goes to trash if it was
  the last). The fresh-clone warning is never drawn, and the screen said the replacement keeps the checkout with its
  changes. Verified: confirmed by source. The trigger is deterministic, not a race. A suspended host keeps repository
  setups in the empty command pane (`launcher/view/history.rs:219-259`). A folder destination still offers repository
  setups (`launcher/recents.rs:170-174`). `take_start_row` fills and then calls `press` in the same handler when the
  keyboard chose the row (`launcher/view/search/start.rs:130-159`; `take_recent` switches to the repository at
  `view/recents.rs:121-122`). `checkout_refusal` returns `None` for a wakeable host before it looks at the preview
  (`request.rs:422-424`). `press_at` reuses the earlier frame's equal `SourceConsent` (`view.rs:1340-1355`), and
  `begin_host_wake` hides the launcher and starts the wake (`view.rs:2962-2990`). Nothing on this path checks that the
  fresh-clone warning was presented. This contradicts SPEC.md, "Managed checkouts" (`SPEC.md:1797-1800`: "choosing
  Managed checkout in Replace with warns under Where that the replacement starts from a fresh clone without the source
  checkout's changes") and SPEC.md:1221-1222 ("launches that retire a session (Replace with) limited to the action and
  consent already shown"). Not clearly settled: the fix can take more than one shape (bind it in the snapshot, or skip
  the auto-press when the fill changes the destination).
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. The shared fix with item 15 is preferred over special-casing
  `take_start_row`. Size S-M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-setup-rerun-omits-tailscale-allowing-ssh-downgrade.md

- Outcome: `fix spec`.
- Assessment: Re-running a failed setup on a Tailscale host used to send only the destination. A concurrent removal or
  retarget could then have probed and set up the host over ordinary SSH. That route is now refused. What remains: if the
  host's Tailscale setting is switched off while a Re-run is in flight, the probe uses the new, ordinary SSH setting
  rather than the Tailscale setting the menu showed. Verified: Partly fixed. 1ec7724e3 (#4714) changed `rerun_command`
  to the `ssh-host` target with host id and destination (`gui/.../hosts/setup.rs:704-710`). `probe_owned` refuses a
  missing row, a non-SSH row or a moved row as stale (`provisioning/service.rs:907-913`), so route (a), silent
  re-registration or install over ordinary SSH after removal or retarget, is gone. Route (b) remains: `requested_mode`
  is taken only from `ProbeDestination::Ssh` (`service.rs:961-964`), and the stored row's current mode decides access
  (`service.rs:974-980`). That mode is whatever the user (or an approved edit) just saved. Unverified: whether anything
  but the user can toggle Tailscale mode.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-launcher-enter-deleted-custom-agent-chip-submits-previous-agent.md

- Outcome: `fix code`.
- Assessment: In the session launcher, pressing Enter on an agent chip chooses that agent and launches. If a custom
  agent is deleted and the user presses Enter on its chip before the launcher redraws, choosing it fails with "That
  custom agent is no longer registered". The launch still goes ahead with whatever agent the form held before. In
  Replace with, that retires the source session and starts the wrong agent in its place. Verified: Confirmed in code.
  `choose_custom_agent` returns early with only a notice
  (`crates/gui/farhelm-gui/src/launcher/view/choices.rs:119-142`), and the chip group's Enter handler calls `press`
  unconditionally (`choices.rs:392-402`). The permission row guards the same case (`permission_enter`,
  `choices.rs:159-163`). The trigger is narrow. The chips are drawn from the store (`choices.rs:348`), and the window
  redraws on store changes. A deletion can only come from this window's Settings, so the user has to have Settings open
  over the launcher, or close Settings while the DELETE is in flight. The Enter must then land in the gap between the
  store update and the next frame, which is sub-second. Retiring the source is what the user agreed to by pressing
  Replace with's action. What they did not agree to is the replacement agent.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Guard `on_enter` as `permission_enter` does, launching only
  when the chosen custom agent became the draft's. Size S. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## template-leave-prompts-can-stack.md

- Outcome: `fix code`.
- Assessment: In the Templates dialog, leaving an edited template asks "Save these changes before leaving?". Two quick
  departure clicks can open two of these questions before the screen redraws, for example clicking another template's
  row and then Duplicate. Answering the newer one with Discard opens an unsaved copy. Save on the older one then saves
  that copy as a new `<name>-copy` template, not the edits it asked about, and moves on. Verified: confirmed by code
  reading. Two clicks before a redraw are needed, as in item 1. - `ask_leave` (`dialog.rs:550-604`) does not check for
  an open question. `prompt::ask` -> `ask_inner` (`prompt.rs:1449-1750`) has no such check either. - The click path has
  no top-layer gate (`components/mod.rs:409-430`). - The Save answer calls `save_editor` on the live editor
  (`dialog.rs:588-603`). Its comment assumes "the veil keeps the draft as it was", which stacked questions break. -
  Duplicate copies the stored baseline, not the draft (`editor.rs:350-360`). This contradicts SPEC_impl.md:984-986
  ("When a confirmation opens, it freezes what it shows: targets ... Its answer buttons come pre-wired to that frozen
  state") and :1002 ("An answer acts on the value presented with the confirmation it answers"). Consequence: the user
  discarded the edits on one question, and those were unsaved form input. An unwanted duplicate template is saved, which
  Delete removes. Nothing user-owned is lost.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code — allow one leave question per editor and bind Save and Discard
  to that editor, refusing if it was replaced. Size S (about 40-80 lines with a test that sends both clicks with no draw
  between them). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal
  or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## stale-host-retry-can-continue-sandbox-delete.md

- Outcome: `fix spec`.
- Assessment: In Hosts, a sandbox or container's Retry normally means "try the connection again". If the helm reports a
  failed deletion of that environment between the last visible frame and the user's Enter or click, the same Retry
  finishes destroying the environment instead. Verified: partly correct. Retry sends only the host id. `remedies::retry`
  re-reads live state and picks `/retry-delete` when the host is now `Deleting { failure }` (`hosts/menu.rs:301-331`,
  `hosts/remedies.rs:177-191`). Retry has no shown-frame gate, unlike Adopt and Take over. The trigger needs a push that
  arrives inside the draw-before-dispatch window (sub-second). The consequence is limited: `Deleting { failure }` arises
  only for a sealed environment. Sealing happens only when its last session was deleted after a confirmation that warned
  the sandbox goes too, and a seal is permanent (`SPEC.md:1693-1709`). So this finishes a deletion the user already
  confirmed, through a control labelled differently. No route was found by which a non-sealed environment is destroyed.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

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

## launcher-hidden-recent-remains-enter-target.md

- Outcome: `fix code`.
- Assessment: The user arrows onto a recent setup in the left pane, then scrolls that pane with the mouse wheel until
  the row is out of view. Enter still launches that setup, and in Replace with it replaces the source. Nothing on screen
  still names the setup. Verified: confirmed. The pane keeps the highlight through wheel scrolls by design
  (`search/start.rs:162-163`, and `:205-211` only reveals a changed highlight), and `search_enter` takes it. SPEC.md
  5788-5790 says an action "is ignored when ... its target is no longer in view". That conflicts with the deliberate,
  commented code choice ("a wheel turn can reveal other choices without moving Enter's target"). No runtime scroll check
  was run.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Keep the highlight but swallow Enter while the highlighted row
  is not visible, as part of the Q-A mechanism. Size S-M. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## hook-save-launches-into-later-folder.md

- Outcome: `fix code`.
- Assessment: The launcher can ask "Set up Restart for Grok/Antigravity?". If the user switches hosts while that
  question is open and then answers **Set up and launch** before the new host's remembered folder has loaded, the
  session can start in that remembered folder. The folder field showed the fallback when they answered. Verified:
  confirmed in the code. The open question is retired only when the agent changes or when an edit happens while the save
  is out (`hook_question.rs:94-105`). The folder default settles at render time without going through the edit path
  (`view/history.rs:294-324`; `view.rs:2290`, compared with `changed()` at `:995-1009`). The answer does not check
  whether a plain Launch would be refused, and the continuation rebuilds the request from the live draft. The folder it
  lands in is the launcher's own default for the host the user picked, which an ordinary Launch would also have used
  once settled. No spec sentence covers a continuation after a background settle.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code, together with item 10. Either bind the destination at the
  answer, or save the setting and leave the launch to a later press when the draft was refused or changed. Two
  reasonable shapes. Size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launch-without-hook-choice-lost.md

- Outcome: `fix spec`.
- Assessment: The user answers **Launch without**, which skips hook setup for this one launch, and then turns on the
  all-hosts hook setting in Settings before that launch finishes. The launch then sets up hooks anyway. The window
  exists while a new environment provisions, or while a slow Replace with keeps running behind its 504. Verified:
  confirmed that the request carries no per-launch skip (`hook_question.rs:165-174`), and the final dispatch reads the
  current setting. SPEC_impl.md, around 4687-4690, specifies exactly that: "The final create dispatch reads the setting
  from the resolved launch kind, including ... a new environment". SPEC.md (around 3180-3184) says only that Launch
  without "launches with no settings write". The finding's reading that it has a one-launch effect is an interpretation.
  The setup that runs is one the user explicitly opted into moments later.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-sandbox-confirmation-does-not-bind-billed-size-it-displays.md

- Outcome: `fix code`.
- Assessment: Launching into a new Tensorlake sandbox asks "Create a sandbox for this session?" and shows the recipe's
  CPU, memory and disk. If that recipe's size is edited (in Settings or from another client) while the question is open,
  or before the helm admits the launch, "Create and launch" creates the sandbox at the new size, which may bill more
  than shown. Verified: confirmed by source. The frozen `SandboxAsk` holds the size words (`sandbox_question.rs:48-56`).
  The answer calls `submit(false, true, …)`, which rebuilds the request from the live catalog (`:135-141`). Only a draft
  edit retires the question, not a recipe push. The create body carries recipe, place and `confirmed`, with no size or
  revision, and the helm reads the current definition at admission. SPEC.md, "Sandbox sessions" (`SPEC.md:1646`) names
  the size in the question. SPEC_impl.md:984-989 freezes what a confirmation shows, but also says "No new generic
  precondition protocol is added on the helm or supervisor ... new precondition checks go only where SPEC already
  requires them". A helm-side size check is therefore a design decision. The trigger needs an edit of that same recipe
  concurrent with the question.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. At minimum, retire or refuse the answer in the GUI when the
  recipe's size differs from what was shown (size S). Fully binding admission needs a request field and a helm check
  (size M, and a spec decision under SPEC_impl.md:988-989). Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-visible-new-wake-replaces-tilde-and-quick-launch-error.md

- Outcome: `fix code`.
- Assessment: Open New on a suspended sandbox or stopped container and leave Folder at the `~` it shows, then press
  Launch. The host wakes and the session silently starts in the folder last used on that host, not `~`. If that host's
  history cannot be read, New comes back with "Couldn't prepare quick launch: …" although no quick launch was used.
  Verified: confirmed by source. `draft_refusal` exempts a wakeable host from `FOLDER_DEFAULT_WAITING` for any launcher,
  not just hidden quick launches (`launcher/request.rs:268-272`). `continue_after_wake` routes any launcher whose
  default is still pending into `start_or_wait_for_folder` (`view.rs:2994-3004`). That function settles from history and
  submits, or sets `QUICK_LAUNCH_FOLDER_UNREAD` without checking `quick_launch_unseen` (`view.rs:401-417`).
  `draft.rs:461-463` states the opposite intent for a visible launcher ("launches into it as any `~`"). SPEC.md,
  "Working directory" (`SPEC.md:1092-1097`) prefills from history and says "a refresh of the recent setups never
  rewrites an open draft's folder"; SPEC_impl.md:995-999 binds actions to what was drawn. Not clearly settled: either
  launch into the drawn `~`, or show that the folder will be chosen after wake. Workspace trust and YOLO in an
  unintended folder were raised as a possible `highest` route, but harm to files there is not established.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. A visible launcher settles to the drawn folder at press time,
  and quick-launch wording is limited to hidden launches. Size M. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-ssh-host-setup-reads-live-skip-confirmation-at-settle.md

- Outcome: `fix code`.
- Assessment: When you add an SSH host or re-run its failed setup, Farhelm decides whether to show the setup plan only
  once the remote check finishes. If "Set up new hosts without asking" is turned on while that check runs, for example
  by answering another host's setup with "Set up, don't ask again", the install starts without showing this host's plan.
  Verified: Confirmed by source. `asks_first(cx)` is read when the reply arrives (`gui/.../hosts/setup.rs:793`,
  `hosts/add.rs:1287`). This is a reading of "Questions: the prompt": the setup question only exists once the reply
  arrives, and the Tensorlake sibling deliberately reads its own preference "when the plan arrives" (setup.rs:724-728
  docs). The trigger needs the preference to flip mid-probe. In one window, that means the user's own permanent answer
  on another setup.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code S (capture at click and thread through the callback) if the
  maintainer wants consistency with 72. Otherwise discard. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## launcher-alias-enter-can-apply-an-undrawn-model.md

- Outcome: `fix spec`.
- Assessment: The user types a model-family word while models are still loading, and the results show only "Loading
  models". If the catalog arrives and Enter lands before the redraw, Enter picks the newly preselected agent and model,
  which the user never saw. The same can happen when a history or repository reply replaces a loading view. Nothing
  launches: only the draft changes. Verified: confirmed. `search_enter` recomputes the live results and their default
  row (`search.rs:316-321`, `search_highlight` at `:240-246`). SPEC.md "Creation" (1206-1207) says Enter "picks the
  visible highlighted row, or leaves the explanation in place when there is none". The 2026-11-21 acceptance covers a
  control still on screen, which this case lacks.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## unshown-upgrade-highlights-can-be-recorded-seen.md

- Outcome: `fix spec`.
- Assessment: When the "What's new" dialog opens, an Escape or Enter pressed in the instant between the dialog being
  drawn and reaching the screen can close it and record the highlights as seen, so the user never sees them. Verified:
  Partly correct. The route matches the documented GPUI behavior (a dirty frame is drawn before a key is dispatched;
  SPEC_impl.md 997-999), and the dismissal hook records with no check that the dialog has been shown
  (`whats_new.rs:131-146`). The window is about one frame. SPEC.md 5787-5790 says such input "may be carried out as
  delivered" but also "is ignored when that frame has not yet been shown". SPEC.md 1221-1222 accepts it except for
  destructive confirmations and Replace with, but that sentence sits in the launcher section. Highlights stay reachable
  from the Release notes page.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## notifications-act-on-unshown-list.md

- Outcome: `fix spec`.
- Assessment: Closing the notification panel marks the items in its last displayed list as read, and Clear resolved
  removes the resolved ones. Both use the list as last drawn, so an Escape or click in the instant before a newly drawn
  list reaches the screen can mark a notice read, or clear one, that the user never saw. Late Delete warnings, which
  name a checkout path left behind, are included. Verified: Confirmed as a route; timing about one frame.
  `rail_notices.rs` builds its snapshot at render, and SPEC.md 3281-3284 says "last displayed list". The information is
  not lost: a marked-read notice stays in history, and clearing needs a separate press. SPEC_impl.md 997-999 documents
  the dirty-frame dispatch.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## launcher-recent-highlight-can-change-path-spelling.md

- Outcome: `fix spec`.
- Assessment: A highlighted recent setup is keyed by its folder's canonical path. If a history refresh brings a newer
  launch of the same folder under a different spelling (an alias or symlink), the highlight stays and Enter launches
  with the new spelling. Verified: partly correct. `launcher/recents.rs:77-87` keys the destination by `canonical_cwd`,
  and the representative's literal `cwd` is sent. After a redraw the new spelling is visible. A different actual folder
  would need the aliases to resolve differently at launch, which is unverified. With that route unestablished, the only
  established consequence is a different spelling of the same folder.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## launcher-yolo-reply-uses-current-question.md

- Outcome: `fix code`.
- Assessment: When the user answers the launcher's YOLO question with "Start, and don't ask again on this host", the
  launch waits for that host-setting write. When the write succeeds, the launcher checks only that some YOLO question is
  in flight, not that it is the same one, and then launches the live draft with the earlier answer's source consent.
  Verified: confirmed that the question identity is missing. `launcher/view/yolo_question.rs:272-276` defines `current`
  as `yolo_in_flight()`, and `:362-376` uses it. Whether a second question can be in flight before the first write
  settles is unverified. While the write is out, the footer is only marked busy (`view.rs:2300-2858`) and the form stays
  editable. An edit could retire question 1, and a press, a host change and a permanent answer on question 2 could
  follow before write 1 settles. Neither that path nor its exclusion was established, so no filter was applied.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Compare the question's identity (for example the `Inline`
  handle) instead of "any in flight". Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-shift-click-mixes-live-range-anchor-with-displayed-row.md

- Outcome: `fix spec`.
- Assessment: Shift-click marks the range from the open session to the clicked row. If the open session changes (from a
  helm update) just before the list redraws, the range is anchored on the new open session over the old order, so it can
  mark rows other than the range on screen. Marks are local and reversible, and Stop or Delete on them still asks.
  Verified: Confirmed. `session_list/marking.rs:550` reads the live selection while `mark_rows` uses the last drawn
  order. The window is one frame. SPEC.md 5795-5797 lets convenience state, marks included, lag behind.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmless case: widen the 2026-11-21 SPEC.md sentence (Creation, "A key or click may act before a
  frame shows an arriving background update ...") from the launcher to the whole app, keeping its exceptions for
  destructive confirmations and launches that retire a session. One spec change covers every item recorded this way. No
  code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-replace-with-button-does-not-carry-own-rendered-delete.md

- Outcome: `fix code`.
- Assessment: The Replace with launch button, which confirms the source's deletion, does not carry the delete guard from
  the frame that drew it. On click it reads the latest presented source-warning snapshot. A delayed click could
  therefore use a newer, more permissive guard than the one the user saw, though no such route is shown. Verified:
  confirmed for the binding. `view.rs:2859` passes the generic `Launcher::press`, and `press_at` reads
  `source_frame.current()` (`view.rs:1341-1355`). SPEC_impl.md 994-999 says handlers carry the drawn data and "use the
  snapshot that admitted the press". SPEC.md 2251 says launching carries "the precondition matching what the launcher
  showed at the click". Unverified: a shipped pointer dispatch to an older button after a more permissive snapshot is
  presented.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix code. Capture the `ShownAction` returned by `source_frame.capture`
  in the button's handler. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-guarded-plain-replace-live-read-and-spec-contradiction.md

- Outcome: `fix code`.
- Assessment: On a plain Replace of a YOLO session, the "Replace as guarded instead" answer reads the session's current
  title and folder when clicked, not what the question showed. If the session left the store just before the click, the
  answer closes the question and does nothing, with no message. The implementation spec also contradicts itself about
  which confirmations are frozen. Verified: confirmed. `session_list/replace_yolo.rs:212-246` reads the live row: a
  missing row returns silently, and the change check covers host and launch only. SPEC_impl.md 989-991 names the
  launcher's YOLO question as the one unfrozen confirmation, while SPEC_impl.md 5328 ("the visible source title and
  directory at answer time") adds another. The title carry-over is trivially undone (the 2026-11-21 decision). The
  working directory is immutable on an honest supervisor.
- Decision: q-frame, "Clicks and keys that act on something other than what the user was shown": the maintainer picked
  "Fix the harmful ones, accept the harmless ones". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Harmful case (deletes, retires a session, gives consent, launches, bills or opens a link): make
  the action use the target, consent and preconditions from the frame the user was shown, or ignore it, per SPEC.md "One
  GUI at a time". Recommendation for reference: fix spec+code. Reconcile SPEC_impl's prompt section with 5328, and add
  the existing "Choose Replace again" note on the missing-row branch. Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

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

## trailing-launcher-recents-display-literal-custom-instead-of-agent-name.md

- Outcome: `fix code`.
- Assessment: The launcher's Recent rows and typed search results label every recent setup of a registered custom agent
  "Custom". Several custom agents' setups therefore look alike. Enter on a highlighted Recent row launches it. Hover
  help and the accessible name do carry the ID. Verified: Confirmed. `harness_label(selection.harness)` is used in
  `crates/gui/farhelm-gui/src/launcher/view/search/start.rs:500` and `launcher/view/search.rs:1098`. The summary used
  for hover and accessible names uses the custom ID (`launcher/recents.rs:355-366`). Other launcher text resolves the
  current display name (`crate::harness::custom_name`). No spec sentence requires the row label, but SPEC.md 494-495
  says "The session list uses the current display name and falls back to the saved ID", and the launcher does the same
  elsewhere.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code. Show `custom_name` in both row renderers and underline matches. Size S. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
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

## launcher-keyboard-recents-hide-custom-flags.md

- Outcome: `fix code`.
- Assessment: Two recent setups that differ only in their custom permission flags look the same in the launcher's Recent
  list. A keyboard user highlighting one cannot see the flags. Only pointer hover and the screen-reader name carry them.
  Verified: Confirmed. The row's visible details give the mode but not the words
  (`launcher/view/search/start.rs:466- 478`), and the full summary with flags is only in hover and the accessible name
  (`launcher/recents.rs:355-373`). No spec sentence requires keyboard reveal. SPEC.md 1502 says "The list adds no Tab
  stops". Actual user confusion is unverified.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: discard, since setups that differ only in custom flags are rare. Alternatively add the
  flag words to the details line. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
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

## trailing-custom-agent-draft-retains-stale-builtin-effort.md

- Outcome: `fix code`.
- Assessment: In the launcher, a custom agent's effort is a typed word, but the form can also hold a hidden built-in
  effort from a restored setup or a switch made before the model list loaded. After editing or clearing the custom
  effort, switching to Claude or Codex brings back the old effort. Applying a model template on the custom agent shows a
  false "effort was cleared" notice. Switching away from a custom agent also drops a typed effort without saying so.
  Verified: Confirmed. `restore_selection` sets both `custom_effort` and `effort`
  (`crates/gui/farhelm-gui/src/launcher/draft.rs:629-646`). `choices_for` passes `agent.effort`, not the custom value
  (`draft.rs:952-965`). `reconcile` keeps the old effort when the catalog is unread, even for Custom
  (`draft.rs:983-986`). The custom effort field writes only `custom_effort`.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code. Never hold a built-in effort while the agent is custom, and reconcile from the
  effective value. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
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

## trailing-launcher-agent-tab-omits-deleted-selected-custom-agent-chip.md

- Outcome: `fix code`.
- Assessment: The launcher's Agent tab can open with a custom agent selected (via Clone, Replace with, a recent setup or
  a template) after that registration was deleted, or the agent can be deleted while the launcher is open. No harness
  chip then appears selected, and the deleted ID is not shown anywhere. The custom Model/Effort/YOLO fields stay
  visible, and Launch is refused with "The selected custom agent is no longer registered", which does not name the
  agent. Verified: Confirmed. `crates/gui/farhelm-gui/src/launcher/view/choices.rs:349-391` builds custom chips only
  from the live catalog, and `.chosen(Custom(id))` matches nothing. The Command tab keeps a retained entry
  (`view.rs:2139-2148`), and the draft's comment (`draft.rs:240`) says "The composer keeps a deleted ID visible".
  SPEC.md has no rule for the Agent tab here; it covers the session list's fallback to the saved ID (SPEC.md:497-498).
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — add a retained, unavailable chip like the Command tab's; size S–M. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-launcher-search-recents-omits-custom-agent-id-and-name.md

- Outcome: `fix code`.
- Assessment: In launcher search, recent setups are matched by where they started, the agent type and the model. For a
  custom agent the agent type is always the word "Custom", so typing the agent's ID or display name (including
  `recent:<name>`) never finds its recent setups. Verified: Confirmed.
  `crates/gui/farhelm-gui/src/launcher/search.rs:716-729` matches `harness_label(entry...harness)` only. The spec's
  "searchable by display name and ID" (SPEC.md:492) is about the agents themselves in the Custom group, not recent
  setups, so this is a reading.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — add the ID and current display name to the matched words; size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
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

## trailing-refresh-custom-capabilities-treats-unloaded-collection-as-deleted.md

- Outcome: `fix code`.
- Assessment: Sometimes the launcher has no custom-agent list: before it first arrives, or when the helm could not read
  its catalog on the first try. The launcher then treats any selected custom agent as deleted. Launch is refused with
  "no longer registered" or "was deleted" messages, and the Command tab offers to switch to generic Custom. Verified:
  Confirmed. `crates/gui/farhelm-gui/src/launcher/view.rs:2254-2274` ignores the collection's availability. Settings
  guards it explicitly (`settings/custom_agents.rs:106-121`, "An unavailable collection must not look like a deletion").
  A later failed read keeps the previous entries (`crates/farhelm-helm/src/ui_channel.rs:474-490`). So the false
  "deleted" state needs the first read to fail, or a launcher opened before the first snapshot. I considered the "Rare,
  self-correcting glitches" filter and rejected it. If the first read keeps failing (a corrupt catalog row, which the
  helm refuses to drop: `farhelm-helm/src/custom_agents.rs:27-38`), the false state lasts until repaired. That is the
  filter's "wrong state that persists ... needing ... manual repair" exclusion.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — check availability as `Recipes::of` does, and decide together with 529; size
  S–M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
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

## trailing-template-replay-drops-custom-agent-effort-switching-to-builtin.md

- Outcome: `fix code`.
- Assessment: Suppose a template, or a quick-launch sequence, switches from a custom agent with the effort "high" to a
  built-in agent such as Claude. The desktop drops the effort to Default, while the CLI applying the same templates
  keeps "high". A non-standard custom effort is cleared with no "effort was cleared" notice. Verified: Confirmed by
  reading the code. `LaunchDraft::choices_for` reads `agent.effort`, not the custom effort
  (`launcher/draft.rs:952-965`), and replay switches agents through `choose_harness` (`templates/apply.rs:509-514`). The
  shared `apply_template` fold keeps the effort. SPEC_impl.md:9417 says "the CLI and `tl:` cannot disagree about what a
  template means", so the GUI and CLI disagree for a sequence. A hand switch from a custom agent also drops the effort,
  so what the hand path should do is itself a design choice.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — have `choices_for` read `effort_value()`, plus tests for the notice; size S.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-unseen-quick-launch-deleted-custom-agent-wakes-suspended-host.md

- Outcome: `fix code`.
- Assessment: The user may trigger a quick launch (⚡, a number shortcut, a sequence or Cmd+J) whose template names a
  deleted custom agent, aimed at a suspended sandbox or container host. Farhelm resumes the host first and only then
  refuses the launch as "no longer registered". A Tensorlake sandbox then bills while it runs, until its idle timeout
  suspends it again. Verified: Confirmed by reading the code; I did not observe it running. The out-of-sight path
  reaches `begin_host_wake` (`launcher/view.rs:2962-2971`, which checks `self.refusal`) before
  `refresh_custom_capabilities` runs; that runs only in render, `press_at` and `submit` (`view.rs:1336`, `1425`,
  `2279`). This contradicts that function's own doc ("before dispatch, including template launches prepared out of
  sight"). SPEC.md:388 says "A launch on a suspended host resumes it first", and no spec rule says a launch that cannot
  succeed must not wake the host.
- Decision: q-custom, "Registered custom agents as first-class agent types": the maintainer picked "Yes, everywhere a
  built-in agent is handled". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Treat each registered custom agent as its own agent type wherever a built-in agent is handled:
  name it by display name (falling back to its ID), match it in search, keep per-registration memory, make template
  saves refuse what can never apply, and treat a catalog that is not loaded as its own state rather than "deleted".
  Recommendation for reference: fix code — refresh capabilities in `prepared` before waking; size S. Size gate: about
  100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over
  that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
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

## antigravity-trust-warning-omitted-from-accessibility.md

- Outcome: `fix spec+code`.
- Assessment: Settings shows a warning under each agent's automatic-trust switch: repository hooks, settings and
  commands become trusted, and for Antigravity the per-launch trust list replaces the saved one. A screen reader cannot
  reach those paragraphs. The switch's accessible description carries only the short mechanism text. Verified:
  confirmed, and the scope is wider than reported. The help paragraphs are plain text with no accessible role
  (`settings/card.rs:103-110`), and the tooltip used as the accessible description is the mechanism only
  (`settings/agent_trust.rs:186-233`). The same holds for Codex, Muse and Pi, including first-class Codex, and for
  Settings help text in general. SPEC requires the help's content but says nothing about screen-reader exposure. Actual
  screen-reader output was not checked.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Add a SPEC.md principle: consent, warning and status text must reach assistive technology. Expose
  this consent, warning or status text to assistive technology under that principle. Recommendation for reference: fix
  spec+code. Record the parity principle for consent text, then put the risk sentence into each trust switch's
  accessible description. Size S-M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## approval-omits-automatic-trust-warning-from-accessibility.md

- Outcome: `fix code`.
- Assessment: When an agent asks to launch or restart a session, the approval card shows a "Trusted workspace" or "Not
  trusted" chip and a warning. A screen reader does not hear them, so a screen-reader user can allow a trusted launch
  without being told. Verified: confirmed, and the scope is wider than reported. The whole Permissions fact has no
  accessible node: the permission label, the YOLO or guarded chip and its description, and the trust chip and warning
  (`approvals/card.rs:388-405`, `:544-608`). Other one-line facts get named tab stops (`FactLine`). SPEC.md
  (around 4646) requires the chip visually only. The approval itself is still bound to the drawn request. Real
  screen-reader output was not checked.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Expose this consent, warning or status text to assistive technology under that principle.
  Recommendation for reference: fix spec+code. Expose the Permissions fact, chip and warning text as named card content
  ahead of the answer buttons. Size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## unavailable-antigravity-trust-switch-announced-as-off.md

- Outcome: `fix code`.
- Assessment: While a trust setting is loading or could not be read, its switch is dimmed and cannot be used. Screen
  readers are still told it is simply "off", with no unavailable state. Verified: confirmed. The shared switch component
  reports `Toggled::False` for the visual fallback and exports no disabled flag (`components/checkbox.rs:246-285`,
  `:315`; `settings/configured.rs:90-116`). This affects every disabled switch in the app, not only Antigravity's. No
  spec rule covers it.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Expose this consent, warning or status text to assistive technology under that principle.
  Recommendation for reference: discard, or fold into the parity principle if 40 and 41 get a spec rule. Size S-M for
  the shared component. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## menu-shortcuts-omitted-from-native-accessibility.md

- Outcome: `other`.
- Assessment: Menu items show their keyboard shortcut as a small chip. Farhelm passes the shortcut to the accessibility
  layer as shortcut metadata, but the pinned macOS and Linux accessibility adapters never hand that field to screen
  readers, so the shortcut may not be announced. Verified: Confirmed by the reviewer's source trace, which matches what
  I could check: `components/menu_item.rs:325-327` sets `aria_label` plus `aria_keyshortcuts`, and Cargo.lock pins
  `accesskit_macos` 0.26.3 and `accesskit_atspi_common` 0.19.1. I did not re-read the adapter sources. The cause is
  library behavior, not a Farhelm omission. No spec rule requires the shortcut to be announced (SPEC.md 1401-1402 covers
  only the number-key menu's names).
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard. The cause is upstream; the workaround (repeating the chord in the description) needs a design choice. If
  kept, adding the chord to the accessible description is size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## update-accessible-names-can-collide-across-host-kinds.md

- Outcome: `other`.
- Assessment: In the attention list, each host's Update button is named after the host. An SSH host whose plain address
  equals a container's name would give two Update buttons the same spoken name, so a screen-reader user could not tell
  them apart. Verified: Confirmed by the reviewer's source trace. Not re-traced end to end; the label paths
  (`hosts/update.rs`, `session_list/attention.rs`) were last touched by refactors only. It needs an unaliased SSH
  destination equal to an existing container's display name, with both needing updates. Each button still acts on its
  own host id.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard (very rare coincidence, nothing acts on the wrong host).
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## launcher-held-host-name-clipped.md

- Outcome: `other`.
- Assessment: In Replace with and Restart with, the host is fixed and shown as plain text. A long name or status suffix
  is cut off, and there is no hover or other visual way to read the full name, so look-alike hosts are hard to tell
  apart. Verified: confirmed. `launcher/view/host_picker.rs:386-398` truncates the held value with no `HoverHelp`. The
  accessible name still carries the full text. SPEC.md 1503 ("Held values show as text") does not require full
  visibility. No wrong-target route.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code. Add hover help carrying the full escaped name. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-shell-group-unlabelled.md

- Outcome: `other`.
- Assessment: The Agent tab separates Bash and Zsh from the agent chips with a visual rule only. Screen readers hear no
  "Shells" grouping or shell-specific description. Verified: confirmed. `launcher/view/choices.rs:313-385` draws the
  separated chips with no group name. SPEC.md 1116 ("a Shells group after the agent chips") and 1507 ("a rule before
  Bash and Zsh") are both met visually. Nothing requires an accessible group label. Real screen-reader phrasing is
  unverified.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard, or fix code with an aria label on the shells row. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-host-picker-omits-expanded-state.md

- Outcome: `other`.
- Assessment: The Host picker does not tell assistive technology whether its list is open, and Enter means "pick this
  host or environment" when it is open and "launch" when it is closed. Verified: confirmed. `host_picker.rs:470-479`
  builds the `Role::ComboBox` button without `.expanded(..)`, which exists in `components/button.rs:540-545`. No spec
  rule requires it.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code. Pass the live open state to `.expanded()`; the review says this keeps the trigger's look as
  `Button::discloses` does, which was not checked. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-disabled-setup-enter-gives-no-reason.md

- Outcome: `other`.
- Assessment: In the New session launcher, arrowing onto a recent setup that cannot be used (for example one that would
  change a held field) and pressing Enter does nothing. It gives no explanation, while hovering the same row with the
  mouse shows why. Verified: Confirmed. `launcher/view/search/start.rs:137-139` returns silently when `row_disabled` is
  some, and OptionRow exposes no disabled state to accessibility. Screen-reader announcement is unverified. SPEC.md
  (launcher) says such rows "show as disabled… Their help explains which field stays fixed", but says nothing about
  Enter on them.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code S. Show the refusal line on keyboard activation and expose disabled state.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-announces-two-selected-setups.md

- Outcome: `other`.
- Assessment: In the launcher's list of recent setups and templates, the row that filled the form stays marked. While
  you arrow to another row, assistive technology is told two rows are selected, though Enter acts on only one. Verified:
  Confirmed. `start.rs:276`: `selected = active || source == key`, passed to both `.selected` and aria-selected. The
  persistent source mark is required by SPEC.md (launcher list: "The row that filled the form remains marked"). How a
  screen reader reads it is unverified.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code S. Keep the source mark visual or in the description; aria-selected only for the active row.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## sidebar-retry-warning-missing-accessible-name.md

- Outcome: `other`.
- Assessment: In the sidebar's attention area, a host that is retrying setup or update after an earlier failure shows an
  amber edge for the unacknowledged warning. A screen reader hears the progress wording but not that a warning is
  pending; the host rail's own status text does mention it. Verified: Confirmed. `session_list/attention.rs:253-254`
  lets live progress own the row's text, with the warning kept only for the edge, as a commented choice. SPEC.md
  2991-2992 requires the amber edge and names no accessible phrase for it.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code. Append "setup or update needs attention" to the working row's accessible description. Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

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

## hook-removal-help-missing-from-accessibility.md

- Outcome: `fix code`.
- Assessment: A screen-reader user on the Grok or Antigravity Settings page hears the hook switch and its short tooltip.
  The visible explanation (that the files persist and which path to delete to remove them) is not exposed to assistive
  technology. The same is true of every Settings card's help text. Verified: confirmed by source.
  `settings/hook_settings.rs:38-40` draws the consent and removal text through `card::help`
  (`settings/card.rs:104-110`), which passes a bare `SharedString`. GPUI exposes no node for that: it has no id
  (`vendor/gpui-pre/src/elements/text.rs:84-95,330-336`). The checkbox exposes only its label and tooltip
  (`hook_settings.rs:47`). This is Settings-wide: `card::help` has 6 callers, and `switch_copy` descriptions use it too.
  SPEC.md (~:3177-3178) says "the setting names the path to remove". No spec text requires visible help to be accessible
  text. How it actually sounds on each platform was not measured.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Expose this consent, warning or status text to assistive technology under that principle.
  Recommendation for reference: fix code. Give `card::help` an accessible text id or role, which fixes every Settings
  page at once. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## hook-hint-omits-live-accessibility-notification.md

- Outcome: `fix code`.
- Assessment: When the hook setup hint (or a trust hint) appears above the terminal while focus stays in the terminal,
  screen-reader users may not be told it appeared. The strip is marked as a status area but never asks for a live
  announcement. Verified: confirmed for the code. `session_pane/trust_band.rs:384-389`, the shared `strip`, sets only
  `Role::Status`, and its doc comment claims it "announces itself without taking focus". Elsewhere the GUI does ask for
  announcements explicitly (`.set_live(gpui::accesskit::Live::Polite)` at `launcher/view/checkout.rs:1214`). Whether any
  screen reader announces a Status role with no live setting is unverified. The reviewer's claims about the AccessKit
  sources were not re-checked. SPEC.md (~~:1844) requires Claude's trust hint to be "announced without taking focus".
  The hook hint (~~:3185) has no such words, and SPEC.md (~:3793) leaves "how each kind of item announces itself" open.
  So this is not clearly-settled for the hook hint. The same one-line fix would satisfy the Claude rule.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Expose this consent, warning or status text to assistive technology under that principle.
  Recommendation for reference: fix code. Mark the shared strip as a polite live region, which fixes the trust and hook
  hints together. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## environment-delete-title-unreachable-by-keyboard.md

- Outcome: `other`.
- Assessment: The Delete confirmation for a session in a sandbox or container shows its warning, then the quoted session
  title. With a very long title, part of the title is clipped. Keyboard paging from the answers scrolls the warning, not
  the title, so a sighted keyboard user cannot reveal the full title. A screen reader gets the full text. Verified:
  confirmed by source. In an anchored question, Page Up/Down/Home/End scroll only the first host-value line
  (`prompt.rs:105-108, 1103-1123`). In the environment Delete question that line is the consequence, which carries the
  environment's name (`session_list/delete.rs:172-189`, `session_list/environment_delete.rs:189-199`). The title is the
  second host-value line and scrolls only by wheel. SPEC_impl.md:1034-1036 says a host-value line "(a quoted session
  title) gives up the height and scrolls, with the wheel and with Page Up, Page Down, Home and End from the answers, so
  the keyboard reaches it". "First host-value line" was a recorded deliberate choice (`prompt.rs:108`, 2026-11-16), so
  this is not clearly settled. The "Dialog explanations out of view" filter excludes keyboard-unreachable content.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (page the title line, or every host-value line). Size S-M.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-muted-waiting-dots-bypass-ringless-status-tooltip-path.md

- Outcome: `other`.
- Assessment: On a muted session waiting on a connected host, hovering the red waiting dot shows the row's general hover
  card, with its notch pointing at the row, instead of help for the dot itself. Verified: Confirmed.
  `session_list/seen.rs:139-142` returns the bare mark with no help owner when the session is muted. SPEC.md 2926 says
  "Hovering a live status dot ... explains that mark", and 3121 requires the mark's tooltip to give the status and say
  Muted. I did not check whether the row's card already says "Waiting" and "Muted"; if it does, only the pointer target
  differs.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code. Give the dot a status-mark help owner with the status and mute wording. Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## launcher-preview-path-no-longer-copyable.md

- Outcome: `other`.
- Assessment: In managed-checkout mode, the launcher's Folder line shows the planned checkout path as plain text. The
  earlier read-only field let the user select and copy the exact path before launching. The compact layout dropped that.
  Verified: confirmed. `launcher/view/checkout.rs:1273` draws a `HostSentence`. #4603 (`f249fdff5`) deliberately changed
  SPEC.md's "folder field" to "Folder line" (now SPEC.md 1990). No spec rule requires copying.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard, unless the maintainer wants copy back, in which case add a copy button. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-add-host-tailscale-checkbox-evicts-focus.md

- Outcome: `other`.
- Assessment: In Add host, the Tailscale host checkbox turns disabled while Farhelm checks the destination, and during
  the short pause after approval cards arrive while reviewing an agent's add-host request. If keyboard focus was on the
  checkbox it is lost, and Tab skips it, while the text fields stay usable. Verified: Confirmed. `hosts/add.rs:915-917`:
  `busy` includes `request.paused(cx)`. `add.rs:1589-1600`: `.disabled(busy)`. Disabled checkboxes stop focus tracking,
  per the reviewer's reference to `components/checkbox.rs`. SPEC.md "Things changing underneath" protects focus in what
  the user is typing, and the agent-dialog pause is something the user did not start. The fix shape is a choice
  (read-only vs disabled, gate on probing only).
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code S.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

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

## launcher-underlining-can-stall-search.md

- Outcome: `other`.
- Assessment: When a typed query matches a recent setup, the launcher underlines every match in that row's full saved
  wrapper prefix. The underline code is quadratic in the number of hits, so a wrapper thousands of characters long with
  many hits could make each redraw slow while the user types. Verified: confirmed. `host_text.rs:513-563`
  (`sentence_runs`) runs `hits.iter().any(...)` once per edge. Severity is unverified: a material stall needs an unusual
  stored wrapper of thousands of characters with thousands of hits, and no measurement was taken.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. The fix is cheap: merge the hit ranges and sweep through them once. Size S. `discard` is also
  reasonable.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-new-container-managed-checkout-stretches-host-block.md

- Outcome: `other`.
- Assessment: In the launcher, choosing New container together with Managed checkout leaves a gap of about 300 pixels
  under the Container host picker. That pushes Repository, Name and the agent choices far down the dialog. Nothing is
  lost. The form is just badly laid out on every such launch. Verified: Confirmed by reading.
  `crates/gui/farhelm-gui/src/launcher/view.rs:2431-2446` gives the container-host block `flex_basis(370px)`. In the
  Managed-checkout branch (`:2476-2484`) that block goes into a vertical column, where flex-basis sets height. No GPUI
  render was run.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — apply the basis and minimum width only in the horizontal Folder row, and add a layout
  assertion. Size S.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## antigravity-trust-link-can-hide-read-failure.md

- Outcome: `other`.
- Assessment: The "workspace not trusted" hint has a Settings button that opens the agent's page at its trust switch. If
  Farhelm cannot read the current trust setting, the switch stays unavailable and its error sits beside it. On
  Antigravity's page, both can be left below the fold, because the page is only scrolled by focusing an available
  switch. Verified: Confirmed by source. `finish_handover` only focuses (and so reveals) an available switch
  (crates/gui/farhelm-gui/src/settings.rs:1087-1100, :1112-1115), and nothing else scrolls on a failed read
  (:1074-1083). The code is unchanged since 470e9e6c. SPEC.md:2567-2568 promises focus only "when it is available" and
  says nothing about revealing a failure. Not filtered: the scroll filter needs unusually large content or a small
  window, and here the page is ordinary.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard. Rare read failure, the error is reachable by scrolling, no spec promise. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## antigravity-trust-dismissal-lost-during-recovery.md

- Outcome: `other`.
- Assessment: The user hides an agent's trust hint with ×. If the session's terminal then becomes briefly unavailable
  and is re-adopted while the same question is still on screen, the hint can come back. Verified: confirmed by source,
  assuming the unavailable row reaches the GUI. The trust flag is stripped from Unknown rows
  (`service/status.rs:339-348`, `:417-420`), and the band retires the dismissal on any row without the flag
  (`trust_band.rs:211-214`). This affects every harness with a trust hint. The consequence is that the user dismisses an
  informational hint a second time.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard. It is rare and costs one extra ×. Size S-M if fixed.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## feedback-external-link-cue-is-visual-only.md

- Outcome: `other`.
- Assessment: In the Send feedback dialog, the Chat on Discord link shows a ↗ arrow meaning "opens in your browser", but
  screen readers hear only "Chat on Discord", with no mention of the browser. Verified: Confirmed.
  `.accessible_name("Chat on Discord")` with the arrow only in the painted label
  (crates/gui/farhelm-gui/src/feedback.rs:588-598). SPEC.md:5076-5077 requires only that the link opens the invite and
  keeps the draft. Nothing in the spec asks for an accessible browser cue. Speech output not checked.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard. No spec requirement, and the link's behavior is correct. Size XS if fixed (one accessible
  description).
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

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

## recipe-tunnel-edit-marks-only-card.md

- Outcome: `other`.
- Assessment: In the recipe editor, editing a tunnel's port or host marks only the "Tunnels" heading as edited, not the
  field that changed. Every other field in the editor gets its own dot. Verified: Confirmed.
  `settings/recipes/editor.rs:1091-1157` draws the tunnel inputs without an edited flag. The only marker is the
  heading's `caption_with_dot` (`:1227-1231`). Other fields use `edited(...)` (`:849`, `:921`, `:999`, …). SPEC.md
  (about line 2569): "A dot marks each edited field and section." The tunnel inputs have no caption, so where the dot
  goes and how an added row counts are design choices. That keeps it from being clearly settled.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — per-field markers, compared by row key. Size S-M. Discard if polish is not wanted now.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

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

## podman-launch-lifetime-notice-hidden.md

- Outcome: `other`.
- Assessment: When the user starts a session in a new container, the launcher says only in the Host picker's help that
  deleting the last session removes the container and its files. A new sandbox gets a visible note. Deletion itself
  still asks and names the file loss. Verified: Confirmed. `environment_ui.rs:305-312` returns `None` for Podman, so
  `launcher/view.rs:2606-2617` draws nothing. The text is only in `launch_help` (`:288-302`). SPEC.md requires the
  visible note only for sandboxes (about line 1640). "Container sessions" says containers follow the sandbox Delete and
  visibility contracts but does not name the launch note. The `None` came in 30441792a with a comment that ties the
  notice to billing facts. No unwarned deletion is established.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — a short Podman `launch_notice`. Size S. Or discard.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## launcher-recent-highlight-mutes-yolo-cue.md

- Outcome: `other`.
- Assessment: On a highlighted recent setup, which is exactly the row Enter is about to launch, the YOLO chip loses its
  red danger colour and takes the ordinary selected-row colour. Only the word "YOLO" remains as the cue. Verified:
  confirmed. `search/start.rs:503-507` uses `ink` instead of `palette.danger` when the row is selected, and `row_inks`
  (`start.rs:780-787`) records the deliberate inverse-ink choice. SPEC.md "Creation" 1204 says "permission danger stays
  red". The fix needs a contrast-safe danger treatment on the selection fill, which is a design choice and would
  overturn the commented one.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code with a red-on-selected chip treatment checked for contrast. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-recent-accepts-modified-return.md

- Outcome: `other`.
- Assessment: In the command field, Shift+Return and Cmd+Return (Ctrl+Return on Linux) do exactly what Return does,
  including taking the highlighted recent setup. The previous launcher accepted only plain Return there. Verified:
  confirmed. `components/text_input.rs:428` maps every `PressEnter` to `Submitted { held }` without modifiers. No spec
  rule requires plain-only Return in the editor. Harmless: the action is the one plain Return takes.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard. There is no harm, and nothing in the spec contradicts it. Size n/a.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-highlights-escape-notation.md

- Outcome: `other`.
- Assessment: When a saved folder contains a hidden character, the launcher shows it in escape notation. Search
  underlining can mark letters inside that notation even though the match came from elsewhere in the real path. Display
  only: the action uses the raw folder. Verified: confirmed. `host_text.rs:525-534` searches the rendered, escaped text
  again. Cosmetic.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard. The fix (projecting raw match ranges through escaping) costs more than the cue is worth. Size
  M if fixed.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-undo-leaves-template-source-mark.md

- Outcome: `other`.
- Assessment: When a template applied in the launcher switches the agent to a riskier permission, a banner offers Undo.
  Undo puts back the earlier settings, but the form keeps naming the template as their source, and its row stays marked
  as the one that filled the form. Verified: confirmed. `apply_now` sets the source marker
  (`launcher/view.rs:1089-1091`). `undo_escalation` (`launcher/view/escalation.rs:167-179`) restores the draft and
  notice but not the marker, and the marker is not captured when the banner is built (:73-100). Only Reset
  (`view.rs:1478`) and fills (`recents.rs:131`) change it. On the spec: SPEC.md:1499-1500 says "The row that filled the
  form remains marked, and the form names its source until Reset choices or another fill." Read literally, the marker
  should stay across Undo, which is current behavior. Restoring the old marker rests on reading Undo as a fill, or on
  the marker having to name the source of the current values. It is a display problem only.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — capture the marker in the banner and restore it on Undo; size S. A discard is defensible
  because the literal spec text allows the current behavior.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-template-rows-rebuild-draft.md

- Outcome: `other`.
- Assessment: In Replace with and Restart with, which keep the host fixed, the wide launcher redraws every template row
  by copying the host list and rebuilding a full snapshot of the form, to check whether that template would change a
  held field. It is wasted work, but nobody has measured any delay. Verified: confirmed as redundant work, with no
  measured latency. - Per template row, `row_disabled` (`launcher/view/search.rs:429-438`) calls `self.hosts(cx)` and
  `held_row_refusal` (`templates/dispatch.rs:259-270`), which rebuilds `apply::snapshot` and clones the template inside
  `held_refusal` (`templates/apply.rs:423`). - The design scale is "tens of sessions across a few hosts"
  (SPEC.md:2829-2830), and template catalogs are small. - No user-visible effect is shown.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard — no measured cost at the stated scale; size none (fix would be S-M).
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## quick-launch-loses-template-source-mark.md

- Outcome: `other`.
- Assessment: When a ⚡ quick launch or a number-key launch cannot start on its own, New opens with the template applied
  and a banner, but the form does not name the template as its source and its row is not marked. When the same shortcut
  is pressed inside an open launcher, the source is marked. Verified: confirmed. `on_the_page`
  (`templates/dispatch.rs:407-443`) receives the applied template's name in `Applied::template`
  (`templates/apply.rs:179-180`) but drops it. `Prepared` (`launcher/view.rs:304-312`) has nowhere to carry it, while
  the in-launcher path sets it (`view.rs:1089-1091`). Whether the spec demands it rests on reading SPEC.md:1499-1500
  ("The row that filled the form remains marked, and the form names its source"). That sentence is in the list-column
  paragraph, and a shortcut fill is not a row press. This is a display gap only.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — carry the marker through `Prepared::Show`/`Launch` for consistency with the in-launcher
  path; size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-recent-hover-can-follow-old-row.md

- Outcome: `other`.
- Assessment: Recent-setup rows are identified by list position. When the list changes under a resting pointer (a
  history reply arrives, or the draft's filter changes), a pending or visible tooltip can keep describing the setup that
  used to sit there. Verified: confirmed. `search/start.rs:282` uses `launcher-recent-{index}` as the id, and tooltip
  retention is keyed by owner id. Whether this is rare is unclear: the list re-filters when the draft changes, so a
  filter match was not claimed. Display only, and it corrects when the pointer leaves.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard, or fix code with content-derived row ids. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## hook-hint-dismissal-survives-next-launch.md

- Outcome: `other`.
- Assessment: Grok and Antigravity sessions launched without hook setup show a "set up Restart" hint above the terminal.
  Hiding it with × also hides it for a later run of the same session, for example after Restart with. Verified:
  confirmed. The dismissal is keyed by session and agent and kept while the row stays eligible
  (`session_pane/hook_band.rs:36-38`, `:59-75`, `:133-136`). SPEC.md (around 3186-3188) says × "hides this occurrence"
  but does not define an occurrence for this hint. The button's own tooltip says "Hide this hint for this session"
  (`hook_band.rs:168`), so the code matches its own wording.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard, or fix spec to define the hook hint's occurrence. The hint is a discovery aid and the tooltip
  already says "this session". Size S.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## hook-hint-can-repaint-old-dismissal-error.md

- Outcome: `other`.
- Assessment: The Grok/Antigravity session hint above the terminal offers "Don't show again". If that save is refused,
  the hint shows a red "Could not save hiding the hook setup prompt and hint. Try again." Suppose the user then turns
  the offer off and back on in Settings. The hint comes back with the old failure line still under it, even though
  nothing failed since. The line stays until another Don't show again is pressed, even across switching sessions.
  Verified: confirmed. `crates/gui/farhelm-gui/src/session_pane/hook_band.rs:88` is the only place the error clears, at
  the start of the next dismiss. It is set at :93-99 (refusal) and :106 (busy). `draw` hides the band while the shared
  dismissal is on (:133-137). The error is kept by session key alone and redrawn when the band returns (:180-191).
  `follow` (:60-76) never clears it, unlike the trust band, which drops its error when another session opens
  (`trust_band.rs:222-229`). SPEC.md "Status" (hook hint paragraph, ~:3185-3190) says nothing about the error's
  lifetime. Not filtered: a refused save followed by a Settings toggle is not one of the rare-trigger kinds the filter
  lists.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Clear the error when the dismissal preference changes or the session or run changes,
  mirroring the trust band. Size S.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## held-escape-discards-launcher-draft.md

- Outcome: `other`.
- Assessment: When the empty pane has a highlighted recent setup or template, the first Escape clears the highlight. If
  Escape is held, the auto-repeat then closes the launcher and discards the draft, on platforms that repeat Escape
  (Wayland and macOS). Verified: confirmed from source. `launcher/view.rs:239-246` returns Stay for the first dismissal.
  `layers.rs:1546-1556` notes a closing key only when a layer closes. `Dismiss` is not in `ONCE_PER_PRESS`
  (`actions.rs:698-720`). SPEC_impl.md 9351 requires the first Escape to clear the highlight. SPEC.md 3829's held-Escape
  rule covers only the case after the last layer closes. Losing the draft is recreatable form input, not `highest`.
  Native repeat was not exercised.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Note the dismissal so its repeats are swallowed. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## hook-diagnostic-path-can-render-doc-link.md

- Outcome: `other`.
- Assessment: The session notifications bell makes Farhelm docs addresses clickable. A hook setup error that quotes a
  file path containing `https://farhelm.io/docs/…` would show part of that path as a docs link. The link can only go to
  Farhelm's docs. Verified: Confirmed as described. The bell searches the whole text for `DOCS_LINK_PREFIX`
  (crates/gui/farhelm-gui/src/notices_bell.rs:97, :778-789). SPEC.md:3268-3271 makes any address starting with that
  prefix clickable, whoever wrote it. The trigger (such a path) is contrived, and the only harm is a misleading but
  harmless docs link. Arguably a filter match, but the filter's "trigger is rare" list doesn't fit cleanly, so I left it
  here.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard. The destination is restricted to Farhelm's docs, as the spec allows. Size S if fixed.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## launcher-result-pick-leaves-approval-focus.md

- Outcome: `other`.
- Assessment: While an agent approval card beside the launcher has keyboard focus, clicking a recent setup or template
  in the launcher applies it, but focus stays on the approval card instead of returning to the command field. Verified:
  confirmed. Rows call `focus::focus_in_layer` (`search/start.rs:136`, `search.rs:362-368`), which refuses the move when
  focus is not inside the top layer (`focus.rs:651-666`). SPEC.md "Creation" 1505 says "A mouse pick returns focus to
  the command field." The fix needs a choice of the right pointer-focus route, which SPEC_impl's in-layer rule
  deliberately limits.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code with the pointer-focus route and its visibility checks. Size S.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-post-exit-alt-release-clears-short-selection-and-queues.md

- Outcome: `other`.
- Assessment: After a program exits, an Alt-click (Option-click) of one character on the final screen still runs the
  "move the cursor here" gesture. It sends arrow keys toward the ended program, which clears the one-character highlight
  that was just copied. The clipboard keeps the copy, and the keys are normally discarded by the closed terminal.
  Verified: Confirmed by source. `end_selection` and `alt_click`
  (`crates/gui/farhelm-gui/src/terminal/attached.rs:4294-4340`) have no gate on program exit. Since 0592c5c6 this also
  happens on frames whose program had mouse reporting on. The effect on the program is unverified: keys go to the PTY
  only if something still reads it.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code (skip the Alt cursor move once the program has exited) or discard. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-multi-clicking-exit-message-copies-different-row.md

- Outcome: `other`.
- Assessment: After a program exits, the terminal draws an exit message below the grid. Double- or triple-clicking that
  message selects a word or the whole row from the last terminal line above it and copies it, replacing the clipboard
  with text the user did not point at. Verified: Confirmed by source. Multi-click goes through `selection_position`,
  which clamps rows below the grid to the last row
  (`crates/gui/farhelm-gui/src/terminal/attached.rs:4004-4035, 4112-4131`), and the release copies it. Since 0592c5c6
  this also applies to frames whose program had mouse reporting on. With reporting off it predates that commit. Losing
  clipboard contents is not "user-owned work" under "Priority buckets". No native gesture was observed.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code (start multi-click selection only on drawn grid rows) or discard. Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-page-keys-skip-rows-in-lifted-terminal.md

- Outcome: `other`.
- Assessment: Shift+PageUp and Shift+PageDown scroll terminal history by a page less one line, so one line stays on
  screen as a landmark. When the exit or status line lifts the grid by one row, that overlap is lost. With two lifted
  rows, paging skips lines that never come into view. Verified: Confirmed. `scroll_page` uses `rows.len() - 1` without
  the lift (`crates/gui/farhelm-gui/src/terminal/attached.rs:1629-1640`). The skipped lines are still reachable by wheel
  or scrollbar.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code (page by the visible row count minus one). Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-recovery-warning-is-cut-off-in-supported-narrow-terminal.md

- Outcome: `other`.
- Assessment: When recovery has lost part of a terminal's history, the terminal shows "Some earlier output could not be
  recovered" above the history. In the narrowest supported window, this one-line notice is wider than the pane and is
  cut off, and nothing reveals the rest. Verified: Partly verified. The notice is painted as one unwrapped line
  (`crates/gui/farhelm-gui/src/terminal.rs:312-314` and the notice painter). The width arithmetic (about 353 px at 14 pt
  against a 320-324 px main pane at the minimum window) is the reviewer's and was not re-measured. The painter predates
  the reviewed change; that change only makes the notice reachable on exited full-screen frames. Ordinary terminals with
  trimmed history are affected the same way. The "Dialog explanations out of view" filter does not apply: this is not a
  dialog, and the clipped text cannot be reached at all.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard (only at the minimum window width, and the notice's meaning survives truncation), or fix code
  (wrap or shorten). Size S-M.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-lifted-viewport-makes-scrollbar-overstate-visible-share.md

- Outcome: `other`.
- Assessment: When the exit or status line lifts the terminal grid, the scrollbar thumb is drawn a little larger than
  the share of output actually visible. Scrolling still reaches both ends. Verified: Confirmed. `scroll_position`
  reports every grid row as visible while also counting the lift padding in the total
  (`crates/gui/farhelm-gui/src/terminal.rs:531-552`). The error is at most one or two rows' share.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard (cosmetic, endpoints correct). Size S if fixed.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-late-replay-history-displace-already-browsed-final-screen.md

- Outcome: `other`.
- Assessment: A replay that stalls can show the exit frame before the scrollback arrives. If the user scrolls or selects
  in that window and the scrollback then arrives, the view can jump into old history, and an existing selection can end
  up naming different rows, without any scroll request. Verified: Confirmed by static reading only (the reviewer's model
  trace). Nobody has observed a stall. It needs the idle-reveal path, user navigation during it, and a late first
  history chunk. No data is lost. A later Cmd+C could copy different text. It does not fit the self-correcting filter:
  the displaced view stays until the user scrolls.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard (rare, compound trigger, no loss). Size M if fixed (re-base provisional anchors).
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-trust-hint-save-errors-use-live-selection-instead-of.md

- Outcome: `other`.
- Assessment: A trust hint's "Don't show again" files any save error under whatever session is selected when the click
  is handled, not the session the hint was drawn on. Separately, the error is kept per session but not per agent. After
  a failed save, if a Replace switches the same session to another agent that also asks about trust, the old "Could not
  save hiding this trust hint" line shows under the new agent's hint. Verified: confirmed by source.
  `session_pane/trust_band.rs:273` reads the live selection, and :288-304 attributes the error to it. The drawn key is
  available to the click handler but not passed (:465-475). `Known::failed` is keyed by session only (:128) and cleared
  only when the session changes (:222-229). The live-selection part (and the silent no-op when nothing is selected,
  :273-275) needs the selection to change between a frame and the click. That matches "Races a person would have to win
  inside a sub-second window": the dismissal is a shared, per-agent preference saved with the drawn agent, and the stray
  error drops when the user leaves the session. The Replace-to-another-agent part is not a timing race and remains. The
  reviewer's point (2), `self.read` using the live target, is harmless, because the read is for the band currently
  shown.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Bind the drawn key into the click handler and key the error by session and agent. Size S.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## terminal-selection-paints-wide-wrap-padding.md

- Outcome: `other`.
- Assessment: A wide character (a CJK glyph, say) that does not fit at the end of a row wraps to the next row and leaves
  an unused cell. A selection across it paints that unused cell as selected, although copying correctly omits it.
  Verified: Confirmed. The painted range comes from `selection.columns(line, cols)` through the row width
  (`crates/gui/farhelm-gui/src/terminal.rs:838-850, 1251-1265`). SPEC_impl.md "Binary rows": "Copy, selection and link
  detection omit that column". Whether "selection" there covers the painted highlight, or only selection text, is a
  reading. SPEC.md's 2026-11-21 accepted wide-character list ("only ...") does not include this.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code (clip the painted highlight at a padded row's last column). SPEC_impl's sentence supports it.
  Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

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

## trailing-podman-suspend-flashes-connecting-and-runs-retry-ladder.md

- Outcome: `other`.
- Assessment: After the user suspends a container session, its host briefly shows "Connecting" between "Suspending…" and
  "Suspended". For sandboxes and containers alike, if the quick check after Suspend hits a transient failure, Farhelm
  runs its full one-minute reconnect sequence, showing Connecting, instead of one quiet check. Verified: Partly
  confirmed. `Podman::change` passes `|_| async { Ok(None) }` as `record` (`podman.rs:824`), so `perform`
  (`lifecycle.rs:109-121`) publishes no Suspended state before `release_hold(host, false)`. The suppression in
  `manager.rs:4586-4594` applies only when the state is already `Suspended { suspended_by_farhelm_at: Some(_) }`. The
  `active` and ladder behavior was not traced end to end. SPEC.md (about line 393) says the host shows Suspending… until
  suspension is confirmed, then Suspended. It does not mention a flash in between.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — have Podman's `record` publish Suspended, and make a non-fresh release run one quiet check.
  Size S-M.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-trust-band-keeps-undismissable-error-after-dismissal.md

- Outcome: `other`.
- Assessment: After a failed "Don't show again" on a trust hint, suppose the user turns the hint off in Settings, or
  another client stores the dismissal. The hint goes away, but its red "Could not save hiding this trust hint" line
  stays above the terminal with no close button. It goes away only when the user leaves the session or the question goes
  away. Verified: confirmed by source. `Facts::shows_failure` (`session_pane/trust_band.rs:101-103`) ignores
  `dismissed`. `draw` draws the error even when the explanation and its buttons are hidden (:358-379). `hide_settled`
  (:313-321) never clears an earlier error after a later success. This port of the old app's `error_visible` is
  commented as intended and pinned by a test. Path 2 (a second click that hits "busy" before the redraw, followed by the
  first click's success) is a sub-second window whose stray line clears on leaving the session, so it fits "Races a
  person would have to win inside a sub-second window". Path 1 (refusal, then dismissal through Settings) is not a race
  and remains. SPEC.md (~:1848-1850): "A failed Don't show again save restores the band with an adjacent error while
  that session is still shown; leaving the session drops the error". It does not settle what happens once the hint is
  dismissed by other means.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Gate the failure line on not being dismissed and clear it after a successful save,
  overturning the commented port choice. Size S.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## trailing-overflowing-banner-stack-resets-to-oldest-after-reconnect.md

- Outcome: `other`.
- Assessment: When enough notices are open that the banner stack scrolls, a reconnect or Take back whose newest notice
  was already present before the disconnect scrolls the stack back to the oldest notice. The newest one (such as
  "connection lost") ends up hidden below. Verified: Confirmed. `main_view.rs:204-207` keeps `drawn_banners` across the
  inert branch (`:261`), while the scroll state is rebuilt. It is not filtered: a reconnect is not a rare trigger in the
  filter's sense, and the display does not correct itself.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Clear `drawn_banners` on the inert branch. Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-shifted-wide-wrap-spacer-drops-background.md

- Outcome: `other`.
- Assessment: A program deletes characters (CSI P) on a colored row that ended in a wrapped wide character. The shifted
  padding cell then loses its background, which leaves a one-column uncolored gap until the program redraws the row.
  Verified: Confirmed by source. The main loop skips `LEADING_WIDE_CHAR_SPACER` at any column, while the padding paint
  is emitted only for the last column (`crates/farhelm-supervisor/src/grid_output.rs:154, 163-169, 225-238`). This is a
  rare edit sequence. It is not in SPEC.md's 2026-11-21 "only" list, so not covered. Not filterable: the gap persists
  until the program redraws the row.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code (skip the leading spacer only at the padded last column, as the reviewer suggests). Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-right-half-wide-erase-keeps-inverse-decoration-and-link.md

- Outcome: `other`.
- Assessment: When a program erases just the right half of a wide character that is reverse-video, underlined, struck
  through or part of a hyperlink, Farhelm keeps drawing the old inverted background or the underline across the erased
  column. That column also stays clickable as part of the hyperlink. Verified: Confirmed. `wide_glyph_owns_next_cell`
  compares only the background and ignores the wide cell's inverse, decoration and link
  (crates/farhelm-supervisor/src/grid_output.rs:249-257). The code is unchanged since a4c7c4d5. For a hyperlink, the
  hover display is still required before a click opens anything. The effect is one cell's rendering and hit area.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Refuse the absorption when the wide cell is inverted, decorated or linked, and add the three
  unit tests. Size S–M.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-deleting-wide-spacer-swallows-shifted-space.md

- Outcome: `other`.
- Assessment: A program deletes just the right half of a wide character, and a following space with the same background
  shifts into that cell. Farhelm then treats the space as part of the wide character, so copying and link detection lose
  the space ("a界Z" instead of "a界 Z"). Verified: Confirmed by source. `wide_glyph_owns_next_cell` absorbs a plain
  same-background space (`crates/farhelm-supervisor/src/grid_output.rs:246-257`). As the reviewer notes, the emulator's
  cells cannot tell a shifted space from an erased right half, so a fix trades against #4598's behavior (keeping the
  glyph full-width after a same-background half-erase) or needs a vendor patch. That is a design choice, so it is not
  clearly settled.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix spec (add this DCH-on-wide-spacer case to the accepted wide-character edge cases). Size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-left-half-wide-erase-drops-orphaned-background.md

- Outcome: `other`.
- Assessment: When a terminal program erases or deletes only the left half of a wide character (CJK, emoji) that has a
  colored background, the leftover right cell is drawn in the default background instead of its color. The wrong cell
  stays wrong until the program redraws it. Verified: confirmed. The vendored `erase_chars`
  (`vendor/alacritty_terminal/src/term/mod.rs:2002-2017`) replaces only the erased cells and leaves the orphaned spacer
  with its old background. `grid_output.rs:163-169` skips every `WIDE_CHAR_SPACER` cell without emitting styled paint,
  while paired spacers are consumed earlier by `col += 2` (`:204-221`). No change since `a4c7c4d5`. The sibling
  `trailing-shifted-wide-wrap-spacer-drops-background.md` is already queued for the same branch.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code together with the shifted-spacer sibling, emitting a styled one-column blank for an orphaned
  spacer; size S.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-resolve-remote-status-clears-local-transport-signal.md

- Outcome: `other`.
- Assessment: When Farhelm runs a command on a host over ssh or podman, the host's exit status comes back on a separate
  channel. If the local ssh or podman process is killed by a signal just after that status arrives but before all output
  has, Farhelm reports success and uses the truncated output (for example a cut-off home path). Verified: Confirmed in
  code: `provisioning/backend.rs:1763-1790` keeps transport failure only for ssh exit 255 and otherwise takes the exit
  code from the status record, discarding `signal`. The trigger needs an outside signal (not the helm's own deadline)
  inside a narrow window; nobody has observed it. The consequence would include reporting success for a command that
  failed, which keeps it out of the filters.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Keep a signal termination as a failure in `resolve_remote_status` and in `probe`. Size S.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-container-host-unanswered-embeds-raw-stderr.md

- Outcome: `other`.
- Assessment: When a container host cannot be reached, the error Farhelm shows on the container's host (and in Delete
  and launch errors) includes the raw SSH error output, which can be up to 64 KiB, and nothing strips control characters
  from it. Every other container error trims and cleans that output first. Verified: Confirmed.
  `container_host_unanswered` (`podman_probe.rs:56-65`) uses `result.stderr.trim()` as it is. Siblings use
  `tensorlake::shell::shown`. The GUI escapes host text (SPEC_impl.md "Host-supplied text"), so no display spoofing in
  the app is established. Helm logs and HTTP error bodies get the raw text. This trigger is common (a host that is
  asleep), so the rare-glitch filter does not apply.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — pass the output through `shell::shown`. A one-line change plus a test, size S.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

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

## trailing-custom-agent-model-effort-error-duplicate-element-id.md

- Outcome: `other`.
- Assessment: If both the Model and the Effort of a custom agent in the launcher are invalid at once, their two error
  lines share one element ID. With assistive technology active this is a debug-build panic, and in other builds an
  accessibility identity clash. Verified: Confirmed. Both use `status_line("launcher-custom-value-error", …)` inside
  anonymous wrappers (`crates/gui/farhelm-gui/src/launcher/view/choices.rs:243-300`). SPEC_impl.md "GPUI cookbook:
  traps" documents the hazard ("Duplicate ids are a debug panic only while assistive technology is active") and gives
  the rule "Rule: `(component, domain id)` ids" for the index case. It does not require unique IDs in so many words, so
  this is not classed clearly settled. Not run under assistive technology.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Use distinct model and effort error IDs. Size S.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-hidden-trash-button-retains-animation-handles.md

- Outcome: `other`.
- Assessment: If the trash button's archive wiggle or "freeing space" bob is running when the button is hidden (opening
  Hosts, marking sessions), its animation timer keeps ticking 10 times a second while the button is hidden. If Reduce
  Motion is then turned on and off while it is still hidden, an interrupted wiggle can resume late when the button comes
  back. Verified: confirmed. `TrashButton::render` (`crates/gui/farhelm-gui/src/trash.rs:764-785`) is the only place
  that drops `bob` and `wiggle`, and it does not run while the button is hidden. Only a docs commit (`6f1ea749a`)
  touched it since the reviewed commit. The late-wiggle part needs a Reduce Motion toggle inside a hidden wiggle of
  under a second, which is rare and cosmetic. The timer part is common, but its cost is not established: whether
  notifying the hidden entity causes window repaints was not verified.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard — the cost is not shown to matter and the late wiggle is very rare; size S if kept.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-light-theme-primary-and-danger-buttons-identical-hover-held.md

- Outcome: `other`.
- Assessment: In Light appearance, pressing and holding a primary (blue) or destructive (red) button looks the same as
  hovering it, so nothing shows the press. Verified: Confirmed. `theme.rs:364-365` and `:377,398` give Light's hover and
  held fills the same colors, and Dark's differ. The test `components/tests/button.rs:498` states that hover and held
  must differ but asserts it only for ordinary buttons. No SPEC rule covers pressed tints.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Two Light tokens and a per-tier assertion. Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

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

## trailing-repo-cache-sweep-single-error-aborts-all-remaining-repos.md

- Outcome: `other`.
- Assessment: At supervisor startup, Farhelm cleans up old repository caches and prepared checkouts. If removing one
  repository's old cache generation fails (for example because a leftover git process still writes in it), the sweep
  stops and skips every remaining repository until the next restart. Only disk space is affected; sessions still start.
  Verified: Confirmed. `crates/farhelm-supervisor/src/repo_cache.rs:218,283` propagate with `?`. SPEC_impl.md "Owned
  checkout admission and lifetime" (3744) says "A sweep failure is logged and leaves unused caches for a later startup.
  Sessions can still start." That accepts leftover caches after a failure but does not say one repository's failure
  should skip the others or their spare cleanup, so it is not explicit coverage.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Log and continue for each path and each repository, as the spare sweep already does. Size S.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

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

## trailing-ticker-leaks-dead-subscribers-under-reduced-motion.md

- Outcome: `other`.
- Assessment: With the operating system's Reduce Motion kept on, the app's animation registry gains a dead entry each
  time a cursor blink, pulse or spinner starts and stops. Nothing removes them until Reduce Motion is turned off, so
  memory grows slowly over a long session. Verified: Confirmed. `crates/gui/farhelm-gui/src/ticker.rs:421-437` pushes
  entries; pruning happens only in `tick` and in `motion_changed` when motion returns (`:441-455`, whose comment says so
  on purpose). Each entry is a few dozen bytes. SPEC_impl.md's line "accumulation without bound over time during
  ordinary operation" (Supervisor metadata retention; Leftover files) is written for the helm and for files, not GUI
  memory.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code. Prune dead entries in `subscribe`, which is a one-line `retain`. Size S.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

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

## launcher-recent-fill-option-named-replace.md

- Outcome: `fix code`.
- Assessment: In Replace with, each recent-setup row in the empty pane is announced as "Replace: ...", but clicking or
  activating the row only fills the form. Only Enter from the command field on the highlighted row replaces. Verified:
  confirmed. `search/start.rs:289-293` names every recent row "Replace: ...", while the click handler passes
  `keyboard=false` (`start.rs:334-337`). The cue overstates rather than hides a destructive action.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code. Put "Replace" only where activation replaces, for example on the highlighted
  row's Enter cap. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-typed-recent-mislabels-enter.md

- Outcome: `fix code`.
- Assessment: In a wide Replace with launcher, a highlighted recent setup in typed search results shows the "⏎ Replace"
  key cap, but Enter on a typed result only fills the form. In other modes it shows a bare ⏎, also implying a launch.
  Verified: confirmed. `typed_choice` (`search/start.rs:560-571`) passes `recent_enter_cap(mode)`, while typed Enter
  only calls `take_recent` (`search.rs:590-594`). SPEC.md "Creation" 1455-1456: "A query result for a recent setup fills
  the form without launching."
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code with a fill-only cap for typed rows. The wording is the only choice. Size S.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## exited-shell-suspend-confirmation-wording.md

- Outcome: `fix code`.
- Assessment: Stopping a sandbox or container session whose shell has already exited asks about closing its remaining
  tabs. The question's extra answer "Stop and suspend" still says in its hover that it stops the shell, and the
  question's appended note speaks of "the stopped shell". Verified: confirmed by source. The ended-session wording
  switch applies only to Confirm stop and Cancel (`session_list/stop.rs:127-138`). `stop_and_suspend_answer` always says
  "Stop this session's {program}" (`session_list/environment.rs:582-591`), and `stop_note` always says "The stopped
  {program}" (`environment.rs:572-578`). SPEC.md:2146-2147 requires the question to name the remaining tabs when the
  main program has exited; whether that extends to the extra answer's hover is a reading. Wording only; the request's
  guard is correct.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code (ended-aware help and note). Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## uninstall-omits-persistent-hook-cleanup.md

- Outcome: `fix code`.
- Assessment: Once Farhelm has set up Grok or Antigravity hooks, they stay in the agents' own configuration on every
  machine where they were written, and they call the installed `farhelm` command. Uninstalling Farhelm removes that
  command, but the uninstall guide never mentions the leftover hook files or how to remove them. Grok and Antigravity
  then keep calling a missing program in every session, inside Farhelm or not. Verified: confirmed for the docs.
  `website/src/content/docs/docs/get-started/uninstall.md` lists only state, the app and the command, and says "Your
  projects, your agents … are not touched". The agent pages do give removal steps
  (`website/src/content/docs/docs/agents/grok.md:24`, `antigravity.md:38-40`). Leaving vendor configuration alone is by
  spec: SPEC.md "Uninstall scope and interaction" (~:966-968) says "agent harness installations and their own data … are
  untouched". What the vendors do when the callback's program is missing (an error, a hang, nothing) is unverified.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code (docs only). Add a paragraph with links to the uninstall page. Size S. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## pi-page-describes-wrong-trust-question.md

- Outcome: `fix code`.
- Assessment: The heading of Pi's Settings page says it is about "Answering the agent's folder-trust questions". Pi's
  switch actually approves project settings and extensions for each launch, and Farhelm recognizes no trust question
  from Pi. Verified: confirmed. `settings/frame.rs:126-134` gives every trust-capable agent without hooks the
  folder-trust description, Pi included. Pi's own switch help says otherwise (`settings/agent_trust.rs:182,219-221`).
  SPEC.md (~~:1871-1873) says "Pi has a switch but no hint band: its project-resource approval is not currently
  recognized as a trust question". It also still calls it a "trust switch" (~~:1852). Fixing this means choosing new
  copy, which is a small wording decision.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code. Give Pi its own description, for example "Approving project settings and
  extensions". Size XS. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## trailing-agent-create-host-help-omits-supported-hostless-launches.md

- Outcome: `fix code`.
- Assessment: `farhelm agent create --help` says `--host` is required unless a template sets the host or `--recipe` is
  given. It leaves out two other cases where no host is needed: a template that launches a recipe, and an agent already
  running inside a Farhelm sandbox or container. Verified: Confirmed. `crates/farhelm/src/main.rs:364-366` carries that
  text. SPEC.md 4723-4724 and the confined- create rules allow the omitted cases. The cost is help wording only; the
  commands themselves work.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code. A one-line help text change. Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## launcher-new-environment-folder-omits-repository-prompt.md

- Outcome: `fix code`.
- Assessment: When launching into a new sandbox or container with Managed checkout but no repository chosen yet, the
  Folder line says the repository will be cloned inside the new environment, and does not ask the user to select a
  repository. Verified: confirmed. `checkout_folder_detail` tests `draft.target.is_some()` before
  `checkout.repo.is_none()` (`launcher/view/checkout.rs:1254-1266`). Launch is still refused with the select-repository
  reason (`request.rs:405-407`), and the summary says "(select a repository)" (`checkout.rs:1314-1318`), so nothing
  wrong is launched.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code (swap the two branches). Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-claude-trust-hint-shows-on-command-and-legacy-ordinary-dirs.md

- Outcome: `fix spec+code`.
- Assessment: When Claude asks "trust this folder?" in a session started from the Command tab (or an old legacy session)
  in an ordinary folder, Farhelm still shows the hint saying it can answer this for you in Settings. Turning that switch
  on does nothing for such launches. Farhelm only applies Claude trust for structured Claude launches, or after cloning
  a new managed checkout. Verified: confirmed. `settings/agent_trust.rs:299-302` names Claude whenever
  `claude_trust_prompt` is set. Its docstring (:293-298) says this is deliberate ("whatever the launch, as the band has
  read it since it was built"). The supervisor applies trust only for a structured selection with trust on
  (`service/core.rs:14776-14779`). SPEC.md (~~:1842-1844) shows the band whenever Claude's question is visible, the
  setting is off and the hint is not dismissed, with no launch-kind condition. SPEC.md (~~:1820-1821) lists which
  launches read the setting without settling Command launches. So the code follows the spec's words, and the open
  question is whether the spec should change.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix spec+code. Show Claude's hint only for structured Claude launches or fresh managed
  checkouts, as Codex, Muse and Antigravity already do, and update the deliberate docstring and the test at
  `session_pane/trust_band_tests.rs` (`only_a_sessions_own_launch_names_the_agent_asking`). Size S. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## trailing-replace-with-existing-env-checkout-promises-trash.md

- Outcome: `fix code`.
- Assessment: In Replace with… on a session inside an existing sandbox or container, choosing Managed checkout shows the
  lifetime note "it moves to the trash when its last session is deleted". Inside a sandbox or container that is not what
  happens: deleting the session discards the checkout, or deletes the whole environment if it was the last session. The
  Delete confirmation itself does state the real outcome. Verified: confirmed. `checkout_lifetime_note`
  (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1300-1307`) chooses the environment wording only when
  `draft.target.is_some()`, which is a new environment. SPEC.md "Managed checkouts" (`SPEC.md:1787-1788`) requires that
  selecting managed checkout "explains the directory's lifetime", and "Sandbox sessions" (`SPEC.md:1698-1702`, `1744`)
  says the checkout is discarded, not moved to the trash. Note: `SPEC.md:1783`'s row-tooltip sentence states the trash
  lifetime generally, so the spec wording itself may need a sandbox caveat. That is why this is not clearly settled.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code — pick the wording by the chosen host's being session-owned too; size S. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-tailscale-setup-probe-exit-255-blames-socket-length.md

- Outcome: `fix code`.
- Assessment: When an SSH connection to a Tailscale host fails during setup or update checks, on a Mac with a long user
  name or a long custom state folder, Farhelm can blame the state folder's path length and say it cannot ask sign-in
  questions. Neither is true for Tailscale hosts. Verified: Confirmed. `provisioning/backend.rs:2882-2894` gates on
  `access.asks_user()`, which includes Tailscale (`ssh.rs:219-224`). `transport.rs:294-305` gates the same check on
  `UserConfig` only, and Tailscale sockets live under the short `/tmp/fhs-…` directory. The trigger (exit 255 on an
  unreachable peer) is not rare in FILTER.md's sense, so the diagnostic filter does not apply.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code XS (narrow the gate to UserConfig, as in transport.rs, plus a unit test). Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-tailscale-destination-format-guidance-discarded.md

- Outcome: `fix code`.
- Assessment: Turning on Tailscale host for an SSH host whose address is an `ssh://` URI, has a port, or uses a config
  alias with underscores fails with "is not a usable ssh destination". It should explain that Tailscale mode needs a
  node name, DNS name or IP with an optional login. The same applies when editing or registering such an address.
  Verified: Confirmed. `ssh/tailscale.rs` `destination_host` produces guidance, but `hosts.rs:998-1003` and the store
  sites wrap it into `HostStoreError::InvalidDestination`. SPEC.md (Tailscale host) promises "an actionable error if
  Tailscale cannot verify the host", which covers verification, not format, so this is a reading. The trigger is
  ordinary.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code S (a dedicated error variant carrying `destination_host`'s message at all
  sites). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-approval-card-yolo-description-wrong-on-restart-and-prompt.md

- Outcome: `fix code`.
- Assessment: Agent approval cards for a plain Restart of a YOLO session, and the currently disabled Prompt cards, say
  "Farhelm applies the host's YOLO rules to this launch". The host's YOLO rule does not apply to plain Restart or to
  Prompt. Verified: confirmed. The string is at `approvals/model.rs:1065`. SPEC.md 4555-4556 ("Plain restart is exempt
  from that YOLO rule, including a restart of a YOLO session") and 4502 ("The YOLO launch rule does not apply because
  this starts no agent"). The fix needs wording per card kind, which is a choice. The Prompt part is dormant.
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code by rewording to describe the bypass, not enforcement, or by varying it per
  action. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal
  or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-gone-terminal-ended-confirmation.md

- Outcome: `fix code`.
- Assessment: A session whose terminal host has died, while the program it ran may still be alive, is shown as ended
  (with the recovery card). Stop on that ended row is sent with "refuse if the agent is running", and so is a Delete
  whose prompt said nothing was alive or only that the agent had ended. If the main program survived its host's death,
  the supervisor lets the guard pass and its cleanup kills that program. The user agreed to stop an ended session, not a
  running program. Verified: Confirmed by source on current main. The terminal runtime's probe returns Gone for a Gone
  host without checking the pinned child (`crates/farhelm-supervisor/src/hosts/terminal.rs:823-837`). The Stop guard
  checks only the probe's live pid (`service/handlers.rs:867-889`), then reaps with the recorded scope or marker sweep
  (`handlers.rs:1120-1132`). The Delete guard treats `Gone` like `Dead` (`service/core.rs:13545-13557`), and teardown
  then roots its sweep at the still-running pinned child (`service/teardown.rs:319-343`). The code itself documents a
  child that outlives its host: "a child that outlived its host (one that ignored the hangup of a host killed outright)"
  (`hosts/terminal.rs:764-770`). SPEC.md "Lifecycle operations", 2026-10-01 decision: "a Stop whose click showed an
  ended agent is refused if the agent is alive again or cannot be confirmed ended, before stopping anything", and a
  Delete "whose prompt said nothing was alive ... is refused if the agent or any terminal tab is running". Nothing on
  main has changed these paths since the reviewed commit (later handler commits touch only custom agents). Unverified:
  that a shipped agent (Claude Code, Codex, a shell) survives its host's hangup in practice; it needs a program that
  ignores SIGHUP, or a custom agent or wrapper that does. No runtime test was run.
- Decision: q-gonehost, "When a terminal host is gone but the program it ran is still alive": the maintainer picked
  "Treat it as alive". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "As long as
  this does not turn horrifically complicated. If it turns very complicated during execution, the plan must turn it into
  a follow-up to-do item."
- Completion criteria: A terminal host that is gone while its main program is still identifiable counts as running for
  confirmations, Stop and Restart cleanup. Recommendation for reference: fix code. Have both guards consult the retained
  pinned identity for a Gone runtime: refuse when the program is still running, and also when the process table cannot
  be read. The spec text already requires the behavior, so this is close to "clearly settled". It is presented because
  the bucket is highest and the fix can be placed in the probe or at each guard. Size M (two guards plus a test that
  needs a surviving child after its host ends; likely over 100 lines with tests). Can share a change with 62. Complexity
  gate (maintainer): if the fix turns very complicated during execution, the executor stops and turns it into a
  follow-up TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about 100 lines
  of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-lifecycle-loses-held-child-after-terminal-loss.md

- Outcome: `fix code`.
- Assessment: When a session's terminal host dies but the program in it survives, a plain Stop or a Restart does not
  target that program directly. On a launch without a systemd scope (macOS, or Linux with no usable user manager),
  cleanup then finds the program only through its environment marker. If the marker cannot be read, Stop reports
  success, or Restart starts a second run in the same folder, while the old program keeps running. Verified: Partly
  correct. The omission is confirmed. Stop and Restart derive a root only from the probe's live pid
  (`service/handlers.rs:867-912`; `service/core.rs:12580-12596`), and the probe ignores the pin for Gone
  (`hosts/terminal.rs:823-837`). Delete alone uses the pin (`service/teardown.rs:319-343`; SPEC_impl.md "Delete" bullet:
  "the agent's runtime gives its child as pinned at adoption, checked against the process table whatever the host now
  says (a departure from 'a root only while the host reports the child alive')"). With a recorded scope, the scope kill
  still reaches the program, so the gap needs all of: an unscoped launch, a program that survived its host, and an
  unreadable marker. SPEC.md's unscoped limit covers processes Farhelm "can still identify: those still descended from
  the session's terminal, and those whose environment marker it can still read". A reparented program is no longer
  descended from the terminal, so whether a pinned identity counts as "identifiable" is a reading of the spec.
  Unverified: survival of the hangup in practice (as in 19). No runtime test was run.
- Decision: q-gonehost, "When a terminal host is gone but the program it ran is still alive": the maintainer picked
  "Treat it as alive". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "As long as
  this does not turn horrifically complicated. If it turns very complicated during execution, the plan must turn it into
  a follow-up to-do item."
- Completion criteria: A terminal host that is gone while its main program is still identifiable counts as running for
  confirmations, Stop and Restart cleanup. Recommendation for reference: fix code. Root Stop and Restart at the
  revalidated pin, as Delete already does, and refuse when the process table is unreadable. Size M. Natural to combine
  with 19. Complexity gate (maintainer): if the fix turns very complicated during execution, the executor stops and
  turns it into a follow-up TODO.md entry that explains what is complicated and why, instead of landing it. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-terminal-nov24.md` ``.

## trailing-authorized-stop-report-completion-without-settling-recorded-launch.md

- Outcome: `fix code`.
- Assessment: Sometimes a new session's launch cannot be confirmed: its terminal host may still be running, or about to
  start the agent. Farhelm keeps such a session as "launching" with status unknown and offers Stop on it. Stop then
  reports success without settling that recorded start. A host start that has not consumed its launch yet could still
  start an agent after Stop said it was done. Restart and Delete both settle such recorded starts first, or refuse.
  Verified: Confirmed as an omission in the source. On a session with no terminal, `handle_stop_session`
  (handlers.rs:838-1145) skips the refusal guard for unconfirmed launches. That guard only runs when
  `refuse_if_agent_running` is set (:842), and agent-approved or legacy Stop passes false. Stop then classifies the
  exit, sweeps the scope and markers (:1120-1132), and replies success (:1153-1159). It never reads the launch records.
  Restart settles them (SPEC_impl.md:2071-2080, `settle_outstanding_launches`, host_restart.rs:535-640), and so does
  Delete (SPEC_impl.md:2032-2033). SPEC.md:2349-2351 requires a visible failure "when Farhelm cannot confirm that the
  cleanup an operation needs has finished". Whether Stop's needed cleanup includes a recorded host start is a reading.
  Not verified: no runtime test has shown an agent starting after a Stop reply. Reaching this branch first needs a host
  end that could not be confirmed. The comment at handlers.rs:840-841 ("without Delete's unrelated recorded-host
  settlement") shows the authors knowingly left that settlement out of the consent guard. It does not show they decided
  Stop never needs it.
- Decision: q-cleanup, "Keeping cleanup evidence until cleanup is confirmed": the maintainer picked "Keep it until
  confirmed". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Another case where this
  has to not be very complicated. If this turns into a lot of code, a lot of complexity, it must be turned into a
  follow-up to-do item that explains the complexity."
- Completion criteria: Every path keeps or settles the cleanup evidence a launch recorded until cleanup is confirmed.
  Recommendation for reference: fix code — have Stop on a terminal-less launching row reuse Restart's settle-or-refuse
  step before it sweeps, with a test; size M (likely near or over 100 lines with the test). Complexity gate
  (maintainer): if the fix turns very complicated during execution, the executor stops and turns it into a follow-up
  TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-agent-re-adoption-discard-cleanup-identities-without-confirming-host.md

- Outcome: `fix code`.
- Assessment: When a session's terminal host stops answering while its process is still visible, the supervisor keeps
  retrying to reconnect. If a retry then cannot find the host's socket file, the supervisor forgets the host and the
  pinned identity of the program it ran, even though the process may still be alive. A later Delete then has less to go
  on and could miss part of that program's process tree. Verified: Confirmed in source. `install_readopted` maps
  `RowHost::Absent` to no terminal (host_handover.rs:1693). The Unavailable arm keeps the pins via `unavailable_again`
  (:1683-1690), whose docs say a missing socket cannot prove the process ended. The Absent result can come from a
  directory scan that never sees the missing name (:1647-1652, :1245-1251). Delete can still find a recorded host
  through its launch record, as the reviewer notes. Missed cleanup needs an unscoped, unmarked live tree as well. Not
  observed. The reviewer withdrew the claim that a live host leaks.
- Decision: q-cleanup, "Keeping cleanup evidence until cleanup is confirmed": the maintainer picked "Keep it until
  confirmed". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Another case where this
  has to not be very complicated. If this turns into a lot of code, a lot of complexity, it must be turned into a
  follow-up to-do item that explains the complexity."
- Completion criteria: Every path keeps or settles the cleanup evidence a launch recorded until cleanup is confirmed.
  Recommendation for reference: fix code — keep the old handle's pins when a retry finds no socket but the old handle
  had process evidence; size S–M. Or discard, if a later Delete is judged enough. Complexity gate (maintainer): if the
  fix turns very complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that
  explains what is complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-tab-close-discards-retry-state-when-final-process-sweep.md

- Outcome: `fix code`.
- Assessment: Closing a terminal tab ends its host and then sweeps once more for anything that outlived the hangup. If
  that last sweep fails, the user sees an error, but the tab has already been forgotten. There is nothing left to retry
  Close on, and the automatic retry does not run. Only deleting the whole session cleans up the rest. Verified:
  Confirmed. host_tabs.rs:1023-1046: when ending the host succeeds, the record is removed and the tab forgotten before
  the result of the final sweep is returned. The retry state (`mark_end_failed`, :1047-1050) covers only a failed host
  end. In the both-fail branch, the error message says the processes "were reaped" (:1054), which ignores a failed final
  sweep. Delete's whole-session sweep and tab-scope listing remain a cleanup route. No surviving process was observed.
- Decision: q-cleanup, "Keeping cleanup evidence until cleanup is confirmed": the maintainer picked "Keep it until
  confirmed". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Another case where this
  has to not be very complicated. If this turns into a lot of code, a lot of complexity, it must be turned into a
  follow-up to-do item that explains the complexity."
- Completion criteria: Every path keeps or settles the cleanup evidence a launch recorded until cleanup is confirmed.
  Recommendation for reference: fix code — keep the record and the retry mark when the final sweep fails, and correct
  the both-failed message; size S–M. Complexity gate (maintainer): if the fix turns very complicated during execution,
  the executor stops and turns it into a follow-up TODO.md entry that explains what is complicated and why, instead of
  landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-retry-delete-ambiguous-restart-record-before-scope-is-confirmed.md

- Outcome: `fix code`.
- Assessment: A Restart can fail while the new agent's scope cannot be confirmed gone. When that happens, Farhelm keeps
  the new launch's record as evidence. If the user retries Restart while systemd still cannot confirm the scope, the
  retry first deletes that record (its socket is gone), and only then refuses over the scope. After a later supervisor
  restart, startup might then treat the failed launch as never started, and bring back the older terminal. Verified:
  Confirmed in source. `settle_outstanding_launches` removes a record whose socket is gone (host_restart.rs:592-605). It
  runs at core.rs:12612-12614, before the reap that can refuse at :12671-12684. The first unwind keeps the record
  (host_restart.rs:782-787), and SPEC_impl.md (Restarting a host agent, around :2087-2100) says the record stays until
  sweep and scope settle. What startup does with the lost record (host_handover.rs, the no-record fallback) was taken
  from the reviewer and not traced by me. The stored scope selection still blocks an unconfirmed relaunch. #4625
  (729ed1622) fixed a sibling case (delete-meanwhile) but not this retry path.
- Decision: q-cleanup, "Keeping cleanup evidence until cleanup is confirmed": the maintainer picked "Keep it until
  confirmed". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Another case where this
  has to not be very complicated. If this turns into a lot of code, a lot of complexity, it must be turned into a
  follow-up to-do item that explains the complexity."
- Completion criteria: Every path keeps or settles the cleanup evidence a launch recorded until cleanup is confirmed.
  Recommendation for reference: fix code — in the settle step, keep a gone-socket record of the current unresolved
  generation until its scope is confirmed; size S–M. Complexity gate (maintainer): if the fix turns very complicated
  during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is complicated and
  why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## trailing-unwind-failed-host-relaunch-drops-pending-scope-cleanup.md

- Outcome: `fix code`.
- Assessment: The same #4625 safety net covers only one of the two ways a Restart's replacement can fail after the
  session was deleted. If the replacement's terminal host fails after starting an agent, and Farhelm cannot confirm that
  agent's processes stopped, no cleanup task is saved. The next supervisor start then removes the leftover record
  without retrying, so those processes can keep running. Verified: confirmed in source; the route is unverified (as for
  413). `service/core/host_restart.rs:764-781` returns `RelaunchFailure::ambiguous` on a failed sweep without calling
  `record_pending_scope_cleanup`. That call is made only in `tear_down_relaunch_deleted_meanwhile` (:825-839).
  `service/core.rs:13106-13118` then finds the row gone and does not republish, and records no obligation. SPEC_impl.md
  "Approved startup retry" reads: "If Restart cannot confirm that the replacement's recorded Linux scope is gone after
  losing its session row, it records the session ID and launch generation in `supervisor.db`." The code contradicts that
  for this path, so the item is close to clearly settled. I did not classify it so because the fix has two reasonable
  shapes (record in the unwind, or in `relaunch` when `!still_exists`), and it is open whether an ambiguous failure for
  other reasons (host may remain) should also record. I also did not confirm the claim about what startup reclaim does
  with the record (`host_reclaim.rs:715-721`).
- Decision: q-cleanup, "Keeping cleanup evidence until cleanup is confirmed": the maintainer picked "Keep it until
  confirmed". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Another case where this
  has to not be very complicated. If this turns into a lot of code, a lot of complexity, it must be turned into a
  follow-up to-do item that explains the complexity."
- Completion criteria: Every path keeps or settles the cleanup evidence a launch recorded until cleanup is confirmed.
  Recommendation for reference: fix code. The SPEC_impl sentence already demands it, and the fix is local. Size S (about
  15 lines plus a fault-injection test, likely under 100). Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.

## tailscale-uninstall-does-not-bind-verified-host.md

- Outcome: `fix code`.
- Assessment: Uninstalling Farhelm from a Tailscale host checks that the machine is the one Farhelm knows, then runs the
  removal steps. Each step looks up the Tailscale name again. If that name starts pointing to a different machine during
  the run, the remaining steps could stop that machine's supervisor, end its terminals and delete its Farhelm folder.
  Verified: Partly correct. Confirmation re-plans and re-verifies identity under the run's lock
  (`provisioning/service.rs:3094-3112`). Each step then rebuilds its SSH command through `RemoteRoute::command` →
  `SshAccess::prepare` (`access.rs:262-278`), which for Tailscale re-resolves the peer and snapshots that peer's
  advertised keys, so nothing ties later steps to the verified machine. The registry destination itself cannot change
  mid-run: a retarget waits on the provisioning lock (`hosts.rs:928`). The live route is therefore Tailscale
  re-resolving the same name, for example after a node is renamed or removed and another device takes the name.
  Unverified: whether Tailscale actually reassigns a name within the seconds to minutes a run takes, and whether the
  other machine would pass the later steps (same login, Farhelm paths, unit). SPEC.md's 2026-10-10 decision ("a
  confirmed uninstall may be carried out against the entry's new machine, and Farhelm adds no check of the installation
  identity") covers registry destination and identity changes, not name re-resolution. SPEC_impl ("uninstall must not
  act on a machine it has not verified") only covers confirmation time.
- Decision: q-bound, "Approved and confirmed operations bound to the machine they were checked against": the maintainer
  picked "Bind them all". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If this
  turns very complicated, the executor must abort and turn it into a follow-up to-do item explaining the complexity."
- Completion criteria: An approved or confirmed operation refuses once the host connection, machine or build it was
  checked against has changed. Recommendation for reference: fix code. Freeze the resolved Tailscale peer (address and
  keys) at confirmation and use it for every step, refusing on a mismatch. Size M. Complexity gate (maintainer): if the
  fix turns very complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that
  explains what is complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-agent-create-drops-target-incarnation-after-approval.md

- Outcome: `fix code`.
- Assessment: When an agent asks to create a session on a named host and the user approves the card, Farhelm checks that
  the host's connection did not change while the card waited. It then does one more step that waits (a re-check of the
  YOLO setting) and sends the launch on whatever connection the host has by then, without checking again. A host
  retargeted to another machine inside that brief gap would receive a YOLO, command or custom launch the user approved
  for a different machine. Verified: Confirmed. `crates/farhelm-helm/src/agent_requests.rs:2722` records the connection
  and `:2777` checks it; then `check_agent_approved` waits on `store.list_hosts()` (`yolo_guard.rs:139-143`) and returns
  only a bool. Dispatch (`agent_requests.rs:3076-3077`) takes a fresh claim and checks `composition_incarnation`, which
  stays `None` unless the launch pins its folder for trust (`:2107-2131`). Only template hosts get a second check
  (`template_host_moved`). The gap is an in-process wait, sub-millisecond in practice, and the harm needs a retarget to
  a different machine rather than a reconnect to the same one. SPEC_impl.md (around 9740) says "A launch is also refused
  if the target host's connection changed while the card waited, since the approval was for the machine the card named."
  That covers the wait, not the moment after it, so this is a reading. `workspace_trust.rs:50-52` documents the
  narrowing on purpose.
- Decision: q-bound, "Approved and confirmed operations bound to the machine they were checked against": the maintainer
  picked "Bind them all". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If this
  turns very complicated, the executor must abort and turn it into a follow-up to-do item explaining the complexity."
- Completion criteria: An approved or confirmed operation refuses once the host connection, machine or build it was
  checked against has changed. Recommendation for reference: fix code. Have `approve_launch` return the incarnation it
  verified, check it at dispatch as `composition_incarnation.or(approved)`, and correct the comments at
  `workspace_trust.rs:50-52`. Size S (about 20 lines plus a test). Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-agent-stop-does-not-pin-target-connection-across-approval.md

- Outcome: `fix spec+code`.
- Assessment: An agent in one session can ask to stop a session on another host, and the user approves that on a card.
  If the target host's entry is pointed at a different machine, or adopts a different installation, while the card
  waits, Allow still sends Stop to whatever now answers for that host. If that machine has a session with the same ID (a
  cloned VM, restored state), Stop kills it and closes its tabs. That is a machine the card never described. Delete,
  Prompt and Launch refuse in this situation. Verified: Confirmed. agent_requests.rs:619-642 routes and describes the
  target with one connection. After approval, it calls `do_stop_session` (sessions.rs:3162-3176), which routes again and
  checks no connection id. Delete checks one (sessions.rs:4190, agent_requests.rs:2931), and so does Prompt
  (agent_requests.rs:2847). SPEC_impl.md:9733 says "What a card shows is what an approval does", and :9740 makes the
  rule explicit only for launches ("refused if the target host's connection changed while the card waited, since the
  approval was for the machine the card named"). SPEC_impl.md:9491-9494 states the recheck only for the requesting
  host's connection. Nothing names it for Stop, so this is not clearly settled. Not verified: whether retarget or adopt
  can actually produce a same-ID session on the new machine without a clone or restore. A plain reconnect to the same
  machine stops the session the card showed, which is harmless.
- Decision: q-bound, "Approved and confirmed operations bound to the machine they were checked against": the maintainer
  picked "Bind them all". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If this
  turns very complicated, the executor must abort and turn it into a follow-up to-do item explaining the complexity."
- Completion criteria: An approved or confirmed operation refuses once the host connection, machine or build it was
  checked against has changed. Recommendation for reference: fix spec+code — add one SPEC_impl sentence that every
  acting verb pins the target connection, and pass the routed connection id into Stop's dispatch with a Conflict
  refusal, plus a test that mirrors Delete's reconnect test; size S. Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-mac-update-miss-app-installed-before-run-admission.md

- Outcome: `fix code`.
- Assessment: Farhelm refuses to set up or update a remote Mac that has the Farhelm desktop app installed. If the app is
  installed after Update checks the Mac but before the update run is accepted (a short window for manual Update, longer
  for automatic), the update can go ahead and write Farhelm's own background service beside the app. Verified: Confirmed
  by source. Update planning inspects (`service.rs` plan_update), and revalidation's Absent branch executes without
  repeating inspect (`service.rs:2963+`). SPEC_impl.md (plan confirmation, around 8237-8240) repeats the desktop-app
  check only for ADD: "If ADD's confirmation finds no answering supervisor on a Mac, it repeats the desktop-app and
  reach checks." The 2026-10-11 best-effort decision covers queued or running runs, not this pre-claim window. Archived
  precedent: the ADD analogue (mac-setup-confirmation-misses-installed-app.md) was approved as "fix code".
- Decision: q-bound, "Approved and confirmed operations bound to the machine they were checked against": the maintainer
  picked "Bind them all". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If this
  turns very complicated, the executor must abort and turn it into a follow-up to-do item explaining the complexity."
- Completion criteria: An approved or confirmed operation refuses once the host connection, machine or build it was
  checked against has changed. Recommendation for reference: fix code S, mirroring the ADD fix for Mac Update's Absent
  branch. Or discard given the window is small for manual Update. Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-podman-verify-place-nonce-in-argv-forgeable.md

- Outcome: `fix code`.
- Assessment: When Farhelm checks, before a container status check, Delete or recovered create, that a container host's
  SSH destination still reaches the recorded machine and account, the "check passed" proof is a random token that
  Farhelm itself sends in the remote command line. A different host reached through a moved SSH alias can see that
  command line. If that host is hostile, it can print the token without running the check. Farhelm then trusts its
  answer, for example "container absent", which makes the helm forget a container that is still running on the original
  host, or let a recovered create go ahead on the wrong host. Verified: Confirmed as a property of the code.
  `crates/farhelm-helm/src/environment/podman_probe.rs:42-51` passes the nonce as `$2` and prints it only after the
  digest matches. `run_verified` (`:155-190`) accepts any stdout that starts with `nonce\n` (`:174`). Nothing in the
  reply depends on the machine ID, so an endpoint that does not run the script honestly can forge the proof. An honest
  different machine still fails closed. SPEC_impl.md, "Podman places, creation and images": "The shell compares a
  command-specific SHA-256 digest so the recorded machine ID stays at the helm. Its marker proves the check ran and
  matched before the exit status is used." SPEC.md, "Local authority and trust between hosts", says the
  deliberate/accidental line "never relaxes the remote-host boundary, removes a protection this specification states for
  a specific case (such as a container host's SSH destination moving…)". Not established: any real hostile host that
  does this. Note that a compromised endpoint already receives build scripts unverified (item 28), so fixing only this
  one does not close the hostile-endpoint case.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix spec — say whether the guard defends against a hostile endpoint.
  If it should, make it a challenge and response (the reply is a digest of a fresh nonce plus the machine ID and uid,
  which only the right machine can compute). If not, narrow SPEC_impl to honest drift. Size S for the spec only, M with
  code. Complexity gate (maintainer): if the fix turns very complicated during execution, the executor stops and turns
  it into a follow-up TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about
  100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over
  that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-podman-setup-scripts-and-creation-proxy-skip-place-verification.md

- Outcome: `fix spec+code`.
- Assessment: While a recipe image builds or a new container is set up, Farhelm sends each setup step to the container
  host as a script over SSH. The script starts with the host's proxy settings, which can include proxy passwords. These
  sends skip the machine and account check. If the host's SSH configuration is edited outside Farhelm mid-build (for
  example the `User` changed to another account on the same machine, which raises no host-key warning), the next step's
  script, proxy passwords included, goes to the new account. That account receives the bytes even though the container
  command then fails. Two smaller parts: the proxy read at container creation is also unverified, and later build stages
  re-read cached settings instead of the ones that passed the check. Verified: Confirmed by reading. `Podman::script`
  and `script_with_proxy` (`podman.rs:433-497`) take a bare host, prepend `build::proxy_exports(proxy)` and call
  `command_with_input` (`podman_probe.rs` "command_with_input"). That runs `SystemPlaceCommands::execute`
  (`podman_probe.rs:288-350`), which writes stdin as soon as `ssh` is spawned, with no `VERIFY_PLACE_AND_EXEC`.
  `creation_proxy` (`podman_probe.rs`, "creation_proxy") reads proxies over the current route without checking the
  identity. The claim that `%C` defeats ControlMaster reuse on a `User`/`HostName` edit is how OpenSSH documents it, but
  it was not run here. Whether the receiving account actually captures the bytes depends on that endpoint. Stock
  `podman exec` with a missing ID exits without reading stdin, but the bytes have already crossed the SSH channel into
  that account's process. SPEC.md "Container sessions" names only create, status and cleanup as verified operations, so
  whether build scripts are covered is a matter of reading the spec. Part 3 overlaps item 77.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix spec+code — state that secret-bearing sends to a container host
  are verified first. Have the remote shell print the verified proof before Farhelm writes any private stdin, and verify
  the creation proxy read. Size M, likely over 100 lines. Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-adopt-identity-leaves-podman-cache-and-claim-stale.md

- Outcome: `fix code`.
- Assessment: After a container host is wiped or reinstalled (or its address now points at a different machine) and the
  user presses Adopt in Hosts, Farhelm keeps the old machine's Podman check. For up to five minutes a new container or
  Rebuild on that host can fail with a misleading "SSH destination may have changed" error. The host also keeps its
  claim on the old machine, so another host entry that really reaches that old machine is refused as a duplicate until
  the adopted host is checked again. Verified: Confirmed by reading, but it heals sooner than the reviewers say.
  `ConnectionManager::adopt` and `HelmStore::adopt_identity` never call `invalidate_route` and never delete
  `podman_place_claims` (grep finds no Podman reference in either). `confirm_cached_admission` (`podman_probe.rs`)
  accepts the old cache within `FRESH_FOR`. But opening the launcher re-checks every container-capable host (GUI test
  `opening_the_launcher_checks_the_container_hosts`, `launcher/environment_view_tests.rs:193-215`). The adopted host's
  next check replaces its unused claim, so both symptoms go away at the next launcher open or Refresh. A launch sent
  before that check finishes fails once, and a retry succeeds.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix code — on Adopt, invalidate the Podman cache entry under claim
  publication, and release an unused claim when `referenced_place_reason` is None. Size S. Discard is reasonable too,
  since it heals itself. Complexity gate (maintainer): if the fix turns very complicated during execution, the executor
  stops and turns it into a follow-up TODO.md entry that explains what is complicated and why, instead of landing it.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-podman-refresh-caches-wrong-machine-on-refusal.md

- Outcome: `fix code`.
- Assessment: If a container host's SSH alias briefly points at another Linux machine with Podman and Farhelm checks it
  during that time, Farhelm keeps that other machine's details (its Podman program path and proxy settings) as the
  details existing containers use. When the alias is put back, status, reconnect, Suspend/Resume and Delete of existing
  containers can keep failing if Podman lives at a different path on the two machines. An interrupted container create
  that recovers in that window can install the other machine's proxy settings into the container. This lasts until the
  host is checked again. Verified: Confirmed by reading. `refresh` (`podman_probe.rs`, "refresh", the admission-refusal
  match) caches `result: Ok(ready)` even when the result is `Changed`, `Twin`, an ordinary refusal or an identity error.
  `runtime()` returns any cached `Ok` for the same route text, ignoring `admission_refusal` and age. Callers use
  `ready.podman` and `ready.proxy` (`podman.rs:441`). The recovery path is real, but it needs an alias move, a probe
  during the move, the restore, and then an interrupted create recovering before any re-check. Opening the launcher
  re-checks, which heals it. Not verified: that two supported hosts install Podman at different absolute paths.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix code — never replace an existing matched observation with one
  whose identity conflicts or fails, and have `runtime()` re-probe when the cached identity does not match the binding's
  recorded identity. Size S-M. Complexity gate (maintainer): if the fix turns very complicated during execution, the
  executor stops and turns it into a follow-up TODO.md entry that explains what is complicated and why, instead of
  landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-podman-refresh-settles-claims-with-stale-pre-probe-row.md

- Outcome: `fix code`.
- Assessment: If a host's Farhelm setup or update changes where Farhelm is installed on it while a container host check
  is running, the check can use the host's old details. A host Farhelm is refusing as a duplicate or changed
  installation can then take the machine's container claim and block another valid entry, or a newly connected owner can
  fail to take an unused claim. Verified: Confirmed that `refresh` reads `row` before `probe()` and uses it for
  `ordinary_state` and `settle_claim` after publication (`podman_probe.rs`, "refresh"). The trigger is a narrow overlap
  of setup/update with a probe. The wrong claim is stored durably. Whether the valid owner's next check takes it back
  depends on the refusal kind (see `settle_claim`), so I cannot say it heals reliably.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix code — re-read the row under claim publication and check the
  entry and route before settling. Size S. Discard is reasonable too, given the narrow trigger. Complexity gate
  (maintainer): if the fix turns very complicated during execution, the executor stops and turns it into a follow-up
  TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-podman-build-prepare-uses-runtime-seam-bypassing-admission.md

- Outcome: `fix code`.
- Assessment: When a container host's Farhelm connection becomes refused (duplicate, changed or unverified installation)
  after a launch or Rebuild has started, later build and preparation stages still start new image-building containers on
  that host. The machine and account are still verified, so the work stays on the right machine and account. Verified:
  Confirmed by reading. `software()` (`build.rs:162`) and `prepare_images_owned` take their observation from
  `self.place()`, that is `runtime()`, which ignores freshness, cached refusals and ordinary refusals. `prepare_images`
  calls `fresh()` only when `rebuild` (`build.rs:291-293`). The admission check happens once at the start. SPEC.md
  "Container sessions" ("A Farhelm connection refusal for a duplicate, changed or unverified installation also prevents
  new work through that host") does not say whether later stages of work already admitted count as new work. Whether
  they do is a matter of reading the spec.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix code — check admission inside `prepare_images` every time and
  before each new helper. Size S-M. Discard is defensible. Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-claim-podman-place-retains-unused-claim-after-twin.md

- Outcome: `fix code`.
- Assessment: If an unused host entry's SSH destination changes outside Farhelm so that it reaches a machine another
  entry already claims, Farhelm correctly refuses it as a duplicate. But it keeps the entry's old machine claim, so
  another entry that really reaches the old machine can be refused as a duplicate of the moved one. Verified: Confirmed.
  In `claim_podman_place` (`store/podman.rs:381-450`), the `Twin` branch returns before `tx.commit()`, so the old
  `(host, M1)` row stays. Only a later successful claim, a retarget or a removal clears it. SPEC_impl.md allows "An
  entry with no containers, image records or helpers can replace its claim" but does not say a refusal should release
  it.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix code — release an unused claim that no longer matches, and
  commit before returning `Twin`. Size S. Complexity gate (maintainer): if the fix turns very complicated during
  execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is complicated and why,
  instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-podman-leftovers-skip-place-identity-verification.md

- Outcome: `fix code`.
- Assessment: In Settings → Containers, if a container host's SSH destination now reaches a different machine or
  account, the "Containers not on this helm" list can show none of the original host's leftovers. Deleting a leftover
  shown earlier can report success while the container still exists on the original host. It shows up again after the
  destination is restored. Verified: Confirmed. `delete_leftover` and `list_owned` use `PlaceTarget::from(HostId)` (no
  identity, `podman.rs:832-865`). `delete_leftover` returns `Ok` when the container is absent at the moved endpoint. A
  container registered to any host is still refused (`podman.rs:847-862`), so another host's live container cannot be
  deleted through this path. This is "success reported for an operation that failed", which keeps it out of the filters.
  SPEC.md requires verification only for containers with a recorded machine and account. Leftovers have no row, but
  their host does have a durable claim.
- Decision: q-podid, "How far the container-host identity check reaches": the maintainer picked "Every command and
  cached result". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "If it turns very
  complicated, the executor must abort and turn this into a to-do item for a follow-up. Explaining what is complicated."
- Completion criteria: Apply the container-host identity check to every command sent to a container host, including
  against a hostile endpoint (the nonce must not be echoable), and drop capabilities and claims learned from an
  unverified machine. Recommendation for reference: fix code — use the host's durable claim when one exists. Size S-M.
  Complexity gate (maintainer): if the fix turns very complicated during execution, the executor stops and turns it into
  a follow-up TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about 100 lines
  of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-container-inherits-removed-proxy-setting.md

- Outcome: `fix code`.
- Assessment: A new container is supposed to get the proxy settings its host has at the moment it is created. If the
  host removed a proxy variable after the recipe image was prepared (up to 24 hours earlier), the new container's
  Farhelm, and the sessions it starts, can still use the old proxy. The prepared image carries the old settings file,
  and Farhelm's update at creation cannot remove a variable that the container's systemd loaded from that file at boot.
  Verified: Confirmed by reading, relying on upstream systemd behavior. Preparation installs
  `environment.d/90-farhelm-proxy.conf` before commit (`build.rs:196`, `:400`), and nothing removes it (no other
  reference to `farhelm-proxy` outside `podman.rs:701-722`). At creation, `podman.rs:1428-1468` rewrites the file, runs
  `unset-environment` for every proxy name and imports only the keys that are present (`podman.rs:731-746`), then
  restarts the supervisor without reloading the manager. In systemd, `unset-environment` changes only the manager's
  client environment, and variables from boot-time environment generators stay in effect (systemd v255 `manager.c`, as
  cited). This contradicts SPEC.md "Container sessions" ("Containers inherit proxy variables from the container host's
  systemd user manager environment at creation… Later host changes affect newly created containers"). Not observed live.
  There is more than one possible fix, so it is not clearly settled.
- Decision: q-proxy, "Proxy settings baked into recipe images": the maintainer picked "Remove them from images".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Committed recipe images carry no proxy file; new containers get only the proxy values read at
  creation. One change settles 60, 282 and 283. Recommendation for reference: fix code — remove the helper proxy file
  before every commit, so boot-time values never come from the image. The same change settles 282 and gives 283 its
  test. Size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or
  a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-recipe-images-retain-proxy-configuration-used-by-their-helpers.md

- Outcome: `fix code`.
- Assessment: Recipe images keep the proxy settings file that was used while they were built. Changing the host's proxy
  settings later, or deleting a session, does not remove the old values, passwords included, from those shared images.
  They stay in the same account's Podman storage. Verified: Confirmed that the file is retained: nothing removes
  `90-farhelm-proxy.conf` before commit (only `podman.rs:701-722` references it). No exposure outside the account's own
  Podman storage is established. The same retained file is what causes item 60.
- Decision: q-proxy, "Proxy settings baked into recipe images": the maintainer picked "Remove them from images".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Committed recipe images carry no proxy file; new containers get only the proxy values read at
  creation. One change settles 60, 282 and 283. Recommendation for reference: fix code — together with 60 (remove the
  file before commit); no extra size beyond 60. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-fresh-proxy-regression-overlooks-installed-boot-file.md

- Outcome: `fix code`.
- Assessment: The test for "new containers use current proxy settings" checks the import step, not the bytes written to
  the settings file that later boots read, and it does not check a removed key. Verified: Confirmed as a test gap
  (`podman/failure_tests.rs`, as cited). The production code writes the fresh map. This test is the evidence item 60
  would need.
- Decision: q-proxy, "Proxy settings baked into recipe images": the maintainer picked "Remove them from images".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Committed recipe images carry no proxy file; new containers get only the proxy values read at
  creation. One change settles 60, 282 and 283. Recommendation for reference: fix code — only as the test for 60,
  otherwise discard; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-wake-state-misclassifies-failed-stopped-and-stalls-60s.md

- Outcome: `fix code`.
- Assessment: Launching on a stopped container or a suspended sandbox wakes it first. When the wake fails after its
  20-second reply bound, and the failure has the same text as an earlier one, the launcher either waits a full minute
  and then says the host "resumed but did not connect", or says the host was "suspended again while it was waking". A
  stopped container is also told "Resuming the host failed" and "suspended again", although stopped containers are never
  supposed to be called suspended. Verified: confirmed by source (unchanged since the reviewed commit). `Watch::observe`
  classifies a provider failure only when the failure text differs from the starting state's, and treats Stopped as
  `Waiting` until a connection is seen (`launcher/launch/wake_state.rs:55-117`). The failure words are fixed: "The host
  was suspended again while it was waking" (`wake_state.rs:228-233`) and "Resuming the host failed:"
  (`launch/wake.rs:149-151`). This contradicts SPEC.md, "Container sessions" (`SPEC.md:5502`: "Stopped has its own state
  and is never labelled Suspended"; the pill says **Starting**). The 60 s path needs a start over 20 s that then fails
  with an unchanged error. The wording defect has a common trigger: any container start refused within the bound. That
  keeps it out of the rare-glitch filter.
- Decision: q-wake, "What waking a host at launch must guarantee": the maintainer picked "Never wedge, fail fast, word
  per provider". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Wait, another
  complexity gate. If this turns complicated, the executor must stop and turn it into a follow-up to the item explaining
  what's complicated and why."
- Completion criteria: Launch-time waking never wedges, fails fast and words failures per provider. Recommendation for
  reference: fix code (track a seen Resuming/Starting phase, and use provider-specific failure words). Size M; with
  tests it likely exceeds 100 lines. Complexity gate (maintainer): if the fix turns very complicated during execution,
  the executor stops and turns it into a follow-up TODO.md entry that explains what is complicated and why, instead of
  landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-hidden-post-wake-folder-wait-wedges-launcher.md

- Outcome: `fix code`.
- Assessment: A quick launch (or a launch that just woke its host) waits out of sight for the host's recent-folder
  history. If the host reconnects meanwhile without being reported as disconnected, the history answer is thrown away
  and nothing retries. The launch stays stuck with its "Resuming…" pill, and new launches and that host's Suspend/Resume
  are blocked until the host disconnects or the window reconnects to the helm. Verified: confirmed by source. After
  wake, `wake::follow` returns early whenever the host is still `Connected` with the same installation, without
  comparing incarnation or verification (`launcher/launch/wake.rs:214-219`). The history reply callback returns before
  `history_settled_for_quick_launch` when the key moved (`launcher/view/history.rs:135-139` vs `:161`). `folder_wait` is
  then never taken again (`view.rs:401-410, 425-441`). The same early return can also strand a non-wake hidden quick
  launch whose host changes during the read (by inspection; not run). This is a persistent wedge, so no filter applies.
- Decision: q-wake, "What waking a host at launch must guarantee": the maintainer picked "Never wedge, fail fast, word
  per provider". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Wait, another
  complexity gate. If this turns complicated, the executor must stop and turn it into a follow-up to the item explaining
  what's complicated and why."
- Completion criteria: Launch-time waking never wedges, fails fast and words failures per provider. Recommendation for
  reference: fix code (settle or re-drive the folder wait when a stale reply is dropped while one is pending). Size S-M
  with a test. Complexity gate (maintainer): if the fix turns very complicated during execution, the executor stops and
  turns it into a follow-up TODO.md entry that explains what is complicated and why, instead of landing it. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-wake-drops-one-time-yolo-confirmation.md

- Outcome: `fix code`.
- Assessment: When a launch needs the one-time "Start YOLO session anyway" confirmation, and the target sandbox or
  container gets suspended or stopped while that question is open, answering it wakes the host. The yes is then thrown
  away, so once the host is back the helm refuses the launch and the same YOLO question appears again. The user has to
  confirm a second time. Nothing launches without confirmation. Verified: confirmed. `Launcher::submit`
  (`crates/gui/farhelm-gui/src/launcher/view.rs:1416-1435`) clears `self.yolo` and returns into `begin_host_wake` before
  `self.request(confirm_yolo, …)`. Nothing passes `confirm_yolo` on, and `continue_after_wake` (`view.rs:2994-3005`)
  ends in `submit(false, …)`. SPEC_impl.md "Questions" (`SPEC_impl.md:1004-1005`) says the answer "carries its
  permission through a wait for a host setting or checkout preview". A host wake is not named there, so carrying it is a
  reading of that rule, not its words.
- Decision: q-wake, "What waking a host at launch must guarantee": the maintainer picked "Never wedge, fail fast, word
  per provider". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Wait, another
  complexity gate. If this turns complicated, the executor must stop and turn it into a follow-up to the item explaining
  what's complicated and why."
- Completion criteria: Launch-time waking never wedges, fails fast and words failures per provider. Recommendation for
  reference: fix code — carry the confirmation through the wake for the same frozen installation, and extend the
  SPEC_impl sentence to name the wake; size S–M. Complexity gate (maintainer): if the fix turns very complicated during
  execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is complicated and why,
  instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-restart-with-wake-refusal-hardcodes-not-launched.md

- Outcome: `fix code`.
- Assessment: In Restart with…, if waking the suspended or stopped host fails, the alert beside the Restart button
  starts "Not launched." where Restart's other refusals start "Restart error.". Verified: confirmed.
  `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:150` and `:278` hard-code "Not launched. ", and
  `view.rs:2737-2740` shows `Note::WakeRefusal` verbatim, which bypasses the mode-aware `lead` (`view.rs:2708-2712`).
  Restart with does wake (`wake.rs:89`). The spec gives two readings: `SPEC.md:1610-1613` says "Restart mode uses the
  same alert beside Restart, with 'Restart error.' before the refusal's words", while `SPEC.md:407-408` says "A failed
  preparation says the session was not launched." Because a wake failure is a "failed preparation", the fix depends on
  which sentence governs, so this is not clearly settled.
- Decision: q-wake, "What waking a host at launch must guarantee": the maintainer picked "Never wedge, fail fast, word
  per provider". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Wait, another
  complexity gate. If this turns complicated, the executor must stop and turn it into a follow-up to the item explaining
  what's complicated and why."
- Completion criteria: Launch-time waking never wedges, fails fast and words failures per provider. Recommendation for
  reference: fix code — take the lead from the mode when drawing `WakeRefusal` (and say so next to `SPEC.md:408`); size
  S. Complexity gate (maintainer): if the fix turns very complicated during execution, the executor stops and turns it
  into a follow-up TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-unverified-resumable-host-shows-checkout-allocation-notice.md

- Outcome: `fix code`.
- Assessment: In the launcher, a managed checkout aimed at a suspended or stopped sandbox or container that Farhelm has
  never verified (no recorded installation identity) shows "The folder is picked automatically when the host is ready".
  Launch is refused at the same time with "The host installation is not verified". The two lines contradict each other.
  Nothing wrong is launched. Verified: confirmed. `LaunchDraft::checkout_refusal`
  (`crates/gui/farhelm-gui/src/launcher/request.rs:415-424`) requires a non-empty identity before `can_wake`.
  `checkout_allocation_notice` (`launcher/view/checkout.rs:902-913`) checks only `can_wake`. The reviewer's follow-on
  point also holds: gating the notice alone would make the Folder line read "Waiting for a current checkout preview"
  forever, so the fix needs one wording choice for the unverified case. How often hosts reach this state was not
  established.
- Decision: q-wake, "What waking a host at launch must guarantee": the maintainer picked "Never wedge, fail fast, word
  per provider". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Wait, another
  complexity gate. If this turns complicated, the executor must stop and turn it into a follow-up to the item explaining
  what's complicated and why."
- Completion criteria: Launch-time waking never wedges, fails fast and words failures per provider. Recommendation for
  reference: fix code — a shared eligibility helper and an explicit unverified line; size S. Complexity gate
  (maintainer): if the fix turns very complicated during execution, the executor stops and turns it into a follow-up
  TODO.md entry that explains what is complicated and why, instead of landing it. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-failed-post-wake-checkout-preview-omits-refusal-alert.md

- Outcome: `fix code`.
- Assessment: When you launch a managed checkout on a suspended host and the checkout preview fails after the host comes
  back (for example, the typed name is already taken), the launcher reopens with the reason only on the Folder line, as
  a polite status. There is no bordered "Not launched." alert beside Launch, and screen readers get no alert. When the
  preview was instead replaced by a newer one, the launcher reopens showing the new folder and no alert. Verified:
  partly correct. Confirmed that `settle_waiting_launch`
  (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:541-590`) calls `reveal_prepared` without a refusal, and that
  the taken-name test (`launcher/checkout_tests.rs:2560-2584`) only asserts `launcher-preview-error`. The failed-preview
  case conflicts with the reading of `SPEC.md:408` ("A failed preparation says the session was not launched") and
  `SPEC.md:1610-1612` (every refused launch gets the alert). The superseded-preview case is deliberate: the doc comment
  at `checkout.rs:528-534` cites SPEC.md's "never submits automatically or silently chooses another directory" and says
  the user is shown the new folder and decides. Whether a failed post-wake preview counts as a "refused launch" is a
  reading of the spec.
- Decision: q-wake, "What waking a host at launch must guarantee": the maintainer picked "Never wedge, fail fast, word
  per provider". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "Wait, another
  complexity gate. If this turns complicated, the executor must stop and turn it into a follow-up to the item explaining
  what's complicated and why."
- Completion criteria: Launch-time waking never wedges, fails fast and words failures per provider. Recommendation for
  reference: fix code — record the failed preview's reason as the launcher refusal (keep the superseded case as is) and
  assert `shown_refusal` in the taken-name test; size S–M. Complexity gate (maintainer): if the fix turns very
  complicated during execution, the executor stops and turns it into a follow-up TODO.md entry that explains what is
  complicated and why, instead of landing it. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-replace-drops-late-checkout-warning.md

- Outcome: `fix code`.
- Assessment: Replace with into another folder or a fresh clone deletes the source session. If that deletion leaves the
  source's checkout on disk (it could not be moved to trash) and the Replace takes longer than the 20-second reply
  bound, the notice naming the left-behind folder never reaches the user. The desktop's ordinary Delete saves such late
  notices to the notices bell. Verified: confirmed by source. The Replace route uses a plain `within_host_reply_bound`
  and drops the late body (`sessions.rs:5202-5240`). The Delete route captures the notice and records it with
  `record_late_delete` after a 504 (`sessions.rs:4099-4135`). `finish_replacement` captures the notice
  (`sessions.rs:5115-5121`), and its own comment says it "must reach the user like any other Delete's". SPEC.md,
  "Managed checkouts" (`SPEC.md:2088-2095`: "That outcome must never be silent ... If the reply timed out before Delete
  finished, its checkout notice appears in the navigation rail's notices bell") is written about Delete. That it covers
  Replace's source Delete rests on a reading (SPEC_impl.md:3907-3908 says Replace's reply carries the notice, and
  :3931-3934 describes the late save for Delete). The folder itself is retained on disk.
- Decision: q-latenotice, "Late "checkout left behind" notices": the maintainer picked "Every route reaches the bell".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A late "checkout left behind" notice from any route that removes a session reaches the notices
  bell. Recommendation for reference: fix code. Reuse the Delete route's late-notice wiring for Replace. Size S-M (about
  40-80 lines with a test). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-timed-out-agent-delete-lose-late-checkout-warning.md

- Outcome: `fix code`.
- Assessment: When an agent asks Farhelm to delete a session and the user approves, a slow teardown can outlast the
  agent's request. If that teardown then leaves the checkout on disk, the notice naming it reaches neither the agent nor
  the notices bell. It survives only in the host's internal logs. Verified: confirmed by source (no runtime check).
  `agent_delete` calls `do_delete_session` or `delete_session_within_environment` directly and returns the reply
  (`agent_requests.rs:2898-2938`). There is no late-notice collector like the desktop Delete's
  (`sessions.rs:4099-4135`). The relay's timed-out retention discards the outcome (per reviewer,
  `service/agent_relay.rs`; not re-read). SPEC.md:2088-2095 states the late-notice rule for Delete. Whether it covers
  agent Delete is a reading; SPEC.md:4332-4338 gives agent Delete the UI's notices.
- Decision: q-latenotice, "Late "checkout left behind" notices": the maintainer picked "Every route reaches the bell".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A late "checkout left behind" notice from any route that removes a session reaches the notices
  bell. Recommendation for reference: fix code if item 59's answer is yes. Share the late-notice recording. Size M. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-environment-delete-drops-checkout-warning-received-after-inner-reply.md

- Outcome: `fix code`.
- Assessment: Deleting one session from a sandbox or container that stays for other sessions waits up to four minutes
  for the environment's Farhelm. If the answer comes later and says a checkout was left behind, the notice is dropped.
  Verified: confirmed by source. `tokio::time::timeout(ENVIRONMENT_DELETE_REPLY_DEADLINE, …)` maps elapsed to
  `EnvironmentUnasked` and drops the reply (`sessions/sandbox.rs:245-252`). The 240 s bound is a deliberate, commented
  choice ("a request still unanswered then has almost certainly lost its connection", `:157-166`). The trigger needs a
  teardown over four minutes, which SPEC.md "Slow hosts" makes acceptable as slowness but not as a lost notice. The
  rare-glitch filter was judged ambiguous by the recording's own independent check; I agree it is unclear, so it does
  not match.
- Decision: q-latenotice, "Late "checkout left behind" notices": the maintainer picked "Every route reaches the bell".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A late "checkout left behind" notice from any route that removes a session reaches the notices
  bell. Recommendation for reference: discard, given the four-minute premise. If item 59 gets a shared late-notice path,
  folding this in is size M. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-interrupted-uploads-invisible-on-card-or-retired-session.md

- Outcome: `fix code`.
- Assessment: You paste or drop a file into the open session, and while it uploads the session loses its terminal (a
  host reboot or update puts it on its recovery card, a container session is retired, or a reconnect lands on an
  unreachable host). The interrupted upload is then reported nowhere until you open another session. If you quit first,
  or the session is a retired container that never gets a terminal again, it is never reported. Verified: Confirmed.
  `crates/gui/farhelm-gui/src/terminal/attachments.rs:401-441` posts held interruptions only for sessions that are not
  selected (`:409`). `session_pane.rs:436-463` drops the terminal set, or mounts none, while the session stays selected
  (card, retired or unavailable). No commits since the reviewed commit touch this. SPEC.md "Attachments" (3752): "Upload
  failures must be visible; an attachment must never disappear silently." SPEC_impl.md "One failure path: the reset"
  covers only a session that stays open with a new terminal set, or a selection that moves on.
- Decision: q-upload, "Interrupted uploads must never disappear silently": the maintainer picked "Fix all three".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: No interrupted upload disappears silently. Recommendation for reference: fix code. Post held
  interruptions to the notice center when the selected session has no terminal set. Size S. Size gate: about 100 lines
  of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-duplicate-interrupted-upload-notices-silently-dropped.md

- Outcome: `fix code`.
- Assessment: An upload notice for session S and file `pasted-1.png` is still showing, undismissed. A second interrupted
  upload with the same session, file name and outcome then posts no new notice and does not scroll the existing one into
  view. Every new terminal names its first pasted image `pasted-1.png`, so two different screenshots can collide.
  Verified: Confirmed. `crates/gui/farhelm-gui/src/notices.rs:91-99` derives equality over session, title, files and
  sent, and `post` (`:177-186`) ignores a notice equal to one already posted. `attachments.rs:562,611-614` restarts the
  pasted counter for each terminal. `main_view.rs:204-206` reveals only a new last notice. The identical notice that is
  already showing does name the file and session, so the second failure is not invisible, only merged into it and not
  revealed.
- Decision: q-upload, "Interrupted uploads must never disappear silently": the maintainer picked "Fix all three".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: No interrupted upload disappears silently. Recommendation for reference: fix code. Give each
  interrupted-upload notice its own occurrence id, which also makes the element ids unique. Size S. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## trailing-interrupted-upload-empty-title-and-batch-title-drop.md

- Outcome: `fix code`.
- Assessment: The notice for an upload interrupted in a session with an empty title reads "Attaching x to was
  interrupted", with a dangling "to". Separately, if the first upload of a batch started before the session's row
  loaded, the notice leaves out a title a later upload in the batch had captured. Verified: Confirmed.
  `notices.rs:142-144` adds " to <title>" for any `Some`, and `attachments.rs:425-426` takes the first upload's title.
  The GUI's Rename accepts an empty title (`session_list/rename.rs:19,131-133`), so an empty title comes from ordinary
  input, not a rare trigger. The batch-title half matches "Rare, self-correcting glitches and imprecise diagnostics" (a
  narrow timing window; the only consequence is detail missing from a message). The empty- title half does not, so the
  item does not fully match.
- Decision: q-upload, "Interrupted uploads must never disappear silently": the maintainer picked "Fix all three".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: No interrupted upload disappears silently. Recommendation for reference: fix code. Filter empty
  titles when building the notice. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## fresh-checkout-trust-can-bind-replacement-directory.md

- Outcome: `fix spec`.
- Assessment: When Farhelm clones a new managed checkout with Claude trust turned on, it checks that the checkout folder
  is the one it created, runs the user's configured post-clone command, and then marks the folder at that path trusted.
  If something replaced the folder in between, Claude trust goes to the replacement. Verified: confirmed in the code.
  The identity check is at `supervisor/src/launch.rs:1755-1797`, the post-clone command at `:1940-1963`, and the trust
  write afterwards at `:1986-1999` (through `trust::resolve`, which canonicalizes the path again). Only two things could
  replace the folder: the user's own post-clone command, or a process running under the same account. SPEC.md "Local
  authority and trust between hosts" says: "Processes the user runs on the target host, including same-account processes
  ... are trusted by this threat model", and that deliberately reshaped files count as the user's intent. The finding
  itself says a lower-authority replacer is unverified.
- Decision: q-intent, "Does a setup the user made on purpose count as the user's intent?": the maintainer picked "Yes,
  it is the user's intent". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Write into SPEC.md "Local authority and trust between hosts" that this deliberate user setup is
  the user's intent. No code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-claude-trust-relative-config-dir-persists-relative-lock.md

- Outcome: `fix code`.
- Assessment: Suppose the user's login shell sets a relative `CLAUDE_CONFIG_DIR` and Claude trust is on. Farhelm's
  automatic Claude trust then looks for Claude's settings inside the session folder, following any symlink there. For a
  managed checkout, it also saves a relative lock path, which the later cleanup resolves against the supervisor's own
  working directory. Verified: confirmed. `agent_kind/claude/trust.rs:47-60` rejects a relative `HOME` but accepts a
  relative config directory. `resolve` (`:96-125`) canonicalizes the config path but builds `lock_path` from the
  uncanonicalized relative path. The trigger is a shell setting the user made on purpose. SPEC.md "Local authority and
  trust between hosts" names "their shell environment" as user intent, and Claude itself would read the same relative
  path. The relative lock path in the supervisor's cleanup is Farhelm's own mistake and does not follow from what the
  setting means.
- Decision: q-intent, "Does a setup the user made on purpose count as the user's intent?": the maintainer picked "Yes,
  it is the user's intent". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Refuse a relative CLAUDE_CONFIG_DIR from the login shell, as the Recommend line describes.
  Recommendation for reference: fix code. Reject a relative config directory the same way as a relative `HOME`, so the
  trust edit is skipped with the existing notice. Size S (about 5 lines plus a unit test). Size gate: about 100 lines of
  diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## antigravity-trust-falls-back-to-uncanonical-folder.md

- Outcome: `fix spec`.
- Assessment: If a folder can no longer be resolved, or its resolved path has no text encoding, between launch admission
  and the moment Antigravity's trust option is filled in, Farhelm puts the folder as submitted into Antigravity's trust
  list. Verified: confirmed, and the behavior is deliberate. The comment at `agent_kind/mod.rs:2447-2451` says: "If the
  path changes after the earlier usability check, retain the original spelling: the subsequent exec will decide whether
  the directory still exists, without accidentally trusting another key." Codex shares the same fallback. The reviewer
  itself says no trust grant outside scope is established. The likely effect is that Antigravity asks its own question.
- Decision: q-intent, "Does a setup the user made on purpose count as the user's intent?": the maintainer picked "Yes,
  it is the user's intent". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Write into SPEC.md "Local authority and trust between hosts" that this deliberate user setup is
  the user's intent. No code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-shared-signin-triggers-auto-update-concurrently-with-manual-action.md

- Outcome: `fix code`.
- Assessment: With automatic updates on, an outdated SSH host that needs a sign-in waits in the attention row. If you
  choose Update or Uninstall on it, your action shares that waiting sign-in. When you answer, the background automatic
  Update starts at once. Your Update can then fail as busy with a warning badge, and your Uninstall's plan races an
  update you did not ask for at that moment. Verified: Confirmed by source. Sharing: `provisioning/setup_sign_in.rs`
  start_manual/start_automatic join one attempt. On Ready, the automatic resume task replans and admits (`automatic.rs`
  begin_sign_in/resume; `service.rs:1962-2017`). Manual planning holds no busy claim. SPEC_impl.md (SSH sign-in, around
  2771-2781) says "A successful check starts batch replanning through the listener's per-host attempt" and "Checks on
  the same registered entry and SSH route share one attempt", so the automatic resume itself follows the spec. The spec
  does not say which caller wins after a shared answer. Uninstall still changes nothing until confirmed, and the
  automatic Update is authorized by the preference, so the uninstall-rule violation is arguable. The spurious Busy
  failure on manual Update is the clearest harm.
- Decision: q-autoupd, "Automatic host updates against what the user explicitly asked for": the maintainer picked "The
  user's action wins". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A pending automatic-update sign-in yields to any user action on that host; SPEC.md's
  contradiction about when the automatic sign-in popover opens is resolved to match. Recommendation for reference: fix
  code M (a manual join supersedes the automatic continuation), and fold 452's test into it. Size gate: about 100 lines
  of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-automatic-signin-tests-omit-shared-prompt-with-auto-updates-on.md

- Outcome: `fix code`.
- Assessment: Maintainer test gap. No test answers a sign-in shared between an automatic update and a manual Update or
  Uninstall while automatic updates stay on, which is the case where 90's race happens. Verified: Confirmed by the
  reviewer. The test file was touched only by d26b59f31 (docs) since.
- Decision: q-autoupd, "Automatic host updates against what the user explicitly asked for": the maintainer picked "The
  user's action wins". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A pending automatic-update sign-in yields to any user action on that host; SPEC.md's
  contradiction about when the automatic sign-in popover opens is resolved to match. Recommendation for reference:
  other. Fold into 90's fix as its regression test if 90 is fixed, otherwise discard. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-spec-md-contradicts-send-handoff-auto-opening-auto-update-popover.md

- Outcome: `fix spec`.
- Assessment: The spec contradicts itself about when an automatic update's sign-in question may open on its own. One
  place says only when you press Answer. The focus rules (and the app) let answering another host's sign-in question
  move straight on to an already-waiting automatic-update question. Verified: Confirmed. SPEC.md:5996-5998 says "…its
  popover opens only when the user presses **Answer**". SPEC.md:3797-3799 says "…the field may move to the same
  sign-in's next question or to another question that was already waiting." `gui/.../sign_in.rs` was unchanged since
  d26b59f31.
- Decision: q-autoupd, "Automatic host updates against what the user explicitly asked for": the maintainer picked "The
  user's action wins". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A pending automatic-update sign-in yields to any user action on that host; SPEC.md's
  contradiction about when the automatic sign-in popover opens is resolved to match. Recommendation for reference: fix
  spec XS (qualify 5997 to allow the already-waiting Send handoff). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-unanswered-auto-update-signin-blocks-set-tailscale-hostbusy.md

- Outcome: `fix code`.
- Assessment: While an automatic update's SSH sign-in question waits unanswered (up to two minutes per prompt), turning
  Tailscale host on or off for that host fails with "This host is busy", though nothing the user started is running.
  Verified: Confirmed. `provisioning/setup_sign_in.rs:318` holds `ssh_route_lock` read for the wait, and
  `hosts.rs:993-997` uses `try_write_owned` → HostBusy. Removal and the preference opt-out cancel automatic checks
  first; `set_tailscale` does not. It recovers when the prompt is answered, expires or is cancelled.
- Decision: q-autoupd, "Automatic host updates against what the user explicitly asked for": the maintainer picked "The
  user's action wins". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A pending automatic-update sign-in yields to any user action on that host; SPEC.md's
  contradiction about when the automatic sign-in popover opens is resolved to match. Recommendation for reference: fix
  code S (cancel auto-only checks for that host before taking the route lock, preserving the retry opportunity), or
  discard. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-host-scoped-trash-confirmation-stays-open-invisibly.md

- Outcome: `fix code`.
- Assessment: In the Trash dialog, if you open a host's "Delete all on …" confirmation and that host is then removed, or
  its trash is emptied elsewhere, the confirmation disappears from view but stays in effect. Every other host's Delete
  and Retry button stays disabled with nothing on screen explaining why. The first Close or Escape is swallowed, and the
  old confirmation can reappear if that host gets a new archive. Verified: confirmed. `TrashDialog::render`
  (`crates/gui/farhelm-gui/src/trash/dialog.rs:405-407`) clears `asked` only once the inline prompt is answered
  (`open_question`, `:241-243`). `host_section` returns `None` for an empty host (`:597-606`) and the loop draws nothing
  for a removed host, yet `deletes_allowed` still requires `asked.is_none()` (`:416-419`), and `dismissed` (`:248-256`)
  swallows the first close. Only `681a39091` (reduce motion) touched this code since the reviewed commit. The governing
  rule is SPEC.md "Keyboard focus" (`SPEC.md:3853-3855`): each confirmation "goes away … when the thing it is about is
  gone". This is close to clearly settled. It is held back because "gone" for an emptied but still-listed host is a
  reading, and the fix has to choose what to do with the focus handles of hosts that are not drawn.
- Decision: q-staleconf, "Confirmations and kept state whose subject is gone": the maintainer picked "Close or
  invalidate them". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A confirmation or kept state whose subject is gone (host removed, listing emptied, account key
  changed) closes or is invalidated. Recommendation for reference: fix code — clear a host-scoped question whose host
  section is not drawn, with a GPUI test; size S–M. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

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

## trailing-tensorlake-orphan-cleanup-retry-falsely-succeeds-after-key-change.md

- Outcome: `fix code`.
- Assessment: In Settings → Tensorlake, an unfinished cleanup of a leftover sandbox (for example, its snapshot deletion
  failed) stays listed with Retry cleanup. If the user then switches to a different Tensorlake account's API key and
  presses Retry, the new account answers "not found" for everything. Farhelm reports success and removes the row, but
  the old account's snapshots were never deleted and may keep billing. Verified: confirmed. `Cleanups` is keyed by
  sandbox ID only, and `delete_orphan` (`crates/farhelm-helm/src/tensorlake/orphans.rs:178-255`) reuses the retained row
  under whatever key is current, so no lookup runs. `sweep_orphan` (`:259-283`) treats `NotFound` from terminate and
  snapshot deletion as done, and lists only the new account's snapshots. The list returns retained cleanups whatever the
  key. SPEC.md (`SPEC.md:609-611`) says this state "lasts until cleanup succeeds or the helm restarts" and says nothing
  about which account it belongs to. The excluded consequence "success reported for an operation that failed" means no
  filter can apply.
- Decision: q-staleconf, "Confirmations and kept state whose subject is gone": the maintainer picked "Close or
  invalidate them". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A confirmation or kept state whose subject is gone (host removed, listing emptied, account key
  changed) closes or is invalidated. Recommendation for reference: fix code — tie retained cleanups to the key that
  admitted them, and refuse a retry under another key with an explanation; size M. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution
  blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-tensorlake-sandboxes-inline-delete-prompt-survives-key-change.md

- Outcome: `fix code`.
- Assessment: In Settings → Tensorlake, if the inline Delete confirmation for a leftover sandbox is open and the user
  clears or replaces the API key, the confirmation stays open. After Clear it keeps the leftover-sandboxes section on
  screen, and after a replacement it lets the user confirm deleting the old account's sandbox under the new key. That
  request normally fails as not found, unless it hits the false-success route in item 546. Verified: confirmed.
  `Sandboxes::follow_account` (`crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:153-175`) clears lists on
  Clear but never `question`, `asked` or `delete_failure`, and `drawn` (`:374-381`) keeps the section drawn while the
  question is open. The rule that bears on it is SPEC.md "Keyboard focus" (`SPEC.md:3853-3855`, a confirmation goes away
  "when the thing it is about is gone"). Whether a sandbox in the previous account is "gone" after a key change is a
  reading.
- Decision: q-staleconf, "Confirmations and kept state whose subject is gone": the maintainer picked "Close or
  invalidate them". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A confirmation or kept state whose subject is gone (host removed, listing emptied, account key
  changed) closes or is invalidated. Recommendation for reference: fix code — drop the question and the delete failure
  whenever the saved key changes; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## grok-manual-hook-replacement-omitted.md

- Outcome: `fix spec`.
- Assessment: Up to v0.43.0, the Grok guide told users to hand-create `~/.grok/hooks/farhelm.json` with three hook
  entries. Automatic setup, from v0.44.0, overwrites that whole file when it differs. The setup prompt and the guide say
  Farhelm "changes" or "installs or updates" Grok's configuration. Neither says that this file is replaced and any edits
  in it are lost. Verified: partly correct. Whole-file replacement is real (`agent_kind/persistent_hooks.rs:40-58`) and
  is spec'd (SPEC_impl.md around 4678: "replaces each differing file atomically"). The consent text is
  `crates/gui/farhelm-gui/src/hook_setup.rs:99-105`. No loss of meaningful user edits is established. The old guide
  asked only for the three Farhelm entries with the user's `farhelm` path, and it said to keep other hooks in other
  files ("leave your other hook files alone").
- Decision: q-hookwords, "Hook setup naming the exact files it replaces or tells users to delete": the maintainer picked
  "accept and fix spec (given in the comment; no picker choice)". (2026-11-24 fewest-questions triage page (main
  c38b93552).) Maintainer's comment: "accept and fix spec"
- Completion criteria: Accept the behavior and say so in the spec: Grok's Farhelm hook file and Antigravity's Farhelm
  plugin folder are Farhelm's own, so setup may replace that file whole and removal may delete that folder. No code
  change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## antigravity-hook-removal-can-delete-extra-files.md

- Outcome: `fix spec`.
- Assessment: When the user turns off Antigravity hook setup, Settings and the docs say to delete the whole
  `~/.gemini/config/plugins/farhelm/` folder. Farhelm writes only `plugin.json` and `hooks.json` there. Anything else in
  that folder would be deleted too. Verified: confirmed for the wording (`hook_setup.rs:113-114`;
  `website/src/content/docs/docs/agents/antigravity.md`, "Set up Restart": "delete that plugin directory"). Setup writes
  two files into a directory it does not check for other contents (`agent_kind/antigravity.rs:17-50`). Nobody has shown
  that anything other than Farhelm puts files in a directory named `farhelm`. SPEC.md (around 4134) says "The Grok and
  Antigravity opt-in covers the files named for Farhelm."
- Decision: q-hookwords, "Hook setup naming the exact files it replaces or tells users to delete": the maintainer picked
  "accept and fix spec (given in the comment; no picker choice)". (2026-11-24 fewest-questions triage page (main
  c38b93552).) Maintainer's comment: "accept and fix spec"
- Completion criteria: Accept the behavior and say so in the spec: Grok's Farhelm hook file and Antigravity's Farhelm
  plugin folder are Farhelm's own, so setup may replace that file whole and removal may delete that folder. No code
  change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-implicit-home-spare-ownership.md

- Outcome: `fix spec`.
- Assessment: A sandbox or container session that uses its home folder as its working folder (with no checkout folder
  configured, Farhelm falls back to the home) can later be replaced with a managed checkout. That first checkout uses
  "Keep a checkout ready" and claims whatever sits at the reserved hidden spare folder
  `~/.farhelm-spares/<owner>/<repo>` in that home. A later supervisor start can then delete it. The reviewer's worry is
  files the user had put at that path before the home first served as a checkout root. Verified: Partly correct. The
  fallback is real: `crates/farhelm-helm/src/sessions.rs:228-244` sets the root to `~` for session-owned hosts only.
  `spare_take.rs` has not changed since the reviewed commit. Two things are unproven: any way for user content to end up
  at `.farhelm-spares/<owner>/<repo>` in a Farhelm-created environment's home, and the declined-claim path and final
  removal check, which the reviewer's own drop check said its excerpts left out. SPEC.md "Supported user environments"
  (6030-6038) says Farhelm owns "their checkout roots and ... reserved spare container (`.farhelm-spares`)" and that
  "anything in that container that Farhelm did not put there ... is out of band, and Farhelm may adopt it and delete
  it". It does not say whether contents placed before the home became a checkout root count as out of band.
- Decision: q-spare, "Who owns the reserved spare folder in a sandbox home": the maintainer picked "Farhelm owns it from
  the start". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Add one SPEC.md sentence: the reserved spare container belongs to Farhelm from the moment its
  folder serves as a checkout root (the sandbox home fallback included), whatever it already holds. No code change.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-antigravity-omitted-from-approval-cwd-pinning.md

- Outcome: `fix spec+code`.
- Assessment: An agent can ask Farhelm to start an Antigravity session with automatic workspace trust in a folder given
  through a symlink. The approval card shows the symlink path, and the target host resolves it only when Antigravity
  starts. Codex, Claude, Muse and Pi pin the real folder before the card. For Antigravity, an existing symlink can hide
  which folder gets trusted, and a symlink repointed while the card waits sends the trusted launch somewhere else.
  Verified: confirmed. `farhelm-helm/src/launches.rs:814-832` still returns `false` for Antigravity. Its comment
  ("Antigravity already resolves its directory in the supervisor") does not address the approval wait, and Codex
  resolves its folder at the same point (`agent_kind/mod.rs:2438-2460`) yet is pinned. SPEC_impl.md around 3677-3681
  lists only "Claude, Codex, Muse or Pi" for pinning, "so retargeting the original alias cannot redirect the launch
  during the wait for Allow". Around 5387-5389 it says only "The supervisor resolves the directory when it fills the
  slot." The omission is documented, but no text accepts redirecting the trust.
- Decision: q-agtrust, "Antigravity left out of approval-card folder pinning": the maintainer picked "Pin Antigravity
  too". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Pin Antigravity's folder before the approval card like Claude, Codex, Muse and Pi, and add it to
  the SPEC_impl list. Recommendation for reference: fix spec+code. Change the one match arm, add Antigravity to the
  SPEC_impl list, and extend the two helm tests. Size S-M. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## trailing-unstamped-host-skips-legacy-claude-settings-cleanup.md

- Outcome: `fix spec`.
- Assessment: Older Farhelm releases wrote a Claude endpoint URL into `~/.claude/settings.json` on a host and then
  recorded locally that it had done so. If the helm crashed or lost the reply between those two steps and Farhelm was
  then upgraded before that host reconnected, the new release thinks the host was never touched, skips cleaning the old
  URL, and Claude may send the newly delivered token to that old endpoint. Verified: confirmed by source. Current
  `retire_legacy_settings` cleans Claude settings only when a stamp exists
  (`provisioning/credential_delivery.rs:1116-1120`) and otherwise records Cleared (`:1145`). The pre-#4593 code wrote
  the file and only then set `record.claude` (`git show 67d8541dc^`, credential_delivery.rs ~600-713), so the gap
  exists. A reconnect under the old release would have re-written and stamped, so the route needs crash-then-upgrade
  with no successful reconnect between. SPEC_impl.md "Named credential catalog" (6219-6220): "An old Claude settings URL
  can otherwise take precedence and receive the new token. ... A host with no legacy stamp records Cleared directly,
  preserving its image settings." That accepts the unstamped branch for image settings but does not address an
  unrecorded Farhelm write, so not covered-spec (the independent drop check said the same). Not the upgrade filter: a
  token sent to a stale endpoint is a trust-boundary consequence, which that filter excludes.
- Decision: q-legacycreds, "A pre-upgrade crash leaving an old Claude endpoint uncleaned": the maintainer picked "Accept
  it in the spec". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: State in SPEC_impl.md "Named credential catalog" that an unrecorded legacy Claude settings write
  by an older release is accepted. No code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-deleted-credential-stays-on-connected-host-after-lookup-failure.md

- Outcome: `fix spec`.
- Assessment: Hosts receive Claude and Codex launch credentials from Settings → Credentials through their recipe.
  Suppose the user takes a token out of the recipe and deletes it from Credentials while a host is connected, and then
  saves some other credential that host uses. The host keeps the deleted token in its launch credentials until it
  reconnects, so new sessions and tabs there still get it. The host shows an amber warning meanwhile. Before the
  reviewed change, that unrelated save happened to remove the deleted token as a side effect. Verified: Confirmed in
  code. credential_delivery.rs:985-997 returns early with `lookup_aborted` when either agent's lookup fails, and leaves
  the file and stamps untouched. A connection keeps the selections it read at connect time (SPEC.md:529-530).
  SPEC.md:571-573 says "If Farhelm cannot read either Claude's or Codex's recipe credential, the host keeps both agents'
  last delivered launch credentials". Whether a deleted credential counts as one Farhelm "cannot read" is a matter of
  reading the spec. The independent drop check said it does not clearly cover deletion. Without that unrelated save, the
  token stayed until reconnect both before and after the change. So the change only removed an incidental cleanup.
- Decision: q-creddelete, "What deleting a credential promises": the maintainer picked "Removal at next connection".
  (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "SPEC - removal is always
  best-effort-at-next-opportunity, not a revocation guarantee."
- Completion criteria: Add the principle to SPEC.md: removing a credential from Farhelm is always best-effort at the
  next opportunity (a connected host drops it at its next connection), never a revocation guarantee; revoking at the
  vendor is the response to a leak. No code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-signed-announcement-feed-replayable-across-files-and-versions.md

- Outcome: `fix spec`.
- Assessment: Released Mac apps check Farhelm's signed news feed at startup and every 12 hours. The brick test needs a
  test feed permanently published next to the real one and signed with the same key and purpose. The app cannot tell the
  two files apart, nor an old real feed from a current one. If the download server ever served the test file (or an old
  feed) at the real address, because of a publishing slip, a mirror or CDN error, or a compromise, every Mac app that
  checks while it is served would show "Farhelm brick test announcement" and keep it in its Announcements history for
  good. Verified: Confirmed. `crates/farhelm-helm/src/network_announcements.rs:22` has a single
  `PURPOSE = "farhelm announcements"` that does not depend on the filename. The test pair must use the same purpose and
  keys (`releasing/AGENTS.md`, the announcements paragraph: "The same purpose and keys apply to the standing
  `announcements-test.json`..."). The brick test probes a fixed path
  (`releasing/mac-vm-test/checkpoint_contract.py:51`). Enqueue is permanent by id (SPEC_impl.md, "Shared preferences and
  announcements"). Neither spec accepts the replay. SPEC.md:5133-5136 says the helm "verifies both the signature and its
  exact announcement purpose", which the shared purpose defeats for this pair. Unverified: how the standing entry is
  actually targeted and when it expires (it is published outside the repo), and any route by which the server would
  serve the wrong bytes.
- Decision: q-announce, "The brick-test announcement feed is signed like the real one": the maintainer picked "Accept
  the risk". (2026-11-24 fewest-questions triage page (main c38b93552).) Maintainer's comment: "SPEC_impl - this is
  intentional to minimize difference between brick test path and production path"
- Completion criteria: Record in SPEC_impl.md that the standing brick-test announcement feed deliberately shares the
  production feed's key and purpose, to minimize the difference between the brick-test path and the production path, and
  that the replay risk this leaves is accepted. No code change. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-tailscale-predictable-tmp-socket-dir-local-dos.md

- Outcome: `fix code`.
- Assessment: For Tailscale hosts, Farhelm keeps its shared SSH connections in a folder under `/tmp` whose name another
  account on the same computer can predict. That account can create the folder first. Farhelm then refuses to connect
  to, sign in to or set up that Tailscale host until reboot. Verified: Confirmed by source. `ssh/tailscale.rs:158-199`:
  the name is a digest of uid, canonical state directory and destination, and a pre-existing foreign entry fails
  `require_private_control_directory` with no fallback. Confidentiality and integrity are preserved; only availability
  is lost. No spec text accepts or rejects hostile local accounts on the helm machine for this. Analogues: SPEC.md 1884
  and 5905 say Farhelm is not designed for working-copy roots or install directories that other accounts can write to,
  and 5160 accepts a narrow loopback-port gap against other local users.
- Decision: q-localdos, "Another local account blocking a Tailscale host": the maintainer picked "Fix it". (2026-11-24
  fewest-questions triage page (main c38b93552).) Maintainer's comment: "but why "add a salt", can we just use a proper
  operating system allocated (securly) temp name please"
- Completion criteria: Replace the predictable /tmp folder name with an operating-system-allocated secure temporary
  directory (mkdtemp-style), not a salted digest. If a stable name across helm restarts turns out to be required, report
  that back to the maintainer before choosing another shape. Recommendation for reference: fix code S. Add a random
  per-helm salt stored in the state directory to the digest, or use a private runtime directory. Alternatively fix spec
  to declare it out of scope. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

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

## wayland-ssh-paste-blocks-window.md

- Outcome: `fix code`.
- Assessment: On Linux (Wayland), pasting into a text field such as an SSH sign-in answer reads the clipboard on the
  window's own thread. If the app that owns the clipboard is slow to supply the text, the whole Farhelm window freezes
  for up to about four seconds per read. Verified: Confirmed by source, not observed. gpui-base 0.9.1 `paste` calls the
  synchronous `read_from_clipboard` first (`~/.cargo/registry/.../gpui-base-0.9.1/src/input/base/state.rs:3217-3224`).
  The vendored Wayland clipboard's sync path blocks in `read_fd_with_timeout(fd, PIPE_READ_TIMEOUT)` with a 4 s timeout
  (`vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs:349-366`, `platform.rs:1329`). SPEC.md (terminal clipboard
  rules, around 3566-3571) allows only "A paste the user starts on macOS may stall the window" and says "Farhelm must
  stay responsive during other clipboard operations." The fix shape is a choice: intercept Paste in `TextInput` on Linux
  with an async read plus a focus check, or patch Kit.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Route
  text-field paste through the async read on Linux. Size S-M. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-failed-rebuild-on-removed-host-blocks-recipe-delete.md

- Outcome: `fix code`.
- Assessment: If a Rebuild or a first-launch preparation of a recipe fails on a container host that holds none of that
  recipe's images, and that host is later removed from Farhelm, the recipe can never be deleted again. Every Delete
  fails with "the container place was removed before its state could be recorded". Re-adding the host does not help,
  because it gets a new ID. Verified: Confirmed by reading. `prepare_images` (`build.rs:273-327`) calls `edit` before
  `places.fresh`, and `edit_at` (`:579`) creates a per-place entry with `or_default()`, so a failure leaves an entry
  that holds only the failure. `needs_cleanup` (`recipes.rs:80-88`) is false for it, so host removal is allowed
  (`store/podman.rs:337-372`). `clean_images` (`build.rs:914-960`) calls `self.edit` for every entry, and `edit`
  (`:548-561`) fails when the host row is gone. `delete_recipe_owned` (`tensorlake/recipes.rs:689-698`) then refuses.
  `hosts.id` is AUTOINCREMENT, so the ID is never reused. No spec sentence requires that Delete always succeed, so this
  is not clearly settled, but the fix needs no design choice.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — in
  `clean_images`, drop entries with `!needs_cleanup()` without calling `edit`, and do not create an entry before
  admission. Size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-refused-setup-confirmation-persists-planned-paths-on-retargeted-host.md

- Outcome: `fix code`.
- Assessment: When you confirm setup on an already-listed SSH host, Farhelm saves the plan's install paths on that
  host's record before it takes the host's lock. If the destination or Tailscale mode changes in that moment, the setup
  is refused, but the planned paths stay saved on the record. Verified: Confirmed by source.
  `provisioning/service.rs:1587-1592` registers before `launch_claimed_run` takes `host_provision_lock` (2550-2568). The
  comment there ("An ADD can have saved its paths before a retarget won the provisioning lock") shows the gap is known;
  the check only compares the network target. The practical consequence is milder than the file says. An ordinary
  retarget already keeps the previous machine's paths (`update_ssh_destination`), and for a re-setup the plan's paths
  usually equal the stored ones. A Re-run of setup repairs it. The window is the time `register()` takes.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code M together
  with 397 (one lock-order change in `claim_register_and_start`), or discard if lock-order risk is unwelcome. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

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

## upgrade-highlights-can-open-after-sign-in.md

- Outcome: `fix code`.
- Assessment: After an upgrade, the Mac app fetches "What's new" from the website. If a startup sign-in prompt (for
  example a security-key touch notice) opens and closes by itself while that fetch is in flight, the highlights dialog
  still opens and takes focus. Verified: Confirmed. `crates/gui/farhelm-gui/src/whats_new.rs:73-79` checks only whether
  a layer is open when the answer arrives, and the user-input mark does not move when a layer opens.
  `sign_in.rs:172-184` closes the popover by itself when its question goes away. SPEC.md 902-903 says in so many words:
  "If the user starts working or another prompt takes focus before the release highlights arrive, the app leaves focus
  and the record alone." So the code contradicts the spec. It is not marked settled because the natural fix (a
  layer-open generation counter) changes the layer manager, which `docs/gpui/foundation-ownership.md` lists as a refused
  path with no extension points, and a fix confined to What's new or the sign-in prompt is also possible.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Refuse a Show
  answer when any focus layer opened during the lookup. Size S to M (about 40-80 lines including a fixture). Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## upgrade-highlights-opt-out-refusal-is-hidden.md

- Outcome: `fix spec+code`.
- Assessment: Pressing "Don't show again" in What's new closes the dialog right away. If the helm refuses to save the
  opt-out, the only trace is a log line: the user believes highlights are off, and the next upgrade shows them again.
  Verified: Confirmed. `whats_new.rs:86-104` logs the refusal and does nothing else, and the dialog has already closed.
  SPEC.md "Errors and diagnostics" (4944-4945): "Every failed operation surfaces a concrete, actionable error in the
  client. A dialog must never close as though an operation succeeded when it failed." Its best-effort exceptions list
  the welcome card and the announcement dismissals, but neither the highlights record nor its opt-out. The fix has more
  than one reasonable shape (a notice after closing, or holding the dialog open until the save settles), and a refused
  Got it is arguably like the welcome card's best-effort save.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix spec+code. Add Got
  it to the best-effort exceptions next to the welcome card, and show a notice when the opt-out is refused. Size S. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-sessions-ui-nov24.md` ``.

## launcher-escape-discards-template-edits.md

- Outcome: `fix code`.
- Assessment: In the wide session launcher, Save as template's name field and checklist sit inline next to the command
  field's search results. With the panel open and results showing, Escape closes the Save as template panel, and its
  typed name and checklist changes go with it. The results stay open, though the user probably meant to hide them. In
  the narrow layout the results are a popover and take Escape first. Verified: confirmed. The launcher's dismissal hook
  closes the save panel before it offers Escape to the inline results (`launcher/view.rs:228-246`,
  `dismiss_command_pane` in `launcher/view/search.rs:618-640`). SPEC.md:1504 ("Escape or clicking outside the results
  hides them and keeps the query") and SPEC.md:1322 ("Escape returns to the launcher", for the panel) both claim this
  Escape. The panel-first order is a deliberate, commented choice: "closes the panel on Escape anywhere in the launcher,
  as the old panel took the launcher's Escape first" (`templates/save_panel.rs:22-25`, `view.rs:228-229`). Only a short
  panel input is lost, which the user can recreate quickly.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — give open
  results the first Escape, as the narrow layout already does, and update the save_panel doc comment. Size S. A discard
  is also reasonable given how little is lost. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-incomplete-template-banner-misplaced.md

- Outcome: `fix code`.
- Assessment: When a template shortcut, the ⚡ quick launch, a sequence or a list pick leaves the launch incomplete, New
  opens with a banner explaining what is missing. The banner is drawn under the Launch and Cancel buttons and tells the
  user to "complete it below", but the form is above it. SPEC.md promises this banner "at the top". Verified:
  confirmed. - The banner text says "below" (`templates/dispatch.rs:370-394`). - The banner is part of the notices
  region (`launcher/view.rs:2767-2780`), and that region is drawn under the actions row (`view.rs:2885-2887`,
  `components/dialog.rs:311-319`). - SPEC.md:1404-1406 says "a prominent alert banner at the top explaining that it
  cannot launch on its own". SPEC_impl.md:9222 says "the banner at the top of its notices region", which matches the
  code's placement but not SPEC.md's apparent meaning. Either the "below" wording or the placement is wrong in every
  layout. - Fixing it means choosing between rewording and moving the banner.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — reword
  "below" to "above", or move the banner into the top band (the escalation banner already uses it, `view.rs:2890-2892`).
  Fix spec as well if the footer placement is kept. Size S. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## launcher-template-row-edits-behind-save-panel.md

- Outcome: `fix code`.
- Assessment: While Save as template is open in the launcher, clicking a template row in the wide list, or taking a
  `tl:` result, still applies that template to the form behind the panel. The panel keeps showing and saving the values
  it captured, so the form and the panel disagree. A shortcut press in the same state is ignored. Verified: confirmed.
  `take_search_row` (`launcher/view/search.rs:350-374`) has no save-panel check, while the shortcut path refuses with
  the panel open (`templates/dispatch.rs:464-468`). SPEC_impl.md:9176-9183 says "A taken `tl:` row in the composer
  search is one such batch ... While Save as template's panel is open a press changes nothing". Whether "a press" there
  covers row picks or only shortcut presses is a reading, and SPEC.md:1363-1364 speaks only of the chord. Hand edits of
  the form are allowed while the panel is open, and the panel saves its displayed snapshot, so nothing wrong is saved.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — refuse the
  row before its search and focus side effects, and at batch admission, matching SPEC_impl's "a press changes nothing".
  Size S. A discard is defensible because nothing wrong is saved. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## hook-hint-settings-focus-return-mismatch.md

- Outcome: `fix spec`.
- Assessment: Opening Settings from a Grok or Antigravity hook hint in a session and then closing it returns keyboard
  focus to the hint, while the spec says closing Settings returns focus to the gear. The trust hint already behaves this
  way. Verified: confirmed. `open_at_hook_setup` and `open_at_agent_trust` pass the hint as the return target
  (`settings.rs:315-347` via `open_dialog`, `:261`). SPEC.md 2576-2577: "When Settings closes, focus returns to the
  gear." The trust hint's identical, older behavior looks deliberate, so whether the spec or the code is wrong is a
  decision.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix spec — say focus
  returns to whatever opened Settings; size S. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## launcher-loses-hook-offer-on-tab-switch.md

- Outcome: `fix code`.
- Assessment: In the new-session launcher, explicitly picking Grok or Antigravity brings up a one-time offer to set up
  conversation hooks. Switch to the Command tab (with no agent declared) and back before answering, and the offer is
  gone for good. Grok stays selected and launches without ever offering setup. The offer comes back in the next
  launcher, and the session hint is unaffected. Verified: confirmed, and slightly broader than the report. This happens
  whether the read is still out or the question is already shown. The tab handler calls only `changed`
  (`launcher/view.rs:2641-2645`). That runs `retire_hook_answer` (`launcher/view.rs:1002`), which drops the question
  when the active hook agent differs (`launcher/view/hook_question.rs:96-105`). A read that settles later is ignored
  because its pick is gone (:66-72). Coming back to the Agent tab starts no new check. `check_hook_question` runs only
  from harness, command-agent and model picks (`choices.rs:109`, `view.rs:2169`, `search.rs:535,559`). SPEC.md (~:3180)
  says "An explicit selection of Grok or Antigravity in the launcher offers …"; it is silent about tab round-trips.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Re-check when
  the tab change restores an agent whose offer was never answered. Size S. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## resume-off-command-gets-hook-warning.md

- Outcome: `fix code`.
- Assessment: A Command-tab launch that declares Grok or Antigravity, with Resume left off and hook setup on, gets hooks
  installed and the silent-report clock armed. If the agent has not reported a minute after the first Enter, the session
  gets a warning: "Until it does, Restart cannot resume this conversation". Restart can never resume this launch,
  because it has no resume command. If the report does arrive, the warning never resolves. Verified: confirmed. The helm
  requests setup by agent kind only (`crates/farhelm-helm/src/sessions.rs:2199-2202`). A successful install marks the
  launch as hooked regardless of its resume template (`crates/farhelm-supervisor/src/service/core.rs:14810-14814`).
  Neither the tripwire (`service/capture.rs:355-385`) nor `notify_hook_silent` (:135-165) checks for a resume command.
  The text is at `service/notifications.rs:247-264`. The trigger needs the vendor's report to be missing, which nobody
  has observed. The non-first-class harness filter is still not clearly met. The missing resume-command check is generic
  and looks like it applies to any hooked command launch with Resume off, Claude's `{farhelm_args}` launches included.
  That Claude path was not traced to the end.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Skip the
  warning, or reword it, when the launch has no resume command. Size S. Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings
  the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## model-picker-skips-hook-setup-offer.md

- Outcome: `fix code`.
- Assessment: In the launcher's Model field, choosing an Antigravity model from "Show all" switches the agent to
  Antigravity but never shows the one-time hook setup offer. Choosing the same model from the launcher's search does
  show it. Verified: confirmed. `launcher/view/model_field.rs:225-236` (`pick_model`, Model row) calls only `chose`. The
  search row does the equivalent edit and then calls `check_hook_question` when the agent changed
  (`launcher/view/search.rs:516-537`). SPEC.md (~~:1153) says "Choosing another harness's model from the full model list
  switches the harness on purpose", and (~~:3180) "An explicit selection of Grok or Antigravity … offers". Treating "on
  purpose" as "explicit selection" is a reading, not the spec's words, so this is not clearly-settled. Grok has no
  catalog models, so this applies to Antigravity only.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Mirror the
  search row's `picked_agent` check in `pick_model`. Size S. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## checkout-input-arms-hook-warning-early.md

- Outcome: `fix code`.
- Assessment: With Grok or Antigravity hook setup on, a fresh managed checkout runs its clone and post-clone command in
  the session's terminal before the agent starts. If the user presses Enter there (answering a prompt from the
  preparation) and preparation then runs for more than about a minute, Farhelm warns that the agent never reported its
  conversation. The agent has not even started yet. The warning is one per launch, so it is used up for that launch.
  Verified: confirmed by source, conditional on qualifying input during preparation. Setup arms the hooked flag before
  the terminal starts (`service/core.rs:14810-14814`). The clock starts on any accepted Enter outside a recognized
  dialog (`service/capture.rs:180-190`, `note_first_input`), with no check that preparation has finished. The tripwire
  checks only elapsed time, the flag and missing capture (:355-385). SPEC.md "Status" (~:3221-3226) starts the clock "a
  minute after the first Enter the user sent it". Whether Enter during preparation was "sent" to the launch is a matter
  of reading the spec. The same mechanism likely applies to every hooked agent in a managed checkout, so the
  non-first-class harness filter does not clearly apply. Not verified for Claude or Codex.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Start the
  clock only once the shim has handed off to the agent. Size M (shim-to-supervisor phase signal). Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-hooks-trust-nov24.md` ``.

## trailing-full-podman-inspection-strand-container-farhelm-needs-to-remove.md

- Outcome: `fix code`.
- Assessment: Before it deletes a container or helper, Farhelm reads Podman's full description of it. If that
  description is over 64 KiB (an image with very many environment variables, labels or mounts), the read fails and
  Farhelm can never delete that container itself. The user has to remove it by hand. Verified: Confirmed by reading.
  `inspect` (`podman.rs:303-350`) runs a plain `container inspect` and uses only ID, name, state and labels. Output goes
  through `capture_child`, which caps a stream at `MAX_CHILD_STREAM_BYTES = 64 KiB` (`provisioning/backend.rs:106`) and
  fails over the cap. Not established: real images that produce over 64 KiB. Typical output is a few to about 20 KB.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — use
  `--format` to project only the needed fields. Size S. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-locally-invalid-checkout-names-start-recipe-environment-before-refusal.md

- Outcome: `fix code`.
- Assessment: When the user launches a new sandbox or container with a managed checkout whose typed name cannot become a
  folder name (no letters or digits, or too long), Farhelm creates the environment first (a sandbox bills briefly), then
  the new Farhelm refuses the name, the environment is cleaned up, and the launch shows as failed. The name could have
  been rejected before anything was created. Verified: Confirmed from the reviewer's trace. I checked that nothing in
  `sessions.rs` or the launcher calls `checkout_basename` before provisioning (it is used only in `farhelm-proto` and on
  the supervisor). SPEC.md (about line 1906) says an empty slug or a component over 200 bytes "is refused" but not when.
  No later commit added an early check.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — validate the
  title and basename with `checkout_basename` before provider work. Size S-M. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-podman-nov24.md` ``.

## trailing-template-summary-misstates-unsupported-permissions.md

- Outcome: `fix code`.
- Assessment: When an agent proposes a template whose approval mode the chosen agent does not offer, the approval card
  and the Templates list show a different mode from the one the template stores. For example, Approve on Claude reads as
  Claude's guarded mode, and Custom on Pi reads as YOLO. The user approves a description that is not what gets saved.
  Launching that template would be refused anyway. Verified: confirmed. - `permission_label`
  (`templates/summary.rs:177-199`) resolves explicit values through `request_permission`, which swaps any unoffered
  value for the agent's default (`farhelm-proto/src/launch.rs:443-451`). - Agent template writes never check whether the
  agent offers the mode: the CLI `fields()` at `main.rs:649-664` does not, and only template application checks it
  (`farhelm-proto/src/launcher.rs:1059-1066`). So the route goes through the shipped CLI, for example
  `farhelm agent template create --agent claude --permissions approve`. - SPEC.md:4648-4649 says "Template cards show
  the permission field when present, resolving a reset to the agent's default when the template names one." That is
  close to clearly-settled. - I classed it needs-decision because the function's doc comment deliberately says it "names
  what the launch would request" (`summary.rs:172-175`), and because the fix has more than one shape: show "Approve" as
  it is, or "Approve (not offered)" as the editor does (`rows.rs:365`). - It cannot make a YOLO or Custom template read
  as safer. Every non-shell agent offers YOLO, and Custom falls back to YOLO.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — resolve only
  a reset (None) and show explicit values as stored, ideally with the editor's "(not offered)" suffix. Size S. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-launcher-nov24.md` ``.

## trailing-tensorlake-setup-rerun-omits-suspended-and-sealed-refusal.md

- Outcome: `fix code`.
- Assessment: Re-running setup on a suspended Tensorlake sandbox is not refused up front with "resume it first". Farhelm
  shows the setup confirmation, briefly replaces the Suspended status, then fails the run. A sandbox already being
  deleted likewise gets a confirmation that is only refused when confirmed. Verified: Confirmed.
  `provisioning/tensorlake_add.rs:745-777` (the `TensorlakeHost` arm) checks busy and owner but not `suspended_refusal`
  or `is_sealed`. The sealed check exists only at confirmation (`tensorlake_add.rs:984`). Suspended half: SPEC_impl.md
  (around 7096-7098) says "Update and setting a host up again are refused for a host the manager shows suspended, before
  anything is probed and again before a confirmed run's preflight probe, with a resume-first message". The code
  contradicts it in so many words, and the fix (call `self.suspended_refusal(&row)` in the probe arm and in
  `start_tensorlake_add`) follows directly, XS. Sealed half: no spec text requires a probe-time refusal.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code XS for both
  halves. The orchestrator may record the suspended half as fix code without asking and present only the sealed half.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-ssh-setup-existing-tensorlake-mutates-before-lock.md

- Outcome: `fix code`.
- Assessment: Confirming a setup plan for an already-listed Tensorlake sandbox reached by its SSH address can save new
  install paths and restart its connection before Farhelm takes the host's lock and checks that it is not suspended.
  This only happens when the planned paths differ from the stored ones. A sandbox that became suspended meanwhile then
  gets dialed anyway. Verified: Confirmed by source. It is the same register-before-lock path as 88
  (`provisioning/service.rs:1587-1592` before `launch_claimed_run`'s lock at 2550). Replacing the actor resets Suspended
  to Connecting, so `require_not_suspended` in revalidation would pass. The spec text in 395's Assessment ("refused …
  again before a confirmed run's preflight probe") applies. Whether the GUI ever sends an SSH-destination setup for an
  existing finished Tensorlake host with differing paths is unverified; the probe refuses unfinished sandboxes
  (`service.rs:917-926`).
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code M together
  with 88. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-provisioning-service-omits-sealed-host-refusal.md

- Outcome: `fix code`.
- Assessment: After you delete a sandbox's or container's last session, Farhelm seals the environment and destroys it.
  If that destruction fails, the host shows as being deleted with a Retry. During that time the helm still accepts an
  Update, or a setup probe aimed at that machine, and would install Farhelm onto a machine the user chose to delete. The
  desktop does not offer Update or Re-run for such a host, so only a direct API call or a click on a menu that was
  already open could get there. Verified: confirmed in code, with a narrow route.
  `ProvisioningService::suspended_refusal` (`crates/farhelm-helm/src/provisioning/service.rs:3151-3172`) refuses
  Stopped/Suspended/TensorlakeChanging. Its `_ => None` arm lets through `HostState::Deleting`, which the manager
  reports for sealed hosts (`manager.rs:2426-2440`, `sealed_state` at 2927-2938). Probe (`service.rs:974`), Update
  (`1725`) and revalidate (`2838`, `2973`) all go through it. The only `is_sealed` check in provisioning is Tensorlake
  Add (`tensorlake_add.rs:984`). The GUI hides Update and Re-run for any `Deleting` host
  (`crates/gui/farhelm-gui/src/hosts/offers.rs:534-555`, `leaving`). The manager's own doc (`manager.rs:2941-2950`)
  accepts that "an API client can still send" Suspend/Resume for such a host. Unverified: whether an Update admitted
  here actually reaches the install step on a sealed sandbox, since the connection client is withheld while sealed.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — one
  `Deleting` arm in `suspended_refusal` (or an `is_sealed` check beside it) plus a test; size S. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-plain-replace-and-restart-do-not-disable-suspend.md

- Outcome: `fix spec`.
- Assessment: While a session launched from the launcher (including Restart with… and Replace with…) is starting on a
  sandbox or container, that host's Suspend and Resume buttons are disabled. A plain Restart or Replace from the
  session's menu does not disable them, so the user can suspend the host while the restart is in flight. The restart
  request then fails or times out until the host is resumed. Verified: confirmed as described, but it rests on how the
  spec is read. `host_lifecycle_refusal`/`targets_host` (`crates/gui/farhelm-gui/src/launcher/launch.rs:233-255`) only
  checks the launcher's held slot. SPEC.md "Creation" (`SPEC.md:1522-1527`) says Suspend and Resume "are refused only
  while this window's own launch targets or is waking that same host". That paragraph opens "After submitting a launch,
  including Restart with or Replace with", so on its face it excludes plain Restart and Replace. SPEC_impl and
  `Work::creates_a_session` count plain Replace and Restart as launches only for the host-registry exclusion. Whether
  "launch" here includes plain Replace and Restart is the open question.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix spec — state that
  only launcher submissions hold the host's Suspend/Resume, which suppresses this class; size S. (fix code is also S–M
  if the maintainer prefers the stronger guard.) Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-suspend-read-activity-still-uses-ordered-lane.md

- Outcome: `fix code`.
- Assessment: Before Suspend (from a session row, the session header or Hosts) or Stop and suspend, the desktop checks
  the host's session activity. That check waits in the same queue as ordered work on every host, so a slow command on an
  unrelated host can delay the Suspend question by up to 20 seconds. If the user clicks elsewhere meanwhile, the
  question is dropped. Verified: confirmed. `read_activity`
  (`crates/gui/farhelm-gui/src/session_list/environment.rs:497-511`) uses `operations::read`, which calls `send` with
  `Lane::Ordered` (`operations.rs:707-716`, `785-796`). `Work::Suspend`, `Resume` and `StopAndSuspend` are already
  `Unordered` (`operations.rs:270-272`). The read is not the channel's "session query" that SPEC_impl keeps ordered
  (`SPEC_impl.md:335-339`, which refers to the projection query). SPEC.md "Slow hosts" (`SPEC.md:5352-5355`) says "one
  slow host must not stop the helm serving the others", but that sentence is about the helm, not this GUI lane, so it
  does not settle the case outright.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code — an unordered
  read helper for `read_activity`, plus a lane test; size S. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item
  back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-environments-nov24.md` ``.

## trailing-clone-replace-with-prefills-guarded-for-unset-permissions.md

- Outcome: `fix spec`.
- Assessment: For an old or cached session whose permission was never recorded, or is unsupported, Clone and Replace
  with prefill Guarded. The session list, Restart with and plain Replace treat the same session as YOLO. Verified:
  confirmed. `launcher/draft.rs:647` uses `request_permission` in `seed_launch`, while `draft.rs:124` uses
  `stored_permission` for Restart with. SPEC.md 1573-1575 classifies a missing permission as YOLO "for confirmation" but
  does not say in so many words what Clone and Replace with prefill (1601-1602: "compile new choices"). Related: TODO.md
  "Plan decisions to revisit", "Revisit Clone and Replace with starting old no-permission sessions as YOLO" (not
  `Planned`).
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix spec. Document
  Guarded as the deliberate prefill, or align the code if the TODO revisit chooses YOLO. Size S. Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-text-nov24.md` ``.

## trailing-harness-upgrades-https-redirect-downgrade.md

- Outcome: `fix code`.
- Assessment: Before a drift run, the runner upgrades each agent CLI. For Goose and Cursor it can fall back to
  downloading the vendor's install script and running it with bash on the maintainer's machine. The download follows
  redirects from HTTPS to plain HTTP or FTP. An attacker on the network could only exploit this if the vendor's HTTPS
  address ever redirected to plain HTTP, and could then run code on the maintainer's workstation. Verified: Confirmed by
  source. `run_installer` uses plain `urllib.request.urlopen` and `bash install.sh`
  (`scripts/harness_upgrades.py:143-151`, URLs at lines 62 and 79). The stdlib follows `https`→`http`/`ftp` redirects.
  Not established: that either URL redirects through a non-HTTPS hop. Without that, an attacker would need to control
  the HTTPS endpoint itself, and then needs no downgrade. The "Harm whose route is not established in shipped code"
  filter would fit, but the unestablished consequence is potentially `highest` (remote code execution on the
  maintainer's workstation), so FILTER.md requires the independent drop check before dropping. It is not dropped here.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix code. Refuse
  non-HTTPS redirects and check `response.geturl()` before writing the script. The documented `curl … | bash` for Cursor
  follows no redirects at all. Size S (about 15 lines plus a stand-in test). Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-tooling-nov24.md` ``.

## trailing-launcher-choose-effort-omits-sync-fields.md

- Outcome: `fix code`.
- Assessment: Choosing "Effort: Default" from search for a custom agent does not resync the launcher's custom effort
  text box. Verified: partly correct. `launcher/view/choices.rs:112-116` indeed skips `sync_fields`. But
  `LaunchDraft::choose_effort` (`launcher/draft.rs:894-896`) does not clear `custom_effort` today, so the box still
  matches the draft. The real bug is the sibling queue item
  `trailing-launcher-search-effort-default-misses-custom-effort.md`. This desync appears only once that item is fixed.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: other. Fold into the
  sibling item's fix (`sync_fields` alongside clearing `custom_effort`) and remove this item with it. Size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-custom-agents-nov24.md` ``.

## trailing-codex-login-save-reports-later-github-failure.md

- Outcome: `fix spec+code`.
- Assessment: Saving a new Codex login can install it on the host and still tell the user "Saved; not delivered to
  <host>", giving a GitHub token problem as the reason. The user may then retry or troubleshoot a delivery that worked.
  Verified: Confirmed. `credential_delivery.rs:719-723` reports any hard failure in the pass, and the Codex login is
  written before the GitHub step. The behavior is deliberate: the test
  `codex_login_save_filters_an_unrelated_launch_ lookup_warning` (`credential_delivery_tests.rs:1449-1453`) calls it
  "the existing pass error contract". SPEC.md 536 ("reports `Saved; not delivered to <host>: <reason>` for failures")
  does not say which failures.
- Decision: q-listed, "Everything else: approve each item's recommendation": the maintainer picked "Approve as listed".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Apply the item's recommendation as written. Recommendation for reference: fix spec+code. Say in
  SPEC.md that a save reports only failures of its own slot, and record which slot failed. Size M. It touches the same
  code as 419; doing them together is natural. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-helm-nov24.md` ``.

## trailing-delete-skips-hangup-for-live-tab-it-knows-only.md

- Outcome: `fix code`.
- Assessment: When you delete an agent session that has terminal tabs open, the tab shells should get a hangup so Bash
  or Zsh can save their history. On hosts without systemd scopes, Delete hangs up only the processes whose Farhelm
  marker it can read. macOS 26+ hides that marker for Apple's own binaries, and that includes the default `/bin/zsh`. So
  deleting an agent session with zsh tabs open on a current Mac can kill those shells outright, without the hangup.
  Verified: Confirmed in source, and the route may be more ordinary than the reviewer said. With a non-shell main
  launch, Delete's whole-session sweep hangs up only marker-carrying tab processes (sweep.rs:1198-1206, from
  `snapshot_proc` :484-495). Pinned tab roots are swept but never hung up. Tab hosts are ended only after the sweep
  (teardown.rs:530-560). The tab helper execs the user's shell directly (launch.rs:1307-1325), and
  SPEC_impl.md:4993-4995 records that macOS 26+ withholds the environment of platform binaries. SPEC_impl.md:4878 says
  "The sweep sends SIGTERM, with SIGHUP as well for tab shells", and SPEC.md:2151-2152 promises the hangup for Delete.
  Stop's tab closes are unaffected: the tab target hangs up everything (:1199). On Linux with scopes, the tab unit gets
  a scope SIGHUP (:1898). Not verified: that interactive zsh ignores SIGTERM (bash does), and that the history is
  actually lost on a real Mac. The fix has more than one shape (hang up only the pinned roots, or their whole subtree),
  so it is not clearly settled.
- Decision: q-hangup, "Shell history hangup on degraded cleanup paths": the maintainer picked "Fix only the Mac tab
  case". (2026-11-24 fewest-questions triage page (main c38b93552), second pass.) The history-saving hangup is a
  courtesy on rare degraded cleanup paths; the macOS 26+ tab case is fixed because it may happen on every such Delete.
- Completion criteria: Add the tab shells Farhelm already tracks (pinned tab roots) to Delete's hangup set, with a unit
  test of a marker-invisible pinned tab. Recommendation for reference: fix code — add the pinned tab roots to Delete's
  hangup set, with a unit test of a marker-invisible pinned tab; size S–M. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-supervisor-nov24.md` ``.
