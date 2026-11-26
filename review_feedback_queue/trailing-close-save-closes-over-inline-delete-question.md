# A finished close-save still closes Settings over a newer inline Delete question

Reviewed commit: f76edf9e3395bd7a3848a71ee1ee9d5283e853e1

## TLDR

Say you answer Save in the close question while a save is slow, then open the Delete question for a leftover Tensorlake
sandbox or Podman container. Settings closes as soon as the save finishes and takes your unanswered Delete question with
it. The commit meant to keep newer Settings confirmations open, but it only sees questions drawn as separate layers, not
ones drawn inside the page.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t04b-settings-save-trust` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. The reviewer proposed a possible drop citing SPEC.md's "a dialog or menu opened from
Settings" sentence; the independent drop check DISAGREED: an inline question is not a layer (SPEC_impl.md) and the
spec's "newer action wins" intent fits it too. No other coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/settings/editing.rs:743`

Revision (turn 2 synthesis): confidence raised from likely to confirmed after checking the remaining premises in source;
a secondary consequence added. Full record follows.

`finish_close` (editing.rs:743-749 at f76edf9e33, unchanged at the tip) now refuses the automatic close only when
`layers::has_interactive_child` (layers.rs:1717 at f76edf9e33, 1719 at the tip) finds a child layer of the Settings
layer that does not pass the pointer through. The leftover Delete questions are inline questions, which are not layers:
Tensorlake (`settings/tensorlake_sandboxes.rs:309`, `prompt::ask_inline`, guarded only by `self.deleting` and a running
cleanup) and Podman containers (`settings/containers.rs:312`, `prompt::ask_inline`, guarded only by `self.deleting`).
SPEC_impl.md: "An inline question is not a layer: its owner decides where it is drawn". Neither guard looks at the
dialog's save batch. Settings stays interactive while a close batch is pending: the commit's own test
`a_completed_close_save_keeps_the_newer_tunnel_question` (credential_tests.rs) switches to Terminal and presses a switch
during the pending save.

Route: edit a form whose save takes a while (e.g. a credential replacement), then Close -> Save. Navigate to Tensorlake
or Containers and press Delete on a leftover; the inline question appears. When the save reply arrives, `advance_batch`
reaches `AfterBatch::None if batch.close && edits.is_empty()` and calls `finish_close`. That finds no child layer and
closes Settings, dismissing the question unanswered.

Secondary: if the user has already answered Delete, the inline question is gone and the delete request is in flight. The
same automatic close then drops the leftover list's in-dialog failure line, so a failed delete reports nothing.
Reopening Settings re-reads the leftovers and shows the row again, so the state is recoverable; that is why the bucket
stays `other`.

The new SPEC.md sentence ("If a dialog or menu opened from Settings still awaits input when the saves finish, both stay
open") may cover only layered dialogs and menus. But the commit title ("keep newer Settings confirmations open") and the
bug it fixes, a newer confirmation lost to an older answer, apply equally to these inline confirmations. No destructive
action runs. The same blind spot applies to a native file chooser opened from the Codex login editor.

Fix: have `finish_close` also consult the Settings children that own inline questions or in-flight row actions (the
`question` and `deleting` fields of the Tensorlake and containers leftover lists, and the recipe editor's inline leave
question). Alternatively, stop the automatic close once the user starts a new interaction after the Save answer. Verify
by reading `finish_close`, `has_interactive_child` and the two `ask_inline` call sites.

Suggested bucket: other. A dropped, unanswered confirmation is recreated in a moment, nothing destructive runs, and a
hidden delete failure becomes visible again on reopening.

Confidence: confirmed (source-established: inline questions are not layers, `has_interactive_child` inspects only
layers, the leftover Delete questions are not gated on the save batch, and Settings stays interactive during a pending
close save per the commit's own test).

proposed_drop: Possibly: SPEC.md's new sentence names "a dialog or menu opened from Settings", which may not include an
inline question inside a Settings page.

possible_cover: none
