# Tensorlake recipe credentials: Farhelm or the image, never both

written against main at 1f19bb8c on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute two triage outcomes recorded on 2026-11-09, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes" (one PR per outcome; a `fix spec+code` outcome is never split). Their `TRIAGE_OUTCOMES.md` entries are
the binding statement of what to do; read each in full from `main@origin`, together with its feedback file under
`review_feedback_queue/`:

1. `codex-endpoint-change-retains-environment-key.md` (`fix spec+code`)
2. `claude-endpoint-change-retains-custom-headers.md` (`fix spec+code`)

### What the PRs do

A Tensorlake recipe chooses, per agent, **None | API endpoint | Subscription** (SPEC.md, the recipe editor paragraph
around line 469). With API endpoint or Subscription, Farhelm delivers credentials into the host's agent settings by
merging its keys into the existing file (`crates/farhelm-helm/src/provisioning/agent_endpoints.rs`, called from
`credential_delivery.rs`). The merge keeps everything else, so credentials a custom image or a hand-edited host already
carries can go to the newly selected server:

- Codex: `codex()` writes Farhelm's keys into `[model_providers.farhelm]` but keeps other keys in that table, so a
  leftover `env_key` (which Codex prefers over `experimental_bearer_token`), `http_headers` or `env_http_headers`
  survives.
- Claude: `claude()` and `claude_subscription()` touch only `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` and
  `CLAUDE_CODE_OAUTH_TOKEN`, so `ANTHROPIC_CUSTOM_HEADERS` (gateway secrets in extra headers) survives and is sent to
  the recipe's endpoint, or to Anthropic's own API in Subscription mode.

PR 1 (Codex) adds the shared SPEC.md rule and fixes Codex. SPEC.md states that for each agent either Farhelm provisions
its credentials (API endpoint or Subscription) or the base image and host do (None), and that credentials for that agent
already in the image or host configuration when Farhelm provisions it give undefined results. That rule qualifies two
existing SPEC.md sentences, which PR 1 edits accordingly: "Other settings remain intact" in the recipe agent-choice
paragraph (around line 473), and "A custom base's existing authentication is preserved and may be inherited by hosts
created from it; its author is responsible for that content" in "Topology" (around line 560), which now applies to the
agents whose recipe choice is None. SPEC_impl.md says Farhelm does better than that contract on a best-effort basis, and
what it does: on API endpoint, `codex()` empties the `farhelm` provider table before writing its own keys, so nothing
else survives there, while unrelated providers, root settings and comments outside that table are preserved. PR 1
rewrites the SPEC_impl.md "Adding Tensorlake hosts" sentence about "selecting and updating `model_providers.farhelm`"
(around line 6442) to match. Focused merge tests cover a preexisting `env_key` and header settings (including an
`http_headers` sub-table) in that table; the `custom` assertion in `merges_preserve_unrelated_settings` flips to absent,
and the `codex()` docstring ("Existing provider tables may contain unrelated keys") changes. `fix:` with a changelog
fragment.

PR 2 (Claude) extends the SPEC_impl.md best-effort statement: when Farhelm provisions Claude (API endpoint or
Subscription), it removes `ANTHROPIC_CUSTOM_HEADERS` from the settings environment along with the keys it already
manages, and a gateway that needs extra headers belongs under None, with the image providing everything. Other settings
stay intact. PR 2 rewrites the SPEC_impl.md sentence that lists the only Claude keys that change (around line 6438).
Focused merge tests cover a preexisting custom-headers value in both modes. `fix:` with a changelog fragment.

Neither PR may change the test process's environment. Credential delivery reruns only when a host's delivery stamp
changes (`credential_delivery.rs`, around line 434), so hosts that already received an endpoint keep their leftovers
until the next delivery (an endpoint, token or mode change). The changelog fragments say the cleanup applies from the
next delivery, not retroactively; no code is needed for that.

### Acceptance criteria

- Two PRs, Codex first, each meeting its ledger entry's completion criteria, recording its execution in the ledger,
  removing its feedback file and index line, and moving its ledger entry to `TRIAGE_OUTCOMES_ARCHIVE.md`.
- The existing None-mode cleanup (`clear_codex`, `clear_claude`) and its tests keep their behavior.

## Decisions already made

- **The outcomes:** the maintainer's answer on the 2026-11-09 triage page, in their words: "either farhelm is
  responsible for provisionoing credentials, or the image is. I forget if we support both today. But it's either or. If
  the user asks farhelm to provision credentials and they have credentials in the image, the result is undefined. fix
  SPEC on that point. That said, to the extent that we can address this so that the behavior is better, let's fix code
  with a hard complexity gate and SPEC_impl must state this is a best-effort behavior to do better than what SPEC (user
  visible contract) says." Both are supported today through the per-agent recipe choice, so no new setting is needed.
- **Hard complexity gate:** about 100 lines of diff per PR (code, tests and docs, not counting the ledger, the queue
  removal or a changelog fragment). Over it, or if either fix needs a new setting, block and bring it back rather than
  expanding. This gate is hard: do not answer it by trimming the agreed tests. Count insertions plus deletions from
  `jj diff --stat`, excluding the ledger, the queue removal and the fragment. Add new spec sentences at a paragraph's
  end or as a short paragraph of their own where that reads well, so dprint does not reflow unchanged text into the
  count.
- **Grouping:** one plan with two PRs, Codex first carrying the shared spec rule, approved on the 2026-11-09 planning
  page.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two `TRIAGE_OUTCOMES.md` entries named above, and the maintainer's words quoted under
Decisions.

**Binding repository constraints:** the rest of SPEC.md's recipe agent-choice paragraph and of SPEC_impl.md "Adding
Tensorlake hosts" (the sentences named under What the PRs do are the ones these PRs rewrite), including the None-mode
cleanup and the refusal of malformed files; the credential delivery description; root `AGENTS.md`, "Tests and
validation", "Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`;
`review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): in `codex()`, call
`provider.clear()` before the existing five-key insert loop. It empties the table, sub-tables included, and keeps the
table's place and form in the document and the existing refusal when `farhelm` is not a table, which inserting a fresh
table over it would silently drop. For Claude, add `ANTHROPIC_CUSTOM_HEADERS` to the keys `claude()` and
`claude_subscription()` remove. Whether to also remove other Claude credential sources (`ANTHROPIC_API_KEY`,
`apiKeyHelper`) is your call within the gate. A nonempty `ANTHROPIC_AUTH_TOKEN` outranks them in API endpoint mode, but
in Subscription mode, and for an endpoint saved without a token (which writes an empty `ANTHROPIC_AUTH_TOKEN`), they
would win, and in the tokenless case an image's `ANTHROPIC_API_KEY` would go to the recipe's endpoint. Both cases are
undefined under the new SPEC.md rule, so either choice is allowed; removing `ANTHROPIC_API_KEY` costs about one line per
function. Verify Anthropic's current precedence order before relying on it, and log the choice. Update the existing test
names and docstrings that promise "only the three managed keys" change.

## Outline

1. `plan/endpoint-credential-ownership/01-codex-provider-table`: the SPEC.md either/or rule and the two SPEC.md
   sentences it qualifies, the SPEC_impl.md best-effort statement and its Codex sentence, the Codex merge and its tests.
2. `plan/endpoint-credential-ownership/02-claude-custom-headers`: the Claude part of the best-effort statement and the
   SPEC_impl.md Claude-keys sentence, the Claude merges and their tests.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-helm`, the `provisioning::agent_endpoints` and credential delivery unit tests through
`python3 scripts/record-test-run.py`, `dprint check` on the spec files, and
`python3 releasing/check-changelog.py format`. No cloud calls.

## Fallbacks

The `podman-environments` plan was pending when this was written; it shares recipes with container hosts and may move
credential delivery. If it (or any other change) has landed and moved the code this file names, follow the current code
and correct this file's names and line numbers to the truth, logged as a DECISION. If it changed the behavior an outcome
is about, so that the ledger's completion criteria no longer make sense, that is a block.

Examples of departures that need the scope reassessment: a list of Claude settings keys classified as credentials, a new
recipe setting, or stripping credentials from tables or environment entries Farhelm does not own beyond the ones named
here.

Decisions to log: how `codex()` empties the provider table, whether Claude also drops `ANTHROPIC_API_KEY` or
`apiKeyHelper`, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-endpoint-credential-ownership-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/endpoint-credential-ownership/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL), removing its feedback file and `review_feedback_queue/INDEX.md` line, and moving its entry to
`TRIAGE_OUTCOMES_ARCHIVE.md`, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
