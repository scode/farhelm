# Use the login shell's PATH in the desktop app

Written against main at 969a63747 on 2026-11-20. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Use the user's proper PATH in the desktop app.** fix the lack of using the users proper PATH in the desktop app.
> (Screenshot: started from the Dock or Finder on macOS, the desktop app runs with the system's minimal default PATH
> rather than the user's shell PATH, so a program the user's ssh config runs, such as a ProxyCommand installed outside
> the system directories, is not found, and signing in to a host fails with a bare "Connection closed" error.)

The maintainer's instruction when choosing it, verbatim:

> yeah so for example /usr/local/bin was missing from path, and on a particular host that broke remote logins because of
> various site installed random things that wer ein various ssh wrappers. we need to pull the real PATH as a login shell
> would use it and use that when launching ssh for remote hosts, and we probably need some general rule here so it
> applies to any other future cases where we invoke user-owned tools (I think remote ssh is the only current case)

And on what happens when the PATH cannot be read, verbatim:

> the logging should go into a farhelm notification (bell at the left side) telling the user that we couldn't collect
> the PATH and to suggest they submit feedback, and that this shouldn't happen unless an interactive login shell takes
> longer than 5 seconds to start.

### Where it stands (from planning, at main 969a63747)

No code resolves a login shell's PATH for the desktop app, its helm or a local supervisor it starts.

- The helm runs only inside the desktop app's process (`run_embedded`, started on a thread from
  `DesktopBootstrap::start` in `crates/farhelm-desktop/src/lib.rs`). Every program it starts by name inherits the app's
  PATH: ssh to remote supervisors (`RemoteRoute::command` in `crates/farhelm-helm/src/access.rs`, spawned in
  `transport.rs`), sign-in (`SignIn::establish` in `ssh_sign_in.rs`, which prepends its helper directory to
  `std::env::var_os("PATH")`), provisioning (`provisioning/backend.rs`), Tensorlake's ssh and `ssh-keygen`
  (`tensorlake/shell.rs`, `tensorlake/ssh_key.rs`), and the Podman lookup on a local place
  (`environment/podman_probe.rs`). The desktop process itself runs `webbrowser`, Linux `pw-play` and `gsettings`, and
  the vendored GPUI's `xdg-open` by name.
- The local supervisor is spawned by `SpawnInputs::spawn` (`crates/farhelm-desktop/src/lib.rs`), which says "The
  supervisor inherits this process's PATH unchanged". It runs git for repository discovery and resolves Podman and
  systemd tools on that PATH. A supervisor that is already running is reused, so it keeps the PATH it started with until
  it is restarted.
- A session's environment starts from the supervisor's own (`host_child_env` in
  `crates/farhelm-supervisor/src/service/core/host_launch.rs`, whose `base` is the supervisor's environment), PATH
  included. The supervisor then runs the user's shell as `-l -i -c` (`crates/farhelm-supervisor/src/launch.rs`, module
  docs; `resolve_shell` and its pure half `resolve_shell_from`), which edits that PATH with the user's startup files.
  The edit does not give the same result whatever it starts from: macOS's `/etc/zprofile` runs `path_helper`, which puts
  the system directories first and moves every entry already on PATH after them. A session that starts from the login
  PATH would have the user's own directories demoted behind `/usr/bin`, and a startup line that adds a directory only
  when it is missing would leave them there. From a Dock launch today the supervisor passes the bare launchd PATH, which
  is what an ssh login starts from, so sessions come out as SPEC.md's "as if the user had SSHed in" contract wants.
- Started from the Dock or Finder, a Mac app gets launchd's minimal PATH, without `/usr/local/bin`. OpenSSH runs a
  `ProxyCommand` through `$SHELL -c` with ssh's own environment, so it is not found. The error is bare because only
  ssh's newest two stderr lines are kept (SPEC_impl.md, "Helm ↔ supervisor transport"), which push out the shell's
  "command not found".
- SPEC.md "Security" already states the general rule this plan defines: "The desktop app uses the user's PATH when it
  looks up a helper by name, as it does for Linux's `pw-play`." It does not say what "the user's PATH" is for an app
  started from the Dock.
- The rail bell (SPEC.md "Status", the paragraph starting "Notifications about a host or the whole app have a separate
  bell"; GUI in `crates/gui/farhelm-gui/src/rail_notices.rs`) draws only notices the helm keeps in its database
  (`crates/farhelm-helm/src/notices.rs`, content type `NoticeContent` in `crates/farhelm-proto/src/notices.rs`). The
  desktop process has no path to it today. The closest existing notice is the app-wide session-limit notice
  (`FleetSessionLimit`, raised and resolved by `reconcile_session_limit`), whose rail entry offers **Send feedback**
  (`session_limit_entry` in `rail_notices.rs`). The closest existing path from desktop startup into the helm's store is
  `devbuild`: read in `DesktopBootstrap::start`, passed through `embedded_helm_args` into `HelmArgs`, and written to the
  store in `serve_with_ready` (`crates/farhelm-helm/src/lib.rs`) before serving. The notices table's CHECK constraint
  names the allowed kinds, so a new kind needs a schema step (the session-limit kinds were added the same way;
  `upgrade_session_limits` in `notices.rs`).

### Acceptance criteria

1. On macOS and on Linux, at every app start (after Linux's second-launch hand-off, which exits before this), the
   desktop app runs the user's login shell once, the same shell sessions use and started the same way (login and
   interactive), with stdin from `/dev/null`, in a session of its own (no controlling terminal, so an interactive shell
   started from a development launch in a terminal cannot take that terminal over), and reads the PATH it ends up with.
   Whatever the startup files print is ignored and can never fill a pipe and stall the lookup: the shell writes its
   `$PATH` to a file the app names, with stdout and stderr discarded, or something equally immune; log the choice as a
   DECISION.
2. On success that PATH replaces the app process's own PATH before the local supervisor or the embedded helm starts, so
   everything the app and its helm run by name uses it. Only PATH is taken from the shell; no other variable changes.
3. A local supervisor the app starts runs with the login PATH too, so its own lookups by name (git, Podman, systemd
   tools) use it. Sessions on that supervisor still start from the PATH the app was started with, exactly as today: the
   app hands that PATH to the supervisor separately (an environment variable is the expected shape), the supervisor
   reads it once at startup without calling `set_var`, and `host_child_env` uses it as the session's starting PATH and
   leaves the variable out of the session's environment, so nothing started inside a session inherits it. A supervisor
   started any other way (by an earlier app launch and reused, by hand, on a remote host over ssh) has no such PATH and
   starts sessions as today. When the lookup fails the app has no login PATH, and the supervisor gets the app's PATH as
   today, with no separate one.
4. The lookup is bounded at 5 seconds. A shell that cannot be started, exits without leaving a usable PATH (none, or an
   empty value), or is still running at 5 seconds counts as a failure: the app keeps the PATH it was started with, logs
   a warning naming which of the three happened, and carries on starting. A shell still running at the limit is killed
   together with its whole process group, so nothing it spawned in that group outlives the lookup.
5. A failure raises one app-wide notice in the rail bell. It tells the user that Farhelm could not read the PATH their
   login shell sets, so programs it starts such as ssh use the PATH the app was started with; that this should not
   happen unless an interactive login shell takes more than 5 seconds to start; and suggests sending feedback, with the
   **Send feedback** button the session-limit notice has (opens the usual dialog, returns focus to the bell). Its text
   is fixed: it does not say which of the three failures happened (the log does), so the notice carries no payload.
6. The notice follows the existing rail-bell rules, as the session-limit notices do: a later start whose lookup succeeds
   resolves it; a failure while one is unresolved keeps its identity and read state; a failure after a resolved one the
   user has not cleared reopens it quietly, keeping its read state; one the user cleared while resolved is raised again
   as a new notice at the next failure.
7. A start whose lookup succeeds behaves exactly as today except for the PATH the app, its helm and the supervisor use.
   The lookup's tests, the process-wide apply and a `host_child_env` test (given a session base PATH, the session's PATH
   is that base and the variable is absent; given none, today's result) are the evidence for this; whether signing in
   from a real Dock or Finder launch now finds a program in `/usr/local/bin` cannot be checked unattended (the live
   install is off-limits and devbuild is the maintainer's flow), so the report asks the maintainer to confirm it on a
   devbuild.
8. SPEC.md "Security" defines "the user's PATH" as the PATH the user's login shell sets, read once when the app starts,
   used for everything the app, its helm and a supervisor it starts run by name but never as a session's starting PATH,
   and says what happens when it cannot be read. SPEC.md "Status" describes the new notice beside the trash and
   session-limit notices. SPEC_impl.md records the mechanism, why it is process-wide (it covers every present and future
   program started by name, including ones in vendored or third-party code, with no per-call-site rule), and why
   sessions keep their own starting PATH (a login shell's startup is not idempotent over its own result; see "Where it
   stands"). The comment in `SpawnInputs::spawn` ("The supervisor inherits this process's PATH unchanged") is rewritten
   to match.

## Decisions already made

Settled with the maintainer on the 2026-11-20 planning page:

- **Platforms:** macOS and Linux.
- **Shell mode:** login and interactive, like sessions.
- **On failure:** keep the PATH the app started with and log a warning, and also raise a rail-bell notice as the
  maintainer's words above describe.
- **What to take:** PATH only, not the shell's whole environment.
- **The local supervisor** (settled on a follow-up page after the planning review): the rule splits by what a program
  is. Everything Farhelm runs itself, the local supervisor's own lookups included, uses the login PATH; a session
  already runs the user's login shell itself, so it keeps starting from the PATH it starts from today. The maintainer
  picked this over keeping the supervisor on the app's original PATH (sessions right, but the supervisor's own tools,
  such as a Homebrew Podman, off the rule) and over letting it inherit the login PATH (its tools right, sessions
  starting from the login shell's result).
- **Release note:** the entry under `## Release note`, as proposed on the page.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.

Planner decisions shown on the page without objection: the lookup happens once at startup and makes the result the app
process's PATH (rather than passing a PATH to each program it starts); the app waits for the lookup before it opens its
window, bounded by the 5 seconds; a supervisor already running keeps its PATH until it restarts; the bare "Connection
closed" message itself is out of scope.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the two quotes above, and the decisions above.

**Binding repository constraints:**

- SPEC.md "Security" (the PATH rule), "Status" (rail bell behavior), and the environment contract that sessions behave
  as if the user had SSHed in (which this plan must not change).
- `.agents/test-authoring.md` and the root rule against tests that modify the test process's environment variables: the
  lookup and its fallback take their inputs by injection, and only the startup code applies the result.
- The workspace is on Rust edition 2024, where `std::env::set_var` is `unsafe` and sound only while no other thread can
  read or write the environment. `farhelm_desktop::run` starts no thread before `DesktopBootstrap::start` today
  (`init_tracing` writes to stderr synchronously; the first thread is the tokio runtime inside `start`), so the PATH
  must be applied before the first long-lived thread: before the tokio runtime, the supervisor spawn, the helm thread
  and the signal thread. A helper thread that has been joined does not violate this, but none is needed: use
  `std::process` and a `try_wait` loop like `check_program_runs`'s, not tokio. Keep a `SAFETY` comment that says which
  threads cannot exist yet and why; `hold_own_version`'s `remove_var` in `crates/farhelm/src/main.rs` is the precedent.
- Root `AGENTS.md` "Conventional Commits", "Changelog fragments", "Tests and validation"; `.agents/validation.md`.

**Planner proposals** (verified against the code):

- **Reuse the supervisor's shell choice.** The desktop crate already depends on `farhelm-supervisor`. Build the choice
  from `launch::resolve_shell_from` and the existing synchronous `passwd_shell_for_euid` in
  `crates/farhelm-supervisor/src/launch.rs` (made public, or wrapped in a small blocking sibling of `resolve_shell`), so
  the lookup uses the shell sessions use. Do not copy the async path's `getent` step: it exists because the supervisor
  is a static binary that cannot load the host's name-service modules, which the dynamically linked desktop binary can.
  The passwd lookup sits outside the 5-second bound; that only matters when `$SHELL` is unset, and the module docs say
  so rather than adding a bound.
- **One small module in `crates/farhelm-desktop`** for the lookup: a function that takes the shell path and the timeout
  and returns the PATH or one of the three failures. The shell path is the test seam; there is no separate command
  runner abstraction, so the tests exercise the real spawn, wait, kill and read code. Tests use fake shell scripts in a
  temporary directory: one whose startup prints a lot of noise, one that leaves no PATH, one that leaves an empty value,
  one that cannot be started, and one that hangs past an injected short timeout and leaves a child in its group that
  must not survive.
- **Failure types without churn.** The three failures are a desktop-local type, used for the log. The helm only learns
  that the lookup failed or succeeded (a `HelmArgs` field), so PR 2 adds no field-carrying type to `farhelm-proto` and
  moves nothing PR 1 built.
- **Carry the result into the helm like `devbuild`:** a `HelmArgs` field set through `embedded_helm_args`, written in
  `serve_with_ready` before serving, for a new app-wide unit `NoticeContent` kind (host NULL, condition key `''`), like
  `FleetSessionLimit`. Split the body of `reconcile_session_limit` into a function that takes the kind and content, and
  keep `reconcile_session_limit` as its caller, rather than writing a modelled copy: the raise, resolve, quiet-reopen
  and re-raise-after-clear rule is one rule. The test template is
  `devbuild_startup_only_seeds_an_unset_remote_update_choice` and the desktop's
  `a_helm_started_with_an_ensure_hosts_file_lists_its_hosts`.
- **The schema step.** The notices table's CHECK constraint names the kinds, so the new kind needs a table rebuild and a
  version step. The earlier session-limit step (`upgrade_session_limits`) rebuilds from the current `notices::SCHEMA`
  text, not a frozen copy, so editing that text in place would give an old database the new shape already at that step.
  Either freeze today's text as a named historical constant for the earlier step (as `TRASH_ONLY_SCHEMA` was), or make
  the new step detect that the shape is already current; do one of the two, and cover the upgrade with a test like the
  existing migration tests in `notices.rs`.
- **Rail entry beside `session_limit_entry`**, with the same focus return. Extract the Send feedback button (closing the
  panel and calling `feedback::open_returning`) into a helper both entries call rather than copying it; folding the two
  entries into one parameterized entry is your call. Test it beside
  `session_limit_notices_explain_the_two_lists_and_open_feedback` in `crates/gui/farhelm-gui/src/rail_notices/tests.rs`.
  The rail notices golden scenes (`crates/gui/farhelm-gui/src/goldens/other_dialogs.rs`) show no session-limit notice,
  so do not add this one there either; no golden re-capture is expected.
- **Test harnesses that launch the desktop binary** (`scripts/gui-agent.py`, `scripts/gui-lifecycle.py`,
  `scripts/devbuild-mac.sh`, `scripts/boat/sandbox/run-gui.sh`, `scripts/test-worker/`, and any Rust test that starts
  `farhelm-desktop`) may rely on the PATH they pass. `gui-agent.py`'s staged runs set a private HOME, PATH and SHELL and
  a profile that puts the staged `ssh` shim first again after the system profile (`docs/gui-agent.md`); check that a
  staged run still finds the shim first, and that the other harnesses still work, by reading them and running what
  `.agents/validation.md` says covers them.

## Outline

Two PRs, in this order:

1. `plan/desktop-login-shell-path/01-login-shell-path` (`fix:`): the lookup module and its tests, applying the result at
   the top of startup with the warning log, the supervisor's separate session starting PATH (criterion 3: the spawn side
   in `SpawnInputs::spawn`, the reading side and `host_child_env` in the supervisor, with the `host_child_env` test),
   the SPEC.md "Security" and SPEC_impl.md text, the rewritten comment in `SpawnInputs::spawn`, any harness adjustment
   the check above needs, and a changelog fragment. The session part is in this PR, not a later one, so no PR in the
   stack ever starts sessions from the login PATH.
2. `plan/desktop-login-shell-path/02-path-notice` (`fix:`): the notice kind in `farhelm-proto`, the helm's schema step
   and reconcile at startup, the `HelmArgs` field, the rail entry, their tests, the SPEC.md "Status" paragraph, the
   fragment from PR 1 extended to mention the notice (an edit of the unreleased fragment, not a second entry, per root
   `AGENTS.md`, "Changelog fragments"), and the TODO.md entry removed.

## Fallbacks

- If something starts a thread before `DesktopBootstrap::start` after all (a library initializer, the agent feature),
  move the lookup and the apply earlier, before that thread, and log it. If no point before every thread exists, do not
  apply the PATH with `set_var` from a multithreaded process; block with the tradeoff (pass the PATH explicitly to every
  spawn instead, which misses programs started inside third-party code).
- If an interactive shell with stdin closed misbehaves in a way the markers and the timeout do not cover (it stops
  itself waiting for a terminal, say), adjust how it is started so it still reads the user's interactive startup files,
  and log it. Dropping `-i` is not allowed: it is the maintainer's choice.
- If a session's environment is built anywhere besides `host_child_env` (another launch path, a restart or adoption
  path), the session starting PATH applies there too; find every such place before writing criterion 3's code, and log
  the list.
- If a test harness cannot keep working with the replaced PATH by adjusting the harness itself (its staged profile, the
  PATH its shell startup sets), a narrow way to skip the lookup that only harnesses use is within scope; name it as
  test-only in SPEC_impl.md and log it.
- If the new notice needs a schema step that collides with another plan's step at rebase, renumber yours
  (`.agents/careful-rebase.md`).

Decisions to log: the exact shell command and how it hands back its PATH, how the hung shell and its group are killed,
where the blocking shell choice lives, the name and shape of the supervisor's session starting PATH, the notice kind's
name, its wording, how the schema step avoids the in-place `SCHEMA` pitfall, and every review finding declined with its
reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-desktop-login-shell-path-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-desktop-login-shell-path-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/desktop-login-shell-path/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves or extends the fragment its type calls for, describing in user-facing words
  what was built. Where what got built departs from the release note entry, the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. The bare
"Connection closed" message is one such follow-up: mention it in the report.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

The desktop app now runs ssh and other programs with the PATH your login shell sets, even when you start it from the
Dock or Finder. Programs your ssh config runs, such as a `ProxyCommand` in `/usr/local/bin`, are now found; before,
signing in to such a host failed with "Connection closed".
