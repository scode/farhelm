# Any custom agent registration change resets the Settings search selection

Reviewed commit: be60e53538e9f361a6488714007a48202e110a8b

## TLDR

While a Settings search is open, if any custom agent registration is added, renamed, edited or deleted (from another
window, the CLI or an agent), the result the user moved to with the arrow keys jumps back to the first result and the
list scrolls to the top, even when the changed registration is not among the results. An Enter pressed just after the
push opens the first result instead of the one the user had picked.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t03b-custom-settings` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/settings/search.rs:550`

Revision (turn 2, synthesis): unchanged in substance; added the shown-versus-drawn analysis and a second trigger.
SettingsDialog::render calls Search::refresh_custom_catalog on every frame (crates/gui/farhelm-gui/src/settings.rs:1753
at be60e53538; 1769 at tip). refresh_custom_catalog (search.rs:550-554 at tip) calls match_current_query whenever
update_custom_entries reports any change to the registration entries; update_custom_entries compares whole Entry values
including registration_words (display name, ID, command), so any change to any registration counts. match_current_query
unconditionally sets `self.selected = 0` and `self.scroll.scroll_to_item(0)` (search.rs:622-623). The doc comment on
refresh_custom_catalog promises to keep the query in step 'without resetting its selection on an unrelated redraw'; it
achieves that for redraws with no catalog change, but a push that changes an unrelated registration (one that does not
match the query, or a query like 'notifications' that matches no custom entry at all) still resets the user's position.
A loss of the store (session::current_store returning None) also empties the registration entries and resets the
selection, then the next session repopulates and resets again. Route: user types a query, presses Down twice to the
third result, another app or `farhelm` CLI edits any custom agent registration, the next render resets selection to 0,
and Enter (open_search_result -> Search::chosen, settings.rs:1187) opens the first result. Because the reset happens
inside render, GPUI's draw-before-dispatch (SPEC_impl.md: 'GPUI can draw a dirty frame before dispatching a key ...
shows that frame only at the next frame') means an Enter the user pressed while result 3 was on screen can be dispatched
after the reset frame is drawn but before it is shown, opening result 1, which the user never saw selected.
SPEC_impl.md's when_shown rule is written for questions and requests, and SPEC.md's 'One GUI at a time' lets input
delivered before a redraw run as delivered, so this is navigation to the wrong Settings page, not a state-changing
action on unseen state. Verify by reading match_current_query and refresh_custom_catalog. Fix: in
refresh_custom_catalog, remember the selected entry's (section, id) before re-matching and restore the index of the same
entry in the new matches when it is still present, falling back to 0 only when it disappeared; or skip re-matching when
the new matches list (mapped to entries) is identical to the old one.

Suggested bucket: other; it only changes which Settings page opens, nothing is saved, deleted or lost, and the trigger
(a registration change during an active search) is rare.

Confidence: confirmed (source-established control flow; not executed).

proposed_drop: none

possible_cover: none
