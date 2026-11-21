# Quitting during retries can keep a retired Suspend timestamp

Reviewed commit: 405b3a3dcec6216ed9252d7a5128de649eab8530

## TLDR

Quitting during supervisor connection retries can preserve an old Suspend time that Farhelm already knows is obsolete.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source, without runtime observation. The actor stores Running evidence before dialing
(manager.rs:4854–4857,4887–4894). A failed dial stays inside the active ladder across later waits and attempts (lines
4502–4589). The clear is not scheduled until the entire connect phase returns (lines 4187–4198,4897–4912). Quitting
aborts the actor without flushing its evidence (lines 3564–3604,3608–3616). A previously stored timestamp can therefore
survive an ordinary quit, then be shown for a different pause if the sandbox suspends before that actor is recreated.
SPEC.md:340–347 and SPEC_impl.md:6723–6727 require a Running answer to retire strictly older stamps.
SPEC_impl.md:6777–6781 accepts a failed clear followed by restart, but here the clear was never dispatched despite a
Running answer and an already failed individual dial. Scope is the avoidable gap across the retry ladder, beyond a
storage error or the shutdown race of a just-dispatched asynchronous write. Estimated bucket other, wording only.
Possible_cover: the stated failed-clear limitation, with that distinction. The assigned follow-up
84bf819d21f41e8a8b808b8cf90aa1ccc44981a1 only edited comments. The six configured backoffs span sixty seconds even
before dial latency (manager.rs:105–125), and the embedded desktop helm uses the terminal actor shutdown after a normal
stop (lib.rs:1518–1534). The updated test waits for Connected before checking durable clear (manager.rs:14673–14692), so
it cannot distinguish dispatch after one unsuccessful attempt from dispatch after the entire ladder. The relevant
manager, clear predicate, and shutdown behavior are unchanged at pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.
Synthesis: A Retry or lifecycle hold nudge normally returns Interrupted from the ladder
(manager.rs:4520–4528,4554–4567), so dispatch is reached before the actor parks, even if the settled result is later
discarded (4197–4202). That is different from shutdown, which aborts rather than nudges. A completed fallback dial that
returns Connected also settles the ladder immediately (4869–4874,4569–4585). The long window in this finding therefore
covers positive provider Running followed by ordinary failed or timed-out dials, not completed fallback connections. F11
separately covers successful settlement with a stale row that suppresses dispatch. F12 covers cancellation after
dispatch, within the documented best-effort boundary.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tensorlake/f5`.
