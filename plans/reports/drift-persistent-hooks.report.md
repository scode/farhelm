# Report: a drift check for the hook files Farhelm leaves in Grok's and Antigravity's configuration

## What this was about

Grok and Antigravity find Farhelm's conversation hooks in files in their own configuration, which Farhelm writes once
the user agrees to set them up. Those files stay behind, so every later run of that agent executes them, including runs
that have nothing to do with Farhelm. Nothing checked against the real CLIs that the files are found inside Farhelm, or
that a plain run of the agent outside Farhelm still works with them installed, without hook errors on screen.

When the plan was written you decided: give this its own drift category rather than folding it into the capture timing
one; outside Farhelm, go as far as one short turn, so the hooks for starting, prompting and finishing all fire; share
code as much as possible; and, if a part got disproportionately complicated, leave it out with a follow-up entry. No
part needed that fallback.

What the PR does: the new `persistent_hooks` drift category runs, for Grok and Antigravity, two sessions against one
private copy of the vendor's configuration (the same mechanism the other drift categories use, so your real files are
never written):

- Inside Farhelm: a launch with consent has to install exactly the files Farhelm's hook setup says it writes for that
  agent, and one short turn has to bring an accepted conversation report, which shows that the CLI found and ran them.
- Outside Farhelm: the same CLI starts the way you would start it yourself (for Grok with one extra flag, see below), on
  the same copy, with every one of Farhelm's environment variables removed. Before anything is typed, the test checks
  that the session's process is the CLI with exactly the expected arguments, that it carries none of Farhelm's
  variables, and that it reads the private copy. The CLI then has to finish two short turns and keep running for a few
  seconds, with no hook output on screen.

Some shared drift code moved so this category could reuse it (the check that no hook output reached the screen, the
short one-word turn, the ordinary launch choice, and the session creation and cleanup). The drift docs describe the
category, and the TODO entry is removed.

## Things you should know

- **The outside half shows that the hooks stay silent and harmless, not that they ran.** With Farhelm's variables gone,
  the hook has nowhere to report and writes nothing, so nothing observable says it fired. The inside half, on the same
  installed files, is the evidence that the CLI reads them. The second outside turn is there so that a finishing hook
  that hung, failed or made the CLI quit after the first turn would show.
- **Grok's outside run passes `--no-leader`, which you would not normally pass.** By default Grok shares one background
  "leader" process, reached through a socket in its configuration directory. The private copy links every entry of your
  real Grok directory, so without the flag the test could attach to a leader you run, which never read the installed
  files and may have another environment. As a result the case covers Grok's own private backend running the hooks, and
  not the default shared-leader mode, where the leader runs the hooks in whatever environment it started with. What that
  leaves open for a user: a shared leader that was first started from inside a Farhelm session would still carry that
  session's Farhelm variables, and the hooks it runs for a plain Grok run elsewhere could then report to that session.
  Farhelm's own Grok launches always pass `--no-leader`, so this concerns only Grok runs outside Farhelm.
- **The outside run needs a systemd user manager, and reports COULD NOT RUN without one.** Removing Farhelm's variables
  also removes the marker Delete uses as a fallback to find processes that left the session's process tree. Without the
  scope, such a leftover process could keep running after Delete, and keep writing through the private copy's links into
  your real vendor configuration while the test restores it. So the test requires the CLI to run in the session's own
  systemd scope, which does not depend on that marker, before it types anything.
- **Only Farhelm's own hook messages and the `{}` answer Farhelm's hook gives Antigravity count as leaked hook output.**
  If a vendor reports a failed hook in its own words, without quoting Farhelm's command, the check would not notice.
- **The inside half assumes the accepted report came from the files Farhelm installed.** The private copy links the rest
  of the vendor's real configuration, so an older hand-made Farhelm hook elsewhere in your real Grok or Antigravity
  configuration would produce the same report. I consider this unlikely; the existing base drift case makes the same
  assumption.

## Open questions and possible follow-ups

- If the default shared-leader mode matters to you for Grok, it would need its own case: a leader started inside the
  test's private copy and scope, and a check that the hooks it runs carry none of Farhelm's environment. I would not do
  it unless you have seen Grok used that way next to Farhelm: it means driving Grok's leader lifecycle in the test,
  which is more machinery than the rest of this category.
- If you know how Grok or Antigravity word a failed hook, those words could be added to the leaked-output check.

## PRs

- #5173 test: check persistent hook files inside and outside Farhelm (the new category, the shared drift helpers, the
  docs, the TODO removal).

## Checks

Run during this plan, through the test recorder unless noted:

- Run `2f5740b4`, `cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`: 70 passed, on the
  final commit after rebasing onto the landed conversation rotation work. These are the drift module's ordinary tests,
  among them the new ones: the category covers exactly the agents with persistent hooks; the outside command, run by a
  real Bash with Farhelm's variables set, leaves none of them and points at the private copy; the environment check
  refuses Farhelm's variables or another configuration, naming variables but never their values; the outside CLI's
  arguments; the exact-argument match; and reading the session's systemd scope from the process's cgroup. Earlier runs
  `5c28fc87` and `e3b1ef18` were the same selection before the rebase.
- Runs `ff77e0f9` and `c3b9f7f4`, `python3 -B scripts/test-run-harness-drift.py`: the drift runner's own tests, OK.
- Live drift runs, `python3 -B scripts/run-harness-drift.py --no-upgrade --category persistent_hooks` (the runner
  records each case), on the final code before the rebase:
  - Grok 1.0.92: PASSED (`ca311582`; earlier `dacbc57e`, before the review fixes).
  - Antigravity 1.5.18: PASSED (`9bbe2e7c`; earlier `5b4cadd0`, before the review fixes).
- Not through the recorder, as they run no tests: `cargo clippy -p farhelm --tests` (its only warning is one already on
  main, in the helm crate: a settings method used only by tests), `cargo fmt --all -- --check`, the test sleep check,
  and `dprint check` on the docs and TODO.md: clean.

Reused: the live runs ran before the rebase. What landed in between (the conversation rotation drift category) adds a
separate test module and docs paragraph and uses none of the code this PR moved; the ordinary tests above ran after the
rebase.

Skipped: every non-drift suite, since only drift test code, its docs and TODO.md changed.

## Review gate

The PR had one fresh-context Opus 5.5 review at high effort: no blocker, three should-fixes and seven nits, all handled.

- Should-fixes, all applied: require the session's systemd scope before the outside run types anything (above); state
  the uncovered shared-leader mode for Grok instead of claiming the hooks do not depend on it; and replace the fixed
  wait after the outside turn, as the evidence that the finishing hook let the CLI carry on, with a second turn.
- Nits: five applied (the argument test compares literal values, each agent's arguments are spelled in one place, the
  wait for the CLI has its own short limit, the arguments must match exactly, and wording); two are stated as
  limitations instead (the vendor's own wording for a failed hook, and the assumption about hand-made hooks).
- These fixes were not reviewed again; runs `2f5740b4`, `ca311582` and `9bbe2e7c` ran after them.
