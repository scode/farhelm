## What this was about

A terminal-link refusal could cover the warning in a session's Delete confirmation while its focused confirming button
remained usable. The Delete shortcut could open that question without moving the pointer off the refused link. The
maintainer's decision was: “It may not cover the warning.”

The refusal now stays below an open session confirmation, whichever opens first. The same rule covers Delete, Restart
and Replace, which share the confirmation panel. The layer-order explanation was updated, and the feedback item was
removed with its execution recorded in the ledger.

## Things you should know

The browser regressions exercise Delete through its keyboard shortcut in both opening orders, establish that the warning
and refusal really overlap, and compare their painted order. Restart and Replace share the checked markup and CSS rule;
they were inspected in source rather than driven separately. The approval-card exception in SPEC.md is preserved, with
its existing browser coverage passing.

## Open questions and possible follow-ups

None within the recorded scope.

## The PRs

- [#2740](https://github.com/scode/farhelm/pull/2740/changes) — keeps terminal-link refusals below session confirmation
  warnings; draft, not merged.

## Checks run, reused and skipped

Browser runs used `python3 scripts/record-test-run.py --runner playwright`, development mode, three configured worker
slots with one private stack each, zero retries, Playwright 1.64.0, and required repository-built tmux 3.7c with a
matching binary hash. Successful reports, source identity, output through EOF and cleanup were complete, with no skipped
cases.

- `npx playwright test 'terminal-link-clicks\.spec\.ts' -g 'header confirmation stays above a link refusal'`: 4 passed,
  two per engine. Run `2d90a292-a712-4eaf-bdf1-b6e174b9f657`.
- `npx playwright test 'terminal-link-clicks\.spec\.ts' -g 'covered terminal disclosures'`: 18 passed, nine per engine,
  including both new cases and existing approval, tunnel, pill and raised-refusal behavior. Run
  `091d475b-60f9-4993-a865-5c98959c3bfa`.
- Negative control restoring the refusal's old raised layer: all four new cases failed at the painted-order assertion,
  as intended. Run `5c3333e7-7d99-4da0-9225-7d011d46aa31`. The temporary test-only override was removed before the
  successful 18-case command.
- The first version of the test incorrectly required the warning to be topmost even where the staged approval card
  overlapped it. All four cases failed in run `53db3f32-93ef-4dad-9a5e-d7ba29b8bf4c`. The corrected oracle compares the
  warning and refusal directly. This failed command is retained and supplies no passing evidence.
- `cargo build` and `dx build --package farhelm-ui --platform web --release`: passed, preparing the real application
  used by the browser runs.
- `python -B scripts/check-test-sleeps.py` with the isolated pinned parser environment: 322 delays inspected, none
  missing a rationale. Changed Markdown passed dprint; changelog format passed.
- `python3 scripts/check-agents-md.py`: zero errors and two existing citation warnings outside this outcome.

The successful browser commands covered the final product and test patch on base `ec74803a`. They and the builds are
reused individually after rebasing onto `660fb5b5`: the full upstream diff contained only queue state and another plan's
report, and this patch's executable inputs were unchanged. The later ledger PR link is bookkeeping only.

Rust checks and the JavaScript unit harness were skipped because no Rust or JavaScript behavior changed. The CSS paint
contract needs browser integration evidence. The unrelated full browser suite and native desktop checks were skipped;
the selected Chromium and WebKit group covers this layer interaction. The repository's dprint configuration has no CSS
or TypeScript plugin, so an initial attempt to format those files returned “no files found”; their formatting was
inspected manually.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reported no findings. It rechecked the corrected
paint-order probe and acknowledged the test issue missed in its first pass. The documentation pass, resume check and
commit/PR wording cold read are complete.
