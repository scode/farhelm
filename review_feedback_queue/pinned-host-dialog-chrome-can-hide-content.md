# Host Settings footer remains clipped in a short window

Reviewed commit: 79a11381ef36b1b39f07dde652e29d718a37c3d5

## TLDR

Host Settings can still hide its footer Close button and save-as-you-go note in a very short window. This commit deletes
the record tracking that remaining case while fixing the separate action dialogs. Escape and header dismissal remain
available.

## Details

The old side of `review_feedback_queue/pinned-host-dialog-chrome-can-hide-content.md:1–39` describes Host Settings;
later sections extend it to setup, removal, refusal and uninstall. The commit deletes the entire 159-line record.

At the reviewed revision, `crates/farhelm-ui/src/hosts/settings_dialog.rs:604` mounts
`host-settings-dialog host-settings-refreshed`, without `host-dialog-framed`. Its footer is at `816–824`.
`crates/farhelm-ui/assets/app.css:2826–2835,3361–3379,3423–3426,3511–3533` retains the viewport cap, clipped outer
overflow, nonshrinking chrome and body-only scrolling. The new whole-card scrolling at `3537–3538` applies only to
`host-dialog-framed`.

The old 360×160 CSS-pixel example allows at most 136px for the card, less than its header, body padding and footer need.
Scrolling the body cannot reveal a sibling footer beyond the clipped card. Focus-induced programmatic movement of hidden
overflow does not establish pointer scrolling. Preserve the remaining tracking record or settle settings separately;
this is incomplete queue removal, not a new settings-layout regression.

Suggested bucket: **other**, for reachability and incomplete tracking. Confidence: **likely**. Class scoping and queue
deletion are confirmed; exact browser thresholds and focus-induced scrolling were not exercised. No security or
user-work-loss consequence is established.

Possible cover: the ledger's `pinned-host-dialog-chrome-can-hide-content.md` entry at `16787–16807` and
`plans/queue/reachable-at-small-sizes.md:12–34` name action dialogs and explicitly instruct queue removal. That is real
possible coverage, but neither expressly discards the original settings case. The landing report explicitly excludes
Host Settings from the fallback (`plans/reports/reachable-at-small-sizes.report.md:118–120`). `SPEC.md:194–198` still
promises visible settings chrome; the queue lifecycle requires narrowing partially addressed findings. No exact
replacement acceptance, Planned, BUGS or queue item was found. The scrolling filter excludes content unreachable by
scrolling. Proposed drop: none.

The new tests exercise the framed action surfaces. The separate settings geometry test uses 360×420, not the 160px case,
so it does not resolve this remainder. The title-shortening destination refutation is a different concern and does not
exclude settings clipping.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; dialog contract, implementation, report
and test reviewers independently reconciled the old record against the selectors and action-only decision. Restore the
original mnemonic narrowed to this case when recording. No runtime reproduction was performed.

Original finding IDs: `dialog-layout-contracts-1/f1`, `dialog-layout-contracts-2/f2`,
`dialog-layout-implementation-1/f3`, `dialog-layout-tests-1/f2`.
