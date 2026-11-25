## What this was about

The November 24 triage settled 40 findings about how the terminal displays output, handles clicks and selection, and
recovers when a program or its terminal host exits. The seven PRs address 29 findings. Nine were closed under the
recorded polish and test limits, and two related lifecycle findings share one follow-up. The PRs are drafts at delivery.
The reviewed pictures are already in the golden reference store for the landing checks.

A printed link now opens only if the text at the press position still points where the last drawn frame showed it did.
Page keys move through the rows actually visible. The scroll thumb reflects how much history fits, and clicks on the
exit line leave the prior selection alone. Selecting a wrapped wide character no longer highlights the unused padding
cell. Editing an isolated wide-character spacer preserves its background without extending the glyph's link or
decoration into the blank.

The Keyboard shortcuts chord stays with a composing terminal if its composition mark still exists when Farhelm handles
the chord. The Send feedback dialog's Discord link announces that it opens in a browser.

## Things you should know

- If a terminal host dies and its program survives, the app can present that session as ended. Confirming Stop or Delete
  on that understanding can kill work that is still running. Without a systemd scope, if Farhelm also cannot read the
  session marker, Stop can report success while the old program keeps running, or Restart can start a second program in
  the same folder. The triage decision was to use the retained process identity to check for a survivor and honor the
  user's confirmation. A supervisor-only patch left the desktop sending the same ended-only consent, let stale activity
  block recovery Restart, and let the background observer record Stop too early. The two fixes now share the Plan
  follow-ups entry **Handle programs that outlive their terminal host**. That follow-up covers both the confirmation
  shown in the app and the current evidence the supervisor needs to stop or restart the right program. Landing can
  proceed with this follow-up recorded.
- The shortcut preserves an in-progress input-method composition when Farhelm still sees it in the focused terminal. On
  Linux, native input handling can clear that mark before Farhelm handles the shortcut. The shortcuts panel may also
  still open during composition in other text fields. The narrowed `shortcuts-chord-during-composition.md` feedback
  records both cases. Source inspection established the Linux ordering. The runtime test exercises a terminal whose
  composition mark is still present.
- Erasing an isolated half of a wide character keeps the blank cell's background. Deleting adjoining halves of two wide
  characters can still leave a spacer from one character beside the other. Those cells look the same to the renderer as
  a valid pair. The fix leaves that ambiguous case alone to preserve legitimate emoji widening.
- A click on a newly updated printed link may do nothing until the new frame is painted. This is the cost of opening
  only the destination the user saw. The terminal interaction PR is just over the approximate 300-line target with the
  fixture and reference changes. The stale-link check needs real click sequences to show both the suppression and the
  next successful click. These tests share the existing displayed-link state.
- The lifecycle and Stop replay findings touch the supervisor triage area. The borrower exit test shares a fixture with
  environment triage. This plan checks the exit result. The separate finding about checkout membership remains in that
  plan. Session UI changed how inactive animations are tracked. The rebase check verifies that the cursor starts
  blinking again when Reduce Motion is turned off. The Antigravity Settings and hook diagnostics findings were reviewed
  under this plan's polish budget and closed without edits to those surfaces.
- The stack was rebased on `0db556ae30947` after refreshing the planning instructions. Landed launcher and session UI
  work changed adjacent window layers, upload notices and animation. The test tooling landing changed where selected
  screenshot updates are stored. The rebase preserved each draft's source and specification changes, preserved main's
  archived decisions and golden references, and carried forward the four approved terminal pictures.
- The polish decision was to fix cases that fit a small patch and test. A narrow terminal can clip the warning that some
  earlier output could not be recovered. Showing the whole warning needs a taller, wrapped history row that still
  scrolls and selects correctly. If a stalled replay shows an exited program's final screen before its history arrives,
  the late history can move the viewer into old rows and change which text an existing selection covers. Preserving both
  would need new viewport and selection handling. That latter condition is based on source inspection. Each closed
  record gives its own reason.

## Open questions and possible follow-ups

The recorded lifecycle follow-up is the outstanding change with the largest impact. Composing in other text fields and
Linux's earlier composition handling stay in the narrowed feedback item. The viewport jump after a stalled replay and
the ambiguous adjacent wide-character edit would need their own focused fixes if revisited.

## PRs

- Test which terminal link spellings show a warning. [#4831](https://github.com/scode/farhelm/pull/4831/changes)
- Reject stale link clicks and correct terminal selection, paging and thumb size.
  [#4837](https://github.com/scode/farhelm/pull/4837/changes)
- Record the terminal-host lifecycle follow-up and close two test findings without code changes.
  [#4845](https://github.com/scode/farhelm/pull/4845/changes)
- Require the first paste fragment's written receipt before testing a mid-paste disconnect.
  [#4848](https://github.com/scode/farhelm/pull/4848/changes)
- Suppress the Keyboard shortcuts chord while terminal text is marked.
  [#4851](https://github.com/scode/farhelm/pull/4851/changes)
- Preserve isolated spacer backgrounds, improve exit tests and announce the browser destination.
  [#4855](https://github.com/scode/farhelm/pull/4855/changes)
- Check terminal input, replay, cursor blink resumption and equivalent link spellings.
  [#4862](https://github.com/scode/farhelm/pull/4862/changes)

### Triage records

| Outcome                                                                          | PR    | Disposition                  |
| -------------------------------------------------------------------------------- | ----- | ---------------------------- |
| `terminal-link-oracle-regeneration-loses-wide-padding.md`                        | #4831 | Fixed                        |
| `trailing-final-frame-boundary-test-leaves-osc-8-and-file.md`                    | #4831 | No change                    |
| `trailing-link-disclosure-module-doc-soft-seam.md`                               | #4831 | Fixed                        |
| `trailing-link-hidden-row-test-step-cannot-fail.md`                              | #4831 | Fixed                        |
| `trailing-link-joined-second-reading-untested.md`                                | #4831 | Fixed                        |
| `trailing-link-label-budget-untested.md`                                         | #4831 | Fixed                        |
| `trailing-link-oracle-exception-asserts-only-absence.md`                         | #4831 | Fixed                        |
| `trailing-link-scanner-disguised-scheme-reason-unpinned.md`                      | #4831 | Fixed                        |
| `trailing-link-scanner-host-start-test-docstring-wrong.md`                       | #4831 | Fixed                        |
| `trailing-link-scanner-zwsp-scheme-test-vacuous.md`                              | #4831 | Fixed                        |
| `terminal-selection-paints-wide-wrap-padding.md`                                 | #4837 | Fixed                        |
| `trailing-lifted-viewport-makes-scrollbar-overstate-visible-share.md`            | #4837 | Fixed                        |
| `trailing-link-disclosure-warning-wording-untested.md`                           | #4837 | No change                    |
| `trailing-multi-clicking-exit-message-copies-different-row.md`                   | #4837 | Fixed                        |
| `trailing-page-keys-skip-rows-in-lifted-terminal.md`                             | #4837 | Fixed                        |
| `trailing-post-exit-alt-release-clears-short-selection-and-queues.md`            | #4837 | Fixed                        |
| `trailing-printed-terminal-link-stale-frame.md`                                  | #4837 | Fixed                        |
| `trailing-recovery-warning-is-cut-off-in-supported-narrow-terminal.md`           | #4837 | No change                    |
| `trailing-gone-terminal-ended-confirmation.md`                                   | #4845 | Deferred to Plan follow-ups  |
| `trailing-lifecycle-loses-held-child-after-terminal-loss.md`                     | #4845 | Deferred to Plan follow-ups  |
| `trailing-live-route-conformance-pass-when-terminal-control-skips-target.md`     | #4845 | No change                    |
| `trailing-stop-classification-test-pass-without-stop-waiting-for-exit.md`        | #4845 | No change                    |
| `terminal-paste-test-misses-first-ack-premise.md`                                | #4848 | Fixed                        |
| `trailing-shortcuts-chord-opens-during-terminal-ime.md`                          | #4851 | Fixed within the scope above |
| `antigravity-trust-link-can-hide-read-failure.md`                                | #4855 | No change                    |
| `feedback-external-link-cue-is-visual-only.md`                                   | #4855 | Fixed                        |
| `trailing-deleting-wide-spacer-swallows-shifted-space.md`                        | #4855 | No change                    |
| `trailing-left-half-wide-erase-drops-orphaned-background.md`                     | #4855 | Fixed within the scope above |
| `trailing-no-existing-assertion-covers-terminal-loss-between-publication-and.md` | #4855 | Fixed                        |
| `trailing-omp-owner-exit-test-overstates-evidence-behind-resume-offer.md`        | #4855 | Fixed                        |
| `trailing-ready-and-borrower-cases-do-not-assert-vanished-terminal.md`           | #4855 | Fixed                        |
| `trailing-right-half-wide-erase-keeps-inverse-decoration-and-link.md`            | #4855 | Fixed                        |
| `trailing-shifted-wide-wrap-spacer-drops-background.md`                          | #4855 | Fixed                        |
| `hook-diagnostic-path-can-render-doc-link.md`                                    | #4862 | No change                    |
| `shift-enter-test-assumes-one-program-read.md`                                   | #4862 | Fixed                        |
| `terminal-prehello-test-misses-output-loss.md`                                   | #4862 | Fixed                        |
| `trailing-alternate-screen-stop-diagnostics-overstate-replay-oracle.md`          | #4862 | Fixed                        |
| `trailing-late-replay-history-displace-already-browsed-final-screen.md`          | #4862 | No change                    |
| `trailing-link-scanner-comparison-leeways-untested.md`                           | #4862 | Fixed                        |
| `trailing-ticker-live-motion-resume-test-misses-blink-resume.md`                 | #4862 | Fixed                        |

## Checks

Runtime selections used `python3 scripts/record-test-run.py`. The test command was `cargo nextest run --locked` with the
package and target narrowed to the named cases below. The rebase used `-p farhelm-gui --lib` for the GUI tests and
`python3 scripts/check-gui-goldens.py --scene terminal-status --scene terminal-wide-glyphs` for the references. Before
each reference publication the renderer ran with `--update` in its default four-process configuration. The static checks
were `cargo clippy --tests -- -D warnings` for the GUI, package-scoped `cargo fmt -- --check`, `dprint check`,
`python -B scripts/check-test-sleeps.py` and `python3 scripts/check-agents-md.py`.

- The rebased tip `7370f502846d` passed all 45 selected GUI cases, including terminal clicks, composition, upload
  notices and Reduce Motion (run `ef6ab35c-0ecd-4d6e-90ca-c06e2866af89`). The terminal-status and terminal-wide-glyphs
  comparisons matched the four published light and dark references (run `2e093645-a149-44c9-99f5-2fb4ea1a4bb1`). GUI
  test Clippy, Rust and Markdown formatting, and the test-delay audit passed. The instruction-link check reported one
  inherited archive warning and no errors.
- Before the rebase, the link tests passed 72 cases on `45506350` (run `7bb001f3-8f13-4e57-8768-6d61db886386`). Two
  exact tests passed on `6a54cf35` (`c025ef95-d12b-4cb8-bca4-9ededfa5987d`) after strengthening the test inputs.
- The terminal interaction selection passed 14 cases on `3b86ec38` (`9947d0d3-0b0f-456b-b0f0-6bd958d52901`). The full
  four-process render (`21fdb1da-bbdf-4395-a5b8-5b0d35e32455`) matched the four approved changed pictures.
- Two adjacent paste tests passed on `717ba7a1` (`b8461b2b-cdf0-4b24-9b75-77a824ea58f6`), and the exact revised
  regression passed on `94c754c1` (`6248b11e-00e7-4552-947a-358a355700fc`). Nine unchanged keyboard cases passed on
  `928e741b` (`2b171411-a28c-459e-8762-259931793695`), and the strengthened composition case passed on `c8d649b5`
  (`b069fc90-be5d-47f6-9bbd-609bd0ee9031`).
- The spacer batch passed 35 tests on `ace64b17` (`17e790a6-7617-482d-9228-da2b52eae1f8`). Two exact supervisor tests
  passed on `ab4ce65d` (`a9726c52-07ee-4e75-ba2c-9866adaee2e3`), covering the strengthened exit and Restart reply
  assertions. The full four-process render (`fcdd8690-7b13-4f23-a486-d3101e87e6fc`) matched the approved wide-character
  pair.
- The final batch's first five tests passed without early returns on `a52349b5`
  (`08dee4fb-f063-4ef5-b311-fce45cd5eedf`). Four still apply after the blink test change, including the real terminal
  host socket replay, PTY input and Stop replay. Both exact cursor cases then passed on `800129ce`
  (`07684308-12f3-475d-a862-ea953da620f2`). The package Clippy checks, formatting and annotated test delay checks passed
  on the corresponding drafts.
- Reused the pre-rebase link, host and supervisor evidence because their source and assertions are unchanged and the
  landed changes do not alter their dependencies or fixtures. The discarded lifecycle experiment and failed test
  revisions are excluded from pass evidence. The lifecycle PR changes only the follow-up and ledger. The targeted runs
  cover the changed paths. The full workspace and release suites were skipped. The existing docs screenshots and video
  show different terminal interactions and need no recapture for this work.

## Review

Each PR passed Sol source review before the rebase, and a cold read of its title and description. Each rebased draft
kept the reviewed source and specification changes. The conflict resolutions affected shared bookkeeping. Opus approved
the changed terminal pictures in both appearances. The complete captures were compared with the approved images before
updating the reference store. The work left over from the reviews is named above and in the archived decisions.
