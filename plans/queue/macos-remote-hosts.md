# Remote macOS hosts: set up, update and uninstall an Apple-silicon Mac over SSH

Written against main at bf86b15a on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Support adding remote macOS hosts.** V1 provisioning targets Linux hosts with a usable systemd user manager
> (SPEC.md, Topology); a Mac can only be added today by running the supervisor binary by hand, and the remote
> provisioning and remote Update flows do not cover it. Make adding a macOS machine as a remote host a supported flow.

Make a helm (a standalone helm on Linux, or the desktop app's embedded one) able to set up, update and uninstall Farhelm
on an Apple-silicon Mac reached over passwordless SSH, the way it does for a Linux host with a systemd user manager
today: discovery first, one confirmed plan that names every file and the background job it creates, no root, and the
same Update, Update all, automatic update and uninstall surfaces. The Mac's supervisor runs as a per-user launchd job
instead of a systemd user service. Remote Mac hosts have no tmux at all.

### Where things stand (as read on 2026-11-08; verify before relying on any line number)

Provisioning lives in `crates/farhelm-helm/src/provisioning.rs` and its submodules under
`crates/farhelm-helm/src/provisioning/`:

- `service.rs` runs the flows: probe and Add planning (`probe_owned`), Update planning (`plan_update_owned`), uninstall
  planning (`plan_uninstall_owned`, `uninstall_plan`), and execution (`execute_actions`, `prepare_payloads`,
  `execute_action`).
- `backend.rs` holds the `ProvisioningBackend` trait and `SystemBackend`. Every remote command is `ssh … "sh -c '…'"`.
  The reach script reports the `/etc/os-release` `ID`, `HOME`, `uname -m`, tmux, and the systemd manager state.
  `parse_reach_output` maps `x86_64` and `aarch64|arm64` to an architecture, then refuses a host without a usable
  systemd user manager. There is no `uname -s` check: a Mac is refused only by the systemd check, and an Apple-silicon
  Mac's `arm64` already maps to the Linux aarch64 payload.
- `plan.rs` holds `ProvisioningAction` and `PlanLayout::plan`: ensure directories (`~/.local/lib/farhelm`,
  `~/.local/state/farhelm`, the systemd user-unit directory), upload and install `farhelm`, optionally a private tmux,
  write the unit (`crates/farhelm-helm/src/units.rs`), daemon-reload, enable, enable linger, restart (Update only), and
  attach. Update reuses that plan with the registered paths (`plan_for_row`). Uninstall's plan disables the unit,
  removes it, daemon-reloads, stops, ends the private tmux server and terminal hosts, and removes the lib directory.
  `inspect_uninstall` reads `systemctl --user show` and `/proc/self/mountinfo`.
- Payloads: `assets.rs`, `payloads.rs`, `release_payloads.rs`. Releases already publish a signed
  `farhelm-aarch64-apple-darwin.tar.gz` (the one `farhelm` binary that runs helm and supervisor), listed in the signed
  `SHA256SUMS`. But `assets.rs` says the macOS entries "never travel over SSH": `farhelm_archive_for` maps an
  architecture only to a musl target, and neither `PayloadArch` nor `PayloadSource::path(kind, arch)` has an OS
  dimension. There is no macOS tmux payload. There is no Intel Mac build ("No Intel Mac (D4)", `dist-workspace.toml`).
- GNU-only remote shell: `stat -c` (several places), `/proc/self/mountinfo`, the missing-path message patterns,
  `systemctl`, `busctl`, `loginctl`. On the sandbox Mac (macOS 26.6), `sha256sum` and `readlink -f` exist (older macOS
  releases may lack them; see the planner proposals for how to handle that).
- The supervisor itself already runs on macOS: the desktop app starts it detached, its state rule is the same as Linux
  (`~/.local/state/farhelm`), process listing goes through `sysctl`, and its boot id comes from `kern.bootsessionuuid`.
  Since the new-terminals cutover, a supervisor runs no tmux unless old terminals exist (SPEC_impl.md, "Terminal
  substrate: private tmux server"), and no shipped launch creates old terminals. There is no launchd code anywhere.
- The supervisor's hello carries no OS field. Planning re-inspects over SSH, so the helm can learn the OS there.
- Tests: `provisioning.rs`'s test module fakes `systemctl`/`loginctl`. `real_provisioning_case` runs real provisioning
  locally, over SSH to localhost, or against a remote destination given by `FARHELM_TEST_SSH_DESTINATION`, with payloads
  from `FARHELM_TEST_BINARY`/`FARHELM_TEST_TMUX`. Its teardown calls `systemctl`. `scripts/test-provision-centos.sh` is
  the Linux real-host harness. Browser tests use a fake backend (`FARHELM_E2E_PROVISIONING_BACKEND_DIR`) whose reach
  result has no OS field.

### Measured on the sandbox Mac during planning (2026-11-08)

The sandbox Mac is an Apple-silicon macOS 26.6 VM with zsh as the login shell, Homebrew, no tmux, Rust 1.99.1 and the
Command Line Tools. Nobody was logged in at its screen.

- `launchctl print gui/<uid>` fails ("Domain does not support specified action"), and `launchctl managername` reports
  `Background`. The `user/<uid>` domain exists.
- Bootstrapping a throwaway job into `user/<uid>` from an SSH session failed with "5: Input/output error" until the
  plist carried `LimitLoadToSessionType` = `Background`. With it, `launchctl bootstrap user/<uid> <plist>` succeeded.
  The job (KeepAlive, RunAtLoad) kept running after SSH disconnected, launchd restarted it when its process was killed,
  and `launchctl bootout user/<uid>/<label>` removed it.
- Not measured: what happens after a reboot (back at boot, at the next SSH login, or only at a screen login); whether a
  plist in `~/Library/LaunchAgents` with that session type is loaded by itself at any of those points; and what launchd
  does to the job's other processes (terminal hosts, agents) when the job is stopped or restarted.

### Acceptance criteria

1. Adding an Apple-silicon Mac over SSH where discovery finds no supervisor offers setup. The setup question names the
   files it places (the binary in `~/.local/lib/farhelm`, state in `~/.local/state/farhelm`, the launchd job's plist)
   and the job's label. It also states what the job does, measured rather than assumed: runs now, restarted by launchd
   after a crash, and what happens after a reboot. Nothing it does needs root.
2. Confirming installs the macOS payload (from the configured release source with the usual signature and checksum
   checks, or from a staged payload directory), starts the job, and registers the host. Sessions launched there work.
   Setup neither checks for, installs, nor mentions tmux on a Mac.
3. Discovery-first is unchanged: a supervisor already running on the Mac (started by hand or by a desktop app there) is
   used as-is, never restarted or replaced. Re-running setup against a provisioned Mac, including from a new helm whose
   registry was lost, finds the supervisor and re-registers the host with its sessions intact.
4. Update on a Mac host installs the helm's own build and restarts the supervisor through launchd, with the same
   refusals as Linux (newer build, newer schema). Sessions and their terminal hosts running on that Mac survive the
   restart, as they do on Linux. Update all and automatic updates include Mac hosts, with the ordinary progress and
   failure display.
5. Uninstall on a Mac host shows its plan first (the job and its plist, ending terminal hosts, the lib directory
   removed, the data directory kept and named), proceeds only on confirmation, and keeps Linux's refusals and retry
   semantics where they apply. On a Mac there is no private tmux server to end.
6. Refusals: an Intel Mac gets the existing "no payload for this architecture" path. On a Mac where the Farhelm desktop
   app is installed, setup, Update (manual, Update all and automatic) and uninstall are all refused with a message
   saying this is not supported, which does not point at starting the supervisor by hand. Discovery still adopts a
   supervisor already running there (criterion 3); only the actions that would install, restart or remove are refused,
   because they would write over the app's own `farhelm` and start a second supervisor beside the app's. A Mac missing a
   command the Mac path needs is refused, naming the command.
7. Linux hosts behave exactly as before; existing provisioning tests still pass.
8. SPEC.md and SPEC_impl.md describe the Mac flow (see Outline), the docs pages that cover adding and managing remote
   hosts say what is supported on a Mac and what is not, and a changelog fragment covers the user-visible change.
9. The real-Mac verification under Validation is done and recorded in the report.

## Decisions already made

Answers the maintainer gave while planning, in their own words where they used any. Do not reverse them.

- **Persistence: a per-user launchd job.** It is written under the user's `~/Library`, started right away, and restarted
  after a crash. The setup question states the reboot behavior as measured (see Validation), and mentions automatic
  login only if that is what the measurement says is needed. No system daemon, no sudo.
- **No tmux on Mac hosts.** In the maintainer's words: "tmux is not supported for this. no one has ever set up existing
  sessions so no session exists on macos that is tmux. so just jump straight to the new world pretending tmux is
  entirely unsupported for everything that is specific to macos hosts. minimize complexity having to do with anything
  tmux related." So: no tmux check, payload, install, version floor or uninstall step on the Mac path, and no
  compatibility code for old terminals on Mac hosts. This is an approved exception to SPEC_impl.md's transition rule
  that provisioning keeps requiring tmux; write the exception into the spec. It covers remote Mac hosts only: the
  desktop app's own Homebrew tmux requirement on the local Mac stays as it is.
- **Uninstall is in scope** for Mac hosts.
- **A Mac with the desktop app installed:** when no supervisor is running there, refuse setup, saying it is not
  supported. In the maintainer's words: "DO NOT point at starting by hand. simply this this is not supported."
- **The desktop app's local supervisor is out of scope.** It keeps being started detached by the app. A Near term TODO
  entry (added with this plan) records moving it to the same launchd mechanism once remote Macs are validated as stable.
- **Testing happens on the maintainer's sandbox Mac**, which the maintainer said can be used freely for builds, running
  tests and running supervisors ("go nuts"). Leave its GitHub Actions runner's files and processes alone; there is no
  other constraint.
- **Apple silicon only**: releases ship no Intel Mac build.
- **Same directories as Linux and the local desktop app**: the binary in `~/.local/lib/farhelm`, state in
  `~/.local/state/farhelm`.
- **Code review gate: gpt-6.1-sol at high effort**, a single fresh-context agent with the general charter (see Code
  review gate).

## Requirement sources

**The user's request:** the TODO entry and the decisions above.

**Binding repository constraints:**

- SPEC.md: Topology (remote provisioning, Update, automatic updates, uninstall, idempotent recovery, no root, no relay);
  "Supported host setup"; "Supported user environments"; "Ownership during cleanup and provisioning"; "Durability and
  resume"; "Upgrade compatibility and client scale" (no upgrade-breaking changes; the peer protocol and any new plan or
  reach fields must stay compatible with hosts and helms of other versions).
- SPEC_impl.md: the provisioning, payload and release-asset sections; "Terminal substrate: private tmux server"; the
  terminal host sections (terminal hosts outlive the supervisor process).
- Root `AGENTS.md`: "Code rules" (its rule is about agent harnesses; for the OS, the planner proposals below ask for the
  Linux-or-Mac choice to be made in one place, as a preference rather than that rule), "The live install is off-limits",
  "Sharing the machine with other agents", "Tests and validation", "Changelog fragments", "TODO.md and FLAKES.md".
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`; `website/AGENTS.md` for the docs
  pages.

**Planner proposals** (change them if the code says otherwise, and log a DECISION):

- Learn the OS in the existing reach script (`uname -s`). Prefer one combined payload target type (Linux x86_64, Linux
  aarch64, macOS aarch64) over a separate OS enum beside the architecture, so an Intel Mac cannot even be represented
  and the payload choice has no impossible combination to check.
- Leave the Linux and systemd code as it is. Add Mac-specific steps to the existing frozen list of plan steps
  (`ProvisioningAction`: write the plist, start, restart, stop and remove the job) and choose between the Linux and Mac
  steps in one place in the plan builder. Do not move the systemd code behind a new service-manager abstraction: that
  would churn the Linux provisioning code for no requirement and put "Linux unchanged" at risk.
- The launchd job: label `io.farhelm.supervisor` (or the closest existing naming convention in the repo), a plist in
  `~/Library/LaunchAgents/`, `LimitLoadToSessionType` = `Background`, RunAtLoad, and `KeepAlive` =
  `{ SuccessfulExit = false }`, which matches the systemd unit's restart-on-failure (plain `KeepAlive = true` would also
  restart it after a clean exit). It runs the same command line, with the same environment hygiene, that the systemd
  unit gives the supervisor. Always use the `user/<uid>` domain, the one the planning measurement showed working from
  SSH: start with `launchctl bootstrap user/<uid>`, restart with `launchctl kickstart -k`, remove with
  `launchctl bootout` and delete the plist. Do not add a `gui/<uid>` path for when someone is logged in at the screen,
  since every later action would then have to find out which domain the job is in. Check that `user/<uid>` also works
  while someone is logged in at the screen. Do not use `launchctl disable` as the counterpart of Linux's disable step:
  it leaves a persistent override that makes a later setup fail to start the job. No setup marker on the plist: on Linux
  the marker tells `farhelm helm setup`'s unit apart from the helm's, and `helm setup` does not exist on macOS, so there
  is nothing to tell apart.
- Terminal hosts and agents must survive a supervisor restart (Update) and a supervisor crash, as on Linux, where
  `KillMode=process` provides that. On launchd, check the job's process-group handling (for example
  `AbandonProcessGroup`) and verify survival on the sandbox Mac rather than assuming it.
- Give the Mac its own commands only where the Linux ones fail there (`stat -c`, `/proc/self/mountinfo`, the GNU
  error-message patterns, the systemd commands), and leave the Linux commands alone. No macOS version floor is required:
  have the inspection check that every command the Mac path uses exists (including `readlink -f`) and refuse otherwise,
  rather than writing fallbacks for older macOS releases. Uninstall's mount-point check needs a macOS answer (`mount` or
  `df` output, or a `stat -f %d` device comparison); pick the smallest one that keeps the check's guarantee.
- Detect the installed desktop app by the location `scripts/install.sh` installs it to (`~/Applications/Farhelm.app`).
  Also check `/Applications` only if it costs nothing extra, and log what you chose. Do this in the inspection that
  setup, Update and uninstall all run, so a single check covers every refusal in criterion 6.
- Payload: `(macos, aarch64)` selects `farhelm-aarch64-apple-darwin.tar.gz` from both the release source and a staged
  payload directory, with no tmux entry. Check that a release binary uploaded over SSH runs on the Mac (no quarantine
  attribute, the linker's ad hoc signature intact).

## Outline

A suggested stack; reshape it if the code argues for another order, without churn.

1. **Setup and Update on a Mac.** The reach script reports the OS and the Mac's required commands; payload selection
   gains the macOS target; the plan's Mac steps and wording (the launchd job); the no-tmux path; the desktop-app and
   missing-command refusals; attach and registration; Update's restart through launchd; Update all and automatic
   updates. Unit tests with faked `launchctl` and the BSD tools, the same way systemd is faked today.
2. **Uninstall on a Mac.** The Mac uninstall inspection and steps, with its refusals.
3. **Spec and docs.** SPEC.md (Topology's "On the Mac, v1 ships no system integration" paragraph now describes the local
   desktop app only, with remote Macs getting the launchd job; the "V1 provisioning targets any Linux host" paragraph;
   uninstall; "Supported host setup"; Durability statements about the Mac supervisor after a crash or reboot, for remote
   Macs), SPEC_impl.md (the launchd job, payload selection, the tmux exception), and the website pages
   `website/src/content/docs/docs/get-started/add-a-remote-host.mdx` and
   `website/src/content/docs/docs/using/manage-hosts.mdx` (or whichever pages cover it). Spec text may instead ride with
   the PR whose behavior it describes; keep the changelog fragment in the first PR with user-visible behavior. The last
   PR removes the TODO entry quoted above.

Size: moderate to large. The cost sits in the Mac plan steps and their remote commands, the launchd semantics that have
to be measured, and real-Mac validation, not in new subsystems. No new persistent state, protocol message or UI is
expected; if one turns out to be needed (for example an OS field the browser must show), that is a departure for the
scope reassessment.

### Validation

The executing machine is Linux; the Mac work is validated on the maintainer's sandbox Mac.

- **Reaching it:** the SSH destination is the single line in `~/.farhelm-mac-sandbox` on the executing machine. Plain
  `ssh` to that destination works with no password (verified 2026-11-08). Never write the destination, its host name or
  its user name into the repository, a PR, the report, a Blocked section or a notification; refer to it as "the sandbox
  Mac". If the file is missing or the Mac is unreachable after a reasonable retry, block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10).
- **Using it:** builds, tests and supervisors are allowed. Put everything you create there under one directory named
  after your scratch directory's uuid (in the Mac's `/tmp` or home), and remove it when the plan closes. Leave the
  GitHub Actions runner alone. Do not leave a Farhelm supervisor or launchd job of yours running there when the plan
  closes; the last real uninstall run normally takes care of that.
- **The helm under test** is one you build and run from your checkout, with its own temporary state directory, never the
  live install (root `AGENTS.md`, "The live install is off-limits").
- **The darwin payload** is built on the sandbox Mac from the same commit as the helm under test (copy the tree there;
  `cargo` and the Command Line Tools are installed), and staged in a payload directory for the helm. Separately, check
  once that the published `farhelm-aarch64-apple-darwin.tar.gz` of the latest release runs after an SSH upload, which is
  the path real users take.
- **Measure before writing the plan wording:** after a setup, reboot the sandbox Mac. It is a VM, and its SSH user had
  passwordless `sudo` on 2026-11-08, so `sudo shutdown -r now` is allowed for this test. That root use is only for
  testing: nothing Farhelm does may use it. Before rebooting, check that the GitHub Actions runner is idle (no job in
  progress), and wait while it is busy. Record whether the job comes back at boot, at the next SSH login, or not until a
  screen login, and make the setup question and the docs say exactly that. If the reboot cannot be done (sudo no longer
  works), do not block: word the setup question and the docs so they promise nothing about reboots ("after a restart of
  the Mac, the supervisor may not return until …" stated as unverified), and list the measurement as an open item in the
  report. If the Mac does not come back within 30 minutes of the reboot, that is the unreachable case above.
- **The watchdog during the reboot:** the Mac's samples will fail while it is down. Record the reboot's start in the
  log. Until the Mac answers again (at most 30 minutes), treat failed Mac samples as expected rather than as a dead
  monitor; the Linux samples keep their normal rule throughout.
- **Real runs, recorded:** setup, a session (agent plus a tab) running, Update with that session surviving, a supervisor
  crash (kill it) with launchd restarting it and the session surviving, re-running setup (recovery), and uninstall.
  Prefer driving these through `real_provisioning_case` (and its siblings) with `FARHELM_TEST_SSH_DESTINATION` pointing
  at the sandbox Mac, extended with a Mac teardown, so the run is a repeatable test rather than a manual procedure; run
  them through `python3 scripts/record-test-run.py`. Report the recorder run ids, never the destination.
- Linux: run the existing provisioning tests that the change touches, by the judgment `.agents/validation.md` describes,
  including the CentOS harness if the shared snippets or plan changed in ways it covers.

## Fallbacks

These departures need the scope reassessment:

- any change to the peer protocol, the registry schema, or the browser UI;
- an Intel Mac payload, or building one;
- touching the desktop app's local supervisor or its tmux preflight;
- a launchd system daemon, sudo, or any root step;
- old-terminal (tmux) support on Mac hosts in any form.

Block, rather than work around, in any of these cases:

- the sandbox Mac is unreachable or `~/.farhelm-mac-sandbox` is missing;
- a supervisor started by launchd cannot be made to leave terminal hosts and agents running across its own restart, so
  Update or a crash would end sessions;
- the job cannot be started from SSH at all on the sandbox Mac with the documented approach (the planning measurement
  says it can).

Log these as DECISIONs:

- the launchd label, plist location and keys, and the bootstrap domain;
- the measured reboot behavior and the wording it produced;
- how the desktop app is detected;
- each shell snippet made portable versus given a Mac variant;
- the mount-point check's macOS answer;
- every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-macos-remote-hosts-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout. The sandbox Mac's SSH destination is in `~/.farhelm-mac-sandbox`
on the executing machine (see Validation); read it, never copy it into anything committed or sent.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs, and what is installed on the sandbox Mac) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type,
`$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux). While you build or test on the sandbox Mac, also sample
its home and `/tmp` filesystems and memory (`df`, `vm_stat`, `sysctl`) over SSH at the same interval. It writes each
sample to a private heartbeat/status file in the scratch directory, and it emits a notification when a watched
filesystem drops under 10% or under 5 GB free or available memory drops under 10% (whichever comes first), when the
number keeps falling, when the condition recovers, and when the monitor itself fails. In Claude Code a notification is a
stdout line of a monitor started with the Monitor tool; elsewhere, the harness's equivalent. A file update alone is not
a notification.

Before relying on the watchdog, verify delivery with a harmless synthetic alert, and verify failure detection by killing
a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a throwaway monitor
without killing it and confirm you are notified within two sample intervals. If any of these notifications is
unavailable or unverified, say so in the log and check the status file at least once a minute. On every path, check
heartbeat freshness before each new build, test run, sub-agent or review launch; a dead monitor, or a heartbeat stale
for two sample intervals, pauses new launches until monitoring is restored.

An alert is an instruction to act: stop launching work, remove jj workspaces, build output and scratch you own that are
no longer needed (on either machine), wait for or stop the job most likely responsible, and resume only when the
watchdog reports headroom. Record the watchdog's handle, watched paths, status path and delivery mechanism in the log,
include its state in every handoff note, and stop it when the plan closes. Do not lengthen the sampling interval to save
turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/macos-remote-hosts/<nn>-<short-name>`.
- Avoid churn; within this run, a PR that needs correcting is restructured rather than corrected on top, including PRs
  an earlier run built.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`. PRs stay drafts: never mark one ready and never merge; landing is the monitor's job.
- The last code PR removes the TODO entry quoted under The goal.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` (or natively, if your harness serves that model natively). If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log the
DECISION.

### Scope reassessment

Before implementing a substantial departure from the outline (examples under Fallbacks; not a blacklist), or when the
same component has needed repeated corrective review rounds, run a fresh-context review at your own model and effort.
Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered, including every review finding you decided not to follow.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Wording in this file that turns
out inaccurate (a line number, a file name, a function name) is corrected to the truth. A material scope expansion, a
weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or
a log entry is not authorization. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should
know and possible follow-ups go into the report's "things you should know" and "open questions and possible follow-ups"
sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, the real-Mac verification is recorded in the report, nothing of yours is left running on
the sandbox Mac, and the last PR removes the TODO entry quoted above. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12). Never edit `plans/` yourself.
