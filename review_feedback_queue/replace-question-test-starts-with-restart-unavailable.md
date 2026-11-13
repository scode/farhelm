# Replace exclusion test begins with Restart unavailable

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

The Replace-question regression cannot detect loss of Restart exclusion: its source session already has Restart
unavailable before the question opens. The other tested controls retain their independent coverage.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified test assertion gap**, without a runtime mutation test
or a claimed production exclusion defect. Reviewed change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; the affected
source remains at `e2e/tests/replace.spec.ts:361,394-395` at the reviewed tip.

The source uses `createSession(request, { title })`. `helpers/fleet.ts:237-245,408-435` supplies a command launch
without an agent declaration or Resume command. SPEC.md:1871-1878 offers Restart only for a declared resumable agent
with a captured conversation. The header derives `restart_available` from that offer and sets `aria-disabled` when it is
absent, independently of native `disabled=lifecycle.busy()` (`session_view.rs:2079,2438-2464`). Playwright's
`toBeDisabled` accepts that pre-existing ARIA state. Removing the lifecycle gate would therefore leave the assertion
passing.

Use the existing resumable fixture and establish Restart enabled before opening the question, disabled while it owns the
source, then enabled after release. If the intended assertion is specifically native busy presentation, assert that
property rather than accepting unrelated ARIA unavailability. No exact specification acceptance, Planned item, unlanded
repair, BUGS, queue, current-ledger or filter cover was established. Nearby old-running-version, Restart wording and
closed-field Enter items test different predicates.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g012/l5`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. No current product bug or user-work loss is alleged. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): replace.spec.ts:360-362 still creates an
ordinary source without resumable agent state, and :394-395 still includes .restart-primary in toBeDisabled. Its
baseline aria-disabled condition already satisfies that assertion before the Replace question. Later terminal-marker
edits do not supply the missing positive availability premise; this remains a source-confirmed test-premise gap.
