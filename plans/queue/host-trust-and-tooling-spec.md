# Execute twelve triage outcomes: spec clarifications for host trust, managed paths, setup recovery and tooling

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's whole purpose is to change them as the user decided.

This plan has no dependency on another plan.

## The goal

Carry out twelve triage outcomes recorded in root `TRIAGE_OUTCOMES.md` in one draft PR. Each one is settled by a
clarification to SPEC.md or SPEC_impl.md; no code changes. Their headings, which are also the feedback file names under
`review_feedback_queue/`, grouped by the principle that settles them:

- A malicious or broken host may deny service to the helm or the desktop app:
  - `repeated-buffer-copying-amplifies-tiny-frame-traffic.md` (`fix spec`, carries the change)
  - `detached-terminal-cleanup-tasks-wait-writer-capacity.md` (`other`, settled by that change)
  - `cancelled-attach-retain-abandoned-terminal-channel.md` (`other`, settled by that change)
- One checkout runs one tooling or harness command at a time:
  - `screenshot-runs-collide-through-shared-fleet-auth.md` (`fix spec`, carries the change)
  - `shared-authentication-files-causing-concurrent-run-interference.md` (`other`)
  - `concurrent-starts-bypass-sweep-serialization.md` (`other`)
  - `publisher-copy-race.md` (`other`)
- Farhelm has dedicated ownership of the paths it manages; out-of-band manipulation of them is unsupported:
  - `snapshot-root.md` (`fix spec`, carries the change)
  - `archive-rename-move-replacement-directory.md` (`other`, settled by that change)
- Setup and uninstall recovery limits:
  - `interrupted-paired-writes-still-split-service-state.md` (`fix spec`)
  - `provisioning-update-replaces-binary-before-setup-guard.md` (`fix spec`)
  - `uninstall-dryrun-locks.md` (`fix spec`)

What each means for someone running Farhelm:

- A host that is hostile or arbitrarily broken can already slow the helm down; the spec currently says Farhelm avoids
  that "where it reasonably can". The user decided that denial of service of the helm or the desktop app by such a host
  is simply acceptable. What stays unacceptable is a host corrupting the helm's own state or affecting other hosts'
  security or data. The three findings in this group are exactly such denial-of-service effects (CPU spent copying a
  flood of tiny frames, cleanup tasks piling up behind a host that stops reading, a routing entry kept for a host that
  stalls during a cancelled attach).
- Development tooling (the browser test stack, the screenshot capture and publisher, deflake) keeps per-checkout files.
  Two runs started in the same checkout at once can trample each other's sign-in files, run pointers, or published
  pixels. The user decided that is out of scope: keeping concurrent agents in separate checkouts is the maintainer's
  responsibility.
- When the last session in a managed checkout is deleted, the checkout folder moves to the trash, and a folder that has
  since been replaced by something else must stay untouched. Two findings show the guard can be fooled by restoring a
  filesystem snapshot over the folder or by a replacement inside the guard's own microsecond window. The user decided
  that Farhelm has dedicated ownership of the managed checkouts it creates and does not defend against out-of-band
  manipulation of them: snapshot restores, kernel-level overrides the user sets up, uncommon filesystems such as FAT,
  and the like.
- Three setup and uninstall edge cases are accepted rather than fixed: a run killed between two adjacent atomic writes
  may leave a state that rerunning the same command repairs; an operator who moves a remote host to `farhelm helm setup`
  while a panel Update is queued gets a visible failure, which is enough; and the uninstall preview (`--dry-run`) does
  not predict a refusal caused by a lock someone holds at that moment. It warns that an install or upgrade running now
  makes uninstall refuse, and the real run refuses clearly for any held lock.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. On main, each of the twelve entries' Execution field reads "planned in
`plans/queue/host-trust-and-tooling-spec.md`" (set by the planning PR); that is the expected starting state, not
`pending`. Read all twelve entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate. Each `fix spec` outcome's principle is
  stated in SPEC.md or SPEC_impl.md, as its ledger entry and the outline below describe, in the user's meaning.
- No code, test, or website change, unless the website check under Validation finds a page that restates a changed rule.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- `dprint check` passes on every changed file.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11),** recorded in root `TRIAGE_OUTCOMES.md` (landed in #1919), quoted here
because the spec text must carry their meaning:

- Denial of service: "SPEC should say that denial of service of helm/desktop in the face of a malicous or arbitrairly
  broken host is acceptable". Given while discussing findings with no reachable trigger, where the user also said
  "anything that means a security boundary is broken like helm being corrupted, or cross-host attackes, must not be
  silently discarded." So the new rule accepts denial of service outright, and keeps corrupting the helm and affecting
  other hosts' security or data unacceptable.
- One run per checkout: "two runs in the same checkout is out of scope. its maintainer responsibility not to have
  multiple agents in the same checkout. this should be SPEC_impl.md."
- Managed paths: "out of scope + spec, but broadern from just snapshots. we do not add complexity or attempt to protect
  against anything beccause \"farhelm has dedicated ownership of this path\". any random out of band stuff like fs
  snapshot restore, weird kernel level overrides instigated by user, random ass filesystems that aren't typically used
  (like FAT), etc we don't support".
- Setup races: the user chose "Accept, fix cheap ones", which for these three outcomes means a spec clarification and no
  code: accept a kill between two adjacent atomic writes because rerunning the same command repairs it, accept the
  remote-setup overlap because non-panel remote setup is best effort, and narrow what the uninstall preview's "blockers"
  means.

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries". Asked how to group the outcomes, the user chose "8 plans as proposed"; this plan is the spec-only group. Asked
about PR shape, the user chose "One PR per plan", so this plan's outcomes share one PR, overriding root `AGENTS.md`'s
one-PR-per-outcome rule for these outcomes only. Review gate: "gpt-6.1-sol high". No-workhorse mode, which
`plans/AGENTS.md` requires.

**Planner proposals** are the placement of each new paragraph and the edits to neighbouring text in the outline below,
checked against main. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest
follows the precedent of earlier grouped triage plans.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, in particular the
documentation-only rule; Execute triage outcomes, as overridden above; Releases and the changelog; Agent scratch space;
"Do not change behavior SPEC.md specifies without surfacing the conflict"), `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), and the `scode-voice` skill for all spec prose.

## Outline

Line numbers drift; find the text by its heading or opening words. Verified on main at d3b28820. All edits are prose in
SPEC.md and SPEC_impl.md, written with the `scode-voice` skill, in the style the surrounding decisions already use: a
paragraph opening "Confirmed 2026-10-11:" (SPEC.md) or "Decided 2026-10-11." (SPEC_impl.md's bold-lead paragraphs),
stating the rule, the reason, and what stays unacceptable. Keep each change as small as the principle allows; do not
restate rules that already exist elsewhere.

- Denial of service (`repeated-buffer-copying-amplifies-tiny-frame-traffic.md`): SPEC.md, "Local authority and trust
  between hosts" and the misbehaving-host paragraphs after it. The paragraph opening "Confirmed 2026-10-01: a
  misbehaving host degrading the helm's performance or availability, the way a denial-of-service attack would, is
  accepted when it cannot easily be avoided" changes so that degrading or denying the helm's or the desktop app's
  performance or availability is accepted outright, whether the host is malicious or arbitrarily broken, and Farhelm
  spends no complexity avoiding it. Keep the "sessions changed" hint spacing sentence as a description of an existing
  mitigation that stays (the user did not ask to remove anything), reworded so it no longer reads as a duty. Restate,
  beside the new rule, what is still not accepted: a host corrupting the helm's own state, or affecting other hosts'
  security or data. The paragraph before it already says "What must still be prevented is a host breaking the helm
  itself or affecting the security of other hosts"; make the two agree rather than contradict (a host that makes the
  helm slow or unavailable is accepted; one that corrupts its state is not). The "Confirmed 2026-10-10: beyond that
  acceptance, two effects ... are accepted even where Farhelm could avoid them" paragraph that follows becomes partly
  redundant; keep its terminal-library remedy and log-volume content, and reword only its opening so it reads coherently
  after the new rule. Also check SPEC_impl.md's "Supervisor metadata retention and nonresponse" paragraph (grep for it):
  if it states a narrower acceptance that the new rule now supersedes, add a cross-reference rather than rewriting it.
  The two `other` outcomes need no further text; their ledger entries say the change settles them.
- One run per checkout (`screenshot-runs-collide-through-shared-fleet-auth.md`): SPEC_impl.md, the bold-lead paragraph
  "Development tooling assumes a single-user machine." (under Testing). Add one or two sentences: a checkout runs one
  harness or tooling command at a time (the browser test stack, the screenshot and video capture and publishing scripts,
  deflake, the docs preview, and the like); keeping concurrent agents in separate checkouts is the maintainer's
  responsibility, as root `AGENTS.md` "Sharing the machine with other agents" already asks; and tooling need not
  tolerate two of its own runs overlapping in one checkout. Note in the PR's ledger updates that root `AGENTS.md`
  already states "One agent per checkout or workspace"; do not edit root `AGENTS.md` (it is agent instructions, not
  spec, and the user asked for SPEC_impl.md).
- Managed paths (`snapshot-root.md`): SPEC.md, directly beside the "Filesystem aliasing beyond symlinks is unsupported"
  paragraph (in the section that ends just before "Supported host setup"), as the ledger entry says. State the user's
  principle in general terms, not as a snapshot carve-out, and scoped to what the user was talking about: Farhelm has
  dedicated ownership of the managed checkouts it creates, their checkout roots, and the trash
  (`farhelm-archived-working-copies`), and out-of-band manipulation of them is unsupported. Name the user's examples:
  restoring a filesystem snapshot over them, kernel-level overrides the user sets up, and uncommon filesystems such as
  FAT. Farhelm adds no complexity to detect or defend against such changes; the foreign-replacement guard exists for
  ordinary replacement (a folder removed and recreated, a re-clone), which the inode plus creation time check already
  catches, and a replacement inside the guard's own check-then-move window counts as out-of-band too. Do not widen the
  rule to the state directory or the installation, and say that the ownership checks other sections require still apply
  unchanged (the installer's checksum matching, uninstall refusing unknown entries in the app bundle, the symlink and
  canonical-path rules). The same section's "Confirmed 2026-10-10" paragraph says an unsupported configuration is
  refused with a clear message "except where this section says otherwise (other login shells, unsupported tmux programs,
  filesystem aliasing, and the proxy case below)"; extend that exception list to cover out-of-band manipulation of
  managed paths and uncommon filesystems. That is a wording fix that comes with the new rule, not a gate trip. Make sure
  the wording does not weaken the existing "Local authority and trust between hosts" rules or the device-number
  paragraph in the managed-checkout section (the btrfs, NFS and overlayfs setups named there stay supported; FAT is the
  example of an uncommon one).
- Interrupted paired writes (`interrupted-paired-writes-still-split-service-state.md`): SPEC.md, "Concurrent and
  interrupted runs". Follow the ledger entry's general rule: a run killed between two adjacent atomic writes may leave a
  state that rerunning the same command repairs, and that counts as the correct "stopped partway" state the section
  already allows, even where the in-between state is one the section's list of wrong outcomes names (such as the helm
  and supervisor services configured against different state directories). State it generally, not as a carve-out for
  setup's unit files only, and keep the rest of the section's list of wrong outcomes as it is.
- Remote host moved to non-panel setup mid-run (`provisioning-update-replaces-binary-before-setup-guard.md`): SPEC.md,
  "Supported host setup". Add a sentence that when a remote host is switched to `farhelm helm setup` (or another
  best-effort route) while a panel setup or Update for it is queued or running, the panel run may act on the binary the
  other route now uses, and a visible failure is enough; Farhelm need not re-check for such a switch mid-run.
- Uninstall preview (`uninstall-dryrun-locks.md`): SPEC.md, the `farhelm uninstall` paragraph ending "`--dry-run`
  reports the proposed actions and any blockers without changing files, stopping processes, or disabling services."
  Clarify that "blockers" means the ownership, recognition and ambiguity refusals the real run would give, and that the
  preview does not predict refusals caused by locks someone holds at that moment (the desktop app open, an installer
  running). The preview's existing warning, verified on main, is "An install or upgrade running now makes uninstall
  refuse, and one started during it refuses instead." It does not mention an open desktop app, so the spec must not
  claim it does: say the preview warns about a running install or upgrade, and that the real run refuses with a clear
  message for any held lock. Changing the warning's text is outside this plan.
- Changelog: spec text that records an existing behavior as accepted changes nothing for someone running Farhelm, so the
  PR's type is `docs:` and it adds no fragment, per root `AGENTS.md` (Releases and the changelog). If you find a change
  that does alter user-facing behavior, that is outside this plan; treat it as a gate trip.

The complexity gate for all twelve: prose changes to SPEC.md and SPEC_impl.md that state the user's decision. A gate
trip is any of: the new text would contradict another SPEC.md or SPEC_impl.md rule in a way that needs a design call
(for example, if accepting denial of service outright conflicts with a promise elsewhere that one host cannot disrupt
others' sessions, and the two cannot be reconciled by wording); the decision rests on a fact that turns out false; or
carrying it out needs a code change. Adjusting neighbouring wording so the new text fits (such as the exception list in
the "Confirmed 2026-10-10" unsupported-configuration paragraph) is part of the change, not a gate trip.

### Validation

Root `AGENTS.md` "Finishing work": this is a documentation-only change that affects no generated artifact or executable
example, so run no Rust, JavaScript, desktop, installer, provisioning or browser tests.

- `dprint check` on every changed file (`SPEC.md`, `SPEC_impl.md`, `TRIAGE_OUTCOMES.md`,
  `review_feedback_queue/INDEX.md`).
- Check that the website does not restate a rule this PR changes: grep `website/src/content` for the denial-of-service,
  misbehaving-host, trash/foreign-replacement, uninstall `--dry-run` and setup-interruption wording. If a page restates
  a changed rule so that it is now wrong, correct that page in the same PR following `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md` (read both first), except the docs Overview page, which root `AGENTS.md` says is edited
  only when the maintainer asks: report it instead. If a website file changes, also run
  `cd website && bun install --frozen-lockfile && bun run build`.
- Reread every changed spec section whole after editing, to confirm it reads coherently with its neighbours.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-trust-and-tooling-spec-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

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

On every path, check heartbeat freshness before each new build, check or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/host-trust-and-tooling-spec/01-spec-clarifications`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits. This PR is `docs:` with no changelog fragment, per the
  outline.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything; here a short body naming the four principles and that
  they record the maintainer's 2026-10-11 decisions is likely worth keeping.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries a gate: the change stays a prose clarification that states the user's decision, per
the outline. If one turns out to need more (a design call between conflicting rules, a decision resting on a false fact,
or a code change), do not grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its
feedback file and index line, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on
the user, followed by what you found and the realistic options with a recommendation (see `installer-startup-prune.md`
in the ledger for the shape). An `other` outcome whose `fix spec` sibling is dropped is dropped with it. Name dropped
outcomes among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only
when every outcome in this plan was dropped, since then there is no PR to deliver. A finding that turns out to be
already fixed or already covered on main is not a gate trip: record the outcome as `discard` with "already fixed" (or
the covering text) and the commit, per root `AGENTS.md`, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions quoted under Requirement sources. Ask the reviewer to check in particular that each new paragraph
carries the user's meaning without widening or narrowing it (denial of service accepted outright, while corrupting the
helm or affecting other hosts' security or data is not; one run per checkout; dedicated ownership of managed paths, in
general terms but limited to managed checkouts, their roots and the trash, with other sections' ownership checks
untouched; the interrupted-writes rule stated generally, as the ledger entry has it), that no new text contradicts
another SPEC.md or SPEC_impl.md rule, and that the prose reads plainly. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: editing a spec section
the outline does not name beyond a cross-reference, rewriting an existing decision paragraph rather than adding to it,
or changing anything outside SPEC.md, SPEC_impl.md and the bookkeeping files), and whenever the same section has needed
repeated corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the
ledger entries, the user's decisions above, this outline, the current diff and the proposed departure (what changed, why
it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where each new paragraph went and why, how the denial-of-service rule was
reconciled with the neighbouring "What must still be prevented" sentence and the 2026-10-10 paragraph, what the
uninstall preview's existing warning says, whether any website page restated a changed rule, the Conventional Commit
type, every outcome dropped by its complexity gate and why, and every review finding you decided not to follow. The user
will ask for these later.

### Unattended fallback

Resolve routine wording and placement forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee beyond what the user decided, or an omitted required behavior needs an agreed fallback or the user's decision;
a review finding or a log entry is not authorization. For a single outcome, the agreed fallback is the complexity gate
above. If current code or specs have moved so that a recorded decision no longer applies, treat it like a gate trip for
that outcome, per root `AGENTS.md` (Execute triage outcomes); never re-triage an item yourself. Block per
`plans/AGENTS.md` (Executing one plan, step 10) only when no outcome is left to ship or the shared PR itself cannot
proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
