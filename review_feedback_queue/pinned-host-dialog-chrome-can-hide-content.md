# Pinned host-dialog controls can exceed the available height

Reviewed commit: c2201032ba978bb0e871edcc3ce3441be3a3423f

## TLDR

In a sufficiently short browser window, the close button and save-as-you-go note disappear and cannot be reached by
pointer scrolling.

## Details

Suggested bucket: `other`. Confidence: likely; clipping follows from source geometry, but the precise browser threshold
and keyboard-induced outer scrolling were not measured. Source: dedicated `gpt-6.1-sol` high adversarial review.
Proposed drop: none.

At the reviewed commit, crates/farhelm-ui/assets/app.css:2924 changes the card to overflow:hidden, while :3043-3048
prevents the header and footer from shrinking and allows only the body to scroll. The card remains capped to viewport
height minus margins (:2506-2527). The header needs at least 85px for its 44px mark, padding and border (:2926-2940);
the body retains 40px vertical padding (:2978-2982); the footer adds padding and controls (:3030-3039). For example, a
360×160 CSS-pixel viewport gives the card only 136px, so footer content starts beyond its clipped boundary. Scrolling
the body cannot reveal this sibling. The first parent allowed whole-card scrolling. Scope is unusually short or heavily
zoomed web viewports; the header dismiss button and Escape remain recovery paths. Provide a compact layout or whole-card
scrolling fallback when the fixed chrome cannot fit. The new tests cover only 360×420
(e2e/tests/terminal-multihost.spec.ts:1646).

Possible cover: No matching coverage found. SPEC.md Topology:178-180 requires visible header/footer without declaring a
minimum height. TODO.md Planned, BUGS.md, the queue index and relevant ledger entries do not cover this trigger.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: app.css:2630 retains the viewport cap,
:3077-3081 retains clipping, and :3200-3208 retains the fixed header/footer. No fixing commit identified.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.

## Additional affected scope

The independent Astra pass also reports this setup-heading mechanism and suggests `high`; Sol suggested `other` because
the narrow/short-window consequence was not measured. The existing index bucket is retained, with both suggestions for
human triage. This repeat adds no new distinct finding.

Reviewed commit: fb80a800629118f1ea43605f45d464d4f0114248

Confidence: likely; actual clipping dimensions and browser focus-induced scrolling were not reproduced. Suggested
bucket: `other`. Source: dedicated `gpt-6.1-sol` high adversarial review. This extends the same finding rather than
adding a duplicate.

crates/farhelm-ui/src/provisioning.rs:804-807 newly puts the complete destination into the pinned header; hosts.rs:3936
supplies it without abbreviation. In assets/app.css:2922-2926 the frame hides overflow, and :3045-3053 makes the
header/footer nonshrinking while only the body yields height. The title's single-line limit at :2951-2956 applies
exclusively to .host-settings-refreshed, which setup does not carry. Consequently a wrapped heading can exhaust the
height capped at :2527, collapsing the plan region and overflowing the footer. Trigger: a long valid destination or SSH
alias, especially in a short phone-width window. Previously the destination remained in the scrollable plan rather than
an unbounded fixed header. Bound the setup heading and preserve usable body height. The new geometry tests use short
destinations and an 850px-tall viewport (e2e/tests/provisioning.spec.ts:529-532).

Possible cover: None found in the queue, Planned bucket, BUGS, relevant ledger entries, or filters.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: assets/app.css:3077-3081,3106-3111,3200-3208
retain the same scoping; provisioning.rs:809-819 retains the unbounded heading.

## Additional affected scope

Reviewed commit: fb80a800629118f1ea43605f45d464d4f0114248

## TLDR

Setting up a host with a sufficiently long SSH destination can leave its plan and answers clipped, especially in a
narrow or short window.

## Details

Suggested bucket: `high`. Confidence: likely — CSS mechanism and missing cap confirmed by inspection; exact browser
clipping threshold and an end-to-end long SSH-alias fixture were not reproduced. Source: dedicated `gpt-6-astra` high
adversarial review. Proposed drop: none.

The new heading renders the entire destination in provisioning.rs:804–810, supplied by hosts.rs:3933–3936.
app.css:3045–3053 makes that heading non-shrinking, while :2922–2926 clips the height-constrained dialog. The existing
heading cap at :2948–2957 applies only to .host-settings-refreshed, which neither setup caller uses. Consequently, a
sufficiently tall wrapped heading consumes the available height and pushes the body/footer outside the visible card; the
outer backdrop cannot scroll the card's clipped contents. Destinations have no application length cap
(farhelm-helm/src/store.rs:1442–1461), and SSH configuration aliases can supply long destinations. Before this commit,
the destination appeared inside the scrolling plan rather than an additional pinned heading. Bound the setup heading
while retaining the full destination in the plan. The new layout tests use short destinations and an 850px viewport, so
they do not exercise this boundary.

Possible cover: None found in the original queue index, relevant triage ledger entries, TODO Planned bucket, BUGS, or
applicable specification sections.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: app.css:3073–3111 and 3197–3208 retain
the same clipping, settings-only title cap, and non-shrinking setup header; provisioning.rs:809–818 still renders the
full name. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content remains unread; the truncated combined output's
missing diff tail was reopened. TRIAGE_OUTCOMES.md was searched and read selectively, not all 13,089 lines. Individual
queue finding files were not opened because the original index supplied no matching candidate. Unrelated portions of
specifications, implementation files, tests and intervening-main changes were not exhaustively read. No Sol reports or
working-copy queue additions were consulted.

Runtime evidence: None run, as requested by default. Static review only; browser geometry remains unverified. No
checkout files were created, modified or deleted.

## Additional affected scope

Reviewed commit: 1e0c3745462087e101bfe5f7f48d31ed1c1cff07

## TLDR

A sufficiently long host name can make the removal dialog’s explanation and buttons inaccessible by normal scrolling.

## Details

Suggested bucket: `high`. Confidence: likely — accepted input and CSS mechanism verified; actual clipping and
autofocus-induced scrolling were not measured in Chromium or WebKit. Source: dedicated `gpt-6-astra` high adversarial
review. Proposed drop: none.

The new host-dialog-framed class at crates/farhelm-ui/src/hosts/settings_dialog.rs:732 and
crates/farhelm-ui/src/provisioning.rs:2828 applies overflow:hidden (assets/app.css:2906–2910). Its header and footer
cannot shrink, while only its body scrolls (:3029–3037). However, heading/version truncation applies exclusively to
.host-settings-refreshed (:2935–2940), which neither changed dialog has. Their new headers therefore wrap without a
height bound. A registered, unaliased SSH destination of several thousand characters is accepted:
crates/farhelm-helm/src/store.rs:1460–1461 imposes no length limit, and :4464–4504 stores it. Opening Remove for that
row can make the header exceed the viewport-capped card, collapsing the explanation and placing the footer outside its
visible area. The previous whole-card overflow:auto allowed scrolling to those controls. Uninstall inherits the same
layout defect when its heading exceeds available height. Bound the pinned heading and retain the complete identifying
text in a scrollable or otherwise accessible location. Escape/header dismissal remains available; no data loss or
unintended operation is established.

Possible cover: None found. TRIAGE_OUTCOMES.md's host-actions-disappear-supported-sidebar-widths.md concerns sidebar-row
horizontal overflow, not these dialog headers. No matching Planned item, BUGS entry, queue item, or filter.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: settings_dialog.rs:815 still opts into
the frame; provisioning.rs:2838 does likewise; app.css:3077–3081,3106–3111,3200–3208 retain the conflicting clipping,
settings-only truncation, and shrink rules. No fixing commit identified.

Review coverage: full commit diff read: true. No reviewed-commit diff hunks unread. Did not read the entire 13,089-line
triage ledger, complete specifications, complete surrounding modules, or all unrelated queue files; searched these for
relevant coverage and read matching sections. Initial combined output was truncated; complete diff content was visible,
and relevant supporting material was reopened separately. No Sol reports or working-copy queue additions read.

Runtime evidence: None, as requested by default. Static review only; browser geometry remains unverified. Created,
modified, and deleted no files; no fetch, rebase, or helper agents.
