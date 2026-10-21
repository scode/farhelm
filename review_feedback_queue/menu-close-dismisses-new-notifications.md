# Popup handback can dismiss a new notification list

Reviewed commit: 738598601c142e9e184ee7f0427e8995b2d89e0f

## TLDR

Switching from a session menu or another notification list to a notification bell may immediately close the new list and
mark its displayed entries read. The source paths share a focus-return guard; actual event ordering and premature
marking remain unverified.

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

## Notification-list handback extension

Additional reviewed commit: `8a9b9b3de5abaa82738866543366f92247273fa3`.

Switching directly between two notification lists adds another caller of the same insufficient popup-identity guard. The
old list's new close effect calls the shared helper using `data-session-id`
(`crates/farhelm-ui/src/list/bell.rs:112–120`). `menu_panel.rs:872–877` rejects another popup's focus only for
`data-row-menu-key` callers; the next guard permits the class used by the new notification panel (`bell.rs:269–284`).
One callback replaces the open bell identity (`list/view.rs:3137–3150`). If the new panel's mount focus precedes the old
handback while its inside-focus flag survives, the old list takes focus and the new focus-leaving check closes it
(`bell.rs:335–351`). If the parent already observed the new list, read-on-close can mark its displayed entries read
(`view.rs:3184–3202`). Preserve the closing popup's identity for bell callers as well as actions-menu callers.

Suggested bucket remains **other**. Confidence: **likely**; caller/helper mismatch is confirmed, but renderer/focus and
parent-observation ordering, frequency and actual premature marking remain unverified. Read state is convenience state;
no security, command execution or user-owned-work loss was established. This extends the existing underlying helper
defect, not a second queue item.

Possible cover: competing-input acceptance at `SPEC.md:1392–1397` does not establish acceptance of one click followed by
automatic handbacks. The subsecond filter needs a human timing window, unestablished here. Rarity alone would not filter
a wrong durable read mark. No exact Planned, BUGS or ledger acceptance for the new claimant was found. Proposed drop:
none; merge the new substance into this item. Provenance: `notifications-whole-1/f2`, later synthesis and empty
discovery in the trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was
performed.
