# Make Set up and launch primary in the session launcher

## What this was about

When setup for Restart is off and you pick Grok or Antigravity, the session launcher asks whether Farhelm may set up the
hooks it needs to resume conversations. The launcher previously highlighted **Launch without**. As the maintainer chose,
**Set up and launch** is now highlighted on the far right, and initial keyboard focus stays on **Cancel**.

## Things you should know

**Set up and launch** still waits for the setting to save before starting the session. If the save fails, the launcher
shows the error and keeps the choices available. **Launch without** still opens the session without writing that
setting. I updated the implementation spec to account for an inline primary answer that waits for a setting write.

## Open questions and possible follow-ups

None.

## PRs

- Promote **Set up and launch** on the Grok and Antigravity Restart setup prompt.
  [#4630](https://github.com/scode/farhelm/pull/4630/changes)

## Checks

- Ran the pinned nextest selection
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^prompt_tests::/) | test(/^launcher::view_tests::/)'` after rebasing
  onto landed launcher and GUI helper changes. All 115 tests passed. Run `c762d53c-77ec-4c5e-9314-8fcd2c39ed48`.
- Ran `python3 scripts/check-gui-goldens.py --update` after rebasing. Rendered 406 Linux references and checked their
  hashes against the manifest. Only the light and dark hook setup pictures changed. Run
  `acea1ce7-6777-4e57-92fc-a6122da31662`.
- Ran `python3 scripts/check-gui-goldens.py --scene launcher-hook-setup --jobs 2`. The first comparison against the old
  references reported the two intended differences, confined to the swapped buttons. Both appearances matched after
  publishing the two new references. Initial comparison `762bafa5-76f0-4b25-8c90-ecfd839bed78`. Published comparison
  `78529ff8-76f7-46ec-a02d-2cae59d6fefa`. Runtime commands ran through `python3 scripts/record-test-run.py`.
- Ran `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the isolated
  `python -B scripts/check-test-sleeps.py`, Markdown formatting, changelog format and `git diff --check`. The final
  draft reuses the post-rebase source checks from `56de863b`. Only the two stored reference hashes changed after those
  checks.
- Skipped the broad runtime battery. The focused tests cover the changed prompt and launcher behavior, and the complete
  Linux image catalog checks for shared visual effects.

## Review gate

The source reviewer verified both repairs from review. The one-off launch test now checks every request so an unintended
settings write would fail it. The prompt documentation describes what happens to focus and dismissal while the save is
in flight. The reviewer checked both screenshot differences, and the commit message passed its separate cold read.
