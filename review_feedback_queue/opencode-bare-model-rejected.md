# Bare OpenCode model names are rejected when another harness lists the same name

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Some documented bare OpenCode model names fail to launch, even though the same model works with an opencode/ prefix.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F1 / COR-ZEN-VALIDATION`, reviewer `correctness_general`, pass 1. Confidence: **definite**. Review disposition: **would
fix**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-helm/src/launches.rs:515`. Recorded from the completed review
without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

Farhelm promises that an OpenCode launch accepts either a bare Zen model name or its `opencode/` spelling, but server
validation treats those spellings differently. It checks that the name is valid for OpenCode, then looks up the original
text in the shared model catalog. For `gpt-6-luna`, `gpt-5.6-terra`, `gpt-6.1-sol`, and `gpt-6-astra`, the bare catalog
entry belongs to Codex. Validation consequently rejects the OpenCode launch as belonging to a different harness, even
though adding `opencode/` makes the same launch succeed. The existing bare-name test uses `private-zen-model`, which has
no competing catalog entry and misses this case.

Use the provider-qualified OpenCode identity when checking catalog ownership and available effort settings, while
preserving the user's original stored selection. Test both spellings of the overlapping models. This restores the
explicit bare-name contract in SPEC.md without changing which providers OpenCode accepts.
