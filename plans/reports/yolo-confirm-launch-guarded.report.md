# Start guarded from the YOLO question

## What this was about

Choosing the agent's safety checks after a YOLO confirmation used to require canceling, changing the permission and
launching again. The session launcher and Restart with now offer **start guarded session instead** when the refused
launch explicitly chose YOLO and that agent offers guarded mode.

The answer is first and blue, while Cancel keeps initial focus. It changes the same permission choice as the ordinary
row and starts the launch without YOLO consent or changing whether the host asks. A canceled or older question cannot
change the draft. The launcher treats it as a new launch and remembers guarded after success through its ordinary launch
path. Plain Replace and plain Restart retain their existing behavior.

## Things you should know

- Managed checkouts still require a fresh preview after the permission changes. The answer waits for it; editing or
  closing the launcher cancels the pending automatic launch. A failed request is not automatically retried.
- Restart with hides the guarded answer when it would merely restore the stored launch with no other change. A named
  conversation remains part of the edit.
- The **Start a session** page and its screenshot alt text were updated. Its `start-a-session/yolo-confirmation`
  screenshot is intentionally stale: this plan expressly excluded recapture.
- The launcher diff includes a mechanical move of its existing submission code into one shared entry point. This lets
  the guarded answer submit immediately after checking the draft, without a deferred desktop event arriving after an
  edit. Another delivered plan, UI session actions (#2496), prevents session actions from overlapping and changes the
  same submission code; the landing monitor must preserve that protection when reconciling the move. This PR is based
  directly on main and contains none of that plan's work.

## Open questions and possible follow-ups

No decision is required. A separately requested docs-screenshot refresh can update the image named above.

## The PRs

- [#2506](https://github.com/scode/farhelm/pull/2506/changes): offer the guarded answer in the launcher and Restart
  with, with tests, specification, docs, changelog fragment and the covered TODO entry removed. Open draft, head
  `00899518`.

## Checks run, reused and skipped

Runtime checks used `scripts/record-test-run.py`. Rust used pinned nextest 0.9.148, four slots and zero retries; these
UI units use no tmux. Browser checks used the pinned tmux build, three configured workers with one private stack each,
both Chromium and WebKit, and zero retries.

- `cargo nextest run -p farhelm-ui --lib`: **592 passed**, zero skipped, in `639446f8-0448-4d27-8374-2aaa7c064075`. The
  earlier focused three-test selection also passed in `75548dd3-1aeb-4e55-b045-9c84fae283ae`; the full run covers the
  completed launcher and Restart with code.
- `npx playwright test tests/github-checkout-composer.spec.ts -g 'guarded checkout answer'`: **6 passed**, three per
  engine, in `3ed522bd-8590-47c4-b66c-a4ee04f3fe39`. These hold the refreshed preview and exercise automatic submission,
  editing and closing.
- `npx playwright test tests/yolo-guard.spec.ts tests/restart-with.spec.ts tests/quick-launch.spec.ts tests/custom-permissions.spec.ts tests/replace.spec.ts tests/launcher-enter.spec.ts tests/terminal-create-idempotency.spec.ts tests/terminal-restart.spec.ts`:
  **318 passed**, 159 per engine, in `70e7f8c9-dd71-49ba-9fc5-6e6e44a16817`, on final head `00899518`. This covers the
  two changed question flows and every required existing consumer of the YOLO-answer selector.
- The first eight-spec command, `f2c96bf3-d821-4a7d-b9a1-9cfad4beeea3`, had 316 passing cases and two failures: the new
  success test expected a visual changed-value marker that the launcher never renders, including for a manual permission
  choice. That marker belongs to Restart with. The test now observes the selected permission and held request instead;
  the corrected case passed on both engines in `1c49df48-e588-43d4-95fa-abdbd1dc6d0b`. The failed record is retained and
  is not counted as a passing command. The complete required selection then passed as recorded above.
- `cargo clippy --all-targets -- -D warnings`, strict web-target UI Clippy, `cargo fmt --all -- --check`,
  `git diff --check`, Markdown formatting checks and `python3 releasing/check-changelog.py format` passed. The isolated
  `python -B scripts/check-test-sleeps.py` passed on the final source: 307 delays inspected, none unexplained.
- CLI and browser fixture builds passed, followed by the explicit Dioxus 0.7.10 release web build. The website build
  passed with 32 pages and all internal links valid; its evidence is reused after a later alt-text-only edit, which
  changes no markup structure or link target.

The production code remains identical to reviewed and built `9a1ed849`. The final `00899518` also includes the test-only
correction above. The successful UI-unit and managed-checkout commands, builds and lints are reused because their code
and inputs did not change; the TODO removal does not affect their executable coverage. Successful completed runtime
records have complete source and reports, observed output EOF, no runtime `SKIPPED` marker, no truncation and no forced
cleanup.

No workspace-wide runtime battery, full browser suite, desktop shell runtime, installer, provisioning-container or
unrelated JavaScript tests were run: the changes are confined to these UI submission paths, covered by the UI units and
selected browser cases. No instruction, heading or file-location change required the agent-instructions checker. The
TypeScript dprint attempt was inapplicable because this repository has no TypeScript formatter plugin; it is not counted
as a pass. CI is manual-only and was not dispatched. Screenshot capture and a preview server were excluded by the
unattended flow.

The complete upstream diff from the base through `7ffa1736` contained only other plans' reports and queue state. It
changes no product code, specification or dependency this work relies on, so no rebase or additional runtime checks were
needed for that movement.

## Review gate outcome

The single fresh **gpt-6.1-sol/high** general review initially found no actionable issue. Runtime validation then
exposed the incorrect visual-marker expectation described above; the same reviewer accepted its correction and confirmed
the remaining transition, request and memory assertions. Product code did not change. Review coverage was static; the
runtime evidence above belongs to this executor. A separate scope review approved the bounded preview wait and shared
submit path. Commit and PR wording passed a fresh cold read. No finding was rejected and no maintainer decision was
needed.

### Landing

Landed as #2506, with the same behavior the report describes, rebased onto this round's main, with one substantial
conflict resolution in the session launcher described below. It was the fourth of five plans in this landing round,
after ui-session-actions, connection-stall-phase and never-started-launch-cause; host-changed-update-warning followed
it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained helm-ingress-types (typed request parsing in the helm, and tighter rules on how the
helm reports errors that came from a supervisor) and create-reservation-retention (a create's retry key is freed when
its session is deleted). Neither changes the launcher, Restart with or how the UI reads a YOLO refusal. A fresh-context
reviewer and I read them, and the other four plans of this round, against this plan.

- ui-session-actions, the first plan of this round, changed the launcher's submit code in place: every manual release of
  the operation lock became a guard drop, and the refusal flags became typed outcomes. This plan had moved that whole
  submit code into one shared entry point, so the two collided; see the fix below.
- With that merged, the guarded answer follows ui-session-actions' rules: in the launcher it applies the permission
  change first and then goes through the shared entry point, which takes the operation lock in the same step; while a
  managed checkout's fresh preview is pending, no lock is held. Restart with's guarded answer uses its existing
  lock-holding path and the typed restart refusal check.
- Plain Replace's YOLO question, now shared between the sidebar and header by ui-session-actions, offers no guarded
  answer, as this plan's SPEC.md change says.
- connection-stall-phase, never-started-launch-cause and host-changed-update-warning do not touch the launcher or YOLO
  questions.

#### Fixes made during the landing

- The launcher's submit code: this plan's report describes moving it as mechanical, and ui-session-actions changed the
  same code in place. The landing merged ui-session-actions' changes into the moved copy (a three-way merge of the old
  code before and after ui-session-actions against the moved copy), and the launcher's submit now simply calls that
  shared entry point. The reviewer compared the result with ui-session-actions' version of that code: the two are
  identical apart from formatting. Nothing of either plan was dropped or duplicated, and no manual lock release remains.
- Its TODO.md removal conflicted with neighboring changes on main; the landing kept both sides.

#### Things you should know

- A launch carries a retry key, so that pressing Launch again after a lost reply repeats the same request instead of
  starting a second session. Choosing "start guarded session instead" drops the refused request's key, because the
  permission changed. That matters in one rare sequence: a YOLO launch is accepted but its reply is lost, the host is
  then switched to ask before YOLO launches, and the retry is refused with the YOLO question. Answering it with the
  guarded choice then starts a second, guarded session beside the first. Changing the permission by hand and pressing
  Launch already behaves the same way, and SPEC.md allows it; the new answer is a shortcut to that path, not a new risk.

#### Checks

Together the five plans change the session launcher and Replace in the UI, the helm's connection attempts and host
updates, and the supervisor's launch startup and terminal handling, about 8,900 added and 2,200 removed lines, including
changes to five browser spec files; that reach is why this round ran the whole Rust workspace and the full browser
suite. All of the following ran on the five plans stacked together on this round's main, including the landing's
conflict resolutions:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the website build (a docs page
  changed), the debug build, the release web build of the UI and `scripts/check-desktop-assets.sh` (which also builds
  the desktop app): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `3907499f-e2bc-4ba3-9a83-ce6526597d54`: 4078 passed, none failed; 40 tests
  were left out by the suite's own filters. Its retained output is complete and has no runtime `SKIPPED` message, so the
  real systemd-scope and SSH tests these plans rely on ran.
- `cargo nextest run -p farhelm-ui --features desktop`, run `63f71d93-80d2-464c-841e-4d7823a61c33`: 773 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `e320936a-a430-48b7-b45b-e8a5b2ef6f57`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `3157847c-bee9-4df2-b327-1137c87e135f`: 2270 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), none failed or flaky; the only
  runtime `SKIPPED` messages are those WebKit clipboard cases.
- Skipped: the workspace and desktop doctests (no documentation example changed), the desktop smoke test (the desktop
  app crate is unchanged; the UI's desktop feature is covered by the check and tests above), the native Mac app (not
  runnable here) and the installer (unchanged).

The report's checks ran on a main from before helm-ingress-types and create-reservation-retention landed and without
ui-session-actions, and it ran selected browser specs only; the full Rust runtime suite and the full browser suite ran
this round, above, on the merged launcher code. The report's description of the submit change as a plain move is no
longer the whole story: the moved code now also carries ui-session-actions' changes. The report's note that the
`start-a-session/yolo-confirmation` docs screenshot is intentionally stale still holds.
