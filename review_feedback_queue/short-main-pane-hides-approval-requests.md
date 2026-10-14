# Short main panes can collapse the approval region to zero height

Reviewed commit: 87945ef3e80a46dba0a67c905c27cd644b027176

## TLDR

In a sufficiently short window, waiting approval requests disappear until the user enlarges the window or changes the
selected session.

## Details

Suggested bucket: `other`. Confidence: likely; the zero-height calculation is confirmed statically, but the precise
browser viewport threshold and focus-induced scrolling were not reproduced. Source: dedicated `gpt-6.1-sol` high
adversarial review. Proposed drop: none.

crates/farhelm-ui/assets/approval-layout.js:20–30 positions the region below the tab strip and sets its maximum height
to max(0, min(pane_height * 0.55, pane_bottom - tab_bottom - 24)). When session chrome reaches within 24px of the pane
bottom, this becomes zero. app.css:6184–6191 provides scrolling only inside that zero-height region, so scrolling cannot
reveal its cards or answers. Session chrome cannot simply shrink away: app.css:983–992 and :5874–5881 preserve the
titlebar and tab strip. The first parent positioned cards within the viewport independently of session chrome. Scope:
unusually short or heavily zoomed browser views; requests fail safely after their approval timeout. Provide a usable
placement fallback when the below-tabs space is exhausted.

Possible cover: SPEC_impl.md:3987–3993 explicitly describes the remaining-space cap, but does not explicitly accept
invisible, unanswerable requests. review_feedback_queue/pinned-host-dialog-chrome-can-hide-content.md concerns different
controls and a different clipping mechanism; it does not cover this approval region.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. approval-layout.js is
unchanged from the reviewed commit; the region still scrolls internally. No fixing commit identified.

Review coverage: full commit diff read: true. No reviewed-commit diff content remains unread. TRIAGE_OUTCOMES.md,
specifications and unchanged surrounding modules were inspected selectively, not exhaustively. Some initial combined
outputs and broad current-main searches were truncated; relevant reviewed diff, rules and approval comparisons were
reopened. Unrelated intervening current-main CSS hunks remain unread and were not used to establish findings. Browser
geometry, scroll anchoring and queued-click delivery were not observed at runtime. Checkout had concurrent reviewer
changes; this review created, modified and deleted nothing.

Runtime evidence: None. Static historical review only; no reproduction tests, builds or recorder runs.
