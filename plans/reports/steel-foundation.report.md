## Since the last review

The landing attempt merged none of the five PRs. This round keeps the same stack: #2728 and #2729 are unchanged apart
from rebasing; #2730 corrects the selected local-host specification; #2731 restores the terminal background and fixes
formatting and the visible-wordmark test; #2732 renames the permission choice group and corrects the affected tests. No
PR was added or dropped.

The terminal again keeps its existing dark gray background, #282c34, in both palettes. The persistent permissions
choices are announced as permission mode, while their visible label and the Permissions search section stay unchanged.
Selected local and remote host marks share the inverse selection color, as requested. The specification also includes
the effort dropdown in the existing Enter-to-launch contract.

The requested full browser run completed: 2679 passed, three failed and 14 skipped. All 36 failures from the landing
attempt passed. The three additional WebKit failures were pre-existing test fixtures; this round tightened their
readiness using existing helpers and verified the affected cases on both engines. The full command remains failed; the
focused passes below do not turn it into a green full run.

## What this was about

This is the shared foundation for Steel: blue-gray dark colors, a light counterpart, solid periwinkle selections, Inter
for interface text, and common styling for buttons, switches, menus and dialogs. Paths, commands, versions, ages and
terminal text keep JetBrains Mono. Settings gains a per-device System/Dark/Light appearance choice. The terminal keeps
its own theme. The later Steel plans still own the host rail, session toolbar and tunnels, later dialog layouts, and
sentence case. The reference mockups and the shared TODO entry remain unchanged.

## Things you should know

The terminal regression came from stylesheet priority: the terminal library's later black viewport rule won over
Farhelm's theme. The corrected selector makes Farhelm's existing terminal token win regardless of insertion order. No
terminal palette or renderer behavior was changed.

The other landing failures required accessibility, expectation or fixture corrections. The following accounts for all 36
failed engine cases from the landing run. The counts are failures in that run, not a claim that later passes erase them.

| Cases | Behavior                                                           | What was corrected                                                                                                                                                                         |
| ----- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 4     | Terminal theme and host-colored frame, both engines                | Product CSS priority, restoring #282c34.                                                                                                                                                   |
| 2     | Launcher word search, both engines                                 | Requested accessible group rename; tests distinguish permission mode from Permissions search results.                                                                                      |
| 6     | Custom permissions, Goose/Pi, Replace with, both engines           | Tests use the intended native effort dropdown and preserve reconciliation/admission checks. Goose/Pi also establishes dropdown focus before reopening the model picker.                    |
| 2     | Guarded-permission help alignment, both engines                    | Tests compare centers of the padded segment well and help control with the existing tolerance.                                                                                             |
| 2     | Template typography, both engines                                  | Shortcut digits are technical values in JetBrains Mono; prose remains Inter. Loaded-font and equal-width checks remain.                                                                    |
| 2     | Drag-copy notice placement, both engines                           | The test dispatches and returns the same integer mouse-release coordinates, retaining the anchoring tolerances.                                                                            |
| 3     | Session-header popover geometry, one Chromium and two WebKit cases | The fixture waits for menu fonts before measuring settled geometry; movement and wrapping assertions remain.                                                                               |
| 6     | Phone setup, Remove host and uninstall questions, both engines     | Tests follow the intended Cancel-first order and retain exact consequence text, containment and action checks.                                                                             |
| 2     | Very long session-title confirmation, both engines                 | At wide width, tests require Cancel left of confirm on one line, the title above both, and all four edges contained.                                                                       |
| 2     | Terminal link-click setting, both engines                          | The test clicks the visible native-radio label, then checks the actual checked state and terminal behavior.                                                                                |
| 2     | Title-band wordmark, both engines                                  | The test requires one visible palette variant before measuring; drag exclusions remain.                                                                                                    |
| 1     | Host removal and rediscovery, WebKit                               | The exact live-status oracle now admits the existing idle — new output state; identity and removal checks remain.                                                                          |
| 1     | Template shortcut preserving editor focus, WebKit                  | The fixture waits for the second question's initial Cancel focus before editing. The later shortcut must still preserve the name field's focus and selection.                              |
| 1     | Two delayed host plans behind Settings, Chromium                   | The existing event-feed stub excludes unrelated retry-detail updates from fake hosts. Both planning replies remain held until Settings owns focus; serial modal admission remains checked. |

The last two failures were reproduced during focused follow-up. The focus failure occurred before the shortcut ran. The
host menu opened and then closed as an earlier fake host's retry details changed, consistent with the existing
movement-dismissal contract. These were isolated using existing test facilities, with no timeout increase, retries or
weaker menu readiness check.

The full run also found three setup or measurement boundaries in older tests. Offer retirement now gives its existing
event-feed stub an initial healthy greeting, preventing unrelated fake-host retry details from moving its menu; both
held offers, the later retarget notification and owner/focus/release checks remain. The long-path sidebar test waits for
fonts before measuring its settled tail, keeping the strict less-than-1px threshold and all text/bidi/overflow checks.
The automatic-setup test waits for its initial probe response before checking the rendered error, then retains the
single-probe count and the busy-operation forced-click guard. Their full-run failures and focused outcomes are recorded
in FLAKES.md with hypothesis-level causes; the later passes do not prove those causes.

## Open questions and possible follow-ups

The visual gate is representative ordinary appshot inspection. It makes no transient-paint, native macOS or physical
input claim. Active host operations, pending Trash, loaded SSH help, the installed updater and successful uninstall
planning retain the preceding report's code-only visual coverage. No live installation was changed.

An earlier artificial native sequence opened Settings, changed appearance rapidly, then immediately closed and reopened
it. It showed a Light Settings card over a Dark page and stale section content briefly. Ordinary input has not
reproduced this, and neither its cause nor a product fix is established. This round added no native timing diagnostics.

Opus observed the existing A+ toolbar clipping at the 650px main pane; the later session-toolbar plan owns that layout.
Mixed sentence case remains with the final wording plan. Neither expands this follow-up.

## The PRs

- [#2728: unpublished visual-review appshots](https://github.com/scode/farhelm/pull/2728/changes).
- [#2729: Inter interface typography](https://github.com/scode/farhelm/pull/2729/changes).
- [#2730: Steel dark palette and solid selection](https://github.com/scode/farhelm/pull/2730/changes).
- [#2731: device appearance choice and Steel Light](https://github.com/scode/farhelm/pull/2731/changes).
- [#2732: shared controls, menus and dialogs](https://github.com/scode/farhelm/pull/2732/changes).

All five remain drafts in one linear stack, updated in this round. Pushed heads, in the same order: `5d261d0e65a6`,
`c26d0d9b3ae0`, `8211af23aa64`, `f51d73fc7c93`, `2c35e63a7054`. None was marked ready or merged by the executor.

## Checks run, reused and skipped

Current static checks passed: workspace Rust formatting, dprint on the changed Cargo/spec files, and the test-delay
checker (322 inspected, zero missing reasons). Current CSS checks passed 53/53 in 4e4e2f86-76a6-4f50-85e7-b5f4ae5c59c0.
The corrected web build passed in e2a41109-01cd-4cf2-803c-71840625b13f. An earlier build selected an unrelated
executable named dx and failed immediately; run 38857758-0b6a-4bed-8879-b2b471e44bcf remains retained as a
tool-selection failure.

Focused browser validation used both Chromium and WebKit, three workers, one owned stack each, pinned tmux 3.7c and zero
retries. Run 8fac9c3a-2c85-462d-a9b6-33e76e2a4f17 produced 38 passes and six failures. Its four deterministic
test-interaction/layout failures passed after correction in d4fec7c3-ec90-40a8-9e63-6203f80c8468 (4/4). The two
remaining setup-boundary failures and their related held-dialog modes passed in 4e8af30a-c780-44bc-8e1a-ee0ccac48220
(8/8). Failed attempts remain failures and are retained.

Ordinary appshots used the existing docs-fleet capture. Run a0c822bd-4cc1-4777-8bc8-3fdf7ecd32a3 saved 148 valid images
before all four layout variants reached the same old permission-group selector and failed. Its eight earlier
main-window/terminal-settings images supplied the current visual review. After the one-line selector correction,
e4acb3b3-e5f9-4c34-a950-3dbe50ddf5c2 passed staging and the entire 46-surface dark-wide sequence. Other variants' saved
images retain their actual scope; the failed command is not described as a passing matrix.

Full run c909ec87-c8e1-4ae1-b0bb-e849195af2c9 used `npx playwright test` on both engines with the same three-worker,
zero-retry policy. It completed in about 131 minutes, exit 1. Console and raw JSON agree on 2679 passed, three failed,
14 skipped and zero flaky outcomes: Chromium 1346 passed and two skipped; WebKit 1333 passed, three failed and 12
skipped. No global errors were reported. Automatic recorder report validation is incomplete because the 18.7 MB raw JSON
exceeds its 16 MiB inspection cap. The complete console, source identity, raw report and failure artifacts remain
retained; the counts above were read directly from the existing report and checked against the console. They are not
recorder-validated counts or evidence of a full-command pass. Skips cover four opt-in real-vendor-agent cases, nine
WebKit clipboard cases blocked by Playwright permissions, and the WebKit copy of the deliberately Chromium-only visual
capture matrix. No missing SSH or systemd substrate was reported.

After the feed and font-readiness corrections, 92271d2f-6ba5-47e5-a622-b9a30684d6f8 passed all eight selected cases:
both offer-admission variants, the long-path tail and automatic setup, each on both engines. The automatic-setup case
passed unchanged in that run. After adding its explicit initial-probe response boundary,
2fa75d14-9eb8-4c63-a070-0ac26c415d70 passed the automatic-setup and long-path cases on both engines (4/4). Both focused
runs have complete recorder reports, no skips, full output EOF and completed cleanup. No product code changed after the
full run, so another two-hour suite would add no useful coverage for these three local fixture corrections.

| Run prefix | Recorded HEAD / complete source fingerprint | Child command or exact selection                                                                                                                                                                                               |
| ---------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 4e4e2f86   | 121fca4d450c / 0159460f8aad                 | node --test app-css-contrast.test.js app-css-tokens.test.js app-css-animations.test.js                                                                                                                                         |
| e2a41109   | 121fca4d450c / 0159460f8aad                 | dx build --package farhelm-ui --platform web --release                                                                                                                                                                         |
| 8fac9c3a   | 121fca4d450c / 0159460f8aad                 | Playwright selection covering every prior failure and both-engine counterparts; 44 cases                                                                                                                                       |
| d4fec7c3   | 121fca4d450c / b0672b6d82b5                 | Goose/Pi permission transitions and very-long-title confirmation, both engines                                                                                                                                                 |
| 4e8af30a   | 121fca4d450c / 45589fa2617e                 | All delayed-host-dialog modes and template-shortcut editor-focus case, both engines                                                                                                                                            |
| a0c822bd   | 121fca4d450c / b0672b6d82b5                 | scripts/docs-screenshots.sh --only visual-review --no-build                                                                                                                                                                    |
| e4acb3b3   | 121fca4d450c / 45589fa2617e                 | npx playwright test -c docs-shots.config.ts docs-shots/visual-review.spec.ts -g 'dark wide' --reporter=line                                                                                                                    |
| c909ec87   | 563407f9ac94 / 129ffeca0c4b                 | npx playwright test; full suite, both engines                                                                                                                                                                                  |
| 92271d2f   | 563407f9ac94 / 896092835980                 | Playwright: offer retirement releases only its owner (both variants), a long cwd shows its tail with characters in logical order, automatic setup refuses forced activation while another host operation is busy; both engines |
| 2fa75d14   | 563407f9ac94 / 2f66e60e8bad                 | Playwright: the same long-cwd and automatic-setup cases; both engines                                                                                                                                                          |

HEADs and fingerprints in the tables are prefixes of the complete identities retained by the recorder. Dirty-source
fingerprints identify the actual tested files. Folding the corrections into their original PRs preserved the tested
files byte for byte. The built web product remained unchanged after its successful build; later edits affected only
tests, capture selectors and the flake ledger. No extra rebuild or visual capture was needed.

Earlier Rust evidence retains its actual baseline scope: UI library run 85b323ce-5cd5-4442-a7c5-af3377b0dded passed 783
tests at the Inter step, and typography run 572f1230-80d1-4622-90bf-3164de9a84d1 passed 29 selected tests after the font
corrections. This is not a claim that all UI library tests were rerun at the final tip. Asset JavaScript
3570be6e-81dd-4a26-8da5-1fdbf4f18eaf passed 358 tests; its executable asset sources are unchanged by this follow-up,
while the changed CSS has the current 53-case run above.

Ordinary native appearance c772ca7a-6aec-4dff-89ac-9bbeb2e024fe retains startup, persistence, font, embedded-asset and
same-terminal input/output evidence. It does not prove this round's terminal CSS correction; the current browser
assertions and ordinary appshots cover that.

| Reused run prefix | Recorded HEAD / complete source fingerprint | Child command                                                                              |
| ----------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 85b323ce          | 6d83cb62cc2e / 0d9960f1c5e3                 | cargo nextest run -p farhelm-ui --lib                                                      |
| 572f1230          | 6d83cb62cc2e / 2dde68f8fe78                 | Same command with -E 'test(/list::templates::tests::\|hosts::tests::destination_details/)' |
| 3570be6e          | 4a9f5fb5ee2f / 13258aefa2da                 | node --test in the UI asset harness                                                        |
| c772ca7a          | 19e9eadd86ae / ccc8ad5d96b0                 | DESKTOP_SMOKE_APPEARANCE=1 scripts/desktop-smoke.sh                                        |

No new full Rust battery, native smoke, release, installer, deployment or live-service restart was justified by this
CSS/accessibility/test follow-up.

## Review gate's outcome

The same required gpt-6.1-sol/high reviewer cleared every corrected unit of #2730, #2731 and #2732, including the final
focus handoffs, wide confirmation geometry and deliberately held modal boundary. The final review has no findings.
Earlier review conclusions that missed two test assumptions were explicitly superseded.

The same reviewer also cleared the final three fixture-readiness corrections and the accompanying flake records,
confirming that the strict geometry threshold, held replies, focus ownership, forced-click guard and cleanup remain. It
found no additional product integration risk requiring another full suite. This was a static assessment, separate from
the reported runtime results.

The required fresh Opus 5.5/high reviewer inspected eight ordinary dark/light main-window and terminal-settings images
at wide and 650px widths. Its verdict is a representative visual pass, with no findings against this round. It checked
host-colored frame styling from CSS and the cited browser assertions, not a frame-enabled image. The other four PRs'
preceding representative visual gates retain their scope; there is no claim that every screenshot or uncaptured state
was inspected anew.
