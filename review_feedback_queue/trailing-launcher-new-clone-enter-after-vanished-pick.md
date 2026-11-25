# New and Clone launch on an Enter whose shown meaning was a pick that vanished before the redraw

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

In the New and Clone launcher, a user who has a template or recent setup highlighted in the side list (or a folder row
highlighted under the folder field) and presses Return expects Return to pick that item. If the window was just
narrowed, or the host's connection changed, before the screen caught up, the same Return instead launches the session
immediately with whatever the form holds. Replace with is now protected against this; New and Clone are not. The
folder-row case depends on a key arriving before the redraw, which the implementation spec says can happen.

## Details

Concerns 7092da4d9e ("fix: honor the shown choice when replacing a session"). Partly pre-existing: the change adds
presentation guards for Replace with only and explicitly keeps the old launch fallthrough for the other modes.
Locations: `crates/gui/farhelm-gui/src/launcher/view/search.rs:301-320` (`search_enter`, empty-field branch) and
`crates/gui/farhelm-gui/src/launcher/view/folder_list.rs:221-249` (`folder_enter`, `return replacing && had_pick`).

Resize path. `search_enter` computes `row = command_pane_fits(window).then(|| start_highlight.clone()).flatten()` from
the live window width. After a resize to a narrow window with no presented redraw, `row` is None although the last
presented frame showed a highlighted template (SPEC.md:1510-1511, "Enter on a highlighted template applies it"). For
`Mode::ReplaceWith` the new `start_enter_frame` check consumes the press. For every other mode control falls through to
`self.close_search(...); self.press(window, cx)` and submits a create. A redraw before dispatch does not help:
`reflow_search` (`search.rs:656-662`) clears `start_highlight` when narrow, so the outcome is the same. The commit's own
test `narrowing_replace_with_cannot_turn_a_template_pick_into_a_launch` (`search_view_tests.rs:1059`) establishes this
premise for Replace with.

Folder path. A store push that changes the chosen host's connection changes `history_key` (`view/history.rs:79-90`). At
the next render, `follow_history` (called from Render, `view.rs:2301`) drops the folder highlight
(`history.rs:111-112`). SPEC_impl.md, "Questions: the prompt", says "GPUI can draw a dirty frame before dispatching a
key", so a Return already typed against the frame that showed the highlighted folder row can reach `folder_enter` with
the highlight gone. In Replace with, `enter_can_replace` (cleared, then recaptured unshown) consumes it. In New and
Clone, `folder_enter` returns false and the field handler (`view.rs:860-863`) calls `press()`, which launches. Without a
redraw, `launch_history(cx)` returns None for the new key (`history.rs:344-351`), the rows shrink, and an out-of-bounds
index hits the same `replacing && had_pick` = false branch.

Verify by following these two sequences in mode New. Fix: apply the presentation guard to every mode's empty-field Enter
(capture `start_enter_frame` unconditionally and refuse when the shown row differs), and treat "the presented frame
showed a highlighted folder row" as consuming Enter in every mode (return `had_pick`, and use an ActionFrame for the
highlight in all modes), with a test per mode.

Bucket: other. The result is an unintended create, with the folder and settings the form showed; nothing is deleted, and
the user can close the session. It could move to high if launching a draft that carries an initial prompt for a YOLO
agent counts as materially harmful. Not highest: nothing is deleted or lost; the launched draft is the one displayed in
the form, and only the meaning of Return changed.

Confidence: confirmed from source for the resize path (`search.rs:301-320`, `656-662`). Likely for the folder path: it
rests on GPUI drawing a dirty frame before key dispatch (stated in SPEC_impl.md, "Questions: the prompt") or on a store
push being processed between frames, plus the source cited above.

Possible cover: the triage outcomes named in 7092da4d9e's message may have scoped this to Replace with on purpose. They
are archived in TRIAGE_OUTCOMES_ARCHIVE.md as "launcher-resize-changes-enter-action.md" and
"trailing-replace-with-folder-enter-out-of-bounds-highlight.md" (decision q-frame, "Fix the harmful ones, accept the
harmless ones"; their completion criteria list "launches" among the harmful cases). Their assessments and fixes covered
Replace with only, so they do not cover New and Clone. Drop arguments triage may weigh: for the resize path,
SPEC_impl.md, "Questions: the prompt": "SPEC.md's \"One GUI at a time\" (2026-10-10) lets input delivered before a
redraw run as delivered", since the resize is the user's own input; for the folder path, SPEC.md:1228-1229, "In the
editable folder field, Enter chooses an arrow-highlighted dropdown entry without launching; with no highlighted entry it
launches as before", read literally as the highlight state at dispatch. Against both, SPEC_impl.md, "Questions: the
prompt", says "The same rule starts at the control that asks the question or sends the request directly ... a handler
for a newly drawn frame may have to refuse the press", and the commit's Replace-only scope reads as a choice to protect
only destructive submits rather than a spec allowance.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g17 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
