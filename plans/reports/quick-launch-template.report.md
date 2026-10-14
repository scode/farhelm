### What this was about

Starting the same agent setup repeatedly should take one action. Templates now have a ⚡ mark: Cmd+P on a Mac, or
Ctrl+Shift+P elsewhere, starts a session from the marked template and selects it without showing the session launcher on
success. A separate Quick launch row in the quick switcher does the same thing. Existing template picks in the switcher
still open New for you to finish.

Quick launch uses New's defaults and launch path, including the open session's host and directory and its remembered
choices. A refusal or YOLO question reveals that same draft for you to finish. With no marked template, the shortcut
opens Templates with guidance to mark one. Missing or edited prerequisites leave a visible way to complete or cancel the
setup. The quick-agent TODO entry is removed.

### Things you should know

The mark belongs to the helm and allows one template at a time. Moving it in another window affects the next shortcut
use. An explicit local choice whose save is still pending or has failed remains in force in that window, following the
existing preference policy. Renaming reads the shared mark afresh and moves a matching mark before removing the old
template name; a failed repoint retains both names and explains the partial rename. Delete clears only a matching mark,
including deletion through the API or an agent. Undo restores the template, without marking it again.

Simultaneous changes from different windows keep the existing last-writer-wins behavior. The single preference ensures
there is never more than one mark; the multi-request rename does not promise to preserve every overlapping mark change.

As approved in the plan, marking a template that makes a managed checkout is advance consent to create its directory
without first displaying the path. The ordinary preview still runs. Losing the previewed path reveals the conflict and
new preview and requires a deliberate launch; the shortcut never retries itself. YOLO confirmation remains mandatory
where the host asks for it. An intervening dialog or another launcher opening retires an older held quick-launch read.
Hidden preparation isolates background keyboard input and never focuses hidden controls.

The new preference upgrades the helm database from schema 48 to 49. An older helm refuses that upgraded database; the
maintainer accepted this downgrade limitation. Firefox may reserve Ctrl+Shift+P for a private window, so the docs point
to the quick-switcher row. Chromium and WebKit were exercised; no Firefox or native desktop shortcut claim comes from
those browser tests.

The docs pages for launch templates, starting sessions and the session list explain the feature. No website deployment
was triggered.

### Open questions and possible follow-ups

No maintainer decision is required to finish this plan. Refresh the docs screenshots after landing: the screenshot
publisher captures main's app, so existing published screenshots remain until the new controls are on main. Native
desktop keyboard integration remains additional validation; WebKit here covers the web engine family, not the desktop
shell. Number shortcuts and configurable shortcuts remain outside this plan.

### The PRs

- [Draft PR #1994](https://github.com/scode/farhelm/pull/1994/changes): quick-launch mark, direct launch, fallbacks,
  tests, specifications and docs together.

### Checks run, reused and skipped

The review unit is based on main `831d65fc`. A final fetch found no intervening main changes, so no rebase interaction
needed repair. Validation was selected for preference durability, launch ordering, modal/focus ownership and browser
integration rather than running the whole repository battery. Counts below belong to individual runs; they are not an
aggregate denominator.

Run on the final behavior:

- `dx build --package farhelm-ui --platform web --release` passed, including the final reveal-only focus handoff.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the source delay checker with
  its isolated interpreter, changed-document `dprint check`, and `python3 releasing/check-changelog.py format` passed.
- `bun run build` passed for all 32 website pages after the final managed-checkout consent wording.
- Recorder-backed Playwright on Chromium and WebKit: held/failed local mark writes, closed Rename, host-read keyboard
  cancellation and manual recovery passed all 10 cases total across the two engines in
  `5f324a71-a305-4127-8c78-68734fe0ebe8`.
- Ordinary immediate focus and deferred quick reveal passed all 10 cases total across the two engines in
  `1cbd0a4c-9a04-4613-97e3-aaa5612b05d0`, including the previously failing WebKit model-field case.

Reused successful evidence, because the corresponding implementation remained unchanged:

- Recorder-backed nextest: backend mark, sparse persistence, deletion/Undo and genuine 48-to-49 migration passed 8/8 in
  `54c61795-b3d6-48e8-a90b-f959d36d3830`. UI preference ordering, acknowledgments, cancellation, unsaved mark
  preservation, modal helpers and eligibility passed 16/16 in `44f34557-b627-4715-8e97-318df7cdf8ab`.
- `cargo clippy --all-targets -- -D warnings` passed for the workspace before the final focus-helper adjustment; the
  final UI-only Clippy run covers that adjustment.
- Playwright managed quick-launch preview and lost-path-race tests passed 4/4 in `2b0a8e66-45f9-4252-a898-29597faa2084`;
  ordinary template/switcher/managed interactions passed 8/8 in `2da9b0b1-6c76-4a0e-826b-3ee691985687`. The broader
  template/switcher selection had 62 passing cases and two inherited expectations for the previous permission label;
  correcting those expectations passed both cases in `9e0d18bd-c2fc-4618-acb2-9ef18acd60bb`.
- In `0e2e1f72-8f1c-4af4-8f10-88be9c2c4fec`, 21 browser cases passed for ordinary selections and quick success, terminal
  input ownership, queued replacement, switcher, YOLO and refusal handoffs. One WebKit ordinary-focus case failed after
  deferring every focus handoff; delay is now limited to quick reveal, and that case is retried in the final selection.

Failed observations remain retained privately. Early Rust runs refused on container setup, a missing lockfile edge or
unupdated test constructors before test execution. Browser fixtures were corrected for exact renamed labels, connected
host identity, superseding preview requests, the replacement endpoint and the real helm build stamp. Run
`3b0b8622-ab13-4923-9224-5e7a058cb366` failed with missing pinned tmux in the launch environment and also exposed the
pre-render loading focus handoff; the corrected launch and handoff passed separately. These same-session corrections are
not treated as successful runs or latent flakes.

Skipped: the full workspace runtime and browser batteries, desktop runtime/installer/release gates and hosted CI. The
focused storage/queue and two-engine browser evidence covers the changed contracts; this plan changes no installer,
provisioning or release pipeline. Native desktop runtime remains a stated coverage limit. Screenshot publication and
Vercel deployment were not part of this unattended plan.

### Review gate outcome

The plan's sole gpt-6.1-sol high correctness reviewer passed the final snapshot with no remaining concrete findings. It
reported six initial issues and two follow-up issues; the implementation and focused tests were corrected. A final
bounded verification also passed the keyboard-focus correction found during browser validation. Scope reassessment
retained the existing preference writer and ordinary New, and rejected an atomic conditional rename API as a stronger
concurrency contract than the approved plan. Commit and PR wording passed the required three-round cold-reader process.
Runtime results were collected by the executor; the correctness reviewer performed static inspection and did not rerun
them. The independent report cold read passed decision sufficiency and public-repository hygiene.

### Landing

Landed alone in its round as #1994. Since the stack was based, main had gained only queue bookkeeping. The plan already
built on the guarded permission mode (#1992) and the confirmation focus and skip work (#1990, #1993), both landed
earlier the same day.

#### Release-relevant

The helm's database moves from schema 48 to 49 (the quick launch mark). As with every schema bump, a helm that has run
this build cannot be downgraded in place. The helm–supervisor protocol version is unchanged.

#### Review

An independent fresh-context review found nothing blocking. It confirmed that:

- a quick launch goes through New's ordinary submit, so the host still decides whether to ask the YOLO question, and
  nothing answers it for the user;
- a managed checkout submits once and never retries a lost path race;
- one key press launches at most once, with key repeat and composing input filtered;
- the shortcut never reaches a terminal program as well;
- a template delete clears only a matching mark, in the same transaction as the delete.

#### Fixes made while landing

- A marked template later edited, through the API say, so that it chooses neither an agent nor a command showed a lit ⚡
  that was disabled. Clicking the lit bolt is the only way to clear a mark in the UI, so the mark could not be turned
  off short of deleting the template. A lit bolt now stays clickable, and its tooltip says it turns quick launch off. A
  new browser test covers it ("a marked template that became ineligible can still be unmarked").
- The helm's package-wide tests had not been run against this change. Four failed: each builds an older database by
  dropping newer columns from a fresh one, and none dropped the new mark column. All twelve such setups now drop it.
  Failed run `d82d6aba-ac04-49b2-b030-2435d56ae3fd` is kept.
- The managed-checkout browser spec marks a template without resetting the helm-wide mark after each test. A test
  abandoned before its `finally` would leak the mark into every later spec, as happened with another preference in the
  previous round. It now resets the mark after every test.
- The test of the shortcut beside other dialogs pressed the shortcut while a just-closed dialog could still be rendering
  out. That dialog could then swallow the press and pass for the guard being tested. It now waits for each dialog to go
  first.
- The UI crate's Cargo.toml was not in the order `dprint` requires.
- Two expectations in the managed-checkout browser spec had been left stale by the previous two landings, which had not
  run that spec. They now expect what the app does:
  - a Replace with new choices stores Claude's guarded request default (from #1992);
  - the trash's permanent-deletion confirmation focuses its confirming button, so Enter answers and Escape still cancels
    (from #1990).

#### Review notes left as they are

- While a quick launch is preparing or its create is in flight, the window shows nothing and ignores clicks and keys.
  With a managed checkout clone that can last tens of seconds and look like a freeze. A small "starting <template>…"
  status while hidden would help; that is a design choice, so it is noted here rather than added.
- In the quick switcher, with no sessions to list (or after a failed session read), the Quick launch row is the only row
  and starts selected. Then opening the switcher and pressing Enter launches at once. Never selecting that row initially
  is a possible follow-up.
- After pressing the shortcut with no mark, then later saving a template from New, the Templates hint still says to mark
  a template.
- A revealed quick-launch fallback keeps the header's New toggle inert, unlike ordinary New.
- One SPEC_impl.md sentence calls the delete transaction "existing" although this change introduced it. The Terminal
  experience section could also list the quick launch chord among those never sent to a program.
- `futures-channel` is declared directly rather than through the workspace.
- Unverified: whether the native Mac app delivers Cmd+P to the page. The report already states this limit.

#### Checks

- Run now, on the final tree:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog format lint and the sleep checker (298 delays, none
    missing a reason);
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
    `cargo check -p farhelm-ui --features desktop`;
  - the helm, UI and protocol packages in full through the recorder (pinned tmux, four slots, no retries): 1693 passed,
    none skipped, run `e02a3bde-159f-4971-9e18-3e616c71ea25`;
  - the asset JavaScript harness (241 passed) and the website build;
  - browser specs on Chromium and WebKit (quick launch, quick switcher, templates, managed checkouts, YOLO guard,
    settings, session confirmations): 180 passed and 4 failed, run `46f0a36d-bc0a-45d4-a38e-771bb145e00f`, kept. The
    four failures were the two stale managed-checkout expectations on each engine;
  - after correcting them, the managed-checkout spec in full on both engines: 32 passed, run
    `b94029fc-398e-4662-9f74-a1dbb035b357`.
- Stopped on purpose: run `80a39866-c593-4633-818b-6c8d6c6d2fdf`, while the review fixes were edited in.
- Not run: full workspace and full browser batteries, desktop runtime, installer and release gates. This plan changes
  none of those paths.
