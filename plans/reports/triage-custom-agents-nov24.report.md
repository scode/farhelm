## What this was about

Custom agents had uneven treatment across the launcher, Settings, Templates and session controls. A deleted registration
could leave an actionable launcher choice, a template could appear ready without naming a registration, and Restart with
could ask for inputs it could no longer use. This stack accounts for all 51 triaged outcomes in 28 PRs. Seven outcomes
have five TODO entries. Three others are recorded without additional code or test changes.

## Things you should know

- A saved setup or session keeps the custom agent's saved ID. Current labels use its registered name when available and
  fall back to that ID. A withdrawn launcher choice cannot submit a different draft on Enter or wake a host through a
  stale quick launch. An older marked Agent template with no registration opens the launcher with a notice so you can
  finish choosing.
- Custom commands always require YOLO handling. An older Command template with explicit No shows that answer so you can
  remove it or replace it. An Agent template editor requires a registration when saving a Custom selection. For an
  already stored custom command, the saved YOLO bit describes the custom declaration. If you switch that draft to a
  built-in, the draft starts without an independent approval answer because the old data cannot distinguish an explicit
  answer from the derived bit.
- Restart with uses the current registration's resumption support and explains why it is unavailable. A stored launch
  without a resume command points you to Clone or Replace with. If the custom executable's home-relative path cannot be
  expanded, Create and retry refuse before allocating a checkout, and Restart refuses before stopping the running agent.
- A successful Delete from the custom agent editor drops that editor's unsaved changes. A deletion made elsewhere keeps
  the draft open for recovery and announces that state to assistive technology. A failed catalog read in Settings shows
  the helm's reason and retains any entries already loaded.
- Settings search finds custom agents by name, ID, command and help text. Non-adjacent terms can match a name, and the
  heading underlines contiguous matches. A launcher search term that is also a built-in shorthand can take the built-in
  route first. Use the existing `recent:` prefix to find the saved setup explicitly.

## Open questions and possible follow-ups

- **Keyboard-highlighted Recent approvals.** Long Custom approval flags can extend below the results viewport. Showing
  the entire list while preserving the keyboard highlight and keeping the compact layout needs a shared scrolling fix.
  Tracked as **Show Custom approval flags in keyboard-highlighted Recent setups**.
  [#4820](https://github.com/scode/farhelm/pull/4820/changes)
- **Unread catalog versus deleted registration.** The launcher can report a registration as unavailable while the
  catalog is still loading or its read has failed. A complete fix needs to preserve known capabilities when a cached
  entry exists and distinguish the unread state at the editor and launch action. Tracked as **Distinguish unread custom
  agent registrations from deletion**. [#4830](https://github.com/scode/farhelm/pull/4830/changes)
- **Shared Settings Delete confirmation.** Moving the editor onto the same question as the rest of the app needs to
  preserve focus on Escape and handle Save on a dirty Close while Delete is open. The prototype exposed both cases.
  Tracked as **Use the shared Delete confirmation for custom agents in Settings**.
  [#4903](https://github.com/scode/farhelm/pull/4903/changes)
- **Restart generation and reporting.** If you turn conversation reporting on and use Restart with, a report sent at
  startup can still be discarded using the old setting. That can leave Restart unavailable for the new run. Turning
  reporting off can still allow an early report, and the restart response can briefly advertise the old Restart choice.
  This timing follows from the code and was not reproduced in a live run. The fix also needs to preserve the old
  session's settings when the replacement never starts. Tracked as **Publish Restart with settings with the replacement
  generation**. [#4946](https://github.com/scode/farhelm/pull/4946/changes)
- **Wrapper defaults per registration.** The old preference belongs to Custom as a group, so it cannot be assigned
  safely to any individual registration. The follow-up covers remembering successful wrapped launches, clearing a
  default on a successful unwrapped launch, using the newly picked agent's default, explicit template overrides, and
  preventing a delayed launch from restoring the default of a deleted registration even if its ID is reused. Tracked as
  **Keep wrapper defaults per custom agent registration**. [#4952](https://github.com/scode/farhelm/pull/4952/changes)

## PRs

- Withdrawn chips cannot launch another draft on Enter. The missing choice shows its saved ID. Model and Effort each
  associate assistive technology with their own status and error.
  [#4810](https://github.com/scode/farhelm/pull/4810/changes)
- Recent setups show the custom agent's name in both wide and narrow launcher views, and search accepts the current name
  or saved ID. [#4817](https://github.com/scode/farhelm/pull/4817/changes)
- Recorded the follow-up for reading every Custom approval flag with the keyboard.
  [#4820](https://github.com/scode/farhelm/pull/4820/changes)
- Choosing Default in search clears the custom effort and updates the visible field. Agent changes retain effort only
  where the new agent offers it, and applying a template reconciles the visible choice with the resulting agent.
  [#4824](https://github.com/scode/farhelm/pull/4824/changes)
- A renamed registration keeps its launcher highlight by ID. A quick launch checks for a deleted registration before
  waking a host. [#4827](https://github.com/scode/farhelm/pull/4827/changes)
- Recorded the catalog availability follow-up after testing the loading and unavailable states.
  [#4830](https://github.com/scode/farhelm/pull/4830/changes)
- The CLI explains valid agent types for mistyped `--agent` values and checks explicit custom registration IDs before
  applying their launch flags. [#4834](https://github.com/scode/farhelm/pull/4834/changes)
- The installer test now proves a custom conversation report exits quietly without starting its callback when an update
  or uninstall holds the installed version. It also proves the callback starts after a successful version claim. Main's
  existing end-to-end test verifies that a parent agent can take over reporting from its still-running child and that
  Farhelm announces the correction. An additional supervisor test for clearing a captured conversation after Restart
  with turns reporting off exceeded triage's small-test budget.
  [#4835](https://github.com/scode/farhelm/pull/4835/changes)
- Added a registered custom agent to the shipped catalog channel's JSON round-trip test.
  [#4838](https://github.com/scode/farhelm/pull/4838/changes)
- A template with an explicit custom agent rejects invalid model text at save time using the same rules as launch.
  [#4846](https://github.com/scode/farhelm/pull/4846/changes)
- Direct Agent quick launch requires a registration. An existing incomplete mark still opens the launcher for manual
  completion. [#4853](https://github.com/scode/farhelm/pull/4853/changes)
- Right-to-left custom names and commands read in the right order in Settings previews and the sidebar tooltip. The test
  driver can mistake the sidebar's name for the page heading because both use the same lookup ID. Screen readers can
  distinguish the two. Triage closed the test-only issue without a standalone test.
  [#4890](https://github.com/scode/farhelm/pull/4890/changes)
- The notice for an externally deleted registration exposes the retained draft's recovery status to screen readers.
  [#4893](https://github.com/scode/farhelm/pull/4893/changes)
- Successful Delete discards the matching dirty editor even when the catalog update arrives before a redraw. The
  lifecycle test covers that timing and the different recovery path for an external deletion.
  [#4899](https://github.com/scode/farhelm/pull/4899/changes)
- Recorded the follow-up for the shared Delete question and its focus and Save behavior.
  [#4903](https://github.com/scode/farhelm/pull/4903/changes)
- Settings Save and Delete explain when the operation is busy, the helm has reset, or the desktop is disconnected.
  [#4906](https://github.com/scode/farhelm/pull/4906/changes)
- Settings marks individual changed fields and keeps long custom agent headings on one line while exposing the full
  accessible name and keeping the scope visible. [#4912](https://github.com/scode/farhelm/pull/4912/changes)
- Settings shows catalog read errors while retaining the open draft and any already loaded entries.
  [#4915](https://github.com/scode/farhelm/pull/4915/changes)
- Settings search can open a specific custom registration or take you directly to the Add form.
  [#4919](https://github.com/scode/farhelm/pull/4919/changes)
- Switching a Command template that says generic Custom to Agent keeps that choice visible and asks you to replace or
  remove it before saving. [#4920](https://github.com/scode/farhelm/pull/4920/changes)
- Templates without an agent can retain valid literal effort. A stored effort the selected agent cannot use stays
  visible and can be cleared with Default. [#4924](https://github.com/scode/farhelm/pull/4924/changes)
- Desktop and CLI classify a custom Command consistently. Every save path rejects an explicit No, and the editor lets
  you clear or replace a retained No. [#4930](https://github.com/scode/farhelm/pull/4930/changes)
- The fixed Custom YOLO pill uses the usual escalation outline. A template that changes the registration retires the old
  escalation and its Undo. [#4932](https://github.com/scode/farhelm/pull/4932/changes)
- The Command main action's accessible name identifies the declared agent. The host YOLO confirmation names the
  registration or saved ID and explains why the custom declaration requires confirmation.
  [#4935](https://github.com/scode/farhelm/pull/4935/changes)
- Restart with explains a missing stored resume command, follows the live registration in the menu and open editor, and
  lets you fold More options when conversation input is no longer required.
  [#4941](https://github.com/scode/farhelm/pull/4941/changes)
- Check custom command home paths before allocating a checkout or stopping an agent. Recorded the separate follow-up for
  admitting startup reports under the replacement's settings.
  [#4946](https://github.com/scode/farhelm/pull/4946/changes)
- The session menu and quick switcher name the registered agent or its saved ID. Long launcher chips fit with an
  ellipsis and retain their full accessible name. [#4950](https://github.com/scode/farhelm/pull/4950/changes)
- Recorded the three coupled wrapper-default outcomes together so preference storage, selection and deletion can be
  tested as one change. [#4952](https://github.com/scode/farhelm/pull/4952/changes)

## Checks run, reused and skipped

- On the final revision `dcab1487` based on `9a16fd9d`, 39 helm tests (`7e7f24d6`) and 109 desktop tests (`8adfdd18`)
  passed. The helm selection focused on destination trust, template application and launch approval. The desktop
  selection covered approval-card trust wording, Restart with, YOLO and template shortcuts. Strict Clippy on all targets
  for `farhelm-helm` and `farhelm-gui`, format, heading references and whitespace passed. All four renderer jobs passed
  `python3 scripts/check-gui-goldens.py` against the published screenshots (`6a612874`). No further reference changes
  were needed.
- On rebased revision `51d8daae`, 667 focused desktop tests passed (run `925cdbaa`), along with 203 selected helm,
  supervisor and protocol tests (`f829a96c`), and five selected CLI and live terminal cases (`2a6cf2ce`). Strict Clippy
  passed on all targets in the five affected packages. Formatting, formatting of the edited specs and TODO, the test
  delay checker, instruction references, changelog format and range whitespace passed. Main's managed-checkout history,
  preview and drawn consent binding were included in the combined desktop selections.
- The test selections ran through `scripts/record-test-run.py` with pinned nextest, four slots and zero retries. The
  commands were `cargo nextest run -p farhelm-gui --lib -E …` for the launcher, template, custom Settings and
  session-control modules, `cargo nextest run -p farhelm-supervisor -p farhelm-helm -p farhelm-proto --lib -E …` for
  custom agents and launch, and `cargo nextest run -p farhelm --bin farhelm --test e2e -E …` for custom-agent CLI and
  live terminal scenarios. Strict Clippy used `--all-targets -- -D warnings` on those five packages.
- The initial rebased full screenshot pass (`16648712`) reached the staging limit in both appearances of the incomplete
  custom quick-launch scene. The fixture expected the old wording after main moved the notice below the actions. The
  corrected scene passed in isolation (`3c5934cd`) on `5f9874b5`. The bounded full run (`2a13c620`) passed and showed
  changes to those two notice images only. Both runs used `python3 scripts/check-gui-goldens.py --update`. The isolated
  run added `--jobs 1 --scene launcher-incomplete-custom-quick-launch`.
- An earlier full screenshot run (`3851cc6b`) hung in the custom-agent Settings sidebar tooltip scene. Its renderer was
  terminated. The scene now scrolls once and uses the harness's bounded staging for hover. The isolated scene and full
  rendering passed after that correction on `b5a4f003` (runs `a64a67f2` and `f71f3187`). The isolated update selected
  `settings-custom-agent-nav-tooltip`. The interrupted run is retained as failure evidence. The images use English
  fixture text because the bundled fonts do not cover Hebrew. A separate Settings driver test at `6b95153d` (`adfdc0cc`)
  used Hebrew to verify the direction boundaries in previews and hover help.
- Reused the 875 focused tests and Clippy after the screenshot-only correction and manifest refresh. Application code
  was identical across those revisions. The original PR evidence also includes
  `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` at `cb9cef98` (`242fd8fd`), the single helm catalog
  round-trip at `98e6ae17` (`97d133aa`), and four supervisor tests plus two live terminal tests at `0a88cbc2`
  (`e39c9cb7`, `90c7c575`). The forwarder fixture and catalog case retained their test logic through rebase, and the
  combined run covers the supervisor changes.
- Skipped the full workspace runtime suite because the landing gate runs it. The accessibility driver checked names,
  roles and field associations. Actual screen-reader announcements were not exercised.

## Review gates

Each draft passed its source gate, with findings resolved or recorded under the plan's scope and size rules. The first
rebase preserved main's consent binding and managed-checkout history while carrying forward the custom-agent launch
checks. The final rebase brought in automatic workspace trust on disposable hosts and Claude's recipe onboarding. Its
only conflict was CLI spec wrapping. The merged text retains the destination-trust policy and the custom Command YOLO
rule. The custom agent behavior patches and visual references were unchanged across that last rebase.

Changed screenshots were reviewed in light and dark by Opus at high effort and inspected by the executor. The rebase
changed only the incomplete quick-launch notice. In an earlier round, both `launcher-custom-escalation` pictures drew a
reviewer finding because the existing banner says “Custom” even when the selected chip says “Acme Code.” I kept the
banner wording outside the escalation change. The reviewed change covers the pill outline and when the banner retires.
The session menu screenshot also shows synthetic age stamps from its fixture, which are separate from the labels this
plan fixes.

### Landing

#### What landed with it

Three plans landed this round, in this order: this one, through its 28 PRs and the landing fix
[#4969](https://github.com/scode/farhelm/pull/4969/changes); then the agent catalog and drift test cleanups (#4945,
#4951, #4954, #4956, #4959); then private Tailscale SSH socket folders (#4957, #4962). Since this plan's PRs were
written, main gained the environment triage, the sandbox workspace trust plan and other launcher work, all landed in the
previous rounds.

Rebasing onto them conflicted in two launcher files, which I merged by hand, keeping both sides:

- **The launcher.** This plan and the environment triage each added a guard that refuses a Launch press that would act
  on a launcher state not yet shown on screen: this plan's covers a custom agent chip whose registration was deleted,
  main's covers a default folder that changed. Both are kept; Launch checks the folder guard first, then the chip guard.
- **The launcher's screenshot scenes.** The environment triage changed the shared setup for scenes of a suspended
  sandbox; this plan adds a scene of a deleted custom agent's quick launch and an empty custom agents list to the same
  setup. The deleted custom agent quick launch scene now uses main's setup with the same sidebar sessions as before, and
  every suspended sandbox scene, main's newer ones included, now loads the empty custom agents list. The full screenshot
  comparison passed on the result, so their pictures are unchanged.

The triage ledgers and the review queue's index conflicted as usual; I kept both sides, and the review confirmed that
all 51 lines removed from the review queue's index match the 51 finding files the plan deleted, and no line main added
was lost. A file-by-file comparison confirmed every other file's change is exactly what the executor pushed. Before
rebasing I removed stale copies of the round's commits that an earlier fetch, made while the executors were pushing, had
left in the landing checkout; they were older revisions of the same commits and never part of what landed.

The landing's review read the three plans against main and each other and found no broken interaction: a refused custom
Restart with is caught before the host wakes, main's "Restart error." wording covers the custom refusals too, the custom
agent check runs before the supervisor stops the agent, and a custom command always counts as YOLO, so hosts that ask
before YOLO launches still ask. It counted no blockers, no should-fix items and four small points across the three
plans; this plan's is in the last section.

#### Fixes made while landing

- **24 new screenshots were missing from the screenshot scene list.** The plan added golden pictures of its custom agent
  launcher, Settings, template and quick switcher states, but not their entries in the scene list of the screenshot
  batch tool (the side-by-side page of the app's screens next to the old app's), so that tool's test failed. #4969 adds
  them.
- **Formatting.** The plan's new triage archive entries had unwrapped lines, which the Markdown formatter's check
  rejects; my merge left one extra blank line in the triage ledger and one call in the launcher's golden scene setup
  longer than rustfmt allows. #4969 reformats all three and changes no words or code.

#### Checks

This round landed three plans in queue order: the custom agents triage, the agent catalog and drift test cleanups, and
private Tailscale SSH socket folders. All the checks below ran once on the three plans' PRs stacked in that order on
main as it stood at the start of the landing (`f6ec0f51`), with the conflict resolutions described in the Landing
section of the custom agents triage report; the stack's top commit was `30ae8ef5`. Worker runs were on a separate arm64
Linux test worker; the rest ran locally on x86_64 Linux. The only later change is the landing fix
[#4969](https://github.com/scode/farhelm/pull/4969/changes), inserted after the custom agents plan's last PR, which adds
24 screenshot scene list entries and reformats two triage ledgers and one call in the launcher's golden scene setup;
after it I ran the screenshot batch tool's test (recorder run `18834801-a217-426a-aea5-61f735104753`),
`cargo fmt --all -- --check`, `dprint check` and the changelog check again, and all passed. That final tree is what main
now holds outside the plans queue, apart from what main gained while the custom agents plan was merging: two TODO.md
entries, a SPEC_impl.md note and new review queue findings, none of which touches code the checks covered.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `40997676-1b25-42dd-a283-44987367a22e`): all 8701 tests passed, with no runtime skip. The plans changed the launcher,
  Settings, templates, the quick switcher and Restart in the GUI, the helm's custom agent registry, templates and
  launches, the CLI, the supervisor's Goose restart, the drift tests, and the helm's Tailscale SSH connections; the
  whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `c1000071-3994-42e2-ad73-d85b8866e537`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `9e8e8c24-edac-4e59-9429-c8ab9f3211cb`): 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `4fc1cb5d-72d7-4edd-870c-909f63840e93`): passed, including the custom agents plan's 24 new scenes and main's sandbox
  scenes whose setup the landing merged with the custom agents plan's.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed on the stack
  (recorder run `8e6013bf-03ee-48d3-9544-f2e4cb90d9eb`) because the 24 new scenes were not in its scene list, and passed
  with the landing fix.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Start a session, and Stop, restart and resume), both in the agent catalog plan.
- `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; one warning, about an
  old citation in the triage archive, already on main) and `python -B scripts/check-test-sleeps.py`, locally: passed.
  `cargo fmt --all -- --check` and `dprint check`, locally: failed on the stack as described in the custom agents triage
  report's Landing section, and passed with the landing fix.
- `bash -n scripts/test-install-sh.sh` and `shellcheck scripts/install.sh scripts/test-install-sh.sh`, locally: passed.
  The custom agents plan added cases to the installer test.

Reused: the custom agents plan's run of the installer test,
`bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (recorder run `242fd8fd`, at the executor's revision
`cb9cef98`), which covers the cases the plan added for a custom conversation report while an update holds the installed
version. Nothing it exercises changed since: main changed neither the installer nor the CLI since the custom agents
plan's base; that plan's later CLI change is in template flags, and the agent catalog plan's only CLI change is how
`--effort` reads its value, neither of which the installer test's conversation report path uses. Also reused: the agent
catalog plan's live Goose named Create and Restart drift run at its executor's revision (recorder run `5e4a1a9a`), which
spends vendor turns and was not repeated; nothing that landed since that plan's base touches Goose's support code. The
executors' other runs were focused selections that the runs above cover again.

Skipped: real agent and drift runs, which spend vendor turns and run only when asked; a real Tailscale network, which
the tests replace with stand-ins; the workspace doctests, since no documentation example changed; a Mac run, so the
macOS limit on socket path length (which the Tailscale plan's socket folder names are sized for) and the macOS installer
are covered only by tests that run on Linux; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing. The report's formatting claims covered the Rust code and the edited specs and TODO, not the triage archive
  entries that failed `dprint`; the long Rust line came from the landing's own merge.

#### Things you should know that the report does not say

- **Writing a template from the CLI with a custom command still refuses to save until you pass `--yolo` or
  `--no-yolo`.** `--no-yolo` is then always refused for a custom agent, since a custom command always counts as YOLO, so
  the hint offers an option that cannot work; the desktop app saves the same template without asking. Say if you want a
  TODO entry for dropping the question in the CLI.
