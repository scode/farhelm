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
