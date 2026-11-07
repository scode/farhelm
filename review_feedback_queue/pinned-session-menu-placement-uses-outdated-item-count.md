# Pinned session menu placement uses outdated item count

Reviewed commit: 2a9fc911f89ee2fe41355ffb7a1226b5d9aad167

## TLDR

Pinned-session menu placement still budgets for eight actions after the menu grew to ten, causing unnecessary scrolling
even when the full menu could fit above.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/row.rs:2495-2550 at 2a9fc911f89ee2fe41355ffb7a1226b5d9aad167`.

Source assessment (read with the later corrections below) (review record g083/c14, event 30):

Reviewed commit: 2a9fc911f89ee2fe41355ffb7a1226b5d9aad167. Completes and supplements the full c14 finding event with
final checks; preserve its source geometry, suggested regression and fix direction. TLDR: an interior live pin's
ten-item menu needlessly scrolls near the bottom of a tall viewport despite room to place the whole menu above. Bucket
other; confidence confirmed source placement/size mismatch, no runtime pixel measurement; proposed_drop false;
possible_cover unknown, excluded sources not searched. New Move controls at row.rs:2495-2550 enlarge the old ordinary
eight-item menu, but menu_panel.rs:132-147 retains the 528px reserve and eight-item measured premise. For a low row,
:207-210 limits actual menu height to 520px. The active .menu-item/.menu-copy/.menu-description CSS yields at least
47.2px for each two-line described command (10 commands = 472px), then header padding/margin/border, four separator
margins and panel padding/border already take the box above 520px even before header text or wrapping. Final bounded
reads confirm MENU_ACTIONS at row.rs:205-215 actually has all ten and session_menu_order at :236-247 permits both Move
directions for an eligible interior live pin; controls remain native buttons. Inclusive-tip CSS retains the same
relevant sizing: app.css:9797-9804 header, 9845-9858 items, 9861-9863 copy/description, 9882 separators; tip :3037-3044
retains max-height inheritance and overflow-y:auto. Tip menu_panel.rs:132-147,200-210 preserves 528px and the unchanged
clamping formula, so no later repair exists through 8a8c9579e04299410652f0e4681975a7198dfc7b. Follow-up c15 used pinned
Dioxus 0.7.10 primary source to refute the stronger possibility that inner scroll itself dismisses the menu: scroll is
non-bubbling and registered locally, not captured by ancestor layout handlers. The finding therefore stays limited to
unnecessary scrolling and initially hidden lower commands, not inaccessible/destructive actions. Fix remains updating
the measured reserve/documentation for the enlarged menu or using actual bounded panel height; regression should
distinguish ample room above from genuinely short viewports. No outstanding premise or lead and no runtime experiment.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g083/c14`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
