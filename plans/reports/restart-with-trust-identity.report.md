## What this was about

With automatic workspace trust enabled, Restart with could trust a folder replaced outside Farhelm because the session
still belonged to the old managed checkout. This also affected ordinary sessions borrowing another session's checkout.
The earlier decision required folder identity to match before automatic trust was granted.

Restart with now uses the same identity-checked ownership read as New for the session's working folder. Stale membership
alone cannot grant trust. The change adds no new identity mechanism and stays within the size gate: 51 changed lines of
code, tests and comments, excluding the feedback cleanup, execution ledger and changelog fragment.

## Things you should know

Plain Restart deliberately keeps its stored launch, including its previous trust choice. Restart with keeps the same
working folder while recomputing trust for its edited launch. A missing or replaced folder can still prevent the
supervisor from restarting the session; this change does not bypass that refusal.

The regression uses the plan's accepted scripted-supervisor approach: a borrowing session retains checkout membership,
but the folder identity read says it is no longer managed. The test verifies that both the reusable launch and a named
conversation launch omit automatic trust. Separate existing supervisor tests exercise real missing and replacement
folders. No vendor agent was run.

## Open questions and possible follow-ups

None. The recorded scope and size gate were met.

## The PRs

- [#2738](https://github.com/scode/farhelm/pull/2738/changes) — Restart with checks checkout identity before granting
  automatic trust; draft, not merged.

## Checks run, reused and skipped

Both runtime commands ran through `python3 scripts/record-test-run.py --runner nextest`, kind `development`, with four
slots, zero retries, pinned nextest 0.9.148 and required repository-built tmux 3.7c with matching binary identity. Their
reports, source capture, output and cleanup were complete; neither printed a runtime `SKIPPED` marker.

- `cargo nextest run -p farhelm-helm --lib -E 'test(/workspace_trust|restart/)'`: 26 passed, including the changed
  Restart-with regression and New/Replace trust cases; 1289 selected out. Run `b93a9182-18c0-4e39-8d22-57ff3e805b7a`.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(managed_directory_snapshot)'`: 2 passed, covering allocated
  ancestry and missing/replaced folders; 1322 selected out. Run `d561a3b1-50f9-4e41-95c9-ce39fb2b3cc4`.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`: passed for the changed crate.
- `cargo fmt --all -- --check`, `dprint check` on the changed applicable files, and
  `python3 releasing/check-changelog.py format`: passed. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 322 delays with no missing rationale.
- `python3 scripts/check-agents-md.py`: passed with zero errors and two existing citation warnings, unrelated to this
  change.

The commands above covered the source patch on base `78846898`; the helm runtime preceded only a helper doc-comment
clarification and bookkeeping edits. The supervisor runtime and Clippy covered the final Rust behavior. After inspecting
the complete upstream diff, the change was rebased onto `6376dae9`: only other plans' queue claims had changed, and the
patch was byte-identical before and after. Those successful commands are reused individually; the later execution-ledger
PR link changes no executable input. No failed command was reused.

Browser, desktop, installer and whole-workspace suites were skipped because the change is confined to the helm's trust
decision and the targeted protocol and identity tests cover its behavior. There are no UI or installer changes.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reported NO FINDINGS. It checked the fixed-folder
canonical-path reasoning, the scripted borrower regression, the acceptance criteria and the full test-authoring
checklist. The separate documentation pass and commit/PR wording cold read are complete.
