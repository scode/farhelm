# Changelog

Notable user-facing changes in each stable release of Farhelm. Release candidates and dev builds are not listed; their changes appear under the stable release that follows them. Entries are written for someone running Farhelm, not for someone reading its source, so internal mechanics are left out unless they change what you have to do. cargo-dist copies each release's section into its GitHub release; `releasing/AGENTS.md` describes the format and how a section is written.

## v0.14.0 - 2026-09-23

### 🚀 Added

- Grok is now a supported harness. Launch it from the session launcher, and Farhelm tracks its conversation so a restarted session can resume it. There is no model or effort picker for Grok. (#845, #861)
- The session launcher can set "workspace trust", avoiding the "do you want to trust this project?" prompts, for Codex, Muse, and Pi sessions with `trust:true` or `trust:false`. The choice applies to that launch only, is remembered as the default for the next new session, and never changes the harness's own trust settings. Claude, Goose, OMP, and Cursor have no such switch. (#863, #869)
- The session launcher's search now takes `name:foo` to set the session name and `host:foo` to pick a host; `host:local` picks this machine. (#864)
- The Hosts header has an `update all` action for remote hosts whose individual Update action is available. Each host shows its own progress and result, and a failure on one does not stop the others. Hosts already busy with setup or another update are skipped, and the local host is never included. (#894)
- The Hosts panel shows the binary upload as its own progress step during setup and updates, so you can tell when Farhelm is still transferring over the network. (#891)

### 🔄 Changed

- After a host reboot interrupts a session, Farhelm now explains what happened and waits for your choice: restart the session with its existing conversation, or replace it with a fresh conversation using the same settings. (#897)
- The desktop app remembers its window size and position, and reopens maximized if it was maximized when closed. If the display layout changed, it picks a safe placement instead of reopening off-screen. (#899)
- OMP session tracking is more reliable: Farhelm only accepts a conversation reported by the session's own interactive OMP, not by subagents or nested OMP processes. OMP sessions started before this version offer only a fresh restart until they are relaunched. (#814)

### 🔧 Fixed

- Selecting a GitHub repository in the session launcher now shows the folder the checkout will be created in, instead of a stale path from earlier. `use existing folder` switches back to editing the path yourself. (#865)
- OpenCode, Goose, Pi, and OMP sessions can be launched without picking a model; the harness then uses its own configured default. (#866)
- After a supervisor restart, sessions keep their last known status instead of briefly showing as running. A session with no known status asks for confirmation before Restart. (#867)
- The destination picker and Browse button in the session launcher line up again. Hovering a session's status dot shows its status, and the unlock icon on YOLO sessions explains the permission mode. (#862)
- Remote host updates no longer abort an upload after 60 seconds while it is still making progress. A stalled upload still times out and leaves the installed binary intact. (#888)
- Host rows show compatible supervisors running an older Farhelm build as `old version` instead of `needs update`, and they stay connected and usable. Hosts whose supervisor is actually incompatible still show `needs update`. (#893)

## v0.13.0 - 2026-09-22

### ✨ Highlights

#### Session archiving is removed

The concept of session archival is removed. No use for it. Might add back in the future if useful.

On upgrade, any archived sessions return to the session list. (#834)

#### More reliable session id tracking for codex

This version should more reliably be able to resume codex sessions. The bug was that sub agents and shelled out sub harnesses were able to report a session id back to farhelm as the currently active session. This should now be prevented.

Other harnesses have similar problems which is planned to be addressed in future releases. (#811)

### 💥 Breaking

- As stated above, session archival is removed. (#834)

### 🚀 Added

- The cursor CLI agent is now a partially supported harness. There is no session tracking yet, so restarts won't resume sessions properly. (#844)
- Better icons for the various harness types. Official ones where allowed by terms of use; otherwise something reasonable. (#847)
- Going forward, releases will have curated release notes, such as the ones you are reading now. (#858)

### 🔄 Changed

- More reliable codex session tracking as mentioned above. (#811)
- Replace gpt-5.6 luna+sol models with their gpt-6-* counterparts. (#859)

### 🔧 Fixed

- The examples in `$farhelm help` are now clearer and no longer include unrelated restart caveats. (#828)
- Rare edge case could cause a helm to inappropriately claim a browser is still attached when attempting to re-attach. (#843)
- The icons indicating harness type in the session list, and the permission lock icon for yolo sessions, are now properly displayed in two fixed columns. Previously they would not be aligned when some sessions were in yolo and others were not. (#850)

## v0.12.0 - 2026-09-21

### ✨ Highlights

#### The session launcher looks like the rest of Farhelm

Previously the session launcher had a considerably different look and feel from the rest of Farhelm. It now uses the same colors, corners, and selection style as the main window, and its "recent-setup" rows, which show recent launches, only show what those launches explicitly override relative to default values. (#819, #820)

### 🔄 Changed

- The session launcher uses the main window's colors, corners, and selection style, and its "recent-setup" rows, that show recent launches, only show what those launches explicitly override relative to default values. (#819, #820)

### 🔧 Fixed

- Buttons, inputs, and selects in the session launcher render in Farhelm's typeface instead of the platform's sans-serif. (#818)
