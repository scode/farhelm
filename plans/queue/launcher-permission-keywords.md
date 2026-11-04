# Launcher: yellow input rejections, and permission words before a harness

Written against main at 1d764d24 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change. A fresh-context scope review checked this plan
against the code before it was written; its corrections are folded in below.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Launcher keywords for guarded/YOLO modes are dead.** Typing "yolo" or "guarded" in the launcher used to select those
> permission modes; it no longer does. Restore that selection.

### What planning found

The keywords are not broken. What the maintainer hit, from their screenshot: the desktop app's New session launcher, no
harness chosen yet, `yolo` typed into the search box. Search answered "Choose a harness first" in the same plain text
color as the rows under it, and the maintainer read that as the feature not working. That is SPEC.md's current rule (the
launcher paragraph around line 726: "On the agent tab, an unmatched effort or permission word, or an `effort:` or
`perms:` query, asks for a harness first when none is chosen"). With a harness chosen, bare `yolo` and `guarded` do
select the mode (`crates/farhelm-ui/src/launch_composer.rs`, `search_results`, the Permissions block, around 1176-1218,
which needs `Some(harness)`; covered by `e2e/tests/composer-word-search.spec.ts`).

Meanwhile, with no harness chosen, the launcher's Guarded and YOLO buttons are already shown and clickable
(`crates/farhelm-ui/src/launch_controls.rs`, the permission controls around 456-515, `harness.is_none_or(...)`), and the
draft already holds a permission with no harness (`crates/farhelm-ui/src/list/create_form.rs`,
`structured_permissions`). So only search refuses what a click already allows.

The maintainer's decision, in their words (2026-11-04): "first we need to make any 'errors' that show up like that be
actual obvious errors like a red color. second, let's allow you to choose yolo before harness. if a later harness choice
invalidates it, we can highlight it just like we already do today (e.g. you pick codex -> guarded -> opencode and we
highlight that guarded became yolo)". Then: "or lets say yellow not red", "since red is used for 'danger'", "(and red is
fine for that, it's the 'danger' warning) (but the 'you have to pick harness first' should be amber/yellow whatever)",
and on the planning page: "also to be clear, the yellow/amber thing applies not just to 'have to choose a harness', also
to any other kinds of 'rejections' of input like this in the launcher".

### Acceptance criteria

- **Yellow for rejections.** Every refusal, error and "can't use that" message in the launcher (New session, and the
  other dialogs that use the same launcher, such as Restart with) is drawn in the app's existing warning color, `--warn`
  family in `crates/farhelm-ui/assets/app.css`, the yellow the host rail uses for an outdated or unreachable host. That
  includes the search messages listed under Decisions, the bordered Launch refusal alert (keeping its border and ⚠, only
  the color changes), plain error lines under fields and in the checkout preview, and red outlines that mark a refused
  field. Both themes use their own warning shades; nothing new is invented.
- **Red stays for danger only.** The "Permission changed: Guarded → YOLO, because OpenCode does not support Guarded."
  banner with Undo (`permission-escalation-banner`), its red outline pulse on the permission control, and the red YOLO
  marking (`launch-composer-danger`, the danger segment) stay red.
- **Plain stays plain.** Loading text and "type X here" hints keep the ordinary text color.
- **Permission words before a harness.** On the agent tab with no harness chosen, bare `yolo` and `guarded` and
  `perms:yolo` and `perms:guarded` offer their Permissions row (its tooltip and help text included, which today assume a
  harness) and accepting it selects the same mode the Guarded or YOLO button would. Other permission words (custom, own
  setting, approve, smart approve, chat) and every effort word still ask for a harness first, now in yellow.
- **The banner on the first harness choice.** When a permission was explicitly chosen before any harness (by button,
  bare word or `perms:`), and choosing the first harness turns it into a more dangerous one, the red banner with Undo
  appears exactly as it does for a harness-to-harness switch. Choosing the first harness with no explicit permission
  still never triggers it. A change to a safer mode keeps whatever the harness-to-harness path does today.
- **The Command tab is unchanged.** It offers no permission rows, with or without these words (SPEC_impl.md around 1159:
  "Command mode has no active structured harness, so it offers no permission actions").
- **Specs and docs match.** SPEC.md's banner paragraph (around 686-700, including "Choosing the first agent or choosing
  a permission directly never triggers it") and launcher search paragraph (around 712-731, including "Permission actions
  are offered only when the selected harness can represent them" and "asks for a harness first"); SPEC_impl.md around
  1155-1159 (permission search) and around 5015 (the escalation comparison, which counts passive memory seeds: before
  the first harness only an explicit choice counts); and the docs page
  `website/src/content/docs/docs/using/start-a-session.mdx` describe the new behavior.

## Decisions already made

Settled with the maintainer on 2026-11-04, in chat and on the planning pages:

- **Yellow, not red, for rejections; red is for danger.** Quoted above.
- **The reach of yellow: all non-danger refusals and errors in the launcher.** Chosen over "only rejections of what you
  entered, red for failures that are not about input", so nothing has to sort refusal reasons into two kinds. A host
  unreachable or a failed request in the launcher is yellow too.
- **Search messages, as listed.** Yellow: "Choose a harness first", every "No … matches" (host, harness, model, effort,
  permissions, folder, recent setup, template), "No model matches; enter a custom model ID in the model field", and "No
  repository matches; enter owner/repo". Plain: "Loading models…", "Loading templates…", "Type a session name after
  name:", "Type owner/repo to choose a repository", "Type a search or clear the search box to launch" (all from
  `launch_composer.rs`, `empty_search_message`). The same split applies to the repository field's message, which uses
  the same function.
- **Permission words before a harness:** allowed, for bare `yolo`/`guarded` and for `perms:yolo`/`perms:guarded`. The
  other permission modes exist only for some harnesses, so they keep asking for a harness first.
- **A later harness that cannot use the mode** replaces it and shows the existing red banner with Undo, as switching
  harnesses already does.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
  be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- SPEC.md, the launcher paragraphs around 686-731: the escalation banner (which today says "Choosing the first agent or
  choosing a permission directly never triggers it"; change it as the acceptance criteria say), the search rules, and
  "an incomplete attempt says why beside Launch".
- The 2026-10-22 decision recorded in `app.css` beside `.launch-composer-refusal-alert`: a refused launch is a bordered
  alert with an icon, not a bare line, because the maintainer missed the bare line. Keep the border and icon.
- `crates/farhelm-supervisor/src/agent_kind/mod.rs` module docs and root `AGENTS.md`, "Code rules": no per-harness
  branching in shared code. Which harness offers which permission stays answered by `offers_permission` and friends.
- Root `AGENTS.md`: "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md".
  `website/AGENTS.md` before touching the docs page (an incidental docs edit: name the changed page, start no preview).
  `.agents/test-authoring.md` and `docs/test-sleep-check.md` for test changes.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- **One warning style, applied by class.** Swap the color variables of the existing launcher rules to the existing
  warning tokens (`--warn`, `--warn-bright` for prose, `--warn-bg-dim`, `--warn-border`, each with light and dark
  values) rather than adding a component or token. The search message needs a way to mark which messages are warnings;
  the smallest is for `empty_search_message` to return a kind alongside the text and the markup to add a warning class;
  the repository field gets it for free, since it uses the same function.
- **Audit first, then change.** List every red (`--danger`) and error-like element the launcher can show
  (`create_form.rs`, `create_form/draft.rs`, `launch_controls.rs`, `list/save_template.rs`, and their CSS), mark each as
  danger (stays red), rejection (turns yellow), already yellow, or plain, and log the list as a DECISION. A launcher
  element that is neither a rejection nor danger is out of scope. From a read-only pass at planning time (verify it):
  - To turn yellow: `.launch-composer-search-message` (warning kind only); the refusal alert
    `.launch-composer-refusal-alert` (text, border and background, around `app.css` 7496); `.create-session-error` as
    the launcher uses it (the rule is shared with `hosts.rs` and `ssh_help.rs`, so scope the change, for example
    `.create-session-form .create-session-error`, and leave the shared rule alone); `.save-template-error` (around 7440,
    rendered by `list/save_template.rs`); `.launch-composer-folder-problem` (around 7519); the folder input's
    `aria-invalid` outline (around 7514).
  - Already yellow, record only: `.create-catalog-error` (around 6813) and `.launch-composer-choice-error` (around 6804,
    which also carries the "selected permission is unavailable ... replaced by YOLO" note).
  - Stays red (danger): `.permission-escalation-banner` and its outline pulse, the YOLO danger marking.
  - Out of scope: `.action-error` (not used in the launcher) and `.restart-error` (the session view's).
- **"Explicit" means the existing notion.** The draft (`crates/farhelm-ui/src/list/create_form/draft.rs`) already keeps
  `permissions_explicit`, set by `choose_permissions`, apart from a passive remembered mode. With no harness chosen and
  nothing chosen, the Guarded button shows as selected; that display is not an explicit choice, so a first pick of
  OpenCode from that state shows no banner. The smallest change: in `capture_permission_switch` (draft.rs), when there
  is no harness, record `draft.permissions_explicit().then(|| draft.permissions()).flatten()` as the permission before;
  `permission_danger(None)` already ranks highest, so a non-explicit state can never escalate. Then drop the `Some(_)`
  requirement on the harness before in `note_completed_permission_switch` (`create_form.rs`, around 502). Undo already
  restores the whole cloned draft.
- **Search changes only where it must.** Pass the existing `agent_without_harness` notion (computed in `create_form.rs`
  around 4372) into `search_results` and offer Yolo and Guarded with no harness only when it holds, keeping today's
  matching rules (bare word substring, `perms:` prefix). Leave `query_needs_harness` alone: it only matters when there
  are no rows, and these four queries always produce rows on the agent tab once search offers them; changing it would
  add a pointless "Set session name: yolo" fallback. The Guarded row's tooltip and help text (`search_result_tooltip`,
  `create_form.rs` around 2357, which today `expect`s a harness and would panic) reuse the Guarded button's no-harness
  text ("Guarded: choose an agent to see its automatic safety mode", `launch_controls.rs` around 481) through one shared
  helper.
- **Type and order.** PR 1 is `fix:` (a refusal shown as plain text read as the feature being broken), PR 2 is `feat:`,
  each with its own changelog fragment. PR 2's fragment covers both the words and the banner. The TODO entry is removed
  by PR 2, the last code PR.

## Outline

1. `plan/launcher-permission-keywords/01-yellow-rejections`: the audit, the CSS and markup changes, the search-message
   warning kind, SPEC.md/SPEC_impl.md wording if they name colors (SPEC.md's banner paragraph says "red"; keep it), and
   tests: a unit test on the message kinds (every listed message on its side of the split) and a browser test that a
   warning message (`yolo` with no harness on the agent tab) and the Launch refusal carry the warning style while a
   plain hint does not. Use a deterministic hint (`name:` or spaces), not a loading message, which shows only while a
   read is pending; the unit test covers the loading branches. Assert classes or computed colors against the theme
   variables, not hard-coded hex values.
2. `plan/launcher-permission-keywords/02-permission-words-before-harness`: search offers Yolo and Guarded on the agent
   tab with no harness (bare and `perms:`), as Planner proposals describe; accepting the row sets the same draft
   permission a button click sets, marking it explicit. The first-harness banner per the acceptance criteria, with Undo
   returning to no harness and the chosen mode. Unit tests: search with no harness on the agent tab (the four queries
   offer their row; `perms:custom` and effort words do not) and on the Command tab (no permission rows); the existing
   test asserting that "guarded" with no harness offers nothing (`launch_composer.rs` around 2160) changes to the
   agent/command split. Browser tests, in `e2e/tests/permission-switch.spec.ts`, which already resets the remembered
   permission around each test and covers "first agent, nothing chosen, no banner": no harness, type `yolo`, accept, the
   YOLO button is selected (not `guarded` for this check: Guarded already shows as selected with nothing chosen); no
   harness, type `guarded`, accept, pick OpenCode, the red banner appears and Undo restores no harness with Guarded
   chosen. Update SPEC.md, SPEC_impl.md and the docs page. Remove the TODO entry.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `dprint check` on changed files;
`cargo fmt --check` and clippy on `farhelm-ui`; the `farhelm-ui` unit tests for `launch_composer` and `create_form`
through the recorder; the launcher browser specs you touch or add (`e2e/tests/composer-word-search.spec.ts` and its
neighbours) through the recorder, both engines as `.agents/validation.md` describes;
`python3 releasing/check-changelog.py format`; `python -B scripts/check-test-sleeps.py` with the interpreter
`docs/test-sleep-check.md` prepares. Browser tests do not run in CI, so a local pass is the only browser evidence.

## Fallbacks

Examples of departures that need the scope reassessment:

- recoloring anything outside the launcher dialogs, or changing a shared CSS rule that views outside the launcher use;
- offering any permission other than YOLO and Guarded without a harness;
- a new banner, notice component or color token;
- touching the helm's refusal texts or the wire protocol.

Decisions to log: the audit list, how a search message is marked as a warning, how "explicit" is carried into the first
harness choice, every test added and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launcher-permission-keywords-log.md` in the parent directory of the checkout you run in, derived as that
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

- A linear stack of reviewable commits, each with a stable bookmark and a draft PR, using the `jjstack` skill. The base
  is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`, "Executing one
  plan" (step 6); never another plan's PRs. Bookmarks are as in the Outline; a different split is a logged DECISION and
  keeps one linear stack under `plan/launcher-permission-keywords/<nn>-<name>`.
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), each `fix`/`feat` with its changelog fragment in the same commit.
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
already made, planner proposals and outline). When a PR changes tests or their fixtures, include the full text of
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
Wording in this file that turns out inaccurate (a line number, a file name, a message text) is corrected to the truth.

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

The plan is complete when a linear stack of draft PRs exists that turns the launcher's non-danger refusals and errors
yellow and lets `yolo` and `guarded` be chosen before a harness with the red banner on a first harness choice that
escalates, as the acceptance criteria say; every PR passed its review gate; and the last code PR removes this plan's
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
