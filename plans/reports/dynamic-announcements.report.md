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

### Landing

#### What landed with it

This was the only plan landed this time: its three PRs ([#4749](https://github.com/scode/farhelm/pull/4749/changes),
[#4753](https://github.com/scode/farhelm/pull/4753/changes),
[#4756](https://github.com/scode/farhelm/pull/4756/changes)) plus one landing fix,
[#4759](https://github.com/scode/farhelm/pull/4759/changes). While the plan was being built, main gained the helm's
handling of unreadable recipe credentials (preserve-creds-on-lookup-failure) and the `farhelm agent` listing of agent
types (custom-agents-cli-visibility), plus plan bookkeeping. Neither touches announcements, and no schema or protocol
numbers collided. The rebase met one conflict, in TODO.md, where this plan and the others each removed their own entry;
all removals were kept.

#### Fixes made while landing

- **Helms started by tests checked the live feed.** The plan skipped the check in development builds and in the helm's
  own unit tests, but helms that other parts of the test suite start (the end-to-end tests, the helm client's tests, the
  desktop app's tests) still fetched `get.farhelm.io` whenever the build carried a release version. The release gate
  runs the whole suite on the tagged commit, so it would have reached the release origin, and once you publish a real
  feed a test that looks at the bell or the announcement queue could have failed depending on its contents. Your
  planning decision said test builds skip the check. #4759 makes the check off unless a helm's launch asks for it, and
  only the desktop app's launch asks. The desktop app is the only thing that starts a helm outside the tests, so every
  real helm still checks; one desktop test that copies the app's own launch settings switches it off again.
- **A misspelled targeting field sent an entry to everyone.** The feed reader silently ignored any field it did not
  know, so an entry with `min_verison` or `oss` in place of `min_version` or `os` would have gone to every helm instead
  of the ones it targets. An unknown field now counts as a broken feed, which, like the feed's other format errors,
  leaves the announcement queue as it was. This has a cost, below: a field you add to the format later will make this
  release ignore the whole feed.
- **Two existing helm tests failed.** Two tests of the checkout settings' database migrations rewind a database to an
  old schema and compare it with that schema's frozen definition. Their rewind did not remove the new announcement
  setting's column, so they failed in the full test run on the test worker (below); the executor's narrower selection
  had not included them. The rewind now removes it.
- The docs pages on the security model (the list of connections that leave your machine) and on Settings → Notifications
  now mention the check and its **Check for announcements** switch. SPEC_impl.md now states both rules above: only the
  desktop app's launch checks, and an unknown entry field is a format error.

#### Checks

The plan adds a network feed to the helm with a database schema step, changes the desktop app's bell, Settings and
Welcome screens, and edits the release brick test and several instruction files, so the landing ran the whole Rust
workspace suite, the desktop app's own tests, clippy with warnings as errors on four targets (the workspace, the
`farhelm` CLI binary, and the desktop app with and without its test-automation bridge), every desktop screenshot against
its reference picture, the brick test's portable test suite, the instruction-file check, the dependency license, source
and notices checks (the lockfile changed), the crate layering check, and the formatting checks (`cargo fmt`, `dprint`),
the test-sleep check and the changelog format check. Worker runs were on a separate arm64 Linux test worker; the rest
ran locally on x86_64 Linux. Two tests failed in the first full run, as described below; no other check failed. The
first attempt at the notices check could not run at all, so it was run again after the fix.

On the plan's PRs rebased onto main, before the landing fix:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `a04d7244-2696-43a3-9239-56c72f55a5f8`): 8548 passed and 2 failed, the two checkout settings migration tests the fix
  repaired (`checkout_config::tests::checkout_open_existing_refuses_older_schema_without_migrating` and
  `checkout_config::tests::checkout_migration_from_26_recreates_the_config_tables`); 86 ignored tests were skipped and
  no test silently skipped for a missing systemd, ssh or Podman setup.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `c06fd96a-45c8-4b65-9ec9-4d2bfcbddf2b`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the same with `--features agent`, on the test
  worker: passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `adf71a3c-b2a4-4013-963b-32b3ca3f7883`): every
  screenshot matched its reference, including the six the plan updated.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`, locally (recorder run
  `d42a3ff9-2742-4d83-a40a-29ae78400481`): passed. These tests exercise the brick test's harness with simulated guests,
  not a real Mac.
- `python3 scripts/check-agents-md.py`, `python3 -B scripts/check-crate-layering.py`, the pinned
  `cargo-deny --locked check licenses sources`, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`
  (in its pinned interpreter), `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed. The
  notices check runs on the worker because it is CPU-heavy; its gated form
  (`--only-if-inputs-changed-since origin/main`) could not run there, since the worker's copy of the tree has no git
  history, so the ungated check ran after the fix (below).

After the landing fix (#4759):

- `cargo nextest run -p farhelm-helm -p farhelm-desktop -p farhelm-helm-client`, on the test worker (recorder run
  `17e7b0ba-3ae8-4b0c-addb-eb51b95bddec`): all 2163 passed, including both repaired migration tests and the new
  misspelled-field cases, none silently skipped.
- The four clippy runs and `python3 -B scripts/dependency-notices.py --check` (the notices file is current), on the test
  worker, and the website build (`bun run build` in `website`), the formatter, the sleep check, `dprint check` and the
  changelog format check locally: passed.

Reused for the fix: the rest of the workspace suite and the screenshots, the brick test suite and the other source
checks above, since the fix only adds a launch switch that is off unless the desktop app's launch turns it on, tightens
how feed entries are read inside the helm, and corrects one test's rewind and some text. Skipped: the native Mac brick
test check, which needs a published release build and the signed test feed (the report says so too); the end-to-end
desktop smoke, whose steps do not open the bell's announcements and whose app launch the fix changes only by one switch
the desktop tests already cover; macOS, since no Mac was used; and CI, which runs only on demand.

#### What the report says that is no longer true

- "Development builds and a switched-off preference skip network checks": so does any helm the desktop app did not
  launch, which in practice means every helm a test starts.
- "The focused selections cover the changed behavior, so the full workspace suite was skipped": the landing ran it, and
  it found the two failing migration tests above.

#### Things you should know that the report does not say

- **Sign the test feed differently from the real one, before you publish it.** The standing test feed the release brick
  test reads (`announcements-test.json` and its `.minisig` signature) is signed with the same trusted comment,
  `farhelm announcements`, as the production feed, and the signature does not cover the file name. If those two files
  were ever served at the production path, by a mistaken upload for instance, every Mac would accept "Farhelm brick test
  announcement" as a real announcement. Nothing is signed yet, so this is cheap to change now: I recommend giving the
  test feed its own trusted comment (for example `farhelm announcements test`) and having the brick test's launch option
  that points the app at the test feed require it. That touches your signing workflow, so I left it to you. I also
  recommend giving every entry an `expires_at`, since an old signed feed stays valid and could be replayed.
- **One unknown value or field rejects the whole feed.** An entry that targets an OS this release does not know (a
  future `"windows"`, say) makes every helm of this release ignore the entire feed, including entries meant for it, and
  after the landing fix the same now holds for a field this release does not know. That follows the spec's rule that
  format errors leave the queue alone, but it means the feed format cannot grow without cutting off this release. I
  recommend skipping only the entries a release cannot read, unknown fields included, before the format gains anything
  new; a skipped entry reaches nobody, which is also the safe outcome for a typo.
- **Pending entries are not checked again when shown.** Version bounds and expiry are applied when an entry is queued,
  not when it is displayed. An entry meant for one version can still open after the user has updated past it, or after
  it expired, if it was queued but not yet shown. I recommend filtering pending entries again when the dialog reads
  them.
- The 12-hour timer counts only time the machine is awake, so on a computer that sleeps a lot the check runs less often
  than every 12 hours of wall time. I suggest no change.
- When the bell's **Open Announcements** finds nothing pending (another window showed them first), it does nothing
  visible, and a failed read is only logged. I suggest no change for now.

#### Possible follow-ups

- The test feed's own trusted comment, and `expires_at` on every entry (above); I recommend the first before signing the
  standing test feed.
- Skipping unreadable feed entries one by one, unknown fields included, instead of refusing the whole feed.
- Filtering pending announcements by version and expiry when they are shown.
