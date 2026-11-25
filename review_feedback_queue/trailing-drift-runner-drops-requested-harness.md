# Drift runner silently skips a requested agent with no case in the chosen category

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When the maintainer asks the drift runner for specific agents in a specific category, an agent that has no case in that
category is dropped without a word, as long as at least one other requested agent does have a case. The run reports
results for the others and exits successfully, so the maintainer can believe the dropped agent was checked.

## Details

Concerns b784617a19. Main reference `scripts/run-harness-drift.py:280-296`: the unknown-harness check, then the
`selected` comprehension filtering `(category, name) in cases`, then the empty-selection error.

`unknown = set(options.harness) - {name for _, name in cases}` validates harness names against every category, not the
selected ones. `selected` then keeps only pairs present in `cases`; the only guard is `if not selected`, which fires
only when nothing at all matches. Example at the tip: `workspace_trust.rs:311-318` registers antigravity, claude, codex,
muse and pi (no grok). `run-harness-drift.py --category workspace_trust --harness claude --harness grok` runs only
workspace_trust/claude, prints one PASSED line, and returns 0.

`summarize(..., explicit=bool(options.harness), ...)` already encodes the rule that an explicitly requested harness must
yield a real verdict (an explicit COULD NOT RUN fails the run, `scripts/run-harness-drift.py:237-240`), and this path
bypasses it because the requested harness never gets a result entry at all. The new test
`test_invalid_category_combinations_run_no_upgrades_or_cases` covers only the all-empty case
(`--category model_selection --harness grok`), so a regression here is untested. `docs/harness-drift-tests.md:21-24`
says `--harness` "narrow[s] the selection", which fits `--category all` filtering but not dropping one of several
explicitly named harnesses from a single named category.

Fix: when `--harness` is given and the categories were named explicitly (not `all`), fail with a parser error for each
requested (category, harness) pair that has no case, or at least print a SKIPPED line per missing pair and make it fail
the run like an explicit COULD NOT RUN. Add a test with two harnesses where one is absent from the category.

Bucket: other (maintainer test tooling; the consequence is a silently missing live check, which could let an agent CLI
drift ship unnoticed). Not highest: no security or data-loss route.

Confidence: confirmed from source.

Possible cover: `docs/harness-drift-tests.md:21-24` ("Combine it with `--harness <name>` to narrow the selection") may
be read as accepting this; it does not mention dropping explicitly named harnesses.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g01 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
