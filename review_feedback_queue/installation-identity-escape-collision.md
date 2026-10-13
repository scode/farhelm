# Different installation identities can look identical

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

Farhelm can show two different installation identities as the same text when asking whether to adopt a changed
installation, weakening the comparison the user relies on.

## Details

Suggested bucket: `highest`, conservatively for a misleading trust decision. Confidence: confirmed display-contract
violation by source inspection; no browser reproduction. Source: adversarial terminal/UI reviewer. Proposed drop: none.
This is not a demonstrated authentication bypass or secret disclosure: the adoption request still binds the correct raw
identity. The security severity assumes that an ambiguous comparison matters to the user's adoption decision.

`crates/farhelm-ui/src/peer.rs:119–132` renders a space as `<U+0020>` but preserves literal ASCII escape-looking text.
Consequently, `display_identity("left right")` and `display_identity("left<U+0020>right")` produce the same label. Both
mismatch evidence (`crates/farhelm-ui/src/hosts.rs:593–598`) and the adoption label (`2755–2760`) use this rendering.
The protocol limits an identity to 256 bytes without requiring UUID syntax (`crates/farhelm-proto/src/io.rs:552–555`);
no additional UUID validation was found along the helm admission path.

The trigger needs a previously recorded, peer-chosen non-UUID identity followed by a different identity containing its
literal escape spelling. Honest supervisor-generated UUID pairs do not collide this way. The general encoding defect
partly predates the range; `dd9573cc` (#1811) adds space escaping and this additional collision. Other escaped
characters and the empty-value placeholder deserve the same injectivity check.

Verify the pair above through the renderer, then through the changed-installation prompt. Encode escape delimiters and
empty values unambiguously so distinct raw identities remain distinguishable. SPEC.md:2333–2336 explicitly establishes
that display contract.

Possible cover: `TRIAGE_OUTCOMES.md`, `ascii-spaces-remain-invisible-host-identity-approval-labels.md`, addresses
whitespace collapse. Its trigger does not cover collision with literal escape text. No exact existing queue item or
accepted decision was found.
