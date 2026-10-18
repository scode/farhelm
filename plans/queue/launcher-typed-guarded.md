# Launcher: select Guarded, the approval modes and default effort by typing

Written against main at d526038e on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

In the session launcher (New, Clone and Replace with), typing bare `yolo` in the search box offers and preselects the
YOLO permission choice, but Guarded is only reachable as `perms:guarded`. Make bare `guarded` work the way bare `yolo`
does. The planning session also audited every launcher choice for a typed route (in search or a text field); the
maintainer accepted the outcome below, which adds typed routes for Goose's and OMP's approval modes and for the effort
"default" choice, and documents the remaining exceptions.

TODO.md entry this plan covers (Near term): "**Select Guarded by typing in the launcher.** Make bare `guarded` offer and
preselect the Guarded permission choice, as bare `yolo` does today, while keeping `perms:guarded` available. During
planning, audit every choice in the New, Clone, and Replace with launcher dialog for whether it can be set through
typing alone (in search or a text field), and identify any other choices that need a typed route or an intentional
exception."

### Acceptance criteria

- With an agent chosen that offers Guarded, an unlabelled query that is any part of the word `guarded` (a substring:
  `ar` offers Guarded the way `ol` offers YOLO) shows the Guarded permission result in the combined search, and the
  exact query `guarded` preselects it, so Enter applies it. Accepting it does exactly what `perms:guarded` and the
  Guarded button do. An agent that does not offer Guarded (Goose, Pi, OpenCode, Cursor today) gets no such result; with
  no agent chosen there is none, as for `yolo` today.
- `perms:guarded`, `perms:yolo`, `perms:custom` and `perms:own` keep working exactly as now.
- `perms:` also offers the approval modes, each only when the chosen agent offers it: `approve`, `smart approve` and
  `chat` on Goose, `approve` on OMP. Matching is the existing `perms:` prefix rule against the word the button shows
  (`perms:smart` offers smart approve; `perms:smart approve` preselects it exactly). Which modes are offered comes from
  the per-harness permission table (`LaunchHarness::offers_permission`), never from a list of agent names in the
  launcher code. Accepting one selects that mode exactly as its button does.
- Bare `approve`, `smart approve`, `chat`, `own` and `custom` offer no permission result in unlabelled search (bare
  `own` and `custom` are too common as words; the approval modes get only `perms:` words).
- `effort:default` offers a result that selects the effort "default" choice (the agent's default reasoning effort, no
  explicit effort), exactly as the "default" button does, and preselects it on the exact word. It is offered in the
  `effort:` scope only, whenever an agent is chosen and that agent offers effort (`LaunchHarness::offers_effort`, the
  condition under which the "default" button appears, `launch_controls.rs` around line 372), whether or not the model
  catalog has loaded or the chosen model lists any effort. Bare `default` stays unoffered as a permission or an effort.
- Every new search result row has hover text. The approval-mode rows take theirs from the existing permission tooltip
  helper (`permission_tooltip`); the tooltip coverage sweep runs with Codex only, so it covers `effort:default` but not
  the Goose and OMP rows, and the Goose browser step below asserts that hover text instead.
- SPEC.md and SPEC_impl.md describe the typed routes and the documented exceptions (below); the docs page "Start a
  session" (`website/src/content/docs/docs/using/start-a-session.mdx`, "Search instead of clicking") no longer says
  `perms:` only picks guarded or the agent's own setting.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-18):**

- Bare `guarded` offers and preselects the Guarded permission choice "exactly as bare `yolo` does today": the same
  matching rule ("any part of the word, a substring match: `ar` offers Guarded the way `ol` offers yolo"), shown in the
  combined search, preselected on an exact match. `perms:guarded` stays.
- The typing audit outcome, accepted as follows:
  - Goose's approve, smart approve and chat, and OMP's approve: add as `perms:` words.
  - The effort "default" button: add `effort:default`.
  - Bare `own` and `custom`: no bare routes ("too common as words"); `perms:own` and `perms:custom` already exist.
  - The Agent / Command tab: a documented exception, as the specs have it today (SPEC_impl.md, the paragraph starting
    "Search is the composer's one initial and post-selection focus target": "There is no search result for the command
    tab itself").
  - The command tab's controls (the YOLO assertion radios, the declared agent type, the Resume checkbox): documented
    exceptions. SPEC.md already says the declared agent type "is chosen in the command tab's own field, not through
    search".
- Update SPEC.md and SPEC_impl.md where they list the typed routes.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- SPEC.md, "Creation", the launch-composer search sentence ("The launch-composer search matches harnesses, launch
  templates (`tl:name`), … `yolo` plus the `perms:yolo`, `perms:guarded`, `perms:custom` and `perms:own` permission
  actions …") and its rule "Permission actions are offered only when the selected harness can represent them; own
  setting is offered only on Cursor"; the command launch paragraph ending "The declared agent type is chosen in the
  command tab's own field, not through search."
- SPEC_impl.md's paragraph "The `perms:` scope offers guarded, YOLO and own-setting choices only where the active
  harness supports them. Bare `yolo` joins the ordinary combined search and gets exact-word preselection; guarded and
  own-setting use `perms:guarded` and `perms:own`. …", and the command-tab exception in the paragraph before it.
- Root `AGENTS.md`, "Code rules": per-harness behavior comes from the places the map in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` names (here `LaunchHarness::offers_permission` and `offers_effort`
  in `crates/farhelm-proto/src/launch.rs`); no `kind == X`, `matches!` over harnesses, or `_` arm over harnesses in
  shared code. Read that map before writing the permission words.
- Root `AGENTS.md`, "Changelog fragments", "Tests and validation"; `.agents/test-authoring.md` for test changes.
- `website/AGENTS.md` and `website/EDITORIAL_RULES.md` for the docs page edit (read both in full first). The edit is
  incidental to a code change, and the run is unattended: start no preview server, and name the changed page in the
  report.

**Planner proposals:**

- The search lives in `crates/farhelm-ui/src/launch_composer.rs`: `search_results` builds the rows (its permission block
  holds the word list and the bare-word rule `scope == SearchScope::All && word == "yolo" && word.contains(...)`), and
  `exact_word_group` decides exact preselection (it already matches `guarded`). Bare `guarded` is the bare-word rule
  widened to the two words `yolo` and `guarded`; nothing else in the ranking needs to change, since
  `default_search_index` already ranks the Permissions group in unlabelled search.
- The permission representation is the executor's choice; pick one and log the DECISION. Either shape is acceptable:
  adding `Approve`, `SmartApprove` and `Chat` variants to `ComposerPermission` (a few trivial arms per match site, no
  test churn), or `ComposerPermission::{OwnSetting, Explicit(LaunchPermission)}` (removes the per-mode arms, but
  rewrites every existing `ComposerPermission::{Yolo, Guarded, Custom}` reference, tests included). Whichever it picks,
  the places that handle a permission result (`apply_composer_search_result` and the hover text and row label in
  `crates/farhelm-ui/src/list/create_form.rs`) take the label from `permission_value`, the hover text from
  `permission_tooltip` (`permission_help` for Guarded, as now) and the applied value from the mode itself. Keep one
  shared search-word function (own setting is `own`, an explicit mode is its `permission_value`) used by both
  `search_results` for matching and `exact_word_group` for exact preselection, rather than two parallel string tables,
  so a row can always match exactly by the spelling it was offered under. Offer the words in today's search order (yolo,
  guarded, own, custom), then the new ones (approve, smart approve, chat), each filtered by `harness.offers_permission`
  (own setting by `omitted_permission().is_none()`, as now). Custom keeps its focus handoff to the flags editor. Leave
  the template path's branches alone: routing explicit template permissions through the search result would change how
  templates apply.
- Today the YOLO row's availability uses `normalized_permissions(harness, Some(Yolo)).is_some()` rather than
  `offers_permission(Yolo)`; every harness offers YOLO, so the two agree. Moving it to `offers_permission` is fine if
  the unit tests confirm nothing changes; log it.
- Approval words match the label the button shows (`smart approve`, with a space), not the wire spelling
  `smart_approve`: `exact_word_group`'s documented rule is that each kind compares by the spelling the search offered it
  under.
- `effort:default`: make the effort result carry an optional effort
  (`ComposerSearchResult::Effort(Option<LaunchEffort>)`, `None` meaning default), so accepting it is the same
  `structured_effort.set(...)` as the default button's `on_effort.call(None)`. Offer it only in the `effort:` scope
  (never in unlabelled search; the effort block today runs for both the unlabelled and the `effort:` scope, so this
  needs its own scope check), first in the list as the button is, matched by the same substring rule the other effort
  words use there, whenever an agent is chosen and `harness.offers_effort()` holds. That condition is independent of
  `compatible_efforts`, which is empty while the catalog has not loaded or when the chosen model lists no efforts; in
  exactly those cases "default" is the only effort button, so it must still be offered. Its row label is
  `Effort: default` and its hover text says it uses the agent's default reasoning effort, as the button's tooltip does.
  The template path that applies an effort through the search result keeps applying `Some(effort)`; leave its separate
  "set no effort" branch alone.
- The documented exceptions: SPEC_impl.md already documents the command tab. Extend SPEC.md's sentence about the
  declared agent type so it also covers the YOLO assertion and the Resume opt-in ("… are chosen in the command tab's own
  fields, not through search"). Rewrite SPEC_impl.md's `perms:` paragraph to say bare `yolo` and `guarded` join the
  combined search, and that own setting, custom and the approval modes have only `perms:` words, with the maintainer's
  reason for own and custom.
- A consequence to keep in view: like `yolo` today, short queries that are part of `guarded` (`a`, `d`, `e`, `r`, `u`,
  `ar`, …) now show the Guarded row when an agent is chosen, and when no harness, model or effort matched, that row is
  the first result Enter would take. The maintainer chose the same rule as `yolo`, so this is intended; check that no
  existing unit or browser test relied on a folder or recent setup being first for such a query, and fix the test's
  premise rather than the rule if one did.

## Outline

Two PRs, both `feat`.

1. `plan/launcher-typed-guarded/01-typed-permissions`: bare `guarded`, the approval-mode `perms:` words, the chosen
   permission representation, unit tests, browser test changes, the SPEC.md and SPEC_impl.md permission sentences, the
   SPEC.md command-tab controls exception, the docs page sentence, and a changelog fragment (`kind: added`) written for
   someone running Farhelm.
   - Unit tests in `launch_composer.rs`: extend `bare_yolo_offers_permissions_and_preselects_exactly` (or add a sibling)
     to cover bare `guarded` (offered, substring `ar` offers it, exact word preselects it, absent on an agent without
     Guarded and with no agent) and that bare `own`, `custom`, `approve`, `chat` offer no permission row; extend
     `permission_search_actions_follow_harness_capabilities` (fix its stale "two settled shorthand words" docstring) to
     cover Goose's and OMP's `perms:` lists and the prefix and exact behavior of `perms:smart approve`. Iterate
     `LaunchHarness::ALL` where a test claims "only where offered", so a new harness is covered.
   - Browser tests: in `e2e/tests/composer-word-search.spec.ts`, switch the structured launch's Guarded step from
     `perms:guarded` to bare `guarded` (its premise assertion that the option is the selected one stays);
     `perms:guarded` keeps its coverage in `guarded-permissions.spec.ts`. Add one step to
     `e2e/tests/goose-pi-composer.spec.ts` that types `perms:smart approve` with Goose chosen, asserts that option is
     selected and carries the smart approve hover text from `permission_tooltip`, presses Enter, and asserts the smart
     approve button is pressed. Do not widen the tooltip coverage sweep to more agents. Update test docstrings that
     describe the words.
2. `plan/launcher-typed-guarded/02-effort-default`: `effort:default`, its unit tests (offered first under `effort:`,
   absent from unlabelled search and without an effort-capable agent, still offered with an empty catalog, exact
   preselection), one browser step (set an effort, type `effort:default`, Enter, the "default" effort button is pressed;
   place it where it does not change what an existing test's launch asserts), `effort:default` in SPEC.md's search
   sentence, an edit to PR 1's changelog fragment rather than a second fragment, the docs page if
   `website/EDITORIAL_RULES.md` leaves room for it there (log the choice), and the removal of the TODO.md entry.

### Validation

Choose per `.agents/validation.md`; at least, for each PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit tests
for `launch_composer` and `list::create_form` through the recorder, `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`, `python3 releasing/check-changelog.py format`, and `dprint check`. When the docs page
changes, the website build (`cd website && bun install --frozen-lockfile && bun run build`). On Chromium and WebKit
through the recorder: `composer-word-search.spec.ts`, `guarded-permissions.spec.ts`, `custom-permissions.spec.ts`,
`goose-pi-composer.spec.ts`, `omp-composer.spec.ts`, and `tooltip-coverage.spec.ts` (it sweeps the `perms:` and
`effort:` result rows for hover text with Codex chosen, so it covers `effort:default` but not the approval-mode rows;
`custom-permissions.spec.ts` adds evidence only if the Custom handling was touched). Not the full suite. The change is
confined to the web UI's launcher; no helm, supervisor, database or protocol change is expected, so no runtime Rust
suites beyond the UI crate's unit tests.

## Fallbacks

- None agreed beyond the decisions. If a representation change spreads beyond the launcher search, its row rendering and
  its apply path, prefer adding enum variants over a wider refactor.

Examples of departures that need the scope reassessment: bare routes for any word other than `yolo` and `guarded`, a
search result for the launch kind or for command-tab controls, a new matching or ranking rule, changes to
`LaunchPermission`, `LaunchEffort` or anything the helm reads.

Decisions to log: the permission and effort result representation, the YOLO availability check, the smart approve
spelling, where the `effort:default` browser step lives, whether the docs page mentions `effort:default`.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launcher-typed-guarded-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/launcher-typed-guarded/<nn>-<short-name>` (for example `plan/launcher-typed-guarded/01-typed-permissions`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
