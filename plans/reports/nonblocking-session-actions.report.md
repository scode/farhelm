## What this was about

Stopping, restarting or replacing a session could make you wait before opening another session because switching would
tear down the work waiting for its reply. Starting a session also kept the launcher over the page for the whole wait.
The requested change was to keep other sessions usable while each action retained its own result and retry state.

Pending session actions now allow sidebar and quick-switcher navigation, marking and actions on other sessions. The
acted-on session still refuses competing actions. Stop shows a spinner and **Stopping…** on its row and at the top of
the open session. Failures, unknown outcomes and partial results remain on the source row and appear at the top when you
return. Switching away no longer cancels the request waiting for that result.

## Things you should know

Submitted launches, including Restart with and Replace with, hide the launcher and show its existing progress pill after
about two seconds. You can switch sessions and type in their terminals. A failure, unknown outcome or question brings
back the same draft; it waits if another dialog is open. The launcher cannot start a second request or accept a
replacement draft while busy. Switching to another host does not change the submitted destination or its retry key. The
launcher remains modal during preparation and its billing or YOLO questions. Launches and host-registry changes still
exclude each other.

A successful New or Clone still opens its new session, as requested. Replace and Replace with open the replacement only
while the original remains selected; otherwise the replacement appears in the list while you stay where you are.
Automatic selection after Delete waits for every admitted Delete, including queued bulk rows. Pending Stop batches do
not delay it, and an explicit open of a deleting session shows its existing deleting state.

The existing list now owns requests that must survive a session switch. The implementation reuses the existing
per-session claims, errors, launch owner and host-operation exclusion. It adds no backend acknowledgement, deadline
change, operation queue or general modal framework. The original TODO entry is removed in the final PR.

## Open questions and possible follow-ups

No open design decision. Browser validation covers Chromium and WebKit, including real local and SSH fixtures;
Tensorlake provider actions use the existing controlled presentation fixtures. It does not establish native macOS
desktop-shell or real cloud lifecycle coverage.

## PRs

- [#3276: keep session actions from blocking navigation](https://github.com/scode/farhelm/pull/3276/changes).
- [#3278: free the page while sessions launch](https://github.com/scode/farhelm/pull/3278/changes).

## Checks run, reused and skipped

The final combined browser run passed all 584 cases, 292 in Chromium and 292 in WebKit, with no skips, failures, flaky
outcomes or runner errors. Recorder `0a9cd431-cd92-4af8-bc4b-d59e08d55e42` ran the eleven affected specs with three
workers and no retries on the published stack at `0fbd8575fbeed93a738afce8d9ffc6c95c577503`. Source, runner policy and
reports are complete; retained output reached EOF without truncation, and all three stacks completed cleanup. The source
fingerprint is `a3d336e5780f5baed369807d6a571e2af3f0bb0e49a9367ab22dccbcc5d0c2de`. The child command, from `e2e`, was:

```sh
npx playwright test 'tests/(bulk-session-actions|header|quick-switcher|replace|sidebar|stop-tabs|tensorlake-sandbox-sessions|terminal|launch-progress|restart-replace-progress|nonblocking-launch)\.spec\.ts'
```

The final focused run, recorder `2c00f1de-b46d-458b-9412-9e77c7413ce8`, passed 32 cases, 16 per engine. It checks real
terminal input and navigation during launch, independent session actions, retained draft and retry identity across
hosts, modal recovery, selection after success, progress timing and existing-host Resume before creation. Its source
fingerprint is `c73d50fa3a15defdf1783cc12721f567361f19242afe528344e80a8d22ef0897`. Folding the reviewed Restart-text
expectation into the first PR preserved the combined tree exactly; only commit ancestry changed before publishing.

`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the pinned
web release build, `cargo fmt --all -- --check`, Markdown checks, the test-delay checker and changelog-format lint
passed. The test-delay checker inspected 429 delays with no missing rationale. Retained phone-sized progress-pill
captures were visually inspected in both engines: the pill fits the window, keeps the page visible and leaves elapsed
time readable when a long name or host is truncated. This presentation is unchanged in the final stack.

The UI Rust suite, `cargo nextest run -p farhelm-ui`, passed 940 tests with no skips in recorder
`349c2dcd-f05d-499f-bbf8-e71f6cefb298`. Its source fingerprint was
`4442312ffa9d0bf69e5860c7cc8bc829c4535cefb28442aebbdb1dd7567091ca`, based on PR #3276. This coverage is reused for the
unchanged unit contracts. The later delta only pins the admitted host through existing choice state, updates its three
launch handoffs and strengthens browser fixtures; renewed Clippy, desktop and web compilation and the cross-host browser
cases cover that delta. The CLI build is reused from the plan's base, since backend, protocol and CLI sources did not
change.

Earlier browser commands remain recorded as failures: `95484497-15f9-4016-9989-6d679cf46894` found a Clear-after-open
test premise; `9b0c7df3-97e9-44cd-b62a-8953667e08dc` found stale ordering, question-lifetime, completion and
error-surface expectations in eight cases; `05f6a7a3-575f-4573-b420-401e46bd1755` found paused-paint focus and
ordinary-host fixture premises. `04355658-05a4-4dcf-9629-20c8be054c58` then ran all 584 cases, with 582 passes and the
plain-Restart exact text expectation failing in both engines because the shared error now includes `Restart:`. That
correction was folded into the first PR. Whole run records were retained before correction. Narrow retries passed; these
failed commands are not counted as successful validation. No latent-flake claim follows from test changes authored and
corrected here.

Workspace-wide Rust, browser specs outside the affected selection, native desktop runtime, provisioning, installer,
release and cloud tests were skipped because no corresponding behavior changed. No CI run was dispatched; CI is manual
and the browser CI job is disabled.

## Review gate outcome

Each PR received the required single fresh-context `gpt-6.1-sol` review at high effort. Findings about durable Restart
observation, delivery-time composer admission, cross-host retry preservation and the named progress-label expectation
were fixed. The final source and fixture reviews are clear. Fresh reviews of the design kept the work on the existing
owners and exclusions, and fresh cold readers checked both PR messages.
