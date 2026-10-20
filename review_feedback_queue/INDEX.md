# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`same-destination-recovery-loses-trash-location.md`](same-destination-recovery-loses-trash-location.md) — Overlapping
  recovery with the same destination may hide retained checkout files from the trash; an affected platform is
  unverified.

## High priority: material UX degradation

## Other: correctness, diagnostics, cleanup, or convenience

- [`notification-test-cleanup-skips-preference-restoration.md`](notification-test-cleanup-skips-preference-restoration.md)
  — A failed notification fixture cleanup can skip later session releases and shared preference restoration.

- [`shortcut-tests-skip-owned-cleanup.md`](shortcut-tests-skip-owned-cleanup.md) — Early cleanup failures in shortcut
  and naming tests skip remaining owned resources or shared checkout configuration restoration.

- [`terminal-focus-tests-arm-cleanup-late.md`](terminal-focus-tests-arm-cleanup-late.md) — A focus-observer setup
  failure can leave a new regression test's live session outside its cleanup scope.

- [`removed-host-strands-tunnel-listeners.md`](removed-host-strands-tunnel-listeners.md) — Removing a host leaves its
  tunnel ports occupied while individual close requests fail.

- [`menu-close-dismisses-new-notifications.md`](menu-close-dismisses-new-notifications.md) — Closing an actions menu can
  immediately dismiss a newly opened notification list and mark its contents read.

- [`selection-history-plan-forgets-upgrade-selection.md`](selection-history-plan-forgets-upgrade-selection.md) — The
  visit-history plan's empty upgrade history conflicts with restoring the existing remembered session.
