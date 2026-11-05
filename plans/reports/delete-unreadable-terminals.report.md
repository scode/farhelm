## Since the last review

Added [#3040](https://github.com/scode/farhelm/pull/3040/changes), the newly authorized surviving-tab visibility and
Delete-warning fix, between the existing drafts [#3024](https://github.com/scode/farhelm/pull/3024/changes) and
[#3032](https://github.com/scode/farhelm/pull/3032/changes). Those two drafts were rebased with their reviewed changes
preserved; no PR was dropped. The earlier round stopped at the middle fix's size gate. This round finishes that
authorized work and the complete three-PR stack remains draft.

## What this was about

Delete could close work that its confirmation said was no longer running. A tab could stop responding between the
guard's two status reads and escape both checks. Separately, old terminal tabs could disappear from view after an
upgrade moved their agent to a new terminal and that agent terminal was later lost, even though Delete still closed
those tabs. A failed automatic reconnect could also discard the identity needed to clean up an agent's or tab's
foreground program, particularly on hosts without systemd containment such as macOS.

Triage chose to fix all three. The first and third fixes were already reviewed drafts when this round resumed. The
maintainer then authorized the complete larger middle fix, including the session list, Rename and Restart replies, and
Delete's live-work count. This round inserted it between the existing drafts.

## Things you should know

Delete now checks for unreadable tabs after counting live tabs, so a tab becoming unreadable during the check requires
renewed consent. Surviving old tabs are found using the session's existing identity even without an agent terminal;
successful lists and Rename/Restart replies keep showing them, and a Delete approved for nothing alive refuses while
they run. A failed reconnect retains the program's previously pinned process number and start time for both agent and
tab; Delete validates that identity before using it for cleanup.

The larger middle fix includes the previously contradictory query-skipping regression and the comments that described
it. Its final size is 183 added-plus-removed lines of code, tests and documentation, excluding ledger, queue removal and
changelog. The authorized proposal was about 164 lines; the 19-line growth covers final documentation and compile
corrections. The first and third changes remain 75 and 100 counted lines. No persistent metadata or terminal-host
protocol changed.

The checks ran on Linux. They do not establish a macOS execution result. The failed-reconnect regression deliberately
reaches a later host-ending refusal after proving the foreground child was cleaned up; it does not claim successful
complete session deletion in that stand-in fixture.

## Open questions and possible follow-ups

The separate lookup used to open and close old tabs still requires an agent terminal, so these surviving tabs can be
visible and counted while requiring an agent Restart before they can be opened or explicitly closed. The background reap
of old tabs and the separate enumeration of dead-tab cleanup scopes also retain that dependency. These limitations
predate the changes and were outside the approved visibility/consent repair; the prior scope assessment and this round's
independent reviewer agreed they remain follow-ups.

## The PRs

- [#3024](https://github.com/scode/farhelm/pull/3024/changes): refuse a nothing-alive Delete when a tab becomes
  unreadable during its final check.
- [#3040](https://github.com/scode/farhelm/pull/3040/changes): keep surviving old tabs visible and included in Delete's
  live-work count after agent-terminal loss.
- [#3032](https://github.com/scode/farhelm/pull/3032/changes): preserve foreground-program cleanup across failed agent
  and tab reconnects.

## Checks run, reused and skipped

The current combined run passed all 8 selected tests, with 1,747 selected out: recorder
`5c92f061-4506-4b6f-b75d-25dda59b88a4`. It used pinned nextest and required pinned tmux, four global slots and zero
retries, through the recorder in development mode:

`cargo nextest run -p farhelm-supervisor --lib -E 'test(the_delete_guard_sees_a_lone_tab) | test(only_an_empty_listing_skips_failed_pane_queries) | test(delete_refuses_a_tab_that_becomes_unreadable) | test(delete_roots_children_after_failed) | test(list_sessions_)'`

This covers surviving-tab discovery in the list and Restart's tab snapshot, Delete's exact refusal, the list/Rename
query-error contract, both earlier regressions, and existing listing cap, ordering and record-size checks. The retained
output and JUnit report are complete, with no runtime substrate `SKIPPED` notices. Tested source:
`8712ba2c6c7d2bef0b97b7ffb8e40757d9366a62`, fingerprint
`13b82d66b822c7700846493d750f45f2c49995668764ffae6752158949aae48d`.

`cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed. Dprint on
changed ledger/index Markdown and `python3 releasing/check-changelog.py format` passed. The isolated pinned
interpreter's `python -B scripts/check-test-sleeps.py` inspected 412 delays with zero missing reasons.
`python3 scripts/check-agents-md.py` reported zero errors and three pre-existing reference warnings.

The earlier expected pre-fix regression failure remains recorded as `734cce6b-b94b-4642-b57c-7a849890d736`. This round's
initial attempted selection failed compilation before executing a test: `15223cb2-4781-4ddb-bc59-fbab91b5e47e`. An
explicit collection type and an unused import were corrected; neither failure is reported as a passing run or a latent
flake. The initial clippy attempt failed for the same two source issues before the corrected clippy passed.

Reused broader evidence from the first round: recorder `e8906ac7-3945-4da3-806e-693de0c94df9` passed 8 Delete/settlement
cases on the dirty first-fix source over `2cbfdc77` (fingerprint
`0cee14bb291aee7af9d4ccac2d150fb44af2e29330a8b6ab0884dac624affaec`); `87768755-36cc-493e-a7ff-fc173a73f879` passed 30
failed-recovery/Delete/tab-listing/root-adapter cases on the dirty third-fix source over `c2d873a9` (fingerprint
`88781622a947b036b72546b794551b6cebb4d94a2ed7d82566e910a8871cd77b`). The final first/third PR patches are identical to
their originally reviewed drafts after careful rebase and middle-fix insertion. The new combined run directly checks
their interaction with the middle fix. Earlier one-case greens `92ce0b42-9f87-46cb-9772-b871ece44ccd` and
`873e47ce-3fa9-43cd-b054-c966c18e7a50` remain recorded, but the newer combined run supersedes them for those
regressions.

The stack was carefully rebased onto `9949406d`. All upstream changes from the old base were read: unrelated CSS,
planning, reports and bookkeeping, with no supervisor, specification, dependency or fixture interaction. Each reviewed
patch was preserved. Later ledger-URL amendments change no tested behavior, so runtime, lint, formatting and delay-check
results remain applicable; document checks were refreshed for the final ledger text.

Browser, desktop, installer, provisioning, full Rust battery and CI runs are skipped: the changed behavior is in
supervisor discovery and consent decisions, covered by owned real-tmux and in-process fixtures; no client, schema, host
protocol or deployment behavior changed. No live installation was changed.

## Review gate's outcome

The required single fresh gpt-6.1-sol reviewer at high effort passed the middle fix after the two compile/lint
corrections. It found no remaining correctness issue and confirmed the adjacent tab-interaction/reap limitations are
pre-existing follow-ups. The first and third drafts passed their required Sol/high reviews in the earlier round. Their
reviewed changes were preserved after insertion. The resume check passed, and the final commit wording passed its cold
read. No required finding was rejected or left unresolved.

### Landing

Landed with no fixes at landing. Delete no longer ends terminal work its confirmation said was not running: a tab that
stops answering during the check now needs fresh consent, old tabs that outlived their agent's terminal stay listed and
are counted, and a failed reconnect keeps what Delete needs to clean up the program.

- #3024: Delete refuses when a tab becomes unreadable during its final check.
- #3040: surviving old tabs appear in the session list and in the Rename and Restart replies, and Delete counts them
  (the larger fix you allowed on 2026-11-05; 183 counted lines against the roughly 164 proposed).
- #3032: a failed reconnect keeps the program's process identity, so Delete can still end it.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the printed-link fix and its documentation (#3018, #3019, #3021) and the v0.35.0
release-notes illustrations (#3038); none touches the supervisor. In this round it landed third, after
unlikely-races-nov4 and durability-revocations and before triage-misc-nov4. This plan changes no specification text.
unlikely-races-nov4 records accepted terminal-host races that touch the same recovery code; the fresh-context review
found nothing in either that the other now contradicts, and none of the other plans changes the supervisor's listing,
Delete or recovery code.

#### Things you should know

- Surviving old tabs are listed and counted, but opening or closing one is still refused with the "no terminal" message
  until the agent is restarted, as the executor's report says. That limit predates the plan, but users can now see such
  tabs and run into it. Two texts no longer match the code: a comment in the supervisor's terminal handling says that
  without an agent terminal a tab lookup finds nothing, which the list now contradicts, and SPEC_impl.md's "Tabs in
  hosts" says every tab lookup merges both kinds of tab, which opening and closing do not. Correcting them, or extending
  the lookup, is a follow-up.
- If tmux fails in some way other than reporting no server, the session list and Rename now fail, even when every
  session is waiting for a restart. That is deliberate (the corrected test requires it), but SPEC_impl.md does not yet
  say so.
- #3032 keeps the previous host process for an agent's stand-in after a failed reconnect as well as for tabs, but
  SPEC_impl.md's "Across supervisor restarts" still describes it only for tabs, and its sentence that such a stand-in
  gives no sweep root is now out of date.
- When a reconnect finds the terminal host gone entirely, the program's identity is still dropped, so a program that
  ignored the hangup of a crashed host is ended only by the marker sweep and its cgroup scope. This predates the plan
  and is a candidate follow-up.

#### Checks

This round landed four plans, stacked in queue order on main `ab98921b`: unlikely-races-nov4, durability-revocations,
delete-unreadable-terminals and triage-misc-nov4. The checks ran once, on the whole stack. While they ran, two
browser-test fixes landed on main: #3042 (the browser suite's setup now waits for each stack's startup session) and
#3043 (a quick-launch test drains host replies before cleanup), each with its flake bookkeeping. No plan in the round
touches a browser test, TODO.md, FLAKES.md or the flake exclusion list, so the runs below still describe what landed.

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the workspace debug build and the UI's release web build: all passed.
- The whole Rust workspace with the pinned tmux, `cargo nextest run --workspace --exclude farhelm-desktop`, run
  `f07cd604-0561-4944-82ec-d22ceee500d1`: 5604 passed, 70 skipped (ignored by design), 1 failed. The failure is
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a recorded flake that sweeps already exclude.
  The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `ffb56e46-5d09-4c86-957e-82c35d390db5`: all 858 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `f02122ed-7f9b-4c1d-bbe2-b54636673478`: all 1047 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `f684cd58-3a29-47db-83c0-9ed452c461c1`, `dffb760d-dbf5-475f-aead-c23f786551bd`,
  `57cccb94-ac0c-47c4-b617-ab5f3bd8a485`, `d80fd8f4-1646-4bae-8eb5-ad7c1b832f03`,
  `b618b816-61d8-4e2f-9c6c-5e17f5e6320e`, `c2b42d58-9c6c-4a82-92c2-b275559a0cfe`,
  `4b38a57a-3ad1-4220-916f-cd8c59370431`, `89515c90-b088-427f-8460-c2ce26e92697`,
  `3ac6a8a3-606c-4d49-9a47-ed284c005719`, `1422993b-e223-4dd2-855f-68ebbb953be9`: 2996 passed, 18 skipped by the specs'
  own design, 6 failed. Then the same suite with every stack's terminals in terminal hosts, since the supervisor's
  terminal listing and recovery changed (`FARHELM_E2E_SUITE_TERMINALS=host`; this run covers only the specs listed for
  host terminals in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `dd43611c-53e6-440e-9b4d-b067b5f8ecaf`,
  `c9f23f58-6955-4127-be9d-23b7d6247d3e`, `c4d61d6b-3b1a-46bf-9705-dbac5d651a28`,
  `ed839b79-675c-4b1d-8c41-e857f88ddc96`, `2e48a313-928b-45e7-9cb5-6cf8df812cb0`,
  `dd213786-0132-4825-b5f4-a3f3923c6659`, `182db21b-bab2-49f2-9b28-7896032965b4`,
  `1cc9cda2-2faa-4a2e-b5a4-a9a458636715`, `7d969e49-d11a-47bb-ab2d-03c4413f218e`,
  `5ef8c251-2019-4f99-8173-78b65f89eb86`: 2081 passed, 75 skipped by the specs' own design, 4 failed. Of the ten
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `f684cd58`,
    `d80fd8f4`, `ed839b79`, `1cc9cda2`), because the Add host form's "SSH" and "Tensorlake sandbox" choices have no
    hover text. Those choices came with the Tensorlake sandbox hosts (#2973), on main before every plan's base, and the
    previous round saw the same four failures; no plan in this round touches the form. It is a break on main and a
    follow-up there.
  - Recorded flakes, each once: the feedback dialog's disabled Send color on WebKit, in "the dialog refuses blank and
    over-long messages without sending" (run `c2b42d58`); "trash header and global confirmation remain reachable at
    width 360" (Chromium, run `ed839b79`); and the launcher's "Runs in YOLO mode" step on WebKit, in
    "create-intent-key-rebinds-on-retarget: a moved host is a new intent" (run `ed839b79`).
  - Three single failures in the ordinary suite, each of which then passed three re-runs on the same build, also on
    sandboxes (the run ids below are the re-runs). "Agent managed checkout approval and keyed recovery" (WebKit; the
    test timed out at 60 seconds, then its cleanup failed because the page had closed; failed in `1422993b`) exercises
    the keyed retry of an agent's checkout request, which durability-revocations changed to key by the requesting host
    entry, so it was re-run on both engines: WebKit `0b58da4b-d9fe-4e02-9f25-38056211276a` and Chromium
    `e0863d93-7ea6-4601-a7f4-75fa98203b5c`, all six passed. "Settings keeps the pending Claude hint choice clickable
    through a held read" (WebKit; a 60-second timeout waiting for an intercepted save to finish, like the recorded
    pending-save flakes; failed in `c2b42d58`, re-run `b0de5934-d629-4547-886b-76e85605d844`). "Expanded setup failure
    closes the menu below it" (Chromium; the host actions panel did not finish measuring within 5 seconds; failed in
    `4b38a57a`, re-run `3a9117e4-f2ad-4000-95ed-18f8aa21a056`). Each is a single failure, not recorded as a flake.
- Not run: the doctests (no doc examples changed), the UI's JavaScript tests (no JavaScript changed), the grid
  component's browser suite (no grid code changed), the desktop smoke tests, the installer tests, the development
  screenshot review (no shot changed) and the native Mac app (not runnable on Linux), so the macOS-specific paths
  (delete-unreadable-terminals' cleanup on hosts without systemd containment) ran only through their Linux tests.

The executor's report calls the PRs drafts and names the main it was built on; what landed is its commits rebased onto
current main, with their content unchanged; only the surrounding lines of the review queue's index differ, because the
plans below it removed neighboring entries. Its checks ran on Linux only, as it says; so did the landing's. Nothing else
in it became untrue at landing.
