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

### Landing

#### What landed with it

It landed in a round of three plans: covered-link-refusal, demo-video-capture and replace-with-checkout-hint. Since its
base, main gained the previous round's nine plans (among them session group dragging, model families, the session-limit
notice and compare-on-save checkout settings), bookkeeping and docs screenshot tooling. When stacked, it conflicted with
main on one import line in the launcher, which compare-on-save checkout settings had changed; I kept main's imports and
added the plan's. Main's launcher changes do not touch what the hint reads, and the reviews found no other interaction.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The behavior is conservative
and correct: the launch dialog says the checkout will be kept only after the folder check confirms the new session's
folder is the same directory as the replaced session's managed checkout, and the warning names that replaced session's
checkout. Nothing is bigger than the plan needs.

#### Fixes made while landing

- **A docs page lost a fact.** The Stop, restart and resume page had said that Replace with keeps the existing sandbox
  or container; the plan's edit replaced that clause with the checkout text. It is back.
- **Both docs pages made the warning sound conditional.** They said "If you replace the last session…, the warning under
  Where explains…", but the warning always shows once Managed checkout is chosen; only what happens to the old checkout
  depends on it being the last session. Both now say so.

#### Things you should know that the report does not say

- The folder check's module comment still says it only asks whether a path is a usable directory; its answer now also
  carries the resolved path the hint relies on.

#### Checks

The three plans change the desktop app's GUI crate, a patch to the vendored GPUI core, the demo video tooling and its
documents, AGENTS.md and the specs. The landing ran the lints, the GUI crate's whole suite, the vendored GPUI core's own
tests, the video tooling's tests, the app's end-to-end smoke and the reference screenshots (goldens). Builds ran
locally; the GUI suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-readme-video.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings` and
  `python3 -B scripts/check-gui-lints.py`: passed.
- The vendored GPUI core's tests (`cargo test --manifest-path vendor/gpui-pre/Cargo.toml --features test-support` with
  the native-key, driver-record and new hit-test occlusion tests), locally: passed
  (`3130bce8-b268-4c31-977b-5bc1afe5a949`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`01c51026-2b44-41af-9830-f4c6eb0e081a`).
- The boat runs did not happen: every remote command on all four boats ended without reporting its result, and the
  harness deleted the boats as unsafe to reuse. That looked like a problem on the boat service's side; nothing ran.

On the fixed tree (the covered-link refusal fix and its test, a screenshot scene's staging, and documents), on four
fresh boats and locally:

- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats: 1085, 1033 and 1022
  passed, 3140 in all, with no runtime skips (`71d71daf-a7e6-4aab-9dc6-39032f010643`,
  `176070d9-503b-4ea8-a89c-39c3f0062335`, `3d8609ce-34b9-431e-821c-e4ed78423b3d`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`409e2e23-c47f-4c6d-9d19-5355b3103ec2`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`29aca505-d814-43b8-b492-c29f1eea2d14`).
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, once
  without and once with `--features agent`: passed.

Reused for the fixed tree: the vendored GPUI core's tests and the video tooling's tests, whose code did not change, and
the GUI lint check, the workspace and desktop clippy runs and the changelog format check, since the fixes changed only
the GUI crate (covered by its clippy runs above) and documents, and no lint configuration or changelog fragment.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the plans stay in the GUI crate and the vendored GPUI
  core.
- A full capture of the demo video: the executor captured the sample and handed it over; the landing changed none of the
  capture code.
- The Mac-only checks: no Mac-specific code changed.
