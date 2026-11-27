# Keyboard users cannot tell apart Recent setups whose wrapper prefixes differ only past the row's cap

Reviewed commit: ed0c91c883c3de132832272bb12cf8679df7e541

## TLDR

Two Recent setups in the launcher can have the same agent, folder and permissions but different wrapper prefixes (the
command placed in front of the agent, for example a sandboxing wrapper). The row shows the wrapper squeezed into a short
space with the middle cut out, so when the differences fall in the middle of the wrapper, a keyboard user who highlights
one of them sees the same text for both. Enter then launches (in the empty-field pane) or fills the form from whichever
is highlighted. The fix for saved Custom approval flags added a readable panel for exactly this reason, but did not
cover wrapper prefixes, which are just as decisive for what runs.

## Details

Priority bucket: `high`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t01f-launcher-custom` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Recorded in the reviewer's suggested `high` bucket; it would move to `other` if wrappers are judged not to carry
safety-relevant choices or if real wrapper pairs rarely differ only in the elided middle (a usage premise the reviewer
did not establish). The code comment on `wrapper_tail` records the cap as intentional, but no SPEC.md or SPEC_impl.md
text accepts it. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/launcher/view/recents.rs:141`

Revision: corrected which row draws the wrapper in each layout after reading recent_choice in start.rs. The conclusion
is unchanged.

This gap predates the reviewed commit. It comes from d2e55c718 (#4406), but ed0c91c883 is the change that set out to let
keyboard users distinguish the setups and stopped at flags. The recent row draws the wrapper through `wrapper_tail`
(crates/gui/farhelm-gui/src/launcher/view/recents.rs:141-148 at tip):
`.max_w(px(144.)).truncate().text_ellipsis_middle()`. That capped tail is the narrow typed-results row (recent_lines,
crates/gui/farhelm-gui/src/launcher/view/search.rs:1229-1318 at tip, called from draw_search_rows when not inline,
search.rs:1095-1110). The wide pane, for both the empty-field list and inline typed results, uses recent_choice
(crates/gui/farhelm-gui/src/launcher/view/search/start.rs:473-560 at tip). It puts `Wrapper: ...` first in the single
details line, then truncates the line at its end (start.rs:508-510 and 541-545) inside the 250-pixel column, so a
wrapper difference near the end is hidden there too. Its doc says 'The accessible name and hover help carry the complete
text when this display reaches its cap'. That is the same reasoning the removed TODO entry rejected for flags: 'Pointer
hover and the screen-reader label give the full choice. When the command field keeps keyboard focus, show the saved flag
words on the highlighted row ... before Enter' (TODO.md hunk removed in ed0c91c883). The new footer
`recent_permission_footer` (search.rs:1145-1203 at ed0c91c883) shows only `selection.permission_flags`, and only when
permissions are Custom, so a highlighted Recent row whose wrapper is clipped gets no footer. `setup_key` includes the
wrapper prefix through the normalized selection (crates/gui/farhelm-gui/src/launcher/recents.rs:111-127), so such setups
are separate rows, and wide-pane Enter launches the highlighted one (SPEC.md: an empty-list recent setup can submit
after filling; the reviewed commit's test wide_recents_show_custom_flags_before_enter shows Enter launching). Route: the
user saves two launches through wrappers such as `sandbox-run --profile=project-a --net=off --` and
`sandbox-run --profile=project-a --net=on --` with the same agent, folder and permission. The empty launcher lists both,
with each wrapper clipped: end-truncated in the wide pane, middle-ellipsized at 144 pixels in narrow results. Down then
Enter launches whichever is highlighted, with no painted way to read the full prefix without the pointer. No SPEC.md or
SPEC_impl.md text describes or accepts the recent row's wrapper cap (searched both for wrapper with recent, row,
truncate, ellipsis, compact). The one related spec sentence, added by this commit at SPEC.md:1547-1549, is limited to
Custom approval flags. To verify, read wrapper_tail and recent_permission_footer as cited. A fix could extend the pinned
footer to show the full wrapper prefix (in the terminal font) whenever the highlighted Recent row's wrapper is clipped,
or always when one is set, using the same paging, and update the SPEC.md sentence.

Suggested bucket: high, because a keyboard user can launch an agent through an unintended wrapper that may carry its
sandbox or network constraints. It moves down to other if wrappers are judged not to carry safety-relevant choices, or
if the visible ellipsis is judged a sufficient cue to check the hover help.

Confidence: confirmed for the source behavior (the cap, the footer covering flags only, the row and Enter paths).
Whether real wrapper pairs differ only in their elided middle is a usage premise.

proposed_drop: none

possible_cover: The design note on wrapper_tail (view/recents.rs:139-140) records the cap as intentional, with the
accessible name and hover help as the escape. That is a code comment, not a spec acceptance. The removed TODO entry said
'Keep the compact wrapper summary', which may be read as the maintainer scoping wrappers out of #5037.
