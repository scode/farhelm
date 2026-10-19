# The main session summary can still say guarded after YOLO is observed

Reviewed commit: 51fd667d28f3a7ffdec09c6b2f6ae3c2ad220bb6

## TLDR

A session's hover summary can say guarded after Farhelm has detected that its agent bypasses permission checks. The
permission shield remains accurate, but the summary does not explain the disagreement.

## Details

The card uses `menu_header_summary_parts` for its agent line (`crates/farhelm-ui/src/list/summary_card.rs:100–103`).
That helper reads stored launch permissions without consulting `session.screen_yolo`
(`crates/farhelm-ui/src/list/row.rs:222–232`). The permission mark's description does account for the observation
(`row.rs:565–580`). A guarded launch with observed YOLO therefore keeps an unqualified guarded label in the main card,
without identifying it as historical launch metadata. A focused rendering check should compare the two hovers for a
guarded Claude session with `screen_yolo=true`. The card could retain the observation and distinguish the stored launch
choice from the observed mode.

Suggested bucket: **highest**, conservatively, for possible misleading security assurance. Confidence: **confirmed
static display disagreement**; a consequential user decision is unverified. This changes no admission behavior, and the
shield itself still reports the observation. Provenance: same-model adversarial trailing review of the stated commit.
Proposed drop: none.

Possible cover: `SPEC_impl.md:625–633` explicitly describes the actions-menu summary as stored launch information;
whether that exception extends to the new main card is unclear. `SPEC.md:1406–1418` describes the observed-YOLO display
and its advisory limits. The landing report, `plans/reports/session-hover-text.report.md`, “Things to keep in mind,”
acknowledges this exact disagreement and recommends leaving it unless the card is authoritative; that is a report
recommendation, not a recorded maintainer decision. No matching Planned item, known-bug acceptance, queue item, ledger
decision or filter was found. Triage should settle the intended meaning of this card before choosing a fix.
