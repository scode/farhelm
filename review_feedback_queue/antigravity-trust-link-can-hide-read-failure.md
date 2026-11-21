# The trust hint’s Settings link leaves an Antigravity read failure below the fold

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

When the trust setting cannot be read, the hint’s Settings link can leave the unavailable switch and its error below the
fold.

## Details

SPEC.md:2444–2449 requires the trust hint entry point to open at the relevant switch and reveal its block.
settings.rs:317–331 starts a targeted handover, but :969–1006 only reveals the target by focusing its handle, which
exists for handover only when the trust read succeeds. A refusal leaves the control unavailable
(settings/configured.rs:90–116) and places the error after that control (settings/agent_trust.rs:432–456). The pinned
Antigravity entry-point test at settings/tests.rs:2358–2399 establishes a viewport where the target starts below the
fold and only comes into view after successful handover. The refusal source test at :2270–2285 covers Claude, whose
switch starts at the top, and does not exercise this trace. No alternate scroll/reveal is wired on the failed trust
read: trust_known stores the result, tries the same focus handover and notifies. The documented deviation accepts
revealing only the switch when the handover succeeds (docs/gpui/parity/settings-and-hosts.md:1554–1562). It does not say
that the refused result can be left outside the viewport. Leave focus on the dialog or a valid control when the switch
is unavailable, and reveal and expose the target's failure independently of switch focus.

Confidence: confirmed by inspection, under the documented viewport premise and a returned read failure.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l18`.
