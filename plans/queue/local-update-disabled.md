# No update for this machine: grey out the host menu's Update on the local row

Written against main at c8792fb3 on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `plans/queue/install-output-layout.md` and `plans/queue/update-while-running.md` (its `INDEX.md`
line says so). Those two rework the installer and how a Mac's own Farhelm is updated, and the maintainer expects them to
settle in the specifications that the desktop app is the only user surface. The text this plan puts on the greyed-out
item depends on what they landed, so read their merged changes to SPEC.md, SPEC_impl.md, `docs/install_uninstall.md` and
the installer's update message before writing any wording. Names below come from main at c8792fb3 and may have moved.

## The goal

In the hosts panel, every host row has a `⋯` menu. For a remote ssh host it offers **update** ("install the newer
Farhelm version"), which makes the helm update that host's supervisor over ssh. The local row ("this machine", the
helm's own machine) offers the same item today, but the helm refuses to update its own machine: planning returns "this
is the helm's own machine; run farhelm helm setup here instead of provisioning from the panel" (`LOCAL_SETUP_HANDOFF`
and `local_handoff_reason` in `crates/farhelm-helm/src/provisioning/service.rs`, reached through `plan_update`). That
error then sits under the local row with no way to dismiss it, and its advice is wrong for a machine whose supervisor is
already running and merely older.

After this plan, the local row's menu shows **update** greyed out (disabled, not hidden), with its description line
pointing at re-running the installer as the way to update this machine. Clicking or activating it does nothing: no
planning request is sent, so the refusal and its stuck error are never reached from the UI.

Acceptance criteria:

- On the local row, wherever the menu would otherwise have offered Update (the supervisor is set up and running, no run
  or plan is in flight, the host is not too new), the menu shows the update item disabled (`aria-disabled`, styled the
  way the menu already styles its disabled items), with a description that tells the user to update this machine by
  re-running the installer. Where the menu would not have offered Update (local setup hand-off showing, a run or plan in
  flight, too new), it does not show the disabled item either.
- Activating the disabled item by mouse or keyboard sends no request and changes no state.
- Remote ssh rows behave exactly as before, including the inline `↑ update` button and the header's `update all`.
- The local row still never gets the inline `↑ update` button and is still excluded from `update all` (both already
  true; keep them true and keep their tests passing).
- The helm side is unchanged: it keeps refusing a local update, with its current wording, for any caller.
- SPEC.md says what the local row's menu shows, user docs say how this machine updates where they describe the host
  menu, a changelog fragment exists, and the TODO.md entry is removed.

## Decisions already made

Requirement sources are kept apart: the maintainer's words, binding repository rules, and planner proposals.

The maintainer's request and answers:

- M1. The TODO.md entry (Near term, "No update for this machine"), verbatim: "Choosing update on the local host ("this
  machine") is not a supported flow: the helm refuses with "this is the helm's own machine; run farhelm helm setup here
  instead of provisioning from the panel" (`crates/farhelm-helm/src/provisioning.rs`), and that error then sticks under
  the host's row with no way to dismiss it. The message is also wrong for the case. The update action should be greyed
  out for this machine, in the host's pop-up menu and in the inline update button on host rows, so the refusal is never
  reached."
- M2. Greyed out, not hidden; the description line points at the installer ("Point at the installer"). Exact wording is
  yours, true to what the landed installer and update docs say.
- M3. On the helm's refusal, the maintainer said: "you should not be able to upgrade the helm like that at all." The
  helm already refuses and keeps refusing. Its wording is not changed by this plan: once the UI cannot reach it, only a
  direct API caller could see it.
- M4. This plan runs after the in-flight installer and updater work (`install-output-layout.md`,
  `update-while-running.md`), which may inform the wording and the spec text.
- M5. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter (below).
- M6. No-workhorse mode (below).

Binding repository rules:

- Host-kind decisions in the UI go through named predicates on `HostKind` in `crates/farhelm-ui/src/lib.rs`, each an
  exhaustive match (that `impl` carries `#[warn(clippy::wildcard_enum_match_arm)]`); do not compare kinds at the call
  site.
- Root `AGENTS.md`: changelog fragment for a `fix` PR; TODO entry removed in the PR that addresses it; "Finishing work"
  for validation; `.agents/test-authoring.md` for test changes; the website's `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md` before touching the docs site.

Planner proposals (with reasons; change them if the code says otherwise, and log a DECISION):

- P1. The inline button half of M1 is already done: `remote_update_available` in `crates/farhelm-ui/src/hosts.rs`
  requires `HostKind::updates_automatically()`, which is false for the local row, and SPEC.md (the host list paragraph:
  "Local hosts, hosts with update options still loading, ... retain the plain words") already says so. The test
  `inline_update_eligibility_requires_age_and_remote_availability` pins it. Nothing to build there beyond confirming it
  on the landed main. M1 literally says "greyed out ... in the inline update button"; showing a disabled inline button
  on the local row would be new UI that contradicts that SPEC.md passage, so it is not built. Record this as a DECISION
  naming the passage, so the report shows the literal wording was weighed.
- P2. Leave the menu model alone. In `ProvisioningMenuState::offered` (`crates/farhelm-ui/src/provisioning.rs`),
  `update == true` on the local row already holds in exactly the states the acceptance criterion names, so `MenuFacts`,
  `offered`, `ProvisioningMenuState`, `host_menu_order` and `has_provisioning_menu` need no change, and disabled items
  already stay in the menu's keyboard order. Add one exhaustive `HostKind` predicate in `crates/farhelm-ui/src/lib.rs`
  (the UI's counterpart of the helm's `HostKind::panel_updates` in `crates/farhelm-helm/src/store.rs`: ssh true, local
  and unrecognized false); do not reuse `updates_automatically`, which answers a different question. Correct the doc on
  `offers_provisioning_actions`, which today says the local row is offered Update even though the helm refuses it, and
  note on `remote_update_available` that its kind guard is now what keeps the local row out of the inline button and
  `update all`.
- P3. Rendering in `HostRow` (`crates/farhelm-ui/src/hosts.rs`): the existing update item becomes disabled when the new
  predicate is false, through the `aria_disabled` attribute and the `if update_disabled { return; }` click guard it
  already has, and its description line comes from the same predicate. The disabled item also shows on an up-to-date
  local row (Update is offered on any running host that is not too new), so the description must not imply an update is
  waiting. If a pure test is wanted, extract a small function from host kind and planning to (disabled, description)
  rather than extending `offered`.
- P4. One PR, `fix:`, carrying the code, Rust tests, the SPEC.md sentence, the docs page change, the changelog fragment
  (`kind: fixed`), and the TODO.md entry removal. SPEC.md: a sentence beside the host-list paragraph saying the local
  host's menu shows Update unavailable with the installer as the remedy. Docs:
  `website/src/content/docs/docs/using/
  manage-hosts.md` says the `⋯` menu can "update Farhelm"; add that this machine
  is updated with the installer, linking the install page if the site has one. If the landed work already changed these
  passages, fit the change to them.

Agreed fallback: if the landed specifications still support a helm machine on which re-running the installer is NOT how
it updates (for example a Linux helm machine while the installer refuses Linux), a description pointing at the installer
would be wrong there. Do not invent a per-platform text; block and ask, with the passages that conflict.

## Implementation outline

Small and localized: one `HostKind` predicate and a render-time change to the existing update item (P2, P3). No change
to the menu state model, no new persistent state, no helm or protocol change, no new HTTP surface. Tests: a render test
that on the local row the item is present, `aria-disabled="true"`, carries the installer description, and activating it
by click or keyboard publishes no `ActionRequest`, while an ssh row's item is unchanged; the existing inline and fleet
tests kept. Update any existing test that asserted the local row is offered an enabled Update.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-local-update-disabled-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/local-update-disabled/<nn>-<short-name>`.
- Expect one PR (P4). Split only if a reviewer would genuinely be helped, without churn: no code added in one PR and
  deleted in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits, `fix:`, with its changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- The last PR removes the TODO.md entry "No update for this machine".

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, a nextest selection of the `farhelm-ui` provisioning and hosts modules,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `dprint check` on changed Markdown, and the
website build if the docs page changes. Browser specs only if you identify a concrete browser risk the Rust render tests
do not cover; say which and why in the report either way.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to a fresh-context
Opus 5.5 agent at high effort, as the user demands (M5). No review swarm. The prompt carries the full charter, because
the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its findings file
(in the scratch directory), and the acceptance criteria: The goal, Decisions already made, and the outline above. For a
PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim. Address what the
reviewer finds before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or
in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a helm or protocol change, a new HTTP surface, a
change to the menu state model, a dismiss control for row errors, per-platform description text; these are examples, not
a blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current diff and
the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the inline-button reading of M1 (P1), the new `HostKind` predicate, the exact
description wording and the docs it was checked against, the SPEC.md and docs wording, and every reviewer finding you
declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The one agreed fallback is the installer-wording block under Decisions already made.
Anything else that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan,
step 10).

## Done criterion

The plan is complete when its draft PR (or linear stack of PRs) exists, satisfies the acceptance criteria, has passed
the review gate, and has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has
reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a
closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-10-04: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-10-04 (claim 98085b).
>
> ### What this is about
>
> In the hosts panel, each host row has a `⋯` menu with an **update** item that makes the helm install the newer Farhelm
> on that host over ssh. On the row for the helm's own machine ("this machine"), choosing **update** gets a refusal
> ("this is the helm's own machine; run farhelm helm setup here instead of provisioning from the panel"), and that error
> then stays under the row with no way to dismiss it. You asked for the item to be greyed out on that row instead, with
> its description line pointing at the installer as the way to update this machine, so the refusal is never reached.
>
> ### What I found
>
> The plan's one agreed fallback applies. It says: if the landed specifications still support a helm machine on which
> re-running the installer is not how it updates (for example a Linux helm machine while the installer refuses Linux),
> do not invent per-platform text; stop and ask. They do:
>
> - SPEC.md, "Installation and updates": the installer supports only a Mac installing the desktop app, and "Linux
>   remains supported for running a helm and session hosts".
> - SPEC.md, the upgrade section: "A Linux machine running a helm remains supported, but the standalone installer
>   temporarily does not cover that setup."
> - SPEC.md's topology section describes the local row's supervisor as either "the desktop app's managed local one, or
>   one running beside a Linux helm", and says `farhelm helm setup` sets up a Linux helm machine.
>
> So the same greyed-out item shows on the Mac app's own machine, where re-running the installer is right, and on a
> Linux helm machine, where the installer refuses to run. Nothing in the specs or docs says how a Linux helm machine is
> updated; setup there starts from a `farhelm` binary you put on the machine yourself and `farhelm helm setup`.
>
> Nothing has been built. The rest of the plan (greyed-out item, no request sent, SPEC.md sentence, docs line, changelog
> fragment, TODO removal) does not depend on the answer; only the description text and the matching SPEC.md and docs
> wording do.
>
> ### The question
>
> What should the greyed-out item's description say, given that this machine may be a Mac running the app or a Linux
> helm machine?
>
> 1. Wording that is true on both, naming the Mac installer as the common case, for example "Update this machine outside
>    the hosts panel; on a Mac, re-run the installer." One string, no platform logic; on a Linux helm it is vague but
>    not wrong.
> 2. Different text per platform: "Re-run the installer to update this machine" on a Mac helm, and on a Linux helm
>    something like "Install the newer farhelm binary here and re-run farhelm helm setup". The UI would need to learn
>    which platform the helm runs on, which nothing it receives from the helm says today, and the Linux instruction
>    would describe an update procedure the docs do not yet define.
> 3. Point at the installer only, as originally planned, and accept that the text is wrong on a Linux helm machine until
>    the installer supports Linux again.
> 4. Hide the item on a Linux helm's own row and grey it out only on the Mac app's machine. This also needs the
>    platform, and it partly undoes your decision that the item be "greyed out, not hidden".
>
> I recommend option 1. It keeps the change as small as planned and is never false. It still tells someone running the
> Mac app to re-run the installer. If you want different wording, give the exact text and I will use it as written.

The maintainer's answer:

We are only targeting Mac OS right now. It's fine.
