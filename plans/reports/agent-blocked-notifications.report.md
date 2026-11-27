### What this was about

You noticed that **Send test** delivered an ntfy alert while a real session sat waiting for you and no alert ever came,
and you wanted two things from ntfy alerts: reliable "agent is waiting for an answer" alerts, and a new "agent is
blocked" alert, so an unattended agent that cannot finish what you asked does not just stop silently for hours. You
settled:

- separate switches under a master one;
- `farhelm agent notify` to send the alert, which also lands in the desktop's Notifications bell;
- instructions injected into the agents Farhelm already gives instructions to, with your exact wording;
- an optional "what counts as blocked" in your own words, with Farhelm's wording shown while you have not written yours;
- the supported-agent lists in Settings derived from the code, not hand-written.

What was built, as three stacked PRs that each work on their own:

- **Waiting alerts.** Settings → Notifications has an **Enable ntfy alerts** master switch and an **Agent is waiting for
  an answer** switch, which names the agents whose questions Farhelm can actually see on screen (Claude Code and Codex:
  questions and approvals; Muse Code and Antigravity: only the folder-trust question; other agents never show as
  waiting, so they never send this alert). Alerts now come about 3 seconds after an agent starts waiting instead of 10.
  A line beside **Send test** says when alerts are saved off, when waiting alerts are off, or when the form has unsaved
  changes, because a test arriving only proves the server and topic work.
- **`farhelm agent notify "<what is blocked: why>"`.** It records a notice in the Notifications bell naming the session
  and host with the agent's reason, and, with the new **Agent is blocked** switch on, sends an ntfy alert ("<title> on
  <host> is blocked: <reason>"). It asks nobody, works from any kind of host, and each session keeps one such notice (a
  new report replaces it as unread; reading it resolves it).
- **The instruction.** With **Agent is blocked** on, sessions started or restarted after you save get your exact
  wording, right after the existing line that tells agents about `farhelm agent instructions`. That covers Claude Code,
  Codex, Muse Code, Goose, Pi and OMP, the agents that already get that line (the switch names them). Under the switch,
  **What counts as blocked** is empty by default with Farhelm's own wording shown under it; typing your own replaces it,
  and **Reset to default** goes back. Setting `FARHELM_AGENT_INSTRUCTIONS=off` in the environment a host's supervisor
  runs in, which already withheld the `farhelm agent instructions` line, now withholds this instruction too.

SPEC.md, SPEC_impl.md, the docs pages "Session list" and "Approve agent requests", and the changelog drafts describe it
(one draft for the waiting-alert fix, and one for the blocked alerts that the third PR extends).

### Things you should know

The first point follows the plan; the next three were decided during execution and may be worth revisiting; the rest are
for your information.

- **The alert you missed was not explained.** Nothing in this work reproduced it, and the likeliest causes were not
  established: the alerts or the master switch saved off at the time, an agent whose questions Farhelm cannot see (it
  never shows as waiting), or the desktop app's window being closed (its helm sends the alerts, and nothing tells you
  when alerts stop for that reason). The Settings line beside **Send test** and the waiting switch's list of agents now
  make the first two visible. The plan's other change, making the helm keep no record of waits while alerts are off, has
  no effect you can see: it used to record them and drop their alerts. If a session is already showing a question when
  you save with alerts on, you still get no alert for that question, only for its next one, as before and as after a
  reconnect.
- **A blocked report does not wait for the session's other pending change.** Farhelm handles one change to a session at
  a time, and the plan treated a blocked report as one of those changes. But then a report would wait, and fail after
  about 35 seconds, whenever the same session had a request still waiting on an approval card, which is a common way for
  an agent to be blocked. So a report skips that queue. The cost is small: a report sent while the same session is being
  deleted can leave a notice for a session that is gone, which reading clears; and a report repeated after a lost reply
  only replaces the same notice and adds at most one alert under the five-alerts-a-minute cap that all ntfy alerts
  share.
- **The three PRs are cut by feature, not by layer.** The plan suggested four PRs (protocol, helm, command line and
  supervisor, desktop). A new command or notice kind has to be handled everywhere in the same PR to build, so each PR
  here carries one working feature end to end. The protocol version between the helm and hosts moves twice (to 93 for
  the command, 94 for the instruction, after main took 92 for agent host add's Tailscale option), which costs nothing
  extra when they release together.
- **A host keeps at most 500 blocked notices**, as many as its session list shows; at the limit, a new report removes
  that host's oldest blocked notice. The session a notice names is reported by that host, which the helm does not trust,
  so a misbehaving host could otherwise fill the bell.
- **One kind of start goes without the instruction**: a session created from a GitHub checkout whose creation was
  interrupted and that Farhelm finished later, from what it had recorded. It still gets the `farhelm agent instructions`
  line, and gets the blocked instruction at its next Restart. The request that finishes such a creation carries no
  instructions, and changing it was left out to keep this change small. SPEC.md says so.
- **An older desktop saving these settings keeps the new choices as they are**: a save that does not mention a switch or
  **What counts as blocked** leaves the stored value alone.
- **Goose gets a new environment variable**, `FARHELM_GOOSE_EXTRA_INSTRUCTIONS`, which carries the instruction to the
  start-up hook Farhelm installs for Goose (empty when there is none). Running sessions keep working across updates, as
  SPEC_impl.md's list of what running sessions hold now records.

### Open questions and possible follow-ups

- Should switching waiting alerts on alert for a session that is already waiting at that moment? Today it does not (see
  above). It would be a small change to the alert policy if you want it.
- Agents run `farhelm agent notify` through their own shell tool, which must pass on the variables Farhelm sets in the
  session so the command can find its session; without them the command fails with an error and nothing is recorded. The
  plan singled out Muse Code because Muse strips the environment of its own hooks, which raised the question whether it
  does the same to its tool's commands. This could not be checked without the real Muse. Possible follow-up: try it once
  in a Muse session.
- No real agent has been run with the instruction yet (that spends vendor turns). The tests check what each agent is
  given at launch and that the start-up hook Farhelm installs prints it; whether each agent then actually calls
  `farhelm agent notify` when blocked is untested. Possible follow-up: a real-agent drift case.
- Possible follow-up: give the interrupted GitHub checkout creation the instruction as well (see above).
- Possible follow-up: say somewhere visible when alerts cannot be sent because the desktop app's window is closed.

### The PRs

- #5118 `fix: send ntfy waiting alerts reliably and say when they are off`
- #5125 `feat: let an agent tell the user it is blocked`
- #5135 `feat: ask agents to notify the user when they are blocked`

### Checks

The stack was rebased twice before delivery. The first rebase brought in desktop changes (terminal tab close buttons,
copying and pasting files, how the desktop reads from the helm, which the Notifications bell relies on and its tests
below cover), documentation and plan bookkeeping, and nothing in the protocol types, the helm, the supervisor or the
`farhelm` command. The second brought in agent host add's Tailscale option (#5126), which changes the agent command, the
protocol types, the helm's agent requests, the desktop's approval cards and Add host dialog, SPEC.md, SPEC_impl.md and
the "Approve agent requests" page beside this stack's own changes there. It takes protocol version 92, so this stack's
two bumps moved to 93 and 94. Run after the second rebase:

- `cargo fmt --all -- --check`, `cargo check --tests` on the six touched crates (the protocol types, helm, supervisor,
  `farhelm` command, desktop GUI and desktop app) and the helm client, and `cargo clippy` on the touched crates with
  `-D warnings`: clean.
- Where this stack and the Tailscale option meet, through the test recorder: the protocol types, the `farhelm` command
  and its agent command tests, the helm's agent requests, notices and ntfy, the supervisor's request handlers, and the
  end-to-end tests of the agent commands and the injected start-up hooks against a real helm and supervisor
  (`cargo nextest run -p farhelm-proto -p farhelm -p farhelm-helm -p farhelm-supervisor --lib --bins --test agent_cli --test e2e -E '...'`):
  831 passed (run 34ec5dec).
- The desktop's Settings, Notifications bell, approval card and Add host dialog tests
  (`cargo nextest run -p farhelm-gui --lib -E '...'`): 467 passed (run 59435376).
- Every reference screenshot, rendered from the rebased tree and compared with the reference set, which includes the
  Notifications settings scenes this stack published: all equal (run bf0db406).
- `dprint check` on SPEC.md, SPEC_impl.md, TODO.md and the two changed docs pages,
  `python3 releasing/check-changelog.py format`, the test-sleep check (306 delays, none lacking a rationale), and the
  website build (all internal links valid): clean.

Reused from the final state before the first rebase (after every review fix but two documentation-only edits). What the
rebases changed in the areas these cover is either tested again above or is the Tailscale option's own code (the helm's
approval handling and test fixtures), which this stack does not touch:

- The touched areas across all three PRs, through the test recorder: the protocol types, the whole supervisor library
  (its create and restart paths changed), the helm's agent requests, notices, ntfy, store, sessions and client, the
  `farhelm` command, the desktop's Settings and bell, and the end-to-end tests of the injected start-up hooks and of the
  agent commands against a real helm and supervisor
  (`cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm -p farhelm-gui --lib --bins --test e2e -E '...'`):
  3106 of 3109 passed (run 9b53d226). The three failures were tests of Goose's exact launch environment, written before
  the new variable above; updated, they pass (run 708a189c, which ran only the 113 Goose launch tests).

- Screenshots: each PR's full re-render changed only Notifications settings scenes (two to four of them, light and
  dark); each was inspected and pushed with the screenshot publish script.

New tests seen failing as intended: the waiting-alert policy test with its alerts-off handling removed (run 2d5e2128),
and the test that the waiting switch's agent list matches which agents' questions status detection recognises on the
captured agent screens, with Muse Code wrongly listed as never waiting (run 4e61c246).

Skipped: the workspace-wide Rust run and the installer and provisioning suites, since the change is confined to alerts,
one agent command and instruction delivery, all covered above; real-agent runs (they spend vendor turns; see the
follow-ups).

### Review gate

Each PR was reviewed by a fresh-context Opus 5.5 agent at high effort, then verified by the same reviewer after the
fixes; every gate passed.

- #5118: the first review found that the changelog over-claimed a fix, that an older desktop's save would have turned
  waiting alerts back on, and that the start-up read of the setting ran in the wrong place and hid its errors, plus test
  and wording issues. All fixed. The verification found one more: a stored setting this build cannot read (one a newer
  build wrote, say) must not stop the helm from starting. Now the helm starts, logs a warning and sends no waiting
  alerts; Settings then says it cannot read the ntfy settings, as it already did for an unreadable configuration.
- #5125: the first review found the waiting-behind-an-approval problem described above, the unbounded notices from a
  misbehaving host, and test and documentation gaps. All fixed and verified.
- #5135: the first review found that Pi and OMP sessions created by an older Farhelm got the
  `farhelm agent instructions` line on Restart but not the blocked instruction, and documentation and test gaps,
  including a missing check that the helm actually sends the instruction. All fixed and verified.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the rest of the round that was landing then and all of the next:
Codex forks and Codex app-server conversations became restartable, `farhelm agent`'s `--model` and `--effort` stopped
taking the next option as their value, an unused helm route for creating Tensorlake sandbox sessions was removed, a
supervisor start now finishes the cleanup a failed Restart left behind, the release brick test no longer looks for a
Welcome card, and a drift test was added. Only the `--model` and `--effort` change touches agent commands, and it does
not affect `farhelm agent notify` or the agent instructions. In the same round, after this plan, three more landed:
claude-restart-before-first-turn (Restart before Claude's first prompt starts Claude fresh),
settings-save-close-delete-prompt (a recipe's Delete question stays open on Keep editing) and
shell-tab-first-close-sweep-backoff (an exited tab's failed first cleanup backs off). The Claude plan passes the
session's launch instructions through Restart, and was written before this plan added the blocked instruction to them;
its test needed the fix described in its own notes, and its fresh start passes the instruction on, as a resume does (by
reading the code; that plan's notes record that no test checks it). This plan is the only one that raised the protocol
version, to 93 and then 94; every place that names the number agrees.

An independent reviewer and I read the change. I looked at the refreshed Settings → Notifications pictures: the **Enable
ntfy alerts** switch, the per-kind switches with the agents each one covers, Farhelm's own wording shown under **What
counts as blocked**, and the line beside **Send test** saying the form has unsaved changes. The reviewer found no place
where the ntfy topic reaches a log, an agent or a reply, and found nothing untrue in the report.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing I found. The landing's checks were wider than the report's: the whole Rust test suite and the desktop app's
end-to-end smoke ran, together with the round's other plans.

#### Worth knowing

- **One agent can use up the alert limit for everyone.** All ntfy alerts share the limit of five a minute, so an agent
  calling `farhelm agent notify` in a loop can crowd out other sessions' waiting alerts for as long as it keeps going.
  Possible follow-up: a separate limit per session for blocked alerts.
- **With the master switch off, the switches under it still look on.** In the reference picture, **Enable ntfy alerts**
  is off while **Agent is waiting for an answer** below it shows on, though nothing is sent. Possible follow-up: show
  the switches under it as inactive while the master switch is off.
- **Turning Agent is blocked off does not stop the bell notices** from sessions that already got the instruction: those
  agents may still run `farhelm agent notify`, which still records its notice; only the ntfy alert follows the switch.
  Those sessions keep the instruction until they are replaced. Possible follow-up: say so in the switch's help.
- **The text an agent sends reaches your phone as written**, up to 500 characters with control characters removed. So an
  agent, or text injected into one, can send an alert from Farhelm that looks official, a link included. That is
  inherent to the feature; the reviewer suggests one line in the switch's help or on the docs page saying alerts carry
  the agent's own words. Possible follow-up.
- **`farhelm agent notify` refuses a reason that starts with `-`** as a usage error, and its help does not say to put
  `--` first. The risk is low, since the instruction asks for "<what is blocked>: <why>".
- **Two small gaps in the tests and the code map.** No test checks that an agent's start-up hook carrying the blocked
  instruction still passes the check Farhelm makes on who reported a conversation; reading the code, it does, but if it
  ever did not, Claude Code, Codex and Muse Code sessions would stop being restartable while blocked alerts are on. And
  the overview at the top of the supervisor's per-agent code does not yet name the two new per-agent capabilities this
  plan added, though they are built the way the repository's rules ask. Possible follow-ups for both.

#### Checks

Run on the round's four plans stacked on main as it stood when the landing started (agent-blocked-notifications,
claude-restart-before-first-turn, settings-save-close-delete-prompt, then shell-tab-first-close-sweep-backoff), after
the landing's fix to claude-restart-before-first-turn's test. Because agent-blocked-notifications raises the protocol
version and the four plans between them touch the helm, the supervisor's agent-specific code, the command line, the
desktop app, its reference screenshots and the docs, the whole Rust test suite ran. The SSH test worker was still
unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- A first run failed at its first step: `cargo clippy --all-targets -- -D warnings` could not compile a new test of
  claude-restart-before-first-turn, the problem described in that plan's notes. The other clippy checks passed and the
  test build then failed on the same error, so no test ran. After the fix, every check below ran from the start.
- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app that runs by default (`cargo nextest run --workspace`,
  built once, split in three and run through the recorder), recorded runs d273d9bc-7896-4c1c-89a4-106d7f060a1d,
  a74ca485-4be5-4e2f-bbf5-ff250895f815 and 3e5673f4-c3a7-461f-9d15-7c0c7956e6a1: 9,180 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache; a boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and no plan in this round touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  8fc4d4bc-523d-4650-856d-e560a8137b59, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  693426e3-0300-4c24-8257-adc94ddb9901: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  fb93b279-02be-42ce-94f6-f269a97c0d79, the test of the screenshot tooling's list of screens, recorded run
  653ebb0b-2dbc-432c-8da0-1f814940b0cb, and the workspace's documentation examples
  (`cargo test --doc --workspace --exclude farhelm-desktop`), recorded run 5dfbb55e-5cbe-4bee-beb4-cac09ef01146: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown,
  `python3 scripts/check-agents-md.py` (the drift-tests document is one the agent instructions name), and the docs
  website build (`bun run build`, which checks every link): passed.

Reused: nothing. Skipped: the opt-in tests, among them the real-agent drift tests, which spend vendor turns; the
installer and provisioning suites, since no plan in this round touches them; and any run on a Mac, since none was
available to the landing.
