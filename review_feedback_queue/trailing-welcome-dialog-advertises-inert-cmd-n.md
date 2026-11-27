# The Welcome dialog advertises Cmd+N, which does nothing while it is open

Reviewed commit: 6814a9be66dde346a740ae96368ae6f1f6e3edcf

## TLDR

On the Mac, Help > Welcome shows a ⌘N key hint beside Start a session. Pressing ⌘N while the dialog is open does
nothing, so the dialog teaches a shortcut that does not work where it is shown. In the empty main pane, where the same
hint appears, ⌘N does work.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t11e-dialogs-welcome` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. SPEC.md's "Global chords stay inactive" sentence covers the inertness, not the hint.
No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/welcome/content.rs:85-94 (6814a9be66; chip added in ce0d95a21d)`

ce0d95a21d added the chip in WelcomeContent::render (welcome/content.rs at ce0d95a21d, the Start button's .content(...)
with NEW_SESSION_SHORTCUT_NAME.map(|name| key_chip(name, palette.on_fill, palette))); NEW_SESSION_SHORTCUT_NAME is
Some("⌘N") on macOS (actions.rs:233-237 at tip). 6814a9be66 routes the dialog through the same render_at, so the dialog
draws the same chip. Cmd+N dispatches NewSession to main_view.rs:327-329 (tip), which calls session_list::open_new;
launcher/entry.rs open_with (tip line ~138) refuses when layers::focus_layer_open(window, cx), which is true while
Welcome is open. SPEC.md "Welcome dialog" (tip ~5162-5164) also states "Global chords stay inactive." So the hint
promises an action the dialog cannot take. In the launcher-style dialogs this group restyled (What's new's Got it,
rename's Save, the launcher's Launch), a key chip on a button names a key that activates that button inside the dialog,
so users reasonably read ⌘N the same way. Verify by source as above, or on a Mac by opening Help > Welcome and pressing
⌘N. Fix: draw the chip only in the pane (render_at with dialog None), or show the Enter chip in the dialog, since focus
starts on Start a session and Enter activates it. Alternatively, have the dialog handle NewSession by closing itself and
opening New, like its Start button, though that departs from the spec's 'Global chords stay inactive'.

Suggested bucket: other — misleading hint text with no data, security or workflow loss; the user can still click or
press Enter.

Confidence: confirmed (source-established: the chip is drawn in both surfaces and open_with refuses under an open focus
layer); the claim that users read a button's chip as an in-dialog activation key is a UI convention inference, not
executed behavior.

proposed_drop: none

possible_cover: the 6814a9be66 content.rs patch belongs to a neighbouring t11 group, which may record the same issue;
the SPEC 'Global chords stay inactive' sentence covers the inertness, not the hint.
