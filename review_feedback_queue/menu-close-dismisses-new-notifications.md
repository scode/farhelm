# Closing a session menu can dismiss newly opened notifications

Reviewed commit: 738598601c142e9e184ee7f0427e8995b2d89e0f

## TLDR

Clicking another session's notification bell while an actions menu is open can immediately close the new list and mark
its displayed notifications read before the user reads them. The event ordering on this revision remains unverified.

## Details

`crates/farhelm-ui/src/list/view.rs:3122–3129` closes the actions menu and opens the bell in one callback. Session-menu
teardown can then return focus to its old toggle (`list/row.rs:1455–1497`). That caller uses `data-session-id`;
`menu_panel.rs:872–877` protects a different keyed popup only for callers using `data-row-menu-key`. Its next guard
accepts focus inside `.host-row-menu-panel`, which includes the newly mounted notification panel.

The new panel requests focus (`list/bell.rs:260–262`). If the old menu's focus return runs afterward, the bell's
focus-leaving check closes it (`314–333`), and the read-on-close effect records its displayed entries as read
(`list/view.rs:3145–3181`). Preserve independently acquired popup focus when returning focus from a closing session
menu. A browser regression should establish focus inside the old menu, perform the single bell click and wait for both
focus operations before checking that the new list remains usable.

Suggested bucket: **other**; focus and notification-read convenience state, with no loss of user-owned work or command
execution established. Confidence: **likely**; the source mechanisms are confirmed, their ordering is unverified here.
No runtime reproduction was performed.

Possible cover: the exact sequence is disclosed in `plans/reports/sidebar-bell-follows-row.report.md:27–30`, introduced
by `b9daccc87439c9e0d519856e5d2b9b4748282ea4`. That report is provenance, not acceptance. Its claimed Chromium trace was
not authenticated. The reviewer compared its development branch with pinned main: the relevant menu teardown and focus
helper are identical, and the notification opening/focusout behavior already exists on main. This is existing behavior
discovered through the report, not a regression introduced by that report.

The existing automatic-dismissal focus and deletion-placement queue items concern different triggers and consequences.
SPEC.md's acceptance of competing user inputs within tens of milliseconds does not establish coverage for one click and
competing automatic focus operations. No exact Planned, BUGS or ledger coverage was found. The subsecond-human-race
filter requires a person to win a timing window; that premise is not established here. Proposed drop: none.

Provenance: separate UI reviewer in the trailing review through the reviewed commit, followed by commissioner source
inspection. Surrounding UI and specification context was read selectively; browser/native behavior was not exercised.
