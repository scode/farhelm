# Approval messages replace blank lines with labels

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

An initial prompt containing blank paragraphs acquires literal “(empty)” labels in its approval card. Copying that
displayed text copies the invented labels too, although the launched session receives the original prompt.

## Details

Suggested bucket: `other`, for display and copy fidelity. Confidence: confirmed static transformation and production
callers; no runtime reproduction. Provenance: retained `ui-app-state-1/f1`, second-turn synthesis. Proposed drop: none.
No changed execution, hidden instruction, wrong target or authorization bypass was established.

`PeerBlock` strips each line's terminator and calls the single-value renderer `display_peer` on that line
(`crates/farhelm-ui/src/peer.rs:255–273`). That renderer substitutes `(empty)` for an empty value at :62–63 and a
placeholder for whitespace-only values at :83. A supported prompt `first\n\nsecond` therefore displays as
`first\n(empty)\nsecond`. Launch prompts flow through this block at `approvals.rs:697–698,512–519`, and template prompts
at :906–909. Commands and resume commands also use it (:821–901,947–951).

Preserve structural blank lines in multiline text while escaping unsafe characters. Check rendered and copied text for
leading and consecutive newlines. The display output is not parsed back into the executed action. Provisioning uses the
same block, but its ordinary generated plans contain no blank lines; no normal provisioning trigger was found.

Possible cover: none found. SPEC.md:2339–2348 permits prompt newlines and requires the full escaped message;
SPEC_impl.md:4979–4988 specifies full multiline rendering. The ledger's “display-peer-misses-invisible-chars.md” fixes
identity spoofing through invisible characters, not alteration of structural blank lines. No matching Planned, BUGS or
current queue item was found. Ordinary supported multiline input is deterministic, so the rare-trigger filters do not
cover it.
