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
