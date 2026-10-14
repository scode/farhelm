# Window resizing can make the copy guidance unreadable

Reviewed commit: 2347268d9663f0f40211f65eee08ff5484667fbe

## TLDR

Narrowing the window while the notice is showing can collapse it into a thin column with most guidance clipped.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed
drop: none.

crates/farhelm-ui/assets/copy-on-select.js:246–260 measures the notice at its previous left position before
repositioning it. With the auto-width fixed box in assets/app.css:5669–5689, that position constrains available width
after a resize. Confirmed with the extracted historical CSS and controller: show the generic notice at release (780,400)
in an 800×600 viewport, then resize to 400×600. The box changes from 432×74 to approximately 61×592, with 2004px of
scroll content. Its pointer-transparent body prevents ordinary scrolling to read the clipped instructions. Resetting
left/top to zero before measurement produces a readable 392×74 box; tooltip.js:252–258 already uses that approach. The
new unit fixture returns a constant size (js-tests/copy-on-select.test.js:193–198), so its resize assertions cannot
expose position-dependent wrapping. Scope: notice presentation, with no demonstrated process or data loss.

Possible cover: No matching queue or ledger item found. The rare-glitch filter does not fully match: resizing is
ordinary input and needs no narrow timing race.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. Current
copy-on-select.js:248–262 and app.css:6257–6277 retain the mechanism; the same browser reproduction produced identical
measurements.

Review coverage: full commit diff read: true. No commit diff hunk remains unread. Initial aggregate tool output was
truncated; affected diff and relied-on context were reopened. Unrelated portions of the large triage ledger,
specifications, TODO, terminal implementation, and queue bodies were not read exhaustively. Accessibility announcement
behavior and full-app WebKit behavior remain unverified. Checkout was not modified; concurrent queue changes belong to
other reviewers.

Runtime evidence: Recorder-backed minimal Chromium reproduction used extracted historical and pinned-main
CSS/controllers, a selected-pane DOM, the real resize listener, and an origin-reset comparison. Run
8cc0477a-4da6-42dc-a298-2413accbb65d completed with exit 0 and complete, untruncated evidence. It reproduces layout
only: no live helm, supervisor, PTY, WebKit, or bundled UI font. Evidence is retained privately by the run IDs. Scratch
helper formatting and type checking passed after adding DOM types; registration run 9cdf640b-38ad-4b8d-8c6d-63bbf76dc28e
executed zero product tests. Earlier helper type-check failure is retained as run 15e0b008-5004-4d7e-9c1b-8cd134189639;
it did not execute the reproduction. No broader runtime suite was run.
