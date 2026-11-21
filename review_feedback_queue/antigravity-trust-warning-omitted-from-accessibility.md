# The Antigravity trust switch leaves its permission warning out of the accessibility tree

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Screen-reader users can enable automatic trust without access to the visible permission and trust-list warnings.

## Details

settings/agent_trust.rs:205–243 defines the mechanism separately from the risk and trust-list replacement. The
Checkbox’s visible content at :418–443 places the paragraphs in settings/card.rs:103–110, where each is a bare
SharedString inside an ID-less Div. The native GPUI tree requires both a global element ID and an accessibility role at
vendor/gpui-pre/src/element.rs:445–460, and SharedString has no ID (vendor/gpui-pre/src/elements/text.rs:330–340). Those
paragraphs therefore add no accessible nodes. components/checkbox.rs:275–315 exposes the short switch title and gives
`with_hover_help` the tooltip. components/mod.rs:345–362 copies that tooltip to the accessible description.
agent_trust.rs:167–176 gives Antigravity only its mechanism text there, which includes ordinary folders but neither
warns about hooks/commands nor describes replacing the trust list. SPEC.md:1747–1754 requires trust-switch help to
describe the trust effect, and :1759–1763 defines Antigravity's per-launch consequence. Put the essential permission and
trust-list information in the switch’s native accessible description or expose a clearly associated and navigable
explanation. No later change in the pinned range connects the full warning to accessible metadata.

Confidence: confirmed by inspection of the app and native tree emitter. Exact spoken output was outside this pass.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l16`.
