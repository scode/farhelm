# Escape in wide command results discards open Save as template edits

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Escape can discard Save as template edits while the launcher still shows the search results that received the key.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view.rs:216-246,999-1014,2494-2507;
view/search.rs:205-217,273-296,509-540; pinned 1a53d9d35889d7dde96e31320cc1681dd013737d view.rs:228-246;
view/save.rs:60-102; crates/gui/farhelm-gui/src/templates/save_panel.rs:134-139

The Save as template panel and wide search results are inline children of the same launcher layer. While the editor has
focus and results are open, the dialog dismissal hook handles Escape by removing the save panel before inspecting
results. This loses its unsubmitted name or checklist changes, whereas the narrow overlay takes Escape first, matching
SPEC.md:1147-1149,1436-1438 at the pinned tip. LO-15 shares the inline arrangement but its route is template
application, not dismissal: it enters the shared batch code directly and omits the open-panel admission check that the
shortcut dispatcher supplies. The checklist still serializes its shown snapshot. Prioritize the editor's active results
for Escape to repair LC-13. Hold template-row activation before clearing the query or moving focus, and at batch
admission, to repair LO-15. Keep the documented inline panel and outside editing behavior. The reviewer suggested other,
confidence confirmed by source, , Possible coverage: related to LO-15 but independent repair.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-command/LC-13`, with
`launcher-matching-contract/LMC-17`.
