# No test covers the bell's guards against a late announcements reply

Reviewed commit: 4e13aff403c4823cfa7a2f35b645c2df176641e2

## TLDR

If the user presses Open Announcements and then clicks or types elsewhere, or closes the notification popover, before
the helm answers, the late answer must not pop the Announcements dialog over what they are now doing. The code has
guards for this, but no test checks them, so a later change could silently break them.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g09-announcements-gui/f3`)

Location: `crates/gui/farhelm-gui/src/rail_notices.rs:248`. Summary: No test covers the bell's stale-reply guards (user
moved on, panel closed or replaced)

Reviewed commit: 4e13aff403c4823cfa7a2f35b645c2df176641e2 (tip e159ffc79ab147be44e92a92b59571650a4f322d still has it).

TLDR: If the user presses Open Announcements and then clicks or types elsewhere, or closes the notification popover,
before the helm answers, the late answer must not pop the Announcements dialog over what they are now doing. The code
has guards for this, but no test checks them, so a later change could silently break them.

Details: `NoticesButton::open_announcements` (crates/gui/farhelm-gui/src/rail_notices.rs:240-264) drops the GET reply
when `focus::has_moved_on(&mark)` (line 248) or when the bell's open panel is no longer the one that asked (line 253).
SPEC_impl.md, "Shared preferences and announcements": "The click's focus mark and panel identity guard that handoff."
SPEC.md "Keyboard focus" (via SPEC_impl's layer rules) forbids opening a layer the user did not start after they moved
on. The only bell test, `network_announcements_open_from_the_bell_and_record_only_the_displayed_batch`
(crates/gui/farhelm-gui/src/rail_notices/tests.rs:402-491), replies immediately with no intervening input and never
closes or reopens the panel before the reply, so deleting either guard leaves every test green. The empty-batch return
on the same line is also untested. Fix: add cases that (a) click elsewhere or press a key between the click and the
reply, (b) close the panel (Escape) and reopen it before the reply, and (c) reply with an empty pending list; each
asserts no Announcements dialog opens and, for (a), that the panel and focus are left as the user left them.

Suggested bucket: other — a test gap with no current user-visible defect.

Confidence: confirmed (grep for the guards' callers and the test file shows only the one happy-path test).

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g09-announcements-gui/f3`.
