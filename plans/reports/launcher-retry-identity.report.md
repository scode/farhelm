# Session launch retries

## What this was about

Retrying a session launch could do the wrong thing in three cases. New, Clone and Replace with kept returning an old
missing-folder refusal after the folder was restored. A browser authentication refusal could make a partially completed
Replace forget the replacement it had already created. Reapplying an unchanged template after a quick launch lost its
reply could likewise start a second session.

During triage, the maintainer chose one rule for every launcher: an unchanged retry keeps the same attempt until it
succeeds or the helm proves nothing was created. Each fix was limited to about 100 changed lines. The three fixes meet
that rule and measure 76, 66 and 56 added/deleted lines respectively, excluding outcome bookkeeping, feedback removal
and changelog fragments. The specification now states the shared rule.

## Things you should know

A repaired folder now permits a fresh attempt when the helm has settled the original refusal. Authentication refusals
retain a partial Replace attempt, allowing the next retry to recover its existing replacement. Reapplying a template
that leaves the launch unchanged recovers the earlier attempt. Changing the launch settings starts a new attempt. No new
retry store or helm protocol was needed.

The authentication browser tests use a consumed 401 without the sign-in marker, after a real replacement was created.
They verify recovery through the other Replace control and after navigating away and back. The recognized sign-in
refusal and an unreadable 401 body are covered by the unit test; the complete sign-in screen sequence was not run.

## Open questions and possible follow-ups

None required to finish these outcomes.

## The PRs

- [#2390](https://github.com/scode/farhelm/pull/2390/changes): retry New, Clone and Replace with after a settled
  refusal's folder is repaired; shared specification rule.
- [#2393](https://github.com/scode/farhelm/pull/2393/changes): preserve an unresolved Replace through authentication
  refusals.
- [#2398](https://github.com/scode/farhelm/pull/2398/changes): preserve retry identity through an unchanged template and
  start fresh for a changed request.

## Checks run, reused and skipped

All runtime runs used the recorder, pinned nextest where applicable, pinned tmux, zero retries and the configured
concurrency. Browser runs used one worker and both Chromium and WebKit.

- PR #2390:
  `cargo nextest run -p farhelm-ui --lib -E 'test(api::tests::a_stale_create_refusal) | test(api::tests::a_yolo_confirmation) | test(api::tests::replace_retains_only_unknown_or_yolo_refusals) | test(list::create_form::tests::an_intent_binding_changes)'`
  passed four tests, run `83e57377-655f-4139-b893-d69e2085ebc8`.
  `npx playwright test 'replace\.spec\.ts' -g 'launchers start fresh after repairing a missing folder'` passed ten
  cases, five per engine, run `e9b5b5fc-790e-43c2-a456-c6db9e831e66`. This covers the real settled refusal, folder
  repair and unchanged retry across all five controls.
- PR #2393:
  `cargo nextest run -p farhelm-ui --lib -E 'test(api::tests::replace_retains_unknown_authentication_and_yolo_refusals)'`
  passed one test, run `4aa25d41-4d62-4bba-9a7a-c7eed114e4ab`.
  `npx playwright test 'replace\.spec\.ts' -g 'plain Replace (retries an unknown outcome|shares a committed retry)'`
  passed eight cases, four per engine, run `03b6630b-d738-4d1d-8705-23a086039f8d`. These distinguish retained identity
  from a duplicate replacement, including a real partial success before the 401.
- PR #2398:
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::tests::an_intent_binding_changes) | test(list::create_form::tests::fresh_retry_matches_user_intent)'`
  passed two tests, run `f46d8794-e0e6-4c47-9d0a-e08b7adf9475`.
  `npx playwright test 'quick-launch\.spec\.ts' -g 'reveals a refusal without automatically retrying|reapplies .* after a lost reply'`
  passed twelve cases, six per engine, run `dec37d38-9a8a-4283-9340-f5b10fa73a9c`. These prove recovery of an already
  committed session after an unchanged template, and a new attempt when the template changes the request.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the isolated test-sleep checker,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and changed Markdown dprint checks
  passed for each outcome. The sleep checker found 304 delays and no missing rationales.
- `cargo build` passed for the first outcome. The native helm and fixture builds were reused for later outcomes because
  their inputs did not change. The Dioxus 0.7.10 web build passed for each browser run.
- Successful runtime evidence is reused after commit creation and the ledger-only PR-link amendments because executable
  code, test fixtures and effective build inputs are unchanged. The second outcome's final comment correction also
  leaves its runtime evidence applicable. This coverage applies to final heads `6b831d4f` (#2390), `8252a770` (#2393)
  and `46f13061` (#2398). Every new main diff from base `b122567a` through `dca5f578` was read: only TODO planning,
  queued goals, claims and other plans' reports changed. No product code, specification or governing instruction
  changed, so no rebase or additional runtime check was needed.
- Broader Rust, browser, desktop, installer and provisioning suites were skipped: these changes affect launcher retry
  classification and template application, covered by the focused unit and browser checks, with no changed server,
  desktop-shell, installer or provisioning behavior. CI was not dispatched; it does not run automatically on PRs.

## The review gate's outcome

Each outcome received the required single fresh-context gpt-6.1-sol review at high effort. The first review found a
missing command YOLO answer in the New browser fixture; the second found two nearby comments still describing the old
401 behavior. Both were corrected and verified by the same reviewers. The third review found no issues. No findings
remain. Fresh cold readers checked each commit and PR message.
