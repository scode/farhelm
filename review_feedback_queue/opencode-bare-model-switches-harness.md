# Entering a supported bare OpenCode model silently switches the launch to Codex

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Pressing Enter on a supported bare OpenCode model can silently select Codex, so the session starts with a different
agent and configuration.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F2 / COR-ZEN-HARNESS`, reviewer `correctness_general`, pass 1. Confidence: **definite**. Review disposition: **would
fix**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-ui/src/launch_composer.rs:339`. Recorded from the completed review
without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

With OpenCode selected, entering `gpt-6-luna` in the custom-model field and pressing Enter without navigating the
suggestions selects Codex instead. The keyboard handler compares the entered text directly against catalog IDs. It finds
the single bare entry owned by Codex; OpenCode's entry is named `opencode/gpt-6-luna`. The form then applies the catalog
entry's owner and changes the selected harness. Its preference for the currently selected harness only helps when that
harness already has an exact matching catalog ID.

Someone using OpenCode's documented bare-name spelling can therefore launch a different agent, with different
configuration and credentials, unless they notice the changed selection. Fixing server validation alone cannot prevent
this: the browser has already changed the request to Codex. Resolve bare names under the selected OpenCode provider
before attempting automatic selection across harnesses, and use that normalized identity consistently for compatibility
checks and selection reconciliation. Cover Enter on overlapping bare names with OpenCode selected, while retaining
intentional cross-harness selections.

Additional original anchor: `crates/farhelm-ui/src/list/create_form.rs:2570` (`apply_model_option`).
