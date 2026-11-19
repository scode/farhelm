### What this was about

Finding a setting meant knowing which section held it. The chosen design adds Search above the section list. Type part
of a setting's name or help to see grouped results and a count beside each matching section. Click a result or choose
one with the arrow keys and Enter to go to it. The Settings guide now describes search.

### Things you should know

- When you open Settings with the gear, keyboard focus starts on the first switch in Sessions. Cmd+F on Mac or Ctrl+F on
  Linux focuses Search. Linux text fields can keep Ctrl+F for moving the caret. One Escape clears a focused query, and a
  fresh press closes Settings.
- Color theme, terminal web-link behavior, ntfy priority, and help-only results open their section at the top. A switch
  that is still loading receives focus when it appears if the search pick is still your latest action in Farhelm.
- App updates and remote localhost tunnels can appear in results on builds where their controls are unavailable. Those
  results open the section at the top. If a credential editor is open, a Tensorlake API key result under Credentials
  takes you back to that editor with your draft visible.
- The tooltip placement audit landed while this plan was in review. Search results and section buttons now follow the
  same hover help placement as the other lists in Settings.

### Open questions and possible follow-ups

The native Mac shortcut remains unverified. Linux tests cover the search interaction and simulate Mac-only Terminal
controls.

### Pull requests

- [#4293](https://github.com/scode/farhelm/pull/4293/changes) adds Settings search and opens the chosen setting.
- [#4294](https://github.com/scode/farhelm/pull/4294/changes) filters the section list and shows match counts.

### Checks

- After rebasing onto the tooltip changes, Linux run `0fdbe788-a862-44a3-89a8-fe01f14622ab` passed all 247 selected
  tests on `1990ab3c`, with
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::/) | test(/^actions::/) | test(/^layers::/) | test(/option_row/)'`.
  This includes search result scrolling, section focus and the shared option-row help placement.
- Linux run `5a4a91a0-38de-4220-9038-0b203dcb2d8c` passed 269 tests on the application source in `e9d82e28`, with
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::/) | test(/^actions_tests::/) | test(/^layers::/)'`.
  Linux run `031fc418-1f0b-406d-83ac-cf2bcc02330b` passed all 11 search tests on the application source in `bcff4ce3`,
  with `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::search::/)'`. This covers counts and focus when
  choosing App without an updater. The terminal keymap coverage from the first run still applies. The rebase kept the
  same Settings shortcut and key contexts.
- `python3 scripts/check-gui-goldens.py --update` passed on Linux for both drafts in runs
  `f9c31142-df6b-44cc-9d6c-e8c16aa1e1c0` and `b3df8424-3800-4e6f-9e29-cb2a186a2110`. Each captured 272 screenshots.
  Reviewed 30 changed Settings images and 4 new search images on the first draft, then the 4 search images that changed
  to show section counts on the second. The tooltip changes affect placement while help is open. The Settings captures
  show the dialog with help closed, and the audit added its own separate tooltip scenes.
- Rust formatting, Markdown, changelog and test delay checks passed on the rebased stack.
  `cargo clippy -p farhelm-gui --all-targets -- -D warnings` passed for both drafts before the rebase. The updated
  search and tooltip code builds as part of the final tests. Backend suites were skipped because the change is in the
  desktop dialog.

### Review gate

Each draft passed one independent source review. The first review led to more complete searchable help and kept a held
Escape from closing Settings after clearing the query. The second kept keyboard focus visible when a search-only section
button disappears.

### Landing

#### What landed with it

It landed alone in its round, right after tooltip-placement-audit, which it was already rebased onto. The independent
reviewer checked that search results and section buttons follow that plan's help placement for lists, and found no
conflict. Main gained nothing else since the plan's base.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The reviewer checked matching
(each word, any case, highlighted where it matched), Escape and Close, unsaved recipe edits while a search hides the
editor, the delayed focus on a switch that is still loading, and that Ctrl+F stays a caret key inside Linux text fields
as the Emacs keys plan made it.

#### Fixes made while landing

- **Search offered settings this build does not have.** On Linux, and on any build whose updater is not active,
  searching "update" listed the App section with two results, and picking one opened a page that only said "App updates
  are unavailable in this build", a state SPEC.md does not describe (it lists App only where an updater is active). On
  Linux, "tunnel" listed "Allow tunnels from remote sessions", which the Terminal section draws only on the Mac, and
  picking it opened the top of Terminal. The plan's tests asserted this behavior on purpose; I read its criterion
  "sections not yet shown in this dialog" as sections the user has not opened yet, not sections this build hides. Search
  now leaves out sections this build hides and the tunnel switch where it is not drawn. The page and the button path
  that existed only for a hidden section are gone. The two tests now assert the results are absent, and a new test
  checks the tunnel switch is searchable only on the Mac.

#### Checks

The plan changes only the desktop app's GUI crate (with new reference screenshots) and the Settings docs page. So the
landing ran the GUI crate's whole test suite, the app's end-to-end smoke, both GUI clippy runs, the reference
screenshots (goldens) and the website build. The suite, the smoke and clippy ran on the maintainer's own test machine,
an arm64 Linux VM that builds and tests the tree itself. The goldens, the website build and the small checks ran locally
on x86_64.

On the plan as delivered, rebased on main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and the website build
  (`cd website && bun install --frozen-lockfile && bun run build`), locally: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-gui)'`, on the test machine
  (`0f6aa19e-94d7-43ec-99f0-d972aaa02038`): all 3204 passed, with no runtime skips.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on the test machine: 49 of its 50 steps passed, and the
  clipboard paste step was skipped, as always on Linux (`f2f50a86-dc40-4810-8c26-aa4bc330493f`).
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` and the same with `--features agent`, on the test
  machine: passed.
- `python3 scripts/check-gui-goldens.py`, locally: passed with the plan's new references
  (`6ac69389-88bf-4ddb-9fb2-de4c0ba836d1`).

On the fixed tree:

- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, then the Settings tests
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::/)'`, on the test machine
  (`8a6b7d94-27da-48bd-907c-19b31cc3e68e`): passed, all 221 tests.
- `python3 scripts/check-gui-goldens.py`, locally: passed (`9bf55125-b604-4584-9e20-4ff473650127`).

The rest of the GUI suite and the smoke were reused: the fix changes only which settings search offers and the two
branches that showed a hidden section, all inside the Settings dialog the settings tests cover.

Skipped:

- Anything on a Mac: the native Cmd+F and the Mac-only Terminal controls were not tried on macOS.

#### Things you should know that the report does not say

- **The search catalog is a hand-written list beside the controls.** A test checks that every listed setting is drawn,
  in order, but nothing catches a control added to a section later and never added to the catalog; its setting would
  just not be searchable.
- **A shortcut mechanism in the dialog layer has one user.** To make Cmd+F or Ctrl+F reach Search even when focus rests
  on the dialog itself, the plan added a general per-dialog shortcut to the layer manager, where the plan had asked for
  a key context on the Settings dialog. A test covers why it is needed; it is more machinery than one shortcut needs
  today.
- **SPEC_impl.md's list of lists that keep their own help edge** does not name the new Settings results list.
- Smaller points: a result's excerpt can show a switch's "off" help while the switch is on, a query of only spaces makes
  the first Escape clear invisible text, the docs page says Cmd+F or Ctrl+F works "from a Settings switch" when it works
  from any control in Settings, and the native Mac Cmd+F is unverified, as the report says.
