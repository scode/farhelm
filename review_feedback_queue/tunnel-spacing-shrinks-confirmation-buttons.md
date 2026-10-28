# Tunnel spacing reaches confirmation answers

Reviewed commit: 4a99d51e560d873d8b2a8d25999926b2083507c5

## TLDR

When the tunnel pill appears, Restart, Replace and Delete confirmation buttons also become narrower.

## Details

Suggested bucket: **other**. Confidence: **confirmed CSS selector applicability**; appearance and usability were not
measured. No hidden warning, consent bypass or user-work loss is established.

`crates/farhelm-ui/assets/app.css:8512` applies `padding: 2px` to every `.btn` below `.titlebar-actions` whenever the
tunnel pill exists. Anchored confirmation answers are descendants of that container, so their previous `2px 8px` padding
becomes `2px` as well. Only horizontal padding changes. The next rule restores menu-item padding but does not protect
confirmation answers. Restricting the compact spacing to the toolbar's own controls would preserve answer sizing. The
confirmation descendants are in `crates/farhelm-ui/src/session_view.rs:2140–2320`.

Possible cover: `plans/reports/steel-session-chrome.report.md:145–150` records this cosmetic deferral, but provides no
maintainer acceptance or queued implementation. This finding rests on the stylesheet and component structure, not on
reviewing the report's compliance. No matching specification exception, Planned item, unlanded plan, BUGS entry, queue
record or ledger decision was found. Introduced here and unchanged through `d64ae3a81547729100529d210d3ea50434ff933d`.
Proposed drop: none.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `action-menus/f1`; the tunnel reviewer
identified the same instance and deferred to this record. Source review only.
