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
