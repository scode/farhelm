## What this was about

Farhelm could misread Linux service files when setup or uninstall decided which installation a service runs. It treated
unsupported path escapes as different characters and counted lowercase service sections that systemd ignores. A separate
macOS reader could lose a hook’s short command line when the agent had a large environment, preventing the conversation
from becoming available for Resume. The maintainer accepted three small fixes in one PR, with a complexity gate for
anything larger.

## Things you should know

Setup and uninstall now refuse an executable path using escapes Farhelm does not support, rather than inventing a
different path. They still read Farhelm’s own generated files, including quoted paths and literal backslashes. Lowercase
service sections are ignored even when they appear after the real service section.

On macOS, the process read allocates for the whole kernel response, which also contains the executable path and
environment. The existing 64 KiB limit still applies to the arguments themselves. The large-environment failure was
identified by inspection; native compilation passed, but no live macOS reproduction establishes which malformed bytes a
particular kernel returned.

All three fixes stayed within their complexity gates. No outcome was dropped, no new mechanism was added, and no product
decision awaits an answer. The three feedback entries were removed and their execution records identify the same draft
PR.

## Open questions and possible follow-ups

No implementation decision awaits an answer. A live macOS check with a large agent environment could strengthen the
runtime evidence; compilation and the unchanged argument-parser tests do not reproduce the kernel failure itself.

## PRs

- [#1796](https://github.com/scode/farhelm/pull/1796/changes): Linux service ownership and macOS conversation-hook
  argument reading. Draft head `54366c8a382d19f561cf4308947f38f28d7cc73d`.

## Checks run, reused and skipped

Focused nextest through the recorder passed 18/18 tests for service-file ownership and the macOS argument parser, with
2,088 cases selected out: run `66f4e1b2-923a-4a94-915c-fdcfbe4e17c8`, selection
`test(units::) | test(procargs2_argv_parsing)` across the helm and supervisor library targets. A final exact
renderer-readback rerun passed 1/1 with 1,019 selected out: run `837ba2fa-6e4d-404d-8e2a-6b1c0c197b22`. Both used four
nextest slots, zero retries and tmux none; selected cases are pure tests with no early-return substrate gaps.

`cargo clippy -p farhelm-helm -p farhelm-supervisor --all-targets -- -D warnings` passed. The
[focused macOS workflow](https://github.com/scode/farhelm/actions/runs/38062296729) passed on the exact draft head
above, including `cargo build -p farhelm --locked` and native installed-uninstall acceptance. The CLI directly depends
on the supervisor, so this compiled the changed macOS reader. The uninstall suite does not exercise the
large-environment hook failure.

`cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, and
`dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed. The isolated
`python -B scripts/check-test-sleeps.py` inspected 281 delays with zero missing rationales. An initial document check
exposed leftover wrapped lines after feedback-index removal; removing each complete bullet and formatting the execution
records fixed it before the source review completed.

No prior runtime run is reused. The rebase onto main `ee306896` carried only concurrent plan claims and preserved the
reviewed source inputs. Final upstream inspection through `467f2829` found only two plan-report deliveries, with no
source or specification interactions; another rebase or runtime run would add no relevant evidence.

Full workspace tests, browser tests, desktop checks, provisioning, release checks and additional installer runs were
skipped because the changes affect two readers and their pure unit contracts. The focused native macOS workflow is used
for the compile that Linux cannot provide. Doctests were skipped because no executable documentation changed. No website
deployment or release was requested.

## Review gate outcome

The prescribed fresh gpt-6.1-sol reviewer at high effort accepted the narrow source changes with no remaining findings.
It checked generated service-file readback, ignored lowercase sections, refusal of unsupported escapes, and the
unchanged argument budget. It also inspected the corrected feedback-index cleanup. This was a source review, not runtime
evidence.

Commit and PR wording passed a fresh gpt-6.1-sol reader at medium effort after the body was clarified to name
conversation recording needed for Resume. A separate reader opened the diff before its blind phase, reported that
contamination, and supplied no valid cold-read evidence; a fresh reader completed the required phases. A separate fresh
native report cold read passed: the report supplies enough product context and evidence to approve or request follow-up,
and contains no private repository-hygiene violations.

### Landing

Landed on 2026-10-10 (UTC) as #1796. This round landed five triage plans together, in order: untrusted-text-escaping,
git-env-isolation, os-readback-fixes, harness-tooling-fixes and ssh-config-atomic. Each rebased onto main with only
conflicts in the review queue's index, where each plan removes only its own entries. Since their stacks were based, main
gained this day's earlier landings (sounds, file downloads, the reboot follow-up of the supervisor's timer sweep) and
the 2026-10-10 spec triage; of the files these plans touch, only the helm's supervisor client changed upstream (download
routing), away from the log line one of them changes. A separate reviewer read all five against each other and main by
reading the code only, and checked each against its triage decisions and completion criteria.

#### Review before merging

No findings. Unit-file readback decodes only the two escapes Farhelm itself writes and refuses ownership for anything
else, section names match exactly, and the macOS argument read is sized the same way the environment read already was,
while keeping the 64 KiB argument budget. A live macOS reproduction remains unverified, as the report says.

#### Checks

- Run now, on the first four stacked in landing order: `dist generate --check` (the release workflow matches its
  sources), `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the
  supervisor, helm, UI and protocol unit tests in full through the recorder with pinned tmux 3.7c, four slots and no
  retries (run `1b086587`, 2699 of 2699), and on Chromium and WebKit with one worker and no retries the spawn, header,
  readers and change-feed specs (run `50ffb9fe`, 44 passed; the two skipped are the real-Claude spawn cases).
- Run now, after the landing's fixes, with ssh-config-atomic stacked on top:
  `cargo clippy -p farhelm-helm --all-targets`, `shellcheck` on the provisioning script, and the helm's client tests
  including the new log-escaping test (run `0067a921`, 68 of 68). The new test was also seen to fail with the escaping
  removed, then restored.
- Reused from the executors: their focused runs for each fix, the hosted macOS compile of the argument-reading change,
  and the deflake end-to-end evaluation.

Nothing in the report above was made untrue by the landing.
