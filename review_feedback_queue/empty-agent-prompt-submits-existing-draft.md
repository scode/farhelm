# An empty agent prompt can submit an existing draft

Reviewed commit: 782dd86ad358d735a64ab8f9a7c8a7bb1a4e0d15

## TLDR

An approved prompt with no text can press Submit on text already waiting in the agent's composer. The approval card does
not show that draft.

## Details

For an empty prompt, the supervisor sends bracketed paste markers around zero bytes and then a separate submit key
(`crates/farhelm-supervisor/src/service/core.rs:10638–10657`). The approval card shows the empty request and a general
warning about interleaving (`crates/farhelm-ui/src/approvals.rs:808–842`). If the target agent has a draft, the submit
key may send it without adding any approved text. Actual behavior depends on the agent's composer and was not reproduced
with a vendor agent.

Suggested bucket: highest, because an unseen draft could direct a consequential action. Confidence: likely on the input
sequence, with vendor behavior unverified. Provenance: trailing review of the stated commit. `SPEC.md`, “Agent-spawned
sessions,” allows an empty message and warns that prompts may interleave with existing composer text. An independent
coverage audit found no explicit acceptance of an empty prompt submitting the pre-existing draft. The delivery path
could reject an empty message or avoid sending Submit when no prompt bytes were added.
