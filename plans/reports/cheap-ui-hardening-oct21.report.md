# Three cheap GUI hardening fixes

## What this was about

The 2026-10-21 triage of the highest-priority review findings left three small GUI items for which you said: "fix
everything that is super cheap and no real downsides, discard the rest." The first two had a size limit of about 100
changed lines (code, tests and docs); the third had the condition that the fix be that small and change nothing about
which words count as version numbers. An item over its limit was to be discarded, not brought back to you for a
decision. All three were fixed.

- Desktop menus exposed a memory address. Every item in a menu (the session, host, app-bar and template number-key
  menus) carries a hidden marker in the page that keyboard focus uses to find it. The marker was built from an internal
  identifier whose text form includes a pointer into the desktop app's own memory, so any script running in the app's
  page could read a native address. Nothing showed it being read or misused; it is hardening.
- The changed-host warning could show two different install identities identically. When an automatic update stops
  following a host because a different Farhelm install now answers at its ssh destination, the warning names the
  recorded identity and the new one so the user can compare them. Both went through the general text escaper, which
  shows an invisible right-to-left override character and the literal text `<U+202E>` the same way. The warning drives
  no decision by itself (adopting the new install is a separate, exact step).
- Terminal output could stall file-link recognition. While finding file links, the terminal skips words that look like
  version numbers (`1.2.3`, `v1.5x`, `1.2.3-rc.1+build.5`). On a long near-miss, `1.1` followed by thousands of hyphens
  and then a character such as `@`, that check backtracked quadratically: 50 to 80 ms for one 8,192-character line in
  Chromium and WebKit, during which the window cannot respond. Nothing showed ordinary output looks like that.

## Things you should know

The menu fix came out at about 120 changed lines, a fifth over the nominal 100. About 35 of those are the code change
and its documentation, well inside the triage's estimate of 40 to 70; the other 85 or so are a unit test that mounts
real menus and checks the marker is digits only, different for two menus, the same across a rerender and new after a
remount. I kept it as a fix rather than discarding it, because the limit says "about" and the code itself is small. If
you meant the limit strictly, that fix should have been discarded; it can be reverted on its own, since the other two
fixes share no code with it. Each menu is now given a number once when it appears, and its markers carry that, so
keyboard focus that is still on its way to a menu item keeps working exactly as before (the browser tests for that
passed on both engines).

The changed-host warning now spells identities with the same strict formatter the adopt prompt already uses, so an
ordinary install identity reads exactly as before and an unusual one gets every hidden or non-ASCII character written
out. Destinations in the same warning still use the looser general escaper, so a destination with a hidden right-to-left
character can still look like one with that character's name typed out; the triage scoped the fix to identities. The
host's detail line also still shows the identity with the general escaper, which its code says is intended; an unusual
identity can therefore be spelled differently there than in the warning.

The version-number fix keeps exactly the set of words the old check accepted. The obvious fix (no hyphens inside the
dotted parts) would have turned `1.2.3-rc.1+build.5`, `1.0.0-beta+exp.sha.5114f85` and `1.2-` into file links, so the
hyphenated tail now takes one of three forms instead. I compared old and new on every string of up to eight characters
from digits, letters, `.`, `-`, `+` and `_`, plus four million random longer strings, and found no difference; the
reviewer ran its own exhaustive comparison over a different alphabet and reasoned through the two patterns, with the
same result. Measured in the same JavaScript engine the browsers' Chromium uses (V8, under node and Deno), the version
check alone on that line went from about 120 ms to 0.1 ms. The whole line gains less, though: the file-name check that
runs next has a near-miss cost of its own on the same line, which this fix did not cover. From the original finding's
browser measurements, such a line should now take about as long as a similar line that never reached the version check,
roughly half its old time; the changelog entry says "about half as long" for that reason.

## Open questions and possible follow-ups

Keep the menu fix although it came out at about 120 lines? I recommend keeping it: the change itself is about 35 lines
and the rest is its test. If you meant the limit strictly, it can be reverted on its own.

The file-name check in terminal link recognition (the check that decides whether a bare word like `report.pdf` looks
like a file) still costs about 100 ms, measured in node, on the same 8,192-character near-miss line. The original
finding mentioned it in passing, but it is not in the review queue as an item of its own, so nothing tracks it. I would
not prioritize it, for the same reason the triage put this one in the lowest bucket: nothing shows ordinary output looks
like that. If you want it fixed, it needs a new finding.

A separate review item, still waiting for triage, says the version-number check also hides some real artifact file names
from file links (a bare name such as `1.2rc1-linux.tar.gz` is treated as a version and gets no link). This plan left it
alone, and keeping the accepted set unchanged leaves that behavior exactly as it was.

## The PRs

- [#2569](https://github.com/scode/farhelm/pull/2569/changes): keep native addresses out of menu focus markers.
- [#2571](https://github.com/scode/farhelm/pull/2571/changes): spell install identities unambiguously in the
  changed-host warning; based on #2569.
- [#2575](https://github.com/scode/farhelm/pull/2575/changes): remove slow backtracking from the terminal version-number
  check; based on #2571.

## Checks run, reused and skipped

- #2569: recorded `cargo nextest run -p farhelm-ui --lib` passed 659 of 659, run `bd988eba-223a-4500-a291-936cc5a484e6`;
  after the review's test edits, recorded `cargo nextest run -p farhelm-ui --lib -E 'test(menu_panel::)'` passed 22, run
  `6c61079b-d3d1-4054-819f-c701f7fb7933`. Both used pinned nextest and tmux, four slots, zero retries. The recorded
  browser run of the two "menu in-flight focus respects" tests in `e2e/tests/sidebar.spec.ts` passed 16 on Chromium and
  WebKit, run `6a1ac5f5-dc56-4267-b69b-f97cafb3ef32`, against a debug build and the release web bundle of that code.
  `cargo check -p farhelm-ui --features desktop` passed.
- #2571: recorded `cargo nextest run -p farhelm-ui --lib -E 'test(provisioning::) | test(peer::)'` passed 38, run
  `e3535408-f12e-45e6-99ec-0e2d716d7743`, on the final code. The new test checks its premise (the general escaper alone
  conflates the two spellings) and the exact spellings.
- #2575: recorded `cd crates/farhelm-ui/js-tests && node --test` passed 329 of 329 on the final tip, run
  `7abacc53-9b40-4792-8f5a-12e67dcc546f`. The new test was also run, unrecorded, against a scratch copy with the old
  pattern restored, where it failed by timing out, so it does detect the regression.
- On the tip: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `dprint check` on
  every changed file, and `python3 releasing/check-changelog.py format` passed. The isolated test-sleep checker found
  314 delays and none missing a rationale; its first attempt stopped on its own time budget while the machine was at a
  load average of about 40, and the retry when it was quieter passed.
- Reused across the final rebase: main moved from `cc776b5c` to `8dc586c4` with review-queue and plan bookkeeping only
  (a batch of auto-triage records, a new plan and its TODO entry, other plans' state changes). I read those diffs; none
  touches the stack's code or its bookkeeping lines, and the rebase had no conflicts, so the runs above still apply.
- Skipped: the full Rust, browser, desktop, installer and provisioning suites. The changes are a per-menu identity
  number, a string formatter call in one warning, and one regular expression, each covered by the targeted runs above.
  CI was not dispatched; it does not run on PRs.

## The review gate's outcome

Each PR got one fresh-context Opus 5.5 review at high effort with the plan, the full triage entry and the test-authoring
checklist. None found a code bug. For #2569 it found a review-queue entry that was not fully removed, and a
triage-record note that had called 120 lines "within" the limit (both fixed; the note now says plainly that it is over),
plus test premise checks that were tightened. For #2571 it suggested exact-spelling assertions and changelog wording,
applied. For #2575 it confirmed the patterns accept the same strings and that no super-linear backtracking remains, and
caught a changelog claim that overstated the speedup and a test that might not have failed against the old pattern on a
fast machine; both were fixed (the test now makes 500 checks inside its five-second limit). Fresh cold readers checked
each commit message; the #2575 message was rewritten to stop crediting the whole line's cost to the version check.

### Landing

Landed as #2569, #2571 and #2575, merged bottom-up, with the content the report describes, rebased onto this round's
main with one code conflict resolution and three changelog changes described below. It was the second of five plans in
this landing round, after triage-specs-oct21; start-remedy-quoting, uninstall-mixed-spelling and
refused-retry-rollback-fence followed it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained popover-anchor-close, which closes menus when the row they are attached to moves and
changed the same UI files, plus documentation commits. A fresh-context reviewer and I read this plan against it:

- The menu focus markers this plan renumbers are not used by popover-anchor-close: it closes menus through each menu's
  existing close action, and returning focus after a close does not read the markers.
- The changed-host warning's new identity spelling is deterministic, so it does not make popover-anchor-close's
  row-movement detection close menus any more often than before.
- The other four plans in the round do not touch menus, the changed-host warning or the terminal's version-number check.

#### Fixes made during the landing

- The changed-host warning PR's import of the identity formatter conflicted with popover-anchor-close's additions to the
  same import line; the landing kept both.
- All three of the plan's changelog fragments described fixes to work that landed after the last release and has not
  shipped: the menu focus markers came from a menu focus fix (#2438), the changed-host warning that names both
  identities came from host-changed-update-warning (#2504), and the version-number check's slowdown came from a change
  that let the dotted parts of version numbers contain letters and hyphens, as part of recognizing file names such as
  `backup.7z` (#2396); the released version-number check had no such slowdown. All three landed on 2026-10-20 or later,
  after v0.30.0 (2026-10-18). The repository's rule is that a fix to unreleased work edits that work's fragment rather
  than adding a second entry, so the landing deleted the three new fragments and added their content as context
  paragraphs to the original fragments, in the same PRs. The report's "about half as long" changelog wording therefore
  no longer appears; the context paragraph instead says the file-name check's remaining cost on such lines was left
  alone, and that whether it also exists in the released version was not established.
- The review-queue index conflicted with entries main had changed; the landing rebuilt it the same way for each PR.

#### Checks

Together the five plans change about 650 lines of code, tests and specification text (more with review-queue
bookkeeping) across the helm's connection errors and uninstall, the supervisor's create path and store, and the UI,
including how every menu remembers its focused item and the terminal's file-link script; that reach is why this round
ran the whole Rust workspace and the full browser suite. All of the following ran on the five plans stacked together on
this round's main, including the landing's conflict resolutions. After the run, the landing moved changelog text between
fragment files in cheap-ui-hardening-oct21's last PR (see its notes); that changed only changelog files:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 scripts/check-agents-md.py` (one warning,
  about a citation main already had), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (which also builds the desktop app): all passed.
  `python3 releasing/check-changelog.py format` passed again after every changelog change.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  slots, no retries, run `461bfc4c-651d-4a70-8498-7a9b5ae524cf`: 4291 passed, none failed; 40 tests marked as ignored
  did not run.
- `cargo nextest run -p farhelm-ui --features desktop`, run `4073dcb5-a14f-42e5-bddd-8b007925da68`: 900 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `4086a6d6-96a5-4691-b05e-e50169546906`: 329 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `b5e88d8c-8a25-4e9b-be8f-ada4f9d48bc3`: 2475 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), 1 failed: on WebKit, "don't ask
  again on a refused YOLO replace marks the host, then replaces with the override", the open flake FLAKES.md recorded on
  2026-10-20, which the deflake sweep already excludes. It exercises a host's YOLO question during Replace, which none
  of the five plans touches; refused-retry-rollback-fence changes only the supervisor's handling of a stale create
  retry.
- An earlier start of these checks was stopped at its build step and discarded, because the landing changed the stack
  while it ran; nothing from it is counted.
- Reused: none. Skipped: the website build (no docs page changed), the doctests (no documentation example changed), the
  desktop smoke test (the desktop app crate is unchanged, and the menu marker fix is covered by the desktop-feature unit
  tests above, which mount real menus), the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were the UI unit tests and selected browser specs on its base; the round ran the full suites above.
The report's note that the menu fix came out at about 120 changed lines, over the nominal 100, still holds. Nothing in
the executor's report became untrue at landing, apart from the three changelog fragments moved as described above.
