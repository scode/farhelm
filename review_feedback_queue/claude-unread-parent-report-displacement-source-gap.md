# Claude unread-report displacement retains a nested-producer source gap

Reviewed commit: 3e23099d37a9cab5d893d1ec052b13042cc77f74

## TLDR

A hooked, unmarked nested Claude could overwrite an unread parent conversation-switch report before admission refuses
the child. An ordinary supported producer and complete consequential sequence remain unestablished.

## Details

Under `crates/farhelm-supervisor/src/`, Claude events share a slot (`agent_kind/mod.rs:2112–2125`); writing replaces it
(`hook_report.rs:306–357`). Rival admission leaves capture unchanged (`service/core/vendor/claude.rs:84–126`), then
settlement deletes the surviving report (`service/report_files.rs:246–265,520–524`). Restart drains before reading its
stored target (`service/core.rs:11312–11347`), but cannot recover overwritten evidence. Missing is an ordinary hooked,
unmarked nested reporter and the required ordering before correction. The vendor test explicitly supplies child hooks as
a stand-in (`crates/farhelm/tests/e2e/real_agent_capture.rs:580–612`); launch-local argv and inherited credentials do
not establish that producer.

Suggested bucket: `other`. Confidence: confirmed conditional composition with missing source/input premises. Possible
cover: `SPEC_impl.md:3129–3144`, retained at trailing tip `3137–3152`, expressly accepts unmarked-runtime slot
replacement followed by refusal; whole Claude tracking scope remains ambiguous beside its newest-report statement and
Grok example. Trailing-tip `SPEC.md:2211–2218` accepts continuing to resume recorded capture after tracking stops, so
that derivative outcome alone is not another bug. No ordinary wrong-Resume work-loss route is claimed. Provenance:
`g081/f2`, independently retained by `a011`. Failed-Restart pin clearing is a separate existing item. No vendor or
runtime reproduction ran.
