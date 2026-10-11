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
