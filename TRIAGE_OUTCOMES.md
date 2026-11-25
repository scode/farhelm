# Triage outcomes

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
