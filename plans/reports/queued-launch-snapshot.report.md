# Queued quick launch keeps the displayed template choices

## What this was about

An armed quick-launch sequence could show one template and launch another after an agent or another client changed a
number shortcut or the quick-launch mark. That could start the session on a different host or in a different folder from
the template shown. The maintainer accepted fixing this only while the change stayed small and localized.

Enter now captures the template names shown by the sequence before fetching current settings and template definitions.
Moving a shortcut elsewhere cannot substitute another template. A key shown as unbound still refuses even when the fresh
settings give it a binding. The feedback entry was removed and its execution recorded.

## Things you should know

The complexity gate held. This uses the existing sequence, dispatch and refusal paths; it adds no stored snapshot or
protocol change. Order and repeated templates are preserved. An unbound number or mark retains its existing binding hint
on the page and refusal in an open launcher.

Only template names are captured. Their definitions are still read fresh, so an edit under the same name applies; a
renamed or deleted template refuses rather than launching the old definition. Single-chord quick launches retain their
fresh binding lookup.

## Open questions and possible follow-ups

None required for this plan. This does not change the separately accepted behavior of templates edited or recreated
under the same name.

## The PRs

- [#2355: keep queued template choices when shortcuts change](https://github.com/scode/farhelm/pull/2355/changes) —
  draft, one reviewable change for the outcome.

## Checks run, reused and skipped

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` passed.
- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(launch_sequence::tests::)'` passed all five selected tests;
  542 other tests were selected out. Recorder run `b6ef984c-9f5e-4618-a900-6680a5fb18b2` used four slots, zero retries
  and no tmux because these are pure unit tests. The two new cases cover moved number and mark bindings, newly bound
  references, sequence order and repetition, fresh definitions and missing-name refusal. Source, output and JUnit
  evidence were complete; selected stdout had no runtime skips, and cleanup required no forced termination.
- Targeted `dprint check` passed for the specification, ledger, feedback index and fragment.
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated
  `python -B scripts/check-test-sleeps.py` passed.
- No earlier plan's check results were reused. The final amendment only added the PR link to the ledger; an interdiff
  confirmed no source change. Main moved only through queue bookkeeping, which was inspected and has no interaction.
- Browser, desktop and JavaScript runs were skipped: no DOM, event capture or JavaScript code changed. The synchronous
  Rust handoff and pure reference resolution are covered by the selected tests and code review. Supervisor, SSH and
  systemd runtime suites do not exercise this boundary.

## Review gate's outcome

A fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design, idiomaticity, test-authoring or scope
issues. The commit message's separate cold read matched its intended meaning and found no contradicted claims or
convention violations. A documentation pass covered every touched file.
