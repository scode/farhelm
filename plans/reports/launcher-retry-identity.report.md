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

### Landing

Landed as #2390, #2393 and #2398, merged bottom-up. It was the first of sixteen plans in this landing round, which
stacked every plan in queue order and checked them together before any merged.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans: host-entry-session-ids (sessions are now identified by their
host entry as well as their ID, so a session ID the browser sees is a full reference containing a colon),
helm-lock-outlives-work, hosts-panel-menu-close and template-list-current-labels. It also gained documentation and plan
bookkeeping: queued structural refactor plans, a trailing-review record, and a SPEC_impl.md rule preferring cleanup on
quit over start-up sweeps. A fresh-context reviewer and I read these and the other fifteen plans of this round against
this plan.

The one interaction was host-entry-session-ids. The browser test this plan extends for retrying after a folder is
repaired builds the Replace request's address from the session ID, and the new ID needs encoding in that address; the
textual conflict there was resolved with the encoded form (below). The retry rule itself does not depend on how sessions
are named. With template-list-current-labels and launch-sequences, which also apply templates, an unchanged template
still keeps the retry attempt: every template applied in an open launcher goes through the same path that preserves it.
launch-sequences lands later in this round. replace-yolo-shared-question, also later in this round, stops Replace from
asking the YOLO question a second time for the same session; when that happens the launch reports an ordinary error, and
the next retry still reuses the same attempt, as this plan expects.

#### Fixes made during the landing

- The Replace test for retrying after a repaired folder conflicted with host-entry-session-ids, which had changed the
  same test to address sessions by their encoded full reference. The resolution has the test expect Replace with at the
  session's encoded address, and New and Clone at the address for creating a session (#2390).

#### Possible follow-ups

- The new SPEC.md sentence under Creation says a retry attempt ends only on success or the helm's definite proof that
  nothing was created. The launcher also starts a new attempt when the helm reports that the host changed underneath the
  request, when the folder it would launch into changes, and, as the report says, when the launch settings change. Those
  behaviors are intended and mostly predate this plan, so the sentence reads stricter than the code. Possible follow-up:
  reword that sentence to say that a changed request also starts a new attempt.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, with the landing's fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`), so I treat the failure as machine load during the
  full run, not as something this round broke.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were focused runs on a main from before host-entry-session-ids; the combined runs above supersede
them for what landed. Nothing else in the executor's report became untrue at landing.
