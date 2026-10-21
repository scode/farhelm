# A secondary press can suppress primary-selection copying

Reviewed commit: 268c20907073c8aa68b66d0c97cd620a240deacf

## TLDR

Pressing another mouse button during a primary drag can leave completed terminal text selected without copying it. Live
terminals and frozen snapshots share the state pattern. Physical browser behavior remains unverified.

## Details

`crates/farhelm-ui/assets/terminal.js:5483–5498` overwrites press ownership on every mousedown. A secondary press
replaces button zero; the first mouseup consumes the record (`5512–5516`). The new primary-only check at
`copy-on-select.js:79–82`, called at `terminal.js:5547–5553`, then rejects it. Snapshot copying similarly overwrites and
consumes its press (`1285–1306`). With local selection enabled, xterm's `SelectionService.handleMouseDown` only starts
selection for button zero; its mouseup keeps the selection while ending drag observation. A secondary press therefore
need not make the selection empty. Bounded vendor-function excerpts support this distinction; no full-bundle or runtime
review is claimed. Preserve primary ownership through its matching release without allowing secondary-only stale recopy.

Suggested bucket: **other**, for clipboard convenience. Confidence: **likely**; handler state and the added rejection
are established relative to the parent predicate, while overlapping-button delivery, context-menu/autoscroll effects and
actual missing copy are unverified. No security or loss of user-owned work was established. Proposed drop: none.

Possible cover: reviewed `SPEC.md:1784–1801` (final `1815–1832`) promises completed-selection copying. Its platform
clipboard-failure exception does not clearly cover losing Farhelm's gesture before requesting a write. Final ledger
`TRIAGE_OUTCOMES.md:16547–16565` requires primary initiation, not cancellation by a later button. Mixed-input navigation
acceptance and timing filters do not fully match a held pointer overlap that can last seconds. No exact Planned, BUGS or
queue match was found. Provenance: `terminal-links-contracts-1/f3`, discovery pass 1 in the trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `terminal-links-contracts-1/f3`.
