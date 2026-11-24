# Restart to update and the bundle reopen carry the brick-test announcement override into the next app instance

Reviewed commit: ccb0efcc9f9607c84bcd99a42f03d6e33a319b85

## TLDR

An app launched with the brick-test feed override keeps it after it restarts itself. This happens on Restart to update
and on the reopen after its bundle changed underneath it. The next instance then polls the test feed instead of the
production one, and its bell reservation stays active. Ordinary users never set the variable, so in practice this
affects release testing.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g08-announcements-helm/f8`)

Location: `crates/farhelm-desktop/src/updater.rs:358`. Summary: Restart to update and the bundle reopen carry the
brick-test announcement override into the next app instance

Reviewed commit: ccb0efcc9f9607c84bcd99a42f03d6e33a319b85 introduced FARHELM_ANNOUNCEMENTS_PATH and stripped it only
from SpawnInputs. 356718b9d0bca3a27758ec6904688da2f54b9639 added the bell reservation keyed on the same variable
(crates/farhelm-desktop/src/lib.rs:230-232). Still present at tip e159ffc79ab147be44e92a92b59571650a4f322d.

TLDR: An app launched with the brick-test feed override keeps it after it restarts itself. This happens on Restart to
update and on the reopen after its bundle changed underneath it. The next instance then polls the test feed instead of
the production one, and its bell reservation stays active. Ordinary users never set the variable, so in practice this
affects release testing.

Details: The desktop reads FARHELM_ANNOUNCEMENTS_PATH from its own environment at every start (lib.rs:65, 230-234) and
passes it to the embedded helm (lib.rs:531-545; helm network_announcements.rs filename()). ccb0efcc9 declared the
override per launch: lib.rs:1427-1428 strips it from processes the app spawns, with the comment 'A brick-test
announcement feed belongs to this launch alone'. The app's two self-relaunch paths do not strip it. (1) Restart to
update: updater.rs:341-367 relaunch_command removes LATEST_OVERRIDE_ENV, documented as 'without carrying a release-test
candidate into the next app instance', and REOPENED_ENV, but not FARHELM_ANNOUNCEMENTS_PATH. Its environment test at
updater.rs:1869 lists only those two names. (2) Bundle reopen: bundle.rs:215-231 reopen_command (used at bundle.rs:94
for MissingVersion::Reopen) strips nothing. bundle.rs:205-208 states that 'The new instance inherits the opener's
environment' for open -n. So the relaunched app re-reads the override, its helm keeps fetching announcements-test.json,
and the desktop keeps reserving the pending batch for the bell (lib.rs:232). Verify by reading relaunch_command and
reopen_command against lib.rs:1427-1428. Fix: add .env_remove(ANNOUNCEMENTS_PATH_ENV) to relaunch_command and to the
reopen path (or to wait_then_open_bundle for both), and extend the updater.rs:1869 environment test.

Suggested bucket: other. Only someone who exported the release-test variable is affected, normally the brick test on a
scratch guest, so the harm is confined to maintainer tooling and stays at another feed's signed, display-only content.

Confidence: likely. The source establishes that the variable is not removed on either path. Unverified premise: that
LaunchServices' open -n passes this variable on to the new instance. The code asserts this at bundle.rs:205-208, and
LATEST_OVERRIDE_ENV is removed on the same path for the same reason.

proposed_drop: none.

possible_cover: g10, which owns the brick-test tooling, may have noted that the P2 'normal relaunch' (RECIPE.md:254-256)
is a tooling relaunch, not the app's own update relaunch. The current P2 order (announcements last, after
update-relaunch) means today's brick test does not hit this.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g08-announcements-helm/f8`.
