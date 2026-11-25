# Session launcher and templates triage

## What this was about

The November 24 review found 41 issues in the session launcher, Templates, and the related approval cards. The most
consequential ones could make Enter or an older button act on a setup the user had not seen, or let a pending settings
save use a newer warning. There were also draft-loss risks, misleading labels, missing screen reader state, and tests
that could pass with the wrong setup.

The stack fixes 31 outcomes, leaves seven unchanged under the triage size limits, defers two fixes to Plan follow-ups,
and closes one that main already covered. The PR list maps all 41 outcomes to their dispositions. Each archived outcome
retains its original decision and the link to its PR.

## Things you should know

- **Replace and Restart consent.** Resize and a shrinking folder list can change what Enter would act on. The launcher
  now checks the action it showed before acting. The main Replace with and Restart with buttons carry the warning from
  the frame that drew them. Set up and launch and the permanent YOLO answer keep the relevant consent through the
  setting write. A plain **Replace as guarded instead** keeps the shown directory and Delete guard while allowing a
  concurrent title change. If the source row disappears, it leaves the existing retry explanation.
- **Template drafts.** Escape closes search before Save as template, a held Escape dismisses only one layer, and a
  template pick cannot change the draft behind its save panel. Templates shows one leave question for the current
  editor. Delete carries the shown template name and whether the button promised Undo. Trust data cleaned up
  automatically on load does not by itself cause a save question.
- **Visible and spoken cues.** Recent setup rows name the click and Return actions separately when they differ. The
  selected YOLO cue stays red. The applied source remains marked through edits, with permission Undo restoring it. The
  host picker reports whether its list is open and the setup picker reports the actual keyboard highlight. Very long
  unaliased SSH names can still exceed the shared tooltip bounds. In a narrow Templates list the unsupported permission
  label takes precedence and the agent name may truncate.
- **Two fixes went past the size gate.** One prototype needed post-layout visibility for an entire recent row, including
  ancestor clipping. The other had to distinguish a waiting folder from a valid launch that wakes a host and settles its
  placement. Both have specific reproduction and acceptance steps in TODO.md.
- **Overlap with other plans.** The session list and shared dialog focus code changed only where required by guarded
  Replace and the duplicate template leave question. The panel under approval requests and the retired CLI flag changed
  only to explain the existing YOLO rules. The Clone test changes the supervisor's request fixture to cover a caller
  omitting the host. The hook fix carries the consent into the launch continuation. The tooling plan's golden runner was
  used as it landed, and this stack adds launcher and approval references. The spec-text plan already generalized the
  harmless-update rule to the whole app in [#4799](https://github.com/scode/farhelm/pull/4799/changes). The rebase
  preserved that rule, main's new Command-message Tab order and missing-mode prefill.

## Open questions and possible follow-ups

- **Ignore Enter on a clipped recent setup.** A recent setup can still become partially or fully obscured after it has
  been shown. Enter should be consumed while its row is clipped and become available again when a frame shows the whole
  row. The new guard in this stack covers a recent that was never shown after keyboard selection.
- **Keep hook setup from launching into a folder that arrived later.** Set up and launch can still use a remembered
  folder that appeared while the all-hosts setting was saving. The separate follow-up will require a fresh press when
  the presented folder was still loading or the destination changed. This stack freezes the Replace warning through that
  same save.

## Pull requests

- [#4812](https://github.com/scode/farhelm/pull/4812/changes) **Fixed seven.** Replace with after resize, hook save
  reading a newer Replace warning, Enter on a shrinking folder list, Escape discarding template edits, Return before a
  highlighted recent has drawn, the host-reorder test premise, and the launcher Tab test's placement premise.
- [#4814](https://github.com/scode/farhelm/pull/4814/changes) **Fixed two.** Hover help shows the held host name. Bash
  and Zsh have their own help and accessible description.
- [#4819](https://github.com/scode/farhelm/pull/4819/changes) **Fixed four.** Permission Undo restores the template
  source mark, the save panel blocks other template picks, a held Escape closes one layer, and the footer test checks
  the active background. **Unchanged three.** A reason on a disabled setup needs a toolkit change, copying the held
  folder needs a new control, and optimizing template-row redraws needs measurement and shared row changes.
- [#4823](https://github.com/scode/farhelm/pull/4823/changes) **Fixed one.** A permanent YOLO answer and the
  continuation after its write apply to the launch and warning the question showed.
- [#4829](https://github.com/scode/farhelm/pull/4829/changes) **Fixed three.** The selected YOLO cue stays red, a click
  that fills the form is labeled accordingly, and Return on a typed recent names what it does. **Deferred one.** Enter
  on a clipped recent. **Unchanged two.** A tooltip can describe the previous setup if history refreshes under a resting
  pointer. Clicking a setup can fill the launcher while keyboard focus stays on the approval card beside it. Both fixes
  exceeded the polish limit.
- [#4832](https://github.com/scode/farhelm/pull/4832/changes) **Fixed three.** A stale Delete click carries the shown
  target and Undo promise, incomplete-template guidance sits below the actions and points up to the form, and
  unsupported permissions are labeled in the template list and approval card. **Unchanged one.** A source mark on quick
  launch needs changes to both launch-preparation outcomes and their callers.
- [#4836](https://github.com/scode/farhelm/pull/4836/changes) **Deferred one.** Hook save launching into a folder that
  arrived later. **Closed as already covered one.** Launch without writes no setting. If you enable hook setup in
  Settings while the launch is still waiting, it can use the newer setting. Main's app-wide rule already allows this
  background update and preserves the protections for destructive actions and launches that retire a session.
  [#4799](https://github.com/scode/farhelm/pull/4799/changes)
- [#4839](https://github.com/scode/farhelm/pull/4839/changes) **Fixed two.** Templates shows one leave question tied to
  its editor, and automatic cleanup of old trust data no longer prompts to save by itself.
- [#4841](https://github.com/scode/farhelm/pull/4841/changes) **Fixed two.** The host picker exposes its open state and
  the setup list announces one keyboard selection even when a different setup filled the draft.
- [#4843](https://github.com/scode/farhelm/pull/4843/changes) **Fixed two.** Replace with and Restart with use the
  clicked button's shown warning, and the plain guarded Replace alternative carries its original Delete guard and uses
  the allowed current title.
- [#4847](https://github.com/scode/farhelm/pull/4847/changes) **Fixed two.** The retired YOLO flag diagnostic and the
  Restart and dormant Prompt approval copy describe when a fresh approval applies.
- [#4850](https://github.com/scode/farhelm/pull/4850/changes) **Fixed three.** Match underlining walks the search
  matches once, only plain Return picks or submits from the launcher field, and the Clone test proves the supervisor
  accepts an omitted host. **Unchanged one.** Underlining an escape sequence in the displayed text can still look odd.
  Mapping raw text and display escapes exceeded the polish limit.

## Checks

- On Linux at rebased tip `c6bd4e41`, recorded `cargo nextest run -p farhelm-gui --lib` for the launcher, Templates,
  approvals, lifecycle, layers and focus. All 1,148 selected tests passed (run `906ceabe-1920-47e6-b8a7-337c84a7dafb`).
  The full `scripts/check-gui-goldens.py` comparison matched the references in both appearances (run
  `1cd3a4d2-d36f-48b9-a26c-87e529703a5c`).

- Reused the supervisor Clone test from `55cbcc24` (run `7fe4db6f-dd9d-493f-b4c6-00c89dc38432`) and the retired CLI flag
  test from `64f5110a` (run `03a24ece-4d2c-4ed0-9c74-7bac0f6a79df`). The rebase preserved the tested command paths and
  their dependencies.
- Reused the host-text underline checks from `55cbcc24` (run `a8fcdf4b-967b-4b74-8e2e-fa74dc910ea6`). The underline
  calculation and text shaping inputs stayed the same. The current full screenshot comparison also exercised the display
  path.
- Reused the individual drafts' accepted source and visual reviews. The rebase changed ledger placement, two TODO
  placements, and overlapping spec prose. The code, fixtures, golden references and changelog patches kept their
  reviewed changes. Main added dialog focus bookkeeping, light-theme hover colors, and session notices, so final checks
  exercise the shared focus stack and compare the full visual catalog.
- Whitespace, Markdown formatting and instruction links passed after rebase, with one existing archive citation warning.
  The Rust delay checker found zero unannotated delays across 310 calls.
- Skipped full workspace CI, platform-specific desktop smoke, provider and live-agent suites. The production changes are
  in the launcher and related UI or wording, and the focused runtime tests and full gallery exercise those changes. The
  documentation-only follow-up used format and instruction-link checks.

## Review gate

All 12 units passed independent Sol high source review and a cold read of their PR wording. The five visible-change
units passed independent Opus high review in light and dark after the tooltip, template label and approval fixture
corrections. The reviewed pictures are in the reference manifest. The narrower review units separate delayed consent,
visual help, the template editor, picker accessibility, guarded Replace, and permission wording so the related behavior
stays reviewable one commit at a time.

### Landing

#### What landed with it

Only this plan landed this round, through its twelve PRs and the landing fix
[#4856](https://github.com/scode/farhelm/pull/4856/changes). Between the point main was at when the executor finished
(`6c3b5091`) and the landing, main gained only plans queue commits, so nothing else could interact with this plan.

The landing's review read the whole combined diff against SPEC.md and SPEC_impl.md. It found the code sound in these
respects: the Replace with and Restart with buttons, and the hook setup and permanent YOLO answers, act on the warning
and launch the user was shown; the Templates editor's Delete and its leave question stay tied to the template they were
shown for; the changes to shared dialog, button and list-row code keep what other parts of the app rely on; and each new
test fails on the code from before this plan. The one exception to those respects is the first item under "Things you
should know"; the smaller items there are wording and feedback gaps.

#### Fixes made while landing

- **A cut-off sentence in SPEC.md.** One of this plan's own PRs, #4814, added "Hover help shows the held host's name."
  to the launcher's description in SPEC.md. Rewrapping that paragraph lost the second half of its last sentence, so it
  ended at "Project registration (associating" and dropped the rule that registering a project is optional and never a
  prerequisite. #4856 restores it. No code depended on it. Main carried the cut-off text only for the minutes between
  #4814's merge and #4856's.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`a64b1ffe`); the rebased stack's top commit
was `c51a68f9`. The only later change is the landing fix [#4856](https://github.com/scode/farhelm/pull/4856/changes),
which restores half a sentence of SPEC.md and changes nothing else, so the only check I ran again was `dprint check` on
SPEC.md, which passed. That final tree is what main now holds outside the plans queue.

- `cargo nextest run --locked -p farhelm-gui -p farhelm-desktop -p farhelm-supervisor -p farhelm --lib --bins` with a
  filter of `package(farhelm-gui) | package(farhelm-desktop)` plus the two tests
  `validate_agent_verb_bounds_the_creating_verbs` and `the_yolo_override_is_refused_under_both_names`, on the test
  worker (recorder run `274a7a97-44d8-435d-bb69-3c0e30e3b34f`): all 3574 selected tests passed, with no runtime skip.
  That is every unit test of the desktop app's interface and of the desktop app itself, plus the only two tests the plan
  changed elsewhere: a supervisor test the plan widened to cover an agent's Clone request that leaves out the host
  (supervisor code unchanged), and the test of the `farhelm` command's refusal of the retired `--confirm-yolo` flag,
  whose message the plan reworded.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and the desktop app's
  two clippy runs (`-p farhelm-desktop --all-targets`, with and without `--features agent`), on the test worker: passed.
- The desktop app's end-to-end smoke (`scripts/gui-e2e-smoke.py`, recorder run `6fb38093-0b60-4e1c-8056-5f2b84d53eb5`),
  on the test worker: 50 steps passed, one skipped, the paste step that headless Linux cannot run. It drives the
  launcher, Replace with its YOLO question, and the dialogs whose Escape and focus handling the plan changed.
- `python3 scripts/check-gui-goldens.py`, the full comparison of the app's screenshot tests against their published
  reference pictures, locally (recorder run `d6e05ada-d584-4ff2-9df3-4a1fd05389e9`): every scene matched, the plan's new
  and changed launcher, Templates and approval card scenes included.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main), `dprint check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed.

Reused: none. The report's own runs covered the same code before the rebase, and everything they covered ran again
above.

Skipped: `scripts/gui-lifecycle.py`, the end-to-end run that starts, quits and relaunches the desktop app, since nothing
in its startup, quit or window handling changed; the rest of the workspace Rust suite, since outside the desktop app the
plan changed only the two tests above, the `--confirm-yolo` refusal message (whose test ran) and the specs; a Mac run,
since no change is macOS-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The report's checks ran on the plan's PRs before the landing, and the ones above ran on the same code after a
rebase that brought in only plans queue commits; the spec fix after them changed only SPEC.md text. The report's
statement that the launcher now checks the action it showed before acting never covered the New and Clone case in the
first item below.

#### Things you should know that the report does not say

- **In New and Clone, a very fast Enter after a template changes the folder can now launch; I recommend a follow-up.**
  Say the Folder list is open with a row highlighted by the arrow keys, and a template is then applied that changes the
  folder: by its number key, or a template press that was queued while the template list was still loading and applies
  once it loads. The plan now clears the highlight, so it cannot point at a folder that no longer matches. If Enter
  arrives before the window has drawn that change, there is no highlighted row, and Enter launches the session with the
  template's choices. Before this plan the highlight kept its position in the new folder list, so the same Enter
  selected whichever folder now sat there and launched nothing. So in that narrow timing this is a regression from a
  harmless wrong selection to a launch the user has not seen. It needs the Enter to land within one screen refresh of
  the template's change. Your triage decision for keys that act on something other than what was shown was "fix the
  harmful ones, accept the harmless ones", and it counts a launch as harmful, so this goes against that decision, if
  only in a timing few users will hit. The landing review found it after the checks had passed. I landed the plan rather
  than hold the 31 review findings it fixed for it, because the fix needs new code and tests and its own checks. The
  follow-up I recommend: have the folder list ignore Enter until the cleared highlight has been drawn, in every mode,
  the way it already does in Replace with.
- **A stale answer to the launcher's YOLO question that turns on "Start YOLO sessions without asking" now closes
  silently.** If the launch behind the question changed between drawing it and the click (a host reconnect, say), that
  answer now neither saves the host setting nor starts the launch, as the plan intended, but the question closes without
  saying why. The user can press Launch again and be asked afresh. I see no need for a follow-up unless you want the
  question to say why it closed.
- **This plan's new Templates list label for an unavailable permission mode reads differently from the rest.** A
  template whose permission mode its agent does not offer now leads its summary with the bare mode name, "Approve (not
  offered)", while every other template shows "Approvals" before the mode. The approval card is not affected, since its
  row is already labeled. A small wording follow-up would put "Approvals" before it too.
- **One changelog draft promises a note the user may not see.** The draft for the Replace warning fix covers the answer
  that replaces a YOLO session with a guarded (asks-before-acting) one. It says Farhelm leaves a note on the session
  being replaced when that session has left the session list by the time the user answers. Since the list no longer
  shows that session, the user usually will not see the note. Release curation may want to soften the sentence.
- **Part of one review finding is no longer recorded.** The finding about holding Escape down (so it repeats) discarding
  the launcher's draft named two places this happens: the launcher with an empty command field, and the question the
  launcher asks before setting up an agent's hooks. The plan fixed the first and archived the finding as fixed, so a
  held Escape on the hook setup question is now recorded nowhere. Ask for a TODO entry if you want it tracked.
