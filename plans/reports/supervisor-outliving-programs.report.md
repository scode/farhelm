### What this was about

When a session's terminal host is killed outright (SIGKILL), the agent running in it gets a hangup. An agent that
ignores the hangup keeps running, no longer attached to anything. Farhelm then shows the session as ended. Delete
already found and ended such a survivor, but Stop and Restart did not: on a launch outside a systemd scope, where the
survivor carries no session marker (the environment variable Farhelm uses to find a session's processes) that Farhelm
can read (macOS, or Linux without a usable systemd user manager), Stop left it running and Restart could start the new
agent beside it, two agents on one conversation.

You decided this case gets cheap sanity checks only, not strong guarantees: no live-status tracking of survivors, no
periodic process polling, and no change to the "agent has ended" confirmations the desktop sends. That rule is now in
SPEC.md ("Lifecycle operations"), with your wording kept.

What was built: Stop and Restart, when they find no running agent, now also look for the exact agent process the
supervisor recorded when the session's terminal host was set up, as Delete does. If that exact process still runs, it
and what it started are ended; Restart does this before it relaunches. If Farhelm cannot read the process table, Stop
and Restart refuse with nothing changed, as Delete does.

### Things you should know

- Only the agent itself surviving is covered. If the agent died of the hangup and only a process it started ignored it,
  nothing changed: that process is still found only through the session's systemd scope or its session marker. SPEC.md
  says so explicitly, so it reads as a known limit rather than a gap.
- The session still reads as ended while such a survivor runs, by your decision; nothing looks for survivors until the
  user runs Stop, Restart or Delete. A consequence worth knowing: because it reads as ended, Stop and Restart end the
  survivor without asking the "stop a working agent?" question, even if it was busy. That is what SPEC.md's existing
  rule for leftover processes says (consent for an ended agent covers them), and the new Stop test checks it with the
  exact consent the desktop sends.
- The plan asked for no release-note headline. The PR still carries the routine changelog fragment every fix gets.
- The PR removes the TODO entry "Handle programs that outlive their terminal host.".
- The same area is about to change again: the plan `supervisor-stop-pending-launch` (Stop settling an unconfirmed agent
  start) was claimed by another executor while this one ran. Both edit Stop's handling of a session with no live agent;
  the landing monitor's careful rebase should treat that as a likely text conflict.

### Open questions and possible follow-ups

- None needed for this plan. If the grandchild case (agent died, a process it started survived outside any scope with an
  unreadable marker) ever matters in practice, it would need a different mechanism than this one; per your policy that
  is not planned.

### PRs

- #5083 fix: reap a child that outlived its killed host on stop and restart

### Checks

Run now, on the PR's final revision unless noted:

- `cargo fmt --all -- --check`; `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`; the test
  sleep check (0 delays without a reason); `dprint check` on the changed docs;
  `python3 releasing/check-changelog.py format`; `python3 scripts/check-agents-md.py` (0 errors).
- Recorder run `ab756181-5426-487f-94ca-bef37df24cc7`:
  `cargo nextest run -p farhelm --test e2e -E 'test(/outlived_its_killed_host/)'`, the two new tests (Stop with an ended
  agent's consent, and Restart without consent to stop a working agent, each after the host is killed with SIGKILL and
  its hangup-ignoring child is confirmed still running): 2 passed.
- Recorder run `5c5cc106-48f6-4d4e-8c01-131d9d1cc360`: the same two tests with the new lookup of the surviving agent
  disabled on purpose: both failed with the survivor still running, so the tests tell the fix from its absence.

Reused: recorder run `e4b5f443-90e2-494d-b6fb-4851e009b470`, the existing supervisor unit tests and end-to-end tests for
Stop, Restart and Delete of agents in terminal hosts, a restarted supervisor keeping its hosts, and a failed or
interrupted Restart being unwound: 121 passed, no runtime skips. It ran on the first revision; later revisions changed
only an error message prefix, test strength and documentation, so its coverage of Stop, Restart and Delete behavior
still applies.

Skipped: the full workspace battery and the desktop, GUI and installer suites, since the change is confined to the
supervisor's Stop and Restart paths, which the selections above cover.

### Review gate

One fresh-context Opus 5.5 reviewer at high effort. Round 1 found no correctness bugs in the code; it found SPEC.md
overclaiming (it implied a surviving grandchild of a dead agent was covered), a changelog context paragraph that named
the wrong gap, tests that did not use the desktop's actual consent or the exact host state the fix acts on, and wording
nits. All were fixed. Round 2: no remaining findings; its one optional wording suggestion was applied.
