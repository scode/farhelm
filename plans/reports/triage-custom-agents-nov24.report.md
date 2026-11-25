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
