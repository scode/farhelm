# Launcher report confuses applying templates with launching

Reviewed commit: 56a8d94b7fc38b42bf8c6ab49a7174b2070bcf04

## TLDR

The landing report says Enter launches queued sessions from the launcher, although Enter there applies templates to the
draft without starting them. This misstates the behavior the change preserves.

## Details

Suggested bucket: **other**. Confidence: **confirmed documentation inconsistency**. No runtime defect, security
consequence or loss of user work is established.

`plans/reports/launcher-enter-feedback.report.md:119–125` describes the old and new sequence behavior as launching. At
the same revision, `SPEC.md:797–810` says Enter in New, Clone and Replace with applies queued templates without
launching. `e2e/tests/launcher-template-shortcuts.spec.ts:138–193` names that behavior and checks the resulting draft
and preserved search. The report should explain application without accepting the search-name fallback.

Possible cover: none identified; no exclusion is proposed. This is a maintainer-facing historical report, not a claim
that public instructions caused an unintended launch.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g022/f1`.
