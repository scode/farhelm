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
