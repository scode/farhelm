# What this was about

Clicking between fields in the New Session launcher could move keyboard focus to the dialog itself. The next Enter then
did nothing, and a second click could make the whole dialog look selected. The maintainer asked for the current control
to keep focus after a gap click, and for Enter on a focused dialog itself to use its main action. The same behavior was
requested for Restart with and other dialogs.

# Things you should know

The focus rule now covers the launcher, Restart with, dialogs that share the host/settings installer, and the row and
header confirmation panels. Empty-space clicks leave the focused control in place. Text remains selectable, so clicking
text in a confirmation panel can still move focus from its button. Enter on a focused dialog container uses the
launcher’s normal submission or that dialog’s visible main button, including destructive confirmations. It still
observes validation and disabled buttons. Dialogs without one main action, such as Settings, do nothing on container
Enter.

The whole-dialog look was the container’s focus ring and gap double-click text selection. Modal containers no longer
show that ring; controls keep theirs. The notification bell’s nonmodal panel keeps its container ring and is outside the
gap rule. The change keeps the existing focus traps and modal isolation. The launcher TODO entry was removed and a fixed
changelog fragment was added.

The selected browser run exposed two existing Settings tests whose complete-switch count and layout premises had fallen
behind the current 16-switch page. Those premises were corrected in this PR so their original layout and isolation
assertions run again. Earlier failed run records were retained; the narrow reproduction showed the same stale count
before the correction. This was test drift, not a focus regression.

# Open questions and possible follow-ups

None required for this plan. The desktop app’s own webview was not exercised; WebKit browser coverage stands in for its
engine family.

# The PRs

- [#2267 — fix: keep dialog focus through gap clicks](https://github.com/scode/farhelm/pull/2267/changes) (draft)

# Checks run, reused and skipped

- Passed: `cargo check -p farhelm-ui --features desktop`, `cargo build`, pinned Dioxus web release build,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check`, isolated test-sleep
  checker, changelog format check, and `git diff --check`.
- Browser recorder run `a8905c41-6267-4604-b542-462811386e50` ran the selected launcher, Restart with, dialog, clone,
  replace, trash, header and Settings specs on Chromium and WebKit: 264 passed and four Settings cases failed on stale
  test premises. Narrow reproduction `4ab3e868-cb92-44be-acdd-be9651ee9d24` confirmed the count mismatch. Recovery
  `8f43fb50-f194-4c6e-b0fe-e877765f377f` exposed a second stale Settings viewport and row-count premise; after
  correcting those, `94e8d804-57c7-4829-b121-9414cb224c41` passed all four affected Settings cases.
- Earlier focused runs `73e08bb3-530c-4ceb-be64-49a020bcfb5f` and `d3e8dfd5-2618-4496-ab00-efd1c8b25c3c` stopped on new
  test assumptions: the gap helper rejected real painted gaps, then the host-removal test expected its confirmation
  button to remain mounted after acceptance. The helper and test were corrected, and narrow run
  `24c68c23-fdd6-467c-8c00-369d78c67359` passed host removal on both engines. All failed records remain retained as
  failures, separate from their later passing observations.
- After the final modal-only ring change and web rebuild, recorder run `e218f9c9-b782-44a0-8454-2cd721acdf0a` passed
  launcher focus and notification-panel focus/close on both engines. The broad run’s other 264 passes are reused because
  the production keyboard code they covered did not change afterward; the Settings recovery and final ring run cover the
  later test and CSS corrections.
- Main moved during the work only through plan queue, report and landing bookkeeping. A clean rebase onto that main
  changed no product behavior, so the existing runtime evidence applies. The full Rust and browser batteries were
  skipped: the selected checks covered the affected behavior and no remaining interaction called for wider runs. CI was
  not requested; draft PRs do not start it.

# Review gate’s outcome

The required fresh-context high-effort review found that the first ring selector also covered the notification bell. It
was narrowed to modal dialogs, and the reviewer’s follow-up confirmed the fix. The same follow-up identified one more
stale Settings geometry count, which was corrected and retested. No review finding remains open.
