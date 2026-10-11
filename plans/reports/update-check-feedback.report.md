## What this was about

In the desktop app, selecting Check for updates could look like an unhandled click: progress and the result were only in
the version readout's hover. You chose progress beside the version, an up-to-date answer for about five seconds, and a
warning-styled failure that stays until dismissed or another check starts.

The readout now shows a spinner with “checking…” or “installing…”. “Up to date” expires after about five seconds; “check
failed” is a keyboard-accessible dismissal button, with the reason still in the hover. A waiting update keeps the
existing red readout, and a failed restart remains hover-only. The specifications describe the behavior, a changelog
fragment is included, and the covered TODO entry is removed.

## Things you should know

Both Check for updates in the help menu and Update on the local host row use this feedback. Automatic checks nobody
joins remain quiet, and the web UI has no updater to mount it. The existing hover wording, verification warning,
build-skew explanation, update menu, installation and scheduling behavior are retained.

Execution found that the UI can miss the checking step of a very fast run. The feedback now retains which user request
an answer belongs to, so an earlier expiry or dismissal cannot hide a later identical answer. A successful user check
with an update waiting has hidden “update ready” words in the same polite live region; automatic or external
installations cannot replay that announcement. This required small presentation bookkeeping in the existing updater,
accepted by the plan's scope review.

## Open questions and possible follow-ups

No decision blocks completion. The updater only runs in an installed Mac app, so its visible interaction was not
observed on this Linux executor. On a Mac, check progress from both entry points, the five-second success, persistent
failure and keyboard dismissal, readiness and failed-restart precedence, and the narrowest sidebar: text should shorten
without moving the gear or help controls. Check reduced motion and VoiceOver start/outcome announcements, including two
consecutive identical outcomes. Source review establishes persistent live-region structure and replacement of repeated
result nodes; actual audible delivery remains unobserved.

## The PRs

- [#1889](https://github.com/scode/farhelm/pull/1889/changes) — desktop update check feedback; remains draft, not
  readied or merged by this executor.

## Checks run, reused and skipped

Passed on final rebased source `2e9e4acc`: `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and `cargo check -p farhelm-desktop`. These check the native
updater, shared presentation and shipped desktop integration after upstream approval-protocol changes.
`cargo fmt --all -- --check`, changed Markdown/CSS `dprint check`, changelog format, and the isolated test-sleep checker
passed; the latter inspected 287 delays with zero missing reasons. The desktop UI compile also passed before rebase; the
final native lint and binary compile cover that source after integration.

Reused recorded runtime on `4bac1060` above base `8a666935`:
`cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(app_updater::) | test(desktop::updater::)'` passed 50
tests, recorder `506692a7-4a11-480d-bcbe-28896c2610a3`; `cargo nextest run -p farhelm-ui --lib -E 'test(app_updater::)'`
passed nine tests, recorder `ac500a28-9e28-44b5-b660-81bf9256b812`. Both used four slots, zero retries and no tmux
requirement, with complete JUnit and no runtime substrate refusal. The only later updater-source change is a documented
web-only diagnostic allowance for an intentionally unreachable component body. Rebase leaves that source unchanged, so
these behavioral results still apply.

Two recorded starts were deliberately interrupted during dependency compilation to take review corrections before tests
began: `c223b547-7e02-434a-9121-d68e357aa893` and `518842cc-416e-473e-bd54-f823ef36daf0`. Both are retained privately
and count as no test verdict. An initial desktop compile exposed a missing component macro import, corrected before the
successful checks.

Careful rebase onto `4047304a` reviewed the complete intervening product diff: agents creating managed checkouts,
approval cards and their protocol, CLI and fixtures, plus queue-only changes. It adds no updater, installation-record or
version-feedback contract. The only textual conflict was adjacent completed TODO removals; both were preserved. Both
plans' separate specification sections remain. Integrated compile/lint above covers the shared UI/protocol composition.

Full workspace runtime and lint batteries, browser, desktop interaction/smoke, installer, provisioning, release, website
and hosted CI checks were skipped. The changed behavior is confined to desktop updater presentation and its existing
worker bookkeeping; the focused unit selections cover the affected contracts and both feature builds compile/lint. A
browser cannot reach this installed-app updater. Native interaction remains the Mac observation above.

## The review gate's outcome

Implementation and investigation stayed with the executor under no-workhorse. The required single fresh-context
`gpt-6.1-sol` high source reviewer found and closed the repeated-outcome identity gap, missing readiness announcement,
stale announcement provenance after record recovery, and a mistaken injected test fixture setting. The final source gate
has no material findings. A scope reviewer on the executor's inherited native model accepted the necessary bookkeeping
and removed a redundant UI identity counter. The full test-authoring checklist was supplied to source review.

The `gpt-6.1-sol` medium wording reader accepted the commit message and PR title; the PR description is empty. Reviewers
did not run tests or observe the Mac app. Actual native model attribution and usage counters were unavailable. Private
galaxy-brain session `a63f6b2c-45f7-4830-bf84-ea76de63472d` retains routing and review events; the working log beside
the checkouts records its location and private run paths.
