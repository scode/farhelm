# Held activity turns a session the user already saw blue (unseen) when the held stamp is published later

Reviewed commit: 7cf83b9d60139588cc2cc999a9d6879951f7b965

## TLDR

A user opens a working agent, watches it finish, and switches to another session a few seconds later. When the agent is
then recognized as idle, its row turns to the blue 'new output' dot, even though the user saw every line. Before this
change the same sequence left the row seen. With a large fleet the window is long: idle recognition needs several
samples of that session, each one sampler rotation apart, so it can come tens of seconds after the last output, and
minutes at the 500-session scale this change targets (lore/2026-11-26-desktop-idle-cpu.md:98 measures each session
sampled about every 62 seconds there). The same happens after a manual Mark read of a running row, and after a mute ends
(SPEC.md promises only later activity can make it unseen again).

## Details

Every seen stamp is a copy of the PUBLISHED activity stamp: the open session's automatic mark
(crates/gui/farhelm-gui/src/session_list/selection.rs:265-284 records `row.info.effective_activity()`), the manual Mark
read (crates/gui/farhelm-gui/src/session_list/seen.rs:101-106), and the helm's mute expiry
(crates/farhelm-helm/src/session_mute.rs:41-55, `unmute_session` with the observed listing). Before 7cf83b9d60, a sample
whose output crossed the one-minute quantum moved `last_activity_at` and hinted at once (old `note_activity`), so the
open client re-recorded seen at that value; later output inside the quantum never moved the stamp, and the idle sample
dates nothing, so a session watched to the end stayed seen. After the change, a crossing only fills `held_activity_at`
(ticker.rs:1646-1662 `hold_activity`, with `activity_to_hold` at ticker.rs:1673) and every later dated sample overwrites
it with the newest observation time (`activity_to_hold`). It is published only by a hinting sample (status change,
permission change, work start; ticker.rs:1489-1495, through `publish_held_activity` at ticker.rs:1690), the
ended-session path (`publish_held_for_ended`, ticker.rs:1079), or the fleet flush (`flush_held_activity`,
ticker.rs:1766-1795). So the seen stamp recorded while the user watches stays at the older published value P. If the
user leaves after the last output F but before the sample that reads the session as idle, that idle sample publishes F >
P (`publish_held_activity`); the session is no longer open so nothing re-records, and SPEC.md 'Status' ('UNSEEN whenever
its most recent activity is newer than that recorded stamp') makes the idle row blue for output the user saw. The same
publish of an observation older than the mark defeats a Mark read on a running row and a mute that ends while held
activity is pending, contradicting SPEC.md 'Status': 'When a mute ends, including by expiry, the helm marks the session
seen through its current activity. Only subsequent activity can make it unseen again.' The premise only needs a crossing
since the last publish, which holds for a session that has worked more than a minute past its last publish (the flush
runs at most once a minute fleet-wide). With more than SAMPLE_TAIL_BUDGET (16) live sessions the gap between F and the
idle sample grows with the rotation period (live/16 ticks per rotation, several samples to settle into idle).
SPEC_impl.md's new paragraph ('Last-activity timestamp') says 'the unseen dot only idle rows show, are not delayed',
which is about delay; it does not address this false-unseen case, and nothing in SPEC.md accepts it. Verify by source:
follow a session through hold (no publish) while open, then the idle sample's publish after the client's selection
moved. Fix directions: publish a session's held value (with its hint) whenever a seen mark is taken for it, e.g. a
supervisor-side 'publish held now' when the helm records seen or a client opens the session; or have seen marks carry
the wall time of the mark and treat a published observation at or before it as seen.

Reviewer revision notes: Revision 3 (synthesis): corrects tip line numbers and confirms the GUI takes no seen mark when
the user leaves a session (selection.rs:265-286 marks only the open session). Revision 2 added the 500-session window
length; revision 1 added the Mark read and mute-end variants.

### Triage notes

- Recorded bucket: `high`. Bucket as the reviewer suggested; it could be argued other if a spurious new-output dot is
  judged cosmetic.
- Suggested bucket (reviewer): high: a moderate, common failure in the attention signal users rely on to find sessions
  that need a look; not data loss. Could be argued `other` if the maintainer considers a spurious blue dot cosmetic.
- Confidence: likely: the mechanism is established from source on both sides (the sampler holds; every seen mark records
  the published stamp); the unverified premise is only how often users leave inside the window.
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g27/f1 (finding), group g27 "Publishing working sessions' activity stamps once a minute", findings log line 6.
