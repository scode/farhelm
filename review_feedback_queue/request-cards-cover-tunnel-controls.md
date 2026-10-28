# Request cards can cover tunnel controls

Reviewed commit: 4a99d51e560d873d8b2a8d25999926b2083507c5

## TLDR

A waiting agent request can cover the opened tunnel list and prevent pointer access to overlapping close controls. Exact
native overlap geometry remains unmeasured, and specification coverage is ambiguous.

## Details

Suggested bucket: **other**. Confidence: **confirmed conditional layering mechanism**; no rendered overlap or occurrence
rate was measured. Keyboard controls remain present. No unintended approval, security bypass or work loss is claimed.

The popup in `crates/farhelm-ui/src/tunnels.rs:910–938` uses both `host-row-menu-panel` and `session-tunnel-panel`.
`assets/app.css:2603–2623` gives it layer 20, while `8549–8556` positions it below the pill without reserving layout
space. Approval cards occupy layer 45 (`7829–7847`); `assets/approval-layout.js:19–43` positions them below the terminal
top without accounting for the floating list. Inspected shell, main-pane, layout, header and pill ancestors do not
isolate those layers. The previous in-flow strip occupied space that the same card geometry cleared.

An open header confirmation also lowers the floating tunnel error to layer 25 (`app.css:8596`), below those cards. That
related diagnostic may be concealed then; without the header confirmation, the reverse overlap is separately recorded in
`tunnel-error-covers-approval-disclosure.md`. The existing `header-confirmation-lowers-link-refusal.md` concerns a
different notice and remains unchanged.

Possible cover: this commit narrows `SPEC_impl.md:6002–6011` to clearance from in-flow notices. That describes the new
geometry but does not explicitly accept inaccessible close controls; `SPEC.md:2091–2096,2734–2735` retains close
controls and ordinary-size usability. The report's overlap observation is not proof that authorization was missing. The
dialog-scroll filter covers a different consequence. No exact Planned item, unlanded plan, BUGS entry, queue item or
ledger acceptance was found. Retain the specification ambiguity for triage. Introduced here and unchanged through
`d64ae3a81547729100529d210d3ea50434ff933d`. Proposed drop: none.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `tunnel-pill/f3` and its related diagnostic
candidate. Source review only.
