# Template rows can change the draft behind the Save as template checklist

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

A template row can change the launcher draft while Save as template remains open with the older values.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view.rs:999–1014,2494–2526;
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:116–132,314–325;
crates/gui/farhelm-gui/src/launcher/view/search.rs:273–296,429–435; pinned 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view/save.rs:60–89; crates/gui/farhelm-gui/src/focus.rs:555–581,621–628;
crates/gui/farhelm-gui/src/launcher/entry.rs:153–169; crates/gui/farhelm-gui/src/templates/dispatch.rs:459–479

The save checklist shares the launcher's dialog and therefore its modal focus authority. A separate top-layer check
cannot hold the list while the panel is open. Save as template mounts an inline entity and focuses its name within the
current launcher rather than opening a new layer. The row's take_search_row path clears the search and asks the focus
coordinator to focus that launcher field before it enters the template batch. Both controls are within the same topmost
dialog; the request uses a fresh mark from the click, so moved-on and top-layer checks do not admit or reject it based
on panel state. The shared apply_batch checks the YOLO write before queueing; shortcut admission separately checks the
save panel and opening it drops old queued batches. To hold template-row pressing consistently, decide panel eligibility
before the row's search/focus side effects and also at shared batch admission, rather than treating the focus
coordinator as an edit guard. The checklist still saves its displayed snapshot. SPEC_impl.md at pinned
tip:679–697,8770–8784. SPEC.md:1291–1298. The reviewer suggested other. Confidence: confirmed by source; no broader
modality claim for the intentionally inline panel.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-options/LO-15`.
