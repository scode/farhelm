# Remote uninstall: remove Farhelm from a remote host from the host menu

Written against main at 5baee365 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says; where this plan changes behavior they describe, it changes them in the same PR.

## The goal

`farhelm uninstall` removes Farhelm only from the Mac it runs on, and nothing removes it from a remote host. The docs'
"Uninstall Farhelm" page (`website/src/content/docs/docs/get-started/uninstall.md`, "From a remote host") tells users to
stop the host's sessions and then run on the host, by hand:

```
systemctl --user disable --now farhelm-supervisor.service
rm ~/.config/systemd/user/farhelm-supervisor.service
systemctl --user daemon-reload
rm -rf ~/.local/lib/farhelm
```

and then remove the host from the list. Add a supported way to do this from Farhelm, and replace those manual steps in
the docs with it.

Acceptance criteria, as a user sees them:

- A remote host's ⋯ menu in the hosts panel has an **uninstall** item (with a description line, like the other items),
  above or beside **remove** as fits the menu's existing grouping. The local host has none: removing Farhelm from the
  Mac is `farhelm uninstall`. The item is unavailable while the host is busy with setup, an update, or another
  uninstall, the same way Update is.
- Choosing it plans the uninstall over ssh and shows a confirmation, rendered from the plan like setup's, naming what
  will be removed on the host (the supervisor's user service, its unit file, the `~/.local/lib/farhelm` directory with
  the binary and any private tmux it holds, all as the concrete paths the plan found) and what is kept (the host's
  Farhelm data directory, named by its path, with a note that deleting it by hand removes the data too). Nothing on the
  host changes until the user confirms.
- Uninstall is refused, with nothing changed and a message naming the reason, when:
  - the host has any session that has not ended, or any session with a live terminal tab; the message names them (by
    title, as the session list shows them) and tells the user to stop them first. A session whose state is unknown
    counts as running. A session list the helm got only part of (truncated) refuses as well;
  - the host is not connected (ssh fails, no supervisor answers, a protocol mismatch, an identity problem), because
    Farhelm cannot check its sessions; the message says to get it connected first. The one exception is the retry below;
  - the host's supervisor unit carries `farhelm helm setup`'s managed-by marker (setup owns it there, the same refusal
    setup and Update give);
  - the supervisor binary the host is actually running from is not the one in Farhelm's own lib directory (an
    unsupported setup per SPEC.md "Supported host setup"; a clear refusal is enough).
- After confirmation, the host row shows progress like Update does. On success the host disappears from the host list
  (its cached sessions go with it, as with remove), and the window that confirmed shows a one-line notice that Farhelm
  was removed from the host and where its data remains, built from the plan it already holds.
- If the run fails partway, the row stays, its step list shows what completed and what did not, and choosing uninstall
  again continues: each step tolerates its earlier completion. A retry is allowed even though the supervisor no longer
  answers, but only when the host has no supervisor unit file left (so nothing can start it again); it then plans only
  the removals still outstanding. Otherwise an unconnected host is refused as above.
- Data stays: the host's state directory is never touched. Linger is left as it is, as local uninstall leaves it.
- The docs' "From a remote host" section describes the menu action only, with no manual commands; for a host Farhelm
  cannot connect to, it says to fix the connection first. `docs/install_uninstall.md`'s opening sentence, which says
  removal from provisioned hosts is "a separate operation", points at it. The Manage hosts page mentions the menu item.
- SPEC.md and SPEC_impl.md describe the operation; the Near term TODO.md entry "Uninstall Farhelm from remote hosts" is
  removed; a changelog fragment exists.

## Decisions already made

Requirement sources are kept apart: the maintainer's words, binding repository rules, and planner proposals.

The maintainer's request and answers:

- M1. The TODO.md entry (Near term), verbatim: "Uninstall Farhelm from remote hosts. `farhelm uninstall` only removes
  Farhelm from the Mac it runs on, and nothing removes it from a remote host: the docs' "Uninstall Farhelm" page tells
  users to stop the host's sessions and then run `systemctl` and `rm` by hand on the host. Add a supported way to
  uninstall from a remote host, and replace those manual steps in the docs with it."
- M2. Surface: an item in the remote host's ⋯ menu; the helm does the work over ssh, like setup and Update. No CLI verb.
- M3. Refuse while the host has running sessions or live terminal tabs, naming them; the user stops them first. Nothing
  is killed.
- M4. Keep the host's data, as uninstall on the Mac does; say where it is.
- M5. On success the host is removed from the host list in the same action. On a partial failure the row stays, showing
  what is left, and the action can be retried.
- M6. Docs: replace the manual steps fully. For a host Farhelm cannot connect to, the page says to fix the connection
  first; no manual commands remain.
- M7. Review gate: two fresh-context reviewers per PR, a Claude Opus 5.5 agent at high effort and a gpt-6-astra agent at
  high effort, both with the general review charter (below). No review swarm.
- M8. No-workhorse mode (below).

Binding repository rules:

- SPEC.md: standard operation must never require falling back to ssh or a separate command line; remote provisioning
  states exactly what it will do and proceeds only on confirmation; "Ownership during cleanup and provisioning": a unit
  carrying setup's managed-by marker belongs to setup and provisioning refuses to touch it "both when planning and at
  the moment of writing", an unmarked unit belongs to provisioning, and provisioning "does not try to tell [a
  hand-written unit] apart from its own"; "Supported host setup": other setups are best-effort and a clear refusal is
  enough; "Uninstall scope and interaction": `farhelm uninstall` does not contact registered hosts (stays true; it is a
  different command); "Install and uninstall": `docs/install_uninstall.md` must stay accurate.
- SPEC_impl.md "Provisioning": the plan is retained behind an opaque one-use confirmation id; consuming it revalidates
  the facts the plan relied on; discovery records the resolved binary and state directory; the remote unit write repeats
  the setup-marker test in the same shell command.
- Root `AGENTS.md` "The live install is off-limits": no test or manual step may stop, disable or delete the maintainer's
  own `farhelm-supervisor.service` or `~/.local/lib/farhelm`. See P7.
- Root `AGENTS.md`: Conventional Commits; changelog fragment for a `feat` PR; TODO entry removed in the PR that
  addresses it; "Finishing work"; `.agents/test-authoring.md` for test changes; `python -B scripts/check-test-sleeps.py`
  when Rust or browser tests change; browser specs on Chromium and WebKit through the recorder; `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md` before editing the docs site.
- If `plans/queue/hover-help.md` has landed by the time this runs, every new clickable control needs hover text per its
  coverage test; menu items with a description line are exempt there.

Planner proposals (with reasons; change them if the code says otherwise, and log a DECISION). File and symbol names are
from main at 5baee365 and may have moved:

- P1. Reuse the provisioning plan/confirm/run machinery as a third operation (`ProvisioningOperation` in
  `crates/farhelm-helm/src/provisioning/plan.rs` has Add and Update). It already provides one-use ids with revalidation
  at consumption, the host busy set (so uninstall cannot run beside an Update and `update all` skips it), per-step
  progress in the row, the reach record from `inspect` (home, the manager's real unit directory, the setup-marker
  refusal), and the route shape of `POST /api/hosts/{id}/update` (no body plans, a `probe_id` body consumes). Do not
  build a parallel confirmation template: give `ProvisioningPlan::confirmation()` an operation-aware heading (it
  hardcodes "Farhelm will set up …") and confirmation lines naming the removals and the kept state directory, and give
  the setup-marker refusal operation-specific wording. Extend any filter that names only Add/Update pending
  confirmations (for example `forget_host`'s) to the new variant.
- P2. Step order keeps the supervisor answering as long as possible: disable the unit (without `--now`), remove the unit
  file (re-checking the setup marker in the same remote shell command, as the unit write does), daemon-reload, then stop
  the unit, then remove the lib directory. The supervisor unit uses `KillMode=process`
  (`crates/farhelm-helm/src/units.rs`), so stopping it kills no sessions in any order. This ordering is what makes M5's
  retry possible: until the unit file is gone the host is still connected and the ordinary checks apply, and once it is
  gone nothing can restart the supervisor, so the retry rule in The goal is safe. Do not add process-local "this run got
  past the stop" memory; it would not survive a helm restart.
- P3. Session check: at planning and again when the confirmation is consumed, read a fresh session list through the
  host's connection (not the periodic snapshot), require the connected state and the identity checks Update planning
  uses (`require_update_trusted` at planning time), refuse on a truncated listing, and treat any session that has not
  ended (`!has_ended()`, not `is_live()`, so `Unknown` counts) or that has a live terminal tab as running. No locking
  against a session started between the check and the stop: with `KillMode=process` such a session survives, unmanaged,
  which matches local uninstall's stance ("neither forcibly terminates"); SPEC.md says the check runs at confirmation.
- P4. Binary check: take the path the host is actually running from the same probe Update planning uses (its dial path),
  not the row's possibly-null `remote_farhelm`, and refuse unless it is inside the plan's lib directory. Do not add a
  check that the unit is "provisioning's own" beyond the setup marker; SPEC.md rules that out.
- P5. Removing the row: the run cannot call the existing remove path. `remove_host_owned` in
  `crates/farhelm-helm/src/hosts.rs` starts by taking the host's provisioning lock, which the run already holds (a 409),
  and `ProvisioningService::forget_host` aborts the host's task, which is the run itself. Give the run its own short
  final sequence under the lock it holds: take the host write lock, remove the registry row through the store, purge the
  host's runs, plans and busy entry without aborting the current task, then stop the host's connection actor and forget
  its cache lock. Factor shared pieces out of the remove path rather than duplicating them.
- P6. The success notice comes from the confirming client, built from the confirmation it holds, when the row disappears
  after a successful run. The helm keeps no result for a deleted host (`GET /api/hosts/{id}/provisioning` 404s once the
  row is gone); do not add retention for it.
- P7. Backends: the new remove actions need only an ssh implementation. Uninstall refuses the local row at planning (the
  same gate Update uses), so the local backend's arm returns an error rather than growing an unreachable implementation
  and its tests. The browser stack's injected backend (`crates/farhelm-helm/src/provisioning/e2e.rs`) needs only the new
  action labels routed through its existing action handling. Every path the run removes (unit file, unit name, lib
  directory) comes from the plan, frozen at planning from the same layout overrides install uses (`PlanLayout`), never
  re-derived from the host's home at run time: the real-transport test runs these steps on the machine executing it, and
  a re-derived path would hit the real `~/.local/lib/farhelm` and the real `farhelm-supervisor.service`, which may be
  the maintainer's live install.
- P8. Tests:
  - Helm unit tests on the fake backend: planning refusals (running session, unknown-state session, live tab only,
    truncated list, unconnected, setup-marked unit, binary outside the lib directory, local row), the confirmation's
    content, step order, row removal at the end, and a retry after a failure past the unit-file removal.
  - The real-transport case in `crates/farhelm-helm/src/provisioning.rs` (`real_provisioning_case`: real ssh to
    localhost with a tempdir layout and a nonce unit name) gains an uninstall leg after its Update, asserting the unit
    and the lib directory are gone and the state directory is kept. Before it confirms, the test asserts that the plan's
    paths and unit name are its own temporary ones.
  - A browser spec in `e2e/tests/terminal-multihost.spec.ts` (its remote is a real connected supervisor; the hosts in
    `provisioning.spec.ts` never connect, so the precondition cannot pass there): menu → confirmation shows the kept
    data path → confirm → row gone and notice shown, with the injected backend absorbing the remove steps (so the real
    remote supervisor keeps running), ending with the file's existing fleet-row restore. The running-session refusal
    stays a helm unit test unless the UI renders it differently from the existing Update-refusal path.
- P9. Stack: PR 1 helm operation, API, SPEC.md and SPEC_impl.md, with its tests; PR 2 UI, docs pages, browser spec,
  changelog fragment (`added`), TODO removal. Types: `feat` for both, or `feat` for the UI PR only if the helm PR has no
  user-visible effect on its own; decide and log it.

## Implementation outline

Helm: a third provisioning operation in `crates/farhelm-helm/src/provisioning/` (plan, service, backend, http, e2e
backend) and its route, plus the row-removal sequence (P5). UI: the menu item and confirmation in
`crates/farhelm-ui/src/hosts.rs` and `crates/farhelm-ui/src/provisioning.rs`, the API client call, the notice. Docs:
`website/src/content/docs/docs/get-started/uninstall.md`, `website/src/content/docs/docs/using/manage-hosts.md`,
`docs/install_uninstall.md`, SPEC.md (next to remote provisioning, or a short subsection under "Install and uninstall"
that says how it relates to `farhelm uninstall`), SPEC_impl.md "Provisioning". No supervisor, protocol or persistent
schema change; no new remote command on the host's own binary (the helm drives everything over ssh, so it works whatever
version the host runs).

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-remote-uninstall-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/remote-uninstall/<nn>-<short-name>`.
- Shape the stack per P9. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted in a
  later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits; a `feat` PR carries its changelog fragment under `releasing/changelog.d/` in the same commit,
  per root `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- Remove the Near term TODO.md entry "Uninstall Farhelm from remote hosts" in the last PR.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, nextest selections of
the helm's provisioning and hosts modules and of `farhelm-ui`'s hosts and provisioning modules, the real-transport
provisioning case (read its retained output: an early-return `SKIPPED` is not evidence that the ssh and systemd
substrate ran), `python -B scripts/check-test-sleeps.py`, `dprint check` on changed files,
`cd website && bun install --frozen-lockfile && bun run build` for the docs pages, and the new browser spec on Chromium
and WebKit through the recorder. Say in the report which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (M7): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at
high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm.
Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new verb on the host's own `farhelm`, a CLI
command, killing or stopping sessions, deleting the state directory or touching linger, a parallel plan or confirmation
mechanism, helm-side retention of results for deleted hosts, a lock against concurrent session creation, a process walk
on the host; these are examples, not a blacklist), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the request, the decisions above,
this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the route and wire shape, the step order and retry rule as built, how the session
check reads its list, the row-removal sequence and what it shares with remove, the menu placement and wording, the
confirmation and notice text, the SPEC wording, the commit types, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. There is no agreed fallback beyond the planner proposals above. Examples that need the
maintainer: the machinery turning out not to fit a third operation without a large refactor, a host state in which
neither the ordinary checks nor the retry rule can decide safely, or a required behavior that cannot be tested without
touching the machine's real install. Record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan,
step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this
plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
