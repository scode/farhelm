# Claude Stop size limit retains a vendor-payload source gap

Reviewed commit: 3e23099d37a9cab5d893d1ec052b13042cc77f74

## TLDR

A Claude Stop callback above the hook's input limit is rejected. Whether supported vendor payloads can reach that limit
and lose needed tracking information remains unestablished.

## Details

`crates/farhelm/src/hook.rs:156–160,316–327,391–417` rejects the whole payload above 65,536 bytes before parsing. The
fixture at `739–743` contains `last_assistant_message` but uses `DONE`, establishing no vendor size or truncation
policy. Ordinary successful SessionStart already selects the same conversation; refusing its later Stop does not erase
that binding. Stronger consequences additionally need a distinct supported lifecycle and, where relevant, a hooked
unmarked nested producer.

Suggested bucket: `other`. Confidence: input rejection confirmed; vendor-production and consequential source premises
missing, not merely unmeasured frequency. Possible cover: the Grok-specific decision at trailing-tip
`TRIAGE_OUTCOMES.md:2801–2812` does not cover Claude. `SPEC.md:2211–2218` accepts continuing with stored capture after
tracking stops, while leaving an underlying tracking bug actionable. The conditional cap lead is retained without
asserting that derivative Resume behavior is a separate work-loss bug. Provenance: `g084/f1`, `a013`. Marked-child
callbacks are separately rejected before writing. No vendor turns, tests or reproductions ran.
