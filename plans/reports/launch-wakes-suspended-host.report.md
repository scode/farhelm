# Launching on a suspended host

## What this was about

Tensorlake suspends idle sandboxes, and users can return to a stopped container. The desktop launcher used to send the
user to Hosts to bring it back before it would start a session. The agreed behavior was for the launch itself to wake
the host and wait for a fresh connection, including when the session needs a new GitHub checkout.

## Things you should know

The launcher hides after the press and a small status pill appears at the bottom of the window. The user can keep
working in other sessions. Once the host connects, a fresh GitHub checkout gets its folder preview on that connection. A
typed name still asks for its exact folder. A template's base name can choose a numbered folder. If another launch takes
the chosen folder after preview, the launcher shows the new path. The user presses Launch again to accept it.

In that window, New remains busy. Suspend, Resume and Stop and suspend are unavailable on the destination host. Host
setup and host registry changes also wait. Restart with and Replace with hold their source session through preparation.
Resume can hold up other requests from the same window for up to 20 seconds, as agreed while the separate work on
parallel requests is pending.

Busy retries and connection waiting share one 60-second budget, paused during this launch's accepted Resume. On expiry a
refusal says the session was not launched. The draft comes back immediately if the user is still waiting for it.
Otherwise a notice offers the draft. A host arriving after expiry cannot start that session. A checkout preview can
finish after the minute once the host has connected.

## Open questions and possible follow-ups

An open launcher can still launch from a recent checkout by pressing Enter on that setup, if it read the history before
suspension and the installation still matches. Opening a launcher while the host is already suspended still works from a
repository choice or template. Showing recent checkouts on that first opening is a possible follow-up because the helm
currently needs a connection to read them. The historical browser test simulated suspension in its window while the helm
could still read history from the connected host.

## Pull requests

- Disable conflicting host controls during an existing-host launch.
  [#4564](https://github.com/scode/farhelm/pull/4564/changes)
- Wake an existing host before launching into a folder. [#4568](https://github.com/scode/farhelm/pull/4568/changes)
- Wake an existing host before previewing a fresh checkout. [#4573](https://github.com/scode/farhelm/pull/4573/changes)

## Checks run, reused and skipped

- Recorded Linux `cargo test --locked -p farhelm-gui --lib` selections for all three drafts. The Hosts, session
  environment and prompt selection covered #4564's Rust changes at `6645703496fd` and passed 94 tests (run
  `39937253-3edf-44ec-bdaa-71c5cca1f011`). The wake, launch requests, pending launches, template dispatch and session
  header selection covered #4568's Rust changes at `7e84831ebe2d` and passed 96 tests (run
  `ebea15fb-f2aa-4aa0-8670-44643630d1ff`).
- For #4573, the checkout, folder defaults, recent setups, launcher search, wake and template dispatch selection passed
  178 tests and found one test-fixture failure (`d69cc385-e0fc-472d-a1e2-f911b776faed`, with the checkout changes in the
  working tree on `7e84831ebe2d` before the last fixture correction). Reused the passing results for tests unaffected by
  the fixture correction. Reran all eight affected checkout wake tests with the corrected fixture in `4807af3b1455`,
  including the marked and numbered shortcuts, launching from a recent checkout and both sides of the time limit. All
  eight passed (run `a1d3cebe-6314-4d6e-9df1-dd21650ab5e2`).
- `python3 -B scripts/check-gui-goldens.py --update` rendered the full Linux catalog for each draft. The selected
  comparisons matched both appearances. The Stop confirmation at `6645703496fd` matched in run
  `d3889b6f-8f1d-4249-9515-64711f9a2eba`. The preparation pill at `7e84831ebe2d` matched in run
  `df65ca70-0b35-4f1d-ba2b-553e94759bba`. The allocation notice at `4807af3b1455` matched in run
  `b0da26f5-d1a8-45e0-808d-449faa2cb687`. Formatting, changelog and test-delay checks passed.
- Main gained the shell-tab fix for shortened session lists. The rebase preserved both sets of screenshot references.
  The combined `SceneList.test_the_committed_scene_list_is_well_formed` check passed (run
  `8075e6b0-5fa2-431c-a06c-a116b15a1cd8`). The rebased drafts have the same diffs as the reviewed and tested drafts.
  Shell-tab retention reacts to an omitted session row, a different event from the host reconnecting during a launch.
  The earlier desktop tests and render checks still cover the changed behavior.
- Skipped the full workspace suite and a real provider wake. The change is in the desktop launcher. The fake helm
  exercises the request, ordered replies and pushed host states. Live provider behavior was outside this check.

## Review gate

Each draft passed the separate Sol high source review after its findings were addressed. The Stop confirmation,
preparation pill and checkout allocation notice each passed the Opus 5.5 high visual review in both appearances. The
stack rebase preserved the reviewed changes.

### Landing

#### What landed with it

The plan's three PRs (#4564, #4568, #4573) landed first in this round, with a landing fix, #4582, followed by agent
trust in every folder, SQLite maintenance and the test worker's staging lock. Since the plan's work was written, main
had gained the desktop app triage and the Tensorlake triage. The rebase conflicted only in the GUI screenshot scene
registry and its reference list, where both sides had added entries; both were kept.

An independent reviewer read the stack against the plan's acceptance criteria and Decisions and against main's desktop
app triage, which changed the same launcher, Hosts and menu files. It found no blocker:

- **Behavior.** Every acceptance criterion and Decision is met, with no helm, protocol or supervisor change. Each launch
  acts only on replies to its own wake, so a late reply from an earlier attempt cannot launch into a later one, and a
  launch is refused if the host comes back as a different installation. The preparation indicator is always taken down.
- **Desktop triage on main.** No functional conflict: the stopped container waiting for its container host offers no
  Resume, which agrees with this plan's rules, and pressing Enter in Replace with… cannot trigger the launch that
  continues after the host wakes.

#### Fixes made while landing

All in [#4582](https://github.com/scode/farhelm/pull/4582/changes):

- **Replace with on a suspended session.** The plan keeps Replace with… and Restart with… available while the
  environment offers Resume, since their launcher resumes the host first. An existing session-menu test still expected
  Replace with… to be refused, so the workspace suite failed. The test now expects it to act and still checks that plain
  Replace and Stop are refused; the session menu's and header's code notes say the same.
- **The docs website** still told users to resume a suspended host before launching (Start a session, Tensorlake
  sandboxes) and used a suspended host as its example of a failed quick launch (Launch templates). They now say the
  launcher resumes or starts the host and what it shows meanwhile.
- **Two lint errors** surfaced on current main: a needless `&` on a palette, and a function type too complex for clippy,
  now behind a named alias.
- **SPEC.md's container paragraph** no longer limits starting a stopped container to launches "in an existing folder".

#### Checks

This round landed four plans stacked in this order on main: the launch that resumes a suspended host, agent trust in
every folder (protocol 84), SQLite maintenance, and the test worker's staging lock. The checks ran on the combined
result. The worker runs used this round's own test worker harness, so they exercised it too; they ran on a separate
arm64 Linux test worker, and the rest ran locally on x86_64.

On the four plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `fa0b5b85-0c85-4685-a517-30a4dc406bf9`): 8304 of 8308 passed. Two failures were the
  worker running out of disk during the run (helm provisioning over SSH and the real Podman environment test, both "no
  space left on device"); the other two were the GUI tests the trust and wake plans' landing fixes correct.
- The four clippy runs (`--all-targets`, `-p farhelm --bins`, and the desktop app with and without `--features agent`),
  on the test worker, not through the recorder: two errors, which the wake plan's landing fix corrects.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `99f4ecdb-f251-4c4c-bdcb-84747ee38556`): all
  184 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `0b374828-a0ef-49ad-9af6-966eeace3dad`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `dbe2bda8-bda8-4af3-b9b6-ffc64f569658`):
  passed.
- `python3 -B scripts/test-worker/test-test-worker.py`, the worker harness's own tests against a local SSH stand-in,
  locally (recorder run `f6f57fc8-5f99-4ac8-8038-9994d9ce98de`): all 9 tests passed (the report counts eight scenarios);
  `shellcheck -x` on the harness scripts and `python3 -m py_compile` on its Python files: passed.
- The docs website build (`bun install --frozen-lockfile && bun run build`), `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

After the landing fixes (#4582, #4583, #4584):

- `cargo nextest run --workspace --exclude farhelm-desktop` again, on the test worker with free disk (recorder run
  `442411a7-47a3-448c-be2c-fb6e4582df74`): all 8309 passed, among them the two tests the full disk had failed, the two
  corrected GUI tests and the new home-directory test.
- The four clippy runs, on the test worker: passed.
- `python3 -B scripts/test-gui-screenshot-batch.py` (which would have failed before #4583 added five trust screenshots
  missing from the screenshot list), the docs website build, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py`, locally: passed.

The first runs' desktop, smoke, golden and harness results still apply. The fixes change no desktop-app or harness code
and nothing that draws: in the GUI library a removed `&`, a type alias, comments and tests; in the helm and the
supervisor only Claude agent requests and Claude trust launches, which the smoke test's stand-in agents never make.

Skipped: a launch onto a real suspended Tensorlake sandbox or stopped container, and a real Claude launch with trust on,
which spend cloud resources or vendor turns; macOS, since no Mac was part of this landing; CI, which runs only on
demand.

#### Things you should know that the report does not say

- New with this plan: on a suspended environment, the session header's menu now opens, and its disabled plain Restart
  and Replace explain "Wait for this host to connect", which never happens for a suspended host; "Resume this host
  first" would be right.
- A launch onto a host that already shows a failed Resume and fails again with the same text says the host was suspended
  again. This is rare.
- Replace with… and Restart with… on a suspended host are tested only when the resume fails; no test covers the host
  resuming and the session launching. The enabled header menu on a suspended environment has no golden picture.
