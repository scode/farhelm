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

### Landing

#### What landed with it

This round landed podman-host-verification first (container hosts must prove which machine they are before Farhelm runs
commands there), then this plan. The two touch separate code and separate parts of the specs. Since this plan's base,
main had gained the Replace with action and the launcher's Enter key changes (#5081, #5082), maintainer tooling for
agent records (#5080) and TODO entries (#5085, #5089); none touches the supervisor.

The closest earlier change is the Restart with fix that landed a few rounds ago (#5071, Restart with's edited settings
saved together with the start of the new run), already in this plan's base. An independent reviewer and I checked the
two together: Restart now ends a surviving agent before it starts the new run or saves any edited settings, and if the
process list cannot be read, or the survivor cannot be confirmed gone, Restart refuses before anything else happens. The
surviving process is recognized by its process number and its start time, so a process number the system has since
reused for another program is never mistaken for it.

#### Fixes made while landing

None. The independent review found nothing to fix.

#### Worth knowing

- **A plan in flight will touch the same code.** supervisor-stop-pending-launch, which another executor is working on,
  also changes what Stop does when no agent is running. Its landing will need to keep this plan's step (ending the
  surviving agent) next to its own, not replace it.

- **Nothing ran on macOS**, the main platform this fix is for. The plan's new tests read Linux's process information, so
  they run only on Linux, and all landing checks ran on a Linux machine.

No follow-up TODO is needed; the first point above is for whoever lands supervisor-stop-pending-launch.

#### Checks

Run once for the whole round, on podman-host-verification and supervisor-outliving-programs stacked in that order on
main as it stood when the landing started; what main gained since each plan's base is in each plan's notes. The test
worker is an aarch64 Linux machine with rootless Podman and a systemd user manager:

- On the test worker, `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`:
  clean.
- On the test worker, `cargo nextest run -p farhelm-helm -p farhelm-supervisor`, recorded run
  43812fa9-6ae7-42ce-8ff5-3cd2a81d4035: 3,606 passed, 12 skipped (tests marked ignored). That is every helm and
  supervisor test, wider than either executor's selection.
- On the test worker, the end-to-end tests of host terminals, relaunches, Restart and Restart with, and the end-to-end
  tests that use a stand-in for Podman (`cargo nextest run -p farhelm --test e2e` filtered to `host_terminals`,
  `host_relaunches`, `restart_with_resume`, `scoped_podman`, `containerized_sessions` and every test whose name contains
  `restart`), recorded run 61da3f76-4294-4420-a2d7-6012a29975ec: 106 passed, and none skipped for lack of systemd or
  SSH.
- On the test worker, the real Podman environment scenario
  (`cargo nextest run -p farhelm --test e2e -E 'test(/^podman_environments::/)'`), recorded run
  ee60cfb9-8dbf-46ae-a67c-bcc4a22c245c: passed, and it did not skip. It runs the production Podman provider against a
  real rootless Podman on the worker, so every container setup script and recipe build command went through
  podman-host-verification's new host proof on a real host. It tests that plan, not supervisor-outliving-programs;
  neither executor ran it.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed, and `dprint check` on the implementation spec again after
  podman-host-verification's landing corrected it.

Reused: nothing. Skipped: the desktop app's tests, the screenshot comparison and the desktop app's end-to-end smoke
test, since nothing in the app changed; the real SSH variant of the Podman scenario, which needs a host set up for it.
