## What this was about

The Mac installer could report a successful update while leaving an app unlaunchable if a file it replaced had become a
folder or a link to one. A link could also send its writes outside the app. Separately, an install without a pinned
version could accept an incomplete latest-version answer as a valid older release. The UI's bundled terminal font and
clipboard add-on also lacked declarations that carry their full license notices into distributed copies. You chose to
fix these three findings together, provided each remained straightforward.

## Things you should know

All three fixes stayed within their complexity limits. An in-place update now refuses directory-shaped file destinations
before changing app contents, with repair advice. An incomplete latest-version transfer now refuses even when the server
reports success; it requests nothing beyond the version answer and creates nothing under the home directory. Existing
404 and other HTTP failure messages remain unchanged.

The font and clipboard notices now use the existing asset mechanism at stable, unhashed paths. The built web bundle
carries both texts byte for byte, and the desktop parity check confirms the desktop app's embedded UI uses the same
asset set. No vendored font, add-on or license bytes changed. The two installer defects were reproduced against unfixed
code before the fixed suite passed.

## Open questions and possible follow-ups

None. No outcome was dropped, no specification conflict arose, and no broader mechanism was needed.

## The PRs

- [#1906](https://github.com/scode/farhelm/pull/1906/changes) — one draft PR for the three accepted fixes and their
  outcome records; ready state and landing remain the monitor's job.

## Checks run, reused and skipped

- Passed now: installer shell syntax and ShellCheck; the recorded installer suite, 549 checks with zero failures, run
  `395e45e0-25fa-41df-8106-f2299b66489a`. Counterfactual run `8bb4f627-7cd5-4387-b7f4-a78a36be746e` retained 14 expected
  failures, all in the new regression assertions, with fixture premises and pre-existing cases passing.
- Passed now: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and the recorded
  focused inventory fence, run `ea3ae630-4a6a-4125-9c32-1af232325c5e`: one selected test passed; 453 tests were
  deliberately excluded by the filter.
- Passed now: recorded `scripts/check-desktop-assets.sh`, run `f73f91ab-b0a0-4d0c-ad84-42a4a8113244`: 26 requested
  assets equal the bundled set. Both unhashed license texts match their source files with `cmp`. This checks actual web
  distribution and the desktop embedded asset set.
- Passed now: changelog format and dprint on the changed Markdown. A separate documentation pass covered every touched
  file.
- Reused after the final rebase: the above execution evidence. Main's intervening changes only claimed two other plans.
  Later product edits only clarified comments; shell behavior, Rust behavior, asset declarations and bundled bytes
  remained identical.
- Skipped: optional Alpine/BusyBox execution, because the guarded assignment is plain POSIX and syntax, ShellCheck and
  the shell fixture suite cover the change. Native macOS uninstall acceptance could not run on Linux and was explicitly
  unnecessary for this plan. Browser tests, test-sleep checking, a full workspace battery and hosted CI add no relevant
  coverage for these asset declarations and shell changes.

## The review gate's outcome

A fresh gpt-6.1-sol reader at high effort found no correctness, design or idiomaticity findings after receiving the full
acceptance criteria, triage decisions and test-authoring checklist. Its source review was accepted after checking its
coverage against the diff and retained evidence; desktop parity subsequently passed. A separate fresh native reader
passed the commit and PR wording cold read; the required native resume check passed too. These are requested model
selections; exact model attribution and usage were not exposed by the harness. Private galaxy-brain evidence is retained
under session UUID `d0e361cc-2cf7-49dd-a656-7f0f6a284fbf`, with its location recorded in the plan working log beside the
checkouts.

### Landing

Landed on 2026-10-11 (UTC) as #1906. The rebase met no conflicts.

#### Review before merging

No blocker or should-fix findings. The installer keeps curl's real exit status for the latest-version answer, so a cut
answer after a 200 now refuses instead of installing the wrong release; the destination check covers all six files an
update replaces, runs before anything in the app changes, refuses a directory or a link to one, and removes nothing
outside the installer's own staging and lock. Both scripts pass `sh -n`, `dash -n`, `busybox sh -n` and `shellcheck`,
and the new installer cases fail without the fix. The license texts are complete and declared alongside the existing
web-links notice. Notes left as they are:

- Possible follow-up worth a look: the bundled font is a Nerd Font, which patches in icon sets under other licenses (for
  example Material Design Icons under Apache-2.0 and Codicons under CC-BY-4.0). Nothing ships those notices; the review
  did not establish whether they are required.
- Only one of the six destination files has a directory test; dropping another from the check would go unnoticed.
- An answer sent with no length over a connection the server closes can still be cut short with curl exiting 0;
  requiring the trailing newline the specification allows would need a specification change.
- A comment beside the clipboard asset still points at the font's license file as its model.

#### Checks

- Run now: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, `sh -n scripts/install.sh`, `shellcheck` on
  both installer scripts, the installer tests (549 checks), `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop` and
  `cargo check -p farhelm-desktop`; the UI and CLI unit tests through the recorder (run `c85c0865`, 622 of 622); the
  release web UI build; and `scripts/check-desktop-assets.sh` (26 assets, requested set equals bundled set).
- Not run: browser tests, since nothing the page renders changed (the stylesheet edit is a comment), and the macOS
  uninstall acceptance, which runs on macOS only.
- Between those runs and the merge, main changed only a development tool.

Nothing in the report above was made untrue by the landing.
