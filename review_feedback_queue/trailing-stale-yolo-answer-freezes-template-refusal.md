# A stale permanent YOLO answer freezes a transient Save-as-template refusal

Reviewed commit: 7bef64a6c450ffc98f27b66902b33bdb9175c204

## TLDR

The user has Save as template open when they answer the YOLO question with "Start, and don't ask again on this host".
The launcher now explains that nothing started: "Not launched. Finish or cancel saving the template before launching."
The user then cancels the template panel, which the message itself told them to do. The message stays up regardless and
keeps telling them to finish or cancel a panel that is gone, until they edit something or press Launch. The same happens
for other passing reasons the launcher can give at that moment, such as waiting for another operation on the session or
a host change in progress.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t01g-launcher-custom` `f8` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:377-383`

All line numbers are against 7bef64a6c4.

- `launcher/view/yolo_question.rs:377-383` (`Owner::stale_permanent_answer`, the branch before the setting is written):
  it takes `launcher.refusal(cx)` once and stores it as `launcher.note = Some(Note::WakeRefusal(...))`.
- `launcher/view.rs:2916-2921` (render): `Note::WakeRefusal` replaces the live refusal alert, so the frozen words are
  drawn in its place. The ordinary refusal path is recomputed from `self.refusal(cx)` on every render.
- `launcher/view/save.rs:95-104` (`close_save_panel`, used by the panel's Cancel and by Escape through `on_dismiss`): it
  clears `save_panel` but leaves `note` untouched.
- The note is cleared only by `changed` (`view.rs:1052`) and `submit` (`view.rs:1544`).
- `launcher::refusal` (`view.rs:1274-1283`) lists the transient reasons this branch can freeze: `HOST_CHANGE_IN_FLIGHT`,
  `SOURCE_BUSY` ("Wait for this session's other operation to finish") and `TEMPLATE_PANEL_OPEN`. Each clears without an
  edit or a press.

Route: a YOLO refusal leaves the question up. The user clicks Save as template, which is not an edit and leaves the
question open (the commit's test `a_permanent_answer_with_save_as_template_open_shows_the_launch_refusal` relies on
this). They click the permanent answer, and the note is set. Then they click Cancel in the panel: the panel closes, and
the note still reads "Not launched. Finish or cancel saving the template before launching." The commit's test and its
golden `launcher-yolo-save-panel-refusal` stop before the panel is closed, so neither notices this.

How to verify: extend that test so that after the refusal it clicks `save-template-cancel`, then asserts on
`launcher-refusal`. By source, the frozen sentence is still drawn.

Possible fixes: (a) for the refusal case, set `pressed` (or a dedicated flag) so the live refusal alert shows,
recomputed each frame, instead of a frozen `WakeRefusal`; or (b) clear the note when the panel closes and when the other
transient conditions end.

Suggested bucket: other. The text is misleading but harmless: nothing starts, nothing is written, and the next press of
Launch clears it.

Confidence: confirmed (source-established: the note is stored once, overrides the live alert in render, and
close_save_panel does not clear it).

proposed_drop: none

possible_cover: none known
