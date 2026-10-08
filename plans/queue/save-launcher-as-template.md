# Save the launcher's setup as a template

Written against main at a38ea524 on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. It builds on the Templates dialog overhaul, which is already on main.

## The goal

Add a "save as template" action to the session launcher (the New session dialog), so a setup the user just made can be
kept as a launch template without retyping every choice in the Templates panel.

Acceptance criteria:

- In New and Clone (not Replace with), the launcher offers "save as template". It opens an inline panel inside the
  launcher with a name field and a checklist of the launcher's current choices, the explicitly chosen ones pre-checked
  per D1.
- Saving creates a template holding exactly the checked fields plus the active tab's launch kind, then closes the
  launcher and opens the Templates panel with the new template open in its editor.
- A taken name is refused with the Templates panel's existing message; nothing is overwritten.
- Enter in the panel's name field does not launch a session, and Escape in the panel does not close the launcher.
- Every new control has a hover text, as the tooltip coverage test requires.
- SPEC.md, SPEC_impl.md and the docs website's launch templates page describe the action.
- The last code PR removes the TODO.md entry "Save the launcher's setup as a template."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Save the launcher's setup as a template. Add a 'save as
template' action to the New session dialog: it asks for a name and shows the launcher's current choices as a checklist,
with the ones the user chose explicitly already checked, and saving creates the template and opens it in the Templates
panel. Most templates start as 'I just set this up, keep it', and today that means retyping every choice in the
Templates panel. Deliberately left out of the Templates dialog overhaul (`plans/queue/templates-dialog-overhaul.md`),
which this builds on."

**The user's decisions (2026-10-08).** The planner proposed each of D1 to D5; the maintainer answered "agree with your
recommendations":

- D1. The checklist lists every field that has a value on the active tab, plus host, destination and session name.
  Pre-checked:
  - agent type, model and effort whenever they are set (New never pre-fills them; on Clone the values copied from the
    source session count as explicit too);
  - permissions and workspace trust only when the user actually chose them, not when they show the remembered default;
  - host only when picked by hand, not the default host and not the host a Clone inherited;
  - folder only when typed or picked, not the `~` default or the open session's folder;
  - a `gh:` repository whenever it is set;
  - session name only when typed;
  - on the command tab, command, YOLO and resume whenever they are set.
- D2. The template always records the active tab as its launch kind; that is not a checklist item. (A "don't switch"
  template carries only host, destination and name, which the checklist already offers.)
- D3. The action appears in New and Clone, not in Replace with, where the host is held fixed and a saved host would be
  refused later.
- D4. After saving, the launcher closes (its draft is gone) and the Templates panel opens on the new template.
- D5. The name field and checklist are an inline panel inside the launcher, like the YOLO confirmation, not a dialog
  over the dialog. A taken name is refused with the Templates panel's existing message, with no offer to overwrite.
- D6. Review gate: "gpt-6.1-sol high effort, no swarm for reviewers."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:**

- SPEC.md "Concepts" (Launch template) and the Launch templates bullet under "Sessions › Creation": applying a template
  is exactly making its edits by hand; choices that come from applying templates count as the user's explicit selection;
  template names are unique; templates belong to the helm, with a host named by its install identity.
- SPEC.md: Launch and Cancel are the first controls in the launcher. The launcher is a `<form>` whose Enter submits, so
  every new button is `type="button"`.
- `check_template_shape` (`crates/farhelm-proto/src/launcher.rs`): name rules and the 64 KiB fields cap.
- `e2e/tests/tooltip-coverage.spec.ts` requires a `data-tooltip` on every control on the screens it visits.
- Root `AGENTS.md` (Conventional Commits; Finishing work, including browser validation; Releases and the changelog;
  Harness-specific code; Docs website; Sharing the machine with other agents; Agent scratch space; The live install is
  off-limits), `website/AGENTS.md` and `website/EDITORIAL_RULES.md`, `plans/AGENTS.md` (Executing), and
  `.agents/test-authoring.md` for test changes.

**What the planner verified (at a38ea524; find code by name):**

- The launcher is `CreateSessionForm` in `crates/farhelm-ui/src/list/create_form.rs`, shared by New, Clone and Replace
  with (Clone and Replace with arrive through its prefill). It holds one signal per field. `template_edits` already
  builds a `farhelm_proto::launcher::LauncherState` snapshot from those signals (with destination and name left out)
  before replaying a template. Nothing maps launcher state back to `TemplateFields`.
- Explicitness signals: permissions and trust have real flags (`structured_permissions_is_explicit`,
  `structured_workspace_trust_is_explicit`), set by clicks, search actions, recent setups, prefill and templates and
  cleared by "reset choices". Host: `chosen_host` is `None` for the default, and `CloneHostState` tells a Clone-bound
  host (`Bound`) from a hand pick (`UserTookOver`). Name: `title_edited`. **Folder: there is no signal for "typed or
  picked".** Typing sets `cwd_edited`, but every pick (search, browse, recents, home) goes through
  `reseed_cloned_field`, which clears the same flag that the `~` default and a Clone's copied folder leave clear. The
  plan needs a new folder-explicit signal, set where the user picks or types a folder and clear at mount and on Clone.
  The `*_raw_seed` values are what a submit sends for text relayed from another host (`submitted_field`); save those the
  same way.
- A host with no recorded install identity, or one in the identity-mismatch phase, cannot be named in a template (the
  Templates editor's host picker lists only hosts with an identity).
- The Templates panel is `TemplatesDialog(hosts, on_close)` in `crates/farhelm-ui/src/list/templates.rs`, mounted by
  `ListView` in `crates/farhelm-ui/src/list/view.rs`. It has no way to open a given template on mount; rows open through
  `depart(Departure::Open(template))`. Its `Field` enum carries per-field labels, presence and removal; its
  `Draft::refusal` checks per-field compatibility; `save_name_refusal` refuses a taken name with "a template named …
  already exists; edit that one, or choose another name". Saving is `api::put_template` (create-or-replace), so the name
  check is the only protection against overwriting.
- The quick switcher already hands a pick from one dialog to the launcher after the first closes (`ListView`'s
  `initial_create_action` / `open_new`); that is the pattern for "launcher closes, Templates opens on X".
- The YOLO confirmation (`yolo_confirm.rs`, mounted inline under Launch) and the Templates editor's inline departure
  prompt are the precedents for inline panels.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

Line numbers drift; find the code by name.

### PR 1: save as template from the launcher

`feat:` with a changelog fragment (kind `added`: the New session dialog can save its current setup as a template). Split
into two PRs only if each builds and is coherent on its own (for example the pure mapping and its tests first); log it.

- **Reading the launcher.** Extend the existing snapshot code in `template_edits` (extract it into a helper both paths
  use) to include folder or `gh:` destination and session name, rather than writing a second reader of the launcher's
  signals. Add the folder-explicit signal described above.
- **One candidate plus a pre-checked set.** A pure, unit-tested function turns the snapshot plus the explicitness
  signals into a candidate `TemplateFields` (the active tab's fields that have a value, plus host, destination, name,
  and `kind` from the active tab) and the set of pre-checked fields per D1. Saving is the candidate with unchecked
  fields removed, using the Templates panel's existing `Field` presence and removal, and the checklist uses the panel's
  field labels. Specifics:
  - command-tab resume appears only when it is on; "resume off" is the default, not a choice, so it is never listed
    (otherwise every command template would carry a resume reset);
  - a host without a usable install identity is left out of the checklist, with a short note saying why it cannot be
    saved;
  - values save as the launcher holds them; raw relayed text saves as the raw seed.
- **Round trip.** Unit tests that applying a saved template with `apply_template` to a fresh launcher state reproduces
  the checked choices, for an agent-tab setup and a command-tab setup.
- **The panel.** A "save as template" button (`type="button"`, hover text) in the launcher's action row after Launch,
  Cancel and "reset choices", shown in New and Clone only. It opens an inline panel with the name field, the checklist,
  Save and Cancel. The checklist is a snapshot taken when the panel opens. The panel owns its keys: Enter in the name
  field saves (or does nothing) and never submits the launcher; Escape closes the panel, not the launcher.
- **Saving.** On Save: run `check_template_shape` and the panel's `Draft::refusal` on the result (so, for example,
  unchecking the agent type while a non-default approvals choice is checked is refused rather than saved broken), fetch
  the template list once and pass it to `save_name_refusal` (refusing with its message if the name is taken or the list
  cannot be read), then `api::put_template`. Errors show inline in the panel; the launcher keeps its draft.
- **Opening the Templates panel.** On success, a `ListView` callback closes the launcher and opens `TemplatesDialog`
  with a new optional prop holding the template just saved, which the dialog opens in its editor on mount (it already
  has the fields; no lookup by name after the list loads).
- SPEC.md: the launcher section and the Launch templates bullet describe the action per D1 to D5. SPEC_impl.md "Launch
  templates": the snapshot reuse, the folder-explicit signal, and the name check. Keep both short.
- Browser tests in `e2e/tests/templates.spec.ts` (reuse its helpers, timestamped names and cleanup): save from New on
  the agent tab and on the command tab, unchecking one pre-checked field, and verify the stored template through the API
  and that the Templates panel opens on it; a taken name is refused and nothing is overwritten; Enter in the name field
  creates no session; the action is not offered in Replace with. Extend `e2e/tests/tooltip-coverage.spec.ts` to open the
  inline panel so its controls are covered. Pre-check rules beyond these belong in unit tests.
- Remove the TODO.md entry.

### PR 2: docs

`docs:`. Update `website/src/content/docs/docs/using/launch-templates.mdx` (following `website/AGENTS.md` and
`website/EDITORIAL_RULES.md`): saving the launcher's setup as a template, what is pre-checked, and that the launcher
closes and the new template opens in the Templates panel. No screenshot changes are required; if a docs screenshot spec
under `e2e/docs-shots/` would show this naturally, leave it for a later screenshot refresh and say so in the report. May
fold into PR 1; log it.

### Validation

Follow root `AGENTS.md` "Finishing work":

- PR 1: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `python -B scripts/check-test-sleeps.py` (per
  `docs/test-sleep-check.md`), the UI and proto crates' unit tests through `scripts/record-test-run.py` (with the pinned
  nextest and tmux setup from `docs/test-run-evidence.md`), and the browser specs `e2e/tests/templates.spec.ts`,
  `e2e/tests/tooltip-coverage.spec.ts` and `e2e/tests/launcher-tabs.spec.ts` on Chromium and WebKit through the recorder
  (builds per root `AGENTS.md`). Also look at the inline panel in a captured screenshot in both engines at a desktop
  width and at phone width, written to the scratch directory, and fix overflow or clipped text.
- PR 2: `dprint check` on changed files and the website build
  (`cd website && bun install --frozen-lockfile && bun run build`).
- `python3 releasing/check-changelog.py format` for the fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-save-launcher-as-template-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/save-launcher-as-template/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a provenance-tracking system for every launcher
field, a dialog stacked over the launcher, keeping the launcher open under the Templates panel, a new helm template API
or a create-only endpoint, an overwrite flow, a change to the template wire format; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the PR split, the folder-explicit signal's shape, which fields the checklist
offers in edge cases (no identity host, Clone-bound values), the panel's Enter behavior, and whether PR 2 folded into PR
1, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
