# A second Close pressed while a close still waits inherits the first press's refusal and delivery baseline, so it re-shows an old refused page instead of asking

Reviewed commit: ee8ef7294a6638d7905fca7dd27083d0d0bb45c9

## TLDR

Someone presses Close while two things are out. One is refused; Settings stays open waiting on the other. They act on
that refusal (or just look) and press Close again. When the second piece of work lands, Settings does not ask about
unsaved changes or close: it jumps back to the page of the earlier refusal, which the person already saw and may have
dealt with, and they must press Close a third time. The same happens with an earlier missed-hosts report: the second
Close lands on the Credentials overview instead.

## Details

wait_to_close (editing.rs:997-1011) keeps an existing PendingClose on a repeated Close or Escape and only replaces its
mark (`Some(pending) => pending.mark = mark`), keeping `refused` and `delivery_generation` from the first press. A
refusal met before the second press is recorded in `refused` by PendingClose::refuse from close_settled
(editing.rs:1063) or close_settled_later (settings.rs:1290-1292), and close_settled returns while another armed receipt
or close_busy remains (editing.rs:1064-1070). The person's input in between does not clear it: has_moved_on is checked
only against the latest mark (editing.rs:1074), which the second press just replaced. When the remaining work settles,
close_settled takes the pending close and, because `refused` is Some, reveals that old page (editing.rs:1077-1085)
instead of calling ask_to_close; likewise `delivery_generation > pending.delivery_generation` (editing.rs:1086-1087)
fires for a report that arrived before the second press. SPEC.md 'Settings' says a refusal keeps Settings open and 'any
input in the meantime cancels that close'; the second press is a new close after that input, and nothing in the spec has
it inherit the cancelled close's outcome. Fix: on a repeated press, if the person moved on since the pending close's
mark, reset `refused` and take a fresh `delivery_generation` baseline (keeping the shared armed count). Verify by
reading wait_to_close and close_settled.

### Triage notes

- Recorded bucket: `other`. The commissioner proposed dropping this under FILTER.md's rare self-correcting glitches
  filter; the independent drop check disagreed (the trigger is not clearly rare: the ntfy Send test, a Tensorlake key
  Test or Clear and a credential Delete can hold a close for seconds, up to the 5 s ntfy send timeout or the 20 s host
  reply bound), so it is recorded.
- Suggested bucket (reviewer): other (a wrong page and an extra Close press; no data loss). The window needs the second
  piece of work to still be out while the person reacts to the first refusal, which the low-latency helm assumption
  makes short.
- Confidence: confirmed from source.
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g02/f3 (finding), group g02 "Settings close waits for outstanding work: editing core", findings log line 13.
