## Since the last review

The earlier round stopped at the number-key fix's size limit. Your answer chose the smaller fix: use uncertainty wording
after any failed save and read the keys again, keeping the shared preference save queue unchanged. That fix is now
#2679, inserted between #2658 and #2661. The other four PRs were rebased without product or test changes; only their
feedback-index conflicts needed resolution. No PR was dropped.

## What this was about

These five fixes concern dialogs that delete or save things. In the trash, a click could permanently delete a checkout
that moved under the pointer while the list changed. Templates could overwrite a name an agent or the command line took
after the dialog opened. A number-key save whose reply was lost could show the old assignment even though the helm had
saved the new one. The host setting for running commands without asking omitted deleting from its explanation. An
untouched Settings save could restore an old checkout folder or post-clone command over a command-line change.

The stack pauses individual trash deletes for 700 ms after layout changes and while a bulk-delete confirmation is open;
rechecks new template names before writing; reports uncertain number-key saves and re-reads their bindings; names
deleting in the host setting's help; and makes untouched checkout Settings saves write nothing.

## Things you should know

The number-key fix follows your smaller-fix decision: even an explicit refusal gets “may not have been saved.” If the
re-read also fails, the dialog says current keys could not be read and shows its previous observation. A timed-out write
can still reach the helm after that read; the dialog never retries it automatically. The requirement to distinguish a
refusal from an unknown outcome is recorded as dropped.

The fixes measured 123, 39, 64, 28 and 54 changed lines respectively, excluding queue bookkeeping and changelog
fragments. The earlier executor treated the trash fix's 123 lines as within “about 100”; this report preserves that
judgment explicitly. The number-key fix stays within the gate and changes no shared save-queue behavior. The existing
fragment for failed number-key saves was updated rather than duplicated.

## Open questions and possible follow-ups

No decision is needed to finish this plan. The trash browser test covers a click after rows move; it does not separately
test disabling entry deletes while a confirmation is open. If renaming a host wraps its trash heading onto another line,
delete buttons below could move without starting the pause; this is a code-reading possibility, not a browser
reproduction. The trash pause and the edited-only Settings save are documented in code but could also be added to the
implementation specification.

Renaming a template has a separate lost-reply ambiguity when moving its shortcuts: the old name is kept, but the move
may already have reached the helm. That path is unchanged. The commands-setting explanation still over-warns slightly:
launches and template writes with an agent-supplied command prefix require an explicit Allow despite the setting; the
specification's summary also names prompting, which is disabled in this version. These were retained as follow-ups
rather than expanding the accepted outcomes.

## The PRs

- [#2651: pause trash entry deletes after the list moves](https://github.com/scode/farhelm/pull/2651/changes).
- [#2658: recheck template names before saving](https://github.com/scode/farhelm/pull/2658/changes).
- [#2679: report uncertain number-key saves](https://github.com/scode/farhelm/pull/2679/changes).
- [#2661: say the commands setting covers deleting](https://github.com/scode/farhelm/pull/2661/changes).
- [#2663: make untouched checkout Settings saves write nothing](https://github.com/scode/farhelm/pull/2663/changes).

## Checks run, reused and skipped

- Ran `cargo build` and `dx build --package farhelm-ui --platform web --release` with Dioxus 0.7.10 to prepare the real
  browser stacks.
- Ran the preference-queue and template-binding unit selection through the recorder:
  `cargo nextest run -p farhelm-ui --lib -E 'test(preference_write_tests::) | test(list::templates::tests::)'`. Run
  `9a8fc7dd-734a-41ac-aed2-cb34187a1c3c`: 43 passed, 711 selected out, complete report and no runtime substrate skip.
  This checks that the existing queue and binding rules still hold.
- Ran the full Templates spec through the recorder: `npx playwright test 'templates\.spec\.ts'`. Run
  `698d6a95-c25e-4748-9472-e29b6a4bc20d`: 53 Chromium and 53 WebKit tests passed, no skips or failures, complete report.
  It includes refusal, a real committed save whose reply is aborted, and a failed follow-up read, plus the template-name
  collision regression. The browser build contains the new number-key fix; the later help and checkout Settings PRs
  leave Templates unchanged.
- Ran `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check` and
  `cargo check -p farhelm-ui --features desktop` successfully at the combined stack tip. Markdown formatting,
  changelog-fragment format and the isolated `python -B scripts/check-test-sleeps.py` check passed. The sleep check
  covered 314 delay calls; the only subsequent test-source edit changed an assertion's explanation.
  `python3 scripts/check-agents-md.py` passed with one existing warning about an unrelated ledger citation.
- Reused `npx playwright test 'trash-dialog\.spec\.ts'` on Chromium and WebKit, run
  `52ec2bf4-22b6-41bb-be76-79c08001ee72`: 22 passed across both engines. It covers base `07f16529` plus the recorder's
  PR-1 working diff, subsequently pushed as `eda17e8e`. Reused `npx playwright test 'settings\.spec\.ts'` on Chromium
  and WebKit, run `c547d214-dd91-4c48-86f6-02c11d0a7b7b`: 42 passed across both engines, covering parent `15e77aca` plus
  the PR-5 working diff subsequently pushed as `b30d9487`. Both retained manifests are complete and exited zero. Those
  product/test patches and the relevant fixture contracts are unchanged after rebasing; upstream's remembered launcher
  folder uses launch history rather than either checkout Settings field. The earlier trash negative check
  `1d07aa13-a5db-420d-957a-f1199c44e4d1` is retained as an expected failure against old code, not counted as a pass.
- Skipped the full Rust and browser batteries and installer/provisioning tests: this stack changes dialog behavior, and
  the focused browser cases, existing unchanged checks and desktop compilation cover its identified risks. The help-only
  change needs no separate runtime test. No test from this resumed round failed.

## Review gate's outcome

Each outcome received the required single fresh-context Opus 5.5 high review. The earlier round addressed the trash
timing test's readiness race, the template-name test's ambiguous assertion and the Settings test's disabled-button
interaction. The resumed number-key review found no correctness defect; its test assertion explanation was corrected,
and the remaining style decisions were recorded in the working log. Independent native checks reconciled the resume
state and examined upstream rebase interactions. Product/test patches in the four existing PRs are unchanged after the
rebase; feedback-index resolutions preserve upstream removals and this plan's removals.

### Landing

Landed as #2651, #2658, #2679, #2661 and #2663, merged bottom-up, with the content the report describes and one
changelog fix described below. The number-key fix (#2679) is the smaller one you chose on 2026-10-23. It was the second
of three plans in this landing round, after update-refuses-newer-host-schema and before build-and-tooling-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained launch-refusal-visibility (the launcher's refusal alert and folder warning); the
recent launcher work (per-host-last-folder) and the failed-save rule (sign-in-preservation-sweep) were already in its
base. A fresh-context reviewer and I read this plan against them and against the other two plans in the round:

- The number-key fix matches the 2026-10-22 rule: a failed save is reported, its value discarded and never retried, and
  the dialog shows the keys the helm reports. The shared save queue is unchanged.
- Neither launch-refusal-visibility nor per-host-last-folder touched the Templates dialog; the new re-read of template
  names before saving is ordered the same way the dialog's Undo already is.

#### Fixes made during the landing

- #2663 makes Settings' checkout folder and post-clone command savable only after an edit. That replaces an unreleased
  change from #2397, whose changelog entry still said saving an unchanged checkout folder keeps its exact name. The
  landing folded the two into #2397's entry, describing the final behavior, and dropped #2663's separate entry, in
  #2663.

#### Possible follow-ups

- Found at landing: SPEC_impl.md does not yet describe the Templates dialog's re-read of names before saving; as the
  report notes, it also does not describe the pause before a trash delete settles or Settings saving only an edited
  checkout folder or post-clone command.

#### Checks

This round landed three plans you unblocked on 2026-10-23: the Update refusal for a host whose database is newer
(update-refuses-newer-host-schema), the dialog save fixes (dialog-saves-oct22) and the build and tooling fixes
(build-and-tooling-oct22). Their executors ran selected tests; this round ran, on all three stacked together on this
round's main, before the one landing fix (which rewrote one changelog fragment and deleted another):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (the devbuild instructions changed; one warning, about a citation main already
  had), `sh -n scripts/install.sh`, `bash -n scripts/devbuild-mac.sh`, `shellcheck -x --severity=warning` on the five
  changed shell scripts, the workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`:
  all passed.
- Through the test recorder: `bash scripts/test-install-sh.sh`, run `7d6f72b3-6747-4db4-a0d5-c89dce396143`, 557 checks,
  none failed; `python3 scripts/test-plans-queue.py`, run `14cb5f16-37fc-485b-98f0-aa023c05e6fb`, 64 passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` with pinned nextest and tmux, four slots, no retries, run
  `709a1d66-f15c-4a4c-aee6-87068ba2ae6c`: 4328 of 4329 passed, 40 skipped by design (helper entries that only run as
  child processes of other tests, and tests that need a real signed-in Claude or Codex). It covers the connection field,
  the Update refusal, the hidden verification command and the GUI's unit tests. The one failure,
  `api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`, is the known open flake already in TODO.md's
  Deflake list; none of these plans touches the code it tests. `cargo nextest run -p farhelm-ui --features desktop`,
  same settings without tmux, run `e13aca7f-da53-4e38-8b75-8de136a7e174`: 920 passed. The asset script tests
  (`node --test` in `crates/farhelm-ui/js-tests`), run `12ed0e03-0b32-47e9-ba61-2f30426d626e`: 333 passed.
- `npx playwright test` in `e2e/` on Chromium and WebKit for `settings`, `templates`, `trash-dialog`,
  `launcher-template-shortcuts`, `quick-launch` and `provisioning`, three workers each with its own helm and
  supervisors, no retries, run `c386c83f-760d-4531-8085-d400657b6611`: 429 passed, 3 failed. Two are open flakes
  recorded in draft PRs that have not merged yet: the WebKit template-shortcut editor-focus test (#2621) and WebKit's
  "offer retirement ... waiting behind another host" (#2673). The third, Chromium's "accepted ADD registers before
  execution and releases the page lock while running" in `provisioning.spec.ts`, timed out opening a host's menu while
  that host was being set up, the same kind of failure as #2673. Repeated alone it failed 4 of 10 on this stack (run
  `506088c3-846a-4e83-8e55-ecf65dee16af`) and passed 10 of 10 on main (run `b073dbcc-4dc5-4c11-bd2e-3f1105db0256`), so I
  bisected. The stack below build-and-tooling-oct22 passed 50 repeats while the stack with it kept failing about one run
  in five, even with no binary or web bundle difference between them. But adding to that lower stack just one file
  nothing reads (a copy of the plan's new changelog fragment, or a meaningless file at the top of the checkout) brought
  failures back, the clean lower stack then failed 3 of 20 (run `7a76db98-fce9-4dbf-963b-4b7d9c1434fc`), and main itself
  failed 2 of 20 when run again (run `2a887633-7de3-45d4-a1a7-98c63c9f3758`). So it fails on main too, and I could not
  tie it to any of these plans; why the earlier runs split so cleanly is unknown. It is not in FLAKES.md yet; I will
  record it in a draft PR beside #2673.
- `scripts/test-provision-centos.sh` through the recorder, run `4630d8a9-426c-4d2b-96a2-120586999598`: a CentOS Stream 9
  container was set up over ssh by this round's helm with the release-style static payloads; passed. The
  build-and-tooling-oct22 plan told its executor not to run this script, because it rewrites the real ssh configuration;
  I ran it anyway as this round's check, after copying that configuration aside, and it was unchanged afterwards. It is
  the one test that runs the private tmux on another distribution and edits the ssh configuration the way the changed
  marker handling does; the configuration was unchanged afterwards.
- After the landing fix: `python3 releasing/check-changelog.py format` passed again.
- Reused: none. Skipped: the doctests (no documentation example changed), the desktop smoke test, the native Mac app and
  the macOS-only installed uninstall suite and devbuild script (not runnable here; the installer suite above runs the
  Mac installer against Linux fixtures, and devbuild was checked with its syntax check and shellcheck only).

The report's checks ran the GUI's save queue and template unit tests and the Templates, trash and Settings browser
specs, partly before rebases; this round ran the GUI's whole test suites and those specs on both engines with all three
plans together. Nothing in the executor's report became untrue at landing.
