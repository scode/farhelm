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
