# The launcher subscription regression masks missing observers

Reviewed commit: 8a55d56321f6b86181a7c8d845d6aca3bb9c907a

## TLDR

A launcher regression can pass after losing one of the subscriptions it claims to protect. Its two replay branches share
an effect, and its positive control changes several other observed fields. No production subscription defect is
established.

## Details

`final_owner_search_keeps_conditional_agent_subscriptions` in
`crates/farhelm-ui/src/list/create_form/draft.rs:3697–3737` calls both Harness and Model replay inside one effect. Both
observe the same agent fields (`2378,2394`). Removing that observation from either branch leaves the effect subscribed
through the other, so the checks cannot detect the omission.

The selected-agent positive control switches a Codex-owned fixture model to Claude (`3770–3777`) after setting Custom
permissions, flags and a wrapper. Reconciliation also clears the model (`draft.rs:735–768`;
`crates/farhelm-proto/src/launcher.rs:146–158`), clears cross-agent Custom permissions/flags (`launcher.rs:98–104`) and
replaces the wrapper (`draft.rs:763–765`). Those surviving subscriptions can satisfy the increased run-count assertion
at `3801–3808` without the intended harness subscription.

Suggested bucket: **other**, test oracle only. Confidence: **confirmed static masking mechanism**; no mutation or
runtime reproduction was performed. Give each replay branch its own effect, and use a harness-change control that keeps
other observed fields equal, asserting that premise before processing events.

Possible cover: none found. `SPEC.md:575–616` and `SPEC_impl.md:790–828` specify reconciliation rather than accept this
test gap. Existing model-effort and retry-key test records concern different missing observables. Planned is empty; no
matching BUGS, ledger or filter entry was found. Proposed drop: none.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g049/f1`; neighboring `g048` accepted the oracle correction in its discovery
turn.
