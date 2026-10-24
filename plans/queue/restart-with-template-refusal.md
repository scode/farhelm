# UI: Restart with and Replace with accept templates that repeat held values

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except for the SPEC.md sentence this plan rewrites at the maintainer's request (below).

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Restart with refuses templates that merely re-assert held fields.** Applying a template in Restart with rejects it
> whenever it sets a field the dialog keeps fixed — the host, the launch kind (agent vs command), and others — even when
> the template sets the same value the session already has, so a template that names the current host or the session's
> own kind is refused pointlessly. Only an actual change should be refused; a same-value assignment should apply as a
> no-op. First step: compare the template's host identity and launch kind against the held values in
> `restart_template_refusal` (crates/farhelm-ui/src/list/create_form/draft.rs) and the proto's `host_fixed` host check
> (crates/farhelm-proto/src/launcher.rs).

Today's refusal is written into SPEC.md on purpose (Lifecycle operations, Restart with, around 1231-1236): templates,
search results, recent setups and template shortcuts that set a held field "are refused without applying any edits, even
when they name its current value". It came from the `restart-with-as-launcher` plan, shipped in v0.31.0. The impact is
wider than the entry says: every template saved from a launcher (New, Clone, Restart with itself) or made in the
Templates editor stores the launch kind (SPEC.md around 729-738), so today no saved template can be applied in Restart
with at all.

Where it lives, from a read of main at aa245430 (line numbers approximate):

- `LaunchDraft::restart_template_refusal` (`crates/farhelm-ui/src/list/create_form/draft.rs` around 722-757) refuses on
  field presence: `kind`, `agent`, `host`, `destination`, `name`, `prompt`, and on the command tab
  `resume_command == Some(None)`.
- `validated_template_batch` (`crates/farhelm-ui/src/list/create_form.rs` around 587-653) builds a launcher snapshot,
  normalizes each template, runs `restart_template_refusal`, then `farhelm_proto::launcher::apply_template`. Callers:
  the single template or shortcut acceptance (`draft.rs` around 2266) and the shortcut batch pre-check (`create_form.rs`
  around 4472).
- The private `apply_template` replay (`draft.rs` around 2257-2419) replays the original `template.fields` as launcher
  actions after validation, and `apply_search_result`'s restart guard (`draft.rs` around 2446-2469) refuses held-field
  actions on presence.
- In the proto crate, `apply_template` with `TemplateContext::host_fixed` (`crates/farhelm-proto/src/launcher.rs` around
  547-559 and 843-858) refuses any template host. Replace with sets `host_fixed` (`LauncherMode::holds_host`,
  `create_form.rs` around 1632-1697 and 2820-2822), so Replace with refuses a template naming the session's own host
  too. `farhelm spawn` on the helm also uses `host_fixed`.
- The helm's restart request carries no templates, and the supervisor's `restart_with_refusal`
  (`crates/farhelm-supervisor/src/service/core.rs` around 3257-3285) still refuses a changed launch kind or agent type.
  Host, folder and name cannot be expressed in a restart request at all. So loosening the UI cannot let a real change
  through.

### Acceptance criteria

- In Restart with, a template or template shortcut (single, sequence, or ⚡) applies when every held field it sets
  equals the session's held value, and those fields apply as no-ops. A template that changes a held field is still
  refused atomically with today's wording, naming the field. Per field:
  - **Launch kind:** equal to the active tab.
  - **Agent type:** equal to the agent the active kind uses (the agent tab's harness, or the command tab's agent; on the
    command tab with no agent, any template agent is a change). An equal agent is dropped, never re-applied: re-applying
    runs the agent-switch reconciliation, which silently upgrades an older session's unset permission to guarded and can
    clear a model or effort.
  - **Host:** the template's install identity equals the held host's identity. A held host with no usable identity (none
    recorded, or an identity mismatch) refuses any template host.
  - **Folder:** a `Folder` destination equals the session's folder as the same text, ignoring a trailing slash. A `~/x`
    template does not match (the browser cannot expand `~` on a remote host) and symlinks are not resolved; those stay
    refused as changes. A GitHub destination is always a change.
  - **Session name:** the text equals the session's raw title, byte for byte, with no title normalization. `name` and
    `name_kind` are dropped together, since the proto refuses a name kind without a name.
  - **Initial prompt** and **clearing the resume command** have no same-value case and stay refused.
- A template whose only fields are equal held fields applies as nothing, with no message, as the entry's "no-op" says.
- In Replace with, a template naming the session's own host applies (the host is dropped); another host is still refused
  with today's wording.
- Search results and recent setups keep refusing on presence in Restart with, as today.
- `farhelm spawn` is unchanged: it still refuses any template host. The proto's `host_fixed` stays a plain flag.
- The replay applies the cleaned template, so a dropped field never reaches `apply_search_result`'s restart guard (which
  would show a stray error after a partial replay), nor Replace with's host action (which is not a pure no-op: it resets
  the destination memory and clone host state).
- SPEC.md's Restart with sentence (around 1231-1236) is rewritten: templates and template shortcuts are refused only
  when they change a held field, while search results and recent setups that set one are still refused even when they
  name its current value. SPEC.md's Launch templates sentence (around 714-720, "a template that sets a field the current
  launcher mode holds fixed") says "changes". SPEC_impl.md's template refusal and validate-then-replay text (around 5444
  and 5472-5477) and the docs page `website/src/content/docs/docs/using/launch-templates.mdx` (around 214-218) match.
  Before editing under `website/`, read `website/AGENTS.md` (root `AGENTS.md`, "Instructions by path"); this is an
  incidental docs edit, so name the changed page in the report rather than starting a preview.
- Docstrings that state the old rule are rewritten: `restart_template_refusal`, `validated_template_batch`, the test
  docstring near `draft.rs` 2998, and `TemplateContext::host_fixed`'s doc where it describes Replace with.
- Tests:
  - Rust unit tests for the field matrix through `validated_template_batch`: each held field equal applies, unequal is
    refused naming the field; name dropped with name kind; a same-agent template on a draft with an unset permission
    leaves the restart edit equal to its baseline; the command tab with no agent refuses any template agent; a GitHub
    destination is refused; a held host with no identity refuses a template host; Replace with's own host applies and
    another is refused.
  - `restart_templates_refuse_held_fields_in_batch_order` (`draft.rs` around 2998-3037) uses a real change and keeps its
    batch-order assertion.
  - The browser loop "restart with refuses a template's held ${field} atomically" (`e2e/tests/restart-with.spec.ts`
    around 1870-1896) uses the session's current values for every field except the prompt, so it pins today's behavior.
    Rewrite it: same value applies (the accompanying edit lands, Restart becomes enabled), a different value is refused
    atomically. One iteration applies a launcher-shaped template (`kind`, `agent` and a model or effort edit, the shape
    every saved template has) and asserts that the restart request carries the edit, so no separate saved-template test
    is needed.
  - "refuses held shortcut batches and bolt templates" (`restart-with.spec.ts` around 1955-1994) switches its held
    templates to real changes, keeping its batch-atomicity and ⚡ coverage, and a same-value ⚡ template applies without
    restarting (the launcher leaves Launch to the user, SPEC.md around 765-766).
  - The Replace with host tests (`e2e/tests/templates.spec.ts` around 1795,
    `e2e/tests/launcher-template-shortcuts.spec.ts` around 243-282) use a different identity for the refusal, and one
    case shows the own host applies.
  - All on both browser engines.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`): the
  refusing behavior shipped in v0.31.0, so this is a new fragment. It says that templates saved from New, Clone or
  Restart with now apply in Restart with when the fixed fields they set match the session, that Replace with accepts a
  template naming the session's own host, and that a template naming a different agent, host, folder or name is still
  refused.
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Replace with's host gets the same rule**: a template naming the session's current host applies. Chosen over fixing
  Restart with only. `farhelm spawn` stays as it is (planner proposal stated on the page, not objected to).
- **Folder equality: the same text, ignoring a trailing slash**, over exact bytes. `~` is not expanded and symlinks are
  not resolved.
- **Templates and template shortcuts (including ⚡) only**; search results and recent setups stay refused in Restart
  with. Chosen over relaxing those too.
- **Rewriting SPEC.md's "even when they name its current value" sentence** for templates is the maintainer's request:
  the TODO entry asks for exactly that behavior, and the planning page said the plan rewrites the sentence.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's Restart with, Replace with and Launch templates sections (apart from the
sentences this plan rewrites); SPEC_impl.md's template application paragraphs; `website/AGENTS.md` for the docs page;
root `AGENTS.md`, "Code rules" (no per-harness branching in shared code: the agent comparison is a plain equality, not a
harness-specific rule), "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One pure helper that, given the launcher mode's held values, drops every held field a template sets to its held value,
  applied inside `validated_template_batch` (beside its existing normalization) so that `restart_template_refusal` and
  the proto `host_fixed` check see only real changes and refuse unchanged as today. Have `validated_template_batch`
  return the cleaned templates (or call the helper once at the top of the private `apply_template`) so validation and
  replay use the same cleaned template. Held values never change within a batch, so cleaning every template against the
  opening values is exact.
- No proto change. Changing `apply_template` to take held values would need a same-harness special case in code the helm
  shares, and the UI replay would still need the same cleaning.
- The host comparison happens where hosts and the chosen host are known (`validated_template_batch`, through the
  launcher snapshot), not inside `restart_template_refusal`, which sees only the draft.

## Outline

One PR, `plan/restart-with-template-refusal/01-held-value-noop`: the helper, its wiring into validation and replay for
Restart with and Replace with, the Rust tests, the browser test rewrites, the docstrings, the SPEC.md, SPEC_impl.md and
docs page wording, the changelog fragment and the TODO.md removal. One PR, because the behavior contradicts a SPEC.md
sentence on main, so the two change together.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, the `farhelm-ui` unit tests for the draft and launcher modules, `python -B scripts/check-test-sleeps.py`
with the interpreter `docs/test-sleep-check.md` prepares, and through `python3 scripts/record-test-run.py`
`restart-with.spec.ts`, the Replace with template tests in `templates.spec.ts` and `launcher-template-shortcuts.spec.ts`
on both engines. Whatever check `website/AGENTS.md` names for a docs page edit.

## Fallbacks

Examples of departures that need the scope reassessment: a proto change, path normalization beyond a trailing slash (`~`
expansion, symlink resolution, a helm round trip), relaxing search results or recent setups, changing `farhelm spawn`,
or a message for a template that applied as nothing.

Decisions to log: the helper's shape and where it is applied, every test changed and why, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-restart-with-template-refusal-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/restart-with-template-refusal/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
