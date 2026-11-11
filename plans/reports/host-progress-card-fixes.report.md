## What this was about

A host's update details appeared when hovering its name in the sidebar, while hovering its progress bar or the space
around it showed nothing. The whole working row now opens the same card. Moving between the name, bar and padding keeps
the card in place, and pressing anywhere dismisses it.

Long connection errors and update messages previously showed their beginning and could be cut through a line. Their
hover previews now show whole lines from the end, keeping the final diagnosis readable beside the current step.

## Things you should know

The card provides the same preview in the sidebar and Hosts, with `Full details in Hosts` when a message is shortened.
Opening Hosts gives access to its full text. A larger window or shorter message restores the whole preview. Operations
whose first steps have not arrived use the name button's plain hover help across the working row.

I used one shared hover owner for the working row and a definite line height for diagnostic previews. Both covered TODO
entries are removed. The two changes amend the existing unreleased release note. The combined code, test and spec diff
is within the plan's approximately 400-line limit.

## Open questions and possible follow-ups

No required follow-up was identified.

## The PRs

- [#3309](https://github.com/scode/farhelm/pull/3309/changes) opens host progress details from the whole sidebar row.
- [#3310](https://github.com/scode/farhelm/pull/3310/changes) shows whole final lines of long errors, based on #3309.

## Checks run, reused and skipped

All runtime checks used `scripts/record-test-run.py`. Browser runs used the pinned tmux, three stack slots, zero retries
and both Chromium and WebKit. Their source and reports were complete, output reached EOF without truncation, and all
three stacks completed cleanup.

- `npx playwright test tests/host-updates.spec.ts tests/tooltip.spec.ts tests/tooltip-coverage.spec.ts` passed all 78
  cases, 39 per engine, with no skips. Run `000177e9-3341-4311-baa1-563c2b07438f`.
- `npx playwright test tests/provisioning.spec.ts -g 'single Update shows inline progress'` passed four cases covering
  both themes and engines. Run `f10d56d8-2a55-409f-a10d-0f322dcaf492`.
- Earlier focused browser runs passed two whole-row card cases and four long-preview cases. Runs
  `321f31ec-84ed-4dda-aad3-a71101c54f0c` and `39836597-d895-420c-a524-154bd4c44785`. Those cases are also in the 78-case
  selection above.
- `node --test` in the UI JavaScript harness passed 563 tests. Run `759beded-2d11-4e4a-b30d-873dd628c9f6`. Its
  owned-clock cases cover rest delay, row crossings, dismissal and nearer targets. Browser glyph measurements cover
  complete final lines of both connection errors and run messages on both surfaces, with and without steps.
- `cargo build -p farhelm`, the pinned `dx build --package farhelm-ui --platform web --release`,
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-spec `dprint check`,
  `python3 releasing/check-changelog.py format` and the isolated-interpreter test-sleep check passed. The sleep check
  inspected 429 delays with zero missing rationales. An initial web-build invocation named the wrong tool path and
  failed before building. Corrected pinned-tool builds passed.
- `python3 scripts/check-agents-md.py` passed with the existing archive citation warning and zero errors.

The final integration and provisioning runs covered `5f492c983fae65d9461965127b244eeffa77ecc3`. Subsequent amendments
only consolidated release notes, so the runtime results retain their coverage. The first PR's clippy and binary build
were reused for the second because it changed CSS, JavaScript, tests, specs and bookkeeping. Fresh main differed from
the stack's base only in another plan's claim, with no interaction requiring a rebase or repeat.

Wider Rust, desktop-runtime, installer, cloud and full-browser batteries were skipped because these selected checks
cover the hover and preview changes. Browser WebKit supplies engine-family coverage for the desktop renderer.

## Review gate outcome

Each PR passed its own fresh-context gpt-6.1-sol high-effort adversarial review, including the test-authoring checklist.
The release-note corrections were checked by the same reviewers. Commit messages passed fresh cold reads. No code
findings or required decisions remain open.
