# Supervisor sweeps on the timer

## What this was about

Session refreshes repeated the supervisor's periodic work between timer ticks, including refreshes prompted by
status-change hints. Stopped sessions also kept reopening launch and checkout-preparation evidence after the launch had
finished. The stack makes refreshes read the state left by the timer and stops repeating launch reads when their result
cannot change.

## Things you should know

Lists and rename replies still query pane state, so agent exits and tab changes show immediately. They no longer apply
hook reports, read launch evidence, clean up artifacts or record outcomes. Resume availability, launch-error
classification outside a pane-death event and notification resolution can appear about one two-second tick later. A
newly dead owned agent pane is recorded at the pane-death event; the timer handles missed events and wider ownership
cases. Unreadable launch evidence no longer fails a session list: the timer logs and retries it.

A launch error already found stays visible even when the supervisor cannot record it or its write fails. This includes
reload before the timer starts. A recorded error's details take precedence over the temporary copy.

Successful no-failure launch and preparation checks stop only after a durable terminal outcome and proof that the launch
cannot write more: its own pane was seen dead, or a recorded host reboot interrupted it. A pane merely missing in the
same boot keeps being checked, because an empty pane query can be transient. Read failures, errors not yet persisted and
supervisors that are not allowed to save outcomes (for example, a replacement still waiting for ownership) do not settle
these reads. Renaming shares the result; relaunching or restarting the supervisor reads again. Error cleanup stops only
after preserving the evidence needed to answer a repeated create request without launching twice, and successfully
removing the leftover launch files; failures retry.

The remaining periodic reads have changing inputs. Resume availability still reloads saved conversation state because an
agent report can arrive late, or Restart can discover that the saved conversation is no longer usable, even after the
agent exits. Notification resolution checks only relevant unresolved notifications and usable Resume state. The
report-folder drain and statement caching from the preceding plan are unchanged. There is no new schema, protocol,
global pass lock or background task. The covered TODO entry is removed.

The workspace run exposed two existing test-fixture defects, corrected in the preparation PR: reboot/resume inspected a
truncated terminal argv witness, and the known restart-confirmation flake launched through a nonexistent shim instead of
a lasting agent. Their narrow reproductions and corrections are recorded in FLAKES.md. The restart-confirmation TODO and
exclusion are removed. A Codex test also needed explicit report and reply-refresh boundaries after lists stopped
sweeping; that migration regression was fixed in the same PR.

The optional CPU comparison was skipped because concurrent builds and a test sweep loaded the shared machine. This
report claims reduced repeated work from the code and regression proofs, not a measured CPU improvement.

## Open questions and possible follow-ups

No decision blocks this delivery. The source audit identified a pre-existing risk outside this plan: if the terminal
service briefly reports no panes while a new checkout is still being prepared, the supervisor can classify that
preparation as having stopped and show the session as Error. This stack keeps absent-pane reads retryable but does not
change that classification. The risk was not reproduced during this run, and whether later preparation completion
repairs the displayed result has not been established.

## PRs

- [#1758](https://github.com/scode/farhelm/pull/1758/changes): shared observation phase and deterministic test
  reconciliation, preserving production behavior.
- [#1761](https://github.com/scode/farhelm/pull/1761/changes): timer-owned sweeps, cached list/rename replies and
  pane-death outcome recording.
- [#1766](https://github.com/scode/farhelm/pull/1766/changes): final launch-read and cleanup settlement, with TODO
  removal.

## Checks run, reused and skipped

The tip workspace command
`python3 scripts/record-test-run.py --runner nextest --kind development --selection '<workspace Rust targets>' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run --workspace --exclude farhelm-desktop`
ran in an owned Linux container limited to four CPUs and 32 GiB. Run `1c7d2fa4-5c88-4696-8e8b-6f13ceac682c` executed
3,390 tests: 3,387 passed and three failed, with 36 runner skips. Its complete console and JUnit remain retained. The
recorded source was `fbdd8ad4a0ef8b86fe7ec5086f42ed56c945f4ae` plus the final PR3 settlement changes in a dirty tree.
The three failures reproduced individually on unchanged source: Codex attribution in
`d0b071da-fe90-4ad3-ab88-31103dbe2c7d`, structured Claude reconstruction in `308f7324-73d1-4e12-a5fd-1ff4591f2d66`, and
restart confirmation in `d2b8c62d-0882-4efc-a251-a5968ac1fabd`.

After fixture corrections, run `0d7b4d8b-871b-4684-8aaa-6af53b1a7ea6` passed structured and unstructured reboot/resume
and restart confirmation, but Codex failed at a later reply-refresh boundary. That retained failure was corrected with
explicit timer reconciliation after Restart's durable withdrawal; exact run `1c4b457d-807d-4542-b1dd-7fa1e7e15468`
passed. These corrected selections used the workspace command with exact name filters, four slots and zero retries. No
runtime substrate skips occurred in the selected correction proofs. The only production edit after the broad run
collapsed an equivalent short-circuit cleanup condition for Clippy; its 3,387 unaffected passes are reused, alongside
the corrected cases, rather than claiming a new full-suite pass.

Fifteen tests printed runtime SKIPPED messages inside JUnit-passing cases: one required a second below-floor tmux, two
lacked passwordless SSH, and twelve lacked a usable systemd user manager for scope/provisioning coverage. They do not
prove those substrates ran. The prior real scoped-wrapper test below remains applicable to unchanged launch and scope
behavior. All sandbox execution used recorded tmux 3.7c and nextest 0.9.143; the container did not supply systemd or SSH
coverage.

Final `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` passed in the same
owned container. The initial all-target lint found one collapsible cleanup condition; the equivalent condition was
folded into #1766 and verified by its reviewer before both final lints passed.

PR1 targeted recorded runs `b68feaa0-3c47-4fec-953c-9638a5141399` (53 tests) and `75ad7cf8-1401-4d37-8f78-4df188f1af25`
(four tests) passed, including real scoped-wrapper coverage. PR2's final review-fix run
`7a55225b-2b4a-406f-806b-b1e1ac6db915` passed six tests, and the exact degraded-reload correction passed in
`56de78da-0fbf-43c5-9d6b-d8cb8fe90271`. PR3's final focused run `295fe617-1479-4a24-ac33-22bc0a50edd7` passed nine tests
covering settlement, retries, cleanup, preparation, reload and run replacement. These runs used pinned nextest and tmux,
four slots and zero retries. Their coverage is reused because later changes were the separately verified test-fixture
corrections, equivalent cleanup-condition collapse, and documentation; no covered behavior changed. No runtime substrate
skips occurred in these selected proofs.

The earlier PR2 run `f3cc262d-1f4d-4d7f-9843-0fa7e990e714` passed 61 tests and failed degraded reload before its cache
fix. Its failed evidence remains retained; the exact correction was verified separately. Compilation-only failures
`ebf20593-cfc7-4b86-bebf-0e39688ebde1` and `a76ac3f5-1227-41b7-ba3e-a03f1e5238b4` are also retained and contribute no
execution coverage. These were same-session development failures, not latent flakes.

Formatting, changed-document dprint, changelog format and the isolated test-delay checker passed; the latter inspected
276 delays with zero missing rationales. Targeted supervisor Clippy passed for PR1 and final PR2. Browser, JavaScript,
desktop, installer, provisioning and website execution were skipped because this stack changes supervisor scheduling and
launch observation, with no UI or deployment change and no failing browser timing evidence. Doctests were skipped
because no executable examples changed. The full upstream diffs through `669c79e2` added only plan files, claims and
TODO metadata, with no executable or binding-spec interaction; no runtime rerun was needed for those additions. No
release or deployment was requested.

## Review gate outcome

Each code PR passed the required fresh-context gpt-6.1-sol review at high effort; no review swarm ran. PR1's two
test-migration findings were fixed and verified. PR2's five findings were fixed and verified: reload error retention,
cached rename behavior, a recording list's no-capture proof, durable-error precedence and stale ownership/Restart
documentation. PR3 had no actionable findings, including verification of the final equivalent cleanup-condition edit.
The subsequent fixture corrections also passed a fresh gpt-6.1-sol high-effort review, including its final boundary
follow-up. Commit and PR wording passed fresh cold reads on gpt-6.1-sol at medium effort. Review agents ran no builds or
VCS operations; recorded execution and lint results are the executor's evidence. Native usage counters and exact runtime
model reporting were unavailable.

### Landing

Landed on 2026-10-10 (UTC) as three squash commits on main, in order: #1758 (supervisor observation separated from
sampling), #1761 (session refreshes read the supervisor's state) and #1766 (settled launch evidence is no longer
reread). They landed after this round's two UI plans, approval-card-layout and resizable-sidebar, on top of which the
stack rebased without conflicts. Main had otherwise gained only plan bookkeeping and TODO entries.

#### Review before merging

A separate reviewer that had not worked on this round's plans read the stack against main and the two UI plans. It found
no interaction with them, no protocol or database change, and no test seam reaching the shipped binary, and it confirmed
the report's account of what lists, rename and stop still do immediately and what now waits for the next two-second
tick.

#### A fix made while landing

The reviewer found one behavior change the plan had not asked for. When the supervisor could not tell whether a stopped
session's launch file was still there (a failed file-system check, not a missing file), the stack made the whole
observation of that session fail: the exit was never recorded and an error was logged every two seconds for as long as
the check kept failing. Before the stack, that inconclusive check simply did not classify the launch as a failed start,
and the exit was recorded from the pane as usual. The landing restored that, while still not letting an inconclusive
check stop future reads (the plan's rule that a read failure never settles them), in #1766 before it merged. No test
exercises an inconclusive check; the supervisor's unit tests passed with the change (below).

The landing also corrected comments in #1761 that still described lists recording outcomes or computing a fresh restart
offer, and one test's description: the pane-death test's check for a status-change hint cannot tell the hint the commit
sends from the one every pane-death wake sends on entry, so its description now says the stored outcome is what proves
the commit.

#### Not changed while landing

The reviewer found that some stopped sessions still have their launch evidence reread every tick, the cost the plan set
out to remove. Reads stop only once the session's own pane was seen dead, or a host reboot interrupted it. An agent that
exited on its own before a reboot keeps its exited row with no pane to see, so each tick still opens its launch status
file and queries its checkout preparation, for as long as the row exists. The same holds within one boot for an exited
session whose tmux session was closed or whose tmux server died. The plan's reason for treating an interrupted launch as
settled (the old launch can no longer write anything) applies equally to an exit from a previous boot, but acting on it
needs to know which boot each row's launch belongs to, which the supervisor does not track per row. That is a possible
follow-up for the maintainer, not a regression: before the stack, these sessions were reread on every tick and every
list.

Smaller notes left as they are: the changelog fragment for the timer-owned sweeps is filed as `fixed`, though it mainly
announces a change in timing (`changed` may read better at curation), and #1758 also carries the fix for the
restart-confirmation flake, with its TODO entry and its exclusion from the flake sweep removed together, as the
instructions require.

#### Checks

- Run now, on all three plans stacked together and including the landing's fix:
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` (the shipped binary, with
  test seams off), and the supervisor's unit tests in full through the recorder with pinned tmux 3.7c, four slots and no
  retries (run `75969832`, 1059 of 1059); `cargo fmt --all -- --check`.
- The two UI plans' browser runs (see their notes) cover the session list and approvals the helm reads from this
  supervisor, on the UI side; they ran without this stack, which the reviewer found does not change anything those plans
  read.
- Reused from the executor: the workspace test run and its corrected selections, the end-to-end cases it lists, and the
  changelog and document checks. The landing's changes are one inconclusive-check branch in the supervisor's
  observation, comments and one test's description; the end-to-end cases do not exercise an inconclusive check, so
  nothing they cover changed.

Nothing in the report above was made untrue by the landing.
