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

### Landing

Landed after two returns to its executor, with fixes the landing made over its last two attempts. Stop now also closes a
session's terminal tabs and their processes, asking first when tabs are showing, and the row above the terminal has a
Stop button.

- #3132: the change, its tests and the protocol change.
- #3133: the guide pages and the TODO entry's removal.

#### What landed since the stack was based, and whether it interacted

The first attempt, earlier on 2026-11-07, sent the plan back because it slowed the session list (an unchanged row
re-rendered on every refresh) and broke two sidebar menu tests; the second sent it back because a Delete test it had
rewritten failed 4 of 12 repeated runs. Between them main landed recipe tunnels over the host's connection
(macos-tunnel-exit), which also changed the protocol, path links split across rows (cut-path-links), three features for
sessions still in old terminals, and the v0.37.1 release changes. This delivery is based on main `ac521332`, and the
executor fixed the second return's failures in the test itself: it now waits for the agent to be running before stopping
it, and for the tab it opens to be listed. A fresh-context review checked each attempt against what had landed and, this
time, against the second return's instructions: all of them are met, and nothing that landed interacts with this plan
beyond the protocol number below.

#### Fixes made at landing

- The protocol version: this plan and macos-tunnel-exit each moved it from 68 to 69, and macos-tunnel-exit landed first,
  so this plan takes 70; its pinning test and its SPEC_impl.md sentence say 70 too (in #3132, made at the second attempt
  and kept by the executor).
- At the second attempt, also kept: an end-to-end test of Delete ending the processes behind a session's tabs had been
  changed to Stop first, which left nothing testing Delete itself; its Delete assertions are back. SPEC.md's list of
  questions the Skip session confirmations setting skips gained Stop, and a supervisor comment now says tabs stay open
  across Restart only (all in #3132).
- At this attempt: SPEC.md's sentence on what that setting's help names gained Stop, which the help text in this
  delivery already starts with (in #3132); and two sub-bullets of the near-term "Podman rigor" TODO entry that this plan
  resolves (Stop descriptions not saying tabs end, and the wrapper guide promising Stop keeps tabs) are removed (in
  #3133).

#### Things you should know

- Every existing host shows **Needs update** until its supervisor is updated. The executor's report says the version
  goes from 68 to 69; it goes from 69 to 70.
- A possible follow-up the review found: a Stop pressed while an agent is still starting is recorded as a launch that
  never started, with a message blaming the host's systemd user manager. That behavior predates this plan (main's own
  test says Stop in that window "can correctly produce a startup error"), but it is what the second return's failures
  were, and the message misleads. Whether such a Stop should read as an ordinary stop is a decision for you.
- The scripts that capture the docs website's screenshots still describe the old Stop: one counts the buttons above the
  terminal without the new Stop, and one says a stopped session's terminal stays without mentioning that tabs close. The
  screenshot refresh procedure updates such scripts first, so they are left to the next refresh.

#### Checks

This attempt's checks ran on the plan stacked on main `ac521332`, after a clean build:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the lint of the UI's browser build,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `b34a4ac0-de44-41a6-b125-86737f0e781a`:
  all 530 passed. The workspace's Rust tests, all but the desktop app's own crate,
  `cargo nextest run --workspace --exclude farhelm-desktop` (with rootless Podman and its offline image present), run
  `741a9238-2169-4d79-90e1-39cff6409d18`: all 5791 passed, 69 skipped by their own design. The desktop-build tests,
  `cargo nextest run -p farhelm-ui --features desktop`, run `985a00ee-7a9c-4cbb-8590-d2efeee39b94`: all 1097 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each ran its share of the suite's spec files through the test recorder,
  `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, built locally; the sandboxes run Node 24.16 rather
  than the development machine's 24.20), runs `28af4f0a-840b-4b6e-b8ac-457a22f83e0f`,
  `675fc1c5-e6b8-46cd-b2f1-8c8802ba2660`, `d32c9429-91cb-4c10-93d4-79d31bc5e93d`,
  `8e380ca4-a6c6-4d7e-9d90-02f3e53623ff`, `4ac230f6-f29a-4fa1-a430-a8b22c8ee32f`,
  `29358e1b-1c83-4089-a665-b589f9ada509`, `17c423fa-2ae8-4678-b563-25506666993d`,
  `8bb6caea-c219-46e0-9344-efec5ae54838`, `edf78d5b-08e3-4941-bae2-42d2497ba5e8`,
  `d3ab5629-d014-4352-a8ab-3a45d4cd5c61`: 3068 passed, 14 skipped by the specs' own design, 4 failed. Two are recorded
  flakes on WebKit, the multi-host create-intent tests (run `8e380ca4`). One is "changed-host advice while following:
  destination" (Chromium, run `29358e1b`) with the recorded update-progress-without-a-step-name signature, on a sibling
  not yet listed; the landing monitor records it in a separate change after this landing. The last, "same-agent search
  and chips preserve custom permission drafts byte for byte" (WebKit, run `28af4f0a`), found a model option missing from
  the session launcher, a part of the app this plan does not touch; it passed three repeats on each engine, runs
  `8038ece7-ff6e-4e22-8c4b-c08c3a77badd` (WebKit) and `9f7f4c60-653b-45f4-9591-e51e24906079` (Chromium), and with one
  failure it is not recorded as a flake. Then the suite's old-terminal set (the tests that only apply to sessions still
  in old terminals) on four sandboxes with the same build, runs `8d834b61-9375-49cb-862b-a2611dd92e85`,
  `ac2e20e1-6ac2-4de2-a994-c9f4540b5568`, `e7ab17e0-168a-4b2e-a0dc-acb8a9431fe2`,
  `4953ffd3-b48a-4f30-bf58-46ebb9010b02`: all passed. Then the Delete test the second return was about,
  `e2e/tests/delete-precondition.spec.ts` (its four tests), ten times on each engine (`--repeat 10`), runs
  `3365d677-3f2b-4e50-a077-e0201cd9b4c0` and `fdf0ea99-749c-48ce-bb3c-d09a40342caf`: all 80 passed.
- After this attempt's fixes: `dprint check` on SPEC.md and TODO.md passed; the fixes change one SPEC.md sentence and
  one TODO.md entry, so the runs above still apply.
- Not run: the doctests (no doc examples changed), the grid terminal suite (no terminal code changed), the desktop smoke
  tests and the installer tests (no desktop shell, packaging or installer code changed), the docs website preview (the
  build above passed; no page was viewed), and the native Mac app (not runnable on Linux).

Nothing else in the executor's report became untrue at landing.
