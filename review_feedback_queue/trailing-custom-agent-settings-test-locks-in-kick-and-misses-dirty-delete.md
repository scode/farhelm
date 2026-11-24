# Custom-agent Settings test locks in external-delete kick-to-Add during in-flight PUT and misses dirty-draft Delete ghost retention

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

The Settings test for custom-agent lifecycle locks in an unhelpful navigation jump that kicks the user off their draft
page when an external delete arrives mid-save, and only exercises Delete on a clean draft, missing the bug where
confirming Delete on a modified custom agent leaves a ghost entry in the Settings sidebar.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/tests.rs:330`.

In crates/gui/farhelm-gui/src/settings/tests.rs:272-373 (`creating_and_deleting_a_custom_agent_follows_the_catalog`),
two test-quality defects align with production bugs in `settings/custom_agents.rs` (g50/l2 and g50/l1): (1)
Broken-behavior lock-in (lines 272-273, 323-338): after pushing an external delete while a
`PUT /api/custom-agents/example-code` is in flight, line 330 asserts
`assert_eq!(heading(cx).as_deref(), Some("Add custom agent"));`, locking in `sync_custom_agents`'s jump away from a kept
draft (`settings/custom_agents.rs:132`:
`selected_deleted |= selected.as_ref() == Some(id) && (!keep || !draft.removed)`), which hides the 404 refusal off-page
and forces the test to navigate back to `settings-section-custom-example-code` at line 338 to observe
`"Register again"`. (2) Missing invariant witness (lines 359-372): the test claims "A delete confirmed on the page
removes it entirely" (line 273), but it only confirms Delete immediately after a clean `201` save (`dirty == false`) and
queues the `remove` push before the `204` reply in one `settle(cx)`. It never tests confirming Delete on a dirty draft
(or a `204` reply clearing `editor.deleting` before `sync_custom_agents` runs), where `Editor::delete`'s callback
(`settings/custom_agents.rs:416-423`) clears `editor.deleting = false` without discarding the dirty draft, causing
`sync_custom_agents` (`settings/custom_agents.rs:131`:
`let keep = !draft.deleting && (draft.saving || draft.dirty(cx))`) to retain the deleted agent as a ghost nav row
offering `"Register again"`. How to verify: inspect settings/tests.rs:272-373 alongside
settings/custom_agents.rs:125-158,415-425. Fix: once `sync_custom_agents` keeps the user on a retained draft during
external delete and `Editor::delete` removes/clears dirty drafts on confirmed Delete, update line 330 to assert the user
stays on the kept draft and sees the 404 refusal inline, and add a test case that edits a field before confirming Delete
(and covers reply-before-push ordering) to prove the deleted agent disappears cleanly.

Reviewer metadata (g51_custom_agents_gui_settings_b/f5): suggested bucket: other (test-quality gap and broken-behavior
lock-in in Settings unit tests; production consequences are tracked by g50/l1 and g50/l2).; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g51_custom_agents_gui_settings_b/f5`.
