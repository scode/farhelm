# Launch refusals and early folder warnings

## What this was about

Launching into a missing folder already failed safely, but the launcher showed the refusal as a small red line that was
easy to miss. The maintainer chose a bordered alert beside Launch, a warning on the folder field when the refusal
concerns that folder, and an advisory folder check before Launch.

The launcher now announces “Not launched.” followed by the original refusal. New, Clone, Replace with and quick launch
use the same presentation. A folder refusal also marks the folder field and names the selected host. Changing the
folder, host or connection removes the old marking. Saving a template is kept separate and never labelled as a failed
launch.

## Things you should know

For an ordinary folder on a connected host, the launcher checks after editing settles for 150 ms. A missing folder or a
path that is not a directory gets an early warning. Launch remains usable: the supervisor still decides at launch time.
Permission failures, timeouts and other inconclusive answers stay silent. Results for an old destination are discarded,
and only one automatic check can be outstanding per launcher. Managed checkouts and disconnected hosts are not checked.

The check uses the existing directory-check operation without listing children or updating folder history. It shares the
supervisor’s two browse workers: a hung mount can occupy them and delay manual Browse on that host until the work
returns. This limitation is documented in the implementation specification.

The alert and field warning fit the tested 390-pixel phone layout. Farhelm currently has a dark palette only; both
operating-system light and dark preferences were exercised, but this work does not add a light theme. The styles use the
existing semantic colors. Restart with remains a separate dialog; its queued follow-up will bring it into the shared
launcher.

## Open questions and possible follow-ups

No decision is needed to finish this plan. The existing browse-worker limitation remains. The separate Restart with plan
can inherit these warnings once it adopts the launcher.

## The PRs

- [#2675](https://github.com/scode/farhelm/pull/2675/changes): prominent launch refusals and destination-specific folder
  marking.
- [#2677](https://github.com/scode/farhelm/pull/2677/changes): an opt-in directory check that neither lists children nor
  refines history.
- [#2678](https://github.com/scode/farhelm/pull/2678/changes): advisory automatic folder warnings and removal of the
  addressed TODO entry.

All three are draft PRs in one linear stack. The executor has not merged them.

## Checks run, reused and skipped

The following checks passed during this execution, before the four-hour pause:

- UI matcher:
  `cargo nextest run -p farhelm-ui --lib -E 'test(=list::create_form::tests::folder_refusals_are_recognized_by_the_supervisor_words)'`,
  one test passed, run `2b1ea8b2-29ce-41c3-9f5c-2c20dfc370a0`. This covers the fixed supervisor refusal phrases.
- Helm route: three selected `sessions::tests` passed, run `775dfde7-b01d-4ae0-9e66-6d85d7cf958e`:
  `folder_checks_do_not_list_or_refine_history`, `browse_directory_routes_to_the_named_host_and_preserves_its_reply`,
  and `a_browse_prepared_against_a_replaced_connection_reaches_no_supervisor`. These cover the new mode, ordinary Browse
  routing, history preservation and stale-connection refusal.
- Browser: run `91bc65d0-801f-4edf-ad0c-87bac84fba0c`, 26 passed (13 Chromium, 13 WebKit), selected from
  `launch-refusal.spec.ts`, `sidebar.spec.ts` and `destination-authority.spec.ts`. This covers missing and existing
  folders, advisory Launch, stale replies, serialized checks, inconclusive answers, asynchronous launch preparation
  failures, template-save wording, and affected manual Browse, clone-generation and phone-layout behavior. The recorder
  retained complete source and runner evidence. The tested dirty tree was based on `230fadd9` and has the implementation
  committed as `c190f7f9`.
- `cargo fmt --all -- --check`; native `cargo clippy --all-targets -- -D warnings`; web
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`; native and web builds.
  These cover formatting, both compiled UI configurations and the helm changes.
- The isolated `scripts/check-test-sleeps.py` check found no missing delay rationales among 315 delays. Dprint passed
  for the touched specifications, TODO and fragments; TypeScript is excluded by this repository’s dprint configuration.
  `python3 releasing/check-changelog.py format` passed for 128 fragments.

These results are reused after the finishing rebases. Every intervening main diff was inspected: only unrelated TODO
prose and other plans’ queue decisions changed, so no executable behavior or tested contract changed. No runtime tests
were repeated merely for rewritten commit IDs.

The whole Rust and browser batteries, installer, provisioning and live desktop checks were skipped: the selected unit,
route and two-engine browser tests cover the changed shared launcher and read-only route. No installer, provisioning,
wire protocol or desktop-only behavior changed. This is targeted evidence, not a whole-suite verdict. The maintainer’s
live installation was not touched.

## Review gate outcome

Each PR passed an independent adversarial review by gpt-6.1-sol at high effort. Findings were corrected and checked
again: managed-checkout mode clears the dormant folder’s warning; asynchronous preparation failures retain the prominent
refusal; the inconclusive-answer test waits for the intended request; and the removed TODO’s plan link is removed too.
Follow-up reviews reported no findings. Commit and PR wording passed fresh cold reads. A fresh resume audit reconciled
the saved state after the pause, and this report received the required independent cold read before delivery.
