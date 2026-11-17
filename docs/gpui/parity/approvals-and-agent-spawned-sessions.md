# Parity: Approvals and agent-spawned sessions

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Agent-spawned sessions, Agents and environments, One GUI at a time.

The end-to-end steps named below are `scripts/gui-e2e-smoke.py`'s; steps 60 and 62 to 64 ran in a full smoke on the test
Mac on 2026-11-16. The running app's card is the `app-approval-card` scene in `docs/gpui/screenshot-scenes.json`,
compared with the old app's pictures at wide, narrow and minimum sizes.

### `tests/approval-layout.spec.ts`

Tests: 28. Covers: explicit host add preserves edited fields and waits for its own answer receipt.

replaced, apart from the deviations below: what every card kind shows and offers, links (the whole URL as data, which
answers each kind offers), the running prompt, environment launch, host edit and host remove cards, the From line's
isolated identity and raw Copy ID (`approvals::model_tests`); the expanded card's drawing, its notes and its alert
(`approvals::card_tests`, goldens `approval-card` and `approval-card-narrow`); the answers, the 700 ms pause after every
change (so a double click cannot answer the next card), the first answer winning and the late notice
(`approvals::answers_tests`); placement below the session's tabs, the cap and the whole-window fallback, the
oldest-first expanded card and the waiting headers, one selection kept across changes, arrival taking no focus with
Enter answering nothing, answering over an open dialog, the late notice, every kind keeping its heading and answers
inside the region in 960, 650 and 320 pixel panes with long texts at their minima, and a long queue scrolling in its own
list below the card and the notice (`approvals::region_tests`); the card staying answerable over the quick switcher and
its focus outliving a closing dialog (`layers_tests`, and end-to-end step 63 for the first); an agent's host add through
the Add host dialog: prefilled and labelled, opening is not approval, Allow before any probe with the edited
destination, only the helm's 204 accepted, a refused, unconfirmed or late Allow probing nothing, Deny on Cancel and
Escape, the setup confirmation always asked (`approvals::host_add_tests`, golden `host-add-dialog-agent`, end-to-end
step 64); Tunnel and open in the Mac app, its warning opening over another dialog and staying when that dialog closes
(`approvals::tunnel_tests`).

- The card over other dialogs, by `layers_tests`:
  `a_press_on_an_approval_card_answers_it_and_leaves_the_quick_switcher_open` ("approval remains answerable while the
  quick switcher is open", with `region_tests::a_card_is_answerable_over_an_open_dialog`),
  `an_approval_card_stays_usable_over_a_dialog` (a press and Enter on Allow answer it over an ordinary dialog) and
  `focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card` (focus put on Allow without answering
  outlives the dialog closing, nothing is answered meanwhile, and Enter then answers once).
- "host add preserves a manual draft and defers only an explicit open", "explicit host add waits behind the quick
  switcher" and "explicit host add opens stay ordered and omit expired queued requests" are the host add deviation
  below: the GPUI app refuses the open with a reason where the old app queued it. `approvals::host_add_tests`
  `another_dialog_or_the_pause_keeps_the_dialog_from_opening` (another dialog open: the open is refused and that dialog
  stays) and `an_answer_or_a_host_change_in_flight_keeps_the_dialog_from_opening` pin the refusal. With no queue, there
  is no order or expiry to test.
- The first case's draft: `approvals::host_add_tests::another_dialog_or_the_pause_keeps_the_dialog_from_opening` (a
  destination typed into the ordinary dialog survives the refused open) and
  `approvals::region_tests::an_agents_host_add_arriving_leaves_a_typed_add_host_draft_alone` (a typed destination and
  alias survive an agent's add arriving, its card drawn and focus left in the alias).
- "host add answer <status> starts zero probes", for 200, 409 and 410:
  `approvals::host_add_tests::a_refused_unconfirmed_or_late_allow_probes_nothing`, which answers with each of the three
  in turn. What a 410 does to the dialog is a deviation below.
- "a link denial opens nothing" and "a preapproved link dismiss opens nothing":
  `approvals::model_tests::link_cards_offer_the_answers_their_kind_and_platform_allow` (a preapproved link's Open and
  Dismiss are its Allow and Deny). That the window opens no link on either is the supersession below.
- "placement follows the empty pane, session mount, and approval remount": `region_tests`
  `the_region_is_centered_below_the_terminal_area_and_capped` (the placement arithmetic) and
  `the_region_draws_only_while_a_request_waits`.
- Its session-mount half: `approvals::region_tests::with_a_session_open_the_region_starts_below_its_tabs` (in the real
  main window with a session open, the region starts 12 pixels below the terminal area under the session's tabs).
- Its remount half, in GPUI form: `approvals::region_tests::a_card_after_the_region_emptied_is_placed_by_its_own_needs`
  (after the region had nothing to draw, a new card is placed by its own measurements, not held in the whole-window
  fallback by earlier, taller occupants). The region also resets its measurements when it empties; that spares only the
  first frame, which the test cannot observe.
- "a short viewport keeps approval content and answers reachable": `region_tests`
  `a_short_pane_gives_the_region_the_window_and_a_tiny_one_scrolls_it` and
  `a_short_window_gives_the_region_the_whole_window_without_waiting` pin where the region goes and when it scrolls as a
  whole.
- Its other half: `approvals::region_tests::in_a_tiny_window_the_wheel_scrolls_the_region_to_deny_and_deny_answers` (in
  a 48-pixel window the wheel scrolls the region, and Deny, then in view, answers).
- Superseded: "a link approval opens synchronously with user activation" was a browser popup rule, and the window opens
  no link itself (the helm does, after Allow), which also covers "an invalid URL on a card is neither opened nor
  answered as allowed" on the window's side.
- Superseded: "only the winning host add window probes its captured edited fields" keeps its single-window half
  (`approvals::host_add_tests::add_answers_allow_with_the_edited_destination_then_probes`), the rest gone with one GUI
  per helm.
- Superseded: "a session without a terminal anchors approval below its top row". `SessionPane::render` in
  `crates/gui/farhelm-gui/src/session_pane.rs` draws the region's anchor (`approvals::region::track_terminal_area`) in
  the terminal area box whatever the box holds: the terminals, the interrupted or retired card, the unavailable card, or
  nothing.
- "a requester session title stays on one line with its full hover text":
  `approvals::card_tests::a_requester_title_is_cut_to_one_line_in_a_narrow_card_with_the_whole_title_on_hover`.
- The long-host half of "a compact card follows the main pane and keeps the answer buttons visible":
  `approvals::region_tests::a_long_host_stays_whole_inside_both_always_allow_buttons_in_a_narrow_pane` (in a 320-pixel
  pane both Always allow buttons paint the whole escaped host and stay inside the card and the region).
- The keyboard choice in "one request stays selected across refreshes and rapid switches rearm the buttons":
  `approvals::region_tests::a_header_chosen_from_the_keyboard_opens_its_card_and_pauses_the_answers` (Tab reaches a
  waiting header, Enter opens its card, and the answers pause).
- "waiting requests scroll independently below a tall card and fixed late notice" and "a narrow short queue and late
  notice keep every answer reachable":
  `approvals::region_tests::a_long_queue_scrolls_in_its_own_list_below_the_card_and_the_notice` (at 960 by 900 the wheel
  scrolls the list to its last header while the notice, the card and its answers stay put; at 320 by 430 the region
  scrolls as a whole, every answer is in view once it has scrolled, and the list still scrolls inside it; in both a
  press on the last header opens its card).
- "long boxes shrink to their computed minima before the body scrolls":
  `approvals::region_tests::long_boxes_give_way_before_the_body_scrolls_and_then_the_answers_stay` (the old two-step
  resize: the boxes give up height before the body overflows, and once they are at their minima the body scrolls with
  the answers pinned).
- The goldens `approval-card`, `approval-card-narrow` and `host-add-dialog-agent` are scenes of one `#[ignore]`d golden
  test, which needs the headless renderer and runs through `scripts/check-gui-goldens.py` rather than in ordinary test
  runs.
- Driven for real on the test Mac on 2026-11-15 against a remote host: a session there asking to open its own loopback
  page showed a link card, whose Tunnel and open asked the tunnel warning and, on Allow, opened a tunnel the Mac read
  the remote page through, the request ending and the agent told it opened. A second press while the warning is up could
  not be driven there: the Mac's screen fits the window so small that the warning covers the button.

### `tests/agent-relay.spec.ts`

Tests: 4. Covers: agent relay: an agent clones its own session across hosts.

replaced: the relay itself (cloning with the source's own command, the target's refusal reaching the agent verbatim, a
host named by its alias and not by its raw destination once aliased) is the Rust suites':
`crates/farhelm/tests/e2e/agent_relay.rs` and `agent_listing_real_stack.rs`, and `farhelm-helm`'s `agent_requests` tests
(`clone_sends_the_source_launch_verbatim`, `a_create_refusal_from_the_target_reaches_the_agent_verbatim`, the alias
resolution tests). The spec ran with every permission on, so it showed no card; what it showed of the GUI, a relayed
session's row appearing live, end-to-end steps 60 and 62 show for spawns

- The alias resolution tests: `create_resolves_by_alias_once_one_is_set` and
  `create_on_the_raw_destination_is_refused_once_aliased`.

### `tests/spawn.spec.ts`

Tests: 4. Covers: a fake agent spawns children that appear without refreshing the observer.

replaced: end-to-end steps 60 (a spawn waits on its card; Allow starts the child, whose row appears in the window with
nothing else done), 62 ("Always allow on <host> only" turns on that host's session grant alone, and the next spawn
starts without a card) and 63 (Deny starts nothing and the agent is told the user declined); "Always allow from <host>"
turning the host's setting on is `approvals::card_tests` and `approvals::answers_tests` for the window's answer and the
helm's `always_allow_turns_the_host_setting_on_and_approves`; the parent filter is `farhelm-helm`'s `sessions_tests`.
The real-Claude case is dropped: it needs vendor credentials and network, which no automated run has, and a real agent's
request reaches the window the way the fake agent's does

- The window's Always allow: `approvals::card_tests::answer_buttons_answer_their_choice_and_nothing_while_busy` (the
  card's "Always allow from <host>" button answers Always allow), with
  `approvals::answers_tests::a_refusal_of_a_waiting_request_stays_on_its_card` for its `always_allow` body.
- The parent filter: `crates/farhelm-helm/src/sessions_tests.rs` `every_filter_dimension_narrows_the_list_on_its_own`.
- "explicit inheritance must preserve the parent's own command": `crates/farhelm/tests/e2e/structured_launches.rs`
  `explicit_spawn_inheritance_relays_the_parents_stored_launch` and the supervisor's
  `an_inheriting_spawn_is_relayed_with_the_sessions_own_launch` (the parent's launch relayed to the helm), and
  `agent_requests::an_inherited_spawn_creates_the_supplied_launch_on_its_own_host` (the helm creating the child with
  it).

## Approval cases in other areas' specs

These cases sit in spec files whose rows belong to other areas' parity files; this area ported them.

### `tests/github-checkouts.spec.ts`

Case: agent managed checkout approval and keyed recovery.

replaced: the card naming the repository and exact folder (`approvals::model_tests`), Allow and Deny reaching the helm
(end-to-end steps 60 and 63), and the keyed recovery (`crates/farhelm/tests/e2e/github_checkouts.rs`,
`agent_create_checkout_replays_and_templates_use_fresh_admission`)

- The repository fact ("managed checkout of", "GitHub: <repo>"):
  `approvals::model_tests::every_kind_shows_exactly_the_old_cards_content`.
- Steps 60 and 63 answer Allow and Deny on spawns, not on a managed checkout's create.

### `tests/provisioning.spec.ts`

Case: host retirement releases before removal and preserves approval focus: Remove success.

replaced: `layers_tests`, `focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card`

- Its approval-focus half: focus put on a card's Allow without answering it, while a dialog is open, stays on Allow when
  the dialog closes, and nothing is answered. The test closes a generic dialog through its opener, which stands for a
  dialog closing because its subject went, and does not drive a real host removal. The removal itself is Settings and
  hosts' half of the case.

### `tests/provisioning.spec.ts`

Case: host retirement releases before removal and preserves approval focus: authoritative Settings disappearance.

replaced: `layers_tests`, `focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card`

- Its approval-focus half: focus put on a card's Allow while the host's Settings dialog is open stays on Allow when the
  dialog closes because the host has gone from the helm's list, and nothing is answered. The same test covers it, with a
  generic dialog's close standing for the disappearance. The dialog's own close is Settings and hosts' half of the case.

### `tests/provisioning.spec.ts`

Case: agent host add asks before setup despite preference and after refusal reopen.

replaced: `approvals::host_add_tests` (`an_agents_add_always_asks_the_setup_confirmation`,
`a_refused_setup_reopens_still_asking`) and end-to-end step 64

### `tests/keyboard-scroll.spec.ts`

Case: SSH command arrival preserves an independent approval owner.

replaced: `layers_tests`, `focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card`

- With focus put on Allow without answering it, a late focus move the dialog makes inside itself (the SSH help's
  recovery when its commands arrive) leaves focus on Allow, and nothing is answered.

### `tests/keyboard-scroll.spec.ts`

Case: an approval's complete initial prompt is keyboard-scrollable.

replaced: the card's long texts are named scroll boxes (`approvals::card_tests`), which scroll from the keyboard
(`components::ScrollBox`'s tests)

- The scroll box tests: `crates/gui/farhelm-gui/src/components/tests/scroll_box.rs`
  `an_approvals_complete_initial_prompt_is_keyboard_scrollable` and
  `a_labelled_box_is_a_tab_stop_with_its_role_and_every_scroll_key_scrolls_it`.
- The card's named boxes: `approvals::card_tests::the_card_paints_its_badge_heading_count_from_line_facts_and_answers`.

### `tests/modal-trap.spec.ts`

Case: Rename wraps Close through Save while a real approval remains usable.

replaced: `layers_tests`, `an_approval_card_stays_usable_over_a_dialog`

### `tests/terminal-replay-rename.spec.ts`

Case: held rename success preserves an unrelated focus destination (its approval half: a card arrives while a submitted
Rename is held, Allow is focused, Rename closing leaves focus on Allow, and Enter answers).

replaced: `layers_tests`, `focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card` (focus put on
Allow without answering while a dialog is open stays there when the dialog closes, and Enter then sends exactly one
Allow) and `an_approval_card_stays_usable_over_a_dialog` (Enter on a focused Allow answers it while the dialog is still
open). The card there is up before the dialog opens; a card arriving takes no focus by
`approvals::region_tests::an_arriving_request_takes_no_focus_and_enter_answers_nothing`. The held Rename itself is
Sidebar and session view's half of the case

### `docs-shots/visual-review.spec.ts`

Case: the approval card's docs screenshot.

docs media

## Deviations

- Tunnel and open has hover help ("Tunnel and open: reach this port on the remote host through a tunnel, then open it in
  your browser"). The old button had none; every control in the GPUI app needs hover help.
- The card's own fact words ("Generated", "None") and its notes are drawn upright where the old card set them in
  italics: the app bundles Inter Regular and SemiBold only, and GPUI does not slant a face itself.
- The From line's Copy ID uses the shared compact button, with 8 pixels of side padding where the old one had 6, so it
  is 4 pixels wider.
- An agent's host add opened from its card while another dialog is open, or while a host is being probed, set up or
  removed, is refused with a reason ("Another dialog is open. Close it, then open this request's Add host again.", or
  "Another host change is still in progress. Open this request's Add host again once it has finished."), keeping the
  open dialog and the request. The old app queued the open behind the current dialog, in click order, dropping one whose
  request expired, and held it while an ordinary add's setup submission was out, so that a refused setup could still
  reopen its draft. The GPUI app has no dialog that waits for the dialog slot yet; with one, this becomes the old queue.
- On a 410 to an agent host add's Allow, the Add host dialog closes at once and the region's late-answer notice explains
  why: the request is no longer waiting, so the dialog has nothing left to answer. The old form instead stayed up
  showing "no longer waiting" in itself until the request left the list (`hosts/add.rs`;
  `approvals::host_add_tests::a_refused_unconfirmed_or_late_allow_probes_nothing` replies 410 while the request is still
  listed, so the close is the reply's).
- The old app held every tunnel action behind one claim, from the press through the consent save to the card's answer,
  and a card pressed meanwhile said "Another tunnel action is still waiting". While the old warning was up its backdrop
  covered every card, and a card's own buttons were disabled during its own action, so the old words appeared only while
  another action was checking consent or finishing. The GPUI card says so only while a tunnel warning is up, whoever
  asked it, the window in which the old app took no press at all: here the cards stay usable beside the warning, and a
  press on any card, the one that asked included, gets the words. Pressed while the consent save is still out, or while
  this request's answer is on its way, it does nothing and says nothing; pressed while another card's Tunnel and open
  answer is out, it goes ahead (`approvals::tunnel_tests`).
- The card's long-text boxes (commands, prompts, wrapper words) have hover help naming them, as every control in the
  GPUI app needs; the old boxes were tab stops without it.
