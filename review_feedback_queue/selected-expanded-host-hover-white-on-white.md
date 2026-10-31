# Selected expanded host labels turn white on white

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

In Light appearance, hovering the selected expanded host can make its name and destination unreadable.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The hover selector has greater specificity than the selected background selector and supplies white --bg-2 while
selected text retains white --on-fill. The expanded labels inherit that foreground. Hosts-management has a separate
override; this finding is Sessions mode. The CSS contrast checker does not resolve the cascade or bind these labels, and
the browser regression checks geometry/text rather than selected-hover colors. Source mismatch is confirmed; rendered
pixels were not observed.

Source locations: `crates/farhelm-ui/assets/app.css:718-726,383,391; crates/farhelm-ui/src/hosts.rs:736-772`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d; assigned introducing commit0ea8f3872ccc53b3c1f02e9c4d9e2fb2e9e531ca
adds expanded selected-label inheritance725-726. TLDR: in light appearance, clicking an expanded host name leaves its
name and destination white on a white row while the pointer remains there. Source cascade is confirmed:
hover:not(:disabled) at718 has specificity(0,4,0) and wins the background over pressed selector719(0,3,0); selected
color stays var(--on-fill), and selected .host-rail-expanded/.host-rail-destination explicitly inherit it at725-726.
Both --bg-2 and --on-fill are#ffffff in light palette383/391. Markup hosts.rs736-772 supplies exactly these classes and
aria-pressed state, with no inline foreground on the names. A normal name click is a shipped producer, exercised as
source by host-short-names.spec.ts351-370. The generic later .btn:hover/focus rules2302-2305 change only background and
have lower specificity; no rail selected-hover repair exists in the later selected-hover group9082-9089.
Hosts-management mode is excluded because its higher-specificity rule829-832 sets ordinary foreground; this finding
concerns the Sessions filter presentation. New host label impact is distinct from pre-existing All/footer hover styling.
Suggested bucket OTHER, confidence confirmed CSS cascade/token and actual DOM-class contract; no rendered observation.
proposed_drop none. possible_cover none exact found: SPEC.md1599-1602 requires expanded identity labels and
selected/hovered rows; SPEC_impl.md493-519 requires legible selection/inverse roles, not unreadable overlap. Planned
bucket empty; searches across plans/queue,BUGS,queue INDEX and ledger found no exact accepted/pending repair.
selected-session-inverse-ink-on-light-controls.md concerns session-menu child surfaces and selected-template contrast
records concern templates; none covers rail identity text. FILTER rare display exception does not apply to an ordinary
click/hover. Test evidence: contrast checker explicitly does not implement CSS specificity
(app-css-contrast.test.js165-168); SELECTED_OWNERS/PRIMARY/SECONDARY185-240 omit host-rail labels. Expanded browser
regression302-378 checks text/visibility, geometry, and membership after click, not computed selected-hover
ink/background. No tests run. A targeted selected-hover accent background or an appropriate neutral foreground would
address the conflicting roles; implementation not requested.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g174/f4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
