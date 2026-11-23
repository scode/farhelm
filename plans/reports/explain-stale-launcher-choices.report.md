# Explain stale launcher choices

## What this was about

The session launcher could clear the search when someone clicked a host that had just disappeared. Pressing Enter on a
recent setup that had disappeared could also do nothing. The maintainer asked for the same inline warning for these
cases, vanished templates and any other explicit keyboard choice that is no longer offered.

The launcher now shows “That choice is no longer available; choose a current result” beneath the command field. It keeps
the search text, closes the results and clears the old highlight. A click or Enter meant for that choice leaves the form
alone.

## Things you should know

A registered host can point at a different installation while keeping the same host entry. A recent setup or saved
folder picked before that change needs a new selection, even if the replacement offers the same choice. Past models
offered by the helm's history of launches into new environments remain usable when the chosen host changes. In a narrow
window the results popover closes so the warning is visible.

## Open questions and possible follow-ups

None.

## PRs

- Show the warning and keep the query after a choice disappears.
  [#4684](https://github.com/scode/farhelm/pull/4684/changes)

## Checks

- Ran 157 targeted launcher tests on `1b161e2f8` using pinned nextest through the recorder
  (`9d985f73-98a8-4d9b-8a21-3604e2ad7be0`). The tests covered a click on withdrawn hosts, templates and recent setups,
  Enter on a withdrawn choice, the next deliberate choice, and replacement installations with matching history. All
  passed with no runtime skips.
- Ran the strengthened past-model regression on `216ca63a` after adding two matching results so the test could
  distinguish the arrow choice from the default (`e8410de2-445a-485e-a921-8cfdfab397e6`). It passed. This revision
  contains the same launcher behavior and screenshot fixture. The screenshot catalog passed its 47 tests on the updated
  manifest (`b61a3b8c-ea62-4be7-9144-b5a70bb4968d`).
- Ran one full Linux golden update with four renderer processes before publishing
  (`831fd1de-5ed0-425f-8154-8fd7db990627` at `1b161e2f8`). Both new screenshots matched the visually approved captures
  byte for byte. The other 408 matched their references. Published the two new Linux references.
- Ran all-target Clippy with `-D warnings` at `1b161e2f8`. Later edits strengthen one test and record the two screenshot
  references, so the earlier production lint and screenshot coverage apply. Formatting, whitespace, changelog format and
  Rust test delay annotations passed.
- Skipped workspace-wide testing and macOS rendering. The changed behavior was exercised with the desktop test harness
  over a fake helm and the full Linux screenshot comparison. The warning uses the existing desktop layout and styling.

## Review gate

Sol spotted that the same past model can come from helm-wide history and remain offered even when a host's installation
changes. The correction keeps that choice available and rejects old host-specific choices. Sol also asked for tests that
distinguish an identical setup from a replacement installation and an explicit past-model choice from the default. Both
tests now exercise those cases. The source review gate passed after these corrections. Opus approved both appearances of
the warning.

### Landing

#### What landed with it

The plan's one PR (#4684) landed in a round with codex-recipe-credentials-at-launch, approval-card-program-wording and
wayland-clipboard-file-paste, none of which touches the launcher's search or recent setups. Since the plan's base, main
had gained the retirement of the "Agent's own setting" permission, which changed the launcher's Restart with, Replace
with and recent setups. The rebase met SPEC.md's launcher paragraph, which main had reworded to drop the old setting,
and the reference picture list, where main had republished the launcher's YOLO question; the resolution keeps main's
paragraph with this plan's new paragraph about choices that disappear, and both plans' pictures. The independent
reviewer confirmed both, and that Restart with, Replace with and recent setups keep main's handling of the retired
setting.

The reviewer found the behavior as decided: a choice that disappeared consumes its Enter or click, shows the warning and
keeps the search text, and a recent setup taken from a host's history is tied to that host's installation, so it is
refused after the host is pointed at another installation but still works after a reconnect to the same one.

#### Fixes made while landing

None.

#### Checks

This round landed codex-recipe-credentials-at-launch (how Codex gets a recipe endpoint, in the helm and the supervisor,
with the protocol version bumped), explain-stale-launcher-choices (the desktop launcher), approval-card-program-wording
(the helm's approvals and the desktop's approval cards) and wayland-clipboard-file-paste (the vendored GPUI Linux
crate's clipboard and the terminal's paste) together. Between them they reach every crate except the desktop app's outer
program, so the landing ran the whole workspace suite, the lints (the desktop app's too), the end-to-end smoke test,
every reference picture and the docs site build on the four plans stacked on main, then narrower runs after the landing
fixes. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH
to itself; the rest ran locally on x86_64 Linux. The first worker attempt (recorder run
`ef6ed206-f59e-44b9-9a19-e73ac2138ffe`) ran out of disk before it compiled anything; the disk was full of this landing's
own earlier build output, which was cleared, and the worker's runners now clean up after themselves (#4701). None of the
runs used a real agent program, a real Tensorlake account or a Wayland file manager: the tests use stand-ins, as the
reports say.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `2dd259d9-fdaa-4006-b3ad-bf8181a7df43`): 8435 of 8437 passed. The two failures are Tensorlake setup tests that
  codex-recipe-credentials-at-launch broke, which its landing fix (#4693) corrects; the failed run is kept. One test, a
  check that stopping a session's containers reaches every selected scope, printed a runtime `SKIPPED` because it found
  no usable systemd user manager that time; run again on its own (`cargo nextest run -p farhelm --test e2e` selecting
  only `lifecycle_stops_containers_in_the_selected_scopes_before_signalling`, run
  `1d248544-da0e-41c5-a68b-d4113c9881c1`) it ran fully and passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `3b7e19df-96bb-4ec7-854c-83031f92e389`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `f5a29c37-701b-4967-b9cd-7517d44f437b`): every picture
  but two matched its reference; the two are the terminal's covered-link scene, light and dark, whose approval card
  approval-card-program-wording rewords, republished by its landing fix (#4694). A full `--update` (recorder run
  `9fe8cb65-be4a-456f-af51-d4310849b6d8`) found those two the only changes.
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot scene list's own checks, locally and not through the
  recorder, so it has no run id), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`,
  `dprint check`, `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in `website/`),
  locally: passed.

After the landing fixes (#4693 and #4694):

- `cargo nextest run -p farhelm-helm -E 'test(/tensorlake_add::tests::|credential_delivery/)'`, on the test worker
  (recorder run `1974b03e-abe5-4a18-b7c7-9716db0f0872`): all 65 passed, the two corrected tests among them.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker,
  with #4693 on main and the other three plans on top: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

Reused: the workspace run and the smoke test above for everything else, since #4693 changes only those two tests, a
comment and a release-notes draft, and #4694 only two reference pictures' manifest entries. Skipped: the tests of the
desktop app's outer program (the part that starts the window and the embedded helm), which nothing here reaches; macOS,
where nothing compiles differently and the Wayland change does not apply; real agent programs, a real Tensorlake account
and a real Wayland file manager; CI, which runs only on demand.

#### Things you should know that the report does not say

- Three queued review findings look resolved by this plan but are still in the review queue: a reconnect that discards
  the search selection, a vanished recent setup that traps the launcher's Enter, and a recent setup's highlight that
  ignores the host's installation. Triage can close them.
