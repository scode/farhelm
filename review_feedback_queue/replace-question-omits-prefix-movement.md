# Replace questions move rows without updating their open popups

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

A Replace permission question arriving above the session list can move another session's row while leaving its open menu
floating at the old position. Its title still identifies the correct session; no destructive retargeting is established.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified conditional source defect**, without a browser geometry
observation. Reviewed change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; current question insertion is at
`crates/farhelm-ui/src/list/view.rs:5818-5852` (introducing revision `5601-5674`).

Start plain Replace A on a host requiring a YOLO answer, then open and focus B's row menu while the reply waits. A's
question is an ordinary in-flow block before the session list. Its border, padding, content and margins consume height
(`assets/app.css:9232-9243`) inside a fixed-height sidebar/content viewport (`609-648`). With enough window height for
the contents, that moves rows without scrolling or resizing the outer sidebar or heading.

Movement captures cover heading, marks, identity, attention, accepted row content and external layout, not the displayed
question-block prefix. Row popup placement is measured once; the registry's probe is callable, not a general position
observer (`list/row.rs:1523-1531` at introduction; `anchored_popovers.rs:708-743`). B's focused menu makes A's guarded
async focus decline, and an async-inline question scrolls only after its Cancel acquired focus
(`yolo_confirm.rs:221-251,303-326`). No incidental scroll or identical-row listing refresh is guaranteed to repair the
missing prefix invalidation. A single-question prefix predates the change; the reviewed change adds independent source
blocks and explicitly avoids background focus/scroll after the user moved on.

SPEC.md:3276-3283 requires an unrelated completion to leave the menu open and the menu to follow its session. Older
SPEC_impl.md:1025-1027's measured-menu closure policy also rejects stale placement, but it is not the remedy to apply
over the final product rule. Track/probe the displayed prefix's geometry through the existing opening ownership and keep
the current menu attached to its session; preserve fresh openings and hidden-panel behavior.

The earlier YOLO-focus plan was abandoned. It proposed focus permission for three questions, not prefix movement, and
its abandonment establishes neither a repair nor acceptance of stale placement. Stop-progress wrapping, marked-count
reflow and closed-popover scan-cost records have different movement inputs. No exact other specification acceptance,
Planned item, pending repair, BUGS, queue, current-ledger or filter cover was found. No runtime check ran.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g010/l9`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): view.rs:6149-6195 still places per-source
questions in normal flow before the rows, outside session-heading at :5680; movement inputs at :4839-4934 still omit
that prefix's shape. The guarded async-inline focus and scroll policy is unchanged, so B's focused menu denies
incidental scrolling to A. The omitted layout input remains source-confirmed, with native movement unmeasured.
SPEC.md:3303-3311 supplies the keep-open/follow rule; the abandoned YOLO-focus plan remains historical rather than a
current repair.
