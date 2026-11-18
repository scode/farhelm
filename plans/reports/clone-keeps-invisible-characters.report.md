## What this was about

A copied folder or session name could look unchanged after an edit in Clone while the launch used a different path or
name. Farhelm showed invisible characters as text such as `<U+200B>` and then stored that visible text on the first
keystroke. Command fields and Custom flags had the same problem, and Bash could read the resulting angle brackets as
redirection. The affected fields now keep invisible characters through edits, and the recent-folder filter uses the same
underlying path as the launch.

Restart with and Replace with also said “then launch” when a suspended or stopped host needed to be resumed in Hosts.
The advice now names the action the user was trying to take.

## Things you should know

As agreed when planning this fix, an exact spelling typed into an affected field stands for its invisible character. If
a copied value contains literal text such as `<U+200B>`, editing the field converts it to the character even when it
still looks the same.

Restart with still spells out a stored command or resume command containing invisible characters and makes that field
read-only. An untouched command remains available for a restart that changes another setting. Clear empties that field
and makes it editable, even if the old command is then typed back in.

## Open questions and possible follow-ups

Pasting a raw invisible character in the middle of a field can move the caret to the end and clear that field's Undo
history when Farhelm replaces its display with the spelling. An edit beside an existing spelling keeps the editor's
normal history. Preserving caret and Undo through live reformatting would need input-widget support that covers paste
and input methods together. That work is deferred.

## PRs

- [#4131](https://github.com/scode/farhelm/pull/4131/changes) keeps invisible characters through launcher edits.
- [#4132](https://github.com/scode/farhelm/pull/4132/changes) names the action to retry after resuming a host.

## Checks

- Ran locally on Linux. The recorded `cargo nextest run --locked -p farhelm-gui --lib -E <focused selection>` passed all
  146 selected tests at `fcbbc0056a9e`. Run `ae1a08f4-7d67-4c93-b21a-53e8aca37f1b` covered launcher editing, templates,
  quick launch, Resume wording and session-list environment actions. No selected test reported a runtime skip.
- `cargo clippy --locked -j 6 -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check`
  on changed Markdown, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py`, and
  `git diff --check` passed. `python3 scripts/check-agents-md.py` passed with an existing archived citation warning.
- In run `2849989c-d129-47bd-9882-6eb9a61da57b`, the Clone test clicked Name while the recent-folder dropdown covered
  it. The earlier interrupted run `847f68dd-1e1f-4aa9-b04a-a2f7a5327570` recorded the same failure. The corrected test
  passed by itself in run `58651975-6c21-42bd-8426-bd9e3a2c4e63` and in the final selection.
- Reused no runtime runs. Skipped screenshot goldens because their scenes do not show the changed inputs or advice. The
  focused selection covers the changed GUI behavior, so the wider workspace and desktop smoke suites were skipped.

## Review gate

Both PRs cleared fresh-context GPT-6.2 Sol high-effort review and a cold read of the commit and PR text. Review caught
the recent-folder filter reading the displayed spelling and strengthened the test for editing after Clear. The paste
behavior is deferred as described above. This report received a separate cold read before delivery.
