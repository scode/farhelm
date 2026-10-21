## Since the last review

The earlier blocked round preserved the trust/hint notification fix (#2443) and tunnel-close completion fix (#2445).
Those PRs were rebased without changing their content. The maintainer accepted the earlier request-parsing boundary; the
new routing PR (#2458) and handler cleanup PR (#2460) now finish that design. No PR was dropped.

## What this was about

The helm must finish an accepted action when its client disconnects. Each handler previously had to arrange that itself;
forgetting could save a setting without notifying other clients, or remove a tunnel without announcing its removal. This
plan makes ownership the default for ordinary authenticated mutations, with deliberate cancellation exceptions at
routing.

## Things you should know

The inventory found two groups of clear omissions, fixed before the routing change: trust-setting saves and trust-hint
dismissals could lose their notification after persisting; a cancelled tunnel close could omit the notification after
removing its listener. These are separate fix PRs. Consent cleanup retains its existing lock-release order and both
notifications for a combined consent/hint change.

The maintainer explicitly accepted one additional difference during execution: a valid authenticated action may now
finish parsing and take effect after its caller disconnects. Ownership begins after authentication, before parsing. The
implementation specification and changelog describe that boundary. Authentication and rejection responses retain their
existing contracts. The earlier block asking for this decision is resolved.

Every route registration was inventoried and rechecked after rebasing: 79 registrations in standalone mode and 77 in
embedded mode, with 34 authenticated mutation methods receiving the default and 17 explicit cancellation exceptions. GET
and implicit HEAD remain cancellable; WebSocket lifetimes, public preflight and token exchange are unchanged. Uploads
and downloads retain upstream abort and unfinished native-save cleanup. Read-only POSTs, Stop, tab operations, tunnel
opening, Retry, provisioning admission, feedback and clipboard keep their documented cancellation boundaries. Shared and
narrower operations retain their own tasks where agent callers, lock boundaries or task-error mapping require them. The
agent relay itself is unchanged.

The redundant complete-handler tasks are removed, and the covered Code cleanup TODO entry is removed. All PRs remain
drafts; this execution does not merge or deploy them.

## Open questions and possible follow-ups

No maintainer decision remains. This guarantee covers client disconnection during a running helm, not survival of a helm
crash or restart. The transport probe that informed the accepted boundary used HTTP/1 and a stand-in mutation;
production-router cancellation regressions separately verify persisted trust and notification behavior. No claim is made
about the frequency of that boundary in browsers or HTTP/2.

## PRs

- [#2443](https://github.com/scode/farhelm/pull/2443/changes): trust-setting and hint notifications survive
  cancellation.
- [#2445](https://github.com/scode/farhelm/pull/2445/changes): tunnel close completes shutdown and announces removal.
- [#2458](https://github.com/scode/farhelm/pull/2458/changes): default request ownership and explicit cancellation
  exceptions.
- [#2460](https://github.com/scode/farhelm/pull/2460/changes): remove redundant handler tasks and finish
  specification/TODO documentation.

## Checks run, reused and skipped

Recorded Rust runs used nextest 0.9.148, tmux 3.7c, four slots and zero retries. Successful records have complete source
identity, JUnit reports, retained output through EOF and process cleanup, with no runtime substrate skips.

- The routing unit passed 55 selected auth, ownership, panic, transfer cancellation, CORS and rejection tests in run
  `f731a14d-43b5-4173-b005-b2214994b2cc`, then 25 client-log and notification contracts in
  `b92372b8-f772-4eb6-a36b-dac9a9228ae2`. Two initial filter patterns selected no tests; the second run corrected that
  coverage gap. These are separate successful observations, not retries of a failure. They cover the executable content
  of routing head `7f8d50c0`; the final unit changes only that unit's test comment, leaving these contracts unchanged.
- The final cleanup passed 250 selected handler and shared-operation contracts, including the migrated trust-save
  cancellation regression, in `19b0f4d4-94df-4ae4-9e24-7ab805508f78`. These cover session and host mutations, approvals,
  tunnel shutdown, trust/preferences, templates and notifications, plus the provisioning service's direct removal call.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test-sleep checker,
  targeted Markdown formatting and changelog fragment formatting passed for the routing unit. The final cleanup also
  passed helm all-target Clippy, formatting, the sleep checker and targeted specification/TODO formatting. A subsequent
  three-line test-comment correction reuses runtime/Clippy evidence for final head `a7e04334` because no executable code
  or assertion changed; formatting and sleep checks were repeated.
- The earlier fix units reused their recorded evidence after a careful rebase proved both unit diffs unchanged and
  inspected all intervening upstream changes. Trust/preferences: 18 passed in `e0234c85-0a60-4bda-ad79-5ab94d271aef`,
  with the two corrected consent/hint cases passing in `43544258-0f11-4ef4-8a32-fd764dd76686`. Tunnels: 19 passed in
  `b0aa49a2-6867-42be-92ed-88ceabf81737`, with the corrected shutdown fixture passing in
  `d0261723-2c93-41bc-b44a-08c27ee482ac`. Reuse is for unchanged cases; corrected cases use their later runs. The
  corrected fixtures preserve the actual lock-release and cancellation boundaries. Their coverage remains applicable to
  rebased fix heads `5fa2bf51` and `4e65a292`: both replayed unit diffs were identical, and upstream changed none of
  their store, lock, feed, tunnel, authentication or fixture contracts. The final 250-test run also exercises the
  integrated trust/hint and tunnel fixes after wrapper removal.
- Whole-workspace Rust, browser, desktop, installer and provisioning execution batteries were skipped because these
  changes affect helm routing/handler lifetime; focused real-router and shared-helper contracts cover the identified
  interactions. No live installation was changed.

The recorded runtime selections below are the exact `-E` arguments for this shared command, after the repository's
guarded pinned-nextest/tmux PATH setup. The selection label describes the corresponding row.

```sh
python3 scripts/record-test-run.py --runner nextest --kind development \
  --selection '<row description>' --concurrency '4 nextest slots; retries 0' --tmux required -- \
  cargo nextest run -p farhelm-helm --lib -E '<filter below>'
```

- Routing, `f731a14d-43b5-4173-b005-b2214994b2cc`:
  `test(request_ownership_tests::) | test(middleware::tests::) | test(auth::tests::) | test(=embedded_ui_tests::api_misses_never_fall_through_either_ui_source) | test(=embedded_ui_tests::embedded_mode_has_no_browser_ui_or_token_exchange) | test(=uploads::tests::upload_attachment_cancelled_handler_still_aborts_upstream) | test(=uploads::tests::attachment_preflight_answers_the_desktop_webview_origin) | test(=downloads::tests::cancelled_desktop_save_removes_staging_and_aborts_the_host) | test(=downloads::tests::desktop_file_cors_wraps_auth_and_preflight) | test(client_log::tests::http::) | test(clipboard::tests::) | test(=feedback::tests::an_oversized_body_is_refused_before_parsing) | test(=feedback::tests::an_unauthenticated_request_sends_nothing) | test(ntfy_tests::)`.
- Corrected client-log/notification selection, `b92372b8-f772-4eb6-a36b-dac9a9228ae2`:
  `test(client_log::tests::router::) | test(ntfy::tests::)`.
- Final handler contracts, `19b0f4d4-94df-4ae4-9e24-7ab805508f78`:
  `test(sessions::tests::) | test(hosts::tests::) | test(approvals::tests::) | test(checkout_config::claude_trust_tests::) | test(preferences::tests::) | test(tunnels::tests::) | test(ntfy::tests::) | test(templates::tests::) | test(request_ownership_tests::) | test(middleware::tests::ownership_layer_) | test(=provisioning::tests::removal_refuses_during_a_run_and_purges_after_it)`.
- Original trust/preferences, `e0234c85-0a60-4bda-ad79-5ab94d271aef`:
  `test(checkout_config::claude_trust_tests::) | test(preferences::tests::) | test(=tunnels::tests::cancelled_consent_handler_still_commits_and_closes_active_relay)`.
- Corrected consent/hint boundary, `43544258-0f11-4ef4-8a32-fd764dd76686`:
  `test(=preferences::tests::cancelled_hint_saves_still_publish_the_committed_dismissal) | test(=tunnels::tests::cancelled_consent_handler_still_commits_and_closes_active_relay)`.
- Original tunnel contracts, `b0aa49a2-6867-42be-92ed-88ceabf81737`: `test(tunnels::tests::)`.
- Corrected close fixture, `d0261723-2c93-41bc-b44a-08c27ee482ac`:
  `test(=tunnels::tests::cancelled_close_still_finishes_shutdown_and_announces_removal)`.

Static commands were `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`,
`<isolated-python> -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
`dprint check SPEC_impl.md` for routing and `dprint check SPEC_impl.md TODO.md` for the final unit. The sleep checker
inspected 305 delays with zero missing rationales. Fragment formatting covered the two fix fragments and the accepted
request-boundary fragment; the final refactor adds no user-visible change requiring another fragment.

## Review gate outcome

Each PR received exactly the required two independent gpt-6-astra/high reviews: general adversarial and
no-functional-change. The two fix reviews confirmed their changes fit the approved partial-action bug class. Review
findings about a consent lock lifetime and an unrelated fixture's initial connection events were corrected and checked
by the same reviewers. The routing reviews accepted only the maintainer-approved earlier parsing boundary and found no
other observable difference. The final cleanup's no-functional-change review found none. Its general review found a
stale test comment about the removed task; that comment was corrected, and the same general reviewer confirmed it
resolved with no remaining findings. No functional-change finding was waived.

Commit/PR wording passed fresh-context cold reads. Resume reconciliation passed. All required code-review findings are
resolved; the reviewers' static-review limits are distinct from the parent's recorded execution evidence.

### Landing

Landed as #2443, #2445, #2458 and #2460, merged bottom-up, with the same content the report describes, rebased onto this
round's main. It was the last of four plans in this landing round, after supervisor-request-modes.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 109's six plans, the browser suite's move to three stacks run in parallel, a
TLS certificate fix, documentation and plan bookkeeping. A fresh-context reviewer and I read them and the other plans of
this round against this plan.

- select-newest-on-close, from round 109, records each session selection together with the shared visit history in one
  helm write. Every branch of the preferences save still goes through that same write after this plan's routing change,
  and selection saves are now finished even when the client disconnects, so a selection made just before a page closes
  still records its visit. A page still cannot overwrite the shared visit history directly. Neither plan changes the
  helm's database schema.
- download-staging-sweep, from round 109, cleans up a desktop save's partial files on Quit. Downloads stay outside this
  plan's ownership, so dropping a download still aborts it and removes its unfinished files; this plan does not touch
  shutdown.
- supervisor-request-modes, earlier in this round: see its notes for how the two plans' cancellation choices agree.

The other conflict was in TODO.md, where round 109 had already removed neighboring entries; the resolution removes only
this plan's own entry.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed before round 109's six plans landed, so none had been tested against them, and
while this round ran, main also moved the browser suite onto three stacks run in parallel (each test worker runs its own
helm and supervisors). The plans were stacked again onto that main before the checks that count, so the browser evidence
below comes from the harness they land on. Together the four change how the supervisor runs every request, which helm
actions finish after a client disconnects, the Settings dialog and the launcher's Enter key; that reach is why this
round ran the whole Rust workspace and the full browser suite. All of the following ran on all four plans stacked
together on that main, including the fixes made while landing them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `scripts/check-desktop-assets.sh`, and the website build (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `fa6052db-ebdf-4204-8d20-25b8780367ec` (kept): 3977 passed, 1 failed:
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, which has an open TODO deflake entry ("Shallow
  pause resets under load"). Because supervisor-request-modes changes how the supervisor reads messages from a host
  connection, I ran that test alone ten times on main without this round and ten times on this round's stack, minutes
  apart on the same machine: 9 and 10 failures. Both sides fail almost every time, so that comparison cannot show
  whether the plan changes the test; it does show the test is now close to always failing on main on this machine, where
  round 109 saw it fail one time in five. See the possible follow-up in supervisor-request-modes' notes.
- `cargo nextest run -p farhelm-ui --features desktop`, run `d8ff6545-9b27-45f8-aa33-7dfac8f07108` (kept): 724 passed, 1
  failed: a timing assertion in the tests of how the UI orders preference saves, code no plan here touches. Ten
  repetitions alone all passed, so I judge it machine load during the full run; on one observation it is not recorded as
  a flake. `node --test` in `crates/farhelm-ui/js-tests`, run `b88528dc-6d84-4d63-996d-56eb15c69510`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers, no retries, run
  `ac3bef42-b713-4f70-8e6e-0fd7b86e0a16` (kept): 2227 passed, 14 skipped by design (WebKit clipboard cases and tests
  that need a real signed-in agent or run on one engine only), 3 failed, all on WebKit:
  - a terminal test and a two-host test each saw a session marked as having new output where they expected a plain
    status. Neither test file is changed by this round, and three repeats of each on both engines then passed (run
    `8158eb60-714f-4b18-bf7f-51c8654840b8`), so I judge both timing under load with three stacks running; on one
    observation neither is recorded as a flake;
  - "the sidebar app bar explains the helm and window builds", which fails inside the browser tests' own way of faking
    the helm's build stamp: a faked reply arrives after its request was already settled. It fails on main without this
    round too, two runs in five (run `db0c82af-6a1e-4359-8724-7fa17c65dbba`), and first showed up on round 109's landed
    code, so it is not from this round; see the possible follow-up in structured-settings-dialog's notes.
- Before the parallel suite landed, the plans also ran on the earlier main. A narrow run there (run
  `87ee3a27-84d7-4a4e-ab9f-8d478f5f4b35`) found the Settings test failure fixed in structured-settings-dialog, and a
  full browser pass, stopped part-way when the parallel change landed (run `b8b3e14f-b202-4015-b7d9-1ef15efb820d`),
  showed the app-bar test above and a launcher search test fixed in launcher-enter-feedback.
- Skipped: the workspace and desktop doctests (no plan adds or changes a documentation example),
  `cargo check -p farhelm-desktop` and the desktop smoke test (the desktop app's own crate is unchanged), the native Mac
  app (not runnable here) and the installer (unchanged).

The report's checks ran on a main from before round 109's landings, so they no longer describe what landed; the combined
runs above supersede them. Nothing else in the executor's report became untrue at landing.
