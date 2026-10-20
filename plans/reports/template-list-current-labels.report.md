## What this was about

The Cmd-J template list had two inconsistencies. Its specification implied that deleting a template invalidated an open
row even if another template took the same name, although picks intentionally resolve the current definition by name.
Separately, after an agent moved a shortcut, the list could still advertise that key beside its previous template, while
pressing the key used the new assignment.

The maintainer chose to clarify the name-based contract and refresh shortcut bindings when Cmd-J opens. The first PR
changes only specification wording and feedback bookkeeping. The second reads preferences independently of the template
catalog, applies the existing protection for pending local shortcut saves, and updates only the two cached binding
fields. Its regression checks both badge kinds, cleared assignments and preservation of unrelated preferences.

## Things you should know

A pick uses whichever template currently has the name, including one edited or recreated after the list opened; only a
currently missing name gives the preparation error in New. This is existing behavior, now stated clearly.

Shortcut labels refresh on opening. A move made while the list remains open can still leave labels stale until the next
opening, as accepted during triage. A failed preference read retains cached labels and does not hide the template list.
Pending shortcut saves keep their precedence over fetched bindings. This refresh does not replace other preferences or
send a preference write to the helm.

The second PR's counted code, tests and documentation diff is 80 lines, below the agreed 150-line gate. The ledger
records the replacement of the original time estimate with that size gate. Both feedback files and index entries are
removed; only the code fix adds a changelog fragment.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [Draft #2381](https://github.com/scode/farhelm/pull/2381/changes): clarify template-list name resolution.
- [Draft #2382](https://github.com/scode/farhelm/pull/2382/changes): refresh template-list shortcut labels, based on
  #2381.

## Checks run, reused and skipped

Run during execution:

- For the documentation PR: `dprint check` on its changed Markdown and `python3 scripts/check-agents-md.py` passed.
  Runtime tests were skipped because no executable or generated content changed.
- For the code PR: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --lib --tests -- -D warnings`,
  changed-Markdown `dprint check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py` passed. The isolated `python -B scripts/check-test-sleeps.py` passed: 304 delays
  inspected, none missing a reason.
- `cargo nextest run -p farhelm-ui --lib -E 'test(list::quick_switcher::) | test(template_bindings)'` through the
  recorder with pinned nextest and tmux, four global slots and no retries: 4 passed, 545 selected out, no failures or
  flaky outcomes. Run `c2283273-7b7a-4f94-b35f-a587be1553a3`. The extra name filter matched no overlay test, so the next
  command fills that separate coverage gap.
- `cargo nextest run -p farhelm-ui --lib -E 'test(number_bindings_refresh_and_clear_through_the_shared_queue)'` with the
  same recorder policy: 1 passed, 548 selected out, no failures or flaky outcomes. Run
  `3e946c0f-ce76-4c85-8106-00e8dc019033`. Both runs tested docs head `89c3f4c27dd2050ec7f250587dffc64cedd9137a` plus the
  code working diff, source fingerprint `841ef1ae7ca306b6523b5a205ca1414b6bebe83121d89077544eef622b1f4304`. Both
  retained outputs are complete, with EOF observed and no runtime `SKIPPED` messages.

These successful commands still cover the stack after its careful rebase onto
`11b5c7cf3be555f1982b0de264307525f3a323c8`: the full intervening diff changes only plan queue/report bookkeeping. The
product/specification diff stayed identical. Later main changes through `16876af9b61d42400cc3cb04627ce349dd9cebc6` only
add a rule for future triage, with no change to this plan's decisions or execution requirements. Later ledger URL and
size updates change no executable behavior. No older executor's runtime evidence was reused.

Skipped broader Rust, browser, desktop runtime, JavaScript and installer checks: the change uses the existing preference
API and Dioxus resource lifecycle without altering layout, focus, input handling, transport or platform code. The
selected tests cover the changed badge transformation and the reused binding overlay. Request cancellation and absence
of a self-triggering signal subscription were checked statically by the independent reviewer, not by a browser run.

## The review gate's outcome

Each PR passed its required single fresh-context gpt-6.1-sol review at high effort with no findings. The code reviewer
checked the installed Dioxus resource/signal implementations: the refresh does not subscribe to the preference signal it
writes, and its task is canceled when the dialog unmounts. It confirmed the 80-line gate and test contracts. Both commit
titles and bodies passed fresh cold reads; the code message was rewritten once to clarify pending shortcut saves. A
separate documentation pass covered every touched file.
