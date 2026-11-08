## Since the last review

Both drafts remain. #3132 corrects the Delete tests' setup and waits for the exact tab opened after Stop to appear in
the session listing. #3133 was rebased without changing its own patch. No PR was added or dropped.

The unmodified whole Delete spec reproduced the reported Error status once in 40 WebKit executions. That failure
happened before any tab opened or Delete ran. The retained trace shows a live terminal followed by a launch failure: the
scope wrapper ended before the agent started. A Running listing does not establish that the agent has started. The trace
does not distinguish Stop interrupting the wrapper from an independent systemd refusal. It does establish that the
test's expectation of an ordinary started-agent exit lacked its required setup.

All four Delete cases now observe the fresh fake agent's readiness output before Stop. That happens before installing
the controlled feed or opening the Delete question, preserving the intentionally stale display and behind-question state
changes. Both post-Stop tab branches wait for the returned tab ID. Exited, exact tab counts, Delete refusals, retained
rows, refreshes and changed-consent assertions remain intact. No product behavior changed in this correction. The whole
spec then passed ten repetitions on each engine: 40 Chromium and 40 WebKit executions, with no skips or retries.

The earlier row-rendering correction and bottom-row menu fixtures remain. Unchanged rows keep stable confirmation
callbacks; the two menu tests use a taller fixture to exercise their original viewport-clamp assertions after the menu
grew. The landing's restored mixed-session Delete coverage, confirmation-setting specification, supervisor comment and
protocol version 70 are preserved.

## What this was about

Stop left ordinary sessions' terminal tabs and their work running, while a containerized Stop ended them. The maintainer
chose consistent behavior and a confirmation when tabs are shown.

Stop now ends the agent and closes its tabs and their processes, keeping the session and agent terminal viewable. The
session row, the buttons above the terminal and bulk Stop ask about visible tabs unless session confirmations are
skipped. Restart and an agent exiting on its own leave tabs running. A tab opened after Stop closes with the next Stop
or Delete. An approved agent-requested Stop has the same tab-closing behavior.

## Things you should know

A tab whose host cannot be read stays open and is named in a notice; healthy siblings still close. The GUI displays the
notice. Agent-requested Stop reports partial cleanup through its existing error response, and its approval card
explicitly authorizes closing tabs.

Consent protects the agent and tabs separately. A screen showing no tabs cannot authorize closing a newly opened tab; a
screen showing an ended agent cannot authorize stopping an agent restarted elsewhere. The guards also apply with
confirmations skipped and separately to every bulk target. A refusal stops nothing and refreshes the display without
automatically opening a question.

Remote supervisors need updating: this plan takes protocol 70 after the tunnel change took 69. The compatibility
boundary prevents an older helm's Stop from gaining permission to close tabs without asking. An old browser's bodyless
or empty-object Stop refuses open tabs; reloading obtains the new confirmation flow. No database migration is added.

The row above the terminal gains a Stop button. With the desktop tunnel pill present, action labels and spacing are
tighter to preserve the supported 650-pixel pane width; labels, icons, readable tabs and both port numbers remain. The
seven changed documentation pages cover Stop/restart/resume, working in a session, the session list, containerized
sessions, agent wrappers, host management and uninstall. This unattended run started no preview server.

## Open questions and possible follow-ups

No required work remains. The two frozen documentation-shot scenarios still describe the old header and Stop behavior;
the first return Decision leaves them to the next screenshot refresh.

The earlier process-cleanup timeout did not recur in its three fresh both-engine attempts or the broader 280-case run.
Its cause remains unconfirmed; those passes do not establish that it cannot recur. Existing cleanup limits for detached
processes on hosts without a usable systemd user manager remain. This round found no evidence that Stop misclassifies an
ordinary started-agent stop, and made no speculative teardown or status change.

## PRs

- [#3132](https://github.com/scode/farhelm/pull/3132/changes): Stop behavior, consent, notices, UI, tests,
  specifications and changelog; draft head `84768a8dec68e9804f37b6968796c7132f3515c5`, based on main.
- [#3133](https://github.com/scode/farhelm/pull/3133/changes): CLI help, agent instructions, seven docs pages and the
  addressed TODO removal; draft head `d6342068473cdff110d543c5794d7f06057ab61f`, based on #3132.

## Checks run, reused and skipped

Both units were carefully rebased onto main `4804aede8523`. The complete upstream diff was inspected: release and
documentation updates, flake bookkeeping and a trash-recovery test wait do not alter Stop's contracts. The original
units had empty interdiffs after rebase. A final fetch found no newer main.

Current tests captured head `582b0564388a` plus the final one-file correction, with complete source fingerprint
`9ea8f6b886bbcf1433f9874aac160d9bbbe424c9628dd9b90bd8a21c81c4b934`. Folding it into #3132 and restacking #3133 changed
history only: the full tree comparison with that tested source is empty.

Checks run for this return:

- Matching CLI/fixture and Dioxus 0.7.10 web builds passed in `1b365eb5-3a95-4216-8fa7-5a3541858b46` and
  `90135c50-7a89-477f-98ec-83ed261f939f`. They captured the rebased source before the test-only correction, fingerprint
  `3f677d7567ca331cf1f322d500df4314d1e469134fb819d65a6723c8f0132806`; production inputs stayed unchanged.
- `npx playwright test 'delete-precondition\.spec\.ts' --repeat-each=10 --retries=0` passed all 80 executions in
  `c62f9f59-6462-4633-91d3-1a43ad96ea6b`, using three workers and three owned stacks. Its actual JSON report was
  inspected: 40 Chromium and 40 WebKit passes, zero skips, failures, flaky outcomes or global errors. This used generic
  recording because strict browser mode refuses `--repeat-each`; these are directly inspected results, not a claim of
  strict recorder policy validation. The corrected exact WebKit case also passed five repetitions in
  `a325652a-29f9-40c1-88ef-ec8d558aec9c`.
- The focused `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm --lib --test e2e` Stop/tab selection
  passed 40 behavioral tests plus fixture setup in `f7231549-fb03-4d29-be7d-8898cb91b5ee`. It covers consent,
  startup/exit classification, cancellation, partial cleanup, real host and tmux tab process trees, Restart preserving
  tabs, mixed-session Delete coverage and the protocol-70 pin. Pinned nextest 0.9.148 used four slots and zero retries;
  pinned tmux 3.7c and the required systemd substrate ran. JUnit's 41 passes include setup; 2,529 selected-out tests are
  not passes. Complete output has no runtime SKIPPED marker.
- The isolated source sleep checker passed: 423 delays, zero missing rationales. Whitespace checks passed. The attempted
  dprint check of TypeScript found no matching configured plugin; it is not counted as a formatting pass.

The unmodified exact WebKit case first passed five repetitions in `4b7d0fb3-0eea-4d20-8be6-b63197c0018b`. The wider
unmodified run `a258a94b-25c5-466b-b314-510d7a1ac8ba` then failed one of 40 executions, with 39 passes. Its trace,
screenshot, status evidence and complete output remain retained separately. Later passes do not turn that aggregate into
a pass or identify the underlying wrapper failure more precisely than described above.

Earlier checks reused per command:

- All 908 UI tests passed in `ab78c17d-82dd-493f-a2c9-0b67f84d7554`, at `9abbc7dac889` plus the callback correction,
  fingerprint `bc746204357116bab6e4c5d531571be1cc5857697bdc2d543bf9624cebe976ed`. This includes the unchanged row
  rendering guards. The current correction changes only the Delete browser fixture; UI callback and rendering inputs
  remain unchanged.
- The complete sidebar, header and session-confirmations specs, with the row Stop confirmation cases, passed 280 cases
  in `2a5b7a15-9b65-44ce-a9f6-0038ff64bf9b`: 140 per engine, at `611b6e192e57`, fingerprint
  `bc35bf03fa0c9a2be65e54e84adf6cae581f0869edfd74a4d47ec6a6824421e6`. Stop controls, callbacks, menu geometry, tested
  CSS and consent paths are unchanged. Upstream old-terminal warnings are inactive in these new-terminal fixtures.
- The original Stop and narrow tunnel-header selection passed 48 browser cases, 24 per engine, in
  `31383bc4-062e-4d74-8ee0-0f02f505deaf`, at `2946e606f99a`, fingerprint
  `f08de9a76f89ce6a579d8275b12f67bcde92ab645c4eb2c41e035cfbfcda4ab5`. Its guard, bulk, notice and header paths remain
  unchanged; the wider 280-case run additionally covers the subsequent callback correction. Both reused browser runs
  have complete strict-policy reports and zero failures, skips, interruptions, unstarted cases or global errors.
- Original tab/consent/API/Podman acceptance passed ten behavioral cases plus setup in
  `b814c010-b24b-4670-9c41-8b6a9fba9656`, at `18f698d46b24`, fingerprint
  `e25a73f4b331c08b3316bc392c0c96cc53216c2a88cc53f5c64eeaa078cc55d3`. Real Podman coverage ran, with no runtime SKIPPED
  marker. Four native-feature confirmation-token cases passed in `b154b933-ce94-4ec3-a4ae-3168859e37b6`, at
  `8945c10a813c`, fingerprint `f3c33921ee23934df2de2102500d27f06d77da34c252c0c5c416341a7ef40ed9`. These unchanged
  backend and mounted-token paths remain covered; native-feature execution on Linux does not establish macOS GUI
  behavior.
- UI all-target Clippy passed in `4ec7b584-ebad-457d-98f1-0f01a25337de`, at `611b6e192e57` and fingerprint
  `bc35bf03fa0c9a2be65e54e84adf6cae581f0869edfd74a4d47ec6a6824421e6`. No Rust callback code changed afterwards.
- `bun run build` passed with internal links valid in `8ac815a8-0c42-40de-9d88-6598f6548a7e`, at `18f698d46b24` and
  fingerprint `e25a73f4b331c08b3316bc392c0c96cc53216c2a88cc53f5c64eeaa078cc55d3`. The Stop docs unit remains
  byte-unchanged; upstream documentation additions affect other paragraphs and do not conflict with its links or
  anchors.

All cited completed runs have complete retained output with EOF, no omitted bytes, no forced cleanup and no cleanup
limitation. Every owned browser stack completed cleanup. Earlier development failures remain retained, including the
row-rendering reproduction, menu-fixture failures and initial type-complexity lint failure corrected in the preceding
round. None is represented as a passed aggregate or erased by later success.

The portable `summarize-test-runs.py --scan-output` inventory covers seven current command records and five explicitly
reused records: eleven successful child commands and the one failed unmodified repetition. Discovery is complete with no
coverage gaps or runtime SKIPPED markers. Generic repetitions supply command outcomes without recorder-validated case
counts; their browser JSON supplies the execution counts above. This selected inventory excludes other historical runs
and is not a count of distinct tests or a flake-rate measurement.

The full workspace and browser batteries, new installer/provisioning runs, release checks and macOS GUI checks were
skipped: this correction changes one browser fixture, and the targeted and reused evidence covers its affected
contracts. No CI run was dispatched; CI is on demand and its browser job is disabled. No live installation was changed.
The frozen documentation-shot scenarios remain deferred by the Decision. No instruction, heading or path move requires
the agent-instructions checker.

## Review gate

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed #3132 and the exact final correction and
reported no actionable finding. #3133's original independent gate remains applicable because its own patch is unchanged.
Earlier consent and wording findings were corrected before the original delivery; no unresolved review finding remains.

Fresh native own-model resume and scope checks passed. The correction uses existing attachment, readiness and listing
helpers without adding global waits or cleanup machinery. Existing approved commit/PR wording was preserved. A separate
documentation pass covered the changed test and its setup, quiet-feed and prompt-drift contracts.
