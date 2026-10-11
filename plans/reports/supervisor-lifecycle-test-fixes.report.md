## What this was about

Seven supervisor tests did not reliably prove Farhelm's session lifecycle behavior. Tests about Stop and Restart could
pass without a running agent; tests about avoiding a terminal query or launch could pass even if that work happened.
Three tests could leave frozen or hangup-resistant helper processes behind after an early failure, adding load to later
tests on the shared machine.

The maintainer chose to fix all seven together, provided the changes stayed small and confined to tests. All seven are
addressed. Farhelm's product behavior and specifications are unchanged.

## Things you should know

The tests now establish a running agent before checking that a refused Stop leaves it alive and that Restart preserves
its terminal tabs. The existing dead-or-absent Stop and dead-agent Restart coverage remains. Listing without terminals
is tested against a verified terminal-query failure; a failed session-spec write is checked directly for the absence of
terminal panes.

The three leaking fixtures have cleanup guards. The frozen helpers remain unreaped until their owning child handle
waits, including when the initial suspension check discovers an unexpected exit. The hangup-resistant fixture rechecks
process start time and non-zombie identity before each cleanup signal. A race remains between that check and the signal:
if the fixture exits and its process ID is reused in between, cleanup could signal the replacement process. This is the
same accepted limitation as existing terminal cleanup. Linux-only process handles could close it on Linux, but adding
them was outside the agreed portable fix. This limitation is confined to test cleanup.

No outcome tripped its complexity gate. The one commit also records all seven completed triage outcomes and removes
their feedback entries. It adds no production seam or changelog fragment.

## Open questions and possible follow-ups

None requires a maintainer decision. These portable tests were executed on Linux; macOS execution remains unverified on
this host.

## The PRs

- [#1910](https://github.com/scode/farhelm/pull/1910/changes) — one draft PR for all seven test repairs. Ready state and
  landing remain the monitor's job.

## Checks run, reused and skipped

The focused and mutation runs used this redacted command, selecting only the tests named below:
`python3 scripts/record-test-run.py --runner nextest --kind development --selection '<named tests>' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor --lib -E '<selected-test-filter>'`.
The pinned nextest and tmux were first placed on PATH using the guarded setup in `docs/test-run-evidence.md`.

The initial focused recorded run passed ten selected tests, including the changed lifecycle tests, retained dead-agent
coverage and the terminal refusal classifier: `babfc0d5-1aa1-4e28-9b3b-964443353801`. It used pinned nextest and tmux,
four global slots and zero retries, with no runtime skips. The 1090 reported exclusions were deliberate filter
exclusions.

That run's filter was:

```text
test(stop_survives_connection_task_cancellation) | test(restart_survives_connection_task_cancellation) | test(delete_reaps_an_unmarked_process_under_a_tab_shell) | test(a_live_stop_that_cannot_record_its_intent) | test(a_dead_or_absent_stop_reports) | test(restart_reuses_live_agent_and_preserves_tabs) | test(restart_replaces_only_the_dead_agent_window) | test(list_sessions_skips_pane_states_when_nothing_has_a_terminal) | test(create_session_never_launches_tmux_after_a_failed_spec_publish) | test(refusal_classification_pins_every_recognized_diagnostic)
```

Four separately recorded production mutations each failed at the intended assertion after their fixture premises passed:
Stop signals before recording intent (`7d795d46-1e51-48bf-9668-8bfdffc3f5b3`); Restart sweeps the whole session
(`adf631bd-f7ea-473a-b75c-c504a865be6e`); listing queries terminals unconditionally
(`73569d32-27f8-40ab-89a0-94f05e39b195`); and session creation launches the terminal before publishing its spec
(`5c7be8c2-4b10-42d7-8277-ec6300185154`). Each ran one exact test once. All production mutations were restored and
verified byte-for-byte. The deliberate failures remain retained; they are not latent flakes.

Their filters, respectively, were
`test(=service::core::tests::a_live_stop_that_cannot_record_its_intent_leaves_the_agent_running)`,
`test(=service::core::tests::restart_reuses_live_agent_and_preserves_tabs)`,
`test(=service::handlers::tests::list_sessions_skips_pane_states_when_nothing_has_a_terminal)` and
`test(=service::core::tests::create_session_never_launches_tmux_after_a_failed_spec_publish)`.

Three fresh live-agent Restart repetitions passed, batch `4eb87f73-69bc-4a64-a536-9a61dea1ea18`, runs
`f08b86d3-b26c-410a-9372-c5d1943e0beb`, `579eaf4a-be95-4944-9f25-c219ca11890a` and
`7607e424-5fe9-4252-a33f-43e71823fc8b`. Each complete report contains one selected pass and 1099 filter exclusions, with
zero retries or runtime skips. The repetition command was
`python3 scripts/hunt-rust-tests.py --repeat 3 --timeout 600 --execute -- cargo nextest run -p farhelm-supervisor --lib -E 'test(=service::core::tests::restart_reuses_live_agent_and_preserves_tabs)'`;
the helper records each attempt separately with `--kind repetition`.

After the review correction, run `8d1b5a55-a412-4250-a9e8-f2b38183a437` passed both frozen-helper cancellation tests and
the new unexpected-exit regression: three passes, 1098 filter exclusions, complete report and no runtime skips. Final
`cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, changed Markdown dprint
and the isolated source delay checker passed. The delay checker inspected 287 delays with zero missing rationales. A
separate documentation pass covered both touched source files and outcome bookkeeping. The other redacted commands were
`<isolated-python> -B scripts/check-test-sleeps.py` and
`dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`.

The final three-test filter was
`test(=service::handlers::tests::stop_survives_connection_task_cancellation) | test(=service::handlers::tests::restart_survives_connection_task_cancellation) | test(=service::handlers::tests::a_stopped_fixture_observation_leaves_exited_children_waitable)`.

The runtime evidence covers the test edits based on `fc5c0d9a`; unchanged tests retain that coverage after
byte-identical mutation restoration, and the corrected guard has the separate affected-test pass above. The careful
rebase onto `540c16e7` introduced no conflict. Intervening installer refusals, UI license declarations and plan
bookkeeping have no interaction with these supervisor tests or specifications. Final lint and formatting passed on the
same source; later PR links only changed Markdown. No runtime rerun was needed for those unrelated changes. Full
workspace, browser, JavaScript, desktop, installer, provisioning and website tests are skipped because this diff changes
only supervisor test fixtures and assertions, and the focused checks cover their identified risks. Hosted CI, deployment
and release work were not requested.

## The review gate's outcome

The prescribed fresh gpt-6.1-sol/high source reviewer accepted the other contracts and inspected all four intended
mutation failures. It found one cleanup hazard: observing suspension with a raw wait could reap an unexpectedly exited
child without updating its owning handle, allowing cleanup to signal a recycled process ID. A nonreaping stop-or-exit
observation and an unexpected-exit regression address that finding. The same reviewer inspected the complete final
source diff and accepted the correction, with no remaining finding. The executor read both full review artifacts and
checked their conclusions against the source and retained evidence; review closure and runtime validation are separate
observations.

Commit and PR wording passed a separate fresh Sol medium cold read. The required native resume check reconciled the
claim, source and retained evidence. Implementation and investigation stayed local, as required by no-workhorse mode.
Native model attribution and usage counters were not exposed; requested model routes are not attribution evidence.
Private galaxy-brain records are retained under session UUID `d0e361cc-2cf7-49dd-a656-7f0f6a284fbf`, with their location
in the plan working log beside the checkouts.

### Landing

Landed on 2026-10-11 (UTC) as #1910. The rebase met no conflicts.

#### Fix made while landing

In #1910, before it merged: the two rewritten Stop tests' docstrings had lost why the tests matter. They again say that
SPEC.md requires a failed Stop to be reported rather than a "stopped" that leaves the session as it was, and why the
intent is recorded before any signal.

#### Review before merging

No blockers; every code change is inside the test modules. Each of the seven repairs now observes what its finding said
it missed, and the stopped-child, HUP-resistant and live-agent fixtures clean up on early failure without reaping or
signalling processes they do not own. Notes left as they are:

- The live-agent restart test checks the tab survives but not that the agent was actually replaced; comparing the agent
  pane's process before and after would close that.
- The new live-stop test uses the fixed session id `s1`; under the regression it guards against, a sweep would look for
  processes marked `s1` machine-wide. Nothing spawns such processes today; a fresh UUID would remove the question.
- The failed-spec test checks the end state only, so a create rolled back after making the window would still pass.
- The new fixtures have not run on macOS.

#### Checks

These two plans, which change only tests, were landed together after the round's product plans: supervisor-lifecycle-
test-fixes first, e2e-session-test-fixes on top.

- Run now, on the two stacked after the fixes: `cargo fmt --all -- --check`, `dprint check`, the test-delay checker (288
  delays, none without a reason) and `cargo clippy --all-targets -- -D warnings`; and the supervisor and CLI unit tests
  with the hook-identity, session-lifecycle, supervisor-stop, terminal-backpressure and terminal-tabs end-to-end tests,
  through the recorder with pinned tmux 3.7c, four slots and no retries (run `8d9839e7`, 1270 of 1270, no runtime skips
  in its output).
- Not run: macOS; the checks above are Linux only.
- Between that run and the merges, main changed only plan bookkeeping.

Nothing in the report above was made untrue by the landing.
