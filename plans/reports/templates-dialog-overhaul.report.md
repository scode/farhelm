# Templates dialog overhaul

## What this was about

Long template summaries could push Edit and Delete beyond the dialog’s edge, and editing opened an unmarked form below
the list. The form showed every field, mixed agent and command choices, offered permissions an agent could not use, and
exposed an unlabeled repository input. Delete had no undo.

The approved redesign now puts a selectable list beside the editor at desktop width. On a phone, choosing a row opens
the editor with a link back to the list. Only fields the template sets appear, with explicit add and remove controls.
The destination choice now labels its two inputs: “folder path” for an existing directory, or “repository” with an
`owner/name` example for a fresh GitHub checkout. Applying the latter makes each new session get its own checkout on the
selected host. The three TODO entries about editing, permission choices and the repository field are removed.

## Things you should know

“Switches launcher to” selects agent, command or don’t switch. Newly saved templates carry that choice; templates agents
create also carry the inferred choice, including an agent type given alone and command approval or resume choices given
without a command. Old stored templates keep their existing application behavior until saved. Opening one whose fields
imply a launch type shows an inferred, unsaved switch; saving makes it explicit. Mixed or incompatible fields stay
visible and prevent saving until changed or removed. For example, changing an agent type from Goose to Claude while
keeping “approve” flags that approvals field; choose an offered value or remove the field before saving.

Models accept arbitrary text in the editor, with suggestions from the chosen agent’s existing model catalog. Applying a
template still follows the launcher’s existing compatibility rules. Approvals and other offered choices follow the
agent. Leaving a field out preserves the launcher’s value; a default value resets it. Command approvals remain the
user’s assertion about the command, rather than an inspection of what it runs.

Save is explicit. Leaving an edited template asks to save, discard or keep editing. Duplicate opens a new unsaved copy.
Delete takes effect immediately and offers undo for about ten seconds while the dialog stays open; closing ends that
notice. Undo restores the saved fields and refuses when a fresh name-list read finds the name taken. The API retains its
existing last-write-wins behavior: a competing write between that read and the restore can still race. No migration, new
storage record or template API change was added.

Screenshot inspection caught wrong initial selected values, pale native controls in WebKit and an overlapping phone save
refusal; these were fixed and recaptured. Reviews also led to preserving drafts when reselecting a row, disabling rows
during reload, keeping retry reachable from a phone editor, restoring focus, and stronger operation and lifetime oracles
in the browser tests. A scope reassessment confirmed these local corrections did not need a new subsystem.

The docs page and all five existing screenshot scenarios are updated. Captures were generated and inspected locally; no
screenshots were published or committed, no website deployment or preview server was started. The live website will
continue to show its existing page and published images until a separate deployment and screenshot refresh.

The stack was carefully rebased over the Mac release-test and fresh-checkout Clone work. Clone’s name and destination
policy was checked against template application, and the combined browser selection passed on both engines. Later main
changes added three plans and their TODO references only; those references were preserved without another runtime run.

## Open questions and possible follow-ups

None required for this plan. “Save as template” from the session launcher remains a separate TODO, as agreed. Atomic
cross-client name reservation remains outside the existing template API contract.

## PRs

- [#1678 — Newly agent-created templates choose their launch tab](https://github.com/scode/farhelm/pull/1678/changes)
- [#1687 — Select and edit templates beside their list](https://github.com/scode/farhelm/pull/1687/changes)
- [#1688 — Explain editing and combining templates](https://github.com/scode/farhelm/pull/1688/changes)

These are a linear draft stack, with no PR marked ready or merged by the executor.

## Checks run, reused and skipped

Source checks passed: `cargo fmt --all -- --check`, targeted Markdown `dprint check`, and
`python3 releasing/check-changelog.py format`. The isolated `python -B scripts/check-test-sleeps.py` passed after Rust
and browser changes: 269 delays inspected, none unannotated. The repository’s dprint configuration has no CSS,
TypeScript or MDX formatter; those direct selections find no supported files. Changed docs prose was checked manually;
unrelated paragraph wraps were preserved. An earlier draft used the Markdown stdin formatter before that wrap-only churn
was removed.

PR1’s final focused nextest run `c2624808-5972-4472-888a-03dde00dc70e` passed two tests covering inferred launch choice
and preserving whole templates after refused edits. Helm all-targets Clippy passed on the same production behavior;
subsequent changes were documentation and a stronger test assertion, so that result was reused.

For PR2, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, and
the release web build passed after the principal review corrections. UI nextest run
`5ff2a7ef-8345-46ce-91f0-defc433f7446` passed all 425 unit tests on the initial dialog implementation; focused Templates
run `f1c1a6e5-1576-484b-9371-9a21075bdec6` passed six after corrections. Unrelated tests selected out of the focused run
are not execution coverage. These checks used pinned nextest and tmux where required. Their evidence is reused for the
final dialog: subsequent production edits affect CSS height, suggestion semantics and count rendering, covered by the
browser checks and a successful final web build. The Mac updater change is independent of Templates; its tests were not
repeated for this plan.

Recorded Templates and tooltip coverage on Chromium and WebKit passed all 26 cases in
`a85eb7cf-ed34-42de-9546-d48ecb140220`, one worker and zero retries. After the phone CSS fix, six affected cases passed
in `78e04e0e-8086-444e-aa59-8d8894a60803`. Final create/edit, failed-list retry and suggestion/departure precision cases
passed in Chromium in `1f6c014c-efc2-4ccf-933b-414fba024f79` and WebKit in `13f9d300-7db5-44ee-b622-e42e49ef1c7a`; the
former command’s WebKit portion failed before launch from the wrong browser cache and is not a whole-command pass. The
paused-clock undo test passed both engines in `6b5946ab-ea0f-44f6-9ea1-1f66709c8f10`. Earlier cases are reused where
these final local corrections do not change their behavior.

After the functional rebase, a new release web build passed and eight cases covering template stacking, command/default
behavior, Replace-with host refusal and fresh-checkout Clone passed on both engines in
`24f1bd87-ef1a-4f55-93aa-0fba1ea8d7fb`. This covers the combined revision based on `79f86df8`. The subsequent rebase
added only plans and TODO references, preserving all tested executable behavior.

All 32 final dialog PNGs were inspected: both engines, desktop and phone, agent, command/add-field, don’t-switch
placement, unsaved departure, undo, legacy and flagged states. Full-height phone supplements show scrolled footers. The
final suggestion role and failed-count condition preserve the successful-list geometry, so these images were reused
without another capture.

PR3’s frozen website install/build passed, 31 pages with all internal links valid. Local
`scripts/docs-screenshots.sh --only launch-templates --no-build` passed staging and all five captures in
`72ccbe8b-9eea-4c15-bf17-f046cf08bacb` after annotation and focus corrections; each PNG was inspected. The website build
is reused for subsequent prose and bold-label edits, which change no links, imports or frontmatter. Skipping duplicate
builds reused the matching final web bundle and CLI, whose executable code was unchanged by the rebase. No-build output
is deliberately unpublishable.

Failed same-session runs are retained in the private working log and recorder evidence. In addition to reproduced UI
regressions and browser-cache refusals, concurrently launching capture and browser commands caused shared authentication
file failures in `596e0df7-ad46-4bdc-a4da-5e402bbef595` and `f7c9e76b-6ca7-42ee-b3e4-a69670c9aa02`; both were rerun
serially. Those failures are not replaced by later passes, and none was classified as a latent flake.

The whole Rust and browser batteries, installer, provisioning and release gates were skipped: the selected checks cover
the template behavior and its identified launcher interaction. Searching other browser specs found only a keyboard-order
visit to the unchanged Templates opener, so no additional spec was needed. No hosted CI was requested.

## Review gate outcome

Each code/test PR received independent fresh-context Opus 5.5 and GPT-6-Astra reviews at high effort, without a swarm.
Both PR1 and PR2 cleared their final corrections. Both PR3 reviewers cleared the shot-spec corrections and inspected all
five final captures; the executor applied a final prose qualification about templates without a saved tab choice. No
unresolved behavior or test findings remain. The implementation and validation were performed locally; only the required
reviews, scope reassessment and process cold reads were delegated. Reviewers inspected source and images; the executor
ran and assessed the tests. Commit and PR wording passed separate cold reads. Private orchestration evidence is
retained; native and orchestrator usage counters were unavailable, and resumed Opus counters have unknown overlap and
are excluded from totals.

### Landing

Landed on 2026-10-06 (UTC) as three squash commits on main, in order: #1678 (new agent-created templates record which
launcher tab they switch to), #1687 (the Templates dialog redesigned as a list beside a field editor) and #1688 (the
website page and its screenshot scenarios). Nothing else reached main while they merged.

#### What else was on main

Nothing that could interact: the executor had already rebased the stack over the Mac release-test and fresh-checkout
Clone work and tested the combination, and since then main gained only the planning queue's own bookkeeping.

#### Review before merging

A separate reviewer that had not worked on the plan checked the stack before anything merged and found nothing that
breaks. It confirmed:

- #1678 adds no field to a template; it only fills in the existing launcher-tab field when an agent creates a template.
  An older helm or client reads such a template fine, a new helm reads old stored templates unchanged, and what the
  `farhelm` command sends is unchanged.
- Outside the two browser specs #1687 updates, no test or capture script drives the Templates dialog: the README image,
  the demo video and the other screenshot scenarios create templates through the helm's API or not at all, and the one
  sidebar test that touches Templates only moves focus to its unchanged button.
- The earlier template fixes still hold in the new dialog: a new or renamed template's save is still refused while the
  list is loading, reloading or failed, with the retry button kept, and duplicate and undo go through the same name
  check. A template still names its host by installation, so the helm's check that a template's host still reaches the
  same installation is unaffected.

Two things for later, neither changed during the landing:

- The dialog infers an old template's launcher tab with the same rule the helm uses, but the rule exists in two places,
  so the two could drift apart if one is changed without the other.
- Until the next docs screenshot refresh, the live website page shows the old dialog's screenshots under the new text,
  as the report says.

The queued quick-switcher plan, which waited for this one, assumes only that the launcher applies templates as before,
which #1687 does not change. It plans to reuse the dialog's template summary text, which still exists but is private to
the dialog's code, so that plan will need to make it reachable.

#### Checks

- Reused: the report's checks, including its browser run on the combined revision after the executor's last functional
  rebase. Since then main gained only the planning queue's bookkeeping, and the code on main after the last merge is
  identical to the final stack.
- Skipped: running anything again during the landing, for the same reason.

Nothing in the report above was made untrue by the landing.
