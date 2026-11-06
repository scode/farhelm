# Empty filter actions escape keeps session marks

Reviewed commit: 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40

## TLDR

Escape from empty-filter actions leaves session marks active instead of clearing them under the list's Escape policy.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/view.rs:5071-5085 at 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40`.

Source assessment (read with the later corrections below) (review record g090/l8, event 16):

NEW verified discovery; resolves lead l8 from readable shipped source. Reviewed commit
1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40. TLDR: after marking sessions and filtering them out, pressing Escape while
focused on Show all hosts or Show all statuses does not clear those marks; widening the filter brings the marked
sessions back despite the Escape. SPEC.md Session list: Marks belong to the current client page and filtering retains
hidden marks; Escape clears marks only from the session list, without preventing default handling, and yields to a menu,
dialog or terminal that owns the key. Source route: view.rs:3245-3249 owns clear_marks; new native buttons at 5073-5082
lie in .filter-empty-actions, a sibling of .session-status-filter and PinDragList. The only view.rs onkeydown calling
clear_marks is on .session-status-filter at 4979-4980; .sidebar-content at 4652 has only onscroll, so Escape from the
new buttons cannot bubble through the clearing handler. Whole UI source search for
clears_on_escape/clear_marks/on_clear_marks found the remaining handlers only in row.rs:1701-1705 and marking.rs:303-307
(marked toolbar), neither an ancestor of the new buttons. The existing clear callback permits the stated idle/no-modal
sequence. Reproduce using ordinary controls: Ctrl/Cmd-click a session to mark it, choose a status with no matches, focus
Show all statuses, press Escape, then activate it; the marked toolbar returns. No runtime test was run; event ancestry
and callback ownership establish the missing path. Fix by giving empty-filter actions the same Escape handling or moving
the list-scoped handler to an ancestor with existing menu/dialog ownership guards, then cover retained hidden marks.
Suggested bucket other: broken keyboard clearing/convenience, no loss of work or destructive-consent route established.
Confidence confirmed. proposed_drop:false. possible_cover: none searched; parent decides. Temporal qualification:
introduced with the assigned new focusable controls; inspected later path history through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b contains no repair. Distinct from supplied g015:l6 spacing and g015:l7
query-assertion findings.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g090/l8`; supporting records
`g088/x6`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
