# Report: drift tests for switching conversations inside an agent

## What this was about

Most agents let the user start over inside the same running session: `/clear` or `/new` start an empty conversation, and
Pi and OMP also offer `/fork` (a copy that branches off) and `/resume` (go back to an earlier one). Farhelm has no other
way to see that switch than the agent's own report. If a vendor's report stops arriving after one of these, Restart
quietly reopens the conversation the user had walked away from, or none at all. The only automated checks of this were
three standalone tests for Claude `/clear` and Codex `/new` and `/clear`, which sat outside the drift runner and so
never ran when the agents were upgraded.

You asked for these checks to move into the drift runner as one shared suite, for the three standalone tests to go, and
for live coverage of Muse (`/clear`, `/new`), Pi and OMP (`/new`, `/fork`, `/resume`), Grok (`/new`) and Antigravity
(`/clear`), each proving that Restart resumes the conversation the agent switched to. You also asked for Codex `/clear`
to be run through Farhelm's real hook so that its "reported but not yet resumable" state and its confirmation at the end
of the next turn are tested. You allowed leaving out Pi's or OMP's `/fork` or `/resume` if driving their on-screen
pickers got too complicated. That was not needed: both are covered.

One part could not be done as asked: the Codex `/clear` "not yet resumable, then confirmed" check. Codex 0.172.0 no
longer goes through that state at all (see "Things you should know"). I delivered rather than blocked, because no
Farhelm behavior is weakened: Farhelm still handles that state, the supervisor's own tests still cover it, and a live
test can only check what the current vendor does. The question for you is only whether that is acceptable, and the
follow-ups below say what could be added.

### What changed

There is a new `conversation_rotation` category in the drift runner
(`python3 -B scripts/run-harness-drift.py --category conversation_rotation`). It covers Claude, Codex, Muse, Pi, OMP,
Grok and Antigravity, and the docs page for the drift tests describes it.

Every case follows the same steps:

1. Start the agent.
2. Ask it to remember a code word.
3. Run the command.
4. Ask it to remember a new word in the new conversation.
5. Require Farhelm to have captured a different conversation.
6. Stop the session and Restart it.
7. Require the restarted agent to recall the new word.

An agent with several commands runs them one after another in one session. That keeps even Pi and OMP, with six turns
and three Restarts, well inside the runner's 12 minutes per agent: they took under a minute and a half.

The operation-specific checks:

- **Fork.** A fork also holds its parent's conversation, so it gets a word the parent never saw. Recalling that word
  proves Restart opened the fork, not the parent. The test also waits for the agent's own "forked" notice before typing,
  so the new word cannot land in the parent by accident.
- **Resume.** Farhelm's captured conversation must move back to exactly the earlier one, and the restarted agent must
  recall the earlier word.
- **Timing between the command and the next prompt.** Only what the old tests established is checked: Claude reports the
  new conversation on `/clear` itself, and Codex keeps the old conversation as Restart's target after `/new` until the
  next prompt. Every other agent only has to report at that prompt.

The three standalone tests and the helpers only they used are gone.

## Things you should know

- **Codex `/clear` no longer works the way the old test expected.** The removed test checked that Codex reports a
  cleared conversation right away, before its saved file exists. In that case Farhelm marks it as not yet resumable, and
  the end of the next turn confirms it. Codex 0.172.0 does not do this. It reports nothing until the next prompt, and
  then the conversation is already saved, so Restart works at once. Farhelm handles both shapes (SPEC.md describes the
  first as something that can happen, not something that must), so the Codex `/clear` case now checks no timing before
  the prompt and passes with either. The supervisor's own tests still cover the "not yet saved" path. That path is no
  longer exercised against a real Codex unless a later Codex goes back to reporting early.
- **One check was lost with the old Codex test.** It also confirmed that Codex's `/clear` leaves the old conversation's
  saved file intact. Nothing checks that now.
- **A possible Muse status gap, not investigated.** Muse 1.4.25 kept showing its "Double checking" work indicator after
  its reply and its end-of-turn report, and refused `/clear` as "can't run while a turn is active", while Farhelm
  already showed the session as idle. The test waits for the indicator to go away. Whether Farhelm should show such a
  session as working is a separate question.
- **OMP's resume picker check depends on your OMP icon setting.** The test recognises OMP's highlighted row by the icon
  your OMP configuration draws. With a different symbol setting the case would time out and fail, never pass falsely.
- **The base drift case may be flaky for Claude Haiku.** While building this, Claude Haiku once answered the base
  category's spelled-out recall prompt literally ("R-E-C-A-L-L-…") after a correct resume (a rotation case that used the
  same prompt; recorded run d587992d-3c92-4100-820c-f829ee9247ff, kept). The rotation category uses a plainer recall
  prompt. The base case still uses the old one, and I left it alone as outside this plan.
- **Side effects of exploring the agents by hand.** Reading each agent's command menus and pickers before writing the
  test answered a few workspace-trust questions, and spent a handful of turns on Pi and OMP. That left trust entries for
  scratch folders in Claude's, Antigravity's and Muse's own settings, and a few small Pi and OMP saved sessions filed
  under those scratch folders. The folders themselves are gone.
- **Goose has no rotation case.** Nobody has looked at what Goose's reporter does when its conversation changes inside
  one process. The test's harness table records that as undecided.

## Open questions and possible follow-ups

- **Muse's work indicator vs. Farhelm's idle status.** See above. I recommend a quick look the next time Muse status
  detection is touched; it is not urgent.
- **Codex `/clear`'s early report and saved file.** If you want a live check of the "not yet resumable" state again, it
  has to wait for a Codex that reports `/clear` before saving; until then the supervisor's tests are the coverage. A
  live check that Codex's `/clear` leaves the old conversation's saved file intact (which the removed test also did)
  could be added to the Codex case cheaply. I recommend adding that one if you value it; the other needs nothing now.
- **The base category's recall prompt for Claude.** Switching the base case to the plainer prompt the rotation and named
  conversation categories use would remove the possible flake. I recommend doing it if `base/claude` ever fails that
  way.

## PRs

- #5170: test: add a conversation rotation drift category (the shared driver, all seven agents' `/clear` and `/new`,
  removing the three standalone tests).
- #5171: test: drive pi and omp fork and resume in the rotation drift (Pi and OMP `/fork` and `/resume`, the docs page,
  removing the TODO entry).

## Checks

All live runs go through the drift runner with `--no-upgrade`
(`python3 -B scripts/run-harness-drift.py --no-upgrade --category conversation_rotation`), so the installed CLIs other
sessions on this machine use were not upgraded.

Run now, on the final stack after its rebase onto `main`:

- **Codex** live: Codex 0.172.0, both `/new` and `/clear`, PASSED (recorded run 674d6d94-8b14-4c1d-9079-6a9220ab42b6).
  The rebase brought in the CLI parsing changes (#5163, #5165), and every agent's Farhelm hook runs that `farhelm`
  binary, so one live case was run again on the rebased code rather than assumed.
- The drift suite's ordinary (non-live) tests, among them the new checks of the agent table, the command and picker
  matching and the capture checks (`cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::/)'`): 58 of
  58 passed (recorded run f4d956e5-9256-4ad2-a0be-1286f273f957).
- The drift runner's self-test (`python3 -B scripts/test-run-harness-drift.py`): 31 passed (recorded run
  3fd30f24-9f25-4753-9c5d-b21927a0c354).

- `cargo clippy -p farhelm --tests`, `cargo fmt --check`, `dprint check` on the docs page and TODO.md, and the
  test-sleep check: clean, except for an unrelated unused-function warning already on `main` in the helm's notification
  settings code.

Reused:

- **Pi and OMP live verdicts,** from the final code of #5171 just before the rebase, all three commands: Pi 1.6.1, 54
  seconds, PASSED (recorded run 647c325c-4bc0-4029-a15a-535287b4a38b); OMP 19.2.1, 83 seconds, PASSED (recorded run
  8ec7a325-973b-43f3-be83-4a2accee44b1). Nothing in the rotation code changed after them; the rebase reason below covers
  them too.

- **Claude, Muse, Grok and Antigravity live verdicts,** from #5170 before its review fixes (reasons below):
  - Claude 2.1.332: PASSED (recorded run 75e9eb62-0dee-4d9e-a74e-0b9325efab82);
  - Muse 1.4.25: PASSED (recorded run 0778fbe0-c268-44c4-ab8f-658bef91f9c0);
  - Grok 1.0.92: PASSED (recorded run d5e470a4-842c-4a9f-b685-e9c7875b031a);
  - Antigravity 1.5.18: PASSED (recorded run 49915e08-ca89-4052-8e18-e2a4f94a4860).

  Why they still apply:
  - #5170's review fixes changed only Codex's `/clear` timing, added a liveness check to Codex's wait, and moved the
    check on the agent's report label after Restart (the same check, later). None changes what these four agents are
    made to do or what passes for them.
  - #5171 restructures the shared steps (each command now says whether it starts, forks or resumes a conversation, and
    Restart's recall prompt became a parameter), and its review fixes added the fork notice wait, Pi's composer check
    and clearer timeouts. For a plain `/clear` or `/new`, which is all these four run, the steps, the prompts and the
    order of checks are the same as in #5170; the new code runs only for a fork or a resume.
  - The rebase onto `main` (this reason covers Pi and OMP as well): the hosts table and file hover changes do not touch
    what these cases run. The CLI parsing changes do reach the `farhelm` binary the hooks call, which is why Codex was
    run again on the rebased code above; it passed. Those changes left the hook's own arguments as they were.

Skipped:

- The other drift categories and the rest of the test batteries. The change adds a test category, removes three
  standalone tests and edits comments in the hook code, so nothing outside the drift suite runs differently.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort, with the plan's charter and the test-authoring checklist. I
did not write the code myself: same-model sub-agents implemented it and the fixes.

- **#5170:** one round. No high findings. The medium: the replacement Codex `/clear` check required the old conversation
  to stay until the prompt, which neither SPEC.md nor the old tests established. It would have failed if Codex went back
  to reporting early, so it was dropped. Five lows were fixed:
  - a comment overstating what moved;
  - a stale helper;
  - the report-label check, which now runs after Restart so a harmless vendor rename cannot hide a Restart failure;
  - a unit test that did not reach the comparison it claimed;
  - the missing wait after a command, now a comment explaining why waiting would prove nothing. Six nits were fixed or
    explained.
- **#5171:** one round. No high findings. The medium: after OMP's `/fork` the test typed without waiting for the fork to
  exist, so a word landing in the parent would have made the check meaningless. It now waits for the agent's "forked"
  notice. Five lows were fixed:
  - Pi's composer clear is checked;
  - a stray Enter after the picker closes is guarded against;
  - an over-broad check was dropped;
  - the timeout messages are clearer;
  - a docs sentence was corrected. Three nits were fixed.
