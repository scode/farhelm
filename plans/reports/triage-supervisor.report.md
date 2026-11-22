## What this was about

Stopping a Bash or Zsh session could lose unsaved command history. A terminal that was already gone could leave the app
trying to attach instead of showing the recovery card. Delete and failed Restart could appear to finish cleanup while a
process or scope still needed attention. This plan fixes those behaviors and records a disposition for 63 review
outcomes across eight draft PRs. The landing monitor will record the result of landing below.

## Things you should know

- Shells get the hangup during Stop, Stop and suspend, and the session teardown in Delete and Replace. The scope path
  and the process-tree path use the same rule. A shell can save history during that opportunity if its own configuration
  allows it.
- A temporarily unreadable terminal host remains eligible for reconnection. A host known gone is reported as a missing
  terminal even if the supervisor retained a handle to it. An exited program can still have a live terminal host with
  readable output.
- Losing the terminal socket alone does not prove the process ended. Delete, tab Close, and Restart use the host's
  recorded process identity to confirm shutdown. Delete keeps the session when its recorded tab scope cannot be
  enumerated safely. If a failed Restart leaves an unconfirmed replacement scope, the reply says so and retains the
  replacement record.
- The supervisor's Interrupted documentation now covers both a host reboot and the removal of a legacy tmux terminal on
  startup. The protocol comments describe the existing fields without changing their format. The sampling, admission,
  and terminal lifecycle documentation is brought into line with the shipped paths.
- The terminal-host plan shares the adopted-host shutdown path and terminal fixtures. This stack changes the
  confirmation check in the supervisor's terminal-host runtime, its stand-in test host, and test callers. It also fixes
  column reconstruction in the e2e transcript helper. The terminal host, emulator, and shipped grid rendering are
  outside those edits. The spec-text plan shares the two specs, and the two protocol docstrings are shared with helm and
  client code. The agent-capture and environment Delete admission findings change supervisor tests or comments.

## Open questions and possible follow-ups

- **Retain failed Restart cleanup when the session was deleted** is in the Plan follow-up section of TODO.md. When
  Delete wins while a replacement is launching, the host record can be kept after an unconfirmed cleanup. The deleted
  session row carried the scope selection. The replacement's work can still run after its session has disappeared. A
  later startup can see the host gone, discard the record, and lose the information needed to stop the work that remains
  in its scope. Completing this part needs a durable scope record and startup retry. The current reply reports the
  unresolved cleanup.
- Four test gaps need more than the small fixes approved for this plan. The retry-backoff test lacks a timestamp at the
  actual scheduling boundary. Kicked-input coverage would need to send input over the displaced connection without the
  ordinary client's local filter. The Stop test checks final classification and a separate test checks the terminal's
  wait. It takes a new observation to prove that Stop itself entered that wait. The Delete directory test observes its
  earlier session fence and needs a witness at the directory gate.
- Two harness findings closed without code changes. In-process reboot diagnostics can free their fixture permit before
  cleanup. Nextest runs the tests in separate processes. A failed restart in the legacy tmux fixture can leave its
  private server alive after losing the socket directory. The original test failure stays visible in the retained
  result.
- The structured shell test runs Bash and verifies live input. Native command-history writes, an equivalent Zsh launch,
  and the missing-program case have no new end-to-end coverage. The remaining legacy tmux tab-only test was discarded
  because startup now retires those tabs before users can operate on them.

## Pull requests

- Shell history on Stop and session teardown. [#4520](https://github.com/scode/farhelm/pull/4520/changes)
- Missing-terminal recovery and cleanup confirmation. [#4527](https://github.com/scode/farhelm/pull/4527/changes)
- Service documentation. [#4529](https://github.com/scode/farhelm/pull/4529/changes)
- Test documentation. [#4530](https://github.com/scode/farhelm/pull/4530/changes)
- Restart and Create regression checks. [#4534](https://github.com/scode/farhelm/pull/4534/changes)
- Tab, Bash, and launch regression checks. [#4538](https://github.com/scode/farhelm/pull/4538/changes)
- Supervisor unit-test checks. [#4543](https://github.com/scode/farhelm/pull/4543/changes)
- Stand-in cleanup and harness triage. [#4546](https://github.com/scode/farhelm/pull/4546/changes)

### Outcome map

#### Shell history on Stop and session teardown

- `bash-stop-loses-unsaved-history.md` — Fixed.

#### Missing-terminal recovery and cleanup confirmation

- `gone-terminal-retained-handle-hides-recovery.md` — Fixed.
- `trailing-failed-host-relaunch-warn-scope-error-definitive-rollback.md` — Both cleanup paths refuse an unconfirmed
  scope. Durable retry after the session row is deleted is deferred to TODO.md, “Retain failed Restart cleanup when the
  session was deleted.”
- `trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md` — Fixed.
- `trailing-delete-held-gone-host-skips-adopted-process-confirmation.md` — Fixed.

#### Service documentation

- `host-lock-table-contradicts-startup-span.md` — Corrected the text.
- `trailing-launch-comment-terminal-host-inherits-tab-marker.md` — Corrected the text.
- `trailing-tab-settle-gate-bare-link-out-of-scope.md` — Corrected the text.
- `trailing-restart-definitive-comment-promises-no-external-effects.md` — Corrected the text.
- `trailing-restart-reply-stale-tab-rediscovery-comment.md` — Corrected the text.
- `trailing-held-start-documentation-retains-the-retired-uncancelled-tmux-child-lifetime.md` — Corrected the text.
- `trailing-host-scan-lock-map-forbids-required-host-io.md` — Corrected the text.
- `trailing-sentinel-reload-comment-denies-recorded-host-delete-fallback.md` — Corrected the text.
- `trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md` — Corrected the text.
- `trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md` — Corrected the text.
- `trailing-stop-comment-falsely-bans-retaining-original-final-screen.md` — Corrected the text.
- `trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md` — Corrected the text.
- `trailing-sampler-docs-promise-deleted-capture-failure-reset.md` — Corrected the text.
- `trailing-interrupted-doc-defines-only-reboot-attribution-after-retirement.md` — Corrected the text.
- `trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md` — Corrected the text.

#### Test documentation

- `systemd-isolation-comments-point-to-active-ledger.md` — Corrected the text.
- `trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md` — Corrected the text.
- `trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md` — Corrected the text.
- `host-flood-stall-window-claim.md` — Corrected the text.
- `trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md` — Corrected the text.
- `trailing-stopped-altscreen-test-comment-claims-inactive-buffer-erasure.md` — Corrected the text.
- `trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md` — Corrected the text.
- `trailing-flood-fixture-false-child-exit-terminal-lifetime-rationale.md` — Corrected the text.

#### Restart and Create regression checks

- `restart-test-counts-zombie-as-running-agent.md` — Fixed the Restart ordering check to treat a zombie as already
  exited.
- `last-output-fixture-readiness-precedes-trap.md` — Fixed the fixture so its readiness line follows installation of the
  exit trap.
- `restart-screen-captured-before-prompt-settles.md` — Fixed the screen comparison to wait for the fake agent's
  post-input prompt.
- `restart-test-types-before-safe-prompt-barrier.md` — Fixed both supervisor-process restart cases to wait for the
  fixture's prompt before typing.
- `trailing-failed-restart-loses-the-live-tab-preservation-regression-oracle.md` — Restored live tab preservation and
  input checks after a definitive failed Restart.
- `child-hangup-masks-missing-stop-tree-sweep.md` — Used the existing hangup-immune fixture to prove Stop reaps the
  unmarked descendant.
- `host-preservation-test-can-select-starter-pid.md` — Read the listening Unix socket's peer PID so the test observes
  the host after the starter forks.
- `stop-annotation-exit-witness.md` — Waited for the host's exit to become visible before asserting Stop's annotation.
- `trailing-removed-completed-keyed-create-reboot-replay-oracle.md` — Restored a completed create replay check after a
  simulated reboot with no surviving hosts.
- `trailing-deleted-ambiguous-create-test-loses-published-handle-and-pending-intent-oracles.md` — Extended the ambiguous
  create test to check the retained list entry and pending keyed reservation.

#### Tab, Bash, and launch regression checks

- `tab-retry-test-measures-observation-time.md` — No change. The end hook includes cleanup outside the configured
  backoff. Checking the scheduling boundary needs additional test instrumentation.
- `kicked-input-test-local-filter.md` — No change. Sending the displaced input through the supervisor would require
  replacing the filtered client's attachment setup in three tests.
- `final-stack-drop-claim-window.md` — Fixed the restart-gap fixture to wait for the old state-directory claim to be
  released.
- `trailing-deleted-integrated-unmarked-tab-shell-delete-oracle.md` — Fixed the host-tab test to prove Delete ends an
  unmarked shell and its hangup-immune job.
- `trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md` — Restored listing, Attach refusal, and explicit
  Close coverage for a host tab before reap.
- `trailing-lone-surviving-old-tab-loses-its-guard-and-stop-regression-oracles.md` — Discarded as obsolete. Startup now
  retires legacy tmux tabs before serving.
- `shell-session-e2e.md` — Added a structured Bash launch and live-input check using the shell's private startup file.
- `grid-text-omits-blank-columns.md` — Fixed transcript reconstruction to preserve omitted columns between runs,
  including after a wide glyph.
- `antigravity-first-refused-pin-test.md` — Added a first rejected Antigravity transcript check that verifies the
  durable pin and identity stay unchanged before a valid report.

#### Supervisor unit-test checks

- `stop-wait-test-releases-exit-before-wait-witness.md` — No change. The current test proves Stop's final classification
  and the adjacent test proves the terminal wait. Establishing that Stop entered that wait would need a new scheduling
  witness.
- `foreign-host-refusal-test-cannot-observe-shutdown.md` — Fixed the refusal test to observe whether the other session's
  host received Shutdown as well as whether its program kept running.
- `cancellation-test-releases-encoder-before-abort-witness.md` — Fixed the running seed test to join the cancelled
  caller while the encoder remains at its gate.
- `seed-cancellation-test-accepts-lost-emulator.md` — Fixed the ended seed test to require a successful emulator read
  within the deadline.
- `trailing-checkout-status-borrower-reload-keeps-origin-runtime.md` — Fixed the checkout interruption test to release
  the origin terminal runtime before reloading the borrower's session.
- `trailing-failed-spec-test-accepts-an-attempted-host-start-as-never-started.md` — Fixed the failed spec publication
  test to require the write failure and verify that the host start path never created its directory.
- `trailing-deleted-startup-error-artifact-readonly-oracle.md` — Restored read-only startup coverage for a durable Error
  row whose launch spec and failure marker must remain intact.
- `trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md` — Fixed request conformance to
  keep its target terminal host alive during the contention observation.
- `trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md` — Fixed the migrated seal test to exercise
  the CHECK on the existing metadata row.
- `trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md` — Fixed the OMP Resume test to wait for the
  owner and its reporter to exit before reading the durable offer.
- `trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md` — No change. A witness at the
  directory wait would need to instrument the mutex acquisition. The existing session-fence witness provides no
  observation at that boundary.
- `trailing-test-one-shot-sample-before-host-catch-up.md` — Fixed the one-shot work-start sample to wait for the resumed
  terminal screen and catch-up first.
- `trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md` — Fixed the list-limit test to read the
  actual connection while all list permits remain held.

#### Stand-in cleanup and harness triage

- `blocking-test-cleanup-prevents-async-listener-release.md` — Fixed a misleading harness failure on a passing test. The
  fixture joins its cancelled listener, unlinks the socket, and verifies that the state directory was removed.
- `reboot-concurrency-early-release.md` — No change. The early permit release affects only concurrent in-process
  diagnostic runs. The maintained nextest runner isolates tests in separate processes, and the finding establishes no
  lost verdict or evidence.
- `trailing-old-server-guard-loses-directory-in-consumed-restart.md` — No change. The leak is conditional on an
  already-failed retirement in a legacy tmux fixture. The original test failure remains visible, with no demonstrated
  loss of recorded evidence.

## Checks run, reused and skipped

Every runtime run used the test recorder with four nextest slots and no retries. The following runs are Linux evidence.
No selected case reported a runtime skip. Commit links added to the outcome archive after the runs do not affect the
code under test.

- **Shell history.** `cargo nextest run -p farhelm-supervisor --lib`, selecting supervisor sweep and teardown, passed 69
  cases at `48b0a3db8bc` in `8e2e7dd4-0de2-4c79-b851-c902f9304ac1`. The corrected saved-Bash-launch fixture passed at
  `d05e3bab4d4f` in `5738d876-a4e5-4ead-ad4f-a2f3724c7370`. The stand-ins exercise the actual signal and the scope
  policy.
- **Lifecycle and cleanup.** Nextest selections for supervisor status, teardown, scope and terminal handling, and the
  CLI's host-relaunch e2e tests passed 107 cases at `60ea5512cb21` in `9743d5ff-1fc6-45c6-9000-f5a7e5033f64`. The 67
  affected cases passed at `130e7c124371` in `1effb533-ed89-4f00-a2f0-394e18282b6e`. The later change to that source was
  the scope enumeration error text.
- **Restart and Create.** `cargo nextest run -p farhelm --test e2e` selected 12 cases at `05f46a18a56a`, all passing in
  `b91fc7d6-ee99-45e5-b07a-6fbd056d943d`. The three stronger assertions passed at `ebfcafe9ea5d` in
  `f45c0e7f-f986-45db-af2b-3df7bf740cf3`. The final assertion borrows a value that the tested form cloned.
- **Tab, Bash, and launch.** Ten selected e2e and supervisor cases passed at `86e981b9a0ed` in
  `454da6fa-f95a-4cc7-9eef-84eb47957e80`. Three revised cases passed at `cca3284beefc` in
  `e1256f0f-84a9-4133-a343-bcbe6ca9b2de`. The final Bash error assertion uses `expect_err` for the tested condition. The
  experimental tab-retry timing check was withdrawn.
- **Supervisor unit tests.** The first 11-case selection at `f5864f628b4b` passed 10 cases in
  `63dd5c53-cdcc-4df5-a8c1-1baab453c76a`. The seal test found that the fresh database had no metadata row to update. The
  four affected cases passed at `4fa7bb3ea9f7` in `8bd113d3-0fd9-4529-a4ce-46869dc4aa92`. This includes the exact
  borrower interruption fixture and the OMP case with Bun. Seven earlier passing observations cover assertions that
  stayed the same. The foreign-host case was tested again for cleanup in the last PR.
- **Harness cleanup.** The exact foreign-host refusal and state-cleanup case passed at `601e02d3ef57` in
  `83a1728b-e50e-4abd-afa7-14eeed80121c`. Its state directory was removed with no cleanup failure in the output. The
  test took 0.12 seconds in this run. The earlier observation at `f5864f628b4b` took 5.24 seconds and retained the
  directory after the harness reported an unconfirmed shutdown. The final edit preserves a listener panic after unlink
  and reports any error looking up the directory. The normal cleanup sequence is the one the test ran.
- **Static checks.** Strict targeted supervisor and e2e Clippy, Rust formatting, the test-delay check, changelog
  fragment validation for the two product PRs, Markdown format, and diff whitespace checks passed for the applicable
  drafts. The instruction check had zero errors and warned about an inherited archive citation whose label is a
  checklist bullet.
- **Skipped.** The two documentation PRs change prose without doctests or generated content. Their validation was
  limited to formatting and instruction checks. Native macOS and workspace-wide tests were outside the targeted Linux
  selections. This stack changes no rendered desktop UI.
- **Upstream interactions.** Compared main `e1889c9919c4` with the stack base `1a53d9d35889`. The trial merge found
  conflicts in the active and archived triage ledgers and the feedback index. The new Podman, checkout, desktop and
  plans changes do not alter the supervisor paths under test. The protocol edits concern different declarations, and the
  spec edits describe separate behavior. The draft stack kept its tested base. The remaining conflicts are shared
  bookkeeping for the landing rebase to reconcile. Prior runs cover unchanged test and product paths, subject to the
  per-run changes above. They provide no full-suite result for the merged revision.

### Recorded test selections

- `8e2e7dd4-0de2-4c79-b851-c902f9304ac1` used `cargo nextest run -p farhelm-supervisor --lib` with `-E` matching
  `/^service::sweep::tests::/`, `stop_live_agent`, `/^service::teardown::tests::/`.
- `5738d876-a4e5-4ead-ad4f-a2f3724c7370` used `cargo nextest run -p farhelm-supervisor --lib` with `-E` matching
  `=service::sweep::tests::a_sigterm_ignoring_main_shell_ends_by_hangup`.
- `9743d5ff-1fc6-45c6-9000-f5a7e5033f64` used `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e` with
  `-E` matching `/^service::(status|sweep|session_hosts|teardown|core::host_restart)::tests::/`,
  `closing_a_tab_whose_runtime_reads_gone_confirms_its_process_first`, `/^host_relaunches::/`.
- `1effb533-ed89-4f00-a2f0-394e18282b6e` used `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e` with
  `-E` matching `a_shutdown_whose_process_stays_is_unconfirmed`,
  `a_listing_during_a_restart_keeps_the_session_as_last_listed`,
  `/^service::(status|teardown|core::host_restart)::tests::/`, `a_broken_user_manager_splits_warn_from_refuse`,
  `delete_refuses_a_recorded_tab_scope_the_manager_cannot_check`, `/^host_relaunches::/`.
- `b91fc7d6-ee99-45e5-b07a-6fbd056d943d` used `cargo nextest run -p farhelm --test e2e` with `-E` matching
  `restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts`,
  `the_seed_can_hold_what_the_old_program_printed_as_it_stopped`,
  `an_orderly_restart_keeps_a_host_agent_running_with_its_screen`,
  `a_host_agent_survives_an_orderly_supervisor_restart`, `a_host_agent_survives_a_killed_supervisor`,
  `an_unusable_replacement_host_is_ended_and_the_restart_unwound`,
  `stop_kills_a_host_agents_tree_and_leaves_the_host_showing_its_screen`,
  `hosts_of_known_sessions_and_unreadable_hosts_are_left_at_start`,
  `a_failed_restart_restores_the_stop_annotation_it_had_cleared`, `a_repointed_working_directory_refuses_the_restart`,
  `a_completed_keyed_create_replays_after_a_reboot`, `an_unusable_host_that_cannot_be_ended_keeps_the_launching_record`.
- `f45c0e7f-f986-45db-af2b-3df7bf740cf3` used `cargo nextest run -p farhelm --test e2e` with `-E` matching
  `a_completed_keyed_create_replays_after_a_reboot`, `an_unusable_host_that_cannot_be_ended_keeps_the_launching_record`,
  `an_unusable_replacement_host_is_ended_and_the_restart_unwound`.
- `454da6fa-f95a-4cc7-9eef-84eb47957e80` used `cargo nextest run -p farhelm -p farhelm-supervisor --lib --test e2e` with
  `-E` matching `a_failed_tab_end_is_retried_by_the_reap_after_its_backoff`,
  `opening_a_tab_on_a_restart_gap_session_is_a_restart_first_conflict`,
  `close_stop_and_delete_kill_a_hangup_immune_unmarked_job_of_a_tab`,
  `an_exited_tab_refuses_attach_before_its_host_is_reaped`, `a_structured_bash_session_takes_input`,
  `a_grid_row_keeps_the_gap_between_runs`, `a_host_agent_survives_an_orderly_supervisor_restart`,
  `a_host_agent_survives_a_killed_supervisor`, `a_host_tab_beside_a_host_agent_opens_attaches_and_closes`,
  `antigravity_pin_corrects_ancestors_once_and_keeps_version_zero_resume`.
- `e1256f0f-84a9-4133-a343-bcbe6ca9b2de` used `cargo nextest run -p farhelm --test e2e` with `-E` matching
  `a_structured_bash_session_takes_input`, `a_host_tab_beside_a_host_agent_opens_attaches_and_closes`,
  `close_stop_and_delete_kill_a_hangup_immune_unmarked_job_of_a_tab`.
- `63dd5c53-cdcc-4df5-a8c1-1baab453c76a` used `cargo nextest run -p farhelm-supervisor --lib` with `-E` matching
  `a_record_naming_another_sessions_host_refuses_and_touches_nothing`,
  `a_cancelled_seed_of_an_ended_runtime_leaves_the_emulator_readable`,
  `a_cancelled_seed_of_a_running_runtime_gives_the_emulator_back`,
  `checkout_reply_metadata_survives_replay_borrowing_and_reload`,
  `create_session_never_starts_a_terminal_after_a_failed_spec_publish`,
  `generation_zero_error_cleanup_preserves_create_replay`,
  `declared_nonblocking_requests_process_the_following_frame_under_contention`, `migrated_and_fresh_schemas_agree`,
  `omp_accepted_capture_survives_owner_exit_and_reload`,
  `waiting_to_running_sample_persists_the_authoritative_work_start`,
  `a_list_waiting_for_its_own_limit_does_not_hold_up_typing`.
- `8bd113d3-0fd9-4529-a4ce-46869dc4aa92` used `cargo nextest run -p farhelm-supervisor --lib` with `-E` matching
  `=store::tests::migrated_and_fresh_schemas_agree`,
  `=service::core::tests::checkout_interruption_status_matches_reload`,
  `=service::core::tests::create_session_never_starts_a_terminal_after_a_failed_spec_publish`,
  `=service::core::tests::omp_accepted_capture_survives_owner_exit_and_reload`.
- `83a1728b-e50e-4abd-afa7-14eeed80121c` used `cargo nextest run -p farhelm-supervisor --lib` with `-E` matching
  `=service::core::tests::host_terminals::a_record_naming_another_sessions_host_refuses_and_touches_nothing`.

## Review gate

All eight drafts cleared the executor review gate. Each had one independent Sol high source review and a separate cold
read of its commit message and PR description. The executor applied the accepted findings and rechecked the affected
code. Landing validation is a separate step. The source reviews strengthened process identity, scope and restart
confirmation, retained-launch and tab assertions, and the distinction between an accepted test premise and a passing
outcome. The no-change and partial dispositions above reflect the agreed small-fix and harness gates. The report had a
separate cold read for whether the decisions are understandable and suitable for the public repository.
