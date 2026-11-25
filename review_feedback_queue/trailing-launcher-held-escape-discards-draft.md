# Held Escape can still close the launcher and discard its draft

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

In a wide launcher window, if the user has typed a query (so results fill the side column) or has the Save as template
panel open, holding Escape a moment too long closes the results or the panel and then the whole launcher, throwing away
the half-built launch setup. The reviewed fix protects only the case where a keyboard highlight sits in the empty list
of recent setups and templates. This assumes the system delivers repeated Escape presses while the key is held, which
the reviewed fix itself also assumes.

## Details

Concerns de3fa38be3 ("fix: preserve the session launcher draft"). The other paths are pre-existing; this commit fixes
the same consequence for one path only, and its message states the general rule ("Closing the launcher takes a fresh
press"). Locations: `crates/gui/farhelm-gui/src/launcher/view/search.rs:638-653` (`dismiss_command_pane`),
`crates/gui/farhelm-gui/src/launcher/view.rs:230-248` (the launcher dialog's `on_dismiss` hook),
`crates/gui/farhelm-gui/src/launcher/view/save.rs:94-103` (`close_save_panel`).

Escape reaches the launcher only as the Dismiss action (`focus.rs:831` -> `layers::dismiss_top` -> `layers::dismiss` ->
the launcher's `on_dismiss` hook, `view.rs:230`). Escape is bound only to Dismiss (`actions.rs:410`), and the launcher
has no Escape key handler of its own, so both the command field and the save panel's name field reach the `on_dismiss`
hook; the existing test `escape_closes_the_panel_and_keeps_the_launcher` (`templates/save_tests.rs:552`) and the new
held-Escape test drive that path. Nothing on it filters `KeyDownEvent::is_held`: held repeats arrive as key-downs with
`is_held` set, and the only filter is the closing guard in `focus::note_press` (`focus.rs:306-327`), which swallows a
repeat only after something called `focus::note_closing`. Closing the layer drops the launcher and its draft
(`view.rs:196-200`, "closing it drops the launcher").

The commit adds `focus::note_closing(true, ...)` in `dismiss_command_pane` only when `start_highlight.is_some()`
(`search.rs:646-649`). When the wide pane is open because of typed text (`inline_results == true`,
`start_highlight == None`), `dismiss_command_pane` closes the results and returns true without noting the close; the
next auto-repeat Escape reaches `on_dismiss` again, `dismiss_command_pane` returns false, no save panel is open, and the
hook returns `Dismissal::Close`. Likewise the `on_dismiss` branch that closes the save panel (`view.rs:243-246` ->
`close_save_panel`) never calls `note_closing`, so a held Escape closes the panel and then the launcher. The narrow
popover is not affected: it is its own layer, and the layer manager's close path calls `note_closing` (`focus.rs:1713`).
The new test `a_held_escape_clears_the_template_highlight_without_closing_the_launcher` (`search_view_tests.rs:1036`)
exercises only the highlight case.

Verify by reading the early `if` in `dismiss_command_pane` and the save-panel branch of `on_dismiss`; neither calls
`note_closing`. Fix: call `focus::note_closing(true, window, cx)` whenever `on_dismiss` keeps the launcher open because
it closed an inner region (results, highlight, save panel), for example in the hook itself before returning
`Dismissal::Stay`, and add tests for held Escape with typed results open and with the save panel open.

Bucket: other (loses unsaved form input the user can rebuild; the bucket rules say unsaved form input is not user-owned
work). It could arguably be high if launcher setups are long, since held Escape is a common keyboard gesture. Not
highest: the loss is unsaved launcher input, which the bucket rules exclude.

Confidence: likely. Source reading confirms that no `is_held` filter or closing guard covers these paths; the premise
not verified by execution is that the platform delivers auto-repeat Escape key-downs to the Dismiss binding (the
commit's own triage item and test rest on the same premise for the highlight case).

Possible cover: SPEC.md around line 3853 ("After Escape closes the last open dialog or menu, a held Escape ... goes
nowhere") covers only closing the last dialog, not an inner region of the launcher; no spec text accepts the draft loss.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g18 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
