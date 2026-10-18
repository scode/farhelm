# Templates: base session names that number themselves when taken

Written against main at d526038e on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Quick-launching a template that sets a session name and a managed-checkout destination works once, then strands New
Session for as long as the first session's checkout exists. Fix it the way the maintainer decided: a template's session
name gets a kind. A base name is used as-is when free and otherwise numbered (`app-review`, then `app-review-2`, `-3`,
…), for the session title and the managed-checkout folder alike. A fixed name behaves exactly as every template name
does today. It is a property of the template, so it behaves the same however the template is applied.

TODO.md entry this plan covers (Near term): "**Allow repeated quick launches from one setup.** Quick-launching a recent
managed-checkout setup while an earlier session from it is still running can strand New Session with the repository name
prefilled in Name. The checkout preview request (`POST /api/github-checkout-preview`) returns
`409 Conflict: a directory with that checkout name already exists; choose another title`, the preview field says
`checkout preview unavailable`, and Launch is disabled with `wait for a current checkout preview before launching`.
Typing `name:...` to pick another name works around it. Diagnose the naming and preview path so the same setup can be
quick-launched repeatedly without manual renaming; the exact naming behavior is TBD."

### Diagnosis (from the planning session's code reading; verify it)

A quick launch (⚡ / Cmd+P, number keys, sequences, the quick switcher's Quick launch row) never reads recent setups. It
opens a hidden New and replays the bound templates into it (`crates/farhelm-ui/src/list/view.rs`, the `quick_launch`
callback). `template_edits` in `crates/farhelm-ui/src/list/create_form.rs` turns the template's session name into a
`ComposerSearchResult::Name` edit, and `apply_composer_search_result` marks that name as typed (`title_edited`). The
launch sends it as an explicit title. The checkout preview (`farhelm-helm/src/sessions.rs`, `preview_github_checkout`,
then the supervisor's `github_checkout_preview` in `crates/farhelm-supervisor/src/service/core.rs`) names the folder
with `checkout_basename` (`crates/farhelm-proto/src/github_checkout.rs`): no title gives the lowest free `repo-N`; an
explicit title gives `repo-<slug>` or `NameError::Occupied`. The first session's folder still exists, so the supervisor
answers 409 "a directory with that checkout name already exists; choose another title", the preview fails, the hidden
launch reveals New, and Launch waits for a preview that never comes (`LaunchPrerequisites::refusal`). Only managed
checkouts are affected, because only there a name becomes a folder. The suffix search that could have helped exists only
for Clone's untouched default name (`CloneCheckoutNaming` in `create_form.rs`, advanced on an occupied preview in the
preview-result effect; `-clone`, then `-clone-2` through `-clone-50`). No test covers it:
`e2e/tests/github-checkouts.spec.ts`'s quick launch test uses an unnamed template, and
`crates/farhelm/tests/e2e/github_checkouts.rs` deliberately locks in refusal of an agent's explicit `--title`, which
must keep passing.

### Acceptance criteria

- Reproduce first. Before changing product code, write the regression browser test (quick launch of a template with a
  session name and a managed-checkout destination, while a folder of that name is occupied) and run it through the
  recorder against unchanged `main@origin`. Record the failing run id in the working log. The same test passes in the PR
  that fixes it. If it does not fail on unchanged main, find out why before building anything, and log what you found.
- A template's session name is a base name or a fixed name. Every template already stored with a session name is a base
  name, with no rewrite of stored data. A newly added session name defaults to base (see planner proposals).
- Base name, managed-checkout destination: the launch uses the name as-is when its folder is free, otherwise the first
  free of `<base>-2`, `<base>-3`, … up to `<base>-50`, the same cap as Clone's search. Title and folder use the same
  number (`app-review-2` gives folder `<repo>-app-review-2` under the existing slug rule). At the cap the conflict stays
  visible as it does for Clone. An empty slug or an over-long name is refused exactly as today.
- Base name, folder destination: the name is used as-is; no numbering (planning-session default below).
- Fixed name: exactly today's behavior for every path. A clash is reported, never silently renamed.
- A name typed in the launcher (the Name field or `name:`): exactly today's behavior.
- Repeated quick launch of a base-name template with a managed-checkout destination starts the second, third, … session
  directly, each in its own numbered checkout, with no New revealed. Number keys, sequences, the quick switcher's Quick
  launch row and `tl:` in the launcher behave the same, through the one shared template-application path; no code
  special-cases quick launch. (Cmd-J, being added by another plan, goes through the same path and gets this for free; do
  not wait for it or add code for it.)
- Launcher Name field after applying a base-name template: it shows the resolved name, with a small pill next to the
  Name label reading "from template · numbered if taken". When the base was taken, the field shows the numbered name
  (`app-review-2`) with a faint note "(app-review is taken)". The pill shows only while the launch goes into a managed
  checkout; with a folder destination the name is used as-is and no pill shows. Typing in the field makes the text the
  user's own name, which then behaves as a typed name (pill gone, no numbering). Both new elements carry tooltips
  (`e2e/tests/tooltip-coverage.spec.ts` holds every visible control to that).
- A fixed-name template applied in the launcher fills Name as a typed name does today, with no pill.
- Templates stack as today: the kind travels with the name, and the last template that sets a name decides both.
- Lost race: a base-name launch whose create loses the displayed folder to another create behaves like Clone's untouched
  default today. Launch reports the conflict and obtains a new preview, which may advance the number, and needs another
  click on Launch; a quick launch reveals New with the conflict and the new preview. It never submits again by itself
  and never silently uses a directory the user was not shown.
- Templates editor: the session-name field's label row gets a two-option segmented control "base name | fixed name",
  with help text "Base name: used as-is when free, otherwise numbered (app-review-2, -3, …). Fixed name: used exactly; a
  clash is reported." Adding the session-name field starts on base name. Removing the field removes its kind. Duplicate
  and undo carry the kind. Showing the kind in the Templates list summary is optional.
- Save as template: the session-name row offers the same base/fixed choice. A name the user typed is saved as a base
  name by default. A name that came from a template keeps that template's kind, and one from a base-name template is
  saved as its base (`app-review`), never as the numbered name the launcher shows.
- Base is stored as absent everywhere: the editor and save as template write no kind for base, and the save reply
  carries the normalized definition, so the editor shows no unsaved changes right after saving a base name.
- Clone: applying a base-name template in Clone replaces Clone's `-clone` default series with the template's series,
  just as a typed name replaces it today. Only one series is active at a time.
- Agent CLI: `farhelm agent template create` and `edit` take `--title-kind base|fixed`. On create it defaults to base;
  on edit, `--title` alone keeps the stored kind, and `--title-kind` alone changes only the kind of a template that has
  a session name. A kind without a session name is refused. The template approval card shows the kind, the
  `farhelm agent templates` listing shows it, and `farhelm agent instructions` explains it.
- Agent create (PR 4, the separable final PR): `farhelm agent create --template X --gh owner/repo` with a base-name
  template numbers the name the way the launcher does, and its approval card shows the numbered name and path. An
  agent's own `--title` stays a fixed name: the existing refusal in `crates/farhelm/tests/e2e/github_checkouts.rs` keeps
  passing unchanged in every PR. Until PR 4, agent create behaves as today (refused when the folder is taken); no PR
  documents that interim state.
- SPEC.md, SPEC_impl.md and the docs page `website/src/content/docs/docs/using/launch-templates.mdx` describe the new
  behavior (paragraphs listed under the outline). Changelog fragments per root `AGENTS.md`.
- The last code PR removes the TODO.md entry above (it ends with this plan's `Plan:` reference).

## Requirement sources

**The user's decisions (2026-10-18), binding.** The maintainer, verbatim:

> "in this particular quick flow, I would want the name to act as a default, and that there is an auto-allocation of a
> name if it's already taken. But of course, that is slightly different than if I'm in the launcher editor and picking a
> name, where I would just want the feedback that the name is taken... we also don't make it so that in the normal,
> slower path, if I pick a name that already exists, it shouldn't just silently change my name? I should know that I
> picked a name that conflicts. I think in an ideal world, maybe we have something like a base name and a fixed name.
> And so if I have a template that has a base name, then it will use the base name literally if it's available, or
> otherwise auto-allocate a name based on it. And if it's a fixed name, it behaves like today. ... it doesn't become a
> special case for the quick launch. It just means that there's a different template, like you have to set the template
> up like that. So I can now use the template, for example, in the launcher dialog as well."

Settled from that:

- A template's session name has a kind: base name or fixed name. Base: used as-is when free, otherwise numbered
  (`app-review`, `app-review-2`, `-3`, …), for the session title and the managed-checkout folder alike, reusing the
  existing Clone suffix machinery and its cap rather than a second numbering scheme. Fixed: exactly today's behavior. It
  is a property of the template and behaves the same however the template is applied: ⚡, number keys, sequences, Cmd-J,
  and `tl:` in the launcher.
- A name the user types in the launcher keeps today's behavior: a clash is reported, never silently renamed.
- In the launcher, after applying a base-name template, Name shows the resolved name with a small marker. The agreed
  mockup: a pill next to the Name label reading "from template · numbered if taken"; the field shows `app-review-2` with
  a faint "(app-review is taken)". Typing in the field turns it into the user's own name.
- The Templates editor gets the base/fixed choice next to the session name. The agreed mockup: a two-option segmented
  control "base name | fixed name" on the session-name label row, help text "Base name: used as-is when free, otherwise
  numbered (app-review-2, -3, …). Fixed name: used exactly; a clash is reported." "Save as template" and the agent CLI
  (`farhelm agent template create … --title`) also need a way to set the kind.
- Existing templates that already have a session name become base names, so the maintainer's ⚡ template starts working
  without an edit.
- No-workhorse mode. Review gate: a single fresh-context gpt-6.1-sol agent at high effort with the general charter; no
  swarm.

**Further user decisions (2026-10-18), binding:**

- Agent create numbers base names too (PR 4 is in the plan). The maintainer, verbatim: "definitely. base name should be
  base name no matter how its created."
- The downgrade limit (an older build cannot use any template once one is fixed) is accepted. The maintainer, verbatim:
  "but if its about downgrading nothing neede din changelog entry downgrades are not supported". So no changelog
  fragment mentions it; the factual note stays in this plan and the report only.
- The "from template · numbered if taken" marker shows only when the launch goes into a managed checkout.
- Save as template from a typed name saves a base name.
- Numbering only concerns managed-checkout folders. Outside a managed checkout two sessions may share a title, as today,
  so a base name is used as-is there. (Proposed by the planning session as a default; the maintainer let it stand.)

**Binding repository constraints:**

- SPEC.md, Creation: the Launch templates bullet ("A launch made after applying templates is ... exactly the launch the
  same hand edits would have made: the resulting choices count as the user's explicit selection"), the Quick launch,
  number shortcut and sequence bullets, and the Templates editor and save-as-template text. SPEC.md, Managed checkouts:
  "If another create wins the displayed path, Launch reports the conflict and obtains a new preview; it never submits
  automatically or silently chooses another directory. An explicit name remains a conflict rather than gaining a
  suffix." Typed and fixed names keep that; base names are the new, specified exception, alongside Clone's untouched
  default. SPEC.md, Agent-spawned sessions: the template write verbs, the templates listing, and "an agent's write ...
  lands only on the template its card showed".
- SPEC_impl.md, GUI: Dioxus, the paragraph on Clone's untouched name ("Occupied-name previews advance only that default
  through `-clone`, then `-clone-2` through `-clone-50`"; the preview API's occupied-409 classification; generation
  checks rejecting late results; a retained ambiguous attempt freezing its name; "Re-preview never submits a launch by
  itself").
- SPEC.md, Upgrade compatibility and client scale: no change may break the update path from any stable release since
  v0.23.0; agents must alert the maintainer to potential loss of state and absence of a downgrade path. See the storage
  proposal below; the report restates the compatibility facts.
- The protocol rules in the docs of `PROTOCOL_VERSION` (`crates/farhelm-proto/src/lib.rs`): `TemplateFields` refuses
  unknown fields and travels inside agent verbs and replies across the helm and supervisor connection.
- Root `AGENTS.md`: Tests and validation, Changelog fragments, Code rules, Talking to the user. `website/AGENTS.md`
  before editing the docs page (an unattended run starts no preview server and names the changed page in its report).

**Planner proposals** (implementation choices, each with its reason; the executor may replace one with a simpler shape
that keeps the decisions and constraints, and logs that as a DECISION):

- Representation: a new optional `name_kind` field on `farhelm_proto::launcher::TemplateFields` holding an enum
  `Base | Fixed`, where absent means base. Reason: absent-means-base makes every existing template a base name with no
  migration and no schema version bump, as the decision wants, and the field must be `Option` so an agent's
  `template edit` can tell "not given" from "base" in `merge_template_fields`. A kind without a session name is refused
  by `check_template_shape`.
- Base is absent everywhere; only `Fixed` is ever stored. The Templates editor's "base name" option and the save-as-
  template panel write no kind. `put_template`'s reply clears `Some(Base)` the way it already clears retired workspace
  trust, so the editor's saved baseline equals what it sent and shows no unsaved changes after a save (the editor's
  dirty check compares draft and baseline, `templates.rs`). `templates::store_template`, which the PUT and the agent
  verbs share, drops `Some(Base)` too, which catches the agent CLI's explicit `--title-kind base` after
  `merge_template_fields`. Reason: an older build cannot decode a template that carries the new field
  (`deny_unknown_fields`), and its store read refuses the whole catalog rather than skipping an undecodable row. Storing
  base as absent confines that to templates explicitly set to fixed.
- New templates default to base. Reasons: a base name behaves exactly like a fixed one whenever the name is free and
  everywhere outside managed checkouts, so it only differs where a fixed name would refuse; a template exists to be
  launched repeatedly, and a fixed name into a managed checkout launches only once while the first folder exists; and it
  is the same rule as for existing templates, so absent means base everywhere, on the wire and in storage.
- `LauncherState` (the shared state `apply_template` produces, in `farhelm-proto`) carries the name's kind next to
  `name`, so the GUI's pill, stacking, the save-as-template snapshot and PR 4's agent create read it from one place.
- One numbering sequence, generalized in place in the GUI in PR 1: `CloneCheckoutNaming` in `create_form.rs` stays in
  `farhelm-ui`. Candidate `n` of a series is the series base at `n = 0`, then `<base>-<n>` for `n` from 2 to 50. Clone's
  untouched default is the series with base `<source title>-clone`, unchanged in behavior; a template base name is the
  series with the template's name. The type tracks one series at a time: the last name edit decides it, so a base-name
  template applied in Clone replaces Clone's `-clone` series (as a typed name already suppresses it today,
  `default_title` returning `None` when edited), and typing ends any series. The occupied-preview effect, the
  `advance(reconciling)` freeze, `retarget` on host or repository change, `effective_title` and every launch-time read
  (display, preview authority, retry binding, submit, draft snapshot) cover both sources. Reason: SPEC_impl.md already
  requires those reads to agree, and this keeps one advance path. Applying a base-name template sets the naming source
  through its own edit rather than `ComposerSearchResult::Name`; a fixed name still uses that typed edit. The
  quick-launch auto-submit effect already waits while the preview is pending, and an advance re-previews without marking
  the preview failed, so quick launch needs no change of its own. Moving the sequence into `farhelm-proto` happens only
  in PR 4, which is the only other user.
- A lost race on a base name follows Clone's untouched default (SPEC.md, Managed checkouts: "A lost-race launch
  re-previews and may advance the default name, but still needs another click on Launch for the newly shown path").
  Reason: it keeps the existing preview/create contract (the user, or the ⚡ consent, approved one displayed path;
  another path needs another look) and needs no new machinery.
- The launcher's pill shows only while Name holds a template base name and the destination is a managed checkout (the
  user's decision); the "(… is taken)" note only when the number advanced.
- Save as template: a template-supplied base name counts as a chosen name for the checklist (today a template's name
  counts as typed, and stays pre-checked).
- Agent CLI flag name `--title-kind base|fixed`, beside the existing `--title`. The CLI sends `name_kind` only when the
  flag is given, so a new CLI talking to a helm that predates this field fails only when it uses the flag.
- PRs 1 to 3 leave agent create's code alone (it keeps today's refusal until PR 4); their SPEC.md text says nothing
  about agent create numbering, which PR 4 adds, so no sentence is written and then replaced.
- PR 4, agent create numbering (`create_for_agent` in `crates/farhelm-helm/src/agent_requests.rs`): when the resolved
  name is a base name and the managed-checkout preview comes back occupied, advance through the same sequence and
  preview again, up to the cap, before showing the approval card. The loop sits after keyed recovery (SPEC.md: an
  approved first attempt is reconciled before another directory preview or card) and costs one supervisor round trip per
  step. PR 4 moves the sequence helper from `farhelm-ui` into `farhelm-proto` so the helm can share it, and makes the
  helm treat the name in an agent create's own flags (`--title`) as fixed inside `agent_launch::resolve`, without the
  CLI sending anything new (an agent's `--title` is a typed name, and doing it helm-side keeps older CLIs on the same
  behavior). PR 4 is required by the user's decision ("base name should be base name no matter how its created"); PRs 1
  to 3 do not depend on it.
- Protocol version: decide against the `PROTOCOL_VERSION` rule whether the new `TemplateFields` field earns a bump.
  Precedent: #2200 added `prompt` to `TemplateFields` without one (SPEC.md records that template cards carrying it fail
  an older decoder). What is at stake: a supervisor build on the same protocol version that predates this field decodes
  every control frame typed, in its connection loop (`parse_control(&frame)?` in
  `crates/farhelm-supervisor/src/service/connection.rs`), so a `farhelm agent templates` or template-write reply that
  carries a fixed template fails to decode there and the `?` ends that whole helm connection: the host drops, every
  agent request waiting on that connection is refused (`HelmLink::fail_all`), and the helm reconnects; repeating the
  listing repeats the drop. A bump instead refuses that supervisor cleanly at the hello until the helm updates it.
  Verify this against the code, then log the DECISION with that effect stated and the reasoning for bumping or not.

## Outline

A linear stack of four PRs, all required. Bookmarks `plan/repeat-quick-launch/<nn>-<short-name>`. PRs 1 to 3 do not
depend on PR 4 and nothing in them exists only to serve it, so PR 4 stays a separable final PR.

1. `fix: number a template's session name when its checkout is taken` (`01-base-names`). The regression browser test,
   written and run failing on unchanged main first (see acceptance criteria). Then: `name_kind` and its enum in
   `crates/farhelm-proto/src/launcher.rs` (shape check, `apply_template` carrying the kind into `LauncherState`, unit
   tests for stacking and for a kind without a name), the helm's normalization in `crates/farhelm-helm/src/templates.rs`
   (`store_template`) with a test that a stored base name has no `name_kind` and an older-format template still decodes
   as base, `put_template`'s reply normalization, the generalized `CloneCheckoutNaming` in `create_form.rs` with unit
   tests (base used as-is, advance on occupied, cap at `-50`, retarget resets, reconciliation freezes, typing makes it
   typed, folder destination uses the base as-is, a base-name template in Clone replaces the `-clone` series, Clone's
   `-clone` behavior otherwise unchanged), the Name pill and "(… is taken)" note, and the protocol decision. Browser
   tests in `e2e/tests/github-checkouts.spec.ts` (or `quick-launch.spec.ts`, whichever fits its fixtures): ⚡ launches a
   base-name template twice and gets `<name>` then `<name>-2` with matching folders and no New; a fixed-name template
   stored through the API with `name_kind: "fixed"` still reveals New with the conflict; `tl:` in the launcher shows the
   pill and the numbered name, and typing replaces it with a typed name that reports the clash; a lost race on a base
   name reveals New with the conflict and a new preview. SPEC.md (Creation's Launch templates and Quick launch bullets,
   Managed checkouts) and SPEC_impl.md (the GUI: Dioxus naming paragraph) in the same PR. A changelog fragment
   (`kind: fixed` or `changed`, written for someone running Farhelm). Per the user's decision, no fragment mentions the
   downgrade limit.
2. `feat: choose base or fixed session names in templates` (`02-editor-kind`). The Templates editor's segmented control
   and help text (`crates/farhelm-ui/src/list/templates.rs`, `Field::Name`; base writes no kind; removing the field
   clears the kind), a test that saving a base name leaves no unsaved changes, duplicate and undo carrying the kind, and
   optionally the kind in the list summary; save as template's kind choice (base writes no kind) and saving the base
   rather than the numbered name (`crates/farhelm-ui/src/list/save_template.rs`, and `launcher_snapshot` in
   `create_form.rs`); the approval card's template rows (`crates/farhelm-ui/src/approvals.rs`) showing the kind. Tests
   in `templates.spec.ts` and unit tests beside the editor and save panel. SPEC.md's editor and save-as-template text;
   the docs page `website/src/content/docs/docs/using/launch-templates.mdx` (read `website/AGENTS.md` first). Edit PR
   1's fragment.
3. `feat: set a template's session name kind from the agent CLI` (`03-agent-cli`). `--title-kind` on
   `TemplateFieldFlags` in `crates/farhelm/src/main.rs`, `merge_template_fields` in `agent_requests.rs`, the listing's
   human and JSON output, `crates/farhelm/src/agent_instructions.rs`. Tests next to the existing template-verb tests in
   `main.rs` and `agent_requests.rs`. SPEC.md, Agent-spawned sessions: the flag. Edit the fragment.
4. Separable final PR: `feat: number base session names in agent-created checkouts` (`04-agent-create`). Moves the
   sequence helper from `create_form.rs` into `farhelm-proto` (the GUI then uses it from there, behavior unchanged);
   `resolve` in `crates/farhelm-helm/src/agent_launch.rs` carries the kind and treats the flags' `--title` as fixed;
   `create_for_agent` advances on an occupied preview with the shared helper. Tests: an agent create from a base-name
   template into an occupied managed checkout gets the numbered name and path on its card and in the created session
   (helm unit tests with the existing fakes, plus a case in `crates/farhelm/tests/e2e/github_checkouts.rs` beside the
   unchanged `--title` refusal); keyed retries still return the first session. SPEC.md, Agent-spawned sessions: that an
   agent create applying a base-name template numbers it as the launcher does, while `--title` stays a fixed name. Edit
   the fragment. This PR removes the TODO.md entry.

### Validation

Choose per `.agents/validation.md`; at least, for the PRs that touch them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`.
- Through the recorder with the pinned nextest and tmux setup: the `farhelm-proto` launcher and github_checkout unit
  tests, `farhelm-helm`'s templates, agent_launch and agent_requests tests, `farhelm-ui`'s create_form, templates and
  save_template tests, `farhelm`'s CLI tests, and for PR 4 the `github_checkouts` e2e test. Not the whole workspace
  unless a specific risk calls for it.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
  `python3 releasing/check-changelog.py format`; `dprint check`.
- Browser, on Chromium and WebKit through the recorder: the new tests plus `github-checkouts.spec.ts`,
  `github-checkout-composer.spec.ts`, `clone.spec.ts` (Clone's suffix search must not change), `quick-launch.spec.ts`,
  `templates.spec.ts`, `tooltip-coverage.spec.ts`, and `agent-relay.spec.ts` for the approval card. Not the full suite.
- The website build (`cd website && bun install --frozen-lockfile && bun run build`) for the docs page edit.

## Fallbacks

- If the generalized naming will not fit `CloneCheckoutNaming` cleanly, a sibling type in `create_form.rs` sharing the
  same sequence is fine. A second numbering scheme (another suffix shape or cap) is not.
- If PR 4 turns out to need more than a bounded preview-advance loop in the helm (a supervisor protocol change, a new
  reservation, changes to keyed-create reconciliation), run the scope reassessment before building more. Agent create
  numbering is the user's decision, so it is not dropped: if the reassessment finds no bounded design within the helm,
  push PRs 1 to 3 and block with the concrete tradeoff as the question.
- If the regression test does not fail on unchanged main, the bug is not what the diagnosis says. Find the real cause
  and log it; the decided behavior (base and fixed names) is still the goal. Block only if the real cause makes the
  decided behavior wrong or impossible.

Examples of departures that need the scope reassessment: a helm schema version step or rewrite of stored templates,
supervisor-side numbering, numbering for folder destinations, a separate quick-launch naming path, automatic
re-submission after a lost race.

Decisions to log: the protocol-version decision, stating what an older same-protocol supervisor's connection does when a
reply carries a fixed template; the naming type's shape; the changelog kind; any test placed in a different spec file
than proposed.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-repeat-quick-launch-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/repeat-quick-launch/<nn>-<short-name>` (for example `plan/repeat-quick-launch/01-base-names`).
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

The report's "things you should know" states the compatibility facts plainly, in product terms: the update path is
unchanged and existing templates keep working as base names; there is no database migration; once any template is set to
a fixed name, a build from before this work cannot list or use any template at all (no ⚡, number keys, sequences, `tl:`
or an agent's `--template`), and it cannot repair that itself, because its Templates dialog cannot list the template
either: the template has to be switched back to base or deleted on the newer build before going back, which also applies
to returning from a development build that saved one; and whether the protocol version changed, with what an older
same-protocol supervisor does on receiving a fixed template (the protocol DECISION).

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
