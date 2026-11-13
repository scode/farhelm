# Heading-growth tests require a surviving session menu to close

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other. The exact heading-only growth,
same-row/shifted-anchor proof and dismissal assertions remain at `e2e/tests/sidebar.spec.ts:2537–2552`.
`SPEC.md:3303–3310` still requires owner-controlled dismissal and following the session; `SPEC_impl.md:1027–1029`
retains the older layout-dismissal rule, subordinated by `SPEC.md:3225–3226`. No exact later repair or cover was found.
Marked-count reflow remains a different popup.

## TLDR

Two sidebar tests require an open session menu to disappear when only the heading grows. The final focus rules require
that menu to stay with its session, so the tests would reject the intended behavior.

## Details

Bucket: **other**. Confidence: **confirmed direct assertion/contract conflict**; original suggestion other. This is test
and specification drift, not a demonstrated runtime failure or security/data-loss route.

Reviewed source change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; the assertions remain at the reviewed tip, whose
later focus rule changes their acceptance target. They were consistent with the earlier close-on-layout design.

`e2e/tests/sidebar.spec.ts:2441-2447` generates “count wraps” and “warning appears” cases. At 2537 it opens the source
menu; 2539-2550 changes only the heading/list count or warning and proves the same first-row session survives and its
anchor moves solely with heading growth. Lines 2551-2552 require the panel removed and its old node disconnected. There
is no user dismissal, owner closure or source-session removal in that sequence.

`SPEC.md:3273-3283` keeps focus within a menu, limits automatic closure to its owner or subject going away, and requires
an open menu to move with its session. Its rulebook preamble at 3198-3200 makes that the target despite lagging UI;
3307-3308 says feature details apply rather than override it. `SPEC_impl.md:1025-1027` still describes one-time
coordinates and closing on layout changes. That old implementation description is the discrepancy, not acceptance of the
test's contrary expectation; 1172-1177 directs using the new product rules.

Correct the expected behavior together with product positioning, retaining the useful second half that protects a fresh
menu from delayed old resize notifications. The separately recorded Stop-progress projection omission also needs a
remedy consistent with keep-open/follow; do not use this record to demand blanket closing on layout change.

The earlier Near term geometry-probe simplification expressly kept heading-growth dismissal and this test passing, so it
did not repair this mismatch. Recording-time main `cb03def00870e18a98a1fd9046073d2849be3253` removes that TODO in
anticipation of a GUI rewrite; it is no longer a current proposal. The marked-toolbar reflow queue item concerns another
popup and trigger. No exact current ledger, Planned item, indexed unlanded plan, accepted bug or filter cover was
established.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `heading-tests-close-menus`; `g015/l3`. Direct
assertion and old/new contract checks remain in `prep/groups/g015/findings.jsonl`. No runtime tests ran.
