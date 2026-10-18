# A refusal banner can hide approval context

Reviewed commit: 7eeb5ff87c6821f85e0931411459dd537381167c

## TLDR

A refused terminal link can show a banner above an approval card and hide the requesting host or session while keyboard
approval remains possible.

## Details

The refusal banner stacks above approval cards (`crates/farhelm-ui/assets/app.css:1337–1362,7155–7157`). It can cover
the requester text (`crates/farhelm-ui/src/approvals.rs:410–420`) while the pointer stays on the link and the focused
approval button can still be activated from the keyboard. The landing report in
`plans/reports/covered-link-hover-refusal.report.md:98–101` acknowledges the overlap. The exact geometry and keyboard
action were not reproduced.

Suggested bucket: highest, because a person may authorize an action without seeing whose request it is. Confidence:
likely, with the overlap acknowledged in the report. Provenance: trailing review of the stated commit.
`TRIAGE_OUTCOMES.md` asked for a visible refusal above cards, and the report calls the overlap an expected trade-off. An
independent coverage audit found no explicit acceptance of hiding requester or host during keyboard authorization. The
banner could leave the card's authorization context visible or disable approval while covering it.
