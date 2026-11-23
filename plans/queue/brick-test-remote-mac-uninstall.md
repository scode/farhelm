# Extend release brick test P1 to cover remote Mac uninstall

Written against main at cb778ae6 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Brick test remote Mac uninstall.** The brick test's P1 covers Uninstall from the remote host's menu only for a
> remote Linux host. Extend it to a remote Mac. That needs new Tart fixture work: the one Mac guest runs the helm, and
> remote setup, Update and uninstall are refused on a Mac with the desktop app installed (SPEC.md, "Topology"), so the
> remote Mac has to be a second Mac guest without the app. The procedure is in `releasing/mac-vm-test/BRICK-TEST.md`.

In product terms: extend the release brick test's P1 tier with a dedicated two-VM pass (`p1-remote-mac`) managed by Tart
(primary Mac VM running the candidate desktop app + headless second Mac VM cloned from `mac_base` without `Farhelm.app`,
respecting Apple's two-macOS-VM concurrency limit). In that pass, add the remote Mac host through the candidate app's
UI, verify its remote `launchd` supervisor (`user/<uid>/io.farhelm.supervisor`) is running, start and stop one quick
plain-command session so `supervisor.db` holds a session ID, run **Uninstall** from the remote host menu, and verify
with fixed before/after SSH snapshots that the `launchd` job is unloaded/stopped, the plist and lib directory/binary are
gone, no Farhelm process remains, and the data directory and database survive with the session ID intact.

### Where it stands (from planning research at main cb778ae6; line numbers approximate)

- `releasing/mac-vm-test/machine_profile.py`:
  - `Profile.load()` validates format 1/2 private profiles (`mac_base`, `linux_base`, `linux_user`, etc.). Note that
    `brick_test.py:580` enforces `self.data['profile'] != self.profile.values` on saved runs, so an optional
    `remote_mac_base` key must remain omitted from `profile.values` when not present in the file, and resolve
    `profile.values.get('remote_mac_base', profile.values['mac_base'])` at `prepare()` time.
- `releasing/mac-vm-test/control.py`:
  - `Control` currently allows only roles `('mac', 'linux')`, and `ready()` checks `/dev/console` GUI login on `mac`
    (inappropriate for headless `remote-mac`) and `systemctl is-active --quiet ssh` on `linux`.
- `releasing/mac-vm-test/network.py`:
  - `configuration()`, `route_guests()`, and `connect()` currently hardcode `fh-linux-<uuid>`, Linux `ip route`,
    `/usr/bin/python3`, and `systemctl --user show-environment`. Generalizing them in place for `peer_role='remote-mac'`
    reuses the exact pinned-SSH and gateway-routing architecture without duplicating it.
- `releasing/mac-vm-test/fixture_probe.py`:
  - Already implements macOS probing (`role == 'mac'`) and self-transport over SSH, including checking
    `~/Applications/Farhelm.app` and `/Applications/Farhelm.app`, but `paths()` omits
    `~/Library/LaunchAgents/io.farhelm.supervisor.plist` and `main()` hardcodes `fh-linux-<uuid>`, `--role linux`, and
    `/usr/bin/python3`.
- `releasing/mac-vm-test/guest_operations.py`, `checkpoint_contract.py`, `guest_tester.py`, `guest_workflow.py`,
  `brick_test.py`:
  - `remote_host_before` / `remote_host_after` (freeze and ordering guards in `guest_operations.py:636-644`) and
    `require_remote_host_effect` + `EffectFailure` (`checkpoint_contract.py:184-240`, consumed by
    `guest_tester.py:304-315`) already own remote-menu verification.
  - `remote_host_snapshot` / `remote_host_payload` (Linux `/proc` + `systemctl`) is shipped over SSH via
    `inspect.getsource()`. Keeping a separate `remote_mac_host_snapshot` + `remote_mac_host_payload` for macOS keeps
    shipped payloads small and OS-honest while sharing `remote_host_before`/`remote_host_after` operation names and one
    unified `require_remote_host_effect` validator.
  - Remote Mac supervisor layout (`SPEC_impl.md:7988-8030`): `~/Library/LaunchAgents/io.farhelm.supervisor.plist`,
    loaded in `user/<uid>` as `io.farhelm.supervisor` (inspected via
    `/bin/launchctl print user/<uid>/io.farhelm.supervisor`), lib directory `~/.local/lib/farhelm`, data directory
    `~/.local/state/farhelm`, database `~/.local/state/farhelm/supervisor.db`.
  - `guest_workflow.py:require_setup` strictly enforces `authorization.get('destinations') != ['mac', 'linux']` (and
    `BRICK-TEST.md:319` states credential approval for one destination does not cover another). Using a quick
    **plain-command** session (`agent type none`, e.g. `/bin/sh`, matching `remote-terminal` in `guest_workflow.py:504`)
    on `remote-mac` proves session creation/stop and `supervisor.db` retention without injecting Codex credentials into
    `remote-mac` or invalidating existing operator `credential-authorization.json` files.
  - Several existing predicates assume `not pass_id.startswith('p0-')` or `priority != 'P0'` means "bonus":
    - bonus preflight (Finder drag/double-click, documented broken on the installed native CUA runtime in
      `OPERATIONS.md:263-275`): `guest_operations.py:518`, `checkpoint_contract.py:168`, `guest_tester.py:371,604`,
      `guest_workflow.py:424`;
    - `brick_test.py`: `test_owned` (`addendum.md` at line 891, P1 automation-failure `native-preflight` record at line
      1009), `require_no_failure` (line 1402), `cleanup` (`--discard-failed` at line 1817), and `add_check` (line 1346).
  - `Operations.ssh()` (`guest_operations.py:416`) hardcodes `/usr/bin/python3`; macOS guests need
    `self.task['settings']['python']` (Homebrew Python). `available_operations()` (`guest_operations.py:248`) exposes
    Linux-only `remote_terminal_hosts` (`/proc`); omit it on `p1-remote-mac`.
  - `plan()` (`brick_test.py:184-236`) is scope-agnostic: every planned pass exists in `self.data['passes']`, and
    `scope` (`p0-only`, `p1-only`, `p2-only`, `full`) fences execution via `require_priority()` and `scope_tiers()` so
    `continue-full` and honest `could not run` reporting work uniformly.

### Acceptance criteria

1. **Private profile and Tart fixture (`machine_profile.py`, `control.py`, `network.py`, `fixture_probe.py`):**
   - `Profile.load()` accepts an optional `remote_mac_base` (validated with `base_binding`, distinct from `linux_base`)
     and keeps it omitted from `profile.values` when absent so existing format-2 profiles and saved `workflow.json` runs
     stay valid unchanged. When absent, `p1-remote-mac` clones `remote-mac` from `mac_base`.
   - `Control` supports role `'remote-mac'` (`fh-<run_id>-remote-mac`), booted headless (`--no-graphics`), with a
     headless macOS readiness probe in `ready()` checking virtualized guest (`kern.hv_vmm_present == 1`),
     `/bin/launchctl print "user/$(id -u)"`, and SSH readiness/host key (never `/dev/console`).
   - `network.py` generalizes `configuration()`, `route_guests()`, and `connect()` in place for `peer_role='remote-mac'`
     (`fh-mac-<uuid>` alias prefix, macOS `route`/`arp -d` on both peers when `route_via_gateway` is true, peer Python
     `/opt/homebrew/bin/python3`, final SSH check proving authenticated SSH and `launchctl print "user/$(id -u)"`,
     username discovered via `id -un` during `prepare()`). No duplicate `sshd` or `Farhelm.app` check in `network.py`.
   - `fixture_probe.py` adds `~/Library/LaunchAgents/io.farhelm.supervisor.plist` to `paths()`, accepts `'remote-mac'`
     and `fh-mac-<uuid>` (invoking the peer with `sys.executable` and `--role remote-mac`), and `brick_test.py` passes
     the pass's expected peer role to `require_clean_pair()`.
2. **Dedicated P1 pass and checkpoints (`brick_test.py`, `guest_operations.py`, `checkpoint_contract.py`,
   `guest_tester.py`, `guest_workflow.py`):**
   - `plan()` always includes `p1-remote-mac` (right after `p0-installer-upgrade`, before `fresh`), always part of P1
     (no `exclude mac` override), fenced in `require_priority()` on `'P1' in self.scope_tiers()` and P0 sequence
     completion (and after `p0-installer-upgrade` cleans up under the existing single-working-pair rule).
   - `p1-remote-mac` runs 6 P1 checkpoints: `artifact-verification`, `fixture-clean`, `install`, `app-start`,
     `remote-provision` (reusing `remote-provision` with `remote_version` generalized for macOS
     `launchctl print user/<uid>/io.farhelm.supervisor`), and `remote-host-uninstall` (folding start + stop of one quick
     plain-command session before `remote_host_before`, then native UI Uninstall + kept-data notice capture +
     `remote_host_after`).
   - `guest_operations.py` reuses `remote_host_before` and `remote_host_after` operation names, selects a dedicated
     macOS `remote_mac_host_snapshot` / `remote_mac_host_payload` (no `/proc`, checking
     `launchctl print user/<uid>/io.farhelm.supervisor`, `~/Library/LaunchAgents/io.farhelm.supervisor.plist`,
     `~/.local/lib/farhelm`, `~/.local/state/farhelm/supervisor.db`, and macOS `ps` for Farhelm processes under the lib
     dir, returning shared filesystem/process/DB keys plus an OS-honest launchd `service` sub-object).
   - `checkpoint_contract.py`'s `require_remote_host_effect` validates both Linux systemd and macOS launchd uninstall
     snapshots (job unloaded/absent, plist and lib/binary absent, zero Farhelm processes left, data dir and
     `supervisor.db` retained with identical non-empty `session_ids`), raising `EffectFailure` on product contradictions
     and `TesterError` on invalid starting premises.
   - `p1-remote-mac` copies Codex credentials only to `('mac',)` (leaving `credential-authorization.json` unchanged),
     skips Linux Codex probes in `prerequisites` and `continuity.py` staging, selects `self.task['settings']['python']`
     in `Operations.ssh()` for `remote-mac`, and omits Linux-only `remote_terminal_hosts` from `available_operations()`.
   - Bonus-vs-P1 predicates across the five modules treat `p1-remote-mac` as P1 (not bonus): P0/P1 preflight (no Finder
     drag), no `addendum.md` required, `native-preflight` guarded on P1 automation failure, P1 failure does not block P2
     or bonus or require `--discard-failed` for cleanup, and `add_check` rejects bonus `extra-*` on `p1-remote-mac`.
   - `named_checks()` exposes `'remote-mac-uninstall'` mapped to `('remote-mac-uninstall', 'p1-remote-mac', None)` as
     the 5th named P1 result whenever `'p1-remote-mac' in self.data['passes']`, keeping saved layout-2 runs compatible
     without bumping `PRIORITY_LAYOUT`.
3. **Documentation and TODO cleanup:**
   - `releasing/mac-vm-test/BRICK-TEST.md`, `RECIPE.md`, `OPERATIONS.md`, and `BRING-UP.md` accurately document
     `p1-remote-mac`, the 5 named P1 results, the Remote Login prerequisite on `mac_base`, and optional
     `remote_mac_base`.
   - The TODO.md entry ("Brick test remote Mac uninstall.") is removed.
4. **Validation:**
   - All existing and new unit tests pass under
     `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` (recorded through
     `scripts/record-test-run.py`, injecting all config/subprocess boundaries without mutating process environment
     variables).

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Tart-managed second Mac VM reusing `mac_base` booted headless without installing `Farhelm.app` (with an optional
  `remote_mac_base` override in `brick-test.json` if ever wanted).
- Dedicated P1 pass (`p1-remote-mac`) with 2 VMs (`mac` + headless `remote-mac`), run after `p0-installer-upgrade`
  cleans up.
- Always include in P1 (no `exclude mac` override needed).
- Product checks on the remote Mac: Add remote Mac + start & stop one quick session (so the retained session database
  proves the session ID survived) + Uninstall via host menu.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (internal release testing harness; no user-facing product change).

**Binding repository constraints:**

- `SPEC.md` ("Topology") and `SPEC_impl.md` (lines 7988–8030): remote Mac setup/uninstall refuses a Mac with
  `Farhelm.app` installed; remote Mac service is `~/Library/LaunchAgents/io.farhelm.supervisor.plist` in `user/<uid>`.
- `releasing/mac-vm-test/BRICK-TEST.md` ("Validate changes to this coordinator") and root `AGENTS.md`: tests inject
  fictional config and subprocess boundaries without modifying process environment variables; never touch the live
  Farhelm install or real Tart VMs during unattended execution (portable unittest suite runs on Linux without Tart).
- `.agents/test-authoring.md` for every test added or changed.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path" (`releasing/AGENTS.md` applies to `releasing/`).

**Planner proposals (refined by fresh-context planning review):**

- Reuse `remote_host_before` / `remote_host_after`, `require_remote_host_effect` (in `checkpoint_contract.py`),
  `remote-provision`, and `remote-host-uninstall` (while keeping `remote_mac_host_snapshot` separate from Linux's
  `remote_host_snapshot` for clean `inspect.getsource()` payloads and OS-honest service fields).
- Use a quick plain-command session (`agent type none`, e.g. `/bin/sh`) folded into the uninstall checkpoint before
  `remote_host_before`, avoiding a redundant Codex checkpoint and keeping existing `credential-authorization.json` files
  (`destinations: ["mac", "linux"]`) valid unchanged.
- Plan `p1-remote-mac` unconditionally in `plan()` and fence via `require_priority()` + `scope_tiers()` so
  `continue-full` and honest `could not run` reporting match every other pass.
- Deliver as a 3-PR linear stack (fixture/SSH -> pass/checkpoints/coordinator -> docs/TODO).

## Outline

1. `plan/brick-test-remote-mac-uninstall/01-tart-remote-mac-fixture-and-ssh`:
   - `releasing/mac-vm-test/machine_profile.py`: optional `remote_mac_base` (validated via `base_binding`, distinct from
     `linux_base`, omitted from `profile.values` when absent).
   - `releasing/mac-vm-test/control.py`: `'remote-mac'` role (`fh-<run_id>-remote-mac`), headless start, headless macOS
     readiness probe in `ready()`.
   - `releasing/mac-vm-test/network.py`: generalize `configuration()`, `route_guests()`, and `connect()` in place for
     `peer_role='remote-mac'` (`fh-mac-<uuid>`, macOS peer routing on both sides, `/opt/homebrew/bin/python3`,
     `launchctl print "user/$(id -u)"`).
   - `releasing/mac-vm-test/fixture_probe.py`: add `~/Library/LaunchAgents/io.farhelm.supervisor.plist` to `paths()`,
     accept `'remote-mac'` and `fh-mac-<uuid>` with `sys.executable`.
   - Unit tests in `test_machine_profile.py`, `test_control.py`, `test_network.py`, `test_fixture_probe.py`.
   - `test: add Tart remote-mac fixture, pinned SSH, and probe support`.
2. `plan/brick-test-remote-mac-uninstall/02-remote-mac-uninstall-pass`:
   - `releasing/mac-vm-test/guest_operations.py`: add `remote_mac_host_snapshot` / `remote_mac_host_payload`; dispatch
     `remote_host_before` / `remote_host_after` and `remote_version` by peer OS; use `self.task['settings']['python']`
     in `Operations.ssh()` for `remote-mac`; keep `prerequisites` free of remote Codex checks on `remote-mac`; omit
     `remote_terminal_hosts` from `available_operations()` on `p1-remote-mac`.
   - `releasing/mac-vm-test/checkpoint_contract.py`: generalize `require_remote_host_effect` for systemd and launchd
     snapshots (requiring non-empty `before['session_ids']`); add macOS remote uninstall instructions (starting/stopping
     one plain-command session before `remote_host_before`, then native UI Uninstall); switch bonus-preflight predicate
     to check bonus passes rather than `not pass_id.startswith('p0-')`.
   - `releasing/mac-vm-test/guest_tester.py` and `guest_workflow.py`: support `p1-remote-mac` (P0/P1 preflight,
     `copy_credentials` to `('mac',)` only, `remote-provision` + `remote-host-uninstall`).
   - `releasing/mac-vm-test/brick_test.py`: plan `p1-remote-mac` after `p0-installer-upgrade` (6 P1 checkpoints), add a
     `pass_roles(pass_id)` helper, resolve `remote_mac_base` defaulting to `mac_base`, discover `remote-mac` username
     via `id -un`, skip `continuity.py` staging on `p1-remote-mac`, pass expected peer role to `require_clean_pair()`,
     fix the bonus-vs-P1 predicates (`addendum.md`, `native-preflight` guard, `require_no_failure`, `cleanup`,
     `add_check`), and expose `'remote-mac-uninstall'` in `named_checks()` when
     `'p1-remote-mac' in self.data['passes']`.
   - Unit tests in `test_brick_test.py`, `test_guest_operations.py`, `test_guest_tester.py`, `test_guest_workflow.py`,
     `test_native_coordinator.py`.
   - `test: add p1-remote-mac pass for remote Mac uninstall brick testing`.
3. `plan/brick-test-remote-mac-uninstall/03-brick-test-docs-and-todo`:
   - Update `releasing/mac-vm-test/BRICK-TEST.md`, `RECIPE.md`, `OPERATIONS.md`, and `BRING-UP.md` for `p1-remote-mac`,
     the 5 named P1 results, Remote Login on `mac_base`, and optional `remote_mac_base`.
   - Remove the TODO.md entry ("Brick test remote Mac uninstall.").
   - `docs: document remote Mac uninstall in release brick test`.

## Fallbacks

- Reusing checkpoint ID `remote-host-uninstall` vs. introducing `remote-mac-uninstall` inside `p1-remote-mac` is a close
  call; if reusing `remote-host-uninstall` proves awkward during PR 2 (for instance in instructions or per-checkpoint
  diagnostics), using `remote-mac-uninstall` (still sharing `remote_host_before`/`remote_host_after` and
  `require_remote_host_effect`) is an authorized routine fork — log the choice as a DECISION.
- Do not modify or run real Tart VMs on this Linux host; validate via the portable unittest suite and note in the report
  that live macOS Tart execution remains to be exercised on the maintainer's Mac at the next brick test.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-brick-test-remote-mac-uninstall-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-brick-test-remote-mac-uninstall-watchdog.json` beside
it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent
scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Read `releasing/AGENTS.md` (and `releasing/mac-vm-test/BRICK-TEST.md`) before editing under `releasing/`. Run builds and
tests as `.agents/validation.md` describes, every runtime test through `python3 scripts/record-test-run.py`, and report
checks run, reused and skipped per root `AGENTS.md`, "Tests and validation". Run the full brick-test unittest suite:
`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` (through `scripts/record-test-run.py`),
`dprint check` on changed Markdown, and `python3 scripts/check-agents-md.py` if any headings change. Apply
`.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live Farhelm
install on this machine (root `AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Brick test remote Mac uninstall."), as root
  `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you; never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
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
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
