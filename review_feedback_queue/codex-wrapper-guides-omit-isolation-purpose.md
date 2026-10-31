# Codex wrapper guides omit the isolation purpose

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The wrapper guides explain generated Codex arguments without explaining their role in keeping one session from sharing
another's daemon.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The guide explains reporting and consequences of dropping forwarded arguments, but omits the no-daemon isolation reason.
Generic forwarding prose and links provide context without stating that purpose. The accepted legacy exception and its
historical-release wording are separate. No harmful reliance on the omission was established.

Source locations:
`website/src/content/docs/docs/agents/custom-commands.md:50-53; website/src/content/docs/docs/agents/agent-wrappers.md:90-102`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery-turn disposition update at d6645f771bee8ed03a66486ef8755834937d759d, before new source analysis. Parent
reports fresh a062 could not approve wholesale exclusion despite the mandatory forwarding text and linked hook guide.
Retain the narrow confirmed omission: these passages explain the generated arguments as conversation reporting without
explaining Codex's additional --no-daemon isolation purpose. Suggested bucket other. Confidence confirmed for the
wording omission only. proposed_drop: none; earlier proposed exclusion was not approved. possible_cover: forwarding/link
text mitigates but does not eliminate the explanatory omission, and no exact accepted exception was found. No harmful
user reliance, security consequence, attack mechanism, or new runtime route is claimed. Original ID and revisions
preserved. This disposition is parent-relayed independent review provenance; no a062 private artifacts were consumed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g142/f2`, `a062/e2`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
