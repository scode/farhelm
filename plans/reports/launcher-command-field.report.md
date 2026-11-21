### What this was about

The session launcher was crowded and its search drop-down covered the form. The agreed layout gives the command field
the top of the dialog, keeps results beside the form on a wide window, and leaves recent setups and templates in that
same column while the field is empty.

### Things you should know

Opening New, Clone or Replace with starts focus in the command field. Enter on the empty field launches the setup in the
form. Down moves into the choices. Clicking a recent setup fills in the form, and Enter on an explicitly highlighted
recent setup can launch it. Choosing a template fills in its choices, and the source label stays visible through further
edits. In narrow windows, type `recent:` or `tl:` to find those choices under the command field.

The Destination card puts Host, Folder and Browse on one address line. The Host menu also offers new environments when
available. The form groups agent settings and folds conversation and wrapper fields under More options. Restart with
changes settings for an existing session while keeping its agent type, host and folder visible and fixed. It starts
focus on Cancel. Replace with starts a new session and deletes the source only after the new session starts
successfully. It keeps that warning beside the action. The primary actions stay pinned beside confirmations and notices.

### Open questions and possible follow-ups

The published Launch templates and Start a session guides still show the old launcher. At the next docs refresh, update
the Start a session images for the harness and model, effort and permissions, destination, search and naming, recent
setups and launch actions including YOLO confirmation. Also update the Launch templates images for applying and saving a
template. Both guides' revised capture steps passed locally.

### PRs

- [#4433](https://github.com/scode/farhelm/pull/4433/changes) reorganizes the launcher form and reuses the session
  list's harness marks.
- [#4436](https://github.com/scode/farhelm/pull/4436/changes) groups the destination address and places new environments
  in the Host menu.
- [#4438](https://github.com/scode/farhelm/pull/4438/changes) folds conversation and wrapper options where the launcher
  can show that they carry values.
- [#4442](https://github.com/scode/farhelm/pull/4442/changes) gives the command field the header and keeps the launch
  actions pinned.
- [#4460](https://github.com/scode/farhelm/pull/4460/changes) keeps recent setups, templates and search results beside
  the form.

### Checks

The targeted `cargo nextest run -p farhelm-gui --lib` selection on `74e779db` used
`test(launcher::) | test(templates::) | test(settings::search::tests::) | test(host_text::) | test(components::tests::dialog::) | test(components::tests::button::)`.
It passed 895 of 896 tests. The failing test showed that an unavailable Container host’s reason was clipped (run
`14f9975d-f4d3-4e76-b0f0-92066e8e6f55`). Revision `87f9fb5a` passed the two focused checks for the entire
unavailable-host explanation and the disabled setup's full details and refusal in one tooltip (run
`f9d22eee-4dce-41a9-8368-a73060641cca`). All 111 tests selected by
`test(launcher::environment) | test(launcher::recents_view_tests::) | test(launcher::search_view_tests::)` then passed
(run `8ebd71d4-9329-42ef-ab50-e88676977397`). Reuse the passing launcher, Templates, Settings search, host-text and
shared control checks from `74e779db`. The final repair changed Container host spacing and disabled-row help, which the
later selection covers.

The full Linux `check-gui-goldens.py --update` render produced 348 test images. It changed or added 18 launcher goldens,
all inspected and published to the screenshot test reference store (run `9eca3241-e780-402b-bd9f-9f16cfec1100`). At
rebased revision `86522ed3`, `check-gui-goldens.py` selected the launcher and Templates scenes and matched all 24
references (run `5643b163-c421-4b4f-a985-469074062166`), and
`docs_shots.py --only start-a-session --only launch-templates` captured all 18 local guide images in one staged desktop
run (run `24cad8f0-909c-49d0-908d-c2374db6722f`). `test-gui-screenshot-batch.py` passed 47 tests (run
`e4c8f2e0-4cea-4557-a2a9-f3744f85f4f0`). `test-docs-shots.py` passed 28 (run `f4ae6d53-ab57-47a0-849f-8caf89639193`).
`bun run build` built all 39 pages with valid internal links. The final rebase after those checks changed queue
bookkeeping only.

The broader workspace suite was skipped because the launcher and shared controls are covered by the focused suite.
Native macOS font and window behavior remain untested in this Linux run.

### Review gate

The Sol code review confirmed fixes that keep your arrow-selected template through a host reconnect and prevent repeated
Enter on a disabled setup from launching the form. High-effort Opus accepted the launcher in both appearances after the
visual fixes. The narrow drop-down keeps plain rows, typed groups keep their plural labels, and the taller container
form scrolls above its pinned actions. The review accepted these details.

### Landing

#### What landed with it

The plan's five PRs merged with a landing fix, #4461. golden-changes-page landed right after it in the same round. It
builds a page of changed golden screenshots and shares only one file with this plan, where each changed a different
function. Nothing else had reached main since the executor based the plan. An independent reviewer and I read the five
PRs against the plan's design. Neither of us found a blocker: the layout matches the agreed design, and the keyboard
rules hold. Enter on the empty field launches until Down is pressed, Up from the first row hands Enter back, and a held
or disabled row's Enter does nothing. The check of a conversation ID typed into the launcher moved into code the helm,
the supervisor and the app share, with the same rules as before. The app now also runs it before sending, so an invalid
conversation ID opens More options, where that field sits, instead of going to the helm. You asked for this plan to be
fixed directly and landed unless something was fundamentally wrong. Nothing was.

#### Fixes made while landing

All in [#4461](https://github.com/scode/farhelm/pull/4461/changes):

- In Replace with, Enter on a highlighted recent setup replaces the session the launcher was opened from, which deletes
  it. The old launcher's recent rows said "⏎ Replace" there; the redesign showed only a bare Return key cap, so nothing
  on screen said Enter was destructive. The cap reads "⏎ Replace" again in that mode, a test checks both forms, and the
  spec says so.
- The docs pages still described controls the redesign removed. The Start a session, Stop, restart and resume, Launch
  templates, Tensorlake sandboxes, Container sessions and Shells pages now speak of the Destination card, the Host menu
  and the command field instead of the Where row, Runs on, a Shells heading and a message above search. The spec's
  container section still said the footer names a new container; the footer summary is gone, so it now says the label a
  screen reader reads for the Launch button does.
- The app's end-to-end smoke test, which the executor did not run, waited for the launcher's old header text, which the
  redesign removed. Its New and Clone steps timed out. They now wait for the command field's placeholder, which names
  the mode. One of the scripted review screenshots the screenshot batch takes (not a golden) rested the pointer on that
  text and now rests it on the Destination heading.
- The plan's five changelog notes describe one unreleased change, so they are merged into one, with a note for release
  curation that the footer's summary line is gone and Runs on is now part of the Host menu.

#### Checks

This round landed launcher-command-field (the launcher, shared controls, a conversation-ID check moved into code the
helm, supervisor and app share, goldens, docs pages and their screenshot steps) and golden-changes-page (the golden
runner and its page). The landing ran the Rust workspace suite, the desktop app's tests, the lint runs, the end-to-end
smoke test and the two affected docs pages' screenshots on a separate arm64 Linux test machine, and the goldens, script
tests and small checks locally on x86_64.

The test machine's disk filled during the first workspace run. Its build folder for this checkout had grown to about 117
GB over earlier rounds, and the optimized development builds the earlier goldens-fast-and-targeted plan introduced
pushed it over. The test machine's script cleans that folder only when a run starts with less than 15 GB free, which was
not enough here; raising that threshold is worth a small follow-up. Four tests failed on the full disk, two of them
saying so outright, and every later run on that machine died with it. Those runs are not evidence of anything (the
workspace run is kept as `cdc3ecf6-840c-42a6-bf87-244a67a4d899`). I removed only that build folder and ran the set again
from a cold build.

On both plans with their landing fixes (#4461 for the launcher, #4462 for the golden page), after the disk was freed:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine (recorder run
  `2ceaf252-220e-430b-812c-b569e2b2df91`): all 8217 passed; the 86 skipped are ignored by default.
- `cargo nextest run -p farhelm-desktop`, on the test machine (recorder run `92bb77f0-7864-41e5-8dba-5846ddb6e477`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's own
  two clippy runs, on the test machine, not through the recorder: passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine. Before the smoke's own fix in #4461, it failed two steps
  that waited for the launcher's removed header text (recorder run `b7d510cb-a992-4815-9bad-35700a12ec90`). With that
  fix (recorder run `f32d7fbe-7ded-4453-8d1d-ac6b112abfe0`), every step passed but paste, which skips on Linux as it
  always does.
- `scripts/docs-screenshots.sh --local --only start-a-session --only launch-templates`, on the test machine (recorder
  run `b5a4cbb5-678a-426a-a160-b4f9ae4f6ec5`): passed, all 18 pictures captured.
- `cargo nextest run -p farhelm-gui -E 'test(/^launcher::/) | test(/^templates::/)'`, locally (recorder run
  `73bd1438-2bb0-43ad-878c-582d0b6f5a99`): all 835 passed, the new key-cap assertions among them.
- `python3 scripts/check-gui-goldens.py --scene 'launcher-*' --scene 'template*' --scene dialog-launcher-frame`, locally
  (recorder run `2dd61402-ac9d-492e-a4df-5c7ad8bb945f`): passed.
- The website build (`bun install --frozen-lockfile && bun run build`), with its link check, locally: passed.
- `python3 -B scripts/test-gui-e2e-smoke.py`, `python3 -B scripts/test-gui-screenshot-batch.py`,
  `cargo fmt --all -- --check`, `dprint check`, `python3 scripts/check-agents-md.py` and
  `python3 releasing/check-changelog.py format`, locally: passed.

These ran on both plans as delivered, stacked, before the landing fixes. They still count: the fixes change what is
drawn only in Replace with's recent rows, which no golden picture shows, and the launcher goldens above passed after
them.

- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `83dbd97b-0984-4847-bd8c-3236760f48fe`): passed, with the launcher's new pictures and no changes page written.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `83208fa7-3a0e-4caa-b2b1-de26a7508ca1`, 21 passed),
  `python3 -B scripts/gui-goldens.py --self-test`, `python3 -B scripts/test-docs-shots.py` (recorder run
  `c69c1ece-81a2-435a-829b-8f9dc45774d5`), `python3 -B scripts/test-gui-agent.py` (recorder run
  `f8be9065-0615-45ac-add4-e8bc9e0858db`, 94 passed) and `python -B scripts/check-test-sleeps.py` (in its isolated
  environment), locally: passed.

Skipped: macOS, which the launcher's report leaves to native validation and which the golden page does not affect; the
screenshot batch itself (only its script tests ran); CI, which runs only on demand.

#### Things you should know that the report does not say

- Clone's dialog is now called "Clone session" rather than "New session". The plan did not ask for it, but it is
  harmless.
- The search's keyboard highlight now stays on the row you chose when results refresh, instead of jumping back to the
  first row. The plan did not ask for this either. The spec's implementation notes and the tests describe it, and I kept
  it.
- The follow-up that matters most: a new container's warning that deleting its last session removes the container and
  its files now shows only in the Host button's hover help, so a user can start one without seeing it. The matching
  sandbox warning stays visible. Giving containers the same visible note is a small change.
- Other possible follow-ups the reviewer found, none of them blocking:
  - In the column beside the form while the command field is empty, the row that filled the form and the row the arrow
    keys highlight get the same solid fill. Only the small ⏎ cap shows which one Enter takes, and Enter on a recent
    setup launches it.
  - The wide launcher has fixed heights (704 pixels, 1000 with Save as template open), with no stated reason, so a short
    form still gets a tall dialog.
  - The check that keeps a template from changing Replace with's fixed host copies a rule from the shared code instead
    of calling it, so the two could drift apart.
  - No test pins that Enter on the empty field launches the form rather than the newest recent setup. A few tests also
    check less than before: that a broken wrapper prefix is reported again after a correction, and the exact wording of
    a recent row's details.
- The docs pages' published screenshots still show the old launcher until the next docs refresh, as the report says. The
  landing captured both affected pages' screenshots on the test machine to check that their steps still work.
