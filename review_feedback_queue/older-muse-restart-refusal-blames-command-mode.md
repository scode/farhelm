# Older Muse Restart refusal blames command mode

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Plain Restart can describe an older Muse agent launch as a command launch with reporting disabled.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The older launch lacks Resume command data, but the diagnostic infers the Command opt-out explanation rather than the
actual compatibility state. Exact intended older-session missing-Resume behavior remains accepted. Filter coverage for
this ordinary older-session diagnostic remains ambiguous; no wrong conversation or lost transcript is claimed.

Source locations: `crates/farhelm-ui/src/session_view.rs:3355-3358`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d. TLDR: opening an older Muse session created through the agent
launcher gives plain Restart an explanation saying this was a command launch that did not opt into Resume. The actual
compatibility reason is that the older agent launch retains its original missing resume command. Confirmed source route:
IntegrationSnapshot::of takes the stored launch's agent kind and resume argv (agent_kind/mod.rs:1825-1833); Muse has an
integration (:831-840); restart_offer emits NoResumeCommand whenever that integrated launch has no resume template
(:1855-1860). Session reply projection recomputes and exposes that offer (service/status.rs:377-382,412-425); the UI
decodes it (farhelm-ui/src/lib.rs:408-434), builds plain Restart's description and continuation from it
(session_view.rs:1885-1907,3180-3211), and offer_clause unconditionally identifies a command launch (:3355-3358). Its
interrupted-card caller shares the same clause (:3427-3469). SPEC.md:2691-2695 and :1384-1387 expressly preserve the
older Muse agent-launch route. The unit test at supervisor agent_kind/mod.rs:3966-3994 independently witnesses the
no-resume offer for Muse. No runtime test was run. No producer/scheduling premise remains missing for the diagnostic
mismatch. Actual action consent remains separate: Restart stays unavailable and no operation is redirected. Suggested
bucket: other. Confidence: confirmed. proposed_drop: none. possible_cover: SPEC.md accepts missing Resume until
recreation, not misidentification of the launch kind. SPEC_impl.md:5945-5958 and the protocol enum comment at
lib.rs:1326-1329 still describe NoResumeCommand as command-launch-only; these are stale narrower explanations rather
than explicit acceptance of the new incorrect UI statement. Coverage checks: TODO.md's Planned bucket is empty (:23-25);
BUGS.md has no matching entry; queue INDEX and queue text searches have no matching item; TRIAGE_OUTCOMES.md:17072-17084
concerns the README/Overview's broad reboot-resume promise, not this message. The landed Muse plan's :119-123 requires
accurate no-resume wording and :518-519 repairs Restart with only, not this plain Restart clause; no unlanded matching
plan found. FILTER.md rare-diagnostic and non-first-class filters do not fully match an ordinary retained older Muse
launch without another rare/unconfirmed condition; the upgrade filter requires an additional rare condition, absent
here. Provenance: the clause existed in assigned commit 2447296f34f256f239a36e5a6faecec5ca028f1d:3317-3320; Muse
integration later gives it a concrete additional caller. It persists at
e84f6b110543cf6848d041a76562bd94de44c5d0:3324-3327 even while that commit separately fixes Restart with at :120-124.
Retain as a cross-group caller/wording interface finding, not a claim that sentence casing created it. Tests: UI
offer-text test at :4283-4417 exercises offer/status text with an agent-kind enum but no launch kind; the Muse
unavailable test at :3713-3721 checks Restart with only, so neither pins plain Restart's description for this older
agent launch.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g058/f1`, `g093/f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
