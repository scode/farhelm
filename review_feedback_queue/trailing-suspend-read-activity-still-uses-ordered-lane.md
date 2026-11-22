# Suspend and Stop and suspend still stall behind slow hosts because read_activity uses the ordered lane

Reviewed commit: 21c2cdfb1e650bedba1a874a9082e7a8cb0231ea

## TLDR

Suspending an environment (from a session row, session header, or Hosts panel) or choosing Stop and suspend from a
session menu still waits behind any slow host running an ordered action for up to 20 seconds, because its pre-flight
activity check still runs on the ordered channel lane instead of the unordered lane.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/environment.rs:504`.

Commit `21c2cdfb1e650bedba1a874a9082e7a8cb0231ea` moves `Work::Suspend` and `Work::StopAndSuspend` to `Lane::Unordered`
(`crates/gui/farhelm-gui/src/operations.rs:270,272`) so environment actions can bypass slow hosts. However, every direct
Suspend entry point and the session menu's Stop and suspend call `environment::read_activity(host, cx, ...)` before
opening the confirmation or sending the `Work::Suspend` / `Work::StopAndSuspend` command:

- Session row menu Suspend / Stop and suspend: `crates/gui/farhelm-gui/src/session_list/menu.rs:1318` ->
  `environment::begin_shown` (`crates/gui/farhelm-gui/src/session_list/environment.rs:464`)
- Session header Suspend: `crates/gui/farhelm-gui/src/session_pane/header.rs:887` -> `environment::begin_shown`
  (`crates/gui/farhelm-gui/src/session_list/environment.rs:464`)
- Hosts panel Suspend: `crates/gui/farhelm-gui/src/hosts/lifecycle.rs:142`

`environment::read_activity` (`crates/gui/farhelm-gui/src/session_list/environment.rs:494-508`) issues
`GET /api/sessions?host=<id>` via `operations::read` (`crates/gui/farhelm-gui/src/operations.rs:779-790`), which calls
`operations::send` -> `send_in_lane(Lane::Ordered, ...)`. On the helm (`crates/farhelm-helm/src/ui_channel.rs:1038`),
`Lane::Ordered` commands queue behind any in-flight ordered command (e.g., a `Rename`, `Restart`, `Replace`, `Launch`,
`Probe`, `RemoveHost`, `Browse`, or checkout preview waiting up to `HOST_REPLY_BOUND` = 20s on an unrelated slow host),
even though `GET /api/sessions?host=<id>` is a pure helm-local cache lookup (`crates/farhelm-helm/src/sessions.rs:897`).

Consequently:

- Moving `Work::Suspend` and `Work::StopAndSuspend` to `Lane::Unordered` prevents an already-sent Suspend from blocking
  later ordered work, and lets the Stop confirmation's secondary 'Stop and suspend' button
  (`crates/gui/farhelm-gui/src/session_list/stop.rs:176`) bypass slow hosts, but every primary Suspend action and direct
  Stop and suspend still blocks behind slow hosts on the ordered lane.
- While `read_activity` is queued behind the slow host, `session_list/environment.rs:462` (`set_reading(key, true, cx)`)
  holds that session's controls busy, and in `hosts/lifecycle.rs:142-153`, if the user interacts elsewhere during the
  stall, the late read trips `Asked::MovedOn` and refuses to open the Suspend confirmation.

Fix direction: Send `read_activity`'s `GET /api/sessions?host=<id>` via `operations::send_unordered` (or an unordered
`read_unordered` helper in `operations.rs`) and add a test pinning `read_activity`'s lane to `Lane::Unordered`.

Reviewer metadata (g41-desktop-bypass-slow-hosts/f2): suggested bucket: other (could not be highest: no security issue
or loss of user data/credentials/processes/work; only delays starting Suspend or causes a MovedOn retry prompt while an
ordered command on a slow host is in flight); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g41-desktop-bypass-slow-hosts/f2`.
