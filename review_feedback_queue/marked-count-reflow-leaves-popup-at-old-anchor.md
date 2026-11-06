# Marked count reflow leaves popup at old anchor

Reviewed commit: a2f38b21870d0c705defc81fc5e53b6f13a3ae2a

## TLDR

A changed marked count may move the menu button onto another toolbar line while its popup stays put. The necessary
rendered geometry remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/view.rs:3871-3873 at a2f38b21870d0c705defc81fc5e53b6f13a3ae2a`.

Source assessment (read with the later corrections below) (review record g087/d1, event 15):

Reviewed commit: a2f38b21870d0c705defc81fc5e53b6f13a3ae2a. TLDR: While a marked-session menu is open, a background
change to the number of visible marks can move its menu button onto another toolbar line while the popup stays at its
old position. This is a visual/navigation defect, not destructive retargeting. Suggested bucket other. Confidence:
unverified after investigation; source establishes missing invalidation and a concrete count-change producer, but the
exact supported width/font/count combination that moves the menu toggle has not been rendered under the source-only
charter. Mechanism: assigned view.rs:3863-3873 derives effective_marks but reports toolbar movement only for
empty/nonempty changes. The assigned app.css addition at :1415-1427 makes .marked-bar a flex-wrap container whose count
is variable-width nowrap text; controls follow the count, and at :1472-1476 the toggle/clear have 24px minima. Necessary
context marking.rs:228-238 registers/leases the popup as Sessions row 'list:marked'; :268-294 measures its fixed
placement only on opening; :239-252 closes for command eligibility-order or layout_epoch changes, not count changes with
the same commands. A concrete producer is the already assigned retained-mark/filter behavior: mark several live
sessions, filter Running so some marks are hidden, open the metadata menu with stable Pin/Read offers, then a hidden
marked session starts running and the feed adds it to the same filtered listing. The count grows (e.g. across a digit
boundary), while !effective_marks.is_empty() and action order stay unchanged. At a flex-wrap threshold that pushes the
menu wrapper to the next line, its old fixed-position popup is neither retired nor remeasured. Existing refutations
checked: view.rs:1704-1713's changed count-banner capture contains actual session IDs only; anchored_popovers.rs:225-226
filters by the exact leased row, excluding 'list:marked'. Row identity captures likewise use session identities.
AppBody's layout_epoch at lib.rs:1934-1948/:2048-2070 observes outer shell scroll and app-sidebar resize; content reflow
inside its fixed full-height sidebar does not itself resize that shell. Popup measurement is not a general
ResizeObserver. SPEC_impl.md GUI menu contract :994-997 explains that fixed measured coordinates require closing when
their anchor could move; this toolbar explicitly uses the ordinary anchored menu shell. Missing premise: a concrete
rendered width/font/mark-count transition moving this specific toggle rather than only another control; source does not
supply font glyph measurements or browser flex-line layout, and runtime/browser rendering is prohibited. A focused
layout regression that keeps menu offers stable, advances an effective count through a digit-width threshold at
supported narrow sidebar widths, asserts toggle geometry changed, and then checks dismissal/reopening would settle it.
Fix direction: include count/toolbar-layout shape in regional movement capture, or directly track/remeasure anchor
geometry without replaying old invalidations into new openings. Temporal check: inclusive tip
8a8c9579e04299410652f0e4681975a7198dfc7b view.rs:3960-3962 still uses only the nonempty boolean. proposed_drop:false.
possible_cover:unknown; parent handles coverage, no queue/plan/filter searches. This is distinct from all supplied
modifier/range interface issues. No runtime reproduction attempted.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g087/d1`; supporting records
`g086/f6`, `g088/x5`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
