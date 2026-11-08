## What this was about

Settings → Credentials showed the **Not encrypted on disk** warning twice above a field labelled only **API key**. The
agreed behavior is one warning for the Credentials overview, one in each credential editor, and a label that names
Tensorlake in Credentials. The Tensorlake settings page retains its own warning and **API key** label.

## Things you should know

The Credentials overview now explicitly includes the Tensorlake API key in its storage warning. This changes the
presentation; saved values are still stored in plain text. Each editor has one visible warning while the overview is
hidden. The spec reflects that rule, and both source TODO entries are removed.

## Open questions and possible follow-ups

None.

## The PRs

- [#3165: clarify credentials warnings and key label](https://github.com/scode/farhelm/pull/3165/changes) — draft, head
  `01a9ee5e063973fc24255e12bbd0b73cbfd93fbc`.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, changed-file `dprint check`, and `python3 releasing/check-changelog.py format` passed.
- `python -B scripts/check-test-sleeps.py` with the isolated pinned parsers inspected 423 delays; none lacked a
  rationale.
- `cargo build -p farhelm`, `cargo build -p farhelm-fixtures`, and the pinned dx 0.7.10 web release build passed.
- Recorder-backed `npx playwright test 'credentials-settings\.spec\.ts' 'tensorlake-settings\.spec\.ts'` passed all 18
  tests: nine Chromium and nine WebKit, three workers, no retries or skips, with required pinned tmux 3.7c. Run
  `54d087dc-e3ec-42f0-b3bc-e05696a35f6b` has complete source and result evidence and complete retained output. It tested
  the final working tree on base `bd8faf85`; committing that tree as the PR head changed no tested source.
- Reused `cargo clippy -p farhelm-ui --all-targets -- -D warnings` from working revision `6a2e07e0`, before the final
  comment-only review corrections. Those corrections changed no checked behavior; final formatting was checked
  afterwards.
- Skipped Rust runtime tests because the nearby unit tests cover unchanged request serialization and message counts; the
  changed rendering and cross-panel behavior are covered by the two browser specs. Broader browser, desktop, installer
  and website checks would not add evidence for this narrow presentation fix.

## Review gate's outcome

The required fresh-context Opus 5.5 reviewer at high effort found no correctness bugs and confirmed the acceptance
criteria. Its wording and documentation findings were addressed. An optional enum replacement for the warning's two
flags was declined in favor of the reviewer's smaller documentation alternative: the Tensorlake inclusion flag applies
only to the plural notice. A separate cold reader checked the commit and PR wording.

### Landing

Landed as delivered, with no fixes at landing. Settings → Credentials now shows one **Not encrypted on disk** warning,
which names the Tensorlake API key, and the key's field there is labelled **Tensorlake API key**; the Tensorlake page
keeps its own warning and its **API key** label.

- #3165: the change, its browser test updates, the SPEC.md paragraph, the two TODO entries' removal and the changelog
  entry.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed on main after this plan's base, and it was the only plan landed in this round, so
the stack applied unchanged. While it merged, a planning change that adds plans and touches only `plans/` and
`TRIAGE_OUTCOMES.md` (#3166) also landed; main now differs from the tested tree by exactly that change. A fresh-context
review checked this plan against the current tree. Every other place that relies on the plain **API key** label is about
the Tensorlake page, which keeps it: the docs website's Tensorlake sandboxes page describes that page, and the v0.36.0
release-notes screenshot script captures it. Only the Credentials test expected two warnings, and this change updates
it. Each credential editor still shows its own warning, worded as before, and the amended SPEC.md paragraph matches what
the page does. The review found nothing to fix.

#### Checks

These ran on the plan rebased onto main `9e54c1e4`:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings` (the UI built for the
  browser), `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check
  that every test delay states its reason (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `cargo build`, and the UI's release web build
  (`dx build --package farhelm-ui --platform web --release`): all passed.
- Four browser spec files on Chromium and WebKit, through the test recorder
  (`python3 scripts/record-test-run.py --runner playwright --kind development --tmux required ...` running
  `npx playwright test` with anchored filters for exactly `credentials-settings.spec.ts`, `tensorlake-settings.spec.ts`,
  `credential-delivery.spec.ts` and `settings.spec.ts`, three workers, no retries): the two this plan changed, the one
  other spec that opens the Credentials section, and the general Settings dialog spec. Run
  `c6b2b3cc-148a-4536-b94d-a929c5cda68c`: all 96 tests passed.
- The UI crate's Rust tests in its desktop build, `cargo nextest run -p farhelm-ui --features desktop`, run
  `418a5634-5632-4f72-862b-a4eaf5112b0f`: all 1097 passed.
- Reused: nothing. The executor's own browser run and lint results were not counted; everything above ran again on the
  rebased tree.
- Not run: the rest of the browser suite, the other crates' Rust tests, the UI's JavaScript unit tests and the docs
  website build. The change touches only two Settings components in the UI crate, the browser tests above, SPEC.md,
  TODO.md and a changelog entry; no supervisor, helm or terminal code, no file the JavaScript unit tests cover, and no
  website page changed.

Nothing in the executor's report became untrue at landing, beyond the PR's state and head commit, which the landing
moved from the draft it lists to merged.
