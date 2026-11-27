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
