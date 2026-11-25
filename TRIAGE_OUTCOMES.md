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
