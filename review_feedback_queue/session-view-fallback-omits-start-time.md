# The promised session-view fallback omits start time

Reviewed commit: ee0d819ddf3a82ad6965b8b8f895ebe98bb1a61b

## TLDR

When a hover summary hides when a session started, opening the session does not reveal that time as the new fallback
promise says. Actual clipping and rendered geometry were not reproduced.

## Details

At this revision, `SPEC.md:1451` includes both start and activity times; added `1452` says the session view provides the
clipped details in full. `crates/farhelm-ui/src/list/summary_card.rs:66,143–146` renders start time separately from
`created_at`. The session view instead constructs its timestamp from `effective_activity()`
(`session_view.rs:2101–2104`) and displays it in the normal and stale details bars (`3265–3270,2870–2875`).
`activity.rs:189–197` calls it last activity; `lib.rs:621–634` uses creation only as an older-sender fallback. Ordinary
post-creation activity therefore hides the separate creation time from this proposed fallback. The complete
timestamp-reference search found creation only in session-view tests. Narrow the promise to available facts or
deliberately add start time.

Suggested bucket: **other**. Confidence: **confirmed missing source-level fallback; actual clipping unexecuted**. No
security or user-work loss was established. Proposed drop: none.

Possible cover: final `TRIAGE_OUTCOMES.md:16809–16823` accepts hover clipping and requests this fallback wording. It
does not expressly accept a missing timestamp in that fallback. This concerns a false alternative-route assurance,
rather than reopening accepted clipping. No exact Planned, BUGS, queue or filter match was found across the required
sources. Provenance: `spec-contracts-contracts-1/f5`, discovery pass 1 in the trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `spec-contracts-contracts-1/f5`.
