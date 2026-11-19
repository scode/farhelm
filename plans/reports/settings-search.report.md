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
