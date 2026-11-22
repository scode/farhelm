# Compact managed checkout in the launcher

Written against main at 6bc33a609 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Managed checkout layout in the launcher explodes.** the layout explodes due to the long message and two controls for
> managed checkouts (the vertical size of the box towards the top). we need to fix (Screenshot: the launcher's
> Destination box with Managed checkout selected; under the Repository field, a wrapped "Managed checkout of <repo> on
> <host>" line with the full checkout path, a paragraph explaining that Farhelm clones into a new folder named after the
> session and trashes it when the last session using it is deleted, and then a separate Folder field repeating the path,
> all in the narrow middle column, which makes the Destination box very tall.) Plan:
> `plans/queue/launcher-managed-checkout-compact.md`.

### Where it stands (from planning research at main 6bc33a609)

The Destination card is built in `crates/gui/farhelm-gui/src/launcher/view.rs`; the managed-checkout parts live in
`crates/gui/farhelm-gui/src/launcher/view/checkout.rs`.

- **Today, on an ordinary host** (view.rs about 2329), Managed checkout draws Host in its own bordered box
  (`ADDRESS_HOST_WIDTH`), a flexible middle column, and Name beside them (`address_and_name`, which wraps Name below
  when narrow). The middle column comes from `checkout_rows` (checkout.rs about 1056): the inline checkout-folder setup
  field (`setup_field`) or its refusal, deliberately above the repository field ("The old order: the setup field above
  the repository field, which it is the precondition for"); the Repository field (`launcher-repository-row`, with the
  suggestions anchor `launcher-repository-anchor`); the incomplete-scan note (`repository_note`); the preview block
  (`preview_block`): the heading "Managed checkout of `<repo>` on `<host>`" (`launcher-preview-heading`) over the path
  (`launcher-preview-path`), or "Waiting for a current checkout preview", or the helm's refusal in warning ink
  (`launcher-preview-error`), in one polite live status region (`Role::Status`, id `launcher-preview`, selectors
  `launcher-preview-ready`, `-pending`, `-failed`, labelled by the heading); and the always-shown `EXPLANATION`. Then
  view.rs adds a Folder row (`launcher-folder-row`) holding a read-only input showing the path again
  (`checkout_folder_field`, input id `launcher-checkout-folder`, kept in step by `show_checkout_path`).
- **The ordinary Folder destination** uses `address_line` (view.rs about 1546): one bordered field with the Host segment
  (`launcher-host-row`) and a right segment under a label row whose right label is padded to sit over the embedded
  input's text; the folder input is `.embedded()` and the border turns accent while it has focus. A new environment
  already reuses `address_line` for Host and Recipe.
- **The new-environment branch** with Managed checkout (view.rs about 2298) draws Recipe in the address, the container
  host, the same `checkout_rows` (whose preview is `environment_checkout`: "Managed checkout of `<repo>` in the new
  environment" / "Cloned inside it once it starts", selector `launcher-preview-environment`), then the Folder row with
  the placeholder "Cloned inside the new environment". `EXPLANATION`'s trash sentence shows there too, although a
  checkout inside an environment is not trashed on Delete.
- **SPEC.md:** about 1428-1430 ("Repository and checkout fields, or a new environment's recipe and place, occupy the
  space beside Host when chosen. The optional session name sits beside the address and drops below it when the form is
  narrow."); about 1718 ("Selecting managed checkout reveals a repository field and explains the directory's
  lifetime."); about 1914 ("In managed-checkout mode, the composer's folder field shows the current preview's effective
  path read-only, or a pending or error state while no path is available."). SPEC_impl.md about 4916-4917 ("The composer
  reads its checkout folder field from that accepted preview. It leaves the field read-only until …"). For a checkout
  inside an environment: SPEC.md about 1634 (Delete discards it rather than trashing it) and about 1730-1732 (Replace
  trashes it inside the environment, discarded with the environment).
- **Tests reading these pieces:** `launcher/checkout_tests.rs` (the folder input's text through `checkout_folder`, 15
  call sites, and its placeholder through `checkout_path_placeholder`, 3; `launcher-preview-error` by name;
  `launcher-repository-row`), `templates/save_tests.rs` (`launcher-preview-error`), `launcher/environment_view_tests.rs`
  (`launcher-repository-row`, `launcher-preview-environment`). Nothing outside the GUI crate's tests uses those ids; the
  docs screenshot script uses `launcher-destination-section` and the setup field's own `checkout-folder` and
  `checkout-folder-save`, which stay.
- **No GUI golden scene shows a managed checkout** (`crates/gui/farhelm-gui/src/goldens/launcher.rs`, scene table in
  `goldens/mod.rs`; `launcher-narrow` is 720x900). The docs screenshot `checkout-setup`
  (`scripts/docs-shots/start-a-session.py`) crops the Destination card in managed-checkout mode with the setup field
  showing.

### The agreed design (layout B, ordinary host)

The maintainer chose this layout after seeing headless renders of today's card and of this layout at 1080x1160. The
renders are not available to you; this section is the design. Only this section's ordinary-host layout and its note
wording are the maintainer's; the parts marked "planner proposal" here and below are not.

```
DESTINATION                                          [ ▭ Folder | ⎇ Managed checkout ]
Host                     Repository                         Name · optional
┌────────────────────────┬────────────────────────────────┐  ┌──────────────────────┐
│ ▭ local             ⌄  │ acme/widget                    │  │ widget-3             │
└────────────────────────┴────────────────────────────────┘  └──────────────────────┘
Folder  ⎇  ~/src/checkouts/widget-3
New folder named after the session; it moves to the trash when its last session is deleted.
```

- **Row 1** is the Folder destination's address line, reused: one bordered field, the Host segment at its usual width,
  then a Repository segment holding the repository input embedded the way the folder input is (no border of its own, the
  address's height, monospace), with no Browse. "Host" and "Repository" label their segments as "Host" and "Folder" do
  in the Folder destination. **Name · optional** sits beside the address as today and drops below it when the card is
  too narrow, by the existing rule.
- **Row 2 spans the full card width** (under the address and Name): the label "Folder" in field-label ink and size, the
  branch glyph (`Icon::Branch`, as on the Managed checkout switch segment), and the clone's path as plain, unboxed text
  in the terminal font at the size the held folder uses (`launcher-folder-held`). Not an input, not boxed.
- **Row 3 spans the full card width:** one quiet help line, exactly "New folder named after the session; it moves to the
  trash when its last session is deleted."
- **Gone:** the "Managed checkout of `<repo>` on `<host>`" heading, the separate preview path, the four-line
  explanation, and the boxed read-only Folder input.

Row 2's states, drawn in the same place after "Folder" and the glyph:

| State                | What row 2 shows                                              |
| -------------------- | ------------------------------------------------------------- |
| No repository chosen | "Select a repository", quiet ink                              |
| Preview pending      | "Waiting for a current checkout preview", quiet ink           |
| Preview ready        | the path, terminal font, ordinary ink                         |
| Preview refused      | the helm's refusal words, warning ink (`palette.warn_bright`) |

Planner notes on this layout (proposals, not the maintainer's words):

- The planning mockup drew the Repository label about 8 px left of the input's text. Treat that as a mockup artifact:
  label the segment the way `address_line` already labels Folder.
- The address border turns accent while the repository input has focus, as it does for the folder input.
- A long path wraps inside the card rather than widening it.
- The setup field (or its refusal) keeps its documented place above the repository field: full width, above the address
  line. The incomplete-scan note goes full width directly under the address and Name, above row 2.

### New-environment branch (planner proposal)

Recipe keeps the address's right segment, so Repository stays a full-width row of its own under the address and the
container host, as today. Below the address-and-name block, rows 2 and 3 replace the environment preview block and the
Folder row:

- row 2 reads "Folder ⎇ Cloned inside the new environment once it starts" in quiet ink (merging today's two strings);
- row 3 must not promise the host's trash. Proposed: "New folder named after the session, inside the new environment;
  deleting its last session removes it permanently." Check it against SPEC.md about 1634 (Delete) and about 1730-1732
  (Replace) before using it; it names deletion only, because Replace trashes it inside the environment.

No golden covers this branch; the report says what was built (see "Unattended fallback").

### Acceptance criteria

1. On an ordinary host in Managed checkout mode, Host and Repository render as one bordered address field
   (`launcher-address`, segments `launcher-host-row` then `launcher-repository-row`), labelled as the Folder destination
   labels its address, with Name beside or below it by the existing wrap rule. There is no separate Host box.
2. Below the address and Name, a Folder line (`launcher-folder-row`) and the lifetime note each span the Destination
   card's content width and show what the design section and its state table say. The preview heading
   (`launcher-preview-heading`), the separate preview path line, the old explanation sentence and the read-only Folder
   input (`launcher-checkout-folder`) are not drawn.
3. The Folder line is one polite live status region (`Role::Status`, id `launcher-preview`), labelled "Folder" for
   screen readers, with the selectors `launcher-preview-ready`, `-pending` and `-failed` on those states. The path's
   node id is `launcher-preview-path`; the refusal's node id is `launcher-preview-error`, its accessible name exactly
   the helm's words; the pending and "Select a repository" words are labelled nodes (plain text is not in the
   accessibility tree). The glyph is hidden from assistive technology.
4. The repository suggestions list opens under the Repository segment, matching its width, and arrows, Enter and Escape
   behave as today.
5. In the new-environment branch, the Repository row, the Folder line (selector `launcher-preview-environment`) and a
   lifetime note show as the planner proposal says or as a logged fallback decides; the host-trash sentence never shows
   there.
6. SPEC.md about 1914 reads "the composer's Folder line shows …" ("folder field" becomes "Folder line", nothing else),
   and SPEC_impl.md about 4916-4917 reads "The composer reads its checkout Folder line from that accepted preview. It
   leaves the folder read-only until …". SPEC.md about 1428-1430 and 1718 stay. The checkout module's docs and the
   view.rs comments that describe the old rows say what is now drawn.
7. Tests cover: the layout (the Repository segment inside `launcher-address`, right of `launcher-host-row`; the Folder
   line below the address and Name, spanning the card; no `launcher-preview-heading`); each Folder line state with its
   selector and the live region; the suggestions list's left edge and width matching the Repository segment's; the
   environment branch's Folder line. Existing tests pass against the new drawing, with
   `managed_checkout_hides_the_folder_until_a_repository_is_chosen`'s docstring describing the Folder line rather than
   "the folder field".
8. Two new GUI golden scenes, light and dark: Managed checkout on an ordinary host with a repository chosen, a name
   typed and the preview ready, at 1080x1160, and the same at 720x900. They are listed in
   `docs/gpui/screenshot-scenes.json`'s `foundation_goldens` (`docs/gpui/screenshot-batch.md`, "The scene list") and
   published per `.agents/validation.md`. No existing golden changes.
9. A `fix:` PR with a changelog fragment (`kind: fixed`), and the TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page, after seeing renders of today's card, a trimmed middle
column (option A) and this layout (option B):

- **Layout B** for an ordinary host, as "The agreed design" describes it, with the path as plain text (the maintainer
  accepted the mockup as shown).
- **SPEC wording** changes only where it names the read-only folder field, as little as possible (criterion 6).
- **The new-environment branch and the narrow width** are in scope only as far as consistency needs.
- **Out of scope:** the template editor's own longer copy of the sentence
  (`crates/gui/farhelm-gui/src/templates/editor/rows.rs`).
- **Goldens:** two new managed-checkout scenes, standard and narrow, both appearances. The docs screenshot
  `checkout-setup` changes at the next docs refresh only; never commit captured media (`.agents/docs-media.md`).
- **One PR** with a changelog fragment, removing the TODO entry.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort, plus the mandatory Opus visual review
  (see "Visual review").
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the decisions above and "The agreed design".

**Binding repository constraints:** the SPEC.md and SPEC_impl.md passages quoted under "Where it stands";
`.agents/test-authoring.md`; the GPUI goldens check and publishing (`.agents/validation.md`);
`docs/gpui/screenshot-batch.md`, "The scene list"; `.agents/docs-media.md`; root `AGENTS.md`, "Conventional Commits",
"Changelog fragments", "Tests and validation", "Code rules" (the environment note is one sentence for every environment,
never a provider branch) and "Instructions by path" for every path you touch.

**Planner proposals:**

- Everything marked "planner proposal" or "Planner notes" above, and the type `fix:` (the TODO entry calls the layout a
  bug).
- **Shape:** split `checkout_rows` into the Repository segment (the embedded input with its anchor, given to
  `address_line` as its right side, or to its own row in the environment branch) and the rows drawn after
  `address_and_name` (the incomplete-scan note, the Folder line, the note), with the setup field drawn above the
  address. The ordinary-host branch becomes an
  `address_line(host, repository, "Repository", "launcher-repository-row", None, ink, palette)` call.
- **Draw the Folder line straight from the preview,** as `preview_block` draws its path today, and remove what only the
  read-only input needed (`checkout_folder_field`, `show_checkout_path`, the `checkout.path` text state and its
  placeholder, the test helpers `checkout_path_text` and `checkout_path_placeholder`); the 18 test call sites read the
  Folder line's state and words through one replacement helper. The input's placeholders "Waiting for checkout preview"
  and "Checkout preview unavailable" go.
- **Accepted consequence of plain text:** the read-only input was a tab stop with a selectable path; the line is
  neither, as the held folder is not. The live region still announces each state.
- **Goldens:** two `LauncherScene` variants in `goldens/launcher.rs` (for example scenes `launcher-managed-checkout` and
  `launcher-managed-checkout-narrow`) that choose Managed checkout, pick a repository through the draft's own choice (as
  `environment_view_tests.rs` does), type a name, and answer the suggestions and preview reads from the fake helm with a
  realistic path.

## Outline

1. `plan/launcher-managed-checkout-compact/01-compact-checkout-destination`: `fix:`, with a changelog fragment
   (`kind: fixed`). Everything in the acceptance criteria: the layout, the Folder line and note, the environment branch,
   the SPEC wording, docs comments, tests, the two goldens with their scene-list entries, and the TODO entry's removal.

## Fallbacks

- **The environment note:** if the proposed sentence turns out inaccurate for some environment, do not branch on
  provider; use wording true for every environment (or the Folder line alone, if nothing is), log it as a DECISION, and
  say so in the report. Never show the host-trash sentence there.
- **The environment layout:** if moving rows 2 and 3 below the address-and-name block needs more than that (a reflow of
  Recipe, container host or Name), put the Folder line and note where the preview block and Folder row are today, log
  it, and report it.
- **Setup field placement:** if above the address line reads wrong in the render, you may move it under the address only
  with a logged reason, and say so in the report.
- **The text state** behind the old input turns out to have another reader: keep it as the model behind the Folder line,
  render it as plain text, and log it.
- **If `launch-wakes-suspended-host` has landed first,** the composer shows a new folder state for a managed checkout on
  a resumable host ("The folder is picked automatically when the host is ready", or "… named after the session when the
  host is ready"). Carry it into the Folder line as another quiet-ink state with its own selector, keep its tests
  passing, and log it.
- `docs/gpui/parity/launcher.md` mentions the preview; it is a migration ledger, so updating it is your call (log it).
- A changed existing golden is a red flag: none shows a managed checkout, so investigate before accepting one.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launcher-managed-checkout-compact-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-launcher-managed-checkout-compact-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/launcher-managed-checkout-compact/01-compact-checkout-destination`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Managed checkout layout in the launcher
  explodes"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Visual review

The maintainer also demands a visual review for every PR that changes what the desktop app draws: a fresh-context
`claude-opus-5-5` agent at `high` effort, launched natively where your harness can (otherwise through
`$scode-harness-shellout`), separate from the reviewer above. Give it headless renders of every changed scene in both
the light and the dark appearance (the GUI golden scenes, regenerated with
`python3 -B scripts/check-gui-goldens.py --update`, and the golden changes page that run writes), the design this file
describes (its mockup text and wording are the agreed design), and the PR's diff. It must be satisfied that the rendered
result matches the intent of the agreed design; loop until it reports no mismatch. A mismatch with the agreed design is
binding and is fixed; any other observation it makes is an input, handled like the reviewer's findings above. Do not
write a launch command here or in the log.

For this plan the binding agreed design is the ordinary-host layout B in "The agreed design": the sketch, its row
bullets, the state table and the note's exact wording. The parts this file marks as planner proposals or planner notes
(label alignment, focus ink, wrapping, the setup field and scan note placement, the new-environment branch) are inputs,
handled like the reviewer's other observations. The two scenes are new, so the changes page shows them without a
reference: give the reviewer both scenes' light and dark captures.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

The report's "things you should know" also says, in every case:

- the new-environment branch's layout and its Folder line and lifetime note, quoted word for word as built, were the
  planner's proposal, which the maintainer has not seen and no golden shows;
- the docs screenshot `checkout-setup` will change at the next docs refresh (nothing was captured or committed here).

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate and,
where it changes what the app draws, its visual review, and the last removing the TODO.md entry this plan covers. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
