# Floating tunnel error can cover approval facts

Reviewed commit: 4a99d51e560d873d8b2a8d25999926b2083507c5

## TLDR

A floating tunnel error can paint over an agent request's approval facts while its answer handlers remain enabled. A
concrete harmful approval layout is **unverified after investigation**.

## Details

Suggested bucket: **other** for the established display concern. Confidence: **conditional layering confirmed; harmful
acceptance unverified after investigation**. A supported layout hiding material facts while an answer remains usable,
and an action exceeding the visible disclosure, would warrant reassessing this as `highest`. Neither was demonstrated.

`crates/farhelm-ui/assets/app.css:8568–8596` puts the closed-list error at layer 45, header-bottom plus 6px, right 10px,
with width up to 480px. Approval cards also use layer 45 (`7829–7847`) and begin 12px below the terminal top with width
up to 660px (`assets/approval-layout.js:19–43`). Cards precede the shell in DOM order (`src/lib.rs:1842`), so the later
error paints above them. The inspected shell, main-pane, layout, header and pill rules introduce no intervening stacking
context. This concerns the state without a header confirmation; that confirmation lowers the error beneath cards.

`src/approvals.rs:516–592` renders requester/action facts before answer controls. Admission (`296–342`) checks arming
and duplicate answers, not whether those facts are covered. `data-link-cover` protects terminal links
(`assets/terminal-files.js:23–43`), not card admission. The error persists until dismissal or a later successful action.
The old per-session error was in flow; the preexisting global tunnel notice is bottom-anchored, so neither is the same
introduced top-overlap mechanism.

Investigation inspected the exact geometry rules, ancestor stacking, DOM order, card contents and handlers, and error
lifetime. What remains missing is a concrete supported native content/window arrangement where material facts are hidden
and an answer remains reachable, together with harmful acceptance beyond that disclosure. Source inspection cannot
settle actual native wrapping and hit testing. A focused fixture or ordered native observation would establish that
premise; no UI execution or reproduction ran in this review.

Possible cover: `SPEC.md:2111–2113` and `TRIAGE_OUTCOMES.md:15917–15931` accept a link-refusal banner covering card
context while the pointer stays on a link. A persistent tunnel error has a different trigger and lifetime, so this is
not complete coverage. `SPEC_impl.md:6002–6011` promises clearance from in-flow notices only, without explicitly
accepting hidden approval facts. No exact Planned item, unlanded plan, BUGS entry, queue record or ledger acceptance was
found. Introduced here and unchanged through `d64ae3a81547729100529d210d3ea50434ff933d`. Proposed drop: none.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `tunnel-pill/f4`. Separate from cards covering
tunnel controls in `request-cards-cover-tunnel-controls.md`.
