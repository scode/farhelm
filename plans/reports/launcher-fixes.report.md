## What this was about

Five problems affected the session launcher. Templates that reset permissions could make an unchanged Launch retry look
like a new request, causing repeated YOLO questions or duplicate sessions after a lost successful reply. A leftover
`gh:` search could keep Launch disabled after an explicit repository selection. Clearing the checkout folder elsewhere
could leave an open launcher without its setup field. Model search and reselecting the same agent could erase unfinished
custom permission text. Keyboard navigation could highlight repository suggestions outside the visible list.

The maintainer approved small fixes with explicit limits for each problem. The five PRs preserve retry identity for an
unchanged normalized selection, retire the earlier search after dedicated repository selection, expire setup answers on
an observed configuration change, retain custom text when its agent and permission choice stay the same, and scroll the
active repository suggestion into view. Each stayed within its agreed limit.

## Things you should know

The initial retry regression assumed OMP still defaulted to YOLO. It now defaults to Guarded, so that fixture failed
before launching. The plan expressly allowed Goose instead; the corrected YOLO-default regression uses Goose, and the
guarded-default regression uses Codex. This changed the test fixture, not the permission defaults or the required retry
guarantee. The original failed run remains recorded privately. It was a test mistake found and fixed during this run,
not a latent flake.

The fixes add no protocol, persisted-state, permission-default, or template-application changes. A real agent switch
still clears custom flags. Configuration changes trigger a fresh setup check, while ordinary title edits leave a
configured repository scan idle. Keyboard scrolling retains focus on the repository field. The live installation was not
changed.

## Open questions and possible follow-ups

None required for these outcomes. No complexity gate failed and no maintainer decision is outstanding.

## The PRs

- [#2160 — unchanged launcher retries retain their request](https://github.com/scode/farhelm/pull/2160/changes).
- [#2161 — repository selection clears the earlier blocking search](https://github.com/scode/farhelm/pull/2161/changes).
- [#2162 — checkout-folder removal restores inline setup](https://github.com/scode/farhelm/pull/2162/changes).
- [#2163 — same-agent search choices preserve custom permission text](https://github.com/scode/farhelm/pull/2163/changes).
- [#2164 — keyboard repository selection stays visible](https://github.com/scode/farhelm/pull/2164/changes).

All PRs remain drafts in that linear order, with one commit per outcome. Each removes its feedback item, updates the
outcome ledger, and carries its own fixed changelog fragment.

## Checks run, reused and skipped

Recorder captures used working trees. The following final draft commits preserve the tested product and test content;
later ledger updates change only bookkeeping. The checkout-setup commit also includes a comment correction after its
tests. These are the covered revisions used for the reuse decisions below:

| Outcome              | Covered draft revision                     |
| -------------------- | ------------------------------------------ |
| Retry keys           | `af88c2860d2cffa4e3f2b658be88359ca5e81cd9` |
| Repository selection | `0aa456fb2bfb28740ba65edae0bc33207c39dc7c` |
| Checkout setup       | `b264ab34e8ba3bb16509ab165cf67f7e71709e3e` |
| Custom text          | `2863f1d1e372928c71e49a2790d3bb1b39f69881` |
| Final scrolling/tip  | `c054be6dc81fa655e662f1663747df257a396374` |

- `cargo nextest run -p farhelm-ui --lib` ran through the recorder after every outcome. Final tip run
  `72d572bf-687b-47bc-bc9e-abff9e168559` passed all 503 tests, with zero skips.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` passed for the first
  outcome and again at the final tip. `cargo clippy -p farhelm-ui --all-targets -- -D warnings` also passed after each
  intervening outcome.
- `cargo build --offline -p farhelm -p farhelm-fixtures` passed during the retry-key outcome on main
  `fdcefeca71df157d07965685c04a1655e7dd52ec` plus that outcome's working-tree edits. No CLI, fixture or shared
  dependency source changed in this plan, so that build was reused through the final tip. From the UI crate,
  `dx build --package farhelm-ui --platform web --release` passed after every outcome, including the final source state.
- Retry regression: run `6705f172-a4d4-441d-9b7c-034043606966` passed the Codex case on Chromium and WebKit and failed
  both OMP fixture premises. Corrected Goose-only run `772f2c51-5322-47bc-9438-59df609e5491` passed both engines. Reuse
  the original Codex passes because the correction touched only Goose's fixture and later fixes do not change retry
  normalization or template behavior. The failed aggregate is not counted as a successful run.
- Repository-selection regression: run `296e0822-776c-4412-ad0f-b3d64a294f0c`, two passes across Chromium and WebKit.
  Reused after later changes: configuration observation, custom-text preservation and scrolling leave search clearing
  and the preview/admission guards intact.
- Inline setup regression: run `de37ffee-4c78-4a05-9aa8-ef469be244c6`, two passes across Chromium and WebKit, including
  external folder removal, title-edit no-rescan and host-override coverage. Reused after a comment correction and later
  independent search-text/scrolling changes; setup behavior and fixture assertions remain unchanged.
- Custom-text regression: run `396c8ba3-1d39-492e-a854-f1a2d0dd1f54`, two passes across Chromium and WebKit. Reused
  after adding repository scrolling, which does not change either agent/model search handler or the custom editor.
- Repository-scroll regression: run `972ebbaf-6d4d-49e7-9914-3bbc9c3a7575`, two passes across Chromium and WebKit,
  covering both wrap directions, forward movement beyond the viewport and retained input focus.
- Browser child commands were `npx playwright test terminal-create-idempotency.spec.ts -g 'a null-permission'`, its
  corrected Goose-only selection `-g 'a null-permission goose'`,
  `npx playwright test github-checkout-composer.spec.ts -g 'destination type keeps'`,
  `npx playwright test github-checkout-composer.spec.ts -g 'inline checkout folder setup'`,
  `npx playwright test custom-permissions.spec.ts -g 'same-agent model and harness search'`, and
  `npx playwright test github-checkout-composer.spec.ts -g 'repository keyboard navigation'`. Each ran through the
  controlled Playwright recorder with one worker and zero retries.
- Passing retained runtime reports are complete with no skipped cases or truncated output. The selected cases use the
  local host; the secondary SSH fixture's connection warnings do not represent skipped required coverage.
- Rust formatting, changed Markdown formatting, changelog format, agent-instruction references, whitespace, and the
  isolated test-sleep checker passed during each outcome and at the final source state. The sleep checker inspected 302
  deliberate delays with no missing rationale.
- Broader Rust, browser, desktop, installer, provisioning and hosted CI runs were skipped: the changes share one
  launcher component, and UI units plus targeted mounted browser regressions cover their changed contracts. No new
  cross-platform, installation or server behavior requires those suites.

## Review gate outcome

The prescribed fresh Sol reviewer at high effort reviewed each PR for correctness, design, language idioms, regression
quality and its complexity limit. The first review found two fixture defects: cleanup could outrun a pending create, and
the test did not establish its stored null-permission premise. Both were fixed and rechecked. A fresh scope review after
the default-permission fixture correction found no unnecessary complexity. The remaining completed source reviews found
no actionable issues. The combined diff was assessed for interactions; none required a wider runtime suite beyond those
targeted checks.

Fresh Sol readers at medium effort checked every commit and PR's wording. Implementation remained in the executor; only
the prescribed reviews were delegated.

### Landing

Landed on its own in round 70, as five PRs (#2160 to #2164), merged bottom-up. They are unchanged from what the report
describes, apart from one changelog correction made during the landing (below).

#### What landed since the stack was based, and whether it interacted

The executor based the stack on main after rounds 68 and 69 had landed. Those rounds brought the wrapped web-link hover
fix and the browser-test cleanup work, which edited two of the same browser spec files. Between that base and the
landing, only this plan's own claim and report reached main, so the landing rebase changed nothing outside `plans/`.

I reviewed the stack against what was already on main, and a fresh-context reviewer did the same independently. Neither
found an interaction:

- The new browser tests follow the cleanup rules round 69 just established. The retry test creates its fixtures inside
  its cleanup scope and releases them through the shared cleanup helper. The inline-setup test keeps its new steps
  inside the existing cleanup. The checkout composer spec's git commands are still isolated from the caller's
  repository.
- All five PRs edit the same launcher component, and none undoes another. The retry fix (#2160) recognizes an unchanged
  retry by comparing what is submitted. The custom-flags fix (#2163) only changes when a search choice rewrites the
  flags text, so an unchanged retry still submits identical text. Choosing a repository (#2161) closes the list, so the
  new keyboard scrolling (#2164) does not fire on a selection. The setup check (#2162) does not depend on the repository
  search that #2161 clears.

#### Fix made during the landing: one changelog entry folded into its feature's entry

The custom-flags fix (#2163) repairs behavior that the custom permission flags feature (#1995) introduced. That feature
has not shipped in any release yet; the latest is v0.28.2. The changelog rules say a fix to unreleased work edits that
work's entry instead of adding a second one, since users never saw the bug. So the landing removed #2163's separate
"fixed" entry and added one sentence to the pending custom-flags entry: the flags you type, including unfinished quotes
and spacing, are kept when you pick a model through search or reselect the same agent. The other four PRs fix behavior
that has shipped, so their separate entries stay. No code changed.

#### Possible follow-ups

- A new code comment in the checkout-setup fix (#2162) says a configuration change must make the launcher check the
  setup again. That holds when a repository is selected. With none selected and the setup field showing, the reviewer
  found that the launcher keeps its earlier answer. In that case the field can stay visible after the folder was set
  from somewhere else. This is not a regression, since before #2162 the launcher never re-checked in that case either.
  But the comment claims more than the code does, and the remaining gap may be worth a small fix. The report's "Things
  you should know" states that configuration changes trigger a fresh setup check; that holds only while a repository is
  selected.
- SPEC_impl.md describes the launcher's remembered setup answer as tied to the host connection and installation. It now
  also expires when the launcher sees the configuration change. That does not contradict the spec, but one clause would
  keep it exact.

#### Checks

The executor ran the launcher's unit tests, clippy and the web build at the stack tip, but ran each targeted browser
regression only at its own PR's revision. All five PRs change one component, so the combined result needed a browser run
of its own before landing.

- Run now, at the rebased tip, after fresh `cargo build` and web release builds: the three browser spec files the stack
  touches, in full, on Chromium and WebKit, through the test recorder with the pinned tmux, one worker and no retries:
  `npx playwright test terminal-create-idempotency.spec.ts github-checkout-composer.spec.ts custom-permissions.spec.ts`.
  70 of 70 passed with no skips, run `fd816c03-cc60-40f9-8ea3-6992dfa3e017`. That covers all five regressions and every
  other case in those three files, together.
- Run now: `cargo fmt --all -- --check`, and `python3 releasing/check-changelog.py format` after the fragment change,
  both clean.
- Reused: the executor's final-tip UI unit run `72d572bf-687b-47bc-bc9e-abff9e168559`, its clippy runs and its
  test-sleep, Markdown formatting and agent-instruction checks. They covered the report's final tip (`c054be6d`). The
  landing rebase crossed only `plans/` bookkeeping and the landing changed only a changelog fragment, so the code they
  tested is what landed.
- Skipped: broader Rust, browser, desktop, installer, provisioning and hosted CI runs, for the reasons the report gives.

The report's PR list says each PR adds a changelog fragment; #2163 no longer does, for the reason above. Apart from
that, and the setup-check qualification under "Possible follow-ups", nothing in the executor's report became untrue at
landing.
