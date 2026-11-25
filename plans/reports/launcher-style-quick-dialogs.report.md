# Launcher-style quick dialogs

## What this was about

The session switcher, template launcher and Rename used different layouts from the session launcher. Rename in
particular put a short title field under a tall heading. The agreed direction was to use the session launcher's field,
result rows and keyboard hints across these dialogs.

Cmd+K now shows the agent, host, status and age alongside each result, with a hint for template search and naming a new
session. Cmd+J shows shortcuts on the rows and marks a template **opens New** when its saved choices need completion.
Rename puts the title field first, with the session's agent, destination and status directly below it. Rename group and
New group use the same editor and footer without the session line.

## Things you should know

- Search, ranking, shortcuts, dispatch and focus return keep their existing behavior. **opens New** reflects the saved
  choices. An otherwise complete template can still open the session launcher if preparation cannot resolve a choice
  such as a model or directory.
- Rename keeps the established Close, field, Cancel, Save Tab order even though Close is drawn to the right of the
  field. Only naming after a quick launch selects the initial title. The editor still sends pasted line breaks unchanged
  so Farhelm can reject invalid titles.
- The keyboard tips show the two arrows in one chip and put the typed example beside the Return chip. The visual reviews
  accepted both. The group-name field still opens empty without a placeholder. The ordinary session launcher matched all
  80 of its reference pictures byte for byte when its shared drawing pieces were extracted, and later full renders
  showed no launcher deltas.
- The session list, launch templates and keyboard shortcuts guides now match the dialogs. The published v0.41.0 New
  group release-note image still shows the earlier layout. The release image and its archived capture steps stay frozen.
  The current screenshot batch and the smoke steps use the current dialog targets.

## Open questions and possible follow-ups

None required for this stack.

## Pull requests

- [#4975](https://github.com/scode/farhelm/pull/4975/changes) shares the launcher field, hints, labels, row contents and
  template marks.
- [#4985](https://github.com/scode/farhelm/pull/4985/changes) gives both quick searches the shared frame and adds the
  session details.
- [#4991](https://github.com/scode/farhelm/pull/4991/changes) gives both template lists the two-line rows and Cmd+J the
  preparation and shortcut cues.
- [#4997](https://github.com/scode/farhelm/pull/4997/changes) updates Rename, Rename group and New group, updates the
  guides and removes the completed TODO.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-gui --lib -E <selection>` passed at each step. Launcher, templates and switcher passed
  305 tests at `2eddb7e7` (`d7182a41-ad87-400a-b869-d3b95efaf1fe`). All 52 switcher tests passed at `5251a5af`
  (`6666aa9c-bef4-405a-af26-4edbdd0911f7`). The switcher, templates and launcher search passed 212 tests at `711ea31e`
  (`cc6dd7be-7672-48ec-9822-8b405be3790c`), with eight template list tests repeated at `3028a7b6`
  (`09637e94-8a2c-4a50-bfd6-9ad219ea6229`). Rename, groups, naming, shared inputs and switcher passed 121 tests at
  `6b4a0a7f` (`9a3ab2ee-c2d1-4a8b-991b-43dbf6f4b588`). Rename, groups, naming and text inputs passed 52 tests after the
  final layout adjustments at `4eada42c` (`461339bc-d997-4389-af82-197d34ad3678`).
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, `python -B scripts/check-test-sleeps.py`,
  `cargo fmt --all -- --check` and `dprint check SPEC.md SPEC_impl.md` passed. `bun run build` in `website/` built 41
  pages and validated links at `1de1f1a9`. The website pages did not change after that run.
  `python3 scripts/test-gui-screenshot-batch.py` passed all 47 cases with the final manifest, and
  `python3 scripts/test-gui-e2e-smoke.py` passed 10 script guard cases after the switcher targets changed. These checks
  have no recorder IDs.
- The bounded full `python3 scripts/check-gui-goldens.py --update` runs covered each draft in both appearances, in stack
  order. Recorder IDs are `9fba560d-0fe5-4542-bb3c-9fdac0e7017e` at `2eddb7e7`, `eb66b914-ca17-4af6-b37a-4d497e8580d8`
  at `5251a5af`, `5e46d9ff-ec11-47e5-ad67-ce3ffdb01772` at `3028a7b6` and `c55a0f35-daf3-459c-963d-6cbaf70ed06c` at
  `4eada42c`. The previous full run at `6b4a0a7f` (`e33bcca0-5a7c-43f4-9820-70ecc35d3ad7`) helped isolate the Rename
  fixes. Generated reference manifests, commit descriptions and the final command-field doc comment changed after
  rendering without changing executable code. Early build errors, a stale assertion and an early custom-agent capture
  failure were corrected and verified in later runs.
- Full workspace nextest and the live desktop smoke and docs captures were skipped in development. Focused GUI tests
  exercised the changed dialog behavior, and the screenshot-batch tests verified the new scene inventory. The landing
  monitor runs its own integration checks.

## Review gate

Each draft received the required independent source and visual reviews. Review corrections included preserving
path-match highlights when shortening a home directory, making the template's preparation tag visibly distinct from a
key, keeping pasted-line-feed handling in the decorated Rename field, and aligning Rename's focus, spacing and session
context with the other dialogs. The final visual review accepted the family in both appearances.
