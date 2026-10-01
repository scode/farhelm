# Pi's conversation reporter changed without a new file name

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

On any host that ran a Pi session on an older release, every Pi session started after upgrading to v0.13.0 or later
silently loses Resume and the `farhelm agent` instructions pointer, permanently, until someone deletes a file in the
supervisor's state directory by hand.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F2 / COR-PI-ASSET`, tagged **definite**. Anchor and title: `crates/farhelm-supervisor/src/pi_extension.rs:27`
— Pi's conversation reporter changed bytes without a new file name, so upgraded hosts permanently lose Pi capture and
Resume.

For Pi sessions, Farhelm learns which conversation the agent is in (what makes Resume possible after a restart) through
a small TypeScript extension that the supervisor writes into its state directory, at
`integrations/pi/farhelm-conversation-v1.ts`, and loads into Pi with `-e`. The same extension also carries the pointer
that tells the agent about `farhelm agent` (passed via `--append-system-prompt`). The supervisor publishes the file with
a write-if-absent rule (`materialize_asset`, `crates/farhelm-supervisor/src/pi_extension.rs:68-93`): if the file already
exists, its bytes must exactly equal the copy compiled into the current binary, otherwise it refuses with "the existing
pi extension does not contain Farhelm's expected bytes". The design intent, stated for the OMP extension a few lines up
(`pi_extension.rs:35-39`), is that a changed asset gets a new file name and is written beside the old one, never over
it.

Commit ce5ea14 (#811, first released in v0.13.0) changed the Pi asset's bytes (it added `--vendor pi` to the callback
the extension makes into `farhelm internal hook`) but kept the `-v1` name; the OMP asset in the same series was
correctly renamed to `-v2` (fc354b4, #814). Any host that ran a Pi session on a release from v0.9.0-rc.4 through v0.12.x
has the old bytes on disk. On every later release, each Pi launch fails the byte check; the create path
(`crates/farhelm-supervisor/src/service/core.rs:13029-13046`) logs a warning and launches Pi without the extension.
Nothing ever replaces the stale file.

The effect is permanent and silent: on upgraded hosts every Pi session loses conversation identity (no Resume offer on
restart) and the instructions pointer, and the only evidence is a supervisor log line. The user would have to know to
delete the file by hand. The fix is to publish the current asset as `farhelm-conversation-v2.ts`, beside the old file,
update SPEC_impl.md's Pi paragraph, and add a test that pins a hash of each published asset's bytes to its file name (or
derives the name from the hash), so a future content change without a rename fails CI.

## Additional detail merged from a second review (de774a1ee8815ce833da77deac593a55d82f7be3)

A separate whole-codebase review found the same problem independently; its item
(`pi-extension-stale-bytes-after-upgrade.md`) was folded into this one. It adds:

- Blob ids: v0.9.0 through v0.12.0 ship `assets/pi-conversation-v1.ts` blob 8b9022d0; ce5ea14 (#811, first in v0.13.0)
  changed it to blob 90cedb74 (adding `"--vendor", "pi"` to the hook argv) under the same published name.
- A verification recipe: write the v0.12 blob
  (`git show v0.12.0:crates/farhelm-supervisor/assets/pi-conversation-v1.ts`) to
  `<state>/integrations/pi/farhelm-conversation-v1.ts` and call `materialize_asset(state, &PI_ASSET)`; it returns the
  mismatch error. The existing test `mismatched_existing_artifact_is_refused` (`pi_extension.rs:132-148`) already
  exercises this refusal with arbitrary bytes.
- An alternative fix to renaming: when the existing file is a regular 0600 file in Farhelm's own private
  `integrations/<vendor>/` directory, replace a byte mismatch atomically with the rename tier instead of refusing.
  Running agents have already loaded their copy. A content-addressed name also prevents any future edit from repeating
  this for either vendor.
- Why FILTER.md's "Rare edge cases in harnesses without first-class support" filter does not apply: the trigger is not
  rare or unconfirmed but deterministic on every host that launched a Pi session under v0.9–v0.12 and then upgraded.
