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
