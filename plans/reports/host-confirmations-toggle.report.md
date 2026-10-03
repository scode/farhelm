## What this was about

The permanent answers in host setup and removal dialogs could turn their questions off, but there was no way in the UI
to turn them back on. The maintainer chose a gear immediately after the sidebar version, opening a settings dialog with
only those two choices.

## Things you should know

The gear opens **settings** with exactly two checkboxes: **set up new hosts without asking** and **remove hosts without
asking**. Unticking one restores its question; ticking one skips it. Each has help text describing the current behavior.
Both permanent-answer buttons now point to the gear as the place to undo the choice.

Changes take effect immediately in the current client and apply to every host on this helm. Other already-open clients
retain their previous behavior until they reload. The dialog states this explicitly. Preference writes keep the existing
best-effort behavior: a storage failure has no new error display.

The dialog follows the existing modal focus and isolation behavior, dismisses an open host menu, and returns focus to
the gear when closed. Long version stamps yield space to the gear, including in the macOS header layout. No other
settings moved into this dialog.

## Open questions and possible follow-ups

None. The implementation stayed within the agreed scope.

## The PRs

- [#1521](https://github.com/scode/farhelm/pull/1521/changes): restore host setup and removal confirmations through the
  sidebar gear. The PR remains a draft.

## Checks run, reused and skipped

- Ran the settings and setup-confirmation cases on Chromium and WebKit: 8 passed, zero skips or failures, recorder
  `f64a6e14-7ef4-464b-82eb-784996c5e644`. These exercise the actual preference route, both directions of both choices
  without reloading, menu dismissal, focus/isolation, and long-version geometry. Inspected the retained narrow-window
  screenshots in both engines.
- Ran six existing header/removal-dialog cases per engine: 12 passed, zero skips or failures, recorder
  `cc384e8a-ec34-42aa-90af-80fe75782274`. These cover macOS control clearance while scrolling, the build-mismatch
  notice, drag-spacer bounds and interaction, and removal-button reachability.
- Ran UI all-target Clippy, the desktop-feature compile check, Rust formatting, changed-Markdown formatting, changelog
  format lint and the isolated test-delay checker (274 delays, zero missing rationales). Built the CLI and release web
  bundle for browser execution. The website build and internal-link validation passed.
- Reused the implementation checks at `498ffbc5` after one attribute-indentation correction. Main advanced only through
  queue/report bookkeeping; its full incoming diffs changed no runtime code or contract used here. No repeated runtime
  checks were needed for those changes.
- Skipped full Rust/browser batteries, desktop runtime, installer, release and remote provisioning checks: this adds a
  UI over the existing preference path, and the focused browser checks cover its changed behavior. The desktop compile
  checks the shared component; native window dragging is unchanged. No pure helper was added that needs a separate Rust
  unit test.

## Review gate's outcome

The required fresh-context Opus 5.5 review at high effort found no correctness blocker. All seven minor findings were
addressed: hint alignment, accessible descriptions, stronger header bounds, accurate test documentation, checkbox
spacing, uniform icon strokes, and wording consistency. The documentation pass and final diff review are complete.
Commit/PR wording passed its fresh cold read; this report passed a separate cold read before delivery.
