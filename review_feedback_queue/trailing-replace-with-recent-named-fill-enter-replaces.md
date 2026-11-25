# Replace with names a highlighted recent setup "Fill" while Enter on it replaces the source session

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

In Replace with, a screen-reader user who arrows down into the Recent list hears the highlighted setup named "Fill from
recent setup: ...". Pressing Enter there does not just fill the form: it fills and immediately runs Replace, which
creates the new session and deletes the source session. Before this change the same row was announced as "Replace: ...",
which matched what Enter does. Sighted users still see the "⏎ Replace" key cap. That key cap is also part of the row for
screen readers, but whether they announce it as the highlight moves is not established; if they do not, the spoken name
is the only cue before the source session is deleted.

## Details

Concerns a02af24df9 ("fix: label recent setup actions in the launcher", #4829). Locations:
`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:288-297` (row accessible name),
`crates/gui/farhelm-gui/src/launcher/view/search.rs:296-318` (`search_enter`), `start.rs:108-162` (`take_start_choice`,
`take_start_row`), `crates/gui/farhelm-gui/src/launcher/view.rs:1335-1369` (`press`, `press_at`).

`draw_start_rows` (`start.rs:286-297`) sets the start-column recent row's accessible name to
`Fill from recent setup: <summary>` whenever the row is enabled, non-container and the mode is ReplaceWith. The row is
an OptionRow (`components/option_row.rs:180-185`: `.aria_label(self.label)`, `.aria_selected(...)`; module docs: "The
label is the accessible name, not drawn text") with `Role::ListBoxOption` and `.accessible_selection(active)`. Per
SPEC_impl.md, "Launcher and quick switcher option rows keep keyboard focus on their field or picker button as the
selection moves", so the highlighted option reaches assistive technology through that selection and its name. Its
description is the hover help (OptionRow passes `describe=true` to `with_hover_help`), which for a recent row is only
the setup summary (`recent_hover`, `recents.rs:152-164`) and never mentions Replace.

The Enter route: with the command field empty, `search_enter` (`search.rs:296-318`) checks in Replace with that the
drawn `start_enter_frame` showed this row (binding Enter to what was drawn; the drawn row's name is the "Fill from
recent setup" name), then calls `take_start_choice(&row, keyboard=true)` -> `take_start_row` (`start.rs:133-162`), which
calls `take_recent` and then `self.press(window, cx)` because `keyboard` is true. `press` -> `press_at`
(`view.rs:1341-1369`) submits with the currently shown source warning. There is no further confirmation, so Enter
performs Replace with's create-then-delete (SPEC.md, "Replace with": "Launching creates the new session and then deletes
the source"; SPEC.md launcher section: "In Replace with, a highlighted recent setup's key cap reads \"⏎ Replace\", since
Enter there replaces, and so deletes, the source").

The only spoken evidence of the Enter action is the key-cap child (`start.rs:536-542`, `Role::Label`, aria_label "⏎
Replace"), a child node of an option that carries its own label. The commit's test
(`replace_rows_name_the_action_and_restart_rows_are_inactive`, `recents_view_tests.rs:298-335`) documents "Enter on the
empty pane's highlighted recent can replace the source" and asserts that the name starts with "Fill from recent setup:".
The mismatch is designed in, and no test checks that the highlighted row's spoken name or description warns of the
replacement. The commit's motivation (a click only fills, so "Replace" misnamed the click) is real, but the change moved
the misnaming onto the destructive key.

Verify by reading the locations above. Fix: when the row is highlighted in Replace with, make its name or accessible
description say Enter replaces the source (for example keep "Replace: ..." on the highlighted row, or add a description
"Enter replaces the source session; click fills the form"), or make Enter on a highlighted recent in Replace with only
fill, matching the name. Add a test asserting the highlighted row's name or description mentions the replacement.

Bucket: high by what is established: a misleading accessible name on a key that deletes the source session (Enter
creates the new session and deletes the source, stopping its live program, while the spoken name says Fill). It moves to
highest if assistive technology announces the highlighted option's name without its "⏎ Replace" key-cap child, because
the spoken name would then be the user's only cue before a session deletion. The reviewer rated "could be highest" as
unclear: the destructive route (Enter -> `take_start_row` -> `press` -> Replace deletes the source) is established in
source, but what the user hears depends on screen-reader behavior on a ListBoxOption with a labelled child, which source
cannot settle.

Confidence: confirmed for the code path (the name says Fill, Enter replaces); likely for user impact, with the
unverified premise that assistive technology reads the option name without the key-cap child.

Possible cover: the commit records the triage outcome for `launcher-recent-fill-option-named-replace.md` as fixed by
this change, and that decision may have accepted the click-versus-Enter split. The archived outcome
(TRIAGE_OUTCOMES_ARCHIVE.md, "launcher-recent-fill-option-named-replace.md": fix code) recommended putting "Replace"
only where activation replaces, for example on the highlighted row's Enter cap. That decision did not consider the
spoken name of the highlighted row, so it does not cover this. SPEC.md's launcher sentence about the "⏎ Replace" key cap
covers only the visible cap, not the accessible name.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g19 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
