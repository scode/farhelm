## What this was about

Replace with opens the session launcher with Folder set to the source's directory. If the source uses a managed
checkout, leaving Folder alone keeps the same checkout and edits. Choosing Managed checkout starts a fresh clone without
those edits, which stay in the old checkout. The launcher gave no way to tell what the two choices meant for work
already in that checkout.

The plan kept both choices and put the explanation alongside them. The Folder hint uses the checkout mark from the
session details bar. The Managed checkout warning names the source checkout even if you choose a different repository
for the replacement.

## Things you should know

- Keeping the source directory keeps the checkout. This includes a session's subdirectory and a symlink the launcher
  confirms points there. When the launcher recognizes a saved symlink spelling, it waits for the existing folder check
  before promising the changes are kept.
- With a fresh clone, the old checkout and its edits stay in place while another session uses it. If the replacement
  removes the checkout's last session, it goes to the host's trash. The initial plan assumed immediate discard inside a
  sandbox or container. Replace archives there too. That trash stays inside the sandbox or container and is deleted with
  it. The warning explains both cases.
- **Start a session** and **Stop, restart, resume** now explain what the two choices mean for the old checkout.

## Open questions and possible follow-ups

There is no decision outstanding.

## Pull requests

In landing order.

- Show the source checkout's name and mark below Folder. [#4177](https://github.com/scode/farhelm/pull/4177/changes)
- Warn under Where about the checkout a fresh clone leaves behind.
  [#4179](https://github.com/scode/farhelm/pull/4179/changes)

## Checks

The nextest runs use `cargo nextest run --locked -p farhelm-gui --lib` with four slots and zero retries.

- On main `c5263a7f` with the reviewed Folder patch applied, recorder `ff9e7137-090f-4b48-a663-ee62eded26ed` passed four
  focused Replace with tests. They cover the source prefill, a confirmed alias for a subdirectory, an alias now pointing
  elsewhere, and a source without checkout membership. The filter was
  `test(/^launcher::replace_with_tests::replace_with_(prefills_the_source|explains_the_sources_checkout|does_not_promise_a_repointed|of_an_unmanaged_source)/)`.
- Initial run `45487efa-d259-4f6e-b26a-3eddbdd8105d` at `c5263a7f` plus the first Folder patch passed 35 of 36. It
  selected `test(/^launcher::replace_with_tests::/) | test(/^session_pane::details_tests::/)`. The failure came from
  assuming the prefill fixture had no checkout. The regression case now explicitly sets the source's checkout membership
  to absent. An intermediate combined run `d3573931-06fe-4a5f-8e9c-fd05f349891d` used the broad filter below on
  `5343c248` plus the earlier second patch. It was stopped while incorporating review findings.
- On draft `5343c248` plus the second patch, recorder `56b89617-e64c-469b-94fc-b6e23edcfe34` passed 144 of 146 selected
  GUI tests. Both failures were in the new warning tests. Their shared fixture had a working directory that disagreed
  with its canonical path, so the starting Folder hint correctly waited for a path check. The tests now give the source
  matching paths and its own checkout metadata. Recorder `080688cd-ccfb-4408-82c6-f1b94fc1caa9` passed both corrected
  tests on `6b46c747` plus the fixture update. The broad filter was
  `test(/^launcher::(replace_with_tests|checkout_tests|draft_tests|folder_check_tests|browse_tests|entry_tests)::/) | test(/^session_pane::details_tests::/)`.
  The retest used
  `test(/^launcher::replace_with_tests::replace_with_warns_(where_about_the_sources_checkout|that_the_checkouts_trash_stays_inside_the_sandbox)$/)`.
  No selected test reported a runtime skip.
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` passed on the combined patch before the final
  wording changes and test fixture corrections, above `5343c248`. Its coverage still applies to the unchanged
  implementation.
- The documentation site built all 37 pages and passed internal link validation with `bun run build`. Cargo format,
  dprint on touched documentation, changelog format, whitespace and the Rust test-sleep check passed.
- The Linux GPUI light and dark screenshot comparison passed in recorder `08020571-16eb-4442-9e88-e357f9393f26` with
  `python3 scripts/check-gui-goldens.py` and libtest's default thread count. It built both UI patches above `5343c248`.
  The subsequent fixture correction does not affect the captures. The new notices reuse existing visual styles and their
  states are covered by the focused GPUI tests.
- The stack was rebased onto `bc98e4ec` to keep an unrelated new TODO entry, with final tip `7ba69005`. Main's other
  change was to the separate documentation screenshot tool. The range diff matched both reviewed patches, and dprint and
  whitespace checks passed again. The tested paths are unchanged by the rebase.
- The full workspace suite was skipped. The focused GUI tests cover the changed flow and the related controls.

## Review gate

Both patches passed independent source review and cold reads of their commit messages. Review caught that a saved alias
can no longer point to the source's directory. A scope check supported retaining the resolved path from the existing
folder check. The second review caught the distinction between Replace's archive and a separate environment Delete's
discard, and tightened the test and docs to match. Two independent scope checks and the final reviews agreed that the
resulting changes are limited to display and documentation.
