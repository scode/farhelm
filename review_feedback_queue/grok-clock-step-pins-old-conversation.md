# A backward clock step pins a Grok session's Restart to the conversation the user left

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If a Grok session's host clock ends up behind the time Farhelm recorded for the conversation it last selected (a host
that rebooted with a wrong hardware clock before time sync, a VM restored from a snapshot, or a clock that ran ahead and
was then corrected), every later `/new` conversation in that session is refused until the clock catches up. Farhelm
keeps offering Restart into the earlier conversation, so Restart reopens, and appends to, a conversation the user is no
longer working in.

## Details

Found by the 2026-10-22 prioritized persistent review, group `205-harness-non-first-class-1of2` (finding f1). Proposed
for drop at recording; the independent check found the spec coverage ambiguous and asked for it to be recorded.

Mechanism: `GrokLocator::merge_report` (`crates/farhelm-supervisor/src/agent_kind/grok.rs:146-212`) orders
`SessionStart` reports only by the vendor callback's RFC 3339 `timestamp` (`EventTime`, `grok.rs:28-80`). A different
UUID is accepted only when strictly newer (`grok.rs:169-174`); otherwise the report is refused with a conflict
(`crates/farhelm-supervisor/src/service/core/vendor/grok.rs:168-169`), and the durable row keeps the previous UUID and
its `resumable` flag. The stored `selected_at` survives supervisor restart and Restart: ordinary relaunches keep the
captured conversation, including `selected_at`, across generations (`crates/farhelm-supervisor/src/store.rs:4144-4169`;
only named Goose starts clear it), and a `load` `SessionStart` for the same UUID with an older time returns `previous`
unchanged (`grok.rs:176-178`). So the fence's high-water mark persists across reboots. The timestamp is taken verbatim
from Grok's hook payload `timestamp` field (`crates/farhelm/src/hook.rs:628-656`).

Scenario: a Grok session selects conversation A at 10:00 by the host clock. The host reboots and comes up at 09:00. The
user Restarts (A resumes; its older `load` report is ignored, which is harmless), then runs `/new` at 09:05. B's
`SessionStart` is older than A's stored 10:00 key, so it is refused, and A stays resumable. The user works in B; the
next Restart resumes A. Every further `/new` is refused until wall time passes 10:00.

Why the spec coverage is ambiguous (the independent check's reasoning): SPEC.md "Durability and resume" (around
2250-2252) says "another UUID must be strictly newer ... Equal or older competing selections fail closed, including
after supervisor restart, without a general event log or clock-recovery protocol." That accepts the ordering design. But
elsewhere in the same section "fail closed" means no Resume is offered (around 2269, 2279 and 2298: for example "its
resume offer withdraws — fail closed, not a silent fresh start"), while here the old conversation stays resumable and
Restart appends to it. The only text that accepts keeping the previous conversation is the delivery race at around
2259-2261 ("If Grok exits or crashes before the replacement `SessionStart` callback arrives, the previous UUID can
remain Farhelm's last known selection"), which is a different trigger. SPEC.md "First-class harnesses" (around
3445-3450) keeps resuming the wrong conversation in scope for non-first-class harnesses, except for "a resume they
accept" under Durability and resume, so the item turns on whether "fail closed" above accepts this outcome.

Uncovered remainder: after a backward clock step, every `/new` is refused until wall time passes the stored timestamp,
and Restart resumes the conversation the user left instead of withdrawing Resume.

How to verify: a unit test of `merge_report(Some(A with selected_at = T), B with selected_at = T - 1h, "SessionStart")`
returns an error; then confirm the stored row still offers A.

Fix sketch (the reviewer's): when a competing `SessionStart` arrives from the admitted foreground process (the corridor
proof already establishes that), let an older timestamp withdraw Resume (set `resumable = false`, or clear the
selection) instead of leaving the old UUID resumable, so the failure is no offer rather than the old conversation.
Alternatively, reset the fence on each launch generation so only ordering within one process is compared.

Suggested bucket: highest (resuming a conversation other than the session's current one), with low likelihood.

Confidence: likely. Confirmed from code: an older-timestamped competing `SessionStart` is refused and leaves the
previous resumable binding in place, and the timestamp and its high-water mark persist as described. Unverified premise:
that Grok stamps `timestamp` from the host's wall clock at emission time rather than from a monotonic or server-side
clock.

Possible cover, for triage to judge: SPEC.md "Durability and resume" (around 2245-2261, the Grok ordering rule and the
accepted delivery race); SPEC.md "First-class harnesses". FILTER.md "Rare edge cases in harnesses without first-class
support" would match on rarity but excludes "resuming the wrong conversation".
