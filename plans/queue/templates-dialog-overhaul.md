# Overhaul the Templates dialog

Written against main at 931da7f1 on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

Replace the Templates panel (the dialog opened by the templates button beside New, where launch templates are created,
edited and deleted) with the design the maintainer approved, and make templates written from now on carry their launch
kind whenever they set agent-launch or command-launch choices. This covers three near-term TODO.md entries, which the
dialog PR removes (root `AGENTS.md`, TODO.md: an entry goes in the PR that addresses it):

- "Make sense of the permission choices."
- "Label the repository field in the template editor."
- "Templates seem impossible to edit once added."

What is wrong today, in product terms (verified in `crates/farhelm-ui/src/list/templates.rs` and `app.css` at the commit
above):

- Editing is unreachable. Each template's row puts its name, a one-line summary and its edit and delete buttons on one
  line. The list is a CSS grid with an implicit `auto` column, which grows to fit the widest row instead of the dialog,
  so a long summary pushes the buttons past the dialog's right edge. The maintainer's screenshot showed exactly that:
  the summary cut off by the dialog edge and no buttons at all. Even when the buttons are visible, edit loads the
  template into a form below the list with no highlight and no scroll.
- Every one of the fourteen fields is always shown as a select with "leave as is", agent-launch and command-launch
  fields side by side, though a template mixing the two can never apply.
- Choosing "set to" for the model or resume command, or any destination, adds a bare text input with no label as its own
  grid cell, stretched to the row's height: the "large blank unlabeled box".
- Permissions always offers yolo, approve, smart approve and chat, whatever the agent type, though most agent types take
  only their default or YOLO. "Runs without approval prompts", the command launch's YOLO statement, sits beside it on
  every template.
- Delete is immediate with no way back.

Acceptance criteria:

- A linear stack of draft PRs exists, shaped as in Outline (the dialog PR may be split, see PR discipline).
- The new dialog matches the approved design (Decisions and Outline below) at desktop and phone widths, in Chromium and
  WebKit, checked by looking at screenshots, not only by tests.
- SPEC.md and SPEC_impl.md describe the new dialog and the launch-kind rule; nothing in them still describes the old
  form's "leave as is" selects.
- `farhelm agent template create` writes the agent launch kind for `--agent` given alone, as it already does for the
  other agent-launch flags.
- The Launch templates docs page and its screenshot spec describe and capture the new dialog; the screenshots are
  captured locally and inspected, never published.
- The three TODO entries are removed by the dialog PR. (The near-term entry for "save as template" was added by the
  planning PR that queued this plan.)
- Every PR that changes code or tests passed the review gate. No PR is marked ready; nothing is merged.

## Requirement sources

**The user's request (2026-10-05):** "let's plan all of the template related todos in one go. i think we need to just
generally overhaul that whole dialog to be generally sensible. Look nice, support editing, adding, removing." The
planner drew mockups of options; the maintainer chose one and answered the open questions in conversation. The mockups
were a private draft and are not in the repository; the decisions below carry everything in them that matters.

**The user's decisions (2026-10-05):**

- D1. Layout: "i like your recommended layout/UI - lets go for that." That layout is a two-pane dialog: the template
  list on the left, the selected template's editor on the right. The editor shows only the fields the template sets, in
  launcher-style controls; fields are added with "+ add field" and removed with a ✕ per field.
- D2. A template that sets agent-launch fields carries the agent launch kind, and one that sets command-launch fields
  carries the command kind, so applying it switches the launcher to that tab instead of being refused ("yes it should
  follow"). This changes SPEC.md.
- D3. Models: "lets allow people to type in any arbitrary model but offer pre-selected options that are informed by
  agent choice _if known_, but don't reject anything if the user enters something." No field requires an agent type.
- D4. Delete takes effect at once and offers undo: a notice for about 10 seconds whose undo saves the template again
  under its old name, refused with a message if that name has been taken in the meantime.
- D5. The top row of the editor is "switches launcher to: agent · command · don't switch" (the maintainer: "perfect i
  love" it). A don't-switch template sets no launch kind and offers only host, destination and session name.
- D6. The dialog's help text says templates stack, with an example applying two in a row, along the lines of: "A
  template is a named set of launcher edits. In New session, type tl:name to apply one; it changes only the fields it
  sets. Templates stack, the later one winning: tl:my-claude then tl:webbuilder starts Claude in a fresh acme/web
  checkout on build-box." Keep the content; wording may be tuned to the final UI.
- D7. "Save as template" from the New session dialog is not part of this plan; it becomes a near-term TODO entry (added
  by the planning PR that added this plan, so nothing to do here).
- D8. Docs screenshots: update the shot spec and capture locally, "but definitley do multi modal inspection" of the
  captured images; do not publish (the docs are updated separately).
- D9. Review gate: "opus 5.5 + astra high, no swarm".
- D10. Interpretations the maintainer saw and did not object to: a stored template with agent or command fields but no
  kind opens in the editor with the kind inferred from its fields and gains the kind when saved, with no migration of
  stored data; "don't reject" is about the editor, and applying a template in the launcher keeps today's rules; when the
  agent type changes so that another choice no longer fits (approve after switching Goose to Claude), the editor flags
  that field inline and refuses to save until it is changed or removed, never rewriting it silently, with the model
  exempt.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, including browser
validation; Releases and the changelog; Harness-specific code, which forbids comparing harnesses in shared code; Docs
website and Docs screenshots; Sharing the machine with other agents; Agent scratch space; The live install is
off-limits), `website/AGENTS.md` and `website/EDITORIAL_RULES.md` for the docs page, `docs/docs-shots/SPEC.md` for the
shot spec, `plans/AGENTS.md` (Executing), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name.

### PR 1: templates written from now on carry their launch kind

`feat:` (changelog `kind: changed`, a one-line draft: a template an agent creates with only an agent type now switches
the launcher to the agent tab when applied, like one with a model or effort). Small; it may fold into PR 2 if keeping it
separate would leave SPEC.md describing an editor that does not exist yet, which is a DECISION to log.

- SPEC.md, "Launch templates": state D2 as the rule for templates Farhelm writes. The Templates panel and
  `farhelm agent template create` write the agent launch kind into a template that sets any agent-launch choice (model,
  effort, permissions, workspace trust, or the agent type with no command fields) and the command kind into one that
  sets a command, YOLO assertion or resume command; a template that sets only host, destination and session name has no
  kind and applies under either. Templates stored without a kind keep applying as stored until saved again.
- SPEC_impl.md "Launch templates" and the agent CLI section: the same, for the panel and for `template create`.
- `crates/farhelm-helm/src/agent_requests.rs`: `with_agent_kind` already writes the agent kind on create for model,
  effort, permissions or trust; extend it to the agent type given without command fields. `template edit` does not
  change (its merge and its refusal to change a template's kind stay as they are). Focused test.
- `apply_template` (`crates/farhelm-proto/src/launcher.rs`) and the helm's shape-only `PUT /api/templates` do not
  change; there is no migration of stored templates.

### PR 2: the new Templates dialog

`feat:` (changelog `kind: changed`: the Templates panel is redesigned, with editing that works, choices that match the
agent type, labeled fields, and undo for delete). Rewrite `crates/farhelm-ui/src/list/templates.rs` and its section of
`crates/farhelm-ui/assets/app.css`. Keep the module's role: a modal dialog, the same open and close path, the
`TemplatesRevision` bump on close, and focus return to the templates button.

- Two panes. Left: "+ new template", then one row per template with its name and a one-line summary that ellipsizes (the
  list's column must not grow past the dialog; `minmax(0, 1fr)` or equivalent), and a count in the footer. The selected
  row is highlighted the way the launcher highlights a selected choice. Right: the editor for the selected or new
  template. Clicking a row opens it. At phone width the panes stack: the list, then, once a row is opened, the editor
  alone with a "‹ templates" back link.
- Editor, top to bottom: name; "switches launcher to: agent · command · don't switch" (D5); then a "sets" section with
  one row per field the template sets, each with a label, the control, and a ✕; then "+ add field", whose menu lists
  only fields that fit the chosen switch and are not yet set. No field requires another to be set first: a resume
  command or a model without an agent type is fine, since another template stacked with it may supply one, and applying
  it keeps today's rules (D3). A line beside it may name what is left as is.
  - agent: agent type, model, effort, approvals (the permissions choice), workspace trust, host, destination, session
    name;
  - command: command, approvals (the YOLO assertion: "asks for approval" or "runs without prompts (YOLO)", with a hint
    that it is the user's statement and Farhelm does not inspect it), agent type (the declared agent type), resume
    command, host, destination, session name;
  - don't switch: host, destination, session name.
- Changing the switch to one that does not offer a field the template sets flags that field inline and refuses to save
  until it is removed, the same as an agent-type change (D10); it never drops a field silently.
- Both launch kinds call the approvals field "approvals", so the user sees one concept; they remain different stored
  fields.
- Choices follow the agent type the template sets, read from the existing per-harness answers in
  `crates/farhelm-proto/src/launch.rs` (`offers_permission`, `omitted_permission`, `offers_only_yolo`,
  `offers_workspace_trust`, `offers_effort`, `offers_model`) and the release model catalog, never from a new table or a
  comparison of harnesses (root `AGENTS.md`, Harness-specific code). In practice: Claude, Codex, Muse, Cursor and Grok
  offer "default (asks)" and YOLO; Goose offers YOLO (its default), approve, smart approve and chat; OMP offers YOLO
  (its default) and approve; Pi and OpenCode take no approvals choice, so the field is not offered. With no agent type
  set, approvals offers default and YOLO, effort offers the whole effort list, and trust is offered. "Reset to default"
  becomes a "default" value inside a control (with what the default means where that helps), and "leave as is"
  disappears, because a field not shown is left as is.
- Model: a plain free-text input that never rejects what is typed (D3), with a suggestion list of the agent type's
  models from the helm's release model catalog (filtered with the existing per-agent-type helper) when the template sets
  an agent type; with no agent type, no suggestions are required. Do not reuse the launcher's model picker and its state
  machine. If a native suggestion list renders badly in WebKit (the desktop app's engine), clickable suggestions under
  the input are an acceptable fallback.
- Text fields get a label and a placeholder and are sized like their neighbors. Destination is a segmented "folder ·
  fresh GitHub checkout" with a labeled input below ("folder path" or "repository", placeholder `owner/name` and a hint
  that each session gets a fresh checkout). Command and resume command take a monospace input with a hint about the
  placeholders SPEC.md defines (`{cwd}`, `{farhelm_args}`, `{conversation}`).
- Saving is explicit. Show an unsaved-changes marker. Every way of leaving an edited template asks save, discard or keep
  editing, inline in the editor footer rather than as a dialog over the dialog: opening another row, "+ new template",
  duplicate, the close button and Escape. Keep `save_name_refusal` and its tests (a new or renamed template needs a
  loaded list and a free name) and the rename order (save the new name, then delete the old).
- Duplicate opens a copy of the selected template as a new, unsaved template with a free name derived from the original.
- Delete per D4. It sits in the editor's footer. The undo notice lives inside the Templates dialog (there is no app-wide
  toast to reuse, and none is wanted), holds the deleted template's fields client-side only, and ends when the dialog
  closes.
- A stored template with kind-specific fields and no kind opens with the kind inferred from them (D10), marked unsaved
  with a short note that saving will store that switch, because until it is saved the stored template does not switch
  the launcher. One that mixes agent and command fields (it can never apply) opens as one kind with the other side's
  fields flagged as above.
- Help text per D6.
- Every control keeps a `data-tooltip`, as the tooltip coverage test requires.
- Tests: keep and adapt the translation tests (`fields_from_form`/`form_from_fields` round trip or whatever replaces
  them), and add focused unit tests for what each switch offers, the per-agent-type choices, the flagging rule and the
  kind written on save. Rewrite `e2e/tests/templates.spec.ts` for the new dialog (create, edit an existing template
  through its row, rename, delete and undo, the name-taken refusal, a template with a long summary still reachable), and
  keep `e2e/tests/tooltip-coverage.spec.ts` passing.
- SPEC.md (the Templates panel sentence) and SPEC_impl.md ("Launch templates": the panel paragraph) describe the new
  dialog.
- The PR that completes the dialog removes the three TODO.md entries this plan covers.

### PR 3: docs and TODO

`docs:`. Update `website/src/content/docs/docs/using/launch-templates.mdx` for the new dialog, following
`website/AGENTS.md` and `website/EDITORIAL_RULES.md`: making, editing and deleting a template (with undo), "switches
launcher to", that templates stack, and that only the fields a template sets are applied. Update
`e2e/docs-shots/launch-templates.spec.ts` and the screenshots' alt text so the shots show the new panel and editor
(`docs/docs-shots/SPEC.md` governs; the staged templates in `docs/docs-shots/scenario.json5` may need a kind added under
D2). Capture locally with `scripts/docs-screenshots.sh --only launch-templates`, look at every image it prints, and fix
the shot spec or annotations until each shows what its alt text says and hides nothing the reader needs (D8). Never run
`scripts/publish-docs-shots.sh` and never commit a screenshot. This PR changes Markdown and a shot spec, so it gets the
review gate only for the spec change.

### Validation

Follow root `AGENTS.md` "Finishing work" and pick the smallest checks that cover each PR:

- PR 1: `cargo fmt --all -- --check`, clippy on the touched crates, and a focused nextest selection for the agent
  template create tests through `scripts/record-test-run.py` (with the pinned nextest and tmux setup from
  `docs/test-run-evidence.md`), plus `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if Rust
  tests changed.
- PR 2: fmt, `cargo clippy --all-targets -- -D warnings` for the UI crate, the UI crate's unit tests through the
  recorder, `cargo check -p farhelm-ui --features desktop`, and the browser specs `e2e/tests/templates.spec.ts` and
  `e2e/tests/tooltip-coverage.spec.ts` on Chromium and WebKit through the recorder (builds per root `AGENTS.md`). Also
  run any other browser spec that opens the Templates dialog or applies templates in the launcher (search `e2e/tests`
  for `templates`). Run the test-sleep checker for browser test changes.
- Visual inspection for PR 2, as an acceptance criterion: capture the dialog in Chromium and WebKit at a desktop width
  and at phone width (an agent template, a command template with the add-field menu open, a don't-switch template with a
  GitHub checkout destination, the unsaved prompt and the undo notice), look at every image, and fix what looks wrong:
  overflow, clipped or overlapping text, unlabeled controls, misaligned rows. Write the captures to the scratch
  directory, not the checkout. Use the dark theme the app ships, and the light one if the app has one.
- PR 3: `dprint check` on changed files, the website build
  (`cd website && bun install --frozen-lockfile && bun run
  build`), the local screenshot capture and inspection above.
- `python3 releasing/check-changelog.py format` for every PR with a fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-templates-dialog-overhaul-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/templates-dialog-overhaul/<nn>-<short-name>`.
- The stack follows Outline: PR 1, PR 2, PR 3. Keep PRs bite-sized without churn: PR 2 may split into two PRs (for
  example the two-pane list and editor with its fields first, then duplicate, delete with undo and the unsaved prompt)
  if each still builds, passes its tests and leaves a working dialog; never add code in one PR that a later PR of this
  stack deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. Every `feat:` or
  `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases
  and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The PR that completes the dialog removes the three TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate two independent
reviews of that PR's changes, and address what both find before moving on. The user demands exactly these reviewers, and
no review swarm (D9):

- a fresh-context agent on Opus 5.5 at high effort;
- a fresh-context agent on gpt-6-astra at high effort, shelled out to the harness that serves that model when the
  executing one cannot reach it natively.

Both get the same prompt, carrying the full charter, because neither reviewer has anything else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a separate
findings file per reviewer (in the scratch directory), and the acceptance criteria for that PR: its part of Outline, the
decisions D1 to D10 that apply to it, and the goal's acceptance criteria. For a PR that changes tests or fixtures,
include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. For PR 2, give the
reviewers the visual-inspection captures as well. Where you disagree with a finding, decide on the merits and log the
DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics. A PR that
changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new shared abstraction
across the launcher and the Templates dialog, a new persisted record, a change to the template wire format or the helm's
template API, a migration of stored templates), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions
above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
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
alternatives considered: in particular each Conventional Commit type and changelog kind, whether and where PR 2 was
split, how the launcher's controls were reused or not, the exact help text, and every review finding you decided not to
follow. The user will ask for these later.

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
