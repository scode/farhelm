### What this was about

On Linux, each agent a session starts runs in a systemd scope of its own, so Farhelm can later make sure every process
the agent started is gone. When you delete a session while its Restart is still starting the replacement agent, the
supervisor tries to end that replacement. If it cannot (the replacement's processes or its terminal will not go away),
it writes down a cleanup it still owes, and retries it each time the supervisor starts.

One case of that retry never finished. If the replacement's terminal was still running, the retry took it for a
replacement still starting up and waited, at every start, forever. The deleted session's agent and terminal could keep
running in the background, and the owed cleanup was never cleared. Nothing else ended them either, because the startup
step that removes orphaned terminals deliberately leaves these to the retry.

You decided the retry should clean up the agent's processes and also end that still-running terminal, after checking it
belongs exactly to the deleted session's replacement, and clear the owed cleanup once both are done.

What was built: exactly that. When the supervisor starts and finds such a terminal still running, it first checks the
terminal is that deleted session's replacement agent (the one the owed cleanup is about). If so, it cleans up the
agent's processes, ends the terminal, and clears the owed cleanup only once both are confirmed; otherwise everything
stays for the next start. A terminal that belongs to anything else, or that it cannot identify, is left running and the
owed cleanup stays. A failed Restart that finds its own replacement still running keeps reporting the Restart as
uncertain, as before. SPEC_impl.md describes it, the changelog entry of the earlier fix that made startup retry this
cleanup (shipped only in the 0.46.0 release candidates) is extended, and the TODO entry is removed.

### Things you should know

- **The terminal is ended even when cleaning up the agent's processes fails.** The owed cleanup stays either way. No
  session is attached to that terminal, so otherwise only this retry, or deleting a later session that reuses the same
  ID, would ever end it. Ending it also hangs up the agent, and the next start then finds the terminal gone and retries
  the process cleanup on its own, which already worked before this change. The alternative was to leave the terminal
  running until the process cleanup succeeds. This was a choice the plan left open.
- **A session that reuses the deleted session's ID is protected.** In a rare sequence, a session restored under the
  deleted session's ID can end up running the very terminal (host) the owed cleanup names. The retry leaves any terminal
  a running session holds alone, and a test now covers exactly that case.
- **One narrow window remains, judged negligible by the review.** A running terminal proves its agent started, not that
  the system had finished setting up the agent's scope. By the time this retry runs, the terminal has outlived a whole
  supervisor run, and ending the terminal hangs up anything still starting, so only an agent that ignores hangups and
  had just started could escape. The review suggested ending the terminal before the process cleanup to close even that.
  It was not done, because the plan's design cleans up first and the gain is very small.
- **The release note.** The plan said none was needed. The earlier change this extends has shipped only in the 0.46.0
  release candidates, so its changelog entry was extended rather than a second entry added.

### Open questions and possible follow-ups

None recommended. If you want the narrow window above closed, the follow-up is to end the terminal before cleaning up
the agent's processes.

### The PRs

- #5133 `fix: end a failed restart's still-running host at startup`: the retry change, its tests, SPEC_impl.md, the
  changelog entry and the TODO entry's removal.

### Checks

Run on the final change (main has since gained only unrelated changes to the desktop app's copy and paste and the helm
client, which touch nothing here):

- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`: clean.
- Through the recorder, the Restart cleanup tests (run 2cd5f69e-8bcb-4d54-ad9e-d69ebe4a7911, 9 passed):

  ```
  cargo nextest run -p farhelm-supervisor --lib -E 'test(/host_restart::tests::/)'
  ```

  They cover a real still-running replacement terminal ended across a supervisor restart with the owed cleanup cleared,
  the terminal ended while the process cleanup fails and the cleanup cleared at the following start, terminals belonging
  to another session or another launch (or unidentifiable) left alone, and a terminal a reusing session still runs left
  alone. Two of the new tests were checked by removing the guard they protect; the first version of the reused-ID test
  still passed without the guard, so it was rewritten to use a real terminal and now fails without it.
- `cargo fmt`, the test-sleep check (no unannotated delays), `dprint check` on the changed Markdown, and the changelog
  format check.

Earlier in the same work, on the code before the review's changes (which touched only tests and comments):

- run 92911178-4562-4be0-8b9b-c89eafc9162a, 38 passed: the inventory's "restarting host agents" selection;
- run f0bff42a-ae59-4497-bd99-c613cae844ae, 14 passed: the "reclaiming orphaned terminal hosts" selection.

Neither printed a skipped result. Both still cover the final change, since the review changed no product code. One
earlier run of the new tests failed (aa51a574-3181-4319-8da5-269f36c5beb2) on a mistake in a new test's own setup, fixed
before the passing runs; it was not a flaky test.

Skipped: the workspace-wide Rust run and the other end-to-end suites, since the change is confined to this startup
retry, whose own tests and the two selections above cover it.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs, and judged sound both the choice the plan
left open (ending the terminal even when the process cleanup fails) and the plan's rule of leaving alone a terminal it
cannot identify. Its findings were:

- the protection for a session reusing the ID had no test;
- the check that the terminal belongs to the same session had no test;
- one sentence in the spec, and comments, overstated that nothing else could end the terminal;
- a comment on the startup time budget left out the terminal shutdown;
- optionally, ending the terminal before the process cleanup, to close the negligible window above; not done (see Things
  you should know).

The four fixes were made, and a verification pass by the same reviewer confirmed them; its remaining wording nit was
fixed too.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous rounds' changes: copying a file's contents and
pasting clipboard text as a file in the terminal, agents asking for a Tailscale host (protocol version 92), more Codex
sessions becoming restartable, and stricter `--model` and `--effort` parsing. The Codex change is in the same supervisor
but only in its Codex-specific code; none of the others touch Restart or the supervisor's start. In the same round,
around this plan, three more landed: remove-legacy-sandbox-sessions-api (an unused helm route removed),
brick-test-inline-welcome (release test instructions) and claude-drift-trust-copy-back (a drift test). None touches this
code.

An independent reviewer and I read the change, looking hardest at whether a supervisor start can now end something it
should not. It ends a leftover terminal only when all of these hold: the Restart that started it can no longer still be
starting it (it belonged to an earlier supervisor run), the terminal identifies itself as exactly that session's agent
at the recorded launch, and no current session is using that terminal. The other startup step that ends leftover
terminals still stays away from every terminal an owed Restart cleanup names, so the two never act on the same one. A
Restart that fails and finds its replacement still running still reports the Restart as uncertain, as before. Nothing to
fix.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing I found. The landing's checks were wider than the report's: every supervisor and end-to-end test ran, together
with the other plans and the previous rounds' changes.

#### Worth knowing

- **The narrow window the report describes stays, and the follow-up to close it is small.** Ending the terminal before
  cleaning up the agent's processes, rather than after, would close it, and nothing else would change, since the owed
  cleanup is already retired only once both steps are confirmed. I would leave it unless such a leftover agent is ever
  seen.
- **One new test could pass without checking what it is meant to.** The test of a leftover terminal tagged for another
  launch checks that the cleanup connected to the test's fake terminal, not that the fake answered the identity
  question. Today it does answer, and the test fails if the tag check is removed, so it works. If the fake ever stopped
  answering, the test would pass without reaching that check; a small optional follow-up would assert that the identity
  question arrived.

#### Checks

Run once, on the round's four plans stacked on main as it stood when the landing started
(remove-legacy-sandbox-sessions-api, restart-startup-scope-cleanup, brick-test-inline-welcome, then
claude-drift-trust-copy-back), so each plan was tested together with the others and with the previous rounds' changes,
which none of the executors' own runs included. The SSH test worker was still unreachable, so the slow suites ran on
boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: clean.
- On three boats, every test of the protocol, helm, supervisor and command-line crates that runs by default, the
  end-to-end suite included (`cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm`,
  built once, split in three and run through the recorder), recorded runs 0aeec1d1-374d-4651-b817-c6196eba36e3,
  79bc70f6-81f9-4677-98ac-5bc0782c1540 and 0e083ecd-f092-4996-ae93-b1866a1bddfd: 4,732 passed, none failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. That covers each executor's own selection and
  more: the helm's Tensorlake launch tests, the Restart cleanup tests, the startup tests of terminal hosts left over
  from earlier runs, and the drift tests' private-configuration tests.
- Locally, the portable brick-test harness
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`), recorded run
  69a626cf-42a8-4fcc-8baa-85e5c53b0de6: 337 passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown, and
  `python3 scripts/check-agents-md.py` (the brick-test runbook is an instruction file): passed.

Reused: nothing. Skipped: the desktop app's and GUI crate's tests, the screenshot comparison and the end-to-end smoke,
since the only change to the app is two comments; the opt-in tests in those crates, among them the real-agent drift
tests, which spend real Claude turns; and a live brick test, which needs the Mac test machine and a release candidate.
