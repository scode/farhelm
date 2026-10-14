# Menu navigation can follow an obsolete keyboard target

Reviewed commit: 57b5c0da0b72b0efb2f5c4c7a0693dd89fc40a45

## TLDR

After focus independently moves within an open session menu, the next arrow can navigate from an older item and reach
Delete unexpectedly.

## Details

Suggested bucket: `highest`, conservatively pending trigger verification. Confidence: likely conditional mechanism. The
unverified premise is a supported pointer or assistive-technology focus move within the open menu without activation or
dismissal. Source: dedicated `gpt-6.1-sol` high review, retained by a fresh independent Sol high drop auditor. Proposed
drop: rejected as ambiguous. This is a pre-existing discovery rather than an introduced regression.

crates/farhelm-ui/src/menu_panel.rs:550–555 always prefers the retained keyboard request over actual focused position
and event origin; :684–694 records that request. Item focus handlers, such as Clone at list/row.rs:2218–2222, update
only the focus signal. If independent focus moves inside the still-open menu, the old request remains authoritative. For
example, an older request for Stop followed by focus on Clone makes Down step toward Delete rather than Clone's
successor. list/view.rs:2046–2077 permits immediate deletion of an ended session without tabs. Verify the independent
focus trigger before choosing a fix; do not discard request precedence needed for delayed application focus callbacks.

Possible cover: SPEC_impl.md:543–553 specifies ordinary menu arrow navigation. The documented reason for request
precedence covers delayed results of Farhelm's own requests, not later independent focus choices. No exact recorded
acceptance was found. Pre-existing behavior alone is not coverage.

Status at frozen endpoint: persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703, menu_panel.rs:550–555,684–694 and
list/row.rs:2242–2245. No fixing commit identified.

Review coverage: the original reviewer read the full commit diff; the independent reviewer traced the specific candidate
and acceptance. No browser or assistive-technology reproduction was performed.
