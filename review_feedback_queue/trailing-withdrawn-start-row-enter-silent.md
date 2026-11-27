# Enter on a recent setup or template withdrawn by narrowing is swallowed without the 'no longer available' notice

Reviewed commit: 0198cd145dd76539db17890740e063d4b22464cc

## TLDR

In New, Clone or Replace with, a user who has arrowed onto a recent setup or template in the launcher's side column and
presses Enter just as the window narrows (the column hides) gets no response and no explanation. Nothing tells them the
highlighted choice went away. Pressing Enter again, as anyone would after an apparently ignored key, launches the draft
as it stands without the template or recent setup they picked. In Replace with that second Enter replaces, and so
deletes, the source session with the unfilled draft.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t01e-launcher-custom` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. FILTER.md's "Rare, self-correcting glitches" filter was considered and not applied: in
Replace with the follow-up Enter replaces and deletes the source session with a draft the user did not finish, a
state-changing consequence outside the filter's boundary. SPEC_impl.md's when_shown paragraph permits refusing the press
but not omitting SPEC.md's message. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/launcher/view/search.rs:326`

`search_enter` (crates/gui/farhelm-gui/src/launcher/view/search.rs:319-344 at 0198cd145d, unchanged at tip f0c042c9
lines 319-344) computes `row` as `start_highlight` only when `command_pane_fits(window)`. `reflow_search`
(search.rs:700-722) sets `start_highlight = None` whenever the window is too narrow and recaptures `start_enter_frame`
with `None`, unshown. An Enter before that frame is presented compares the shown frame (`Some(Some(template))`) to `row`
(`None`), fails, and hits the bare `return` at search.rs:335: no `refuse_stale_choice`, no refusal text, no notify.
0198cd145d widened this guard from Replace with only to every mode except Restart with
(`!matches!(self.draft.mode, Mode::RestartWith(_))`). The new tests pin the silent behavior:
`assert_narrowing_drops_enter` in crates/gui/farhelm-gui/src/launcher/search_view_tests.rs (added at ~1293-1309) narrows
the window, sends Enter, checks nothing was sent, and then expects a second Enter to send the create/replace with the
template never applied. SPEC.md (launcher search, lines ~1245-1247 at tip) says: "If an explicitly highlighted choice
disappears before Enter, or a clicked row has already disappeared, the field shows 'That choice is no longer available;
choose a current result', keeps the query and clears the highlight. That Enter applies no choice and launches nothing.
The same rule applies to the empty field's recent setups and templates." The narrowing case meets the premise (an
explicitly highlighted start row disappears before Enter), but the message is never shown. The nonempty-query path
(search.rs:346-352) and stale start rows (`RecentRefusal::Stale` in search/start.rs) do call `refuse_stale_choice`.
SPEC_impl.md's `when_shown` rule allows refusing a press on an unpresented frame, but it does not override SPEC.md's
required explanation for this case. Verify by reading search.rs:319-344 and the test helper. Fix: when the shown frame
held `Some(row)` and the current `row` differs (in particular, is `None` after narrowing), call
`self.refuse_stale_choice(None, window, cx)` instead of returning silently. Keep the silent consume only for the plain
unpresented-no-change case. Update `assert_narrowing_drops_enter` to expect the STALE_ROW refusal after the first Enter.
The folder field's analogous silent consume (folder_list.rs:221-245) is not covered by that SPEC sentence, which names
search results and the empty field's recent setups and templates, so it is not part of this finding.

Suggested bucket: other. The harm needs the user to press Enter again without noticing that the column and highlight
went away; the second Enter acts on a correctly displayed frame, so the established consequence is a spec mismatch plus
a confusing silent key. Losing the explanation in Replace with, where the second Enter deletes the source, could argue
for `high`.

Confidence: confirmed (source-established; behavior is pinned by the commit's own test).

proposed_drop: none

possible_cover: SPEC_impl.md's when_shown paragraph (~line 1001: "a handler for a newly drawn frame may have to refuse
the press") permits refusing, but not omitting the SPEC.md message.
