# The brick test's announcements check does not wait for the feed delivery

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639

## TLDR

After the brick test starts the candidate app with the test feed, nothing waits for the app to actually download the
feed before the tester looks at the notification bell. If the download is slow, the tester can report a failure that is
really just timing. Worse, if the app's single download attempt hits a transient network error, the app does not try
again for 12 hours, so the check reports the candidate as FAILED because of a network blip.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g10-brick-announcements/f4`)

Location: `releasing/mac-vm-test/checkpoint_contract.py:66`. Summary: Announcements check has no wait for the feed
delivery, and a one-off fetch error becomes a candidate 'fail'

Revision of candidate f4 after the concrete check the orchestrator asked for. Re-recorded as a finding.

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639 (still present at tip
e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: After the brick test starts the candidate app with the test feed, nothing waits for the app to actually download
the feed before the tester looks at the notification bell. If the download is slow, the tester can report a failure that
is really just timing. Worse, if the app's single download attempt hits a transient network error, the app does not try
again for 12 hours, so the check reports the candidate as FAILED because of a network blip.

Details (what happens between launch and bell inspection, checked in source):

- `launch_announcements` (releasing/mac-vm-test/guest_operations.py:723-758) runs
  `open -n --env FARHELM_ANNOUNCEMENTS_PATH=...`, then `wait_running` (releasing/mac-vm-test/guest_ui.py:22-44), which
  polls only `version()` (process list and supervisor version) until the candidate app and supervisor run, then
  `check_announcement_launch` reads its environment. No step observes the feed fetch, the announcement queue or the bell
  notice.
- `announcements_instructions` (releasing/mac-vm-test/checkpoint_contract.py:55-76, line 66) says 'Bind the new
  candidate and require an unread notice in the notification bell above Hosts' with no instruction to wait, poll or
  re-check. No fixed operation exposes delivery readiness (OPERATIONS, guest_operations.py:48-55; required_operations
  'p2-announcements', checkpoint_contract.py:197). Generic prompt text says 'Query readiness or state; never infer
  success from elapsed time' (guest_tester.py managed_prompt), which permits but does not require re-querying the bell.
- Product side (context, g08 scope): the helm's first network tick runs immediately at startup
  (crates/farhelm-helm/src/network_announcements.rs:322-326, tokio interval's first tick), with a 15 s timeout per
  request (line 25) and two sequential GETs (fetch_pair, lines 251-266), so delivery can land up to ~30 s after launch.
  Any transport/HTTP error is logged as a warning and the next attempt is CHECK_INTERVAL = 12 h later (line 26). The
  brick-test bell reservation keeps an early delivery for the bell, but nothing covers a late or failed one.
- Result classification: the tester has no access to the helm log, so a missing notice reads as a product failure;
  managed_prompt maps that to `fail`, imported as P2 `announcements: fail`. The usual case is benign (the model's bind
  and AX reads take seconds and the guest just proved reachability with the probe), so this is a sporadic false fail,
  never a false pass: pass still needs the exact title in dialog and history. Fix: add a fixed read-only operation (or
  instruction) that polls for the delivery, e.g. wait up to ~45 s for the unread notice with fresh AX reads, and treat
  'no notice after the bound' as could-not-run unless the helm log shows a verification/schema error; or have the
  operation read the helm log for the network-announcement warning to separate transport errors (could-not-run) from
  authentication/schema refusals (fail).

Suggested bucket: other — an intermittent misattributed result in nonblocking maintainer release tooling.

Confidence: likely — the code path is confirmed (no wait; one-shot fetch with 12 h retry); unverified premise: how often
the model's first bell inspection precedes delivery or the single fetch fails transiently in the guest.

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g10-brick-announcements/f4`.
