### What this was about

The missing-conversation warning could appear before an agent was due to report, gave no useful next step, and
disappeared from consideration after the supervisor restarted. On the Mac, reopening or updating the app could leave a
session without that warning even though Restart could not resume its conversation.

This stack implements the agreed behavior: an Enter answering a recognized dialog does not start the warning clock; the
warning names the agent and gives a useful next step; a launch that received the hook remains checked after supervisor
restart. The delay is unchanged: 65 seconds after the first qualifying submitted line, if Farhelm still has no
conversation identity.

The warning uses the same reporting timing as Restart's explanation:

- Claude normally reports a few seconds after it starts.
- Goose normally reports as soon as it starts.
- Codex normally reports once you submit your first prompt.

Each warning says Restart cannot resume until the report arrives and gives the agreed advice: if you launched that agent
with a custom command, check that it passes on `{farhelm_args}`; otherwise, send feedback from the help (?) menu.

### Things you should know

After supervisor restart, the clock starts again at the next qualifying Enter. It does not recover a deadline from
before the restart. Launches stored before this change remain unchecked because their hook status is unknown. Existing
notification text stays as stored; a recurrence uses the current wording. Repeated checks and supervisor restarts
preserve the one notification for an unresolved problem in that launch.

The new database version is 29. Upgrades preserve sessions, conversations and notifications; older supervisors refuse
the upgraded database on downgrade, an accepted scope decision.

The existing screen reading is taken before input delivery. A stale dialog reading can delay the clock to a later Enter;
the input path does not take an extra screen capture. Current Claude screen classification treats its background-agent
wait announcement as working, so that announcement does not suppress the clock.

When a failed new-session launch leaves a session behind, the current supervisor keeps its missing-conversation warning
off, even if Farhelm cannot establish whether an agent actually started. It also attempts to clear the saved hook policy
so later supervisor restarts keep that session unchecked. Such a session can remain unwarned until a successful relaunch
establishes a new policy.

If a relaunch has an uncertain outcome, the current supervisor likewise keeps that launch's warning off. The next
supervisor restart can restore checking from the policy saved before the launch, provided that write succeeded and
recorded an injected hook. Retained sessions whose launch policy is already known recover that saved value normally.

Saving or clearing this diagnostic policy is best effort. A storage failure does not turn an otherwise healthy launch
into a refusal, but a later supervisor may then recover an old policy or no hook evidence. This preserves the existing
launch behavior rather than making diagnostics a launch prerequisite.

Both covered TODO entries are removed. No plan files are changed by the PRs.

### Open questions and possible follow-ups

None required. The next-Enter clock reset, unchecked older launches, downgrade refusal, and ambiguous-relaunch gap
follow the agreed scope.

### PRs

- [#1722](https://github.com/scode/farhelm/pull/1722/changes): exclude recognized-dialog Enter from the warning clock.
- [#1723](https://github.com/scode/farhelm/pull/1723/changes): give agent-specific reporting timing and the agreed next
  step.
- [#1725](https://github.com/scode/farhelm/pull/1725/changes): preserve the launch hook policy across supervisor
  restarts.

All PRs remain drafts in one linear stack; landing belongs to the monitor.

### Checks run, reused and skipped

Ran these on the final PR3 tree:

- `cargo fmt --all -- --check`, changed-file `dprint check`, and `python3 releasing/check-changelog.py format`: passed;
  cover formatting and all three changelog fragments.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm --bins -- -D warnings`: passed. Both workspace configurations cover the supervisor with and
  without its test seams. An initial targeted lint attempt caught one missing fixture field, corrected before runtime
  validation.
- Isolated `python -B scripts/check-test-sleeps.py`: 269 delays inspected, zero missing reasons. No delays were added.
- Recorded
  `cargo nextest run -p farhelm-supervisor --lib -E '<capture, hook policy, migration parity, notification and v17 upgrade selection>'`:
  20 passed, run `ae42cc27-a581-4a52-9fae-0bc890646dd1`. Covers the new restart/uniqueness proof, generation and retry
  resets, real hooked create, old-row defaults and the populated v17 upgrade.
- Recorded
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor --lib -E '<all proto tests; supervisor migrations, relaunch and retained-refusal selection>'`:
  176 passed, run `568e445d-52db-4d0b-9ab1-43c3bf35eff2`. Covers timing contracts, changed historical fixture rewinds
  and launch lifecycle interactions.
- Recorded `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::)'`: 19 passed, run
  `044feec6-f486-4b40-9c68-a03655a4b6b1`; fixture-build setup also passed. Covers reported and historical conversations
  surviving supervisor restart, stale reports and disabled-hook launches.

All recorded runs used the pinned nextest and tmux, four global slots and zero retries. Their output and JUnit reports
are complete; selected tests emitted no runtime substrate-skipping messages. Runner skips are tests outside the
selections.

Reused PR1's 11 capture passes (`42924c4b-523b-48b5-a1c6-81b02b99f242`, commit
`07476d060dff72704b9b16a97936663301f9f318`) and PR2's 13 capture/notification passes
(`a3c916b7-9dd6-4094-ba0e-55c86fb12a59`, commit `14da2b52a9c9a3293519374c1eabec35f2bf464f`) for their dialog and text
contracts. PR3 leaves the input route and text construction unchanged and additionally exercises the integrated capture
behavior.

Main advanced from the stack base `1f237fc8` only by the other desktop-encoding plan's queue/report delivery,
`adf78aba`. Its complete diff was inspected: no product code or specification changed, so no rebase or further runtime
run was needed. The three PR3 recorded runs cover its final tree, committed as `dd97543b` without subsequent source
edits.

Skipped unrelated supervisor modules, the full workspace and e2e batteries, browser/desktop runtime, installer,
provisioning and website checks: the concrete risks are covered by the selected storage/lifecycle tests and hook
integration module. Notification rendering and desktop code did not change. No hosted CI, deployment or release was
requested.

### Review gate

Each code PR received its required fresh-context review, requested on gpt-6.1-sol high. PR1 exposed an authoritative
specification conflict: it still said dialog Enter counted and that screen classification affected only Restart
confirmations. The specification now describes the agreed warning-clock exception while keeping terminal input delivery
unconditional. The reviewer confirmed the correction. PR2 and PR3 had no findings. Commit/PR wording passed fresh cold
reads. Implementation stayed local; the harness did not expose native model reporting or usage counters, and the private
session evidence records that gap.
