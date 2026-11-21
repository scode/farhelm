# Container cleanup’s host identity can outlive the displayed confirmation

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A registered-container cleanup confirmation passes the displayed host number. Whether it can reach a different
installation after that number’s identity changes remains unverified.

## Details

The existing inline confirmation captures the displayed leftover row
(`crates/gui/farhelm-gui/src/settings/containers.rs:277–305`). The registered-container branch forwards only its host
number (`settings/containers.rs:684–696`). Both behaviors predate the Settings card change. The independent check
verified the captured row and the request shape, but did not establish a supported registration or replacement action
that makes the same host number identify a different installation while that confirmation stays actionable. The missing
premise is a shipped identity transition of that kind, which would be needed to establish a possible deletion beyond the
displayed target. Preserve a current installation or provider-generation check if that transition is supported.

Confidence: Unverified after investigation.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `recipeCards/l5`.
