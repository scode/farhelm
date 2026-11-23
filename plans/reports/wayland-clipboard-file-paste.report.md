## What this was about

On Wayland, copying files from a file manager and pasting into a Farhelm terminal could type their local paths or
produce nothing. The terminal now attaches local files in selection order. If the clipboard offers both local files and
other links, it attaches the local files, as requested. When the URI list contains no local files, Farhelm can still
paste an image or plain text from that selection. A copied browser link pastes as text if the browser offers it.

## Things you should know

A pending paste can be cancelled if the clipboard selection is replaced, the offer is destroyed, or Farhelm loses
keyboard focus before Farhelm finishes reading the clipboard. This also applies to an image-only or text-only offer.
Ordinary text fields still prefer plain text.

## Open questions and possible follow-ups

Validation used synthetic clipboard offers to exercise fallback and cancellation deterministically and a fake helm to
check the attachment behavior. A desktop smoke test can also check that files copied in a real Wayland file manager
attach in selection order.

## PRs

- [#4682](https://github.com/scode/farhelm/pull/4682/changes) attaches files pasted from a Wayland file manager.

## Checks

- Ran `dprint check TODO.md vendor/README.md releasing/changelog.d/wayland-clipboard-file-paste.md` and
  `git diff --check` after rebasing. Both passed.
- Reused the five standalone clipboard tests at `827305c83128` (run `dd10c52c-86ad-4e10-a38c-ea792bcb0444`). Used
  `cargo test --offline --no-default-features --features wayland,test-support --lib attachment_clipboard_tests` with a
  copy of the vendored Linux manifest and overrides to the vendored GPUI core and renderer.
- Reused
  `cargo nextest run -p farhelm-gui --lib -E 'test(terminal::attachments::) or test(only_the_clipboards_text_is_pasted)'`
  (27 tests, run `b657b038-3caa-45e3-a2e6-5a58633ea576`) and `cargo clippy -j 4 --all-targets -- -D warnings` (run
  `71f8edc1-108a-47fc-85b9-5cb0ca8d0969`), both passed at `827305c83128`.
- Reused
  `cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record --test hitbox_occlusion`
  (5 cases, run `a0f9c1ad-c71a-49bf-aa66-710ce5cf6079`) from `a824418c`. Review changed only the Linux backend and its
  patch, so the core test coverage still applies.
- Reused these terminal vendor checks from `a824418c`. Review left both terminal libraries and the paired tests
  untouched. `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi` passed 62 tests (run
  `1d0ea1d5-4989-4736-ac2f-fbe4f600c4b7`).
  `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml` passed 166 tests (run
  `e0ecc980-bcbd-4d58-acae-ff8e0d89e837`). `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml` passed 3
  tests (run `4240072b-6e07-4bf2-9104-2d67b7bf0b41`).
- Every GPUI patch layer applied to its pristine source and the five reproduced trees matched at `a824418c`. Replaying
  the revised Linux patch series at `827305c83128` also matched the checked-in tree.
- Reused `cargo fmt --all -- --check`, `rustfmt --check --edition 2024` on the two changed vendor Rust files,
  `python -B scripts/check-test-sleeps.py`, and `python3 releasing/check-changelog.py format`, all passed at
  `827305c83128`. The rebase kept the patch identical. Main changed Stop and suspend feedback and isolated a real-agent
  test setup, which have no bearing on clipboard offers or attachment transfer. Skipped the full workspace suite because
  the targeted cases covered these paths. The synthetic clipboard test setup needed no desktop session, so a live
  compositor and file manager smoke test was skipped.

## Review gate

Independent source review found three issues. Paste can now resume when reading the clipboard takes time, checks that
Farhelm still has keyboard focus before continuing, and skips a malformed remote file URL even if the URL parser can
repair it. All three findings were addressed and the reviewed candidate passed the checks above.

### Landing

#### What landed with it

The plan's one PR (#4682) landed last in a round with codex-recipe-credentials-at-launch, explain-stale-launcher-choices
and approval-card-program-wording, none of which touches the terminal's paste or the vendored GPUI code. Since the
plan's base, main had gained the retirement of the "Agent's own setting" permission and Settings keeping its list of
leftover Tensorlake sandboxes when a refresh fails, neither near the paste path; the rebase met only TODO.md, where this
plan's removal of its own entry was kept.

The landing's independent reviewer checked that the change to the vendored GPUI Linux code matches its patch file
exactly (every hunk, the patch's 406 added and 2 removed lines, and the totals in vendor/README.md), that only local
`file:` addresses are attached and only regular files uploaded, that files are tried before an image and an image before
text, with the fallback reading the same clipboard offer, that a replaced selection, a destroyed offer or lost keyboard
focus cancels the paste with nothing half pasted, and that X11 and macOS are unaffected. The plan said more than once to
leave the Wayland client code untouched, but the change adds the keyboard focus check there. The reviewer found that
Wayland's rules call for it; the report mentions the check but not that it departs from the plan.

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

- The five new clipboard tests inside the vendored GPUI Linux code run in no check the repository documents; the
  executor ran them through a temporary copy of the crate's manifest. Adding them to the vendored crates' checks would
  be a follow-up.
- The report says cancellation was exercised, but its test only marks the paste cancelled itself. Nothing tests the
  parts that decide to cancel (a newer clipboard selection, lost keyboard focus, a clipboard offer that went away). A
  follow-up could add such a test.
- The file list's addresses are parsed more strictly than drag and drop parses them, so a file manager that leaves
  characters such as spaces unencoded could have its files ignored and the paste fall back to text. The reviewer was not
  sure any file manager does this.
- The clipboard read has no size limit. That was already true for pasted images and text; the file list inherits it.
- The patch file's blank context lines are now truly empty rather than a single space. `git apply` accepts both, but the
  next regeneration of the patch will show it as a change.
