## What this was about

When Farhelm quit with its Mac window maximized, its next launch showed the ordinary-size window first and then grew
into place. Farhelm now calls macOS's Zoom before showing the window. macOS keeps the ordinary size and position so the
next Zoom can return to them.

## Things you should know

- The macOS build and first visible frame are unverified. If there is a Mac compile error, devbuilds and the next Mac
  release build would fail after this lands until the error is fixed. The executor and an independent reviewer read the
  macOS changes for type and AppKit issues.

## Open questions and possible follow-ups

Run this on a Mac before the next release to verify the startup fix.

```sh
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/devbuild-github.sh | bash -s -- 4547
```

Use the start command it prints. Note the ordinary window's position and size, zoom it, quit, and relaunch. The first
visible frame should fill the display's work area without a grow animation. An ordinary-size window with no animation
means startup Zoom failed. Compare launch time with an ordinary launch. Choose Window → Zoom to restore the original
position and size, then choose Zoom again and confirm it animates as usual. If the hidden zoom delays launch, a
follow-up could suppress the resize animation only for startup.

## Pull requests

- Open a restored maximized Mac window at its zoomed size. [#4547](https://github.com/scode/farhelm/pull/4547/changes)

## Checks

- Linux passed `cargo build --locked -p farhelm-gui -p farhelm-desktop` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-desktop --all-targets -- -D warnings`.
- GPUI passed
  `cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record --test hitbox_occlusion`
  (5 tests, recorder `c35c9604-cb6a-453e-87d6-0f481b1d436e`).
- VTE passed `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi` (62 tests, recorder
  `a55014b3-d03c-417d-99ab-7e5a3f145912`). Alacritty passed
  `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml` (166 tests, recorder
  `6f79bb93-057e-45a0-ad45-196e2aa178db`). Paired terminal tests passed
  `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml` (3 tests, recorder
  `09ddb2d2-8c70-47fe-9a33-de1a4a410205`).
- `git apply --reverse --check` passed for both GPUI patches, and a scratch reverse-application and file comparison
  recovered the original files byte for byte. `dprint check` on the changed Markdown,
  `python3 releasing/check-changelog.py format` and `rustfmt --edition 2024 --check --config skip_children=true` on the
  four edited Rust files passed. `python3 scripts/check-agents-md.py` reported zero errors and four existing reference
  warnings. `cargo fmt --all -- --check` exited unsuccessfully over a pre-existing formatting difference in the What's
  New dialog.
- Reused the Linux build, vendored test runs and source review after rebasing their base from `e1889c9919c4` to
  `00e65c60536d`. Only the queue and a supervisor plan report changed upstream. The patch was byte-identical and the
  tested sources match draft head `50b8a5b49653`. Clippy checked those same sources and completed after the rebase.
- Skipped the Mac build, native text tests and first-frame check because this host runs Linux. The frame keeper and its
  tests did not change. Screenshot comparisons cannot exercise native opening or Zoom.

## Review gate

The required independent GPT-6.2 Sol source review at high effort found no actionable issues. It checked both sides of
the macOS-only field, the ordering of native frame positioning, Zoom and presentation, the callback lock, and the
existing paths for user Zoom and the initial frame report. A cold wording pass checked the commit and draft PR text.

### Landing

#### What landed with it

The plan's PR, #4547, merged alone in this round, with a landing fix, #4557. Since the plan's work was written, main had
gained the supervisor triage plan and a TODO entry; neither touches window opening or the vendored GPUI library, and the
rebase had no conflicts.

An independent reviewer read the change, the macOS window-opening code it edits, the code that runs while the window
zooms, and how Farhelm remembers the window frame. It found no blocker:

- **The order of steps.** The window is put at its ordinary position before the hidden zoom, so macOS remembers the
  right size and position for Window → Zoom to go back to. The old step that corrected the position after showing was
  removed, because it would now move the already-zoomed window. Zooming before showing runs Farhelm's resize handling
  right away, and the reviewer confirmed this cannot deadlock. Both code paths that open a window pass the new setting.
- **Remembering the frame.** The window's first size report after opening already says maximized, which is what Farhelm
  waits for before it starts saving window moves and resizes, so the remembered ordinary size and position are left as
  they were. Fullscreen, ordinary windows and Linux are unchanged.
- **Vendoring rules.** The change to the vendored GPUI library comes with the patch files and `vendor/README.md` entry
  the vendoring rules require, and the patches match the code.

#### Fixes made while landing

All in [#4557](https://github.com/scode/farhelm/pull/4557/changes), wording only:

- **The changelog fragment** said the window "opens at its zoomed size on the next launch", which it did before too. It
  now says the window appears already zoomed instead of opening at its ordinary size and growing into place.
- **Why "maximized" allows a two-point gap.** SPEC_impl.md and the desktop parity notes for the GPUI migration
  (`docs/gpui/parity/desktop-integration-and-linux-packaging.md`) said a zoom came out a point short, as if that were
  still true. That was the old zoom after showing; they now say so, and that nobody has checked on a Mac whether the new
  zoom has the same gap. The Mac check could show whether the tolerance is still needed.
- **SPEC.md** said Zoom returns the window to "the ordinary frame it opened from"; it never visibly opens there now, so
  it says "its remembered ordinary frame".

#### Checks

The plan changes Mac-only window opening in the vendored GPUI library, with its patch files, and a comment in the GUI
library. The landing machine runs Linux and the shared test Mac was unreachable, so nothing ran on a Mac. Instead the
landing type-checked the Mac code from Linux and ran the Linux checks that cover the touched crates and the saving and
restoring of the window frame. The worker runs were on a separate arm64 Linux test worker; the rest ran locally on
x86_64.

On the plan as delivered, rebased onto main:

- `cargo check --locked --target aarch64-apple-darwin -p gpui-pre-macos`, locally, in a separate target directory with a
  Mac-only shader file replaced by an empty placeholder: passed with no warnings. It type-checks the vendored GPUI core
  and Mac crates with the change; it neither links nor runs anything. The GUI library and desktop app could not be
  checked this way, because a TLS dependency needs Apple's C toolchain.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the
  same with `--features agent`, on the test worker, not through the recorder: passed.
- The vendored GPUI patch tests
  (`cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support` with its three test
  targets), on the test worker (recorder run `4fa0ce8a-b514-4181-984b-fe1e103778eb`): all 5 passed.
- `python3 -B scripts/gui-lifecycle.py`, the Linux app lifecycle test with maximized save and restore, on the test
  worker (recorder run `dc1b36c7-a5d9-4401-af1f-8ffde916ff03`): every step passed.
- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py`, locally: passed. The What's New formatting difference the report mentions was
  fixed on main by the previous round's landing.

After the landing fix (#4557), which changes only Markdown: `dprint check`,
`python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

Reused from the report: the vendored VTE, Alacritty and paired-terminal tests, which the change does not touch. Skipped:
the Rust workspace suite, the GUI reference screenshots and the end-to-end smoke test, since Linux behavior and drawing
are unchanged; CI, which runs only on demand.

#### Things you should know that the report does not say

- The report warns that a Mac compile error would break devbuilds and the next Mac release. The vendored GPUI code the
  plan changed now type-checks for the Mac, so a compile error there is unlikely. Farhelm's own GUI library and desktop
  app were not compiled for the Mac, but the plan changes only a comment in them.
- If the Mac check shows the hidden zoom does nothing (the window opens at its ordinary size with no animation), the
  damage is worse than a missing animation fix. Farhelm stops saving window moves and resizes for that whole session,
  until the user zooms by hand, and because the window is still remembered as maximized, every later launch hits the
  same thing. That would make a fix urgent.
