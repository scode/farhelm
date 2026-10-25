## What this was about

When uninstalling Farhelm from a remote host was refused because its identity changed, the refusal quoted identities
that could look identical: the dialog collapses spaces. The maintainer chose to remove those values and point to the
host list, which already makes spaces visible.

Both affected messages now follow that choice. The existing mismatch message names the host list's recorded and reported
identities and retains adopt, correcting the destination, and Remove. A different identity found during uninstall's
fresh check retains the exact Retry instruction, letting the host list explain the resulting state without promising
that both identities will appear.

## Things you should know

The identity checks and uninstall protections are unchanged. Tests cover doubled-space identities, missing identities,
the preserved recovery advice and refusal before host changes. The TODO entry is removed and a fixed changelog fragment
is included. No broader identity-formatting helper or dialog change was needed.

## Open questions and possible follow-ups

The separate setup/update refusal still quotes identities in the same diagnostic style. That was explicitly outside this
uninstall-only plan and remains a possible follow-up.

## The PRs

- [#2708: clarify changed-identity uninstall refusals](https://github.com/scode/farhelm/pull/2708/changes), open draft.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::uninstall_refusal::) | test(provisioning::tests::uninstall_)'`,
  through the recorder with pinned nextest, required pinned tmux, four slots and zero retries: 29 passed, no failures or
  flaky outcomes; 1,289 cases were outside the selection. Run `cb3a3f26-395e-4b6b-af75-d03406db6083` has complete source
  capture, report and output. Retained output contains no runtime `SKIPPED` marker.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`, `dprint check TODO.md releasing/changelog.d/uninstall-identity-display.md`, and
  `python3 releasing/check-changelog.py format`: passed.
- The isolated interpreter's `python -B scripts/check-test-sleeps.py`: 315 delays inspected, zero missing rationale.

The runtime run covered main `876d7073` plus this plan's complete working diff. Those checks and the review apply to
final PR commit `35c5bb67654d461b1128c80f1091385c378e04eb`: the rebased diff is byte-identical, and the inspected
upstream changes through `2f832408` contain only queue bookkeeping and other plans' reports. No earlier runtime result
was reused; these current checks were retained across that bookkeeping-only rebase without repeating them.

Browser tests were skipped as agreed because the dialog renders the supplied string and its layout did not change. The
full workspace battery, desktop smoke and installer/release tests add no relevant evidence for these two messages and
their removed parameters. No instruction file, heading or file location changed, so the agent-instruction checker was
not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no actionable issues in correctness, design,
idiomatic code or tests. It received the full review charter, acceptance criteria and verbatim test-authoring checklist.
The commit and PR wording passed a separate cold read. A documentation pass covered every touched file. There were no
rejected findings, scope departures or weakened guarantees.
