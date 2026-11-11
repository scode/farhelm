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

### Landing

#### What landed with it

This plan landed in the same round as tensorlake-key-order, which changes the SSH options Farhelm uses for Tensorlake
connections. The two touch separate code and separate sections of the specs, and nothing else besides plan bookkeeping
and a TODO.md entry (#3313) reached main while they were in flight. A second, independent review of the two diffs
together found no interaction.

#### Fixes made while landing

None.

#### Minor behavior the landing review noted

These do not need a decision, and none blocked the landing:

- In a very short browser window, when two long messages share the hover card, one message's share of the space can be
  less than one line. That message then shows no text at all, only its heading and `Full details in Hosts`. Before this
  change it showed a sliver of a cut-off line, which was not readable either.
- If a host finishes its work while the card is showing and the pointer is resting on the row's progress bar or padding,
  the card can stay up until the pointer moves onto another element. This is new with this change, since before it only
  the host's name opened the card.

#### Checks

These checks ran on the round's combined tree: host-progress-card-fixes with tensorlake-key-order stacked above it, on
main before either landed. Test runs went through the recorder with the pinned tmux first on `PATH`, and none printed a
runtime `SKIPPED` line. Since then main has gained only a TODO.md entry (#3313), so what merged is the tested tree.

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and
  `cargo clippy -p farhelm-ui -p farhelm-helm --all-targets -- -D warnings` passed.
- `node --test` in `crates/farhelm-ui/js-tests` passed with no failures.
- Browser: `npx playwright test --project=<engine>-<spec> --repeat-each=3 --retries=0` for `host-updates`, `tooltip` and
  `tooltip-coverage` on Chromium and WebKit ran 234 tests, and all passed. They ran on Tensorlake sandboxes at the
  maintainer's request, one project per sandbox. The sandboxes run Node 24.16 against 24.20 locally. Run ids:
  `2d4b9620-510e-4d01-ae1a-205dceb4db55`, `8008bcd0-4d70-4904-b694-943ec79afae2`,
  `ae9ca261-c478-4d0d-90f0-939975240076`, `a662159e-e9d3-411f-b958-f73dc54f6d48`, `58770221-2996-452b-9453-ee93b6fd02fe`
  and `5e49b5ad-f42a-46b3-b6c3-4a950e3d09a4`.
- Rust: the seven focused Tensorlake SSH option tests that tensorlake-key-order's executor ran passed again (run
  `f57c4baa-0379-4214-a338-534766140639`). So did every test in the helm's `tensorlake::`, `provisioning::`, `ssh::` and
  `transport::` modules (`cargo nextest run -p farhelm-helm --lib -E <filter>`: 515 passed, run
  `0dcbabeb-54bd-4c29-9f1a-3efce5acdc88`), which covers each caller that builds the Tensorlake ssh options.

Reused from host-progress-card-fixes' executor: its browser run of the provisioning spec's
`single Update shows inline progress` test on both engines, which the hover change does not reach beyond what the
repeated runs above cover.

Skipped: the rest of the browser suite and the workspace's other Rust tests, because neither change reaches beyond the
sidebar's hover card and the Tensorlake ssh options. Also skipped: a live Tensorlake gateway, an OpenSSH 7.6–7.9 client
(the executor checked those from upstream source), and hosted CI (manual-only, not dispatched).
