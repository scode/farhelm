### What this was about

The desktop launcher and template editor each drew their own Runs on, Recipe and Container host controls. The maintainer
asked to consolidate them during the GPUI migration. The two screens now draw those controls from the same code, which
removes 123 lines overall.

### Things you should know

When saving a template, you can still choose a container host whose Podman check failed or a sandbox recipe that has no
snapshot yet. The editor defaults to the first listed container host. At launch, Farhelm checks readiness, prefers the
last container host you used while it remains usable, and shows the recipe's build status for the selected container
host. The existing control labels, saved choices and keyboard handling carry over.

### Open questions and possible follow-ups

None.

### PRs

- [#4068](https://github.com/scode/farhelm/pull/4068/changes) shares the placement picker between the launcher and
  template editor.

### Checks

- Passed on the final change: `cargo fmt --all -- --check`,
  `dprint check TODO.md docs/gpui/parity/templates-and-quick-launch.md`, and `git diff --check` before committing. The
  base's foundation check
  (`git show dc52d486:scripts/check-foundation-paths.py | python3 -I - --repo . --base dc52d486 --head b7471ab0`) passed
  and reported the one expected launcher export.
- Passed on `e34951bc`: `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and all 793 selected tests from
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^launcher::/) | test(/^templates::/)'` (run
  `da7ac312-5e33-4239-84d2-298e72b812ef`). Those tests include default container hosts, unusable options and retained
  template choices.
- Passed on `e34951bc`: `python3 scripts/check-gui-goldens.py` compared the existing light and dark Linux captures pixel
  for pixel (run `67263931-0ee2-4b6f-85cd-32d95c654b42`). Reused Clippy, nextest and golden results for the final PR
  revision because only Rust comments changed after that snapshot.
- Skipped the full workspace suite because the change is confined to desktop picker wiring. Used the existing golden
  suite for visual checks. The live-app screenshot scenes and golden references were left untouched, and no separate
  screenshot batch ran.

### Review gate

The independent GPT-6.2 Sol high reviewer found no behavior regressions. Its documentation finding led to clarifying how
each screen picks a default container host. The commit wording passed its cold read.

### Landing

#### What landed with it

Since the PR was built, main gained one other change, the docs follow-up for environment recent setups and the
suspended-host launch refusal (#4067), which touched only website pages and TODO.md. The landing rebased this PR onto
it. The two met only in TODO.md, where each had removed its own entry; both removals are kept, which leaves the "GPUI
last mile" section with its heading and introduction and no entries. Whether to drop that heading is your call.

An independent reviewer compared the shared picker line by line with the two copies it replaces, for the launcher and
for the template editor. It found both screens unchanged: the same places offered, hidden or disabled; the same
defaults, labels, help, names tests find controls by and keyboard behavior; the same edits written into the launch being
set up or the template; and the template editor's own authoring rules kept (the Runs on switch always shown, only an
incompatible recipe base disabling a recipe, an unusable container host still choosable with its reason). No test
changed, and both screens' existing tests still exercise the shared code.

#### Fixes made while landing

- **Comments and docstrings only, from the review.** One launcher comment had lost the reason the Folder field is
  updated when Runs on changes (otherwise Existing host would show the folder typed earlier while the launch uses the
  home folder). The original wording is back. The new shared types now document their contract, and the template
  editor's code says why it ignores a press on the already chosen New sandbox or New container segment once nothing is
  left to place it on (for example, the last container host was removed while the editor was open, so the segment only
  stays because it is the template's choice): passing that press on would move the template to Existing host, while the
  launcher ignores the same press on its own. The editor's module docs again summarize its authoring rules and say that
  it does not draw the launcher's note about building the recipe at the chosen place.

#### Things you should know that the report does not say

- **The GPUI foundation ownership table has no row for the shared picker's file.** The picker lives in the launcher's
  code (`crates/gui/farhelm-gui/src/launcher/view/placement.rs`) and now also draws the template editor's controls. The
  editor reaches it through a launcher file the table does cover, so the ownership check passed, but a later change to
  the picker file alone would change the template editor without the check reporting it. The plan forbade editing the
  ownership table (`docs/gpui/foundation-ownership.md`), and the GPUI area sessions that owned it have ended, so adding
  the row is a small follow-up for you to schedule.
- No test covers the template editor ignoring that press when nothing is left to place a new environment on; the GPUI
  parity doc for templates and quick launch (`docs/gpui/parity/templates-and-quick-launch.md`) states the behavior
  without citing a test. A short test would protect the difference the new comment describes.
- The plan named an Opus design consult and a GPT-6.1 Sol reviewer; the executor's working log says a maintainer
  override replaced both with GPT-6.2 Sol, which the plan's Decisions do not record. The landing's own review and checks
  covered the result either way.

#### Checks

The change is a refactor of the desktop app's GUI code with no intended visible change, so the landing ran the GUI
crates' whole test suites, the app's end-to-end smoke and its screenshot goldens. Builds and lints ran locally; the test
suites ran on boat sandboxes (cloud VMs), from binaries built locally.

On the PR rebased onto main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `python3 -B scripts/check-gui-lints.py` and `python3 -B scripts/test-check-gui-lints.py`: passed.
- The check that a GUI change modifies the app's shared foundations only where they allow it
  (`scripts/check-foundation-paths.py`, from the base revision): nothing refused; it listed the one-line addition to the
  launcher view that makes the picker reachable from the template editor.
- `cargo nextest run -p farhelm-gui -p farhelm-grid-model -p farhelm-input-mapping` from a locally built archive, split
  into two halves of the suite on two cloud VMs: 1571 and 1567 tests passed, none failed, none reported a runtime skip
  (`916f9ec3-53f3-4c36-b3a3-a8110bb4d245`, `2a0fb2ed-a46d-44e5-b754-fcd775f59c55`).
- The app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py --no-build`, through the recorder) on a cloud VM: 49
  steps passed, the clipboard paste step skipped as it always is on Linux (`3976197a-0320-44c4-8bba-0639ecc4553f`).
- The app's screenshot goldens (`python3 scripts/check-gui-goldens.py`, through the recorder), locally: passed
  (`c05a8297-1a41-4b26-a788-6c2ff05ec220`).

After the landing fixes, which changed only comments and docstrings in two GUI files: `cargo fmt --all -- --check`,
`cargo clippy -p farhelm-gui --all-targets -- -D warnings` and `python3 -B scripts/check-gui-lints.py` passed, and
rustdoc reported no broken link in the changed lines. The test runs above were reused for the final commit, since
comments change no behavior.

Skipped: the rest of the Rust workspace, the desktop process's own tests, the Linux app lifecycle and the Mac-only
checks, since the change touches only the launcher's and template editor's placement controls.
