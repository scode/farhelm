# Report: restarting a Claude session before its first prompt

## What this was about

Restart was offered for a Claude Code session a few seconds after it started, because Claude reports its conversation id
as soon as it starts. But Claude saves the conversation only once the first prompt is submitted. So a Restart before
that ran `claude --resume <id>` for a conversation that did not exist yet. Claude exited with "No conversation found
with session ID", and the session was left with an empty terminal. You hit it on two hosts, both times on a fresh
session, one of them right after a Replace.

You decided:

- At Restart, check the conversation file Claude's report named. Resume it when the file exists, and start fresh when it
  does not, since there is nothing to lose. You chose this over making Restart unavailable until the first turn, and
  asked that nothing poll or watch for the file: there is one check, when Restart runs.
- This narrows SPEC.md's confirmed rule that Restart never starts fresh, and is recorded there as your decision of
  2026-11-26.
- The fresh start runs the session's own start command. A missing file after real turns is treated the same. Nothing
  tells the user that Restart started fresh.
- Add a real-Claude test that restarts before the first prompt, and run it once.

Three PRs build it.

### What changed for someone using Farhelm

- **Restart before the first prompt now works.** A Claude session restarted before you sent it anything starts Claude
  again in the same directory, with the same choices, and the new Claude reports its own conversation as usual. Restart
  stays available from launch.
- **Everything else resumes as before.** A session whose conversation file exists resumes it, including one whose first
  prompt was submitted but whose first turn has not finished: Claude writes the file about a second after the prompt is
  submitted. Sessions captured before this change resume exactly as before, without the check, so one restarted before
  its first prompt still fails the old way. A Restart that names a conversation (Restart with a chosen conversation)
  runs that conversation, whatever was saved.
- **Restart with** a changed command or changed choices starts the replacement's command when nothing was saved.
- **Custom agents** can no longer report a conversation ID starting with `claude:`, because Farhelm now stores Claude's
  conversations under that prefix. The custom-agent contract page and its changelog say so.
- **Docs.** SPEC.md's Restart section, its resume promise and its paragraph on checking conversation files describe the
  exception. The Claude page in the docs explains it, with the caveat below.

## Things you should know

- **How Farhelm tells "nothing saved" apart.** Claude's start report already names the file the conversation will be
  saved to. Farhelm now keeps that path beside the conversation id. When Restart runs, it prepares both the resume and
  the start command, stops the old Claude, and only then looks for the file: once the old Claude is gone, nothing can
  still create it, so a prompt saved at the last moment is resumed. Only "the file does not exist" starts fresh. Any
  other answer (no permission to look, a directory where the file should be) resumes, so the check can never be why a
  Restart that worked before now starts over. The file is never read.
- **A different view of the home directory breaks the check.** If a wrapper runs Claude with its own view of `~/.claude`
  (a separately mounted directory, say) while Farhelm sees the host's, every Restart starts fresh, even when the
  conversation was saved. This is on the Claude docs page.
- **Claude's own cleanup.** A conversation file Claude's retention cleanup removed now makes Restart start fresh
  silently, where Claude used to print an error. That matches your decision.
- **The old conversation id stays after a fresh start.** I kept it rather than clearing it. The next Restart checks the
  same missing file and starts fresh again, so the old id does no harm, while clearing it would leave Restart
  unavailable whenever the new Claude cannot report. Normally the new Claude reports its own conversation within seconds
  and replaces it.
- **A start command Farhelm would refuse.** If the session's start command fails the checks Farhelm runs before a
  launch, Restart does not refuse: it resumes without checking, as it did before, and logs a warning. This can only
  happen for a session stored by an older build, or after a launch rule was tightened. No test covers it, because the
  normal paths cannot create such a session.
- **No notice, by design.** Nothing in the app says that Restart started fresh. The supervisor's log has one line.
- **The real-Claude run.** The capture-timing drift case for Claude now presses Restart at Claude's prompt before
  anything is typed. It requires the restarted Claude to have been started without `--resume`, to show its prompt and to
  report a conversation of its own. It spends no extra vendor turn. I ran it with `--no-upgrade`, so it used the
  installed Claude Code 2.1.332 rather than upgrading the CLI that other sessions on this machine use.
- **Not changed:** the upgrade of the drift runner's agents, and anything for agents other than Claude.

## Open questions and possible follow-ups

- **Should Farhelm say when a Restart started fresh?** You decided on no notice. If it ever confuses someone, a short
  line in the session's terminal or a notice would be a small change. I recommend leaving it as decided.
- **Clear the old conversation id after a fresh start?** Covered above. I recommend keeping it.

## PRs

- #5131: fix: keep the transcript Claude reports with its conversation.
- #5137: fix: restart Claude fresh when it saved no conversation.
- #5138: test: restart real Claude before its first prompt (also removes the TODO entry).

## Checks

Run now:

- On the stack rebased onto main, the tests of what changed
  (`cargo nextest run -p farhelm-supervisor -p farhelm-proto -p farhelm --lib --bins --test e2e`):
  - the supervisor's agent-kind, Claude capture and capture-pass tests and the new Restart tests;
  - the conversation-prefix and custom-agent contract tests;
  - the end-to-end Claude capture and Restart modules (`hook_identity::`, `restart_with_resume::`, including a new test
    that starts fresh, then resumes once the fake Claude saved);
  - the capture-timing drift module's ordinary tests.

  210 of 210 passed (recorded run e403c7f2-1fdd-4b03-872b-493e313729b0).
- On the same final code, the "restarting host agents" selection from `.agents/validation.md`, every supervisor test
  matching `restart`, the end-to-end `host_relaunches::` and the new Restart tests: 164 of 164 passed (recorded run
  2cd43ced-ca8c-43c3-b119-3d21f8785bd8).
- `cargo clippy` with all targets for the supervisor, proto, CLI and fixtures crates, `cargo fmt --check`, the
  test-sleep check, `dprint check` on the changed Markdown, `python3 releasing/check-changelog.py format`, the
  agent-instructions check and the docs website build: clean.

Reused:

- The real-Claude drift case
  (`python3 -B scripts/run-harness-drift.py --no-upgrade --category capture_timing_hooks --harness claude`), on Claude
  Code 2.1.332: passed on each revision of #5138, last in recorded run 099df32d-ca54-43dc-930a-21b5b6b6f0eb. After that
  run only a shared helper and a comment changed in #5138, and the rebase brought nothing that touches the Claude
  restart path, so it still applies.

Skipped:

- The rest of the workspace tests and the desktop, GUI and installer batteries. The change is in the supervisor's Claude
  capture and Restart path, which the tests above cover, including the full Restart selection.

## Review gate

Each PR had fresh-context Opus 5.5 reviews at high effort with the plan's charter and the test-authoring checklist:

- **#5131:** two rounds. Round 1 found one high problem: two manually run real-Claude tests compared the stored value,
  now a `claude:` value, with the plain id in the hook log, and would have failed. It also found one medium (no test of
  the pass that copies a stored capture into the session list when the report arrives before the session is listed), two
  lows and three nits. All were fixed. Round 2 found three nits; two were fixed, and one about comments describing the
  next PR's check was left, since the stack lands together.
- **#5137:** three rounds. Round 1 found no bugs. It found seven lows and three nits, all fixed. The two that changed
  behavior: a start command failing its checks now resumes instead of refusing the Restart, and the "was anything saved"
  rule became a check that names no vendor. Round 2 found two lows and three nits, all wording, all fixed. Round 3 found
  one low (a wrong comment in the end-to-end test) and two nits, all fixed.
- **#5138:** three rounds. Round 1 found one medium: after the Restart, the case accepted the first Claude's report as
  the new one. Now it requires a new conversation id. It also found one low and two nits, all fixed. Round 2 found two
  lows: the case now checks the restarted process's own command line rather than relying on Claude exiting on a bad
  resume, and its timeout names what was missing. It also found two nits, all fixed. Round 3 found two nits, both fixed.
