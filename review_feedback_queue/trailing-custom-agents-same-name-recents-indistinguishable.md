# Recent setups for two custom agents with the same display name look identical

Reviewed commit: 461aef886b69b2134c58bef660994663081bfedb

## TLDR

If two registered custom agents have the same display name (Settings allows this), their recent setups in the New dialog
show the same visible agent name. The user cannot tell from the row which program it will run, and Enter on the
highlighted row launches it straight away (custom agents always run as YOLO). The distinguishing ID appears only in
hover help and the screen-reader name.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t01a-launcher-custom` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/launcher/view/search.rs:1115-1133 (recent_agent_words) at 461aef886b`

Revision (turn 2, synthesis): preserved unchanged in substance. Rechecked at the tip f0c042c9: recent_agent_words
(view/search.rs:1204-1225) is identical, and the later commits touching these files (4b64754c9, ed0c91c88, 0198cd145,
7bef64a6c) do not add the ID to the visible recent row. 4b64754c9's CustomRegistration states only change the chip help
and launch refusals, not the name drawn in recents. The Agent tab chips (tip choices.rs:417-425) likewise label by
display_name with the ID only in hover help; that is the same convention, cited in proposed_drop.

recent_agent_words (view/search.rs:1115-1133 at 461aef886b; tip view/search.rs:1204-1225) draws the current display name
and appends "(id)" only when the query matches the ID but not the name. It is used for the empty-field recent column
(view/search/start.rs recent_choice) and the search rows (recent_lines). selection_summary (recents.rs:358-377) appends
"(id)" whenever name != id, but that summary feeds only the accessible name and hover help. The commit's docstring on
selection_summary says it keeps the ID 'when it can distinguish registrations with the same display name', and the test
recent_custom_agent_uses_its_current_name_in_both_layouts says 'The visible name distinguishes recent agents before
selection', but visible text never shows the ID in the empty-field layout. Display names need not be unique:
CustomAgent::validate (farhelm-proto custom_agents.rs:25-35) checks only non-empty and presentation-safe characters, and
git grep found no helm-side uniqueness check. SPEC.md 'Recent setups': 'When the empty command field's keyboard
highlight is on that row, Enter also launches it through the ordinary Launch path.' Route: register agents a and b both
named 'Code'; launch each once; open New; two recent rows read 'Code'; Enter on the highlighted row launches whichever
registration it holds. Fix: draw the ID beside the name in visible text whenever another registration or recent row
shares the display name, as selection_summary already does for speech.

Suggested bucket: other. Duplicate names are a user choice, both programs are the user's own registrations, and the ID
is available on hover; it degrades identification rather than causing loss. It would move up only if the destructive
Replace-with Enter path (the Replace key cap) were shown to commonly hit this.

Confidence: confirmed (source-established; not executed).

proposed_drop: possible. Basis: the Agent tab's custom chips share the same display-name-only label (tip
choices.rs:417-425, HoverHelp 'Use custom agent {id}'), and duplicates are user-created. The commissioner decides.

possible_cover: none
