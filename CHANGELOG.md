# Changelog

Notable user-facing changes in each stable release of Farhelm. Release candidates and dev builds are not listed; their changes appear under the stable release that follows them. Entries are written for someone running Farhelm, not for someone reading its source, so internal mechanics are left out unless they change what you have to do. cargo-dist copies each release's section into its GitHub release; `releasing/AGENTS.md` describes the format and how a section is written.

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
