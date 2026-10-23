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

### Landing

Landed as #2675, #2677 and #2678, merged bottom-up, with the content the report describes and one style fix to #2675
described below. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained only queue bookkeeping and a TODO.md edit. The base already held the recent launcher
work: per-host-last-folder (the launcher remembers the last directory per host), session-view-fixes-oct22 (the YOLO
launch question says an earlier attempt "may already have started" when a reply was lost), and alias-in-add-host (an
alias field when adding a host). A fresh-context reviewer and I read the plan against them:

- The folder check is the existing authenticated folder-browse request with a new flag that makes it return only whether
  the folder exists. It goes through the same host routing and connection check as Browse and does not add to the folder
  history.
- The launcher's code and tests that landed are byte for byte what the executor tested, apart from the style fix below.
- dialog-saves-oct22 (saving in the Templates and other dialogs), which an executor is rebuilding after your answer,
  touches none of these files.

#### Fixes made during the landing

- #2675's new alert style used a literal corner radius, which the stylesheet's rule against literals outside its token
  list forbids; the asset script tests caught it, which the executor had not run. The landing switched it to the panel
  radius token (4 pixels instead of 6), in #2675.

#### Things you should know

- "Not launched." also appears when the launcher cannot know whether the launch happened: a reply that was lost or timed
  out, a success the app could not read, or an error reply that does not carry the helm's confirmation that nothing was
  created (which today includes sign-in refusals, an unavailable host and some of the helm's own refusals). Such a
  create may have started a session; pressing Launch again unchanged is safe, because the launcher then reuses the same
  request and the helm does not start a second session, and the YOLO question already says an earlier attempt may have
  started. The label then invites changing the form and launching again, which starts a second agent if the first create
  landed. The cases where a session was in fact created are rare (they need a lost or unreadable reply). I tried a
  landing fix that left the label out when the outcome is unknown, but an independent review of it found two more paths
  that get the label wrong: launches into a new managed checkout report every failure as unknown, so a refusal the helm
  settled would lose its label, and a failure that arrives after the form has moved on to a different setup still says
  "Not launched. a launch for an earlier setup was refused" whatever its outcome (a quick-launch browser test expects
  that text). Fixing all of it changes more of the launcher and its tests than a landing fix should, so I reverted it
  and the plan landed as built. See the first follow-up.

#### Possible follow-ups

- Found at landing: show "Not launched." only for refusals the helm settled and launches that were never sent, and
  something else (or no label) when the outcome is unknown. That needs managed-checkout launches to say whether the helm
  settled their refusal, the "earlier setup" message to follow the outcome (and say "did not complete" rather than "was
  refused" when unknown), and SPEC.md's "every refused launch" sentence to match. Note that "settled" is narrower than
  "refused": the helm attaches its proof that nothing was created only to some refusals, not to its own refusals of a
  launch, a host it cannot reach, or a sign-in refusal, so under this rule those would lose the label too; keeping it
  there means teaching the helm to prove more refusals, a larger change. The wording for the unknown case is yours to
  choose. Options: leave the label out there (simplest, but those everyday helm refusals lose it too unless the helm
  proves more of them), or show a different lead such as "The launch may have started." there. I recommend leaving it as
  landed until a user meets the lost-reply case, since a second launch needs a lost reply and then a changed form; if
  you want it fixed, the different lead is clearer than no label.
- Found at landing: right after you switch hosts in the launcher, the folder check can run on the previous host's
  directory before the new host's remembered directory arrives, and briefly warn that it does not exist on the new host.
  Fixing it means making the check wait like Launch already does; that reorders launcher code, so I left it.
- Found at landing, before the next docs screenshot refresh: the screenshot stack's invented directories do not exist,
  so the new warning will appear under the folder field in the "Start a session" shots unless the staging answers the
  check as an existing directory.
- Found at landing: the "Start a session" docs page does not mention the warning under the folder field or that it never
  blocks Launch.
- Found at landing, minor: a code comment and one browser test treat "not usable" as a permission problem, but the host
  only says "not usable" for a path through a file or an invalid path; a real permission error has other words, which
  the check already ignores. The new SPEC_impl.md paragraphs also sit under the unrelated "Composer history" heading.
- For release curation: the separate changelog entry for the folder check's request flag (`kind: none`) describes the
  launcher's folder warning as upcoming, which is now in the plan's main entry; it can be dropped.

#### Checks

The executor's checks were selected tests on an uncommitted tree, reused after a long pause and rebases, and did not
include the GUI's asset script tests. So this round ran, on the plan as delivered (the code that landed, apart from the
corner-radius fix):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the workspace debug build, the
  UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- Through the test recorder with pinned nextest and tmux, four slots, no retries: `cargo nextest run -p farhelm-helm`,
  run `96283103-6e4a-4802-96d3-5f587857e0b7`, 1302 passed; `cargo nextest run -p farhelm-ui`, run
  `d426ee4a-b1f0-4efd-83bf-7ebd8becd02d`, 755 passed; `cargo nextest run -p farhelm-ui --features desktop` (without
  tmux, which those tests do not use), run `19eac98e-e985-4fd3-907f-29b27ad2464a`, 920 passed.
- The asset script tests (the GUI's JavaScript and stylesheet rules), `node --test` in `crates/farhelm-ui/js-tests`, run
  `b4114fac-0c02-4267-a5d3-d9d8062a16b6`: 332 passed, 1 failed, the stylesheet token check on the new alert's literal
  corner radius.
- `npx playwright test` in `e2e/` on Chromium and WebKit for the four spec files the plan changes and six more launcher
  specs (`launch-refusal`, `destination-authority`, `quick-launch`, `sidebar`, `yolo-guard`, `replace`, `clone`,
  `terminal-create-idempotency`, `permission-switch`, `restart-with`), three workers each with its own helm and
  supervisors, no retries, run `54971f83-c30d-4e8e-9c0a-15d4c4361bd8`: 564 passed.

On the landed tree, which differs from the above only in the corner radius: the asset script tests, run
`740a89d4-e364-4ae3-8f53-6f41516803c3`, 333 passed. A landing fix to the "Not launched." label was tested and then
reverted (see "Things you should know"); its runs describe code that did not land.

Reused: the Rust, browser and build results above for the landed tree, since a corner radius in the stylesheet changes
none of what they check. Skipped: the rest of the Rust workspace (only the helm and the GUI changed), the rest of the
browser suite (the launcher specs above cover the changed code), the doctests (no documentation example changed), the
desktop smoke test, the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were selected tests run before a long pause and reused after rebases; this round ran the helm's and
the GUI's whole test suites, the asset script tests and ten launcher browser spec files on the plan. Nothing in the
executor's report became untrue at landing.
