### What this was about

When a visible host first needs an SSH answer or reports a setup failure, an amber light travels from the center of the
Farhelm window and flashes its row. Farhelm read the desktop's Reduce Motion setting at launch. On Linux, the GUI
toolkit followed the desktop portal while the app was open. If those readings differed, Farhelm could start a moving
attention cue that the toolkit skipped to its transparent last frame. The user saw no cue at all. The plan called for
one live setting and for clocks and deadlines to keep running when animations pause.

### Things you should know

- On Linux, Farhelm follows changes from the desktop portal. On macOS, the app refreshes the setting when its window
  regains focus, before any queued attention cue starts.
- An attention cue that is already flying becomes a stationary flash through its original expiry. That cue stays put if
  motion is enabled again. The launcher banner and quick-launch pill keep their original animation start time so a
  completed entrance cannot play again after a preference change.
- With reduced motion on, the terminal cursor stays visible, status pulses steady, and spinners pause. The trash icon
  loses its lift or tilt from an animation. The icon still shows when space is being freed, and ages, counters, and
  one-shot deadlines keep running.

### Open questions and possible follow-ups

An optional native macOS check is to switch away from Farhelm, change Reduce Motion while an attention cue is waiting,
and return to check that the cue uses the new setting. The activation order was verified in source, and the open-window
toggle tests ran on Linux.

### PRs

- Adds the GUI toolkit's live preference observer. [#4681](https://github.com/scode/farhelm/pull/4681/changes)
- Makes the desktop app follow that preference and covers changes in an open window.
  [#4687](https://github.com/scode/farhelm/pull/4687/changes)

### Checks run, reused and skipped

- Passed 93 focused GUI tests and 723 terminal and session-list tests with pinned nextest on Linux. The runs cover
  changing the preference in an open window, keeping both clocks and deadlines live, drawing the stationary attention
  flash, clearing the trash animation, and restoring motion without replaying a completed entrance. Runs
  `d9a6d774-df6b-4d8a-a0bf-771d432b0432` and `f214578a-cbc6-4c45-b32c-bb1ef8124048` cover source `15b7b93c`. The later
  change to a screenshot fixture does not enter these tests.
- The first run exposed a session-list test that counted the preference-change redraw as animation, run
  `85a588bb-3658-48a3-8f05-ce6662cfbf50`. The test now checks that redraw before measuring a quiet second.
- Ran `python3 scripts/check-gui-goldens.py`. In the first full comparison, `1af1a337-1298-4db6-9f1b-05fad04437f4`, two
  Delete screenshots showed ordinary session rows because a redraw could stage them before the fake helm had connected.
  The scene now waits for the live fixture. The focused light and dark comparison passed in
  `47b2dc5c-67e7-4ca6-85e4-2fb72296bb4e`, then the full Linux golden catalog matched its references in
  `4e6d9d9e-f9ef-469f-9aef-8cc0dd4e24e9`.
- Recreated the vendored GPUI tree from its checked archive and replayed its patches, matching the checked-in source.
  Reused six standalone GPUI test results from `7c0c3eab-813c-406c-a13c-6ea4bb6408a8` at `45bab938`. The later observer
  change was a documentation comment and the rebase kept the patch intact. The vendored terminal suites also passed,
  with 62, 166 and 3 tests in `91c64b48-c4eb-4550-a15e-1904b9f37035`, `220b478c-af43-45b7-a4f5-6dc55e348e05` and
  `09fc77f8-7857-480a-8772-f72898f6a822`.
- Passed `cargo check --locked -p farhelm-desktop`, strict Clippy for the workspace defaults and separately for the
  desktop crate, Rust and Markdown formatting, changelog format, and the test-delay checker. The instruction checker
  passed with one existing archived-heading warning. The desktop checks preceded the screenshot fixture change and still
  cover the desktop binary.
- Native macOS runtime coverage requires a Mac. The Linux runs targeted the affected desktop behavior and vendor
  patches. The full Rust workspace battery and CI would add unrelated coverage.

### Review gate

The requested independent source reviewer checked both PRs. The observer review clarified the callback contract and
registered its new test in the recorded suite. The desktop review caught replay of a finished transition when motion was
restored. Its retained animation state fixes that, and the reviewer verified tests for the visible cue, immediate timer
cancellation, session-list redraw, and screenshot fixture readiness. The final source-review disposition is pass for
both drafts. Both PR descriptions passed a separate cold read.

### Landing

#### What landed with it

The plan's two PRs (#4681, #4687) and one landing fix (#4707) landed first in a round of six plans; the others
(fallback-unattachable-remembered-tab, fix-muse-conversation-reporting, gui-goldens-in-releases,
repo-cache-lock-redesign and brick-test-remote-mac-uninstall) touch neither motion nor the vendored GPUI core. Since the
plan's base, main had gained, among others, the Wayland file paste, whose own vendored GPUI patch changed the same patch
table in vendor/README.md. The rebase met that table and its totals line: the resolution keeps this plan's table, which
adds a reduced-motion row, together with main's updated clipboard row. TODO.md conflicts were only entry removals.

The landing's independent reviewer confirmed that the vendored GPUI tree matches its new patch file exactly, that a
change of the setting reaches the app's animations live on Linux, and on macOS when the window becomes active, while
clocks and deadlines keep running, and that every reference picture still matches on the combined stack.

#### Fixes made while landing

- The reviewer found three code comments and a note in the GPUI design docs still describing the removed design (the
  fade-in component reading the setting from the app's animation ticker, and the desktop binary handing the app the
  setting at startup). #4707 corrects them. All of them are in Farhelm's own crates and docs, not in the vendored GPUI
  tree, so the tree still matches its patch file.
- vendor/README.md's list of Farhelm's own GPUI patch layers, and its sentence on which layers mark their files, left
  out the new reduced-motion layer, and its total of Farhelm's GPUI changes was wrong. #4707 corrects all three.
- #4707 adds the first Linux caveat below (some desktops lose reduced motion) to the release-notes draft. The second, a
  brief entrance animation at startup, is left out of it as too small for release notes.

#### Checks

This round landed toolkit-reduced-motion (the vendored GPUI core, the desktop app's outer program and the desktop
window), fallback-unattachable-remembered-tab (the helm's terminal relay, its message format, the helm client and the
desktop's terminal), fix-muse-conversation-reporting (the supervisor's Muse hooks and the farhelm command's hook),
gui-goldens-in-releases (instructions only), repo-cache-lock-redesign (the supervisor's repository caches) and
brick-test-remote-mac-uninstall (the release brick test's harness) together. Between them they reach every crate, so the
landing ran the whole workspace suite, the desktop app's own tests, all lints, the end-to-end smoke and Linux lifecycle
suites, the vendored GPUI core's own tests, the brick test harness's portable tests, every reference picture and the
docs site build on the six plans stacked on main, then narrower runs after the landing fixes. The worker runs were on a
separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH to itself; the rest ran locally on
x86_64 Linux. None of them used a real Muse Code, a real Mac or a real GitHub repository; the landing's tests use
stand-ins for those (the Muse plan's own report also ran a real Muse Code offline).

On the six plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `94c1ab85-1040-4d08-bd23-7e079a2f1ad7`): 8451 of 8454 passed and none printed a runtime `SKIPPED`. The three failures
  are spare checkout builder tests that repo-cache-lock-redesign broke, which its landing fix corrects; the failed run
  is kept.
- `cargo clippy --all-targets -- -D warnings` on the test worker failed once, on a function the landing itself had given
  an eighth argument while merging fix-muse-conversation-reporting with main's Codex credential work; see that plan's
  notes. After that was corrected it passed, with the other lints, as listed below.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `4d9ea5a4-7ef3-4809-af75-6c2ee1a832f2`): all
  184 passed.
- The vendored GPUI core's tests of Farhelm's patches, `cargo test --manifest-path vendor/gpui-pre/Cargo.toml` with
  `--features test-support` and the four test binaries `.agents/validation.md` names (the new `reduce_motion` among
  them), on the test worker (recorder run `21c42e62-e85d-40eb-87a1-ad67ca5b23f0`): passed.
- The brick test harness's portable tests, `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, on
  the test worker (recorder run `7be643a1-4eab-4d1c-88c2-fab46002f967`): all 315 passed.
- `python3 -B scripts/gui-e2e-smoke.py` and `python3 -B scripts/gui-lifecycle.py`, on the test worker (recorder runs
  `33d62eb8-f64c-4de5-8c8d-76496785f56d` and `f451571c-c20a-45a5-b779-256fd6f1d490`): of the smoke test's 50 steps, 49
  passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app); every lifecycle step
  passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `6a8c0f83-8308-4330-873a-afc10d70e032`): every picture
  matched its reference on the combined stack.
- `python3 -B scripts/test-gui-screenshot-batch.py` (a self-test of a script that starts nothing, which the validation
  guide lists without the recorder, so it has no run id), `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and the docs site build (`bun run build` in `website/`), locally: passed.

After the landing fixes:

- The three vendored terminal crate suites, which the validation guide asks for whenever anything under `vendor/`
  changes (`cargo test` for `vendor/vte` with `--features ansi`, `vendor/alacritty_terminal` and `vendor/pair-tests`),
  on the test worker (recorder runs `1a1b556e-ae84-4955-862f-6194d6a7666e`, `a4d64bd6-a402-4686-b307-4be230c334ed` and
  `a322826a-3d5a-4799-bf98-898c9926208b`): 62, 166 and 3 passed. The round's own change under `vendor/` is in the GPUI
  tree and its notes, which these crates do not build from.

- `cargo nextest run -p farhelm --test spare_builder`, on the test worker (recorder run
  `77ae841d-669f-4777-8e54-c5b326d2496b`): all 13 passed, the three corrected tests among them.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the docs site build, locally:
  passed.

Reused: the workspace run, the desktop app's tests, the smoke and lifecycle suites and the reference pictures above for
everything else, since the fixes change only the spare builder tests, comments, docs, release-notes drafts and one lint
attribute. Skipped: macOS, where the reduced-motion and remote Mac uninstall changes have their own paths that no run
here reaches, and where the repository caches meet macOS filesystems and Apple's Git; real Muse Code, a real Mac and
real GitHub repositories; CI, which runs only on demand.

#### Things you should know that the report does not say

- **Some Linux desktops lose reduced motion.** On Linux the app now takes the setting only from the desktop portal, the
  standard channel through which a desktop tells apps about its settings. The old startup check also read GNOME's own
  "animations off" setting directly, so a desktop whose portal does not pass the reduced-motion setting on now gets full
  motion even with animations turned off. This follows from your TODO entry, which asked to follow GPUI's preference
  everywhere and remove the separate startup check; neither the plan nor the report mentions the loss. Reading GNOME's
  setting as a fallback when the portal has no answer would be a follow-up, and it would bring back a second source of
  the setting, which is what the TODO entry asked to remove.
- On Linux the first moments after the app starts can show motion with reduced motion on, until the portal answers,
  since the setting is no longer read before the window opens: an entrance animation, such as a banner sliding in, can
  play once.
