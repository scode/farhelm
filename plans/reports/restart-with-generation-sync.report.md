### What this was about

Restart with lets you change a session's launch settings while resuming its conversation. One of those settings, for a
custom agent, is whether the agent may report which conversation it is in. A custom agent typically reports right as it
starts. Restart with used to start and show the restarted agent first and only then save the edited settings, so that
first report could be judged by the old setting. If you had just turned reporting on, the report was refused and the
conversation you had just named was never recorded, so Restart could not resume it later. If you had just turned
reporting off, the report could still be accepted. The restart's reply also described what Restart could resume using
the old settings (for example, saying the session had no conversation reporting right after you turned it on). This was
a deferred follow-up from triage; you made no other decisions about it.

The fix saves the edited settings at the very start of the restart, before the new agent can start, and builds the
restarted session (what lists show and what the reply says) from them. A restart with new settings that fails before
anything starts puts the old settings back. One that fails once its agent has started, or may have, keeps the new
settings, because that agent is running with them.

### Things you should know

- While a restart is in progress, a session list still shows the session as it was just before the restart, with the old
  settings and the old description of what Restart can resume, until the restart finishes. This predates the change and
  is deliberate (it keeps the session from disappearing from the list mid-restart); nothing can restart the session
  again during that time. The plan's wording ("any concurrent session listing") could be read to include that moment; I
  left it alone.
- A restart that fails partway, in a way where Farhelm cannot tell whether the new agent started, now keeps the new
  settings. Before, it kept the old ones even though an agent might be running with the new ones.
- The implementation notes in SPEC_impl.md ("Restart-with backend wire and persistence") described the old order (save
  after the agent starts), so the PR rewrites that paragraph to match. SPEC.md needed no change.
- No changelog entry was planned for this plan, but repository rules ask every fix to add a draft entry, so the PR adds
  one (custom-agent reporting shipped in v0.45.0). Drop it at curation if it is too minor.

### Open questions and possible follow-ups

- If a restart with new settings fails after its replacement agent briefly ran, and that agent reported a conversation
  in that moment, Farhelm keeps that conversation recorded even though the old settings come back (after a restart that
  turned reporting on, the restored setting would have refused that report). Plain restarts already keep a report made
  in that moment. The window is tiny, so I recommend leaving it; a follow-up could discard reports made by an agent
  whose restart then failed.

### PRs

- #5071 fix: apply restart-with settings before its agent can report

### Checks

Run now, all through the test recorder, on the final code unless noted:

- Supervisor unit tests for restart, relaunch and rename
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(restart) | test(relaunch) | test(rename) | test(renamed)'`):
  116 passed (run 7638ec59). It includes the new test that holds a restart open and reports from inside it, in both
  directions. A deliberate probe that withheld the settings from the restart's first step made that test fail as it
  should (run df1f63e9); the probe was reverted. That test passed again after its last tightening (run dc724479).
- Reused from before the last change: end-to-end Restart and Restart with tests in terminal hosts
  (`cargo nextest run -p farhelm --test e2e -E 'test(/^restart_with_resume::/) | test(/^host_relaunches::/) | test(/^command_shell::restart/)'`):
  43 passed, none reported a skipped substrate (run 3770963b). This ran before the last change, which touched only that
  unit test and code comments.
- `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test
  delay checker (306 delays inspected, none lacking a rationale), `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`: all clean.

Skipped: the full workspace battery and the GUI, desktop and installer checks. The change is confined to the
supervisor's restart path and its store, which the selections above cover; nothing in the GUI, helm or wire protocol
changed. main moved only in `TODO.md` and `plans/` since the PR's base, so no rebase was needed and the runs still
apply.

### Review gate

Two rounds, each a fresh Opus 5.5 reviewer at high effort with the plan's general charter. The first found no bug in the
change itself. It asked for the SPEC_impl.md paragraph and two code comments to be updated, and pointed out that the
end-to-end report checks could not reliably tell the old behavior from the new. All three were addressed, the last by
adding the unit test above. It also suggested a small simplification, which was taken. The second round found no
problems of substance and two low-severity items (a test assertion that could pass for the wrong reason, and two
overstated comments), both fixed. The gate passed after the second round.
