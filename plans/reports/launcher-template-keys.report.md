# Template shortcuts in the session launcher

## What this was about

Template shortcuts used to start a session only when no dialog was open. In New, Clone, or Replace with, they did
nothing, and the quick-launch chord could reach the browser. The maintainer wanted those shortcuts to apply templates to
the open draft, including several presses in order, and wanted Cmd-J to open a template-only list for quick launch. The
keys, labels, dialog rules, and launch paths also needed one shared definition and dispatch path so later shortcut
changes do not drift between screens.

The four draft PRs share the shortcut rules and template resolver, apply shortcut templates to an open launcher without
submitting it, and add the Cmd-J list. A refused template leaves that press or sequence unapplied and shows the reason
in the launcher's template error area. Cmd-J filters template names, highlights an exact case-sensitive match, and
starts the selected template through the existing fresh-read quick-launch path. The specs and the Start a session,
Launch templates, and Session list guides describe the behavior. Both covered TODO entries are removed.

## Things you should know

Cmd-J uses Command+J on macOS and Ctrl+Shift+J elsewhere. Browser shortcut documentation lists Ctrl+Shift+J actions in
Chrome, Edge, and Firefox on Windows/Linux and Command+J in Firefox on macOS, so those browsers may keep the chord. The
desktop app is the intended reliable target. Physical key delivery in those browsers was not verified. The Mac desktop
shortcut branches, including Command+X keeping Cut when text is selected in a launcher field, were checked by JavaScript
unit tests and desktop compilation, not by a Mac desktop end-to-end run.

Review caught a partial application of a completed sequence before a later template was refused. The final launcher
checks the whole sequence first, so that refusal leaves the draft as it was. Review also caught Cmd-J falling through
after an armed sequence; it now cancels that sequence and opens the list. The changed docs pages were incidental to this
unattended code plan, so no preview server was started.

## Open questions and possible follow-ups

None needed to complete the agreed behavior. A physical Mac desktop key run and checks in browsers that reserve Cmd-J
would give stronger platform evidence if those clients become a priority.

## PRs

- [#2304: shared shortcut keys](https://github.com/scode/farhelm/pull/2304/changes) — draft, head `8c60c2ee`.
- [#2306: shared template dispatch](https://github.com/scode/farhelm/pull/2306/changes) — draft above #2304, head
  `8e5e9304`.
- [#2310: apply shortcuts in the launcher](https://github.com/scode/farhelm/pull/2310/changes) — draft above #2306, head
  `2adfb946`.
- [#2316: Cmd-J template list](https://github.com/scode/farhelm/pull/2316/changes) — draft above #2310, head `f1c8fe28`.

## Checks run

- On the final stack rebased onto main `5f1ff618`: `cargo build`, Dioxus web release build,
  `cargo check -p farhelm-ui --features desktop`, website build with internal-link validation,
  `cargo fmt --all -- --check`, `dprint check`, changelog fragment format, diff whitespace, and the isolated test-sleep
  checker passed. The checker found no unexplained delay among 304.
- UI JavaScript recorder `02c2edec-a99d-4a63-94ed-d89ab437a141` passed 306/306. UI Rust nextest recorder
  `5a3a1bdf-6e4e-4752-960a-bd27a70f9582` passed 95 selected launcher and template tests, with 442 excluded by the
  selection and no failed selected test. The Rust run used pinned nextest and tmux, four slots, and no retries.
- Browser recorder `68adf373-046f-448c-b3d0-8d0c2ec8328d` passed 16/16 launcher shortcut and Cmd-J cases across Chromium
  and WebKit. Recorder `564dd8b4-0f55-42df-9e3e-9feef6bf342d` passed four typed-search and tooltip integration cases
  across the same engines. Both ran one worker with no retries and cleaned up their owned stacks.

## Checks reused

- PR #2304's earlier browser selection passed 116/116 quick-launch, quick-switcher, and composition cases
  (`27d61e32-f0dc-45e6-8aa9-7efcea74ce67`). PR #2316's earlier existing quick-switcher run passed 18/18
  (`66677566-e2fe-44eb-bbb7-bb49e55cddd7`). Those runs covered the final behavior before the rebase; the post-rebase
  focused runs above cover the interaction with main's new typed launcher search and tooltip changes.
- `cargo clippy --all-targets -- -D warnings` passed before the rebase and remains applicable to the stack's Rust
  changes: the only conflict resolution kept both sides' TODO removals, and the post-rebase desktop compile and 95
  selected unit tests passed on the combined source.

## Checks skipped

- The full Rust and browser batteries were not run. The changed shortcut, launcher, switcher, typed-search, and tooltip
  paths have focused coverage in both browser engines and selected Rust tests. A broader existing-spec browser attempt
  (`35fc17a1-1fb2-4b14-80f4-b58f2e8e5dc0`) timed out after 157 of 208 cases, with one case interrupted and 50 unrun; it
  is incomplete evidence. The later targeted runs cover the interrupted quick-switcher path and the final stack's
  launcher and tooltip interactions, not every unrun case.
- CI was not dispatched. It runs only on demand, and its browser job is disabled. No physical Mac desktop key test was
  run.
- An initial Cmd-J browser fixture omitted an explicit permission choice and stopped at the normal YOLO question
  (`69175f61-6de1-4cdd-9ebf-aac5df09dd2f`, four passes and two failures). The corrected fixture passed narrowly and in
  the later both-engine runs. This was a same-session fixture error; the failed run remains retained separately.

## Review gate

Each PR had the required fresh-context gpt-6.1-sol high review. The reviewers' concrete findings were corrected and
rechecked, including modifier matching and dialog context in #2304, atomic sequence refusal in #2310, and Cmd-J
precedence plus browser boundary coverage in #2316. The structural single-definition, label, and resolver checks passed,
and no review finding remains open. Each commit and PR message passed its separate cold read.

### Landing

Landed on its own in round 98, as #2304, #2306, #2310 and #2316, merged bottom-up, with one fix made during the landing
(below).

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base and the landing round's start, and two documentation
changes (a recorded trailing review and a TODO.md entry) while the stack merged; no other plan landed in this round. The
landing rebase was clean. The base already held the day's launcher and shortcut work, and the landing monitor and a
fresh-context reviewer both read the stack against it. Neither found an interaction:

- A template applied by its shortcut in the session launcher keeps, replaces or removes a wrapper prefix exactly as
  `tl:` does, strips the per-launch trust value the launcher no longer offers, and leaves the typed search and its open
  results alone. The open question from the agent-wrapper landing, about carrying a template's prefix across an agent
  change, is inherited unchanged.
- Applying from the launcher never launches, so it never asks for a name; a pick from the Cmd-J list goes through the
  ordinary quick launch, so naming after quick launch applies there.
- A number key in the launcher reads the current shortcut assignments from the helm, so it applies the template the key
  belongs to now, even after an agent moved it.
- No other Cmd+J or Ctrl+Shift+J exists in the app, its terminal handling or the Mac menu. Cmd-J respects input-method
  composition and held keys, opens only when no dialog is open, and opens over approval cards as the quick switcher's
  Cmd+K does. Cmd+X is still only the start of a launch sequence, never a template key, and the Mac's Edit > Cut keeps
  working when text is selected.

#### Fixes made during the landing

The changelog fragment described Cmd-J only in its second paragraph, which release curation reads as context rather than
as an entry, so the new template list could have been left out of the release notes. Cmd-J now has its own "added"
fragment.

#### Possible follow-ups

- **Pressing an unchanged template again can start a second session after a lost reply.** If a quick launch's reply is
  lost, New opens with the launch kept for a retry. Pressing the same template key again now re-applies the template;
  even though nothing changes, that drops the retry identity, so the next Launch can create a second session if the
  first one had in fact started. `tl:` already behaved this way, and the permission search results do too (the typed
  launcher choices landing recorded that), so there is no general rule yet. Deciding that re-applying an unchanged
  choice keeps the retry identity everywhere would cover all three.
- The Cmd-J list shows ⚡ and number-key badges from the window's cached assignments, so after an agent moves a shortcut
  a row can show a key that now belongs to another template; pressing the key does the right thing, and starting from
  the list looks the template up fresh. The agent quick-launch templates landing recorded the same stale-window problem
  for the Templates dialog.
- The reviewer found, by reading the code and without reproducing it, a rare sequence in which the app refuses a launch
  sequence that the page has already started: from then on, template shortcut presses can be silently dropped until the
  page reloads. Fixing it needs each press's state to carry an identity, not a one-line change.
- SPEC.md's keyboard section still says the number shortcuts launch directly; inside the launcher they now apply, which
  the section's pointer to session creation covers.

#### Checks

- Run now, on the rebased stack (the landed diffs are identical to it): `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `python3 releasing/check-changelog.py format` after the fragment split; `cargo nextest run -p farhelm-ui --lib`
  through the test recorder, four slots and no retries: 537 passed, run `f052858a-a791-4b03-8d87-f618f297b961`;
  `node --test` in the UI's JavaScript test directory, run directly: 306 passed; then fresh `cargo build` and web builds
  and `npx playwright test` through the recorder on `quick-launch.spec.ts`, `quick-switcher.spec.ts`,
  `ime-composition.spec.ts`, `launcher-enter.spec.ts`, `composer-word-search.spec.ts`, `templates.spec.ts`,
  `session-action-shortcuts.spec.ts`, `tooltip-coverage.spec.ts` and `launcher-tabs.spec.ts`, on both engines, one
  worker, no retries: 270 passed, run `65d005d2-a5ec-4f43-9899-68efe0c2ce69`. These run in full the files the executor's
  broadest browser attempt left unfinished.
- Reused: the executor's website build, dprint and test-sleep checks; the landing changed only a changelog fragment
  after them.
- Skipped: a physical Mac desktop key run (Cmd+X against Edit > Cut, an unhandled Cmd+J), and whether browsers deliver
  Cmd+J or Ctrl+Shift+J to the page, as in the report.

Nothing in the executor's report became untrue at landing.
