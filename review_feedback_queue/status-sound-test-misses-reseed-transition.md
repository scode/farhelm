# Status-filter sound test cannot detect missing reseeding

Reviewed commit: 604db033ba76823bb1f88f2c16e52024c1e5b6f8

## TLDR

The new sound regression can pass if changing the status filter stops silently resetting sound history. It does not
establish a production sound defect.

## Details

Suggested bucket: **other**, for test coverage. Confidence: **confirmed static discrimination gap**. No mutation test or
runtime reproduction was performed.

`e2e/tests/sounds.spec.ts:109–134` keeps its session Idle throughout All → Waiting (empty) → All (Idle).
`crates/farhelm-ui/assets/sounds.js:27–38` emits an event for newly visible Waiting or Running → Idle, but not absent →
Idle. Ignoring status-only changes in the sound-scope comparison at `src/list/view.rs:1718–1723` therefore preserves
every silence and approval assertion in this test. Production currently compares the whole filter correctly. At pinned
tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`, surrounding sound tests `240–378` cover host changes or initial
mounting, rather than closing this status-only gap.

Reveal a consistently controlled Waiting row through a status-only change and assert silence, then establish a later
audible transition within that view. No source premise is missing for the current test's inability to discriminate.

SPEC.md:1737–1746 and SPEC_impl.md:1192–1210 require silent reseeding. Searches found no exact accepted exception,
Planned item, unlanded repair, BUGS entry, queue item, ledger decision or filter. The older sound-test decision concerns
cleanup. Possible cover: none. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g007/g007-sound-status-seed-witness`.
