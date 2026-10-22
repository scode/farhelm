# Startup selection read outlives intervening visits

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

Deleting the current session can return to an older session rather than the one just visited if a startup preference
response remains delayed through those visits and the deletion.

## Details

Suggested bucket: **other**, for navigation correctness. Confidence: **confirmed conditional source mechanism**; adverse
scheduling was not reproduced. No destructive operation, forwarded typing or user-work loss is established.

Let startup's pre-pick GET capture history `[D]` and delay its response. Manually visit A and await its successful save,
then visit B and await its save, then delete B. Navigation remains allowed while the read is pending
(`crates/farhelm-ui/src/list/view.rs:2914–2925,3336–3341`). Deletion clears the matching pane (`2095–2127`;
`lib.rs:1834–1838`). The old request still owns the resolving flag (`view.rs:2971`), preventing a new removal-time read.

Its eventual completion sees an empty selection and idle operations (`2981–2987`), installs its old snapshot
(`2993–2995`) and opens D (`2997–3012`). The helm can already hold `[B,A,D]`, making A the eligible previous visit.
Every selection PUT may have succeeded; this differs from the queued omission of coalesced intermediate writes. Reject
pre-pick results after intervening selection lifecycles and obtain a fresh read when selection becomes empty.

Possible cover: `SPEC_impl.md:308–318` describes the current-selection gate, but does not expressly accept this older
lifecycle after a later removal. `SPEC.md:1765–1775` requires shared visits immediately before the pick. Ordinary
preference-write ordering acceptance (`SPEC_impl.md:240–256`) does not settle a sequence awaiting all acknowledgments.
The existing removal regression stages its read after visits settle (`e2e/tests/sidebar.spec.ts:4230–4245`). The timing
filters do not fully match a wrong-session selection, and the delay need not be subsecond. No exact other coverage was
found. Proposed drop: none; retain the specification ambiguity.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, discovery `g068/f3`.
