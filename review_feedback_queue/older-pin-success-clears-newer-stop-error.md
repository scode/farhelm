# An older Pin success clears a newer Stop failure

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

A Stop failure or partial-cleanup notice can disappear from its row and open-session view when an independently
requested Pin finishes later, even though Stop was never successfully retried.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified conditional source/result-ownership defect**, without
runtime reply-timing evidence. No security, data or process-loss consequence is claimed. Reviewed change:
`bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; references below use the reviewed tip.

Start Stop A, mark that pending row and choose bulk Pin while A is unpinned. Bulk metadata checks Hosts, ordinary row
questions and Rename, not the pending lifecycle set (`crates/farhelm-ui/src/list/view.rs:4366-4406`); marking offers Pin
for the drawn unpinned row (`marking.rs:230-238,354-419`). Let Stop's error or partial notice arrive before Pin's
already-dispatched success. Stop stores `Stop: ...` at `view.rs:2520,2528`. Pin's success unconditionally removes the
session entry (`4054-4072`) from that same `operations.errors` map (`1120`).

The row reads this map (`5936-5937`), and the open view derives and renders its Stop error from it
(`session_view.rs:705-712,2759-2763`). No separate retained Stop result restores the notice after the Pin removal, and a
successful listing refresh cannot recreate a client-side request failure. `api.rs:3768-3783` awaits an independent Pin
request with no error/result identity tied to that lifecycle failure.

The single error slot and Pin cleanup predate the change. The reviewed patch removes bulk metadata's global pending
exclusion and keeps marking/menu use available while Stop waits, exposing this overlapping route directly. A deliberate
later Stop retry clearing its own error is not this sequence. Keep a Pin result from clearing an error it does not own,
whether through per-attempt ownership or distinct action result state.

SPEC.md:1846-1848 requires a failure, unknown outcome or partial result on the source row and open-session view;
`4241-4242` requires an actionable error. Pin success is not a Stop acknowledgement. Existing notice Retry/Clear and
credential-error display items involve different owners/selection rules. No exact accepted limitation, Planned item,
pending plan, known bug, queue, current-ledger or clear filter cover was established. Independent reply ordering alone
does not establish the rare-trigger prerequisite for filtering lost diagnostics.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g010/l13`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. The separate `g010/c11` refutation of destructive metadata bypass remains
valid and did not certify diagnostic retention. Proposed drop: none. No runtime test ran.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): view.rs:2735-2739 still stores a Stop
notice in shared errors, while on_pin at :4345-4359 removes the same entry on success. bulk_metadata at :4688-4736 still
admits Pin independently of pending lifecycle work; session_view.rs:691-698,2770 still derives and displays Stop errors
from that map. Added environment and retired-session checks do not fence an older Pin success against a newer Stop
failure. The notice-retry item concerns another result owner. The conditional reply-ordering defect remains
source-confirmed; runtime frequency and work loss are not claimed.
