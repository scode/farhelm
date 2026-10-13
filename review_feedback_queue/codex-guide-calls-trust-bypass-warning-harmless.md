# The Codex guide dismisses a trust-bypass warning

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

The Codex guide calls its hook warning harmless without explaining that the same bypass lets other unreviewed hooks run.
A reader could dismiss a warning that matters to their configured hooks.

## Details

Suggested bucket: `highest`, conservatively for security reassurance. Confidence: likely; the wording and bypass
behavior are established, but harmful reader reliance is unverified. Source: adversarial documentation reviewer. Its
proposed covered drop was rejected by an independent reviewer. This is a documentation finding, not a newly discovered
execution bypass.

`30b94e0a` adds the short guide's statement that Codex's hook warning "is harmless"
(`website/src/content/docs/docs/agents/codex.md:13–14`). That warning concerns the per-launch hook-trust bypass. As
SPEC_impl.md:2116–2138 explains, it also runs unreviewed hooks from the user's Codex configuration home and a trusted
workspace's `.codex/` configuration. The short guide does not qualify its reassurance or link to the detailed
explanation at that statement.

Possible cover: the bypass itself is deliberately accepted until explicit hook installation becomes available.
`website/src/content/docs/docs/how-it-works/agent-hook-injection.md:20–23,176–179` already warns about unreviewed home
hooks and explains how to disable injection. TODO.md also tracks fuller user documentation and explicit hook
installation. Those facts reduce the scope of this finding but do not explicitly accept unconditional reassurance in the
short Codex guide; the documentation TODO is not a Planned-bucket suppression.

Clarify which visible warning is expected and disclose or link to the configured-hook consequence. Preserve the accepted
integration behavior. Do not claim a demonstrated exploit or that an informed user is forbidden to accept the bypass.
