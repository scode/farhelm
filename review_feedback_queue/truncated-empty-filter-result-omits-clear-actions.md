# Truncated empty filter result omits clear actions

Reviewed commit: 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40

## TLDR

A truncated empty-filter result can omit the clear-filter links needed to recover through that view.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/list/view.rs:5056-5085; crates/farhelm-ui/src/rows.rs:809-825 at 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40`.

Source assessment (read with the later corrections below) (review record g090/l11, event 20):

NEW verified finding in discovery pass 2; resolves l11 from source. Reviewed commit
1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40. TLDR: when a host's capped listing has no rows matching the chosen status, the
empty-result explanation loses Show all hosts / Show all statuses instead of offering the new clearing shortcuts.
SPEC.md Session list:1849-1853 promises buttons below an empty result to clear each applicable filter and separately
requires a truncated listing to retain its qualified no-match message; preserving that wording does not require
suppressing the independent clear actions. Source: rows.rs:806 accepts filtered zero-match empty replies; :809-814
returns NoMatchLine::Text for truncated replies. view.rs:5056-5058 renders only a message for Text, while
.filter-empty-actions and both clearing handlers exist exclusively in the complete-list Filters branch at :5059-5085.
Reachable source premise: helm aggregate.rs:636-647 propagates a selected/any host's list_truncated, then
assemble:450-463 applies the user filter and independently sets matching to zero while retaining hosts_truncated=true.
For example, a host's retained prefix contains no Waiting rows while its wire cap was hit. Thus zero returned matches
and truncated=true are compatible, not an impossible synthetic combination. Fix by keeping qualified wording but
deriving/painting clear actions independently from the answered host/status for either variant. Suggested bucket other
(missing convenience controls); confidence confirmed by branch structure and producer. proposed_drop:false;
possible_cover:none searched, parent decides. Caveats: header status choices and the host chip remain usable, no
data/work loss or highest route, and SPEC.md:2050-2058 explicitly treats above-cap fleets as outside the product's
intended scale. This finding is limited to the new empty-result clear-link promise; it does not request pagination, cap
changes or stronger completeness claims. No runtime experiments. Previously inspected relevant path history through
8a8c9579e04299410652f0e4681975a7198dfc7b has no repair.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g090/l11`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
