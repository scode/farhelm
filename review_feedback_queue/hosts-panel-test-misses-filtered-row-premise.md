# Hosts preservation test never witnesses the hidden row

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

The regression claiming to preserve a selected terminal while its session row is filtered out can pass without ever
establishing that the row disappeared.

## Details

Suggested bucket: **other**, for test coverage. Confidence: **confirmed missing boundary witness**. No production
selection regression or runtime mutation result is claimed.

`e2e/tests/hosts-panel.spec.ts:25–30` selects Waiting and immediately opens Hosts. `helpers/list-header.ts:9–13` waits
only for the local radio to be checked, without waiting for the listing or checking that this session's row is absent.
Later assertions inspect controls, the terminal DOM lifetime marker and its existing revealed attachment (`60–72`). A
delayed listing, or a no-op status filter, can satisfy them without exercising exclusion. The omission remains at pinned
tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

Assert that the selected session's row is absent while Sessions is still visible, after choosing Waiting and before
opening Hosts. Retain the terminal lifetime marker and session-specific readiness checks. There is no missing source
premise for the absent witness; delayed-reply frequency was not measured.

SPEC.md:1478–1492 requires preservation through filtering and panel swaps; SPEC_impl.md:670–675 requires retained
owners. The landed sidebar plan's general coverage requirement is not a pending repair. Searches found no exact Planned,
unlanded-plan, BUGS, queue, ledger or filter coverage. Possible cover: none. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g016/g016-f2`.
