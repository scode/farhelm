## What this was about

Emoji in agent status output were reported clipped in the macOS desktop app: scrolling repaired them until more output
arrived. The terminal's default width table counted the tested emoji as one column, while agents and tmux used two. That
disagreement also put cursor-addressed status updates in the wrong columns.

The terminal now selects the upstream grapheme width provider before opening or parsing any output, including reconnect
prefill. The six status emoji in the plan occupy two columns; the following text, in-place updates, coloured neighbour
boxes and reconnect cursor agree with those columns. The add-on is vendored unmodified with its bundled dependency
notices, and its browser script and notice are declared for both web and desktop assets. The specification records the
width contract, and the TODO entry is removed.

## Things you should know

The maintainer chose the experimental grapheme add-on because the stable Unicode 11 alternative leaves
variation-selector emoji one column wide. Compatibility was checked against xterm.js 6.0.0's own release manifest: it
pairs with `@xterm/addon-unicode-graphemes` 0.4.0. The published archive's registry integrity was verified before
copying its bundle; the source bundle remains byte-identical. The web build performs its normal JavaScript optimization.

The reported clipped pixels were not reproduced on macOS. As authorized during planning, this ships the demonstrated
width correction with buffer, DOM text and geometry gates, without claiming that the original macOS symptom has been
reproduced or eliminated in every font. WebKit was exercised as the desktop engine-family stand-in, not as the native
macOS application.

An owned test server using pinned tmux 3.7c returned `variation-selector-always-wide on` with an empty configuration. No
tmux setting or second PR was needed. The replay assertion checks a restored cursor after a variation-selector emoji and
a subsequent live reply through the replacement attachment. No live installation was used.

## Open questions and possible follow-ups

Glyphs wider than two cells may still spill into a neighbour; that upstream xterm limitation remains a possible
follow-up awaiting an upstream release. The DOM renderer and vendored terminal bundle remain unpatched.

## The PRs

[PR #2723](https://github.com/scode/farhelm/pull/2723/changes) — give terminal emoji the two-column width used by agents
and tmux, with the scroll regression fixture correction described below.

## Checks run, reused and skipped

The recorder ran `npx playwright test` with the `terminal-unicode`, `terminal-links`, `terminal-link-clicks`,
`terminal-files`, `terminal-clipboard`, `terminal-scroll-freeze` and `terminal-font` spec patterns on Chromium and
WebKit: run `1406a4ef-1ecc-49d6-ab16-9f23346b3f4f`, three workers with one owned stack each, zero retries, pinned tmux
3.7c. It completed with 130 passes, nine expected WebKit clipboard permission skips and one existing scroll-fixture
failure. All eight new Unicode executions passed. The report and output are complete, without omitted bytes or cleanup
limits; source identity is base `d5687fce` plus the working diff, complete fingerprint
`67f7f920b345b258a78f2d0b6a2e1f3894c956e267c787e1327c5c49d7d04ae0`. Clipboard and copy-on-select ran in Chromium; the
WebKit skips do not establish clipboard coverage there.

The scroll failure was a test setup defect. The trace shows the first wheel already put the viewport 39 lines behind the
live tail, with rendered text matching the buffer. The helper nevertheless demanded a position twenty lines before an
old absolute coordinate, which live output had overtaken before the wheel took effect. The original exact-case rerun
`13270595-cac1-4772-a3d4-59557da77f34` reproduced the Chromium failure and passed WebKit. The helper now requires an
initial live-tail snapshot, a real wheel gesture and enough separation from the current tail, retaining the deadline and
display-consistency assertions. This existing fixture defect is recorded as fixed in FLAKES.md. Scheduling pressure as
the reason for the first-wheel delay remains unproven; no product scrolling change was made.

The corrected helper's three callers passed on both engines in run `16467ed8-2c41-488c-8250-cad52b0f4960`: six
executions, zero failures or skips, with two active workers under the same three-slot/zero-retry policy and pinned tmux.
This exact selection was
`npx playwright test 'terminal-scroll-freeze\.spec\.ts' -g 'a plain flood, scrolled back|a plain flood survives a sibling band'`.
The source is base `ac20773b` plus the working diff, complete fingerprint
`1d147589d2a84a9881c7f99ffa7b2919d555c93dc25636ee3911da00c2039930`; report and output are complete, without omitted
bytes or cleanup limits. This adds corrected-case evidence without erasing either retained original failure.

The initial new-spec run `4346f781-9325-4780-a46b-76ec868ea3f2` passed the six width, redraw and geometry executions and
failed both reconnect comparisons because tmux replay serialized trailing blank padding. The new test helper now
normalizes trailing padding in both buffer and DOM snapshots; interior text, the cursor column and the live next-cell
assertion remain strict. The exact reconnect rerun `e9cef30b-0dda-459c-9c51-8b3e848cfec0` passed both engines. These
same-session test-development failures are retained privately and are not treated as latent flakes.

A deliberate negative control, run `c60a7daf-0ba3-4222-a24b-cbf7b8f7d0e2`, selected the old Unicode 6 provider before
the new width assertion. Both engines failed exactly because the six emoji occupied one column and the next character
was in column one. This was the current build with the old width model selected, not a separate unmodified-main build.
The temporary selection was removed and the final spec restored byte-for-byte before the final regression run. The
negative control has complete source identity `96eb8155bac474091fef83d39f49a42851e8278a61179c68b117cd2211c28771`,
complete report and output, no omitted bytes, and clean cleanup.

The recorded UI JavaScript harness (`node --test` in the UI's JS test directory, recorder run
`01614c19-f872-442f-8c38-ba3c2323dd0d`, tmux none) passed 333 tests with no failures or skips. Its source identity is
base `d5687fce` plus the working diff, complete fingerprint
`9f7e303f27cda8239c9e9dc0fa3479ae1773764a722cc08532b6d7a8957ab125`. Later changes only corrected the browser test helper
and bookkeeping, so the JavaScript result remains applicable.

`cargo build`, the UI's `dx build --package farhelm-ui --platform web --release`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
`dprint check SPEC_impl.md TODO.md` and `python3 releasing/check-changelog.py format` passed. The isolated
`python -B scripts/check-test-sleeps.py` passed with 323 delays inspected and zero missing rationales under Python 3.12.
Two earlier attempts under Python 3.15 exhausted its 60-second source-check budget and are incomplete observations, not
passes. The add-on source compared equal to the verified package, and the built web assets contain its optimized script
and shipped notice.

The stack was carefully rebased onto `ac20773b`. Intervening main changes were unrelated TODO and planning bookkeeping,
plus a supervisor test-fixture isolation fix and its flake record. Both appended flake records were retained; the TODO
patch still removes only the emoji entry. The product and new Unicode-test patches stayed identical. The older broad
browser run's successful cases and JS, native build, UI lint and format evidence remain applicable; the corrected scroll
helper has its own focused run above. The web build was refreshed after removing an extra blank line from the notice,
and its shipped notice again compared equal to the source. No runtime rerun is needed for that whitespace-only notice
change or for upstream's supervisor-test-only edit.

The full Rust, desktop smoke, installer, provisioning, website and unrelated browser suites were skipped: the product
change is terminal asset wiring and width-provider selection, covered by the UI compile/lint, built browser assets and
the named terminal regressions. The desktop asset comparison script derives the new declarations automatically; a
separate desktop rebuild would add no evidence about the demonstrated width mismatch. Native macOS pixel reproduction
remains the explicit gap above. No hosted CI run was requested.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no actionable findings, including on the
trailing replay-padding correction. It reviewed the full plan and changed tests with the complete test-authoring
checklist. No findings were rejected and no scope departure required reassessment. A separate cold reader understood the
commit and PR wording and found no contradicted claims or convention violations.
