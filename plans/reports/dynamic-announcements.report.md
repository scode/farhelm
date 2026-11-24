# Signed announcements

## What this was about

Farhelm could only show announcements baked into a release. The helm now checks a signed feed at startup and every 12
hours, so an announcement can reach people without a new build. The maintainer chose the existing release origin and
verification keys, targeting by version, OS and expiry, and an opt-out in Settings → Notifications. Built-in release
announcements still use the same durable queue.

## Things you should know

- New feed entries raise an unread reminder in the sidebar bell. Its action opens the pending batch. Closing the dialog
  records the entries it showed and returns focus to the bell. An entry that arrives while the dialog is open keeps the
  reminder active until it is shown. Shown entries stay in Help → Announcements and the same ID is never delivered
  again. A normal window start or reset still opens an already-pending batch automatically.
- Signature verification runs before parsing. Targeting runs on the user's helm. Farhelm includes no user or machine
  identifier or running version in a feed request. A 404 is quiet absence during ordinary use. The check defaults to on
  for both new and upgraded installs. Development builds and a switched-off preference skip network checks. Existing
  pending entries and history remain available after opting out.
- The native release brick test checks the same origin and verification keys using a standing signed test feed. For that
  one launch the pending batch waits for the bell, including when the feed arrives before the first window. The
  checkpoint calls for captures of the bell action, opened announcement and history, then verifies that the following
  normal launch has no override. A missing fixture is reported separately from installation, session and feedback
  results.

## Open questions and possible follow-ups

- The native P2 check still needs a published release build and its standing signed fixture. Signing and publishing
  remain in the maintainer's external workflow. Local tests exercised queue admission with an injected successful
  verification and rejected wrong-purpose or tampered bytes through the real verifier with checked-in test-key fixtures.
  The combined successful path against the release keys remains for P2.
- Linux Notifications references were updated and reviewed. macOS references still need a native renderer.

## The PRs

- Verify and queue signed announcements in the helm. [#4749](https://github.com/scode/farhelm/pull/4749/changes)
- Open pending announcements from the bell and add the Settings opt-out.
  [#4753](https://github.com/scode/farhelm/pull/4753/changes)
- Exercise signed announcements in the release brick test and document the feed.
  [#4756](https://github.com/scode/farhelm/pull/4756/changes)

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-proto -p farhelm-helm --lib` selected protocol, announcements, notices, preferences,
  store and UI channel tests. The run at `8aa65b40` passed 607 and exposed seven historical schema fixtures with the new
  setting still present during rewind. The seven passed at `e6dedfb7` after correcting that fixture. The earlier passes
  still apply because the only intervening change was the fixture. Recorder IDs `fef0c408-d4aa-45e7-985c-8e69163d2beb`
  and `c05c77ec-d415-4192-b565-8044a9162519`. Strict default, shipped CLI and desktop Clippy passed.
- `cargo nextest run -p farhelm-gui -p farhelm-desktop` selected GUI announcements, bell and Settings tests, plus
  desktop tests. The run at `93916a80` passed 471 and exposed one bell handoff failure. The corrected bell and
  announcement selection at `9fc02092` passed all 28. The earlier Settings and desktop results apply outside the changed
  handoff. At `4f3df442` all 48 selected Welcome, bootstrap, announcement and bell cases passed, including an entry
  already pending when the window opened. Recorder IDs `34edee2a-776e-4d0c-ba31-2f575c8fb9d6`,
  `83e63ce7-3ef7-44c3-9e75-ecff68427e8e` and `01d25f78-79f7-4a03-9830-469a9deb9d8f`.
- `python3 -B -m unittest discover -s releasing/mac-vm-test` initially passed 317 cases and found two reporting fixtures
  that assumed feedback was the only P2 result. At `4f3df442` all 320 cases passed. The launch isolation case also
  passed after switching the process sample to Darwin's explicit environment flag. Recorder IDs
  `301ff2e9-5c20-47e2-8e96-d34ac73383cd`, `bd950b4c-b9c6-44fb-9c2d-82d3ff99d317` and
  `bc4bcfbd-eed8-47fc-89de-f2958a0799cd`. These are portable tests with simulated guests.
- `python3 scripts/check-gui-goldens.py --update` produced six intended Linux changes, which were reviewed and
  published. The Notifications, Welcome and update-pill comparison passed on the rebased stack at `2b41b1ff`. The rebase
  covered the terminal-link warning and update-pill changes from main. It preserved the six Notifications images and the
  independently updated pill references. Recorder IDs `f68176d3-7e78-446e-ae6f-438baaf573c9` and
  `64f37768-2c97-494c-abfd-82c7dc106c78`. Strict desktop and GUI Clippy passed on the rebase. The first two PRs retained
  the same patches as the checked versions.
- Rust and Markdown formatting, test-delay, changelog, GUI and crate-layering checks passed. The agent-instruction link
  check had zero errors and one existing archive reference warning. The focused selections cover the changed behavior,
  so the full workspace suite was skipped. A published build and signed fixture are needed for the native Mac check.

## Review outcome

The three source reviews cleared after corrections. A stale acknowledgement cannot clear a new reminder, the preference
is read before each fetch, and invalid announcement link authorities are dropped while retaining the text. The bell
reads a fresh queue snapshot before handing off focus. The brick test accounts for early delivery and waits for the
prior app to exit before each launch, and a failed fixture probe leaves the later checks available. The remaining native
and signing validation limits are above.
