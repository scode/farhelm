# Show how to start a 2FA host's ssh connections by hand

Written against main at aa0cb0c4 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan. The plans `release-notes-menu-item.md` and `intro-tour.md` also add items
to the `?` menu; whichever lands later rebases onto the others (see the menu order below).

## The goal

A host that asks for a second factor or another approval on every ssh login works with Farhelm only on a best-effort
basis (SPEC.md "Supported user environments"): the helm runs ssh without prompting, so the user has to start the helm's
two shared ssh connections to that host by hand, one for the host's sessions and one for setup and updates, at the
helm's own socket paths. Today that means knowing Farhelm's internal socket naming, and the docs say they do not cover
it. Make it discoverable: the UI shows, per host, the exact `ssh` commands that start both connections, composed by the
helm so they carry its socket paths and its connection settings.

The design was settled on a mockup the maintainer approved: https://snippets.scode.org/s/farhelm-ssh-2fa-help/. It is
described in words below, which win where the two differ; the mockup's command lines are stand-ins.

### Acceptance criteria

- **The `?` menu item.** A new item labeled `ssh needs 2FA?`, description line
  `start a host's ssh connections yourself`, with hover text in the style of the other bar menu items, for example
  `ssh needs 2FA?: for a host that asks for a second factor or other approval on every ssh login, the commands that start Farhelm's ssh connections to it by hand`.
  Offered in the web UI and the desktop app alike. Order: send feedback, documentation, release notes (if that item is
  on main), **ssh needs 2FA?**, welcome (if that item is on main), then check for updates where it is offered. The `?`
  button's own hover text, which lists the items, includes it. Keyboard behavior of the bar menus covers it like the
  others.
- **The dialog** it opens, titled `ssh needs 2FA?` with the same summary line, follows the existing host dialogs' look
  and modal behavior (Escape, focus return, backdrop). Its body:
  - An explanation, in this sense: Farhelm connects to hosts without prompting, so it can't answer a second factor or
    other approval a host asks for on every ssh login; for such a host, run both commands below in a terminal **on this
    Mac** (desktop app; the web UI says **on the machine the helm runs on, as the user it runs as**, which may not be
    the machine the browser runs on: `%C` hashes the connection as the running user's ssh config resolves it, and the
    sockets live in the helm's private state directory), before Farhelm needs the host; each asks for approval once,
    then keeps its connection open in the background for Farhelm to use; when a connection drops, run its command again.
  - A recommendation, in the maintainer's words' sense: take a screenshot of this dialog and ask an agent to write a
    script that runs these commands for you. (Maintainer's words: "lets explicitly tell the user we recommend taking a
    screenshot of this and asking an agent to set up a script to automate this for you".)
  - One section per registered ssh host (the local host is left out), headed by its name and destination, with two
    labeled commands, `sessions connection` and `setup and updates connection`, each with a copy button using the UI's
    existing clipboard path (native bridge first in the desktop app).
  - With no ssh hosts registered, a line saying so that points at add host.
- **The commands.** Composed by the helm, never by the UI, from the helm's own state directory and the same building
  blocks its own ssh connections use (the connection overrides, the control socket naming for each of the two shared
  connections, the ControlPath quoting), so they cannot drift from what the helm does. Each command starts a master that
  prompts as ssh normally would (no `BatchMode`), backgrounds itself once connected, and keeps running; it carries every
  connection override (in particular `ClearAllForwardings=yes`, because port forwards belong to the master and a master
  started from the user's own ssh config would otherwise put that config's forwards on Farhelm's connection, where no
  client can cancel them), the helm's ControlPath for that connection, and the destination exactly as registered, after
  `--`. Every argument is quoted so that a paste into a POSIX shell passes it to ssh unchanged, whatever the path or
  destination contains: the ControlPath value needs two layers, `ssh_control_path_option`'s ssh-config escaping and then
  `shell_quote` (`farhelm_proto::text::shell_quote`, re-exported in `ssh.rs`) on top.
- **Verified against real OpenSSH, once, by hand.** Against a private sshd you start yourself (or `ssh localhost` if it
  works without a password), with key authentication standing in for a second factor: start a master with a composed
  command, confirm it with `ssh -O check` on the helm's ControlPath, then run the helm's own argv for that host and
  connection (from `ssh_base_args`) and confirm it reuses the master with no new authentication in the sshd log. Record
  what you ran and saw in the report. Do not add this to `scripts/test-provision-centos.sh` or any other release-gate
  harness: SPEC.md says Farhelm need not add complexity for such hosts. Unit tests cover the composition.
- **No socket.** Three cases mean a connection cannot be shared from this helm, and the cause is the helm's state
  directory, not the host, so the dialog says so once at the top, in place of the commands it cannot give: the path is
  too long for a unix socket (`control_socket` returns nothing; name the connection, the path and the limit that was
  hit, as SPEC.md asks, computed from the same constants `control_socket` uses: the directory plus a separator plus 40
  for `%C` plus `MASTER_TEMP_SUFFIX`, one more for the setup connection's `p`, against `MAX_SOCKET_PATH`); the path
  contains `${` (`control_socket` returns nothing; no limit, ssh would expand it); the path is not UTF-8
  (`ssh_base_args` returns an error naming it rather than `None`). Each says that hosts needing 2FA cannot use that
  connection from this helm. The setup connection's limit is one character lower, so it can be the only one affected.
  The mockup's sentence about the limit ("needs 40 more") is wrong arithmetic; do not copy it.
- **Adding a host.** The add-host dialog gets a line under its existing help text:
  `Does this host ask for a second factor or other approval on every ssh login? Start its connections yourself first.`,
  the second sentence a link that opens the same dialog with an extra section at the top, `the host you're adding`,
  holding the two commands for the destination currently typed, so the user can start both before pressing add (the
  check add runs would otherwise fail on such a host). With the field empty, that section asks for the destination
  first. Closing the dialog returns to the add-host dialog with what was typed intact. A typed destination is checked
  with `store::destination_is_usable` (non-empty, no leading `-`, no NUL) before any command is composed for it, for a
  clear error; not the full registration path, whose duplicate and alias checks do not apply. The `--` before the
  destination is the actual guard. (The check add runs uses the setup connection, so starting both masters first does
  make add work.)
- **API.** The helm serves the commands through an authenticated endpoint the dialog calls, taking an optional extra
  destination for the add-host case. The helm's state directory still does not appear as a field on `/api/hosts`
  (`crates/farhelm-ui/src/hosts.rs`, `state_remedy`'s comment, explains why the UI never learns it); helm-composed
  command text that contains the path follows the same precedent as the helm's local-supervisor remedy.
- **Specs and docs.** SPEC.md "Feedback" (which lists the `?` menu's items) and SPEC.md "Supported user environments"
  (the 2FA paragraph, which should now point at the dialog) are updated; SPEC_impl.md describes the endpoint and that
  the commands come from the same code as the helm's own ssh arguments. The docs page
  `website/src/content/docs/docs/get-started/add-a-remote-host.mdx` stops saying the docs do not cover 2FA hosts and
  explains the dialog, and `website/src/content/docs/docs/using/send-feedback.md`, which lists the `?` menu's items,
  names the new one; follow `website/AGENTS.md` and `website/EDITORIAL_RULES.md`. Never edit `release-notes.mdx` or
  curated `CHANGELOG.md` sections.
- **Tests.** Rust unit tests of the composition (both connections, every override present, quoting of awkward paths and
  destinations, `--` before the destination, the no-socket cases with their limits); a helm test of the endpoint
  (registered ssh hosts listed, local host left out, extra destination validated); Playwright on Chromium and WebKit for
  the menu item and its order, the dialog's host sections with a copy control per command, the empty state, and the
  add-host link with a typed destination. Update what already lists the `?` menu's items: `e2e/tests/feedback.spec.ts`
  asserts their order, and `e2e/tests/tooltip-coverage.spec.ts` hard-codes the help menu's item count (`2` today) in its
  `expectCovered` call.
- A changelog fragment (`kind: added`), as root `AGENTS.md` requires for a `feat` PR.
- The TODO.md entry ("Help users start the shared ssh connections themselves.") is removed in the last code PR.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: a `farhelm` CLI command printing the commands (the maintainer chose the `?` menu only); an item in each
host's `⋯` menu; starting the connections from Farhelm itself, a terminal integration, or any automatic retry; changing
how the helm's own ssh connections are built; changing the socket naming or its length limits.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Help users start the shared ssh connections themselves.** A host that needs an interactive approval for every new
> ssh connection (a second factor, say) only works if the user starts the helm's shared connections for it by hand, at
> the helm's socket paths, because the helm itself runs ssh non-interactively. Today that means knowing Farhelm's
> internal socket naming. Make it discoverable by printing the exact `ssh` command that starts each shared connection,
> carrying the helm's connection overrides and its `ControlPath`. Showing only the socket paths is not enough: a master
> started from the user's own ssh config puts that config's port forwards on Farhelm's connection, where no client can
> cancel them. Say so when a host has no socket at all because the state directory's path is too long.

The maintainer's words with it: "for ssh master command i'm open to other ideas but for now i'd say ? menu -> ssh needs
2FA?", and then: "oh we should also do something on initial host add flow. like a "Does this host need 2FA / interactive
prompt to ssh? See...."".

**The user's decisions (2026-10-14):**

- The `?` menu only (no host `⋯` menu item, no CLI command), plus the add-host line.
- The mockup's design, approved "as mocked", with the addition of the screenshot-and-agent recommendation quoted above.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Supported user environments" (2FA hosts are best effort: the user starts
both shared-connection sockets for the destination the host was registered with; Farhelm need not add complexity for
such hosts; limits that are hit are named); SPEC.md "Feedback" (the `?` menu); SPEC_impl.md's ssh transport and
connection-override sections; the helm's state directory not being exposed on `/api/hosts`; root `AGENTS.md` "Talking to
the user", Docs website, Conventional Commits, Finishing work, Releases and the changelog; `.agents/test-authoring.md`
for every test change; `plans/AGENTS.md`, Executing one plan.

**Planner proposals:** the exact command form (`-M`, `-N`, `-f` or equivalents), the endpoint's shape, the hover text,
the connection labels, the add-host section, and the PR shape. The wording in the acceptance criteria may be tightened,
but keep its sense; correct anything untrue and log it.

## Outline

Line numbers drift; find the code by name.

- **Helm, composition.** `crates/farhelm-helm/src/ssh.rs` holds `ssh_base_args`, `CONNECTION_OVERRIDES`,
  `options_with_overrides`, `SharedConnection` (the supervisor connection, socket `%C`, and the provisioning connection,
  socket `p%C`), `control_socket` (with the length limits `MAX_SOCKET_PATH` and `MASTER_TEMP_SUFFIX`),
  `ssh_control_path_option` and the re-exported `shell_quote`. Add a function beside them that composes the user-run
  master command for a destination and a connection, or reports why there is none, reusing those pieces; `%C` stays a
  literal token, so nothing has to compute the hash. Its module docs (which already explain 2FA and port forwards) gain
  a pointer to it.
- **Helm, API.** A small authenticated endpoint next to the hosts routes in `crates/farhelm-helm/src/lib.rs` that lists
  the registered ssh hosts with their two commands, plus the optional extra destination, validated like registration
  (`store::destination_is_usable`).
- **UI.** `crates/farhelm-ui/src/app_bar.rs`: a new `BarMenuItem`, inserted into `BAR_MENU_ITEMS` at the stated place
  (that array holds both bar menus' items, so the place is before `CheckForUpdates`), offered by `help_menu_items`, its
  click opening the dialog; both variants of the `?` button's hover text (with and without the updater). A new dialog
  component on the pattern of `settings.rs`'s `SettingsDialog` (or `feedback.rs`), using the existing clipboard helper.
  `hosts.rs`'s `AddHostForm` gets the line and link, opening the same dialog with the typed destination.
- **Specs, docs, fragment, TODO removal**, as listed.

Expected shape: one PR for the helm side (composition, endpoint, their tests, SPEC_impl.md), one for the UI (menu item,
dialog, add-host line, Playwright, SPEC.md, docs, fragment, TODO removal). Types `feat`.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`;
- the farhelm-helm tests touched, through the recorder (`ssh::` and the endpoint's);
- the by-hand OpenSSH verification above;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the new or changed specs, `feedback.spec.ts` and `tooltip-coverage.spec.ts`;
- `cargo check -p farhelm-ui --features desktop` (the dialog's desktop wording and clipboard path);
- the website build, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-ssh-2fa-help-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout. A private sshd
for the verification binds its socket and port per root `AGENTS.md` (random names, kernel-chosen ports).

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/ssh-2fa-help/<nn>-<short-name>` (for example `plan/ssh-2fa-help/01-master-commands`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is a new feature, so `feat`. The PR that makes the
  feature visible adds its changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md`
  (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a new ssh test harness, a change to
  how the helm builds its own ssh arguments, a CLI command, or a per-host menu item (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular: the command form and how it was verified, the endpoint shape, any wording
change, the no-socket message, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallback: wording above that turns out inaccurate is corrected to the truth. If the by-hand OpenSSH check cannot
be run on the executing machine (no sshd can be started, no passwordless `ssh localhost`), record that in the report as
a gap rather than building a harness.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: dropping the add-host line, showing only
socket paths instead of full commands, composing the commands in the UI, adding a CLI or per-host menu item.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
