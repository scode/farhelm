# Agent approval omits the workspace trust choice

Reviewed commit: d6abf445783e49711b5ad71c1a84e11fbed29d32

## TLDR

An agent-created session in a managed checkout can receive automatic workspace trust without a plain-language trust line
on its approval card.

## Details

The change removes the card's explicit workspace-trust row (`crates/farhelm-ui/src/approvals.rs:856–868`). The card
still shows the compiled command, which includes a trust flag or marker, and the setting is an explicit global opt-in. A
person approving the agent's launch must infer the trust effect from that command rather than from the card's own
explanation. The risk is mistaken consent to running repository settings or commands; no mistaken approval was
reproduced.

Suggested bucket: highest, conservatively, for the possible trust consequence. Confidence: likely presentation risk;
actual mistaken authorization is unverified. Provenance: trailing review of the stated commit. `SPEC.md`, “Managed
checkouts,” accepts the opt-in trust setting for agent-requested launches, but an independent coverage audit found no
acceptance of removing this disclosure from an approval card. Restoring a clear line for the effective trust choice
would make the card's consequence readable without parsing agent flags.
